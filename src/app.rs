use crate::config::Config;
use crate::editor_state::{AppLauncherState, CommandEditorState, DragState, EditorInputMode};
use crate::keys::{Action, Mode};
use crate::layout::PaneId;
use crate::pane::PaneManager;
use crate::session_picker::SessionPicker;
use crate::source::registry::PluginRegistry;
use crate::source::PaneSpec;
use crate::theme::Theme;
use crate::tmux;
use anyhow::Result;
use ratatui::layout::Rect;

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
    /// Busy indicator message — shown in status bar + overlay when set
    pub busy: Option<String>,
    /// Spinner frame counter
    pub spinner_tick: usize,
    /// Currently highlighted menu tab index (for keyboard navigation)
    pub selected_hint: usize,
    /// Background task result receiver (for async operations like azlin discovery)
    pub bg_result: Option<std::sync::mpsc::Receiver<BgTaskResult>>,
}

/// Result from a background task.
pub enum BgTaskResult {
    AzlinSessions(Vec<crate::tmux::SessionInfo>),
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
            busy: None,
            spinner_tick: 0,
            selected_hint: 0,
            bg_result: None,
        }
    }

    pub fn editor_input_mode(&self) -> EditorInputMode {
        self.command_editor
            .as_ref()
            .map(|e| e.input_mode.clone())
            .unwrap_or(EditorInputMode::Browse)
    }

    pub fn add_local_tmux(&mut self, name: &str, _owned: bool) {
        let source = crate::source::local_tmux::LocalTmuxSource::attach(name.to_string());
        self.pane_manager.add(Box::new(source));
    }

    pub fn create_local_tmux(&mut self, command: Option<&str>) -> Result<()> {
        let name = tmux::generate_session_name();
        let source = crate::source::local_tmux::LocalTmuxSource::create(name, command)?;
        self.pane_manager.add(Box::new(source));
        Ok(())
    }

    // Delegate to pane_ops module
    pub fn add_from_spec(&mut self, spec: &PaneSpec) -> Result<()> {
        crate::pane_ops::add_from_spec(self, spec)
    }

    pub fn attach_remote(&mut self, host_session: &str) -> Result<()> {
        crate::pane_ops::attach_remote(self, host_session)
    }

    pub fn add_remote_session_pane(&mut self, host: &str, session_name: &str) {
        crate::pane_ops::add_remote_session_pane(self, host, session_name);
    }

    // Delegate to action_handler module
    pub fn handle_action(&mut self, action: Action) -> Result<()> {
        crate::action_handler::handle_action(self, action)
    }

    // Delegate to ipc_handler module
    pub fn handle_ipc(&mut self, cmd: crate::ipc::IpcCommand) -> String {
        crate::ipc_handler::handle_ipc(self, cmd)
    }

    // Delegate to mouse module
    pub fn handle_mouse_down(&mut self, col: u16, row: u16, main_area: Rect) {
        crate::mouse::handle_mouse_down(self, col, row, main_area);
    }

    pub fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        crate::mouse::handle_mouse_drag(self, col, row);
    }

    pub fn handle_mouse_up(&mut self) {
        crate::mouse::handle_mouse_up(self);
    }
}

// Delegate run functions to event_loop module
pub fn run_azlin(resource_group: Option<String>) -> Result<()> {
    crate::event_loop::run_azlin(resource_group)
}

pub fn run(
    config: Config,
    initial_sessions: Vec<String>,
    new_commands: Vec<String>,
    layout: Option<String>,
    save_layout: Option<String>,
) -> Result<()> {
    crate::event_loop::run(config, initial_sessions, new_commands, layout, save_layout)
}
