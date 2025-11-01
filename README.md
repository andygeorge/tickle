# Tickle 🎯

An extremely **vibe-coded** smart systemd service and Docker restart tool. For systemd, it intelligently chooses between `restart` and `stop`/`start` based on service capabilities.

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
cargo install --git https://github.com/andygeorge/tickle#0.4.0
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
