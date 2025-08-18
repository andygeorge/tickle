#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn test_help_option() {
        let output = Command::new("cargo")
            .args(&["run", "--", "--help"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage: tickle"));
    }

    #[test]
    fn test_no_args() {
        let output = Command::new("cargo")
            .args(&["run"])
            .output()
            .expect("Failed to execute command");
        
        assert!(!output.status.success());
    }
}
