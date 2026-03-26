//! Dispatches high-level `Action` values to App state mutations.

use crate::app::App;
use crate::editor_state::{AppLauncherState, EditorInputMode};
use crate::keys::{Action, Mode};
use crate::layout::SplitDirection;
use crate::source::command::CommandSource;
use crate::source::http::HttpSource;
use crate::source::local_tmux::LocalTmuxSource;
use crate::source::tail::TailSource;
use crate::source::NewPaneRequest;
use crate::tmux;
use anyhow::Result;

pub fn handle_action(app: &mut App, action: Action) -> Result<()> {
    match action {
        Action::Quit => app.should_quit = true,
        Action::AddPane => {
            app.create_local_tmux(None)?;
        }
        Action::DropPane => {
            app.pane_manager.remove_focused();
        }
        Action::FocusNext => app.pane_manager.focus_next(),
        Action::FocusPrev => app.pane_manager.focus_prev(),
        Action::EnterPaneMode => {
            if let Some(pane) = app.pane_manager.focused() {
                if pane.is_interactive() {
                    app.mode = Mode::PaneFocused;
                }
            }
        }
        Action::ExitPaneMode => {
            app.mode = Mode::Normal;
        }
        Action::OpenSessionPicker => {
            if app.config.remote.is_empty() && !app.config.azlin.enabled {
                app.picker.refresh()?;
            } else {
                app.picker
                    .refresh_with_remotes(&app.config.remote, &app.config.azlin)?;
            }
            app.mode = Mode::SessionPicker;
        }
        Action::PickerUp => app.picker.select_prev(),
        Action::PickerDown => app.picker.select_next(),
        Action::PickerConfirm => {
            if let Some(session) = app.picker.confirm() {
                let name = session.name.clone();
                let host = session.host.clone();
                if let Some(host) = &host {
                    app.add_remote_session_pane(host, &name);
                } else {
                    app.pane_manager
                        .add(Box::new(LocalTmuxSource::attach(name)));
                }
            }
            app.mode = Mode::Normal;
        }
        Action::PickerCancel => {
            app.mode = Mode::Normal;
        }
        Action::RunBinding(cmd) => {
            let name = tmux::generate_session_name();
            let source = LocalTmuxSource::create(name, Some(&cmd))?;
            app.pane_manager.add(Box::new(source));
        }
        Action::SendKeys(keys) => {
            if let Some(pane) = app.pane_manager.focused_mut() {
                let _ = pane.source.send_keys(&keys);
            }
        }
        Action::DiscoverAzlin => {
            if app.config.azlin.enabled {
                app.picker
                    .refresh_with_remotes(&app.config.remote, &app.config.azlin)?;
            } else {
                let azlin_cfg = crate::azlin_integration::AzlinConfig {
                    enabled: true,
                    resource_group: None,
                    default_user: None,
                };
                app.picker
                    .refresh_with_remotes(&app.config.remote, &azlin_cfg)?;
            }
            app.mode = Mode::SessionPicker;
        }
        Action::PickerScanAzlin => {
            let rg = app.config.azlin.resource_group.as_deref();
            let result = crate::azlin_integration::discover_remote_sessions_sync(rg);
            if let Ok(remote_sessions) = result {
                for session in remote_sessions {
                    let already_listed = app
                        .picker
                        .sessions
                        .iter()
                        .any(|s| s.name == session.name && s.host == session.host);
                    if !already_listed {
                        app.picker.sessions.push(session);
                    }
                }
            }
        }
        Action::PickerAddAll => {
            let sessions: Vec<_> = app.picker.sessions.clone();
            for session in &sessions {
                let name = session.name.clone();
                if let Some(host) = &session.host {
                    app.add_remote_session_pane(host, &name);
                } else {
                    app.pane_manager
                        .add(Box::new(LocalTmuxSource::attach(name)));
                }
            }
            app.mode = Mode::Normal;
        }
        Action::OpenSettings => {
            let source = crate::source::settings::SettingsSource::from_config(&app.config);
            app.pane_manager.add(Box::new(source));
        }
        Action::EditorUp => {
            if let Some(ref mut editor) = app.command_editor {
                editor.select_prev();
            }
        }
        Action::EditorDown => {
            if let Some(ref mut editor) = app.command_editor {
                editor.select_next();
            }
        }
        Action::EditorDelete => {
            if let Some(ref mut editor) = app.command_editor {
                if let Some(key) = editor.delete_selected() {
                    app.config.bindings.remove(&key);
                    let _ = crate::config::save_bindings(&app.config.bindings);
                }
            }
        }
        Action::EditorAdd => {
            if let Some(ref mut editor) = app.command_editor {
                editor.input_mode = EditorInputMode::InputKey;
                editor.input_buffer.clear();
                editor.pending_key = None;
            }
        }
        Action::EditorEdit => {
            if let Some(ref mut editor) = app.command_editor {
                if let Some((key, cmd)) = editor.entries.get(editor.selected).cloned() {
                    editor.pending_key = Some(key);
                    editor.input_buffer = cmd;
                    editor.input_mode = EditorInputMode::InputCommand;
                }
            }
        }
        Action::EditorSetKey(c) => {
            if let Some(ref mut editor) = app.command_editor {
                editor.pending_key = Some(c);
                editor.input_buffer.clear();
                editor.input_mode = EditorInputMode::InputCommand;
            }
        }
        Action::EditorTypeChar(c) => {
            if let Some(ref mut editor) = app.command_editor {
                editor.input_buffer.push(c);
            }
        }
        Action::EditorBackspace => {
            if let Some(ref mut editor) = app.command_editor {
                editor.input_buffer.pop();
            }
        }
        Action::EditorConfirm => {
            if let Some(ref mut editor) = app.command_editor {
                if let Some(key) = editor.pending_key {
                    let cmd = editor.input_buffer.clone();
                    if !cmd.is_empty() {
                        app.config.bindings.insert(key, cmd.clone());
                        if let Some(entry) = editor.entries.iter_mut().find(|(k, _)| *k == key) {
                            entry.1 = cmd;
                        } else {
                            editor.entries.push((key, cmd));
                            editor.entries.sort_by_key(|(k, _)| *k);
                        }
                        let _ = crate::config::save_bindings(&app.config.bindings);
                    }
                }
                editor.input_mode = EditorInputMode::Browse;
                editor.input_buffer.clear();
                editor.pending_key = None;
            }
        }
        Action::EditorCancelInput => {
            if let Some(ref mut editor) = app.command_editor {
                editor.input_mode = EditorInputMode::Browse;
                editor.input_buffer.clear();
                editor.pending_key = None;
            }
        }
        Action::EditorClose => {
            app.command_editor = None;
            app.mode = Mode::Normal;
        }
        Action::SplitVertical => {
            let name = tmux::generate_session_name();
            if let Ok(source) = LocalTmuxSource::create(name, None) {
                app.pane_manager
                    .split_focused(SplitDirection::Vertical, Box::new(source));
            }
        }
        Action::SplitHorizontal => {
            let name = tmux::generate_session_name();
            if let Ok(source) = LocalTmuxSource::create(name, None) {
                app.pane_manager
                    .split_focused(SplitDirection::Horizontal, Box::new(source));
            }
        }
        Action::ToggleMaximize => {
            app.pane_manager.toggle_maximize();
        }
        Action::SwapPane => {
            app.pane_manager.swap_focused_with_next();
        }
        Action::OpenAppLauncher => {
            app.app_launcher = Some(AppLauncherState::new());
            app.mode = Mode::AppLauncher;
        }
        Action::LauncherUp => {
            if let Some(ref mut launcher) = app.app_launcher {
                launcher.select_prev();
            }
        }
        Action::LauncherDown => {
            if let Some(ref mut launcher) = app.app_launcher {
                launcher.select_next();
            }
        }
        Action::LauncherConfirm => {
            if let Some(ref launcher) = app.app_launcher {
                if let Some(usage) = launcher.selected_usage() {
                    let arg = usage.split_once(' ').map(|(u, _)| u).unwrap_or(usage);
                    let request = crate::source::parse_new_arg(arg);
                    handle_new_pane_request(app, request)?;
                }
            }
            app.app_launcher = None;
            app.mode = Mode::Normal;
        }
        Action::LauncherCancel => {
            app.app_launcher = None;
            app.mode = Mode::Normal;
        }
    }
    Ok(())
}

fn handle_new_pane_request(app: &mut App, request: NewPaneRequest) -> Result<()> {
    match request {
        NewPaneRequest::TmuxCommand { command } => {
            let _ = app.create_local_tmux(Some(&command));
        }
        NewPaneRequest::Command {
            command,
            interval_ms,
        } => {
            let source = CommandSource::new(command, interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::Tail { path } => {
            if let Ok(source) = TailSource::new(&path) {
                app.pane_manager.add(Box::new(source));
            }
        }
        NewPaneRequest::Http { url, interval_ms } => {
            let source = HttpSource::new(url, interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::Clock => {
            app.pane_manager
                .add(Box::new(crate::source::clock::ClockSource));
        }
        NewPaneRequest::Weather { city, interval_ms } => {
            let source = crate::source::weather::WeatherSource::new(city, interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::SysInfo { interval_ms } => {
            let source = crate::source::sysinfo::SysInfoSource::new(interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::Snake => {
            let source = crate::source::snake::SnakeSource::new();
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::Sparkline {
            command,
            interval_ms,
        } => {
            let source =
                crate::source::sparkline_monitor::SparklineSource::new(command, interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        NewPaneRequest::Settings => {
            let source = crate::source::settings::SettingsSource::from_config(&app.config);
            app.pane_manager.add(Box::new(source));
        }
    }
    Ok(())
}
