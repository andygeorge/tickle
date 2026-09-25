// src/main.rs
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command, exit};
use std::time::SystemTime;

#[derive(Debug)]
enum ServiceState {
    Active,
    Inactive,
    Failed,
    Unknown,
}

#[derive(Debug)]
enum RestartStrategy {
    Restart,
    StopStart,
}

#[derive(Debug)]
enum TickleCommand {
    Tickle,
    Start,
    Stop,
    History,
    Completions,
}

struct ServiceManager;

impl ServiceManager {
    fn new() -> Self {
        ServiceManager
    }

    /// Check if systemctl is available
    fn check_systemctl_available(&self) -> Result<(), String> {
        match Command::new("systemctl").arg("--version").output() {
            Ok(_) => Ok(()),
            Err(_) => Err("systemctl is not available. This tool requires systemd.".to_string()),
        }
    }

    /// Get the current state of a service
    fn get_service_state(&self, service_name: &str) -> Result<ServiceState, String> {
        let output = Command::new("systemctl")
            .args(["is-active", service_name])
            .output()
            .map_err(|e| format!("Failed to check service status: {}", e))?;
        let status = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

        match status.as_str() {
            "active" => Ok(ServiceState::Active),
            "inactive" => Ok(ServiceState::Inactive),
            "failed" => Ok(ServiceState::Failed),
            _ => Ok(ServiceState::Unknown),
        }
    }

    /// Check if a service can be restarted (exists and is enabled/available)
    fn can_restart_service(&self, service_name: &str) -> Result<bool, String> {
        // First check if the service unit exists
        let output = Command::new("systemctl")
            .args(["cat", service_name])
            .output()
            .map_err(|e| format!("Failed to check if service exists: {}", e))?;
        if !output.status.success() {
            return Ok(false);
        }

        // Check if restart is supported by looking at the service configuration
        let output = Command::new("systemctl")
            .args(["show", service_name, "--property=CanRestart"])
            .output()
            .map_err(|e| format!("Failed to check restart capability: {}", e))?;
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            if result.contains("CanRestart=yes") {
                return Ok(true);
            }
        }

        // Fallback: try to determine if we can restart based on service type
        let output = Command::new("systemctl")
            .args(["show", service_name, "--property=Type"])
            .output()
            .map_err(|e| format!("Failed to check service type: {}", e))?;
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            // Most service types support restart except oneshot without RemainAfterExit
            if result.contains("Type=oneshot") {
                // Check if RemainAfterExit is set
                let remain_output = Command::new("systemctl")
                    .args(["show", service_name, "--property=RemainAfterExit"])
                    .output()
                    .map_err(|e| format!("Failed to check RemainAfterExit: {}", e))?;

                let remain_result = String::from_utf8_lossy(&remain_output.stdout);
                return Ok(remain_result.contains("RemainAfterExit=yes"));
            }
            return Ok(true);
        }

        // Default to trying restart first
        Ok(true)
    }

    /// Determine the best restart strategy for a service
    fn determine_restart_strategy(&self, service_name: &str) -> Result<RestartStrategy, String> {
        if self.can_restart_service(service_name)? {
            Ok(RestartStrategy::Restart)
        } else {
            Ok(RestartStrategy::StopStart)
        }
    }

    /// Execute systemctl restart
    fn restart_service(&self, service_name: &str) -> Result<(), String> {
        println!("🔄 Attempting to restart {}...", service_name);

        let output = Command::new("systemctl")
            .args(["restart", service_name])
            .output()
            .map_err(|e| format!("Failed to execute restart command: {}", e))?;
        if output.status.success() {
            println!("✅ Successfully restarted {}", service_name);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Restart failed: {}", stderr.trim()))
        }
    }

    /// Execute systemctl stop then start
    fn stop_start_service(&self, service_name: &str) -> Result<(), String> {
        println!("🛑 Stopping {}...", service_name);

        let stop_output = Command::new("systemctl")
            .args(["stop", service_name])
            .output()
            .map_err(|e| format!("Failed to execute stop command: {}", e))?;
        if !stop_output.status.success() {
            let stderr = String::from_utf8_lossy(&stop_output.stderr);
            return Err(format!("Stop failed: {}", stderr.trim()));
        }
        println!("▶️ Starting {}...", service_name);

        let start_output = Command::new("systemctl")
            .args(["start", service_name])
            .output()
            .map_err(|e| format!("Failed to execute start command: {}", e))?;
        if start_output.status.success() {
            println!("✅ Successfully stopped and started {}", service_name);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&start_output.stderr);
            Err(format!("Start failed: {}", stderr.trim()))
        }
    }

    /// Start a systemd service
    fn start_service(&self, service_name: &str) -> Result<(), String> {
        println!("▶️ Starting {}...", service_name);

        let output = Command::new("systemctl")
            .args(["start", service_name])
            .output()
            .map_err(|e| format!("Failed to execute start command: {}", e))?;

        if output.status.success() {
            println!("✅ Successfully started {}", service_name);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Start failed: {}", stderr.trim()))
        }
    }

    /// Stop a systemd service
    fn stop_service(&self, service_name: &str) -> Result<(), String> {
        println!("🛑 Stopping {}...", service_name);

        let output = Command::new("systemctl")
            .args(["stop", service_name])
            .output()
            .map_err(|e| format!("Failed to execute stop command: {}", e))?;

        if output.status.success() {
            println!("✅ Successfully stopped {}", service_name);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Stop failed: {}", stderr.trim()))
        }
    }

    /// Main tickle operation
    fn tickle_service(&self, service_name: &str, force_stop_start: bool) -> Result<(), String> {
        self.check_systemctl_available()?;

        // Get current service state
        let state = self.get_service_state(service_name)?;
        println!("📊 Current state of {}: {:?}", service_name, state);

        let strategy = if force_stop_start {
            RestartStrategy::StopStart
        } else {
            self.determine_restart_strategy(service_name)?
        };
        println!("🎯 Using strategy: {:?}", strategy);

        match strategy {
            RestartStrategy::Restart => self.restart_service(service_name),
            RestartStrategy::StopStart => self.stop_start_service(service_name),
        }
    }
}

/* ------------------ History management ------------------ */

struct HistoryEntry {
    timestamp: String,
    command: String,
    target: String,
    success: bool,
}

impl HistoryEntry {
    fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.splitn(4, " | ").collect();
        if parts.len() != 4 {
            return None;
        }
        Some(HistoryEntry {
            timestamp: parts[0].trim().to_string(),
            command: parts[1].trim().to_string(),
            target: parts[2].trim().to_string(),
            success: parts[3].trim() == "SUCCESS",
        })
    }
}

/// Convert seconds since Unix epoch to an approximate YYYY-MM-DD string
/// using the same formula as get_timestamp(), so log entries match.
fn approx_date_for_secs(secs: u64) -> String {
    let days_since_epoch = secs / 86400;
    let years = days_since_epoch / 365;
    let remaining_days = days_since_epoch % 365;
    let year = 1970 + years;
    let month = (remaining_days / 30) + 1;
    let day = (remaining_days % 30) + 1;
    format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
}

struct HistoryManager {
    history_dir: PathBuf,
    history_file: PathBuf,
}

impl HistoryManager {
    fn new() -> Result<Self, String> {
        let home_dir =
            env::var("HOME").map_err(|_| "Could not determine HOME directory".to_string())?;

        let history_dir = PathBuf::from(home_dir).join(".tickle");
        let history_file = history_dir.join("history.log");

        Ok(HistoryManager {
            history_dir,
            history_file,
        })
    }

    /// Ensure the history directory exists
    fn ensure_directory(&self) -> Result<(), String> {
        if !self.history_dir.exists() {
            fs::create_dir_all(&self.history_dir)
                .map_err(|e| format!("Failed to create history directory: {}", e))?;
        }
        Ok(())
    }

    /// Get a formatted timestamp without external dependencies
    fn get_timestamp() -> String {
        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs();
                // Convert to a basic date/time format manually
                // This is approximate but works without dependencies
                let days_since_epoch = secs / 86400;
                let time_of_day = secs % 86400;
                let hours = time_of_day / 3600;
                let minutes = (time_of_day % 3600) / 60;
                let seconds = time_of_day % 60;

                // Approximate year (starting from 1970)
                let years = days_since_epoch / 365;
                let remaining_days = days_since_epoch % 365;
                let year = 1970 + years;

                // Rough month/day (not accounting for leap years perfectly, but close enough)
                let month = (remaining_days / 30) + 1;
                let day = (remaining_days % 30) + 1;

                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    year,
                    month.min(12),
                    day.min(31),
                    hours,
                    minutes,
                    seconds
                )
            }
            Err(_) => String::from("unknown-time"),
        }
    }

    /// Log a command execution to history
    fn log_command(&self, command: &str, target: &str, success: bool) -> Result<(), String> {
        self.ensure_directory()?;

        let timestamp = Self::get_timestamp();
        let status = if success { "SUCCESS" } else { "FAILED" };
        let log_entry = format!("{} | {} | {} | {}\n", timestamp, command, target, status);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)
            .map_err(|e| format!("Failed to open history file: {}", e))?;

        file.write_all(log_entry.as_bytes())
            .map_err(|e| format!("Failed to write to history file: {}", e))?;

        Ok(())
    }

    /// Display the history
    fn show_history(&self, lines: Option<usize>) -> Result<(), String> {
        if !self.history_file.exists() {
            println!("📜 No history found. Start using tickle to build your history!");
            return Ok(());
        }

        let contents = fs::read_to_string(&self.history_file)
            .map_err(|e| format!("Failed to read history file: {}", e))?;

        let all_lines: Vec<&str> = contents.lines().collect();

        if all_lines.is_empty() {
            println!("📜 History file is empty.");
            return Ok(());
        }

        println!("📜 Tickle History ({})\n", self.history_file.display());
        println!(
            "{:<20} | {:<10} | {:<20} | {:<10}",
            "Timestamp", "Command", "Target", "Status"
        );
        println!("{}", "-".repeat(70));

        let lines_to_show = match lines {
            Some(n) => {
                let start = if all_lines.len() > n {
                    all_lines.len() - n
                } else {
                    0
                };
                &all_lines[start..]
            }
            None => &all_lines[..],
        };

        for line in lines_to_show {
            println!("{}", line);
        }

        println!("\nTotal entries: {}", all_lines.len());
        Ok(())
    }

    /// Clear the history
    fn clear_history(&self) -> Result<(), String> {
        if self.history_file.exists() {
            fs::remove_file(&self.history_file)
                .map_err(|e| format!("Failed to clear history: {}", e))?;
            println!("🗑️  History cleared successfully.");
        } else {
            println!("📜 No history file to clear.");
        }
        Ok(())
    }

    /// Display statistics derived from history
    fn show_stats(&self) -> Result<(), String> {
        if !self.history_file.exists() {
            println!("📊 No history found. Start using tickle to build your stats!");
            return Ok(());
        }

        let contents = fs::read_to_string(&self.history_file)
            .map_err(|e| format!("Failed to read history file: {}", e))?;

        let entries: Vec<HistoryEntry> = contents.lines().filter_map(HistoryEntry::parse).collect();

        if entries.is_empty() {
            println!("📊 No history entries to analyze.");
            return Ok(());
        }

        let total = entries.len();
        let successes = entries.iter().filter(|e| e.success).count();
        let failures = total - successes;
        let success_pct = (successes * 100) / total;
        let failure_pct = 100 - success_pct;

        let sep = "=".repeat(50);
        println!("📊 Tickle History Statistics");
        println!("{}", sep);
        println!();

        // Overview
        println!("📈 Overview");
        println!("  Total operations:  {}", total);
        println!("  Successes:         {}  ({}%)", successes, success_pct);
        println!("  Failures:          {}  ({}%)", failures, failure_pct);
        println!();

        // Per-command breakdown
        let tickle_count = entries.iter().filter(|e| e.command == "tickle").count();
        let start_count = entries.iter().filter(|e| e.command == "start").count();
        let stop_count = entries.iter().filter(|e| e.command == "stop").count();

        println!("🎯 Command Breakdown");
        println!("  tickle:  {}", tickle_count);
        println!("  start:   {}", start_count);
        println!("  stop:    {}", stop_count);
        println!();

        // Most-tickled services (top 5)
        let mut service_counts: HashMap<&str, usize> = HashMap::new();
        for entry in &entries {
            *service_counts.entry(entry.target.as_str()).or_insert(0) += 1;
        }
        let mut sorted_services: Vec<(&str, usize)> = service_counts.into_iter().collect();
        sorted_services.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

        println!("🏆 Most-Tickled Services (Top 5)");
        for (i, (service, count)) in sorted_services.iter().take(5).enumerate() {
            println!("  {}. {} ({})", i + 1, service, count);
        }
        println!();

        // Recent activity (last 7 days)
        let now_secs = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_) => 0,
        };

        println!("📅 Recent Activity (Last 7 Days)");
        for days_ago in (0u64..7).rev() {
            let target_secs = now_secs.saturating_sub(days_ago * 86400);
            let date_str = approx_date_for_secs(target_secs);
            let day_count = entries
                .iter()
                .filter(|e| e.timestamp.starts_with(&date_str))
                .count();
            let label = if days_ago == 0 {
                " (today)".to_string()
            } else if days_ago == 1 {
                " (yesterday)".to_string()
            } else {
                format!(" ({} days ago)", days_ago)
            };
            println!("  {}{}: {}", date_str, label, day_count);
        }
        println!();

        // Longest streak of successes
        let mut current_streak: usize = 0;
        let mut max_streak: usize = 0;
        for entry in &entries {
            if entry.success {
                current_streak += 1;
                if current_streak > max_streak {
                    max_streak = current_streak;
                }
            } else {
                current_streak = 0;
            }
        }
        println!("🔥 Longest Success Streak: {}", max_streak);
        if current_streak > 0 && current_streak == max_streak {
            println!("   (current streak)");
        } else if current_streak > 0 {
            println!("   Current streak: {}", current_streak);
        }

        Ok(())
    }
}

/* ------------------ Compose projects (label discovery) ------------------ */

const LABEL_PROJECT: &str = "com.docker.compose.project";
const LABEL_CONFIG_FILES: &str = "com.docker.compose.project.config_files";
const LABEL_WORKING_DIR: &str = "com.docker.compose.project.working_dir";

#[derive(Debug, Clone, PartialEq)]
struct ComposeProject {
    name: String,
    working_dir: String,
    config_files: Vec<String>,
}

impl ComposeProject {
    /// Compose CLI arguments that pin an operation to this project, from anywhere.
    fn compose_args(&self) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            self.name.clone(),
            "--project-directory".to_string(),
            self.working_dir.clone(),
        ];
        for file in &self.config_files {
            args.push("-f".to_string());
            args.push(file.clone());
        }
        args
    }

    /// Config files the labels pointed at that are not actually on disk. Compose
    /// labels can hold values no local path resolution can satisfy — `-` for stdin,
    /// a remote URL, a path from another host — and those must not reach the cache.
    fn missing_config_files(&self) -> Vec<String> {
        self.config_files
            .iter()
            .filter(|f| !Path::new(f).exists())
            .cloned()
            .collect()
    }

    fn to_cache_line(&self) -> String {
        format!(
            "{}\t{}\t{}",
            self.name,
            self.working_dir,
            self.config_files.join(",")
        )
    }

    fn from_cache_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 3 {
            return None;
        }
        let config_files: Vec<String> = parts[2]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if parts[0].is_empty() || config_files.is_empty() {
            return None;
        }
        Some(ComposeProject {
            name: parts[0].to_string(),
            working_dir: parts[1].to_string(),
            config_files,
        })
    }
}

/// Parse `docker ps` label output — `<config_files>\t<working_dir>` — into a project.
/// Every container of a project carries the same labels, so only the first line matters.
fn parse_project_labels(name: &str, stdout: &str) -> Option<ComposeProject> {
    let line = stdout.lines().find(|l| !l.trim().is_empty())?;
    let (raw_files, working_dir) = line.split_once('\t')?;
    let working_dir = working_dir.trim();

    let mut config_files = Vec::new();
    for file in raw_files.split(',').map(str::trim).filter(|f| !f.is_empty()) {
        if Path::new(file).is_absolute() {
            config_files.push(file.to_string());
        } else if working_dir.is_empty() {
            return None;
        } else {
            config_files.push(
                Path::new(working_dir)
                    .join(file)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    if config_files.is_empty() {
        return None;
    }

    Some(ComposeProject {
        name: name.to_string(),
        working_dir: working_dir.to_string(),
        config_files,
    })
}

/// Parse `docker ps` project-name output into a sorted, de-duplicated list.
fn parse_project_names(stdout: &str) -> Vec<String> {
    let mut names: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Remembers where each compose project lives, so a project stays reachable after
/// `down` removes the containers that carried its labels.
struct ProjectCache {
    cache_file: PathBuf,
}

impl ProjectCache {
    fn new() -> Result<Self, String> {
        let home_dir =
            env::var("HOME").map_err(|_| "Could not determine HOME directory".to_string())?;
        Ok(Self::new_in(
            PathBuf::from(home_dir).join(".tickle").join("projects.tsv"),
        ))
    }

    fn new_in(cache_file: PathBuf) -> Self {
        ProjectCache { cache_file }
    }

    fn entries(&self) -> Vec<ComposeProject> {
        fs::read_to_string(&self.cache_file)
            .map(|contents| {
                contents
                    .lines()
                    .filter_map(ComposeProject::from_cache_line)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn store(&self, project: &ComposeProject) -> Result<(), String> {
        if let Some(parent) = self.cache_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }

        let mut kept: Vec<ComposeProject> = self
            .entries()
            .into_iter()
            .filter(|e| e.name != project.name)
            .collect();
        kept.push(project.clone());

        let body: String = kept
            .iter()
            .map(|e| format!("{}\n", e.to_cache_line()))
            .collect();

        // Write-then-rename: a concurrent tickle must never observe a truncated cache.
        let temp_file = self.cache_file.with_extension(format!("tmp{}", process::id()));
        fs::write(&temp_file, body).map_err(|e| format!("Failed to write project cache: {}", e))?;
        fs::rename(&temp_file, &self.cache_file).map_err(|e| {
            fs::remove_file(&temp_file).ok();
            format!("Failed to replace project cache: {}", e)
        })
    }

    fn lookup(&self, name: &str) -> Option<ComposeProject> {
        self.entries().into_iter().find(|e| {
            e.name == name && e.config_files.iter().all(|f| Path::new(f).exists())
        })
    }
}

/// Ask docker for the compose labels of any container belonging to `name`.
fn query_project_labels(name: &str) -> Result<String, String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("label={}={}", LABEL_PROJECT, name),
            "--format",
            &format!(
                "{{{{.Label \"{}\"}}}}\t{{{{.Label \"{}\"}}}}",
                LABEL_CONFIG_FILES, LABEL_WORKING_DIR
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to run docker: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker ps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Every compose project docker currently knows about, running or not.
fn known_project_names() -> Vec<String> {
    Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("label={}", LABEL_PROJECT),
            "--format",
            &format!("{{{{.Label \"{}\"}}}}", LABEL_PROJECT),
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_project_names(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Render a project list for an error message, capped so a busy host does not
/// bury the actual error under a wall of names.
fn summarize_project_names(names: &[String]) -> String {
    const MAX_SHOWN: usize = 10;
    let shown = names.iter().take(MAX_SHOWN).cloned().collect::<Vec<_>>();
    let summary = shown.join(", ");
    if names.len() > MAX_SHOWN {
        format!("{} (and {} more)", summary, names.len() - MAX_SHOWN)
    } else {
        summary
    }
}

/// Locate a compose project by label, falling back to the cache once `down` has
/// removed the labelled containers.
fn resolve_compose_project(name: &str, cache: &ProjectCache) -> Result<ComposeProject, String> {
    // A docker failure is not fatal: the cache may still know where the project
    // lives, and compose itself may be reachable through the legacy CLI.
    let labels = query_project_labels(name).unwrap_or_else(|e| {
        eprintln!("⚠️  Warning: {}", e);
        String::new()
    });
    let labelled = parse_project_labels(name, &labels);

    if let Some(project) = &labelled {
        let missing = project.missing_config_files();
        if missing.is_empty() {
            if let Err(e) = cache.store(project) {
                eprintln!("⚠️  Warning: Failed to cache project location: {}", e);
            }
            return Ok(project.clone());
        }
        return Err(format!(
            "Project '{}' is labelled with compose files that are not on this host: {}",
            name,
            missing.join(", ")
        ));
    }

    if let Some(project) = cache.lookup(name) {
        println!(
            "💾 No containers found for '{}'; using cached location.",
            name
        );
        return Ok(project);
    }

    if !labels.trim().is_empty() {
        return Err(format!(
            "Containers for '{}' exist but carry no usable compose file labels.",
            name
        ));
    }

    let known = known_project_names();
    if known.is_empty() {
        Err(format!(
            "No compose project named '{}' found, and docker reports no compose projects at all.",
            name
        ))
    } else {
        Err(format!(
            "No compose project named '{}' found. Known projects: {}",
            name,
            summarize_project_names(&known)
        ))
    }
}

/* ------------------ Compose helpers ------------------ */

/// Return the first compose file found in the CWD, if any.
fn find_compose_file() -> Option<&'static str> {
    // Check common names in a sensible order
    let candidates = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
        "container-compose.yml",
        "container-compose.yaml",
    ];
    candidates
        .into_iter()
        .find(|&name| Path::new(name).exists())
        .map(|v| v as _)
}

/// Whether the `docker compose` plugin is installed, as opposed to the command
/// having failed for a reason worth reporting.
fn compose_plugin_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Try running `docker compose <args...>` first; fall back to `docker-compose <args...>`.
fn run_compose_with_best_cli(args: &[&str]) -> Result<(), String> {
    // Prefer modern `docker compose`
    let try_docker_compose_plugin = Command::new("docker")
        .args(std::iter::once("compose").chain(args.iter().copied()))
        .output();
    if let Ok(out) = try_docker_compose_plugin {
        if out.status.success() {
            return Ok(());
        }
        // Only fall back when the plugin itself is absent. Otherwise this was a real
        // compose failure, and reporting the legacy CLI's error instead would hide it.
        if compose_plugin_available() {
            return Err(format!(
                "Compose command failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }

    // Legacy `docker-compose`
    let legacy = Command::new("docker-compose")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run docker-compose: {}", e))?;
    if legacy.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Compose command failed: {}",
            String::from_utf8_lossy(&legacy.stderr).trim()
        ))
    }
}

/// Run a compose subcommand against a pre-built selector (`-f FILE` or a full project pin).
fn run_compose(selector: &[String], subcommand: &[&str]) -> Result<(), String> {
    let args: Vec<&str> = selector
        .iter()
        .map(String::as_str)
        .chain(subcommand.iter().copied())
        .collect();
    run_compose_with_best_cli(&args)
}

/// Perform `compose down` then `compose up -d` against the selected stack.
fn compose_down_up(selector: &[String], label: &str) -> Result<(), String> {
    println!(
        "🐳 Compose stack: {}. Performing `docker compose down`...",
        label
    );
    run_compose(selector, &["down"])?;
    println!("🚀 Bringing stack back up in detached mode...");
    run_compose(selector, &["up", "-d"])?;
    println!("✅ Compose stack restarted.");
    Ok(())
}

/// Start compose stack
fn compose_start(selector: &[String], label: &str) -> Result<(), String> {
    println!("🐳 Starting compose stack: {}...", label);
    run_compose(selector, &["up", "-d"])?;
    println!("✅ Compose stack started.");
    Ok(())
}

/// Stop compose stack
fn compose_stop(selector: &[String], label: &str) -> Result<(), String> {
    println!("🐳 Stopping compose stack: {}...", label);
    run_compose(selector, &["down"])?;
    println!("✅ Compose stack stopped.");
    Ok(())
}

/* ------------------ Log following ------------------ */

/// Replace the current process with `docker compose <selector> logs -f`.
/// Tries `docker compose` first, falls back to `docker-compose`.
fn follow_compose_logs(selector: &[String]) -> ! {
    println!("📋 Following compose logs (Ctrl+C to stop)...");
    let tail = ["logs", "-f"];
    let args: Vec<&str> = selector
        .iter()
        .map(String::as_str)
        .chain(tail.iter().copied())
        .collect();

    let err = Command::new("docker")
        .args(std::iter::once("compose").chain(args.iter().copied()))
        .exec();
    // exec() only returns on failure — try legacy CLI
    eprintln!(
        "⚠️  docker compose not available ({}), trying docker-compose...",
        err
    );
    let err = Command::new("docker-compose").args(&args).exec();
    eprintln!("❌ Failed to follow logs: {}", err);
    exit(1);
}

/// Replace the current process with `journalctl -f -u SERVICE`.
fn follow_service_logs(service_name: &str) -> ! {
    println!("📋 Following logs for {} (Ctrl+C to stop)...", service_name);
    let err = Command::new("journalctl")
        .args(["-f", "-u", service_name])
        .exec();
    eprintln!("❌ Failed to follow logs: {}", err);
    exit(1);
}

/* ------------------ CLI / UX ------------------ */

fn print_version() {
    println!("tickle {}", env!("CARGO_PKG_VERSION"));
}

fn print_usage() {
    println!("Usage: tickle [COMMAND] [OPTIONS] [service_name ...]");
    println!();
    println!("COMMANDS:");
    println!("  start               Start one or more services or a compose stack");
    println!("  stop                Stop one or more services or a compose stack");
    println!("  history             Show command history");
    println!("  history clear       Clear command history");
    println!("  history stats       Show history statistics");
    println!("  completions bash    Print bash completion script");
    println!("  completions zsh     Print zsh completion script");
    println!("  completions fish    Print fish completion script");
    println!("  (default)           Restart/tickle one or more services or a compose stack");
    println!();
    println!("OPTIONS:");
    println!("  -f, --follow        Follow logs after the operation completes");
    println!("  -s, --stop-start    Force stop/start instead of restart (tickle only)");
    println!("  -p, --project <p>   Act on a running compose project by name, from anywhere");
    println!("  -n <lines>          Show last N lines of history (with history command)");
    println!("  -v, --version       Show version information");
    println!("  -h, --help          Show this help message");
    println!();
    println!("Behavior:");
    println!("  • If run in a directory containing a compose file (docker-compose.yml/.yaml,");
    println!("    compose.yml/.yaml, container-compose.yml/.yaml) and no <service_name> is");
    println!("    provided, tickle will operate on the compose stack:");
    println!("        tickle          -> docker compose down && docker compose up -d");
    println!("        tickle start    -> docker compose up -d");
    println!("        tickle stop     -> docker compose down");
    println!();
    println!("  • With --project, tickle finds the stack by its");
    println!("    com.docker.compose.project label, so you can run it from any directory:");
    println!("        tickle -p paperless        -> down && up -d that project");
    println!("        tickle start -p paperless  -> up -d that project");
    println!("        tickle stop -p paperless   -> down that project");
    println!("    The project location is remembered in ~/.tickle/projects.tsv, so a");
    println!("    stopped stack stays reachable after its containers are removed.");
    println!();
    println!("  • Otherwise, tickle will operate on the named systemd service(s):");
    println!("        tickle nginx             -> restart nginx");
    println!("        tickle start nginx       -> start nginx");
    println!("        tickle stop nginx        -> stop nginx");
    println!("        tickle nginx postgresql  -> restart both in sequence, then summarize");
    println!();
    println!("  • When multiple services are given, each is logged separately in history.");
    println!("    A summary is printed at the end; exit code is non-zero if any failed.");
    println!();
    println!("  • History is stored in ~/.tickle/history.log");
    println!();
    println!("Shell Completions:");
    println!("  • Bash:  eval \"$(tickle completions bash)\"");
    println!("  • Zsh:   eval \"$(tickle completions zsh)\"");
    println!("  • Fish:  tickle completions fish | source");
    println!();
    println!("Examples:");
    println!("  tickle nginx");
    println!("  tickle nginx postgresql redis    # stack tickle — restart all three");
    println!("  tickle start nginx postgresql    # start both services");
    println!("  tickle stop nginx postgresql     # stop both services");
    println!("  tickle start apache2");
    println!("  tickle stop postgresql");
    println!("  tickle --stop-start apache2");
    println!("  tickle -p paperless         # restart a compose project from anywhere");
    println!("  tickle stop -p paperless    # stop that project");
    println!("  tickle -f -p paperless      # restart it, then follow its logs");
    println!("  tickle history              # Show full history");
    println!("  tickle history -n 10        # Show last 10 entries");
    println!("  tickle history clear        # Clear all history");
    println!("  tickle history stats        # Show history statistics");
    println!("  tickle start                # in a compose project directory");
    println!("  tickle stop                 # in a compose project directory");
    println!("  tickle                      # in a compose project directory");
    println!("  tickle -f nginx             # restart nginx then follow journalctl");
    println!("  tickle -f                   # restart compose stack then follow logs");
    println!("  tickle completions bash     # print bash completion script");
    println!("  tickle completions zsh      # print zsh completion script");
    println!("  tickle completions fish     # print fish completion script");
}

/// Parse command from arguments
fn parse_command(args: &[String]) -> TickleCommand {
    if args.len() > 1 {
        match args[1].as_str() {
            "start" => TickleCommand::Start,
            "stop" => TickleCommand::Stop,
            "history" => TickleCommand::History,
            "completions" => TickleCommand::Completions,
            _ => TickleCommand::Tickle,
        }
    } else {
        TickleCommand::Tickle
    }
}

/* ------------------ Shell completions ------------------ */

fn print_bash_completions() {
    print!("{}", r#"# tickle bash completion
# Source this file or add to ~/.bashrc:
#   eval "$(tickle completions bash)"

_tickle_compose_projects() {
    docker ps -a --filter label=com.docker.compose.project \
        --format '{{.Label "com.docker.compose.project"}}' 2>/dev/null | sort -u
}

_tickle_completions() {
    local cur prev words cword
    _init_completion 2>/dev/null || {
        COMPREPLY=()
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
        words=("${COMP_WORDS[@]}")
        cword=$COMP_CWORD
    }

    local subcommands="start stop history completions"
    local flags="-f --follow -s --stop-start -p --project -h --help -v --version"

    # A compose project name always follows -p/--project
    if [[ "$prev" == "-p" || "$prev" == "--project" ]]; then
        mapfile -t COMPREPLY < <(compgen -W "$(_tickle_compose_projects)" -- "$cur")
        return
    fi

    # --project cannot be combined with service names, so offer only flags after it
    local word
    for word in "${words[@]:1}"; do
        if [[ "$word" == "-p" || "$word" == "--project" ]]; then
            mapfile -t COMPREPLY < <(compgen -W "-f --follow -s --stop-start" -- "$cur")
            return
        fi
    done

    # Handle subcommand-specific completions
    case "${words[1]}" in
        completions)
            mapfile -t COMPREPLY < <(compgen -W "bash zsh fish" -- "$cur")
            return
            ;;
        history)
            mapfile -t COMPREPLY < <(compgen -W "clear stats" -- "$cur")
            return
            ;;
        start|stop)
            # Complete service names for start/stop
            local services
            services=$(systemctl list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
                | awk '{print $1}' | sed 's/\.service$//')
            local user_services
            user_services=$(systemctl --user list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
                | awk '{print $1}' | sed 's/\.service$//')
            local compose_services=""
            for f in docker-compose.yml docker-compose.yaml compose.yml compose.yaml container-compose.yml container-compose.yaml; do
                if [[ -f "$f" ]]; then
                    compose_services=$(docker compose config --services 2>/dev/null || docker-compose config --services 2>/dev/null || true)
                    break
                fi
            done
            mapfile -t COMPREPLY < <(compgen -W "$services $user_services $compose_services" -- "$cur")
            return
            ;;
    esac

    # First word after tickle: offer subcommands, flags, and service names
    if [[ $cword -eq 1 ]]; then
        if [[ "$cur" == -* ]]; then
            mapfile -t COMPREPLY < <(compgen -W "$flags" -- "$cur")
        else
            local services
            services=$(systemctl list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
                | awk '{print $1}' | sed 's/\.service$//')
            local user_services
            user_services=$(systemctl --user list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
                | awk '{print $1}' | sed 's/\.service$//')
            local compose_services=""
            for f in docker-compose.yml docker-compose.yaml compose.yml compose.yaml container-compose.yml container-compose.yaml; do
                if [[ -f "$f" ]]; then
                    compose_services=$(docker compose config --services 2>/dev/null || docker-compose config --services 2>/dev/null || true)
                    break
                fi
            done
            mapfile -t COMPREPLY < <(compgen -W "$subcommands $flags $services $user_services $compose_services" -- "$cur")
        fi
        return
    fi

    # After flags like -f/-s, complete service names
    if [[ "$cur" == -* ]]; then
        mapfile -t COMPREPLY < <(compgen -W "$flags" -- "$cur")
    else
        local services
        services=$(systemctl list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
            | awk '{print $1}' | sed 's/\.service$//')
        local user_services
        user_services=$(systemctl --user list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
            | awk '{print $1}' | sed 's/\.service$//')
        local compose_services=""
        for f in docker-compose.yml docker-compose.yaml compose.yml compose.yaml container-compose.yml container-compose.yaml; do
            if [[ -f "$f" ]]; then
                compose_services=$(docker compose config --services 2>/dev/null || docker-compose config --services 2>/dev/null || true)
                break
            fi
        done
        mapfile -t COMPREPLY < <(compgen -W "$services $user_services $compose_services" -- "$cur")
    fi
}

complete -F _tickle_completions tickle
"#);
}

fn print_zsh_completions() {
    print!("{}", r#"#compdef tickle
# tickle zsh completion
# Add to ~/.zshrc:
#   eval "$(tickle completions zsh)"
# Or place this file in a directory on $fpath.

_tickle() {
    local context state state_descr line
    typeset -A opt_args

    _arguments -C \
        '(-h --help)'{-h,--help}'[Show help message]' \
        '(-v --version)'{-v,--version}'[Show version information]' \
        '(-f --follow)'{-f,--follow}'[Follow logs after operation completes]' \
        '(-s --stop-start)'{-s,--stop-start}'[Force stop/start strategy instead of restart]' \
        '(-p --project)'{-p,--project}'[Act on a compose project by name]:project:_tickle_compose_projects' \
        '1: :_tickle_commands' \
        '*: :_tickle_service_args'
}

_tickle_compose_projects() {
    local -a projects
    projects=(${(f)"$(docker ps -a --filter label=com.docker.compose.project \
        --format '{{.Label "com.docker.compose.project"}}' 2>/dev/null | sort -u)"})
    (( ${#projects} )) || return 1
    _wanted projects expl 'compose project' compadd -a projects
}

_tickle_commands() {
    local commands
    commands=(
        'start:Start a service or compose stack'
        'stop:Stop a service or compose stack'
        'history:Show command history'
        'completions:Generate shell completion scripts'
    )
    _describe 'command' commands
}

_tickle_service_args() {
    case "$words[2]" in
        completions)
            local shells=('bash:Bash shell' 'zsh:Zsh shell' 'fish:Fish shell')
            _describe 'shell' shells
            return
            ;;
        history)
            local subcmds=('clear:Clear command history' 'stats:Show history statistics')
            _describe 'subcommand' subcmds
            return
            ;;
    esac
    _tickle_services
}

_tickle_services() {
    local -a services user_services compose_services

    services=(${(f)"$(systemctl list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
        | awk '{print $1}' | sed 's/\.service$//')"})
    user_services=(${(f)"$(systemctl --user list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
        | awk '{print $1}' | sed 's/\.service$//')"})

    local compose_file
    for compose_file in docker-compose.yml docker-compose.yaml compose.yml compose.yaml container-compose.yml container-compose.yaml; do
        if [[ -f "$compose_file" ]]; then
            compose_services=(${(f)"$(docker compose config --services 2>/dev/null || docker-compose config --services 2>/dev/null || true)"})
            break
        fi
    done

    _values 'service' $services $user_services $compose_services
}

_tickle "$@"
"#);
}

fn print_fish_completions() {
    print!("{}", r#"# tickle fish completion
# Add to your fish config or place in ~/.config/fish/completions/tickle.fish:
#   tickle completions fish | source

# Disable file completions for tickle
complete -c tickle -f

# Helper: list loaded systemd services (system + user)
function __tickle_systemd_services
    systemctl list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
        | awk '{print $1}' | sed 's/\.service$//'
    systemctl --user list-units --type=service --state=loaded --no-legend --no-pager 2>/dev/null \
        | awk '{print $1}' | sed 's/\.service$//'
end

# Helper: list compose services if a compose file is present in cwd
function __tickle_compose_services
    set -l compose_files docker-compose.yml docker-compose.yaml compose.yml compose.yaml container-compose.yml container-compose.yaml
    for f in $compose_files
        if test -f $f
            docker compose config --services 2>/dev/null; or docker-compose config --services 2>/dev/null
            return
        end
    end
end

# Helper: list compose project names known to docker, running or not
function __tickle_compose_projects
    docker ps -a --filter label=com.docker.compose.project \
        --format '{{.Label "com.docker.compose.project"}}' 2>/dev/null | sort -u
end

# Helper: true when -p/--project has not been given yet
function __tickle_no_project
    for token in (commandline -opc)
        switch $token
            case -p --project
                return 1
        end
    end
    return 0
end

# Helper: true when no subcommand has been given yet
function __tickle_no_subcommand
    for token in (commandline -opc)[2..]
        switch $token
            case start stop history completions
                return 1
        end
    end
    return 0
end

# Subcommands (only when no subcommand present yet)
complete -c tickle -n __tickle_no_subcommand -a start       -d "Start a service or compose stack"
complete -c tickle -n __tickle_no_subcommand -a stop        -d "Stop a service or compose stack"
complete -c tickle -n __tickle_no_subcommand -a history     -d "Show command history"
complete -c tickle -n __tickle_no_subcommand -a completions -d "Generate shell completion scripts"

# history subcommands
complete -c tickle -n "__fish_seen_subcommand_from history" -a clear -d "Clear command history"
complete -c tickle -n "__fish_seen_subcommand_from history" -a stats -d "Show history statistics"

# completions shells
complete -c tickle -n "__fish_seen_subcommand_from completions" -a bash -d "Bash shell"
complete -c tickle -n "__fish_seen_subcommand_from completions" -a zsh  -d "Zsh shell"
complete -c tickle -n "__fish_seen_subcommand_from completions" -a fish -d "Fish shell"

# Flags (valid outside of history/completions subcommands)
complete -c tickle -n "not __fish_seen_subcommand_from history completions" \
    -s f -l follow      -d "Follow logs after the operation completes"
complete -c tickle -n "not __fish_seen_subcommand_from history completions" \
    -s s -l stop-start  -d "Force stop/start instead of restart"
complete -c tickle -n "not __fish_seen_subcommand_from history completions" \
    -s p -l project -r -f -a "(__tickle_compose_projects)" -d "Compose project"
complete -c tickle -s h -l help    -d "Show help message"
complete -c tickle -s v -l version -d "Show version information"
complete -c tickle -n "__fish_seen_subcommand_from history" \
    -s n -d "Show last N lines of history" -r

# Service names (for tickle, start, stop) — not valid once --project is present
complete -c tickle -n "not __fish_seen_subcommand_from history completions; and __tickle_no_project" \
    -a "(__tickle_systemd_services)" -d "Systemd service"
complete -c tickle -n "not __fish_seen_subcommand_from history completions; and __tickle_no_project" \
    -a "(__tickle_compose_services)" -d "Compose service"
"#);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_absolute_config_files_from_labels() {
        let out = "/srv/paperless/docker-compose.yml\t/srv/paperless\n";
        let project = parse_project_labels("paperless", out).expect("should parse");

        assert_eq!(project.name, "paperless");
        assert_eq!(project.working_dir, "/srv/paperless");
        assert_eq!(
            project.config_files,
            vec!["/srv/paperless/docker-compose.yml".to_string()]
        );
    }

    #[test]
    fn resolves_relative_config_files_against_working_dir() {
        let out = "docker-compose.yml,docker-compose.override.yml\t/srv/paperless\n";
        let project = parse_project_labels("paperless", out).expect("should parse");

        assert_eq!(
            project.config_files,
            vec![
                "/srv/paperless/docker-compose.yml".to_string(),
                "/srv/paperless/docker-compose.override.yml".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_all_but_the_first_container_line() {
        let out = "/srv/a/compose.yml\t/srv/a\n/srv/a/compose.yml\t/srv/a\n";
        let project = parse_project_labels("a", out).expect("should parse");

        assert_eq!(project.config_files.len(), 1);
    }

    #[test]
    fn rejects_labels_with_no_config_files() {
        assert!(parse_project_labels("ghost", "").is_none());
        assert!(parse_project_labels("ghost", "\t/srv/ghost\n").is_none());
    }

    #[test]
    fn rejects_relative_config_files_with_no_working_dir() {
        assert!(parse_project_labels("ghost", "compose.yml\t\n").is_none());
    }

    #[test]
    fn builds_compose_args_with_project_name_and_every_config_file() {
        let project = ComposeProject {
            name: "paperless".to_string(),
            working_dir: "/srv/paperless".to_string(),
            config_files: vec![
                "/srv/paperless/compose.yml".to_string(),
                "/srv/paperless/override.yml".to_string(),
            ],
        };

        assert_eq!(
            project.compose_args(),
            vec![
                "-p",
                "paperless",
                "--project-directory",
                "/srv/paperless",
                "-f",
                "/srv/paperless/compose.yml",
                "-f",
                "/srv/paperless/override.yml",
            ]
        );
    }

    #[test]
    fn parses_unique_sorted_project_names() {
        let out = "beta\nalpha\nbeta\n\nalpha\n";

        assert_eq!(
            parse_project_names(out),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn cache_line_round_trips_a_project() {
        let project = ComposeProject {
            name: "paperless".to_string(),
            working_dir: "/srv/paperless".to_string(),
            config_files: vec![
                "/srv/paperless/compose.yml".to_string(),
                "/srv/paperless/override.yml".to_string(),
            ],
        };

        let restored = ComposeProject::from_cache_line(&project.to_cache_line());

        assert_eq!(restored, Some(project));
    }

    #[test]
    fn rejects_malformed_cache_lines() {
        assert!(ComposeProject::from_cache_line("").is_none());
        assert!(ComposeProject::from_cache_line("paperless\t/srv/paperless").is_none());
    }

    #[test]
    fn reports_config_files_that_are_not_on_disk() {
        let dir = scratch_dir("missing_files");
        let present = project_in(&dir, "paperless");
        let mut absent = present.clone();
        absent
            .config_files
            .push(dir.join("nope.yml").to_string_lossy().into_owned());

        assert!(present.missing_config_files().is_empty());
        assert_eq!(
            absent.missing_config_files(),
            vec![dir.join("nope.yml").to_string_lossy().into_owned()]
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lists_every_project_when_there_are_only_a_few() {
        let names = vec!["alpha".to_string(), "beta".to_string()];

        assert_eq!(summarize_project_names(&names), "alpha, beta");
    }

    #[test]
    fn truncates_a_long_project_list() {
        let names: Vec<String> = (0..15).map(|i| format!("p{:02}", i)).collect();

        assert_eq!(
            summarize_project_names(&names),
            "p00, p01, p02, p03, p04, p05, p06, p07, p08, p09 (and 5 more)"
        );
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("tickle_unit_{}", name));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).expect("failed to create scratch dir");
        dir
    }

    fn project_in(dir: &Path, name: &str) -> ComposeProject {
        let compose_file = dir.join("compose.yml");
        fs::write(&compose_file, "services: {}\n").expect("failed to write compose file");
        ComposeProject {
            name: name.to_string(),
            working_dir: dir.to_string_lossy().into_owned(),
            config_files: vec![compose_file.to_string_lossy().into_owned()],
        }
    }

    #[test]
    fn cache_returns_a_stored_project() {
        let dir = scratch_dir("cache_store");
        let cache = ProjectCache::new_in(dir.join("projects.tsv"));
        let project = project_in(&dir, "paperless");

        cache.store(&project).expect("store should succeed");

        assert_eq!(cache.lookup("paperless"), Some(project));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_keeps_one_entry_per_project_after_restore() {
        let dir = scratch_dir("cache_replace");
        let cache = ProjectCache::new_in(dir.join("projects.tsv"));
        let mut project = project_in(&dir, "paperless");

        cache.store(&project).expect("first store should succeed");
        project.working_dir = dir.join("moved").to_string_lossy().into_owned();
        cache.store(&project).expect("second store should succeed");

        let contents = fs::read_to_string(dir.join("projects.tsv")).expect("cache should exist");
        assert_eq!(contents.lines().count(), 1);
        assert_eq!(cache.lookup("paperless"), Some(project));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_drops_entries_whose_config_file_is_gone() {
        let dir = scratch_dir("cache_stale");
        let cache = ProjectCache::new_in(dir.join("projects.tsv"));
        let project = project_in(&dir, "paperless");
        cache.store(&project).expect("store should succeed");

        fs::remove_file(&project.config_files[0]).expect("failed to remove compose file");

        assert_eq!(cache.lookup("paperless"), None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_returns_nothing_for_an_unknown_project() {
        let dir = scratch_dir("cache_miss");
        let cache = ProjectCache::new_in(dir.join("projects.tsv"));

        assert_eq!(cache.lookup("nope"), None);
        fs::remove_dir_all(&dir).ok();
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let command = parse_command(&args);

    // Handle version and help for any command structure
    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                exit(0);
            }
            "-v" | "--version" => {
                print_version();
                exit(0);
            }
            _ => {}
        }
    }

    // Handle completions command (no history manager needed)
    if matches!(command, TickleCommand::Completions) {
        let shell = args.get(2).map(|s| s.as_str()).unwrap_or("");
        match shell {
            "bash" => print_bash_completions(),
            "zsh" => print_zsh_completions(),
            "fish" => print_fish_completions(),
            "" => {
                eprintln!("❌ Error: Please specify a shell: bash, zsh, or fish");
                eprintln!("   Usage: tickle completions <bash|zsh|fish>");
                exit(1);
            }
            other => {
                eprintln!("❌ Error: Unknown shell '{}'. Supported: bash, zsh, fish", other);
                exit(1);
            }
        }
        exit(0);
    }

    // Initialize history manager
    let history_manager = match HistoryManager::new() {
        Ok(hm) => hm,
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to initialize history: {}", e);
            // Continue without history
            return;
        }
    };

    // Handle history command
    if matches!(command, TickleCommand::History) {
        // Check for subcommand (clear or stats)
        if args.len() > 2 && args[2] == "clear" {
            match history_manager.clear_history() {
                Ok(()) => exit(0),
                Err(e) => {
                    eprintln!("❌ Error: {}", e);
                    exit(1);
                }
            }
        }

        if args.len() > 2 && args[2] == "stats" {
            match history_manager.show_stats() {
                Ok(()) => exit(0),
                Err(e) => {
                    eprintln!("❌ Error: {}", e);
                    exit(1);
                }
            }
        }

        // Check for -n option
        let mut lines_to_show = None;
        let mut i = 2;
        while i < args.len() {
            if args[i] == "-n" && i + 1 < args.len() {
                match args[i + 1].parse::<usize>() {
                    Ok(n) => {
                        lines_to_show = Some(n);
                        break;
                    }
                    Err(_) => {
                        eprintln!("❌ Error: Invalid number for -n option");
                        exit(1);
                    }
                }
            }
            i += 1;
        }

        match history_manager.show_history(lines_to_show) {
            Ok(()) => exit(0),
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                exit(1);
            }
        }
    }

    // Determine if we have service names and parse other options
    let mut force_stop_start = false;
    let mut follow = false;
    let mut project_name: Option<String> = None;
    let mut service_names: Vec<String> = Vec::new();
    let start_index = match command {
        TickleCommand::Start | TickleCommand::Stop => 2, // Skip "tickle" and "start"/"stop"
        TickleCommand::Tickle => 1,                      // Skip just "tickle"
        TickleCommand::History | TickleCommand::Completions => unreachable!(), // Already handled above
    };

    // Parse remaining arguments — collect all non-flag tokens as service names
    let mut i = start_index;
    while i < args.len() {
        match args[i].as_str() {
            "-f" | "--follow" => {
                follow = true;
            }
            "-s" | "--stop-start" => {
                if matches!(command, TickleCommand::Tickle) {
                    force_stop_start = true;
                } else {
                    eprintln!("❌ Error: --stop-start option only valid with tickle command");
                    exit(1);
                }
            }
            "-p" | "--project" => {
                i += 1;
                match args.get(i) {
                    Some(name) if !name.is_empty() && !name.starts_with('-') => {
                        project_name = Some(name.clone())
                    }
                    _ => {
                        eprintln!("❌ Error: --project requires a compose project name");
                        exit(1);
                    }
                }
            }
            arg if !arg.starts_with('-') => {
                service_names.push(arg.to_string());
            }
            _ => {
                eprintln!("❌ Error: Unknown option: {}", args[i]);
                print_usage();
                exit(1);
            }
        }
        i += 1;
    }

    if project_name.is_some() && !service_names.is_empty() {
        eprintln!("❌ Error: --project cannot be combined with service names");
        exit(1);
    }

    // Pick the compose stack to act on: an explicit --project, else a compose file in the CWD.
    let compose_selection: Option<(Vec<String>, String, String)> = if let Some(name) = &project_name
    {
        let cache = match ProjectCache::new() {
            Ok(cache) => cache,
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                exit(1);
            }
        };
        match resolve_compose_project(name, &cache) {
            Ok(project) => Some((
                project.compose_args(),
                name.clone(),
                format!("compose:{}", name),
            )),
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                exit(1);
            }
        }
    } else if service_names.is_empty() {
        find_compose_file().map(|compose_file| {
            // Get current directory name for better history context
            let dir_name = env::current_dir()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());

            (
                vec!["-f".to_string(), compose_file.to_string()],
                compose_file.to_string(),
                format!("compose:{}:{}", dir_name, compose_file),
            )
        })
    } else {
        None
    };

    if let Some((selector, label, target)) = compose_selection {
        let result = match command {
            TickleCommand::Tickle => compose_down_up(&selector, &label),
            TickleCommand::Start => compose_start(&selector, &label),
            TickleCommand::Stop => compose_stop(&selector, &label),
            TickleCommand::History | TickleCommand::Completions => unreachable!(),
        };

        let success = result.is_ok();
        let cmd_name = match command {
            TickleCommand::Tickle => "tickle",
            TickleCommand::Start => "start",
            TickleCommand::Stop => "stop",
            TickleCommand::History | TickleCommand::Completions => unreachable!(),
        };

        // Log to history
        if let Err(e) = history_manager.log_command(cmd_name, &target, success) {
            eprintln!("⚠️  Warning: Failed to log to history: {}", e);
        }

        match result {
            Ok(()) => {
                println!("🎉 Compose {} completed successfully!", cmd_name);
                if follow {
                    follow_compose_logs(&selector);
                }
                exit(0);
            }
            Err(e) => {
                eprintln!("❌ Compose error: {}", e);
                exit(1);
            }
        }
    }

    if service_names.is_empty() {
        eprintln!("❌ Error: No service name provided and no compose file found");
        print_usage();
        exit(1);
    }

    // Check if running as root/with sudo for systemd operations
    if let Ok(output) = Command::new("id").arg("-u").output() {
        let uid_output = String::from_utf8_lossy(&output.stdout);
        let uid = uid_output.trim();
        if uid != "0" {
            println!("⚠️  Warning: You may need to run with sudo for system services");
        }
    }

    let service_manager = ServiceManager::new();

    let cmd_name = match command {
        TickleCommand::Tickle => "tickle",
        TickleCommand::Start => "start",
        TickleCommand::Stop => "stop",
        TickleCommand::History | TickleCommand::Completions => unreachable!(),
    };

    if service_names.len() == 1 {
        // Single service — identical behavior to today
        let service_name = &service_names[0];

        let result = match command {
            TickleCommand::Tickle => service_manager.tickle_service(service_name, force_stop_start),
            TickleCommand::Start => service_manager
                .check_systemctl_available()
                .and_then(|_| service_manager.start_service(service_name)),
            TickleCommand::Stop => service_manager
                .check_systemctl_available()
                .and_then(|_| service_manager.stop_service(service_name)),
            TickleCommand::History | TickleCommand::Completions => unreachable!(),
        };

        let success = result.is_ok();

        // Log to history
        if let Err(e) = history_manager.log_command(cmd_name, service_name, success) {
            eprintln!("⚠️  Warning: Failed to log to history: {}", e);
        }

        match result {
            Ok(()) => {
                println!(
                    "🎉 {} completed successfully!",
                    match command {
                        TickleCommand::Tickle => "Tickle",
                        TickleCommand::Start => "Start",
                        TickleCommand::Stop => "Stop",
                        TickleCommand::History | TickleCommand::Completions => unreachable!(),
                    }
                );

                // Verify final state for non-tickle operations
                if !matches!(command, TickleCommand::Tickle) {
                    match service_manager.get_service_state(service_name) {
                        Ok(final_state) => {
                            println!("📊 Final state: {:?}", final_state);
                        }
                        Err(e) => {
                            println!("⚠️  Warning: Could not verify final state: {}", e);
                        }
                    }
                }

                if follow {
                    follow_service_logs(service_name);
                }
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                exit(1);
            }
        }
    } else {
        // Multiple services — stack tickle: operate in sequence, log each, then summarize
        if let Err(e) = service_manager.check_systemctl_available() {
            eprintln!("❌ Error: {}", e);
            exit(1);
        }

        let mut results: Vec<(String, Result<(), String>)> = Vec::new();

        for service_name in &service_names {
            println!("\n--- {} {} ---", cmd_name, service_name);
            let result = match command {
                TickleCommand::Tickle => {
                    service_manager.tickle_service(service_name, force_stop_start)
                }
                TickleCommand::Start => service_manager.start_service(service_name),
                TickleCommand::Stop => service_manager.stop_service(service_name),
                TickleCommand::History | TickleCommand::Completions => unreachable!(),
            };
            let success = result.is_ok();
            if let Err(e) = history_manager.log_command(cmd_name, service_name, success) {
                eprintln!("⚠️  Warning: Failed to log to history: {}", e);
            }
            if let Err(ref e) = result {
                eprintln!("❌ {}: {}", service_name, e);
            }
            results.push((service_name.clone(), result));
        }

        // Print summary
        let total = results.len();
        let failed_count = results.iter().filter(|(_, r)| r.is_err()).count();
        let ok_count = total - failed_count;

        println!("\n📋 Summary ({} services):", total);
        println!("{}", "─".repeat(40));
        for (svc, res) in &results {
            match res {
                Ok(()) => println!("  ✅ {}", svc),
                Err(e) => println!("  ❌ {} — {}", svc, e),
            }
        }
        println!("{}", "─".repeat(40));

        if failed_count > 0 {
            eprintln!("❌ {} succeeded, {} failed", ok_count, failed_count);
            exit(1);
        } else {
            println!("🎉 All {} services {}ed successfully!", total, cmd_name);
            if follow {
                follow_service_logs(service_names.last().unwrap());
            }
        }
    }
}
