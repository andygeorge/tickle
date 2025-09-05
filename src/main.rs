// src/main.rs
use std::env;
use std::path::Path;
use std::process::{Command, exit};

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
            .args(&["is-active", service_name])
            .output()
            .map_err(|e| format!("Failed to check service status: {}", e))?;
        let status = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();

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
            .args(&["cat", service_name])
            .output()
            .map_err(|e| format!("Failed to check if service exists: {}", e))?;
        if !output.status.success() {
            return Ok(false);
        }

        // Check if restart is supported by looking at the service configuration
        let output = Command::new("systemctl")
            .args(&["show", service_name, "--property=CanRestart"])
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
            .args(&["show", service_name, "--property=Type"])
            .output()
            .map_err(|e| format!("Failed to check service type: {}", e))?;
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            // Most service types support restart except oneshot without RemainAfterExit
            if result.contains("Type=oneshot") {
                // Check if RemainAfterExit is set
                let remain_output = Command::new("systemctl")
                    .args(&["show", service_name, "--property=RemainAfterExit"])
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
            .args(&["restart", service_name])
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
            .args(&["stop", service_name])
            .output()
            .map_err(|e| format!("Failed to execute stop command: {}", e))?;
        if !stop_output.status.success() {
            let stderr = String::from_utf8_lossy(&stop_output.stderr);
            return Err(format!("Stop failed: {}", stderr.trim()));
        }
        println!("▶️ Starting {}...", service_name);

        let start_output = Command::new("systemctl")
            .args(&["start", service_name])
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
    for name in candidates {
        if Path::new(name).exists() {
            return Some(name);
        }
    }
    None
}

/// Try running `docker compose <args...>` first; fall back to `docker-compose <args...>`.
fn run_compose_with_best_cli(args: &[&str]) -> Result<(), String> {
    // Prefer modern `docker compose`
    let try_docker_compose_plugin = Command::new("docker").args(std::iter::once("compose").chain(args.iter().copied())).output();
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
    let legacy = Command::new("docker-compose").args(args).output()
        .map_err(|e| format!("Failed to run docker-compose: {}", e))?;
    if legacy.status.success() {
        Ok(())
    } else {
        Err(format!("Compose command failed: {}", String::from_utf8_lossy(&legacy.stderr).trim()))
    }
}

/// Perform `compose down` then `compose up -d` against the given compose file.
fn compose_down_up(compose_file: &str) -> Result<(), String> {
    println!("🐳 Compose file detected: {}. Performing `docker compose down`...", compose_file);
    run_compose_with_best_cli(&["-f", compose_file, "down"])?;
    println!("🚀 Bringing stack back up in detached mode...");
    run_compose_with_best_cli(&["-f", compose_file, "up", "-d"])?;
    println!("✅ Compose stack restarted.");
    Ok(())
}

/* ------------------ CLI / UX ------------------ */

fn print_version() {
    println!("tickle {}", env!("CARGO_PKG_VERSION"));
}

fn print_usage() {
    println!("Usage: tickle [OPTIONS] <service_name>");
    println!("");
    println!("OPTIONS:");
    println!("  -s, --stop-start    Force stop/start instead of restart");
    println!("  -v, --version       Show version information");
    println!("  -h, --help          Show this help message");
    println!("");
    println!("Behavior:");
    println!("  • If run in a directory containing a compose file (docker-compose.yml/.yaml,");
    println!("    compose.yml/.yaml, container-compose.yml/.yaml) and no <service_name> is");
    println!("    provided, tickle will execute:");
    println!("        docker compose -f <file> down && docker compose -f <file> up -d");
    println!("");
    println!("Examples:");
    println!("  tickle nginx");
    println!("  tickle --stop-start apache2");
    println!("  tickle -s postgresql");
    println!("  tickle               # in a compose project directory");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // If no args, try compose behavior first.
    if args.len() < 2 {
        if let Some(compose_file) = find_compose_file() {
            match compose_down_up(compose_file) {
                Ok(()) => {
                    println!("🎉 Compose tickle completed successfully!");
                    exit(0);
                }
                Err(e) => {
                    eprintln!("❌ Compose error: {}", e);
                    exit(1);
                }
            }
        } else {
            eprintln!("❌ Error: No service name provided");
            print_usage();
            exit(1);
        }
    }

    let mut force_stop_start = false;
    let mut service_name = "";

    // Simple argument parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                exit(0);
            },
            "-v" | "--version" => {
                print_version();
                exit(0);
            },
            "-s" | "--stop-start" => {
                force_stop_start = true;
            },
            arg if !arg.starts_with('-') => {
                service_name = arg;
                break;
            },
            _ => {
                eprintln!("❌ Error: Unknown option: {}", args[i]);
                print_usage();
                exit(1);
            }
        }
        i += 1;
    }

    if service_name.is_empty() {
        // If we reach here with no service, we already handled compose above.
        eprintln!("❌ Error: No service name provided");
        print_usage();
        exit(1);
    }

    // Check if running as root/with sudo
    if let Ok(output) = Command::new("id").arg("-u").output() {
        let uid_output = String::from_utf8_lossy(&output.stdout);
        let uid = uid_output.trim();
        if uid != "0" {
            println!("⚠️  Warning: You may need to run with sudo for system services");
        }
    }

    let service_manager = ServiceManager::new();

    match service_manager.tickle_service(service_name, force_stop_start) {
        Ok(()) => {
            println!("🎉 Tickle completed successfully!");

            // Verify final state
            match service_manager.get_service_state(service_name) {
                Ok(final_state) => {
                    println!("📊 Final state: {:?}", final_state);
                }
                Err(e) => {
                    println!("⚠️  Warning: Could not verify final state: {}", e);
                }
            }
        },
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            exit(1);
        }
    }
}
