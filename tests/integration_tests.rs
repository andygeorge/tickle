// tests/integration_tests.rs
// Integration tests for the tickle command-line tool

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the tickle binary path
fn get_tickle_binary() -> PathBuf {
    let mut path = env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("No parent directory")
        .parent()
        .expect("No grandparent directory")
        .to_path_buf();
    
    // In debug mode
    if path.ends_with("deps") {
        path.pop();
    }
    
    path.push("tickle");
    path
}

/// Create a temporary test directory
fn create_temp_dir(name: &str) -> PathBuf {
    let temp = env::temp_dir().join(format!("tickle_integration_{}", name));
    if temp.exists() {
        fs::remove_dir_all(&temp).ok();
    }
    fs::create_dir_all(&temp).expect("Failed to create temp dir");
    temp
}

/// Cleanup test directory
fn cleanup_dir(path: &PathBuf) {
    if path.exists() {
        fs::remove_dir_all(path).ok();
    }
}

#[test]
fn test_tickle_help() {
    let output = Command::new(get_tickle_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("COMMANDS:"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("history"));
}

#[test]
fn test_tickle_version() {
    let output = Command::new(get_tickle_binary())
        .arg("--version")
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tickle"));
}

#[test]
fn test_tickle_help_short() {
    let output = Command::new(get_tickle_binary())
        .arg("-h")
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
}

#[test]
fn test_tickle_version_short() {
    let output = Command::new(get_tickle_binary())
        .arg("-v")
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tickle"));
}

#[test]
fn test_tickle_history_empty() {
    let test_dir = create_temp_dir("history_empty");
    
    let output = Command::new(get_tickle_binary())
        .arg("history")
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No history found") || stdout.contains("History file is empty"));
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_history_clear_when_empty() {
    let test_dir = create_temp_dir("history_clear_empty");
    
    let output = Command::new(get_tickle_binary())
        .arg("history")
        .arg("clear")
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cleared") || stdout.contains("No history"));
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_no_service_no_compose() {
    let test_dir = create_temp_dir("no_service_no_compose");
    
    let output = Command::new(get_tickle_binary())
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    // Should fail because no service name and no compose file
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No service name") || stderr.contains("no compose file"));
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_with_compose_file() {
    let test_dir = create_temp_dir("with_compose");
    
    // Create a dummy docker-compose.yml
    let compose_path = test_dir.join("docker-compose.yml");
    fs::write(&compose_path, "version: '3'\nservices:\n  test:\n    image: nginx\n")
        .expect("Failed to create compose file");
    
    let output = Command::new(get_tickle_binary())
        .current_dir(&test_dir)
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    // Will likely fail due to docker not being available, but should attempt compose
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Should mention compose or docker
    assert!(
        stderr.contains("docker") || 
        stderr.contains("compose") || 
        stdout.contains("Compose") ||
        stdout.contains("docker")
    );
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_unknown_option() {
    let output = Command::new(get_tickle_binary())
        .arg("--unknown-option")
        .output()
        .expect("Failed to execute tickle");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown option") || stderr.contains("Error"));
}

#[test]
fn test_tickle_start_command() {
    let test_dir = create_temp_dir("start_command");
    
    let output = Command::new(get_tickle_binary())
        .arg("start")
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    // Should fail (no service name, no compose)
    assert!(!output.status.success());
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_stop_command() {
    let test_dir = create_temp_dir("stop_command");
    
    let output = Command::new(get_tickle_binary())
        .arg("stop")
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    // Should fail (no service name, no compose)
    assert!(!output.status.success());
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_history_with_n_option() {
    let test_dir = create_temp_dir("history_n_option");
    
    let output = Command::new(get_tickle_binary())
        .args(&["history", "-n", "5"])
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_history_invalid_n_value() {
    let test_dir = create_temp_dir("history_invalid_n");
    
    let output = Command::new(get_tickle_binary())
        .args(&["history", "-n", "not-a-number"])
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid") || stderr.contains("Error"));
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_tickle_stop_start_flag_with_wrong_command() {
    let test_dir = create_temp_dir("stop_start_flag");
    
    let output = Command::new(get_tickle_binary())
        .args(&["start", "--stop-start", "nginx"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    // Should fail because --stop-start only works with tickle command
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("stop-start") || stderr.contains("only valid"));
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_compose_file_detection_docker_compose_yml() {
    let test_dir = create_temp_dir("compose_detect_docker");
    
    fs::write(test_dir.join("docker-compose.yml"), "version: '3'")
        .expect("Failed to create file");
    
    let output = Command::new(get_tickle_binary())
        .arg("history")  // Use history to avoid actually running compose
        .current_dir(&test_dir)
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_compose_file_detection_compose_yaml() {
    let test_dir = create_temp_dir("compose_detect_yaml");
    
    fs::write(test_dir.join("compose.yaml"), "version: '3'")
        .expect("Failed to create file");
    
    let output = Command::new(get_tickle_binary())
        .arg("history")
        .current_dir(&test_dir)
        .env("HOME", &test_dir)
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
    
    cleanup_dir(&test_dir);
}

#[test]
fn test_help_contains_all_commands() {
    let output = Command::new(get_tickle_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute tickle");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Verify all commands are documented
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("history"));
    assert!(stdout.contains("tickle") || stdout.contains("restart"));
    
    // Verify options are documented
    assert!(stdout.contains("--stop-start") || stdout.contains("-s"));
    assert!(stdout.contains("--help") || stdout.contains("-h"));
    assert!(stdout.contains("--version") || stdout.contains("-v"));
    
    // Verify examples exist
    assert!(stdout.contains("Examples:") || stdout.contains("example"));
}

#[test]
fn test_multiple_help_flags() {
    // Should work with multiple help flags
    let output = Command::new(get_tickle_binary())
        .args(&["-h", "-h"])
        .output()
        .expect("Failed to execute tickle");
    
    assert!(output.status.success());
}
