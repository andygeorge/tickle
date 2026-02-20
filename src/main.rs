// src/main.rs
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
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

/// Try running `docker compose <args...>` first; fall back to `docker-compose <args...>`.
fn run_compose_with_best_cli(args: &[&str]) -> Result<(), String> {
    // Prefer modern `docker compose`
    let try_docker_compose_plugin = Command::new("docker")
        .args(std::iter::once("compose").chain(args.iter().copied()))
        .output();
    if let Ok(out) = try_docker_compose_plugin {
        if out.status.success() {
            return Ok(());
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // If the failure might be due to missing plugin, we'll try legacy next.
            // Otherwise still try legacy for compatibility.
            // println!("debug docker compose error: {}", stderr);
            // fallthrough
            if !stderr.is_empty() {
                // continue to legacy attempt
            }
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

/// Perform `compose down` then `compose up -d` against the given compose file.
fn compose_down_up(compose_file: &str) -> Result<(), String> {
    println!(
        "🐳 Compose file detected: {}. Performing `docker compose down`...",
        compose_file
    );
    run_compose_with_best_cli(&["-f", compose_file, "down"])?;
    println!("🚀 Bringing stack back up in detached mode...");
    run_compose_with_best_cli(&["-f", compose_file, "up", "-d"])?;
    println!("✅ Compose stack restarted.");
    Ok(())
}

/// Start compose stack
fn compose_start(compose_file: &str) -> Result<(), String> {
    println!("🐳 Starting compose stack: {}...", compose_file);
    run_compose_with_best_cli(&["-f", compose_file, "up", "-d"])?;
    println!("✅ Compose stack started.");
    Ok(())
}

/// Stop compose stack
fn compose_stop(compose_file: &str) -> Result<(), String> {
    println!("🐳 Stopping compose stack: {}...", compose_file);
    run_compose_with_best_cli(&["-f", compose_file, "down"])?;
    println!("✅ Compose stack stopped.");
    Ok(())
}

/* ------------------ Log following ------------------ */

/// Replace the current process with `docker compose -f FILE logs -f`.
/// Tries `docker compose` first, falls back to `docker-compose`.
fn follow_compose_logs(compose_file: &str) -> ! {
    println!("📋 Following compose logs (Ctrl+C to stop)...");
    let err = Command::new("docker")
        .args(["compose", "-f", compose_file, "logs", "-f"])
        .exec();
    // exec() only returns on failure — try legacy CLI
    eprintln!(
        "⚠️  docker compose not available ({}), trying docker-compose...",
        err
    );
    let err = Command::new("docker-compose")
        .args(["-f", compose_file, "logs", "-f"])
        .exec();
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
    println!("Usage: tickle [COMMAND] [OPTIONS] [service_name]");
    println!();
    println!("COMMANDS:");
    println!("  start               Start a service or compose stack");
    println!("  stop                Stop a service or compose stack");
    println!("  history             Show command history");
    println!("  history clear       Clear command history");
    println!("  (default)           Restart/tickle a service or compose stack");
    println!();
    println!("OPTIONS:");
    println!("  -f, --follow        Follow logs after the operation completes");
    println!("  -s, --stop-start    Force stop/start instead of restart (tickle only)");
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
    println!("  • Otherwise, tickle will operate on the named systemd service:");
    println!("        tickle nginx    -> systemctl restart nginx (or stop+start if needed)");
    println!("        tickle start nginx -> systemctl start nginx");
    println!("        tickle stop nginx  -> systemctl stop nginx");
    println!();
    println!("  • History is stored in ~/.tickle/history.log");
    println!();
    println!("Examples:");
    println!("  tickle nginx");
    println!("  tickle start apache2");
    println!("  tickle stop postgresql");
    println!("  tickle --stop-start apache2");
    println!("  tickle history              # Show full history");
    println!("  tickle history -n 10        # Show last 10 entries");
    println!("  tickle history clear        # Clear all history");
    println!("  tickle start                # in a compose project directory");
    println!("  tickle stop                 # in a compose project directory");
    println!("  tickle                      # in a compose project directory");
    println!("  tickle -f nginx             # restart nginx then follow journalctl");
    println!("  tickle -f                   # restart compose stack then follow logs");
}

/// Parse command from arguments
fn parse_command(args: &[String]) -> TickleCommand {
    if args.len() > 1 {
        match args[1].as_str() {
            "start" => TickleCommand::Start,
            "stop" => TickleCommand::Stop,
            "history" => TickleCommand::History,
            _ => TickleCommand::Tickle,
        }
    } else {
        TickleCommand::Tickle
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
        // Check for subcommand (clear)
        if args.len() > 2 && args[2] == "clear" {
            match history_manager.clear_history() {
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

    // Determine if we have a service name and parse other options
    let mut force_stop_start = false;
    let mut follow = false;
    let mut service_name = "";
    let start_index = match command {
        TickleCommand::Start | TickleCommand::Stop => 2, // Skip "tickle" and "start"/"stop"
        TickleCommand::Tickle => 1,                      // Skip just "tickle"
        TickleCommand::History => unreachable!(),        // Already handled above
    };

    // Parse remaining arguments
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
            arg if !arg.starts_with('-') => {
                service_name = arg;
                break;
            }
            _ => {
                eprintln!("❌ Error: Unknown option: {}", args[i]);
                print_usage();
                exit(1);
            }
        }
        i += 1;
    }

    // Determine the target for history logging
    let target: String;

    // Handle compose file operations when no service name is provided
    if service_name.is_empty() {
        if let Some(compose_file) = find_compose_file() {
            // Get current directory name for better history context
            let dir_name = env::current_dir()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());

            target = format!("compose:{}:{}", dir_name, compose_file);

            let result = match command {
                TickleCommand::Tickle => compose_down_up(compose_file),
                TickleCommand::Start => compose_start(compose_file),
                TickleCommand::Stop => compose_stop(compose_file),
                TickleCommand::History => unreachable!(),
            };

            let success = result.is_ok();
            let cmd_name = match command {
                TickleCommand::Tickle => "tickle",
                TickleCommand::Start => "start",
                TickleCommand::Stop => "stop",
                TickleCommand::History => unreachable!(),
            };

            // Log to history
            if let Err(e) = history_manager.log_command(cmd_name, &target, success) {
                eprintln!("⚠️  Warning: Failed to log to history: {}", e);
            }

            match result {
                Ok(()) => {
                    println!("🎉 Compose {} completed successfully!", cmd_name);
                    if follow {
                        follow_compose_logs(compose_file);
                    }
                    exit(0);
                }
                Err(e) => {
                    eprintln!("❌ Compose error: {}", e);
                    exit(1);
                }
            }
        } else {
            eprintln!("❌ Error: No service name provided and no compose file found");
            print_usage();
            exit(1);
        }
    } else {
        target = service_name.to_string();
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

    let result = match command {
        TickleCommand::Tickle => service_manager.tickle_service(service_name, force_stop_start),
        TickleCommand::Start => service_manager
            .check_systemctl_available()
            .and_then(|_| service_manager.start_service(service_name)),
        TickleCommand::Stop => service_manager
            .check_systemctl_available()
            .and_then(|_| service_manager.stop_service(service_name)),
        TickleCommand::History => unreachable!(),
    };

    let success = result.is_ok();
    let cmd_name = match command {
        TickleCommand::Tickle => "tickle",
        TickleCommand::Start => "start",
        TickleCommand::Stop => "stop",
        TickleCommand::History => unreachable!(),
    };

    // Log to history
    if let Err(e) = history_manager.log_command(cmd_name, &target, success) {
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
                    TickleCommand::History => unreachable!(),
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
}
