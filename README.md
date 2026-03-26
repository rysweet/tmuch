# tmuch

TUI tmux multiplexer -- display multiple tmux sessions in a single terminal, with SSH remote access, widget panes, and Azure VM discovery.

Built with [ratatui](https://ratatui.rs). Captures tmux session output via `tmux capture-pane` and renders it in an auto-reflowing pane grid.

## Features

- **Local tmux sessions** -- attach any number of existing or new tmux sessions
- **SSH remote sessions** -- view remote tmux sessions via `user@host:session` syntax, using SSH ControlMaster for persistent connections
- **Widget panes** -- built-in clock, weather, sysinfo gauges, sparkline charts, and a snake game
- **Programmable panes** -- `watch:`, `tail:`, and `http:` prefixes for non-tmux content
- **Layouts** -- save and restore pane arrangements with `--save-layout` and `--layout`; split panes vertically/horizontally with drag-to-resize borders
- **Azure VM discovery** -- automatic discovery of running Azure VMs and their tmux sessions via `tmuch azlin` (requires `az` CLI)
- **Shell completions** -- bash, zsh, fish, elvish, powershell
- **Self-update** -- update from GitHub Releases with `tmuch update`
- **Theming** -- customizable border colors, status bar, and title styles via `~/.config/tmuch/theme.toml`

## Install

```bash
cargo install --path .
```

Or download a prebuilt binary from [Releases](https://github.com/rysweet/tmuch/releases).

To release a new version, push a `v*` tag (e.g. `git tag v0.4.0 && git push --tags`). The `release.yml` workflow builds cross-platform binaries automatically.

### Shell Completions

```bash
# Bash
tmuch completions bash > ~/.local/share/bash-completion/completions/tmuch

# Zsh
tmuch completions zsh > ~/.zfunc/_tmuch

# Fish
tmuch completions fish > ~/.config/fish/completions/tmuch.fish
```

## Usage

### Basic

```bash
# Attach to existing tmux sessions
tmuch session1 session2

# Create new sessions running commands
tmuch -n "tail -f /var/log/syslog" -n "top"

# Key bindings for quick commands
tmuch -b '1=htop' -b '2=docker logs -f app'

# Mix existing sessions and new commands
tmuch my-dev-server -n "htop" -b '3=docker logs -f app'
```

### SSH Remote Sessions

```bash
# Attach to a remote tmux session (user@host:session)
tmuch azureuser@myvm.eastus.cloudapp.azure.com:main

# Multiple remote and local sessions
tmuch local-session user@remote1:session1 user@remote2:session2
```

SSH connections use ControlMaster for persistent multiplexing -- one TCP connection per host, reused across all capture polls and send-keys operations. The control socket is created at `/tmp/tmuch-ssh-user@host:port` and cleaned up on exit.

Configure remote hosts in `~/.config/tmuch/config.toml`:

```toml
[[remote]]
name = "devbox"
host = "devbox.internal"
user = "azureuser"
port = 22
poll_interval_ms = 500
```

### Widget Panes and Apps

Launch built-in widgets with `-n` or interactively with `Ctrl-N`:

```bash
tmuch -n 'clock:'              # Live clock
tmuch -n 'weather:Seattle'     # Weather card (from wttr.in)
tmuch -n 'sysinfo:'           # CPU/mem/disk gauges
tmuch -n 'snake:'             # Playable snake game
tmuch -n 'spark:echo 42:2000' # Sparkline chart from command output
tmuch -n 'settings:'          # Settings panel
```

### Programmable Panes

```bash
# watch: runs a command periodically (watch:command:interval_ms)
tmuch -n 'watch:kubectl get pods:5000'

# tail: follows a file with tail -f
tmuch -n 'tail:/var/log/syslog'

# http: polls a URL periodically (http:url:interval_ms)
tmuch -n 'http:http://localhost:8080/health:3000'
```

### Layouts

```bash
# Save current pane layout on exit
tmuch session1 session2 --save-layout my-workspace

# Restore a saved layout
tmuch --layout my-workspace

# List saved layouts
tmuch layouts
```

Layouts are stored as TOML files in `~/.config/tmuch/layouts/`.

### Azure VM Discovery (Azlin)

```bash
# Discover all running VMs and their tmux sessions
tmuch azlin

# Filter by resource group
tmuch azlin --resource-group my-rg
```

While running tmuch, press `Ctrl-G` to open the session picker pre-populated with all discovered Azure VM sessions. Press `a` in the picker to add all sessions at once.

Enable azlin integration in config:

```toml
[azlin]
enabled = true
resource_group = "my-default-rg"  # optional
```

Requires `az` CLI to be installed and authenticated (`az login`).

### Self-Update

```bash
tmuch update
# or
tmuch self-update
```

## Key Bindings

### Normal Mode

| Key | Action |
|-----|--------|
| `q` / `Ctrl-Q` | Quit |
| `Ctrl-A` | Add new pane (creates new tmux session) |
| `Ctrl-D` | Drop focused pane |
| `Ctrl-S` | Open session picker |
| `Ctrl-G` | Discover Azure VMs (azlin picker) |
| `Ctrl-E` | Open settings panel |
| `Ctrl-N` | Open app launcher |
| `Ctrl-V` | Split focused pane vertically |
| `Ctrl-H` | Split focused pane horizontally |
| `Ctrl-F` / `F11` | Toggle maximize |
| `Ctrl-X` | Swap focused pane with next |
| `Tab` / arrows | Focus next pane |
| `Shift-Tab` | Focus previous pane |
| `Enter` | Enter pane-focused mode |
| `1`-`9` | Run configured command binding |

### Pane-Focused Mode

All keystrokes are forwarded to the tmux session. Press `Esc` to return to Normal mode.

### Session Picker (Ctrl-S)

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Navigate |
| `Enter` | Attach selected session |
| `a` | Add all sessions at once |
| `z` | Scan azlin VMs for sessions |
| `Esc` | Cancel |

### App Launcher (Ctrl-N)

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Navigate |
| `Enter` | Launch selected app |
| `Esc` | Cancel |

## Configuration

`~/.config/tmuch/config.toml`:

```toml
[bindings]
1 = "htop"
2 = "docker logs -f app"

[display]
poll_interval_ms = 150
border_style = "rounded"

# Remote SSH hosts
[[remote]]
name = "devbox"
host = "devbox.internal"
user = "azureuser"
port = 22
poll_interval_ms = 500

# Azure VM discovery
[azlin]
enabled = true
resource_group = "my-rg"
```

### Theming

`~/.config/tmuch/theme.toml`:

```toml
[border]
focused = "yellow"
focused_attached = "green"
unfocused = "#3c3c3c"
remote = "#283c50"
style = "rounded"  # rounded, plain, double, thick

[title]
focused = "white"
unfocused = "#787878"
attached_label = "green"

[status_bar]
bg = "black"
mode_fg = "black"
mode_bg = "cyan"
text = "white"
version = "darkgray"

[hints_bar]
bg = "#1e1e1e"
```

## Auto-Update Check

tmuch checks for updates on startup (non-blocking, 24-hour cache). Suppress with:

```bash
export TMUCH_NO_UPDATE_CHECK=1
```

## Architecture

```
src/
  main.rs               CLI args (clap), config loading, entry point
  app.rs                App state, delegation to modules
  event_loop.rs         Terminal event loop, tick-based rendering
  config.rs             TOML config parsing, startup validation
  consts.rs             Shared constants, version
  keys.rs               Key event handling, mode switching, action dispatch
  action_handler.rs     High-level action dispatch to state mutations
  mouse.rs              Mouse click/drag handling for pane focus and border resize
  pane.rs               Pane state management, PaneManager
  pane_ops.rs           Pane creation from specs, remote attachment
  layout.rs             Binary tree layout with split/resize/swap
  layouts.rs            Save/restore layout files
  ui.rs                 ratatui rendering (pane grid)
  ui_bars.rs            Hints bar and status bar rendering
  ui_overlays.rs        Session picker, command editor, app launcher overlays
  session_picker.rs     Session list popup (local + remote)
  editor_state.rs       Command editor and app launcher state
  theme.rs              Theme loading and color parsing
  tmux.rs               Local tmux subprocess interaction
  azlin_integration.rs  Azure VM discovery via azlin-azure
  ipc.rs / ipc_handler.rs  Unix socket IPC for `tmuch ctl`
  update_check.rs       Non-blocking background update check
  self_update.rs        Binary self-update from GitHub Releases
  source/
    mod.rs              ContentSource trait, PaneSpec enum, parse_new_arg()
    local_tmux.rs       Local tmux session source
    ssh_subprocess.rs   Remote SSH tmux source (ControlMaster persistent connection)
    command.rs          Periodic command execution (watch:)
    tail.rs             File tailing (tail:)
    http.rs             HTTP URL polling (http:)
    clock.rs            Clock widget
    weather.rs          Weather widget (wttr.in)
    sysinfo.rs          System stats widget (CPU/mem/disk gauges)
    snake.rs            Snake game widget
    sparkline_monitor.rs Sparkline chart widget
    settings.rs         Settings panel widget
    settings_render.rs  Settings tab rendering
    registry.rs         Plugin registry for app launcher
```

## Testing

```bash
# Unit tests (293 tests, ~72% line coverage)
cargo test

# Coverage report
cargo llvm-cov

# Lint
cargo clippy --all-targets -- -D warnings

# E2E tests via tmux
./tests/run-e2e.sh
```

## License

MIT
