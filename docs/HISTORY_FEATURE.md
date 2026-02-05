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
