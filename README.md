# tmuch

TUI tmux multiplexer — display multiple tmux sessions in a single terminal.

Built with [ratatui](https://ratatui.rs). Captures tmux session output via `tmux capture-pane` and renders it in auto-reflowing grid panes.

## Screenshots

**Two panes side-by-side:**
```
┌ e2e-pane-a ────────────────────────────────────────────────────────────────────────────────┐┌ e2e-pane-b ────────────────────────────────────────────────────────────────────────────────┐
│content-from-pane-a                                                                         ││content-from-pane-b                                                                         │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────┘└────────────────────────────────────────────────────────────────────────────────────────────┘
 NORMAL  [2/2] e2e-pane-b | ^A:add ^D:drop ^S:list Tab:next Enter:focus
```

**Session picker overlay (Ctrl-S):**
```
┌ e2e-pane-a ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                    ┌ tmux sessions ─────────────────────────────────┐                                                                    │
│                                                                    │▶ ampl                                          │                                                                    │
│                                                                    │  amplihack-bf                                  │                                                                    │
│                                                                    │  e2e-pane-a                                    │                                                                    │
│                                                                    │  e2e-pane-b                                    │                                                                    │
│                                                                    │  fixme                                         │                                                                    │
│                                                                    │  mem (attached)                                │                                                                    │
│                                                                    │  memr                                          │                                                                    │
│                                                                    │  tmuch-e2e-runner                              │                                                                    │
│                                                                    │                                                │                                                                    │
│                                                                    └────────────────────────────────────────────────┘                                                                    │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 PICKER  [1/1] e2e-pane-a | j/k:nav Enter:select Esc:cancel
```

**Three panes in grid layout (auto-reflow):**
```
┌ e2e-pane-a ────────────────────────────────────────────────────────────────────────────────┐┌ tmuch-1774457453500 ───────────────────────────────────────────────────────────────────────┐
│base-pane                                                                                   ││azureuser@devr:~/src/tmuch$                                                                 │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
│                                                                                            ││                                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────┘└────────────────────────────────────────────────────────────────────────────────────────────┘
┌ tmuch-1774457456019 ───────────────────────────────────────────────────────────────────────┐
│azureuser@devr:~/src/tmuch$                                                                 │
│                                                                                            │
│                                                                                            │
│                                                                                            │
│                                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────┘
 NORMAL  [3/3] tmuch-1774457456019 | ^A:add ^D:drop ^S:list Tab:next Enter:focus
```

**Focused mode (Enter on a pane):**
```
┌ e2e-pane-a [ATTACHED] ───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
│                                                                                                                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 FOCUSED  [1/1] e2e-pane-a | Esc:unfocus
```

## Install

```bash
cargo install --path .
```

Or download a prebuilt binary from [Releases](https://github.com/rysweet/tmuch/releases).

## Usage

```bash
# Attach to existing tmux sessions
tmuch session1 session2

# Create new sessions running commands
tmuch -n "tail -f /var/log/syslog" -n "top"

# Key bindings for quick commands
tmuch -b '1=amplihack copilot' -b '2=amplihack claude'

# Mix existing sessions and new commands
tmuch my-dev-server -n "htop" -b '3=docker logs -f app'

# Self-update
tmuch update
```

If launched with no arguments and tmux sessions exist, tmuch opens the session picker. If no sessions exist, it creates a default one.

## Key Bindings

### Normal Mode

| Key | Action |
|-----|--------|
| `Ctrl-Q` | Quit |
| `Ctrl-A` | Add new pane (creates new tmux session) |
| `Ctrl-D` | Drop focused pane |
| `Ctrl-S` | Open session picker |
| `Tab` | Focus next pane |
| `Shift-Tab` | Focus previous pane |
| `Enter` | Enter pane-focused mode |
| `1`-`9` | Run configured command binding |

### Pane-Focused Mode

All keystrokes are forwarded to the tmux session. Press `Esc` to return to Normal mode.

### Session Picker

| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate down |
| `k` / `↑` | Navigate up |
| `Enter` | Attach selected session |
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
```

## Auto-Update

tmuch checks for updates on startup (non-blocking, 24-hour cache). Suppress with:

```bash
export TMUCH_NO_UPDATE_CHECK=1
```

Update manually:

```bash
tmuch update
# or
tmuch self-update
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
  main.rs           CLI args (clap), config loading, entry point
  app.rs            Event loop, state machine
  config.rs         TOML config parsing
  consts.rs         Shared constants, version, sanitization
  tmux.rs           All tmux subprocess interaction
  layout.rs         Grid computation (pure function)
  pane.rs           Pane state management
  ui.rs             ratatui rendering
  keys.rs           Key event handling, mode switching
  session_picker.rs Session list popup
  update_check.rs   Non-blocking background update check
  self_update.rs    Binary self-update from GitHub Releases
```

Key design: `tmux capture-pane -p -e` polling at 150ms. Each pane's tmux window is resized to match the ratatui cell dimensions so line wrapping is correct. ANSI escape codes are preserved and rendered via `ansi-to-tui`.

## Testing

```bash
# Unit tests (22 tests)
cargo test

# E2E tests via tmux (33 assertions across 10 scenarios)
./tests/run-e2e.sh

# Lint
cargo clippy --all-targets -- -D warnings
```

## License

MIT
