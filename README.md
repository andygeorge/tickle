# Tickle 🎯

An almost-entirely vibe-coded smart systemd service and Docker restart tool. For systemd, it intelligently chooses between `restart` and `stop`/`start` based on service capabilities, and keeps a history of `tickle`s.

More docs in [docs/](./docs/).

## Install

```bash
cargo install --git https://github.com/andygeorge/tickle#0.7.0 && sudo cp ~/.cargo/bin/tickle /usr/bin/tickle
```

Or from source:

```bash
git clone https://github.com/andygeorge/tickle
cd tickle
cargo build --release
sudo cp target/release/tickle /usr/local/bin/
```

## Usage

```bash
tickle nginx                        # restart a service (smart detection)
tickle nginx postgresql redis       # restart multiple services in sequence
tickle start nginx postgresql       # start services
tickle stop nginx postgresql        # stop services
tickle -s apache2                   # force stop/start instead of restart
tickle -f nginx                     # restart, then follow journalctl logs
tickle                              # in a compose dir: down + up -d
tickle -f                           # restart compose stack, then follow logs
```

Run without arguments in a directory containing a compose file
(`docker-compose.yml`, `compose.yaml`, `container-compose.yml`, etc.) and
tickle restarts the whole stack: `docker compose down && docker compose up -d`.

Multi-service runs print a per-service summary and exit non-zero if any
service failed.

## History

Every operation is logged to `~/.tickle/history.log`.

```bash
tickle history          # show full history
tickle history -n 10    # last 10 entries
tickle history stats    # totals, per-command breakdown, streaks
tickle history clear    # wipe it
```

## Shell Completions

Completions cover subcommands, flags, and systemd service names; compose
service names too when a compose file is present.

```bash
eval "$(tickle completions bash)"   # ~/.bashrc
eval "$(tickle completions zsh)"    # ~/.zshrc
tickle completions fish > ~/.config/fish/completions/tickle.fish
```

## Development

```bash
cargo build
cargo test
```
