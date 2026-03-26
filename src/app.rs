use crate::config::Config;
use crate::ipc::{IpcCommand, IpcServer};
use crate::keys::{self, Action, Mode};
use crate::layout::{PaneId, SplitDirection};
use crate::layouts;
use crate::pane::PaneManager;
use crate::session_picker::SessionPicker;
use crate::source::clock::ClockSource;
use crate::source::command::CommandSource;
use crate::source::http::HttpSource;
use crate::source::local_tmux::LocalTmuxSource;
use crate::source::registry::PluginRegistry;
use crate::source::snake::SnakeSource;
use crate::source::sparkline_monitor::SparklineSource;
use crate::source::ssh_subprocess::{RemoteConfig, SshSubprocessSource};
use crate::source::sysinfo::SysInfoSource;
use crate::source::tail::TailSource;
use crate::source::weather::WeatherSource;
use crate::source::{self, NewPaneRequest, PaneSpec};
use crate::theme::Theme;
use crate::tmux;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, MouseEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

/// State for the app launcher overlay.
pub struct AppLauncherState {
    pub apps: Vec<(&'static str, &'static str, &'static str)>, // (name, desc, usage)
    pub selected: usize,
}

impl AppLauncherState {
    pub fn new() -> Self {
        let registry = crate::source::registry::PluginRegistry::new();
        let apps: Vec<_> = registry
            .list()
            .iter()
            .map(|info| (info.name, info.description, info.usage))
            .collect();
        Self { apps, selected: 0 }
    }

    pub fn select_next(&mut self) {
        if !self.apps.is_empty() {
            self.selected = (self.selected + 1) % self.apps.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.apps.is_empty() {
            self.selected = if self.selected == 0 {
                self.apps.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_usage(&self) -> Option<&'static str> {
        self.apps.get(self.selected).map(|(_, _, u)| *u)
    }
}

/// Input mode for the command editor overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorInputMode {
    Browse,
    InputKey,
    InputCommand,
}

/// State for the command editor overlay.
pub struct CommandEditorState {
    pub entries: Vec<(char, String)>,
    pub selected: usize,
    pub input_mode: EditorInputMode,
    pub input_buffer: String,
    pub pending_key: Option<char>,
}

impl CommandEditorState {
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn delete_selected(&mut self) -> Option<char> {
        if self.entries.is_empty() {
            return None;
        }
        let (key, _) = self.entries.remove(self.selected);
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
        Some(key)
    }
}

/// State for tracking border drag resize.
pub struct DragState {
    /// Path to the split node being dragged.
    pub split_path: Vec<usize>,
    /// Direction of the split being dragged.
    pub direction: SplitDirection,
    /// Area of the parent split node, used to compute ratios.
    pub parent_area: Rect,
}

pub struct App {
    pub pane_manager: PaneManager,
    pub config: Config,
    pub mode: Mode,
    pub picker: SessionPicker,
    pub should_quit: bool,
    pub command_editor: Option<CommandEditorState>,
    pub pane_rects: Vec<(PaneId, Rect)>,
    pub theme: Theme,
    pub plugin_registry: PluginRegistry,
    pub drag_state: Option<DragState>,
    pub app_launcher: Option<AppLauncherState>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let theme = Theme::load();
        Self {
            pane_manager: PaneManager::new(),
            config,
            mode: Mode::Normal,
            picker: SessionPicker::new(),
            should_quit: false,
            command_editor: None,
            pane_rects: Vec::new(),
            theme,
            app_launcher: None,
            plugin_registry: PluginRegistry::new(),
            drag_state: None,
        }
    }

    pub fn editor_input_mode(&self) -> EditorInputMode {
        self.command_editor
            .as_ref()
            .map(|e| e.input_mode.clone())
            .unwrap_or(EditorInputMode::Browse)
    }

    pub fn add_local_tmux(&mut self, name: &str, _owned: bool) {
        let source = LocalTmuxSource::attach(name.to_string());
        self.pane_manager.add(Box::new(source));
    }

    pub fn create_local_tmux(&mut self, command: Option<&str>) -> Result<()> {
        let name = tmux::generate_session_name();
        let source = LocalTmuxSource::create(name, command)?;
        self.pane_manager.add(Box::new(source));
        Ok(())
    }

    pub fn add_from_spec(&mut self, spec: &PaneSpec) -> Result<()> {
        match spec {
            PaneSpec::LocalTmux {
                session,
                create_cmd,
            } => {
                if tmux::session_exists(session) {
                    self.pane_manager
                        .add(Box::new(LocalTmuxSource::attach(session.clone())));
                } else if let Some(cmd) = create_cmd {
                    let source = LocalTmuxSource::create(session.clone(), Some(cmd))?;
                    self.pane_manager.add(Box::new(source));
                } else {
                    let source = LocalTmuxSource::create(session.clone(), None)?;
                    self.pane_manager.add(Box::new(source));
                }
            }
            PaneSpec::Command {
                command,
                interval_ms,
            } => {
                let source = CommandSource::new(command.clone(), *interval_ms);
                self.pane_manager.add(Box::new(source));
            }
            PaneSpec::Tail { path } => {
                let source = TailSource::new(path)?;
                self.pane_manager.add(Box::new(source));
            }
            PaneSpec::Http { url, interval_ms } => {
                let source = HttpSource::new(url.clone(), *interval_ms);
                self.pane_manager.add(Box::new(source));
            }
            PaneSpec::Plugin {
                plugin_name,
                config,
            } => {
                if let Some(source) = self.plugin_registry.create(plugin_name, config.clone()) {
                    self.pane_manager.add(source);
                } else {
                    anyhow::bail!("Unknown plugin '{}'", plugin_name);
                }
            }
            PaneSpec::Remote {
                remote_name,
                session,
            } => {
                let remote = self
                    .config
                    .remote
                    .iter()
                    .find(|r| r.name == *remote_name)
                    .ok_or_else(|| anyhow::anyhow!("Remote '{}' not found in config", remote_name))?
                    .clone();
                let source = SshSubprocessSource::new(
                    remote.name.clone(),
                    remote.host.clone(),
                    remote.user.clone(),
                    remote.port,
                    session.clone(),
                    remote.poll_interval_ms,
                );
                self.pane_manager.add(Box::new(source));
            }
        }
        Ok(())
    }

    /// Attach a remote tmux session by parsing user@host:session syntax.
    pub fn attach_remote(&mut self, host_session: &str) -> Result<()> {
        // Parse: [user@]host:session
        let (user_host, session) = host_session
            .rsplit_once(':')
            .ok_or_else(|| anyhow::anyhow!("Remote format: [user@]host:session"))?;

        let (user, host) = if let Some((u, h)) = user_host.split_once('@') {
            (u.to_string(), h.to_string())
        } else {
            ("azureuser".to_string(), user_host.to_string())
        };

        // Check if this matches a named remote from config
        let remote = self
            .config
            .remote
            .iter()
            .find(|r| r.host == host || r.name == host)
            .cloned()
            .unwrap_or(RemoteConfig {
                name: host.clone(),
                host: host.clone(),
                user,
                key: None,
                port: 22,
                poll_interval_ms: 500,
            });

        let source = SshSubprocessSource::new(
            remote.name.clone(),
            remote.host.clone(),
            remote.user.clone(),
            remote.port,
            session.to_string(),
            remote.poll_interval_ms,
        );
        self.pane_manager.add(Box::new(source));
        Ok(())
    }

    fn add_remote_session_pane(&mut self, host: &str, session_name: &str) {
        if let Some(remote) = self.config.remote.iter().find(|r| r.name == host).cloned() {
            let source = SshSubprocessSource::new(
                remote.name.clone(),
                remote.host.clone(),
                remote.user.clone(),
                remote.port,
                session_name.to_string(),
                remote.poll_interval_ms,
            );
            self.pane_manager.add(Box::new(source));
        }
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => self.should_quit = true,
            Action::AddPane => {
                self.create_local_tmux(None)?;
            }
            Action::DropPane => {
                self.pane_manager.remove_focused();
            }
            Action::FocusNext => self.pane_manager.focus_next(),
            Action::FocusPrev => self.pane_manager.focus_prev(),
            Action::EnterPaneMode => {
                if let Some(pane) = self.pane_manager.focused() {
                    if pane.is_interactive() {
                        self.mode = Mode::PaneFocused;
                    }
                }
            }
            Action::ExitPaneMode => {
                self.mode = Mode::Normal;
            }
            Action::OpenSessionPicker => {
                if self.config.remote.is_empty() && !self.config.azlin.enabled {
                    self.picker.refresh()?;
                } else {
                    self.picker
                        .refresh_with_remotes(&self.config.remote, &self.config.azlin)?;
                }
                self.mode = Mode::SessionPicker;
            }
            Action::PickerUp => self.picker.select_prev(),
            Action::PickerDown => self.picker.select_next(),
            Action::PickerConfirm => {
                if let Some(session) = self.picker.confirm() {
                    let name = session.name.clone();
                    let host = session.host.clone();
                    if let Some(host) = &host {
                        self.add_remote_session_pane(host, &name);
                    } else {
                        self.pane_manager
                            .add(Box::new(LocalTmuxSource::attach(name)));
                    }
                }
                self.mode = Mode::Normal;
            }
            Action::PickerCancel => {
                self.mode = Mode::Normal;
            }
            Action::RunBinding(cmd) => {
                let name = tmux::generate_session_name();
                let source = LocalTmuxSource::create(name, Some(&cmd))?;
                self.pane_manager.add(Box::new(source));
            }
            Action::SendKeys(keys) => {
                if let Some(pane) = self.pane_manager.focused_mut() {
                    let _ = pane.source.send_keys(&keys);
                }
            }
            Action::DiscoverAzlin => {
                // Open picker pre-populated with azlin VM sessions
                if self.config.azlin.enabled {
                    self.picker
                        .refresh_with_remotes(&self.config.remote, &self.config.azlin)?;
                } else {
                    // Even without azlin config, try discovery
                    let azlin_cfg = crate::azlin_integration::AzlinConfig {
                        enabled: true,
                        resource_group: None,
                    };
                    self.picker
                        .refresh_with_remotes(&self.config.remote, &azlin_cfg)?;
                }
                self.mode = Mode::SessionPicker;
            }
            Action::PickerScanAzlin => {
                // Scan azlin VMs and add their sessions to the picker
                let rg = self.config.azlin.resource_group.as_deref();
                let result = crate::azlin_integration::discover_remote_sessions_sync(rg);
                if let Ok(remote_sessions) = result {
                    for session in remote_sessions {
                        let already_listed = self
                            .picker
                            .sessions
                            .iter()
                            .any(|s| s.name == session.name && s.host == session.host);
                        if !already_listed {
                            self.picker.sessions.push(session);
                        }
                    }
                }
                // Stay in picker mode
            }
            Action::PickerAddAll => {
                let sessions: Vec<_> = self.picker.sessions.clone();
                for session in &sessions {
                    let name = session.name.clone();
                    if let Some(host) = &session.host {
                        self.add_remote_session_pane(host, &name);
                    } else {
                        self.pane_manager
                            .add(Box::new(LocalTmuxSource::attach(name)));
                    }
                }
                self.mode = Mode::Normal;
            }
            Action::OpenSettings => {
                let source = crate::source::settings::SettingsSource::from_config(&self.config);
                self.pane_manager.add(Box::new(source));
            }
            Action::EditorUp => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.select_prev();
                }
            }
            Action::EditorDown => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.select_next();
                }
            }
            Action::EditorDelete => {
                if let Some(ref mut editor) = self.command_editor {
                    if let Some(key) = editor.delete_selected() {
                        self.config.bindings.remove(&key);
                        let _ = crate::config::save_bindings(&self.config.bindings);
                    }
                }
            }
            Action::EditorAdd => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.input_mode = EditorInputMode::InputKey;
                    editor.input_buffer.clear();
                    editor.pending_key = None;
                }
            }
            Action::EditorEdit => {
                if let Some(ref mut editor) = self.command_editor {
                    if let Some((key, cmd)) = editor.entries.get(editor.selected).cloned() {
                        editor.pending_key = Some(key);
                        editor.input_buffer = cmd;
                        editor.input_mode = EditorInputMode::InputCommand;
                    }
                }
            }
            Action::EditorSetKey(c) => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.pending_key = Some(c);
                    editor.input_buffer.clear();
                    editor.input_mode = EditorInputMode::InputCommand;
                }
            }
            Action::EditorTypeChar(c) => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.input_buffer.push(c);
                }
            }
            Action::EditorBackspace => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.input_buffer.pop();
                }
            }
            Action::EditorConfirm => {
                if let Some(ref mut editor) = self.command_editor {
                    if let Some(key) = editor.pending_key {
                        let cmd = editor.input_buffer.clone();
                        if !cmd.is_empty() {
                            self.config.bindings.insert(key, cmd.clone());
                            // Update entries list
                            if let Some(entry) = editor.entries.iter_mut().find(|(k, _)| *k == key)
                            {
                                entry.1 = cmd;
                            } else {
                                editor.entries.push((key, cmd));
                                editor.entries.sort_by_key(|(k, _)| *k);
                            }
                            let _ = crate::config::save_bindings(&self.config.bindings);
                        }
                    }
                    editor.input_mode = EditorInputMode::Browse;
                    editor.input_buffer.clear();
                    editor.pending_key = None;
                }
            }
            Action::EditorCancelInput => {
                if let Some(ref mut editor) = self.command_editor {
                    editor.input_mode = EditorInputMode::Browse;
                    editor.input_buffer.clear();
                    editor.pending_key = None;
                }
            }
            Action::EditorClose => {
                self.command_editor = None;
                self.mode = Mode::Normal;
            }
            Action::SplitVertical => {
                let name = tmux::generate_session_name();
                if let Ok(source) = LocalTmuxSource::create(name, None) {
                    self.pane_manager
                        .split_focused(SplitDirection::Vertical, Box::new(source));
                }
            }
            Action::SplitHorizontal => {
                let name = tmux::generate_session_name();
                if let Ok(source) = LocalTmuxSource::create(name, None) {
                    self.pane_manager
                        .split_focused(SplitDirection::Horizontal, Box::new(source));
                }
            }
            Action::ToggleMaximize => {
                self.pane_manager.toggle_maximize();
            }
            Action::SwapPane => {
                self.pane_manager.swap_focused_with_next();
            }
            Action::OpenAppLauncher => {
                self.app_launcher = Some(AppLauncherState::new());
                self.mode = Mode::AppLauncher;
            }
            Action::LauncherUp => {
                if let Some(ref mut launcher) = self.app_launcher {
                    launcher.select_prev();
                }
            }
            Action::LauncherDown => {
                if let Some(ref mut launcher) = self.app_launcher {
                    launcher.select_next();
                }
            }
            Action::LauncherConfirm => {
                if let Some(ref launcher) = self.app_launcher {
                    if let Some(usage) = launcher.selected_usage() {
                        // Parse the usage string as a -n argument
                        let arg = usage.split_once(' ').map(|(u, _)| u).unwrap_or(usage);
                        let request = crate::source::parse_new_arg(arg);
                        match request {
                            NewPaneRequest::TmuxCommand { command } => {
                                let _ = self.create_local_tmux(Some(&command));
                            }
                            NewPaneRequest::Command {
                                command,
                                interval_ms,
                            } => {
                                let source = CommandSource::new(command, interval_ms);
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Tail { path } => {
                                if let Ok(source) = TailSource::new(&path) {
                                    self.pane_manager.add(Box::new(source));
                                }
                            }
                            NewPaneRequest::Http { url, interval_ms } => {
                                let source = HttpSource::new(url, interval_ms);
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Clock => {
                                let source = crate::source::clock::ClockSource;
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Weather { city, interval_ms } => {
                                let source =
                                    crate::source::weather::WeatherSource::new(city, interval_ms);
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::SysInfo { interval_ms } => {
                                let source =
                                    crate::source::sysinfo::SysInfoSource::new(interval_ms);
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Snake => {
                                let source = crate::source::snake::SnakeSource::new();
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Sparkline {
                                command,
                                interval_ms,
                            } => {
                                let source = crate::source::sparkline_monitor::SparklineSource::new(
                                    command,
                                    interval_ms,
                                );
                                self.pane_manager.add(Box::new(source));
                            }
                            NewPaneRequest::Settings => {
                                let source = crate::source::settings::SettingsSource::from_config(
                                    &self.config,
                                );
                                self.pane_manager.add(Box::new(source));
                            }
                        }
                    }
                }
                self.app_launcher = None;
                self.mode = Mode::Normal;
            }
            Action::LauncherCancel => {
                self.app_launcher = None;
                self.mode = Mode::Normal;
            }
        }
        Ok(())
    }

    /// Handle an IPC command, returning a JSON response string.
    pub fn handle_ipc(&mut self, cmd: IpcCommand) -> String {
        match cmd {
            IpcCommand::ListPanes => {
                let panes: Vec<serde_json::Value> = self
                    .pane_manager
                    .panes()
                    .iter()
                    .map(|(id, p)| {
                        serde_json::json!({
                            "id": id,
                            "name": p.name(),
                            "source": p.source_label(),
                        })
                    })
                    .collect();
                serde_json::json!({ "ok": true, "panes": panes }).to_string()
            }
            IpcCommand::AddPane(spec) => match self.add_from_spec(&spec) {
                Ok(()) => serde_json::json!({ "ok": true }).to_string(),
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }).to_string(),
            },
            IpcCommand::RemovePane(id) => {
                self.pane_manager.remove(id);
                serde_json::json!({ "ok": true }).to_string()
            }
            IpcCommand::FocusPane(id) => {
                self.pane_manager.focus_id(id);
                serde_json::json!({ "ok": true }).to_string()
            }
            IpcCommand::Split { direction, spec } => {
                let dir = if direction == "horizontal" {
                    SplitDirection::Horizontal
                } else {
                    SplitDirection::Vertical
                };
                // Create the source from spec, then split
                let result = match &spec {
                    PaneSpec::LocalTmux {
                        session,
                        create_cmd,
                    } => {
                        if tmux::session_exists(session) {
                            Ok(Box::new(LocalTmuxSource::attach(session.clone()))
                                as Box<dyn crate::source::ContentSource>)
                        } else {
                            LocalTmuxSource::create(session.clone(), create_cmd.as_deref())
                                .map(|s| Box::new(s) as Box<dyn crate::source::ContentSource>)
                        }
                    }
                    PaneSpec::Command {
                        command,
                        interval_ms,
                    } => Ok(Box::new(CommandSource::new(command.clone(), *interval_ms))
                        as Box<dyn crate::source::ContentSource>),
                    PaneSpec::Plugin {
                        plugin_name,
                        config,
                    } => self
                        .plugin_registry
                        .create(plugin_name, config.clone())
                        .ok_or_else(|| anyhow::anyhow!("Unknown plugin")),
                    _ => Err(anyhow::anyhow!("Unsupported spec for split")),
                };
                match result {
                    Ok(source) => {
                        self.pane_manager.split_focused(dir, source);
                        serde_json::json!({ "ok": true }).to_string()
                    }
                    Err(e) => {
                        serde_json::json!({ "ok": false, "error": e.to_string() }).to_string()
                    }
                }
            }
            IpcCommand::Maximize(id) => {
                self.pane_manager.focus_id(id);
                self.pane_manager.toggle_maximize();
                serde_json::json!({ "ok": true }).to_string()
            }
            IpcCommand::SendKeys { id, keys } => {
                if let Some(pane) = self.pane_manager.get_mut(id) {
                    let _ = pane.source.send_keys(&keys);
                }
                serde_json::json!({ "ok": true }).to_string()
            }
            IpcCommand::Quit => {
                self.should_quit = true;
                serde_json::json!({ "ok": true }).to_string()
            }
        }
    }

    /// Handle mouse-down for border drag detection.
    pub fn handle_mouse_down(&mut self, col: u16, row: u16, main_area: Rect) {
        // Check if click is on a split boundary for drag resize
        if let Some(layout) = self.pane_manager.layout() {
            if let Some(split_ref) = layout.find_split_at(col, row, main_area, 1) {
                self.drag_state = Some(DragState {
                    split_path: split_ref.path,
                    direction: split_ref.direction,
                    parent_area: split_ref.area,
                });
                return;
            }
        }

        // Otherwise, focus the pane under cursor
        for (id, rect) in &self.pane_rects {
            if col >= rect.x
                && col < rect.x + rect.width
                && row >= rect.y
                && row < rect.y + rect.height
            {
                self.pane_manager.focus_id(*id);
                break;
            }
        }
    }

    /// Handle mouse drag for border resize.
    pub fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        if let Some(ref drag) = self.drag_state {
            let path = drag.split_path.clone();
            let direction = drag.direction;
            let parent_area = drag.parent_area;

            let ratio = match direction {
                SplitDirection::Vertical => {
                    if parent_area.width == 0 {
                        return;
                    }
                    let rel = col.saturating_sub(parent_area.x) as f32 / parent_area.width as f32;
                    rel.clamp(0.1, 0.9)
                }
                SplitDirection::Horizontal => {
                    if parent_area.height == 0 {
                        return;
                    }
                    let rel = row.saturating_sub(parent_area.y) as f32 / parent_area.height as f32;
                    rel.clamp(0.1, 0.9)
                }
            };

            self.pane_manager.set_ratio_at(&path, ratio);
        }
    }

    /// Handle mouse up — stop dragging.
    pub fn handle_mouse_up(&mut self) {
        self.drag_state = None;
    }
}

/// Run azlin discovery: list all VMs and their tmux sessions, then launch TUI.
pub fn run_azlin(resource_group: Option<String>) -> Result<()> {
    let config = crate::config::load()?;

    // Use CLI arg, then config, then azlin native config
    let rg = resource_group.or(config.azlin.resource_group.clone());

    if rg.is_none() {
        eprintln!(
            "\x1b[33mNo resource group specified. Use -r <RG> or set default_resource_group in ~/.azlin/config.toml\x1b[0m"
        );
    }

    eprintln!(
        "Discovering Azure VMs{}...",
        rg.as_ref()
            .map(|r| format!(" in {}", r))
            .unwrap_or_default()
    );
    let vms = crate::azlin_integration::discover_vms(rg.as_deref())?;

    if vms.is_empty() {
        eprintln!("No running VMs found.");
        return Ok(());
    }

    eprintln!(
        "Found {} running VM(s). Listing tmux sessions...",
        vms.len()
    );

    // Discover sessions using SSH subprocess (works with all auth methods)
    let discovered = crate::azlin_integration::discover_remote_sessions_sync(rg.as_deref())?;

    if discovered.is_empty() {
        eprintln!("No tmux sessions found on any VM.");
        return Ok(());
    }

    for s in &discovered {
        eprintln!("  {}:{}", s.host.as_deref().unwrap_or("?"), s.name);
    }

    // Launch TUI with all discovered sessions
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    io::stdout().execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    // Create SshSubprocessSource panes (uses system ssh)
    for session_info in &discovered {
        let vm_name = session_info.host.as_deref().unwrap_or("unknown");
        let vm = vms.iter().find(|v| v.name == vm_name);
        if let Some(vm) = vm {
            if let Ok(remote) = crate::azlin_integration::vm_to_remote_config(vm) {
                let source = SshSubprocessSource::new(
                    remote.name,
                    remote.host,
                    remote.user,
                    remote.port,
                    session_info.name.clone(),
                    remote.poll_interval_ms,
                );
                app.pane_manager.add(Box::new(source));
            }
        }
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        let term_size = terminal.size()?;
        let pane_count = app.pane_manager.count();
        if pane_count > 0 {
            // Reserve 2 rows: top hint bar + bottom status bar
            let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));
            let rects = app.pane_manager.resolve_layout(main_area);
            app.pane_rects = rects.clone();
            for (id, rect) in &rects {
                let w = rect.width.saturating_sub(2);
                let h = rect.height.saturating_sub(2);
                if w > 0 && h > 0 {
                    if let Some(pane) = app.pane_manager.get_mut(*id) {
                        if let Ok(content) = pane.source.capture(w, h) {
                            pane.content = content;
                        }
                    }
                }
            }
        }

        terminal.draw(|frame| ui::draw(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(poll_duration)? {
            let term_size = terminal.size()?;
            let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) =
                        keys::handle(key, &app.mode, &app.config, &app.editor_input_mode())
                    {
                        app.handle_action(action)?;
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_down(mouse.column, mouse.row, main_area);
                    }
                    MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_drag(mouse.column, mouse.row);
                    }
                    MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_up();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode()?;
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

pub fn run(
    config: Config,
    initial_sessions: Vec<String>,
    new_commands: Vec<String>,
    layout: Option<String>,
    save_layout: Option<String>,
) -> Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    io::stdout().execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    // Start IPC server
    let (ipc_tx, ipc_rx) = mpsc::channel();
    let _ipc_server = IpcServer::start(ipc_tx).ok();

    // Load layout if specified
    if let Some(layout_name) = &layout {
        let spec = layouts::load(layout_name)?;
        for pane_spec in &spec.panes {
            app.add_from_spec(pane_spec)?;
        }
    } else {
        // Attach initial sessions (supports user@host:session syntax for remote)
        for name in &initial_sessions {
            if name.contains(':') {
                // Remote session: user@host:session or host:session
                if let Err(e) = app.attach_remote(name) {
                    eprintln!("warning: remote session '{}': {}", name, e);
                }
            } else if tmux::session_exists(name) {
                app.add_local_tmux(name, false);
            }
        }

        // Create panes for new commands (supports watch:/tail: prefixes)
        for cmd in &new_commands {
            match source::parse_new_arg(cmd) {
                NewPaneRequest::TmuxCommand { command } => {
                    app.create_local_tmux(Some(&command))?;
                }
                NewPaneRequest::Command {
                    command,
                    interval_ms,
                } => {
                    let source = CommandSource::new(command, interval_ms);
                    app.pane_manager.add(Box::new(source));
                }
                NewPaneRequest::Tail { path } => {
                    let source = TailSource::new(&path)?;
                    app.pane_manager.add(Box::new(source));
                }
                NewPaneRequest::Http { url, interval_ms } => {
                    let source = HttpSource::new(url, interval_ms);
                    app.pane_manager.add(Box::new(source));
                }
                NewPaneRequest::Clock => {
                    app.pane_manager.add(Box::new(ClockSource));
                }
                NewPaneRequest::Weather { city, interval_ms } => {
                    app.pane_manager
                        .add(Box::new(WeatherSource::new(city, interval_ms)));
                }
                NewPaneRequest::SysInfo { interval_ms } => {
                    app.pane_manager
                        .add(Box::new(SysInfoSource::new(interval_ms)));
                }
                NewPaneRequest::Snake => {
                    app.pane_manager.add(Box::new(SnakeSource::new()));
                }
                NewPaneRequest::Sparkline {
                    command,
                    interval_ms,
                } => {
                    app.pane_manager
                        .add(Box::new(SparklineSource::new(command, interval_ms)));
                }
                NewPaneRequest::Settings => {
                    let source = crate::source::settings::SettingsSource::from_config(&app.config);
                    app.pane_manager.add(Box::new(source));
                }
            }
        }

        // If no panes, open session picker or create a default
        if app.pane_manager.is_empty() {
            let sessions = tmux::list_sessions()?;
            if sessions.is_empty() {
                app.create_local_tmux(None)?;
            } else {
                app.picker.refresh()?;
                app.mode = Mode::SessionPicker;
            }
        }
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        // Capture pane content with actual terminal dimensions
        let term_size = terminal.size()?;
        let pane_count = app.pane_manager.count();
        if pane_count > 0 {
            // Reserve 2 rows: top hint bar + bottom status bar
            let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));

            // If maximized, only capture the maximized pane at full area
            if let Some(max_id) = app.pane_manager.maximized {
                app.pane_rects = vec![(max_id, main_area)];
                let w = main_area.width.saturating_sub(2);
                let h = main_area.height.saturating_sub(2);
                if w > 0 && h > 0 {
                    if let Some(pane) = app.pane_manager.get_mut(max_id) {
                        if let Ok(content) = pane.source.capture(w, h) {
                            pane.content = content;
                        }
                    }
                }
            } else {
                let rects = app.pane_manager.resolve_layout(main_area);
                app.pane_rects = rects.clone();
                for (id, rect) in &rects {
                    let w = rect.width.saturating_sub(2);
                    let h = rect.height.saturating_sub(2);
                    if w > 0 && h > 0 {
                        if let Some(pane) = app.pane_manager.get_mut(*id) {
                            if let Ok(content) = pane.source.capture(w, h) {
                                pane.content = content;
                            }
                        }
                    }
                }
            }
        }

        // Draw
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if app.should_quit {
            break;
        }

        // Handle IPC commands
        while let Ok(msg) = ipc_rx.try_recv() {
            let response = app.handle_ipc(msg.command);
            let _ = msg.response_tx.send(response);
        }

        // Handle events
        if event::poll(poll_duration)? {
            let term_size = terminal.size()?;
            let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) =
                        keys::handle(key, &app.mode, &app.config, &app.editor_input_mode())
                    {
                        app.handle_action(action)?;
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_down(mouse.column, mouse.row, main_area);
                    }
                    MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_drag(mouse.column, mouse.row);
                    }
                    MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                        app.handle_mouse_up();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Save layout if requested
    if let Some(layout_name) = save_layout {
        let specs: Vec<PaneSpec> = app
            .pane_manager
            .panes()
            .iter()
            .map(|(_, p)| p.source.to_spec())
            .collect();
        layouts::save(&layouts::LayoutSpec {
            name: layout_name,
            panes: specs,
        })?;
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
