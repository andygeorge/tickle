# Tickle 🎯

## [Documentation](./docs/)

A smart systemd service and Docker restart tool. For systemd, it intelligently chooses between `restart` and `stop`/`start` based on service capabilities.

## Features

- 🔍 **Smart Detection**: Automatically determines if a service supports restart or needs stop/start
- 🎯 **Service State Checking**: Shows current service status before and after operations
- ⚡ **Fast & Reliable**: Built in Rust for performance and safety
- 🛡️ **Error Handling**: Comprehensive error messages and status reporting
- 🎨 **User Friendly**: Clean CLI with emoji indicators and helpful output
- 🐳 **Docker Compose Integration**: Automatically detects and manages Docker compose stacks

## Installation

### From Source
```bash
git clone https://github.com/andygeorge/tickle
cd tickle
cargo build --release
sudo cp target/release/tickle /usr/local/bin/
```

### Using Cargo
```bash
cargo install --git https://github.com/andygeorge/tickle#0.5.2 && sudo cp ~/.cargo/bin/tickle /usr/bin/tickle
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

### Docker Compose Integration
When run without arguments in a directory containing a compose file, tickle will:
- Detect the first available compose file (docker-compose.yml, docker-compose.yaml, compose.yml, compose.yaml, container-compose.yml, container-compose.yaml)
- Execute `docker compose down` followed by `docker compose up -d`

```bash
# In a Docker Compose project directory
tickle                    # Will restart entire compose stack

# Restart specific service in a compose file
tickle nginx              # Will restart just the nginx service
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

# Restart entire Docker Compose stack (in compose project directory)
$ tickle
🐳 Compose file detected: docker-compose.yml. Performing `docker compose down`...
🚀 Bringing stack back up in detached mode...
✅ Compose stack restarted.
🎉 Compose tickle completed successfully!
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
- Docker CLI installed (for compose functionality)
- Appropriate permissions (usually requires sudo for system services)

## Options

- `-s, --stop-start`: Force stop/start strategy instead of restart
- `-h, --help`: Show help message
- No arguments: When run without arguments in a compose project directory, will restart entire Docker Compose stack

# Tickle History Feature

## Overview
Added a comprehensive history tracking feature to the `tickle` command-line tool. All command executions are now automatically logged to `~/.tickle/history.log`.

## Key Changes

### 1. New History Manager Module
- **`HistoryManager` struct**: Manages all history-related operations
- **Automatic directory creation**: Creates `~/.tickle/` directory on first use
- **Plaintext logging**: Stores command history in human-readable format

### 2. History Log Format
Each entry contains:
- **Timestamp**: YYYY-MM-DD HH:MM:SS format
- **Command**: The operation performed (tickle, start, stop)
- **Target**: Service name or compose file
- **Status**: SUCCESS or FAILED

Example log entry:
```
2024-02-05 14:30:45 | tickle | nginx | SUCCESS
2024-02-05 14:31:12 | start | compose:docker-compose.yml | SUCCESS
2024-02-05 14:32:00 | stop | apache2 | FAILED
```

### 3. New Commands

#### View History
```bash
# Show full history
tickle history

# Show last N entries
tickle history -n 10
tickle history -n 25
```

#### Clear History
```bash
tickle history clear
```

### 4. Features
- **Automatic logging**: Every tickle, start, and stop operation is logged
- **Tracks both systemd services and compose stacks**
- **Success/failure tracking**: Logs whether operations succeeded or failed
- **No external dependencies**: Uses only standard library for timestamp generation
- **Graceful degradation**: If history logging fails, the main operation continues with a warning

### 5. File Location
- **Directory**: `~/.tickle/`
- **Log file**: `~/.tickle/history.log`
- **Format**: Plain text (one entry per line)

### 6. Usage Examples

```bash
# Regular usage (automatically logged)
tickle nginx
tickle start apache2
tickle stop postgresql

# View your history
tickle history

# View last 20 entries
tickle history -n 20

# Clear all history
tickle history clear
```

## Implementation Details

### Timestamp Generation
The implementation uses a simple timestamp algorithm that doesn't require external dependencies:
- Calculates seconds since Unix epoch
- Converts to approximate date/time format
- Provides consistent, sortable timestamps

### Error Handling
- History failures don't prevent main operations
- Warnings displayed if history logging fails
- Creates directory automatically on first use

### Privacy & Security
- Logs stored locally in user's home directory
- Plain text format for easy inspection
- No sensitive data logged (only service names and operation types)

## Updated Help Output
The help text now includes information about the history command:

```
COMMANDS:
  start               Start a service or compose stack
  stop                Stop a service or compose stack
  history             Show command history
  history clear       Clear command history
  (default)           Restart/tickle a service or compose stack

  • History is stored in ~/.tickle/history.log

Examples:
  tickle history              # Show full history
  tickle history -n 10        # Show last 10 entries
  tickle history clear        # Clear all history
```

## Benefits
1. **Audit trail**: Track all service management operations
2. **Debugging**: Review what commands were run and when
3. **Documentation**: Automatic log of system administration activities
4. **Troubleshooting**: Quickly identify failed operations
5. **Learning**: Review past commands for reference

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

.
