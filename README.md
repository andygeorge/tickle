# Tickle 🎯

An almost-entirely vibe-coded smart systemd service and Docker restart tool. For systemd, it intelligently chooses between `restart` and `stop`/`start` based on service capabilities, and keeps a history of `tickle`s.

More docs in [docs/](./docs/).

## Install

```bash
cargo install --git https://github.com/andygeorge/tickle#0.9.0 && sudo cp ~/.cargo/bin/tickle /usr/bin/tickle
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
tickle -p paperless                 # restart a compose project from anywhere
tickle stop -p paperless            # stop that project
```

Run without arguments in a directory containing a compose file
(`docker-compose.yml`, `compose.yaml`, `container-compose.yml`, etc.) and
tickle restarts the whole stack: `docker compose down && docker compose up -d`.

## Compose Projects by Name

`-p`/`--project` acts on a compose stack from any directory. Tickle finds it
by the `com.docker.compose.project` label that compose puts on every container,
reads the project's config files and working directory off the same labels, and
runs the operation against those.

```bash
tickle -p paperless         # down + up -d
tickle start -p paperless   # up -d
tickle stop -p paperless    # down
tickle -f -p paperless      # restart, then follow the stack's logs
```

`--project` cannot be combined with service names. If the project is unknown,
tickle lists the projects docker does know about.

Because `stop` runs `down`, it removes the very containers that carry the
labels. Tickle therefore records each project's location in
`~/.tickle/projects.tsv` when it finds one, and falls back to that record when
no containers remain — so `tickle start -p paperless` still works after a stop.
Cache entries whose compose file has since disappeared are ignored.

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
service names too when a compose file is present, and compose project names
after `-p`/`--project`.

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
