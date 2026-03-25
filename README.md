# tmuch

TUI tmux multiplexer -- display multiple tmux sessions in a single terminal, with SSH remote access, programmable panes, and Azure VM discovery.

Built with [ratatui](https://ratatui.rs). Captures tmux session output via `tmux capture-pane` and renders it in auto-reflowing grid panes.

## Features

- **Local tmux sessions** -- attach any number of existing or new tmux sessions
- **SSH remote sessions** -- view remote tmux sessions via `user@host:session` syntax
- **Azure VM discovery** -- automatic discovery of running Azure VMs and their tmux sessions via `tmuch azlin`
- **Bastion tunnel support** -- automatic Azure Bastion tunnel creation for private VMs
- **Programmable panes** -- `watch:`, `tail:`, and `http:` prefixes for non-tmux content
- **Layouts** -- save and restore pane arrangements with `--save-layout` and `--layout`
- **Shell completions** -- generate completions for bash, zsh, fish, elvish, powershell
- **Self-update** -- update tmuch from GitHub Releases with `tmuch update`

## Screenshots

**Two panes side-by-side:**
```
+-e2e-pane-a-------------------------------------------++-e2e-pane-b-------------------------------------------+
|content-from-pane-a                                    ||content-from-pane-b                                    |
|                                                       ||                                                       |
+-------------------------------------------------------++-------------------------------------------------------+
 NORMAL  [2/2] e2e-pane-b | ^A:add ^D:drop ^S:list Tab:next Enter:focus
```

**Session picker overlay (Ctrl-S) with remote sessions:**
```
+-e2e-pane-a-----------------------------------------------------------------------------------+
|                                                                                               |
|                                 +- tmux sessions -------------------------+                   |
|                                 | > my-local-session                      |                   |
|                                 |   dev-server [vm-eastus]                |                   |
|                                 |   build-agent [vm-westus]               |                   |
|                                 +------------------------------------------+                  |
|                                                                                               |
+-----------------------------------------------------------------------------------------------+
 PICKER  [1/1] e2e-pane-a | j/k:nav Enter:select a:add-all Esc:cancel
```

**Three panes in grid layout (auto-reflow):**
```
+-e2e-pane-a---------------------------++-tmuch-1774457453500-------------------+
|base-pane                              ||azureuser@devr:~/src/tmuch$            |
|                                       ||                                       |
+---------------------------------------++---------------------------------------+
+-tmuch-1774457456019------------------------------------------------------+
|azureuser@devr:~/src/tmuch$                                                |
|                                                                           |
+--------------------------------------------------------------------------+
 NORMAL  [3/3] tmuch-1774457456019 | ^A:add ^D:drop ^S:list Tab:next Enter:focus
```

**Focused mode (Enter on a pane):**
```
+-e2e-pane-a [ATTACHED]---------------------------------------------------------------------+
|                                                                                            |
|                                                                                            |
+--------------------------------------------------------------------------------------------+
 FOCUSED  [1/1] e2e-pane-a | Esc:unfocus
```

## Install

```bash
cargo install --path .
```

Or download a prebuilt binary from [Releases](https://github.com/rysweet/tmuch/releases).

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
tmuch -b '1=amplihack copilot' -b '2=amplihack claude'

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

Configure remote hosts in `~/.config/tmuch/config.toml`:

```toml
[[remote]]
name = "devbox"
host = "devbox.internal"
user = "azureuser"
key = "~/.ssh/id_ed25519"
port = 22
poll_interval_ms = 500
```

### Programmable Panes

```bash
# watch: runs a command periodically (watch:command:interval_ms)
tmuch -n 'watch:date:1000'
tmuch -n 'watch:kubectl get pods:5000'

# tail: follows a file with tail -f
tmuch -n 'tail:/var/log/syslog'
tmuch -n 'tail:/tmp/app.log'

# http: polls a URL periodically (http:url:interval_ms)
tmuch -n 'http:http://localhost:8080/health:3000'
tmuch -n 'http:https://api.example.com/status'
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

While running tmuch, press `Ctrl-Z` to open the session picker pre-populated with all discovered Azure VM sessions. Press `a` in the picker to add all sessions at once.

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
| `Ctrl-Q` | Quit |
| `Ctrl-A` | Add new pane (creates new tmux session) |
| `Ctrl-D` | Drop focused pane |
| `Ctrl-S` | Open session picker |
| `Ctrl-Z` | Discover Azure VMs (azlin picker) |
| `Tab` | Focus next pane |
| `Shift-Tab` | Focus previous pane |
| `Enter` | Enter pane-focused mode |
| `1`-`9` | Run configured command binding |

### Pane-Focused Mode

All keystrokes are forwarded to the tmux session. Press `Esc` to return to Normal mode.

### Session Picker

| Key | Action |
|-----|--------|
| `j` / down | Navigate down |
| `k` / up | Navigate up |
| `Enter` | Attach selected session |
| `a` | Add all sessions at once |
| `Esc` | Cancel |

## Configuration

`~/.config/tmuch/config.toml`:

```toml
[bindings]
1 = "amplihack copilot"
2 = "amplihack claude"
3 = "tail -f /var/log/syslog"
4 = "top"
5 = "docker logs -f app"

[keys]
quit = "Ctrl-q"
add_pane = "Ctrl-a"
drop_pane = "Ctrl-d"
next_pane = "Tab"
prev_pane = "Shift-Tab"
select_session = "Ctrl-s"

[display]
poll_interval_ms = 150
border_style = "rounded"

# Remote SSH hosts
[[remote]]
name = "devbox"
host = "devbox.internal"
user = "azureuser"
key = "~/.ssh/id_ed25519"
port = 22
poll_interval_ms = 500

# Azure VM discovery
[azlin]
enabled = true
resource_group = "my-rg"
```

### Config Validation

On startup, tmuch checks for common configuration issues:
- If `azlin.enabled = true` but the `az` CLI is not found in PATH, a warning is printed.
- If remote hosts are configured but no SSH key is found (`~/.ssh/azlin_key`, `~/.ssh/id_rsa`, `~/.ssh/id_ed25519`), a warning is printed.

## Auto-Update

tmuch checks for updates on startup (non-blocking, 24-hour cache). Suppress with:

```bash
export TMUCH_NO_UPDATE_CHECK=1
```

## Layout

Panes auto-arrange in a grid: `cols = ceil(sqrt(n))`, `rows = ceil(n/cols)`. The layout recomputes on every pane add/remove and terminal resize.

| Panes | Layout |
|-------|--------|
| 1 | Full screen |
| 2 | Side by side |
| 3-4 | 2x2 grid |
| 5-6 | 3x2 grid |
| N | `ceil(sqrt(N))` columns |

## Architecture

```
src/
  main.rs               CLI args (clap), config loading, entry point
  app.rs                Event loop, state machine, azlin runner
  config.rs             TOML config parsing, startup validation
  consts.rs             Shared constants, version, sanitization
  tmux.rs               All tmux subprocess interaction
  layout.rs             Grid computation (pure function)
  pane.rs               Pane state management
  ui.rs                 ratatui rendering
  keys.rs               Key event handling, mode switching
  session_picker.rs     Session list popup (local + remote)
  azlin_integration.rs  Azure VM discovery, bastion tunnel management
  update_check.rs       Non-blocking background update check
  self_update.rs        Binary self-update from GitHub Releases
  source/
    mod.rs              ContentSource trait, PaneSpec enum, parse_new_arg()
    local_tmux.rs       Local tmux session source
    ssh_tmux.rs         Remote SSH tmux source (persistent connection, reconnect, remote resize)
    command.rs          Periodic command execution (watch:)
    tail.rs             File tailing (tail:)
    http.rs             HTTP URL polling (http:)
```

Key design: `tmux capture-pane -p -e` polling at 150ms. Each pane's tmux window is resized to match the ratatui cell dimensions so line wrapping is correct. ANSI escape codes are preserved and rendered via `ansi-to-tui`.

SSH sources use azlin-ssh for connection pooling. The background task holds a persistent connection, with automatic reconnection on error (shows "Reconnecting..." in the pane, waits 5 seconds, retries). Remote tmux windows are resized to match the local pane dimensions before each capture.

## Testing

```bash
# Unit tests
cargo test

# E2E tests via tmux
./tests/run-e2e.sh

# Lint
cargo clippy --all-targets -- -D warnings
```

## License

MIT
