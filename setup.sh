#!/bin/bash

# Complete setup script for the Tickle Rust application

echo "🚀 Setting up Tickle - Systemd Service Restart Tool"

# Create project directory
mkdir -p tickle
cd tickle

# Initialize git repository
git init

# Create Rust project structure
mkdir -p src tests docs

# Create Cargo.toml (content from previous artifact)
cat > Cargo.toml << 'EOF'
[package]
name = "tickle"
version = "0.1.0"
edition = "2021"
description = "A smart systemd service restart tool"
authors = ["Your Name <your.email@example.com>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourusername/tickle"
keywords = ["systemd", "service", "restart", "cli", "system-administration"]
categories = ["command-line-utilities"]

[[bin]]
name = "tickle"
path = "src/main.rs"

[dependencies]
# No external dependencies needed - using only std library

[dev-dependencies]
# Add test dependencies here if needed

[profile.release]
# Optimize for size and performance
opt-level = 3
lto = true
codegen-units = 1
strip = true
EOF

# Create README.md
cat > README.md << 'EOF'
# Tickle 🎯

A smart systemd service restart tool that intelligently chooses between `restart` and `stop`/`start` based on service capabilities.

## Features

- 🔍 **Smart Detection**: Automatically determines if a service supports restart or needs stop/start
- 🎯 **Service State Checking**: Shows current service status before and after operations  
- ⚡ **Fast & Reliable**: Built in Rust for performance and safety
- 🛡️ **Error Handling**: Comprehensive error messages and status reporting
- 🎨 **User Friendly**: Clean CLI with emoji indicators and helpful output

## Installation

### From Source
```bash
git clone https://github.com/yourusername/tickle
cd tickle
cargo build --release
sudo cp target/release/tickle /usr/local/bin/
```

### Using Cargo
```bash
cargo install tickle
```

## Usage

### Basic Usage
```bash
# Restart a service (smart detection)
tickle nginx

# Force stop/start instead of restart
tickle --stop-start apache2
tickle -s postgresql
```

### Examples
```bash
# Restart nginx (will use 'systemctl restart' if supported)
$ sudo tickle nginx
📊 Current state of nginx: Active
🎯 Using strategy: Restart
🔄 Attempting to restart nginx...
✅ Successfully restarted nginx
🎉 Tickle completed successfully!
📊 Final state: Active

# Restart a oneshot service (will use stop/start)
$ sudo tickle --stop-start my-oneshot-service
📊 Current state of my-oneshot-service: Inactive
🎯 Using strategy: StopStart
🛑 Stopping my-oneshot-service...
▶️ Starting my-oneshot-service...
✅ Successfully stopped and started my-oneshot-service
🎉 Tickle completed successfully!
📊 Final state: Active
```

## How It Works

1. **Service Detection**: Checks if the service exists and what type it is
2. **Strategy Selection**: 
   - Uses `systemctl restart` for services that support it
   - Falls back to `systemctl stop` then `systemctl start` for services that don't
3. **State Verification**: Shows service state before and after the operation

## Service Types Supported

- ✅ **Simple/Forking Services**: Uses `restart`
- ✅ **Notify Services**: Uses `restart` 
- ✅ **Oneshot with RemainAfterExit**: Uses `restart`
- ✅ **Oneshot without RemainAfterExit**: Uses `stop`/`start`
- ✅ **All Other Types**: Smart detection with fallback

## Requirements

- Linux system with systemd
- systemctl command available
- Appropriate permissions (usually requires sudo for system services)

## Options

- `-s, --stop-start`: Force stop/start strategy instead of restart
- `-h, --help`: Show help message

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run -- nginx

# Build optimized release
cargo build --release
```

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
EOF

# Create .gitignore for Rust projects
cat > .gitignore << 'EOF'
# Rust/Cargo
/target/
Cargo.lock
**/*.rs.bk
*.pdb

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
.DS_Store?
._*
.Spotlight-V100
.Trashes
ehthumbs.db
Thumbs.db

# Logs
*.log

# Environment
.env
EOF

# Create basic test file
cat > tests/integration_tests.rs << 'EOF'
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
EOF

# Create LICENSE files
cat > LICENSE-MIT << 'EOF'
MIT License

Copyright (c) 2025 Tickle Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
EOF

echo "📁 Creating initial git commit..."
git add .
git commit -m "Initial commit: Tickle systemd service restart tool

- Smart service restart detection
- Support for both restart and stop/start strategies  
- Comprehensive error handling and user feedback
- Clean CLI with helpful output"

echo ""
echo "✅ Tickle project setup complete!"
echo ""
echo "🔧 Next steps:"
echo "1. cd tickle"
echo "2. cargo build                  # Build the project"
echo "3. cargo test                   # Run tests"  
echo "4. sudo cargo run -- nginx     # Test with a service"
echo "5. cargo build --release        # Build optimized version"
echo "6. sudo cp target/release/tickle /usr/local/bin/  # Install globally"
echo ""
echo "📖 The main.rs file contains the complete application code."
echo "🎯 Try: sudo ./target/release/tickle --help"
