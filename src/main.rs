// src/main.rs
use std::env;
use std::fs;
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

#[derive(Debug)]
enum OperationMode {
    SystemdService(String),
    DockerCompose(String), // Path to compose file
}

struct ServiceManager;

impl ServiceManager {
    fn new() -> Self {
        ServiceManager
    }

    /// Check for Docker Compose files in current directory
    fn detect_compose_file(&self) -> Option<String> {
        let compose_files = [
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml", 
            "compose.yaml"
        ];

        for file in &compose_files {
            if Path::new(file).exists() {
                return Some(file.to_string());
            }
        }
        None
    }

    /// Check if docker compose command is available
    fn check_docker_compose_available(&self) -> Result<(), String> {
        match Command::new("docker").args(&["compose", "version"]).output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err("docker compose is not available or not working properly.".to_string())
                }
            }
            Err(_) => Err("docker compose is not available. Please install Docker with Compose plugin.".to_string()),
        }
    }

    /// Execute Docker Compose down and up
    fn tickle_docker_compose(&self, compose_file: &str) -> Result<(), String> {
        self.check_docker_compose_available()?;

        println!("🐳 Found Docker Compose file: {}", compose_file);
        println!("🛑 Bringing down containers...");

        // Execute docker compose down
        let down_output = Command::new("docker")
            .args(&["compose", "-f", compose_file, "down"])
            .output()
            .map_err(|e| format!("Failed to execute docker compose down: {}", e))?;

        if !down_output.status.success() {
            let stderr = String::from_utf8_lossy(&down_output.stderr);
            return Err(format!("Docker compose down failed: {}", stderr.trim()));
        }

        println!("▶️ Starting containers in detached mode...");

        // Execute docker compose up -d
        let up_output = Command::new("docker")
            .args(&["compose", "-f", compose_file, "up", "-d"])
            .output()
            .map_err(|e| format!("Failed to execute docker compose up: {}", e))?;

        if up_output.status.success() {
            println!("✅ Successfully restarted Docker Compose services");
            
            // Show running containers
            println!("📋 Current container status:");
            let ps_output = Command::new("docker")
                .args(&["compose", "-f", compose_file, "ps"])
                .output();
            
            if let Ok(output) = ps_output {
                let ps_result = String::from_utf8_lossy(&output.stdout);
                println!("{}", ps_result);
            }
            
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&up_output.stderr);
            Err(format!("Docker compose up failed: {}", stderr.trim()))
        }
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

    /// Main tickle operation for systemd services
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

    /// Determine operation mode based on arguments and environment
    fn determine_operation_mode(&self, args: &[String]) -> Result<OperationMode, String> {
        // If no service name is provided, check for compose file
        if args.len() < 2 || (args.len() == 2 && (args[1] == "-h" || args[1] == "--help" || args[1] == "-v" || args[1] == "--version")) {
            if let Some(compose_file) = self.detect_compose_file() {
                return Ok(OperationMode::DockerCompose(compose_file));
            }
        }

        // Check if there are flags but no service name
        let mut has_service_name = false;
        for arg in args.iter().skip(1) {
            if !arg.starts_with('-') {
                has_service_name = true;
                break;
            }
        }

        if !has_service_name {
            if let Some(compose_file) = self.detect_compose_file() {
                return Ok(OperationMode::DockerCompose(compose_file));
            } else {
                return Err("No service name provided and no Docker Compose file found.".to_string());
            }
        }

        // Find the service name (first non-flag argument)
        for arg in args.iter().skip(1) {
            if !arg.starts_with('-') {
                return Ok(OperationMode::SystemdService(arg.clone()));
            }
        }

        Err("No service name provided.".to_string())
    }
}

fn print_version() {
    println!("tickle {}", env!("CARGO_PKG_VERSION"));
}

fn print_usage() {
    println!("Usage: tickle [OPTIONS] [service_name]");
    println!("");
    println!("If no service_name is provided and a Docker Compose file is found in the current");
    println!("directory, tickle will restart the Docker Compose services instead.");
    println!("");
    println!("OPTIONS:");
    println!("  -s, --stop-start    Force stop/start instead of restart (systemd only)");
    println!("  -v, --version       Show version information");
    println!("  -h, --help          Show this help message");
    println!("");
    println!("Examples:");
    println!("  tickle                    # Restart Docker Compose if compose file exists");
    println!("  tickle nginx              # Restart nginx systemd service");
    println!("  tickle --stop-start apache2   # Force stop/start apache2 service");
    println!("  tickle -s postgresql      # Force stop/start postgresql service");
    println!("");
    println!("Supported Docker Compose files:");
    println!("  - docker-compose.yml");
    println!("  - docker-compose.yaml");
    println!("  - compose.yml");
    println!("  - compose.yaml");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Handle help and version flags first
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_usage();
                exit(0);
            },
            "-v" | "--version" => {
                print_version();
                exit(0);
            },
            _ => {}
        }
    }

    let service_manager = ServiceManager::new();

    // Determine what operation to perform
    let operation_mode = match service_manager.determine_operation_mode(&args) {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            print_usage();
            exit(1);
        }
    };

    match operation_mode {
        OperationMode::DockerCompose(compose_file) => {
            match service_manager.tickle_docker_compose(&compose_file) {
                Ok(()) => {
                    println!("🎉 Docker Compose tickle completed successfully!");
                },
                Err(e) => {
                    eprintln!("❌ Error: {}", e);
                    exit(1);
                }
            }
        },
        OperationMode::SystemdService(service_name) => {
            let mut force_stop_start = false;
            
            // Parse flags for systemd service mode
            for arg in args.iter().skip(1) {
                match arg.as_str() {
                    "-s" | "--stop-start" => {
                        force_stop_start = true;
                    },
                    _ if arg.starts_with('-') => {
                        eprintln!("❌ Error: Unknown option: {}", arg);
                        print_usage();
                        exit(1);
                    },
                    _ => break,
                }
            }

            // Check if running as root/with sudo
            let output = Command::new("id").arg("-u").output();
            if let Ok(output) = output {
                let uid_output = String::from_utf8_lossy(&output.stdout);
                let uid = uid_output.trim();
                if uid != "0" {
                    println!("⚠️  Warning: You may need to run with sudo for system services");
                }
            }

            match service_manager.tickle_service(&service_name, force_stop_start) {
                Ok(()) => {
                    println!("🎉 Tickle completed successfully!");
                    
                    // Verify final state
                    match service_manager.get_service_state(&service_name) {
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
    }
}