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

pub(crate) fn handle_new_pane_request(app: &mut App, request: NewPaneRequest) -> Result<()> {
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
        NewPaneRequest::DebugLog => {
            let source = crate::source::debug_log::DebugLogSource::new();
            app.pane_manager.add(Box::new(source));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::source::{ContentSource, PaneSpec};

    struct MockSource(String);

    impl ContentSource for MockSource {
        fn capture(&mut self, _w: u16, _h: u16) -> anyhow::Result<String> {
            Ok("mock".into())
        }
        fn send_keys(&mut self, _keys: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn name(&self) -> &str {
            &self.0
        }
        fn source_label(&self) -> &str {
            "mock"
        }
        fn is_interactive(&self) -> bool {
            true
        }
        fn to_spec(&self) -> PaneSpec {
            PaneSpec::Command {
                command: "mock".into(),
                interval_ms: 1000,
            }
        }
    }

    fn mock_app() -> App {
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("a".into())));
        app.pane_manager.add(Box::new(MockSource("b".into())));
        app
    }

    #[test]
    fn test_quit() {
        let mut app = mock_app();
        assert!(!app.should_quit);
        handle_action(&mut app, Action::Quit).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_drop_pane() {
        let mut app = mock_app();
        assert_eq!(app.pane_manager.count(), 2);
        handle_action(&mut app, Action::DropPane).unwrap();
        assert_eq!(app.pane_manager.count(), 1);
    }

    #[test]
    fn test_focus_next_prev() {
        let mut app = mock_app();
        let ids = app.pane_manager.pane_ids_in_order();
        app.pane_manager.focus_id(ids[0]);
        let initial = app.pane_manager.focused_id();
        handle_action(&mut app, Action::FocusNext).unwrap();
        assert_ne!(app.pane_manager.focused_id(), initial);
        handle_action(&mut app, Action::FocusPrev).unwrap();
        assert_eq!(app.pane_manager.focused_id(), initial);
    }

    #[test]
    fn test_enter_exit_pane_mode() {
        let mut app = mock_app();
        assert_eq!(app.mode, Mode::Normal);
        handle_action(&mut app, Action::EnterPaneMode).unwrap();
        assert_eq!(app.mode, Mode::PaneFocused);
        handle_action(&mut app, Action::ExitPaneMode).unwrap();
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_enter_pane_mode_no_panes() {
        let mut app = App::new(Config::default());
        handle_action(&mut app, Action::EnterPaneMode).unwrap();
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_picker_cancel() {
        let mut app = mock_app();
        app.mode = Mode::SessionPicker;
        handle_action(&mut app, Action::PickerCancel).unwrap();
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_picker_confirm_empty() {
        let mut app = mock_app();
        app.mode = Mode::SessionPicker;
        handle_action(&mut app, Action::PickerConfirm).unwrap();
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_picker_up_down() {
        let mut app = mock_app();
        handle_action(&mut app, Action::PickerUp).unwrap();
        handle_action(&mut app, Action::PickerDown).unwrap();
    }

    #[test]
    fn test_send_keys() {
        let mut app = mock_app();
        handle_action(&mut app, Action::SendKeys("Enter".into())).unwrap();
    }

    #[test]
    fn test_open_settings() {
        let mut app = mock_app();
        let count_before = app.pane_manager.count();
        handle_action(&mut app, Action::OpenSettings).unwrap();
        assert_eq!(app.pane_manager.count(), count_before + 1);
    }

    #[test]
    fn test_editor_operations_no_editor() {
        let mut app = mock_app();
        handle_action(&mut app, Action::EditorUp).unwrap();
        handle_action(&mut app, Action::EditorDown).unwrap();
        handle_action(&mut app, Action::EditorDelete).unwrap();
        handle_action(&mut app, Action::EditorAdd).unwrap();
        handle_action(&mut app, Action::EditorEdit).unwrap();
        handle_action(&mut app, Action::EditorSetKey('5')).unwrap();
        handle_action(&mut app, Action::EditorTypeChar('x')).unwrap();
        handle_action(&mut app, Action::EditorBackspace).unwrap();
        handle_action(&mut app, Action::EditorConfirm).unwrap();
        handle_action(&mut app, Action::EditorCancelInput).unwrap();
    }

    #[test]
    fn test_editor_close() {
        let mut app = mock_app();
        app.mode = Mode::CommandEditor;
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![],
            selected: 0,
            input_mode: crate::editor_state::EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        });
        handle_action(&mut app, Action::EditorClose).unwrap();
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.command_editor.is_none());
    }

    #[test]
    fn test_editor_with_state() {
        let mut app = mock_app();
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![('1', "top".into()), ('2', "htop".into())],
            selected: 0,
            input_mode: crate::editor_state::EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        });
        handle_action(&mut app, Action::EditorDown).unwrap();
        assert_eq!(app.command_editor.as_ref().unwrap().selected, 1);
        handle_action(&mut app, Action::EditorUp).unwrap();
        assert_eq!(app.command_editor.as_ref().unwrap().selected, 0);

        // Test add flow
        handle_action(&mut app, Action::EditorAdd).unwrap();
        assert_eq!(
            app.command_editor.as_ref().unwrap().input_mode,
            crate::editor_state::EditorInputMode::InputKey
        );
        handle_action(&mut app, Action::EditorSetKey('3')).unwrap();
        assert_eq!(
            app.command_editor.as_ref().unwrap().input_mode,
            crate::editor_state::EditorInputMode::InputCommand
        );
        handle_action(&mut app, Action::EditorTypeChar('l')).unwrap();
        handle_action(&mut app, Action::EditorTypeChar('s')).unwrap();
        assert_eq!(app.command_editor.as_ref().unwrap().input_buffer, "ls");
        handle_action(&mut app, Action::EditorBackspace).unwrap();
        assert_eq!(app.command_editor.as_ref().unwrap().input_buffer, "l");
        handle_action(&mut app, Action::EditorCancelInput).unwrap();
        assert_eq!(
            app.command_editor.as_ref().unwrap().input_mode,
            crate::editor_state::EditorInputMode::Browse
        );
    }

    #[test]
    fn test_editor_edit_and_confirm() {
        let mut app = mock_app();
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![('1', "top".into())],
            selected: 0,
            input_mode: crate::editor_state::EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        });
        handle_action(&mut app, Action::EditorEdit).unwrap();
        assert_eq!(
            app.command_editor.as_ref().unwrap().input_mode,
            crate::editor_state::EditorInputMode::InputCommand
        );
        assert_eq!(app.command_editor.as_ref().unwrap().pending_key, Some('1'));
        // Type new command
        handle_action(&mut app, Action::EditorTypeChar('h')).unwrap();
        handle_action(&mut app, Action::EditorTypeChar('t')).unwrap();
        // Confirm saves
        handle_action(&mut app, Action::EditorConfirm).unwrap();
    }

    #[test]
    fn test_toggle_maximize() {
        let mut app = mock_app();
        assert!(app.pane_manager.maximized.is_none());
        handle_action(&mut app, Action::ToggleMaximize).unwrap();
        assert!(app.pane_manager.maximized.is_some());
        handle_action(&mut app, Action::ToggleMaximize).unwrap();
        assert!(app.pane_manager.maximized.is_none());
    }

    #[test]
    fn test_swap_pane() {
        let mut app = mock_app();
        handle_action(&mut app, Action::SwapPane).unwrap();
    }

    #[test]
    fn test_open_app_launcher() {
        let mut app = mock_app();
        handle_action(&mut app, Action::OpenAppLauncher).unwrap();
        assert_eq!(app.mode, Mode::AppLauncher);
        assert!(app.app_launcher.is_some());
    }

    #[test]
    fn test_launcher_navigation_and_cancel() {
        let mut app = mock_app();
        handle_action(&mut app, Action::OpenAppLauncher).unwrap();
        handle_action(&mut app, Action::LauncherDown).unwrap();
        handle_action(&mut app, Action::LauncherUp).unwrap();
        handle_action(&mut app, Action::LauncherCancel).unwrap();
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.app_launcher.is_none());
    }

    #[test]
    fn test_launcher_confirm() {
        let mut app = mock_app();
        handle_action(&mut app, Action::OpenAppLauncher).unwrap();
        let count_before = app.pane_manager.count();
        handle_action(&mut app, Action::LauncherConfirm).unwrap();
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.pane_manager.count() >= count_before);
    }

    #[test]
    fn test_handle_new_pane_request_clock() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(&mut app, NewPaneRequest::Clock).unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_snake() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(&mut app, NewPaneRequest::Snake).unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_command() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(
            &mut app,
            NewPaneRequest::Command {
                command: "echo hi".into(),
                interval_ms: 5000,
            },
        )
        .unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_http() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(
            &mut app,
            NewPaneRequest::Http {
                url: "http://localhost".into(),
                interval_ms: 5000,
            },
        )
        .unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_weather() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(
            &mut app,
            NewPaneRequest::Weather {
                city: "London".into(),
                interval_ms: 300_000,
            },
        )
        .unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_sysinfo() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(&mut app, NewPaneRequest::SysInfo { interval_ms: 2000 }).unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_sparkline() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(
            &mut app,
            NewPaneRequest::Sparkline {
                command: "echo 42".into(),
                interval_ms: 2000,
            },
        )
        .unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }

    #[test]
    fn test_handle_new_pane_request_settings() {
        let mut app = mock_app();
        let n = app.pane_manager.count();
        handle_new_pane_request(&mut app, NewPaneRequest::Settings).unwrap();
        assert_eq!(app.pane_manager.count(), n + 1);
    }
}
