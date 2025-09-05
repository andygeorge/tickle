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
git clone https://github.com/andygeorge/tickle
cd tickle
cargo build --release
sudo cp target/release/tickle /usr/local/bin/
```

### Using Cargo
```bash
cargo install --git https://github.com/andygeorge/tickle#0.3.0
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

