use crate::config::Config;
use crate::editor_state::EditorInputMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    PaneFocused,
    SessionPicker,
    CommandEditor,
    AppLauncher,
}

#[derive(Debug)]
pub enum Action {
    Quit,
    AddPane,
    DropPane,
    FocusNext,
    FocusPrev,
    EnterPaneMode,
    ExitPaneMode,
    OpenSessionPicker,
    PickerUp,
    PickerDown,
    PickerConfirm,
    PickerCancel,
    RunBinding(String),
    SendKeys(String),
    DiscoverAzlin,
    PickerAddAll,
    PickerScanAzlin,
    OpenSettings,
    EditorUp,
    EditorDown,
    EditorDelete,
    EditorClose,
    EditorAdd,
    EditorEdit,
    EditorSetKey(char),
    EditorTypeChar(char),
    EditorBackspace,
    EditorConfirm,
    EditorCancelInput,
    SplitVertical,
    SplitHorizontal,
    ToggleMaximize,
    SwapPane,
    OpenAppLauncher,
    LauncherUp,
    LauncherDown,
    LauncherConfirm,
    LauncherCancel,
}

pub fn handle(
    event: KeyEvent,
    mode: &Mode,
    config: &Config,
    editor_input_mode: &EditorInputMode,
) -> Option<Action> {
    match mode {
        Mode::Normal => handle_normal(event, config),
        Mode::PaneFocused => handle_pane_focused(event),
        Mode::SessionPicker => handle_picker(event),
        Mode::CommandEditor => handle_command_editor(event, editor_input_mode),
        Mode::AppLauncher => handle_app_launcher(event),
    }
}

fn handle_normal(event: KeyEvent, config: &Config) -> Option<Action> {
    // Ctrl-key combos first
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return match event.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('a') => Some(Action::AddPane),
            KeyCode::Char('d') => Some(Action::DropPane),
            KeyCode::Char('s') => Some(Action::OpenSessionPicker),
            KeyCode::Char('g') => Some(Action::DiscoverAzlin),
            KeyCode::Char('e') => Some(Action::OpenSettings),
            KeyCode::Char('v') => Some(Action::SplitVertical),
            KeyCode::Char('h') => Some(Action::SplitHorizontal),
            KeyCode::Char('f') => Some(Action::ToggleMaximize),
            KeyCode::Char('x') => Some(Action::SwapPane),
            KeyCode::Char('n') => Some(Action::OpenAppLauncher),
            _ => None,
        };
    }

    match event.code {
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrev),
        KeyCode::Enter => Some(Action::EnterPaneMode),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::F(11) => Some(Action::ToggleMaximize),
        // Arrow keys for pane navigation
        KeyCode::Down | KeyCode::Right => Some(Action::FocusNext),
        KeyCode::Up | KeyCode::Left => Some(Action::FocusPrev),
        KeyCode::Char(c) => {
            // Check command bindings
            config
                .bindings
                .get(&c)
                .map(|cmd| Action::RunBinding(cmd.clone()))
        }
        _ => None,
    }
}

fn handle_pane_focused(event: KeyEvent) -> Option<Action> {
    // Escape exits pane-focused mode
    if event.code == KeyCode::Esc {
        return Some(Action::ExitPaneMode);
    }

    // Convert key event to tmux send-keys format
    let keys = key_to_tmux_string(event)?;
    Some(Action::SendKeys(keys))
}

fn handle_picker(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc => Some(Action::PickerCancel),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::PickerUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::PickerDown),
        KeyCode::Enter => Some(Action::PickerConfirm),
        KeyCode::Char('a') => Some(Action::PickerAddAll),
        KeyCode::Char('z') => Some(Action::PickerScanAzlin),
        _ => None,
    }
}

fn handle_command_editor(event: KeyEvent, input_mode: &EditorInputMode) -> Option<Action> {
    match input_mode {
        EditorInputMode::Browse => match event.code {
            KeyCode::Esc => Some(Action::EditorClose),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::EditorUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::EditorDown),
            KeyCode::Char('d') => Some(Action::EditorDelete),
            KeyCode::Char('a') => Some(Action::EditorAdd),
            KeyCode::Char('e') | KeyCode::Enter => Some(Action::EditorEdit),
            _ => None,
        },
        EditorInputMode::InputKey => match event.code {
            KeyCode::Esc => Some(Action::EditorCancelInput),
            KeyCode::Char(c) if c.is_ascii_digit() => Some(Action::EditorSetKey(c)),
            _ => None,
        },
        EditorInputMode::InputCommand => match event.code {
            KeyCode::Esc => Some(Action::EditorCancelInput),
            KeyCode::Enter => Some(Action::EditorConfirm),
            KeyCode::Backspace => Some(Action::EditorBackspace),
            KeyCode::Char(c) => Some(Action::EditorTypeChar(c)),
            _ => None,
        },
    }
}

fn handle_app_launcher(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc => Some(Action::LauncherCancel),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::LauncherUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::LauncherDown),
        KeyCode::Enter => Some(Action::LauncherConfirm),
        _ => None,
    }
}

pub(crate) fn key_to_tmux_string(event: KeyEvent) -> Option<String> {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = event.code {
            return Some(format!("C-{}", c));
        }
    }

    match event.code {
        KeyCode::Char(c) => Some(c.to_string()),
        KeyCode::Enter => Some("Enter".into()),
        KeyCode::Backspace => Some("BSpace".into()),
        KeyCode::Tab => Some("Tab".into()),
        KeyCode::Up => Some("Up".into()),
        KeyCode::Down => Some("Down".into()),
        KeyCode::Left => Some("Left".into()),
        KeyCode::Right => Some("Right".into()),
        KeyCode::Home => Some("Home".into()),
        KeyCode::End => Some("End".into()),
        KeyCode::PageUp => Some("PageUp".into()),
        KeyCode::PageDown => Some("PageDown".into()),
        KeyCode::Delete => Some("DC".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn default_config() -> Config {
        Config::default()
    }

    // --- Normal mode ---

    #[test]
    fn test_normal_q_quits() {
        let action = handle(
            key(KeyCode::Char('q')),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn test_normal_ctrl_q_quits() {
        let action = handle(
            ctrl('q'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn test_normal_ctrl_a_adds() {
        let action = handle(
            ctrl('a'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::AddPane)));
    }

    #[test]
    fn test_normal_ctrl_d_drops() {
        let action = handle(
            ctrl('d'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::DropPane)));
    }

    #[test]
    fn test_normal_ctrl_s_session_picker() {
        let action = handle(
            ctrl('s'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::OpenSessionPicker)));
    }

    #[test]
    fn test_normal_ctrl_g_azlin() {
        let action = handle(
            ctrl('g'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::DiscoverAzlin)));
    }

    #[test]
    fn test_normal_ctrl_e_settings() {
        let action = handle(
            ctrl('e'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::OpenSettings)));
    }

    #[test]
    fn test_normal_ctrl_v_split_vertical() {
        let action = handle(
            ctrl('v'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::SplitVertical)));
    }

    #[test]
    fn test_normal_ctrl_h_split_horizontal() {
        let action = handle(
            ctrl('h'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::SplitHorizontal)));
    }

    #[test]
    fn test_normal_ctrl_f_maximize() {
        let action = handle(
            ctrl('f'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::ToggleMaximize)));
    }

    #[test]
    fn test_normal_ctrl_x_swap() {
        let action = handle(
            ctrl('x'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::SwapPane)));
    }

    #[test]
    fn test_normal_ctrl_n_app_launcher() {
        let action = handle(
            ctrl('n'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::OpenAppLauncher)));
    }

    #[test]
    fn test_normal_tab_focuses_next() {
        let action = handle(
            key(KeyCode::Tab),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::FocusNext)));
    }

    #[test]
    fn test_normal_backtab_focuses_prev() {
        let action = handle(
            key(KeyCode::BackTab),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::FocusPrev)));
    }

    #[test]
    fn test_normal_enter_enters_pane() {
        let action = handle(
            key(KeyCode::Enter),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EnterPaneMode)));
    }

    #[test]
    fn test_normal_arrows() {
        let a = handle(
            key(KeyCode::Down),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(a, Some(Action::FocusNext)));
        let a = handle(
            key(KeyCode::Right),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(a, Some(Action::FocusNext)));
        let a = handle(
            key(KeyCode::Up),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(a, Some(Action::FocusPrev)));
        let a = handle(
            key(KeyCode::Left),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(a, Some(Action::FocusPrev)));
    }

    #[test]
    fn test_normal_f11_maximize() {
        let action = handle(
            key(KeyCode::F(11)),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::ToggleMaximize)));
    }

    #[test]
    fn test_normal_binding() {
        let mut config = default_config();
        config.bindings.insert('1', "top".to_string());
        let action = handle(
            key(KeyCode::Char('1')),
            &Mode::Normal,
            &config,
            &EditorInputMode::Browse,
        );
        match action {
            Some(Action::RunBinding(cmd)) => assert_eq!(cmd, "top"),
            other => panic!("expected RunBinding, got {:?}", other),
        }
    }

    #[test]
    fn test_normal_unknown_char_no_binding() {
        let action = handle(
            key(KeyCode::Char('z')),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(action.is_none());
    }

    #[test]
    fn test_normal_unknown_ctrl_none() {
        let action = handle(
            ctrl('z'),
            &Mode::Normal,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(action.is_none());
    }

    // --- PaneFocused mode ---

    #[test]
    fn test_focused_esc_unfocuses() {
        let action = handle(
            key(KeyCode::Esc),
            &Mode::PaneFocused,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::ExitPaneMode)));
    }

    #[test]
    fn test_focused_forwards_keys() {
        let action = handle(
            key(KeyCode::Char('a')),
            &Mode::PaneFocused,
            &default_config(),
            &EditorInputMode::Browse,
        );
        match action {
            Some(Action::SendKeys(s)) => assert_eq!(s, "a"),
            other => panic!("expected SendKeys, got {:?}", other),
        }
    }

    #[test]
    fn test_focused_forwards_enter() {
        let action = handle(
            key(KeyCode::Enter),
            &Mode::PaneFocused,
            &default_config(),
            &EditorInputMode::Browse,
        );
        match action {
            Some(Action::SendKeys(s)) => assert_eq!(s, "Enter"),
            other => panic!("expected SendKeys(Enter), got {:?}", other),
        }
    }

    #[test]
    fn test_focused_forwards_ctrl() {
        let action = handle(
            ctrl('c'),
            &Mode::PaneFocused,
            &default_config(),
            &EditorInputMode::Browse,
        );
        match action {
            Some(Action::SendKeys(s)) => assert_eq!(s, "C-c"),
            other => panic!("expected SendKeys(C-c), got {:?}", other),
        }
    }

    // --- SessionPicker mode ---

    #[test]
    fn test_picker_navigation() {
        let action = handle(
            key(KeyCode::Up),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerUp)));
        let action = handle(
            key(KeyCode::Down),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerDown)));
        let action = handle(
            key(KeyCode::Char('k')),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerUp)));
        let action = handle(
            key(KeyCode::Char('j')),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerDown)));
    }

    #[test]
    fn test_picker_confirm() {
        let action = handle(
            key(KeyCode::Enter),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerConfirm)));
    }

    #[test]
    fn test_picker_cancel() {
        let action = handle(
            key(KeyCode::Esc),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerCancel)));
    }

    #[test]
    fn test_picker_add_all() {
        let action = handle(
            key(KeyCode::Char('a')),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerAddAll)));
    }

    #[test]
    fn test_picker_scan_azlin() {
        let action = handle(
            key(KeyCode::Char('z')),
            &Mode::SessionPicker,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::PickerScanAzlin)));
    }

    // --- AppLauncher mode ---

    #[test]
    fn test_launcher_navigation() {
        let action = handle(
            key(KeyCode::Up),
            &Mode::AppLauncher,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::LauncherUp)));
        let action = handle(
            key(KeyCode::Down),
            &Mode::AppLauncher,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::LauncherDown)));
        let action = handle(
            key(KeyCode::Enter),
            &Mode::AppLauncher,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::LauncherConfirm)));
        let action = handle(
            key(KeyCode::Esc),
            &Mode::AppLauncher,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::LauncherCancel)));
    }

    // --- CommandEditor mode (Browse) ---

    #[test]
    fn test_editor_browse_keys() {
        let action = handle(
            key(KeyCode::Esc),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorClose)));
        let action = handle(
            key(KeyCode::Up),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorUp)));
        let action = handle(
            key(KeyCode::Down),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorDown)));
        let action = handle(
            key(KeyCode::Char('k')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorUp)));
        let action = handle(
            key(KeyCode::Char('j')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorDown)));
        let action = handle(
            key(KeyCode::Char('d')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorDelete)));
        let action = handle(
            key(KeyCode::Char('a')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorAdd)));
        let action = handle(
            key(KeyCode::Char('e')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorEdit)));
        let action = handle(
            key(KeyCode::Enter),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::Browse,
        );
        assert!(matches!(action, Some(Action::EditorEdit)));
    }

    // --- CommandEditor mode (InputKey) ---

    #[test]
    fn test_editor_input_key_mode() {
        let action = handle(
            key(KeyCode::Esc),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputKey,
        );
        assert!(matches!(action, Some(Action::EditorCancelInput)));
        let action = handle(
            key(KeyCode::Char('5')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputKey,
        );
        assert!(matches!(action, Some(Action::EditorSetKey('5'))));
        let action = handle(
            key(KeyCode::Char('a')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputKey,
        );
        assert!(action.is_none()); // non-digit ignored
    }

    // --- CommandEditor mode (InputCommand) ---

    #[test]
    fn test_editor_input_command_mode() {
        let action = handle(
            key(KeyCode::Esc),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputCommand,
        );
        assert!(matches!(action, Some(Action::EditorCancelInput)));
        let action = handle(
            key(KeyCode::Enter),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputCommand,
        );
        assert!(matches!(action, Some(Action::EditorConfirm)));
        let action = handle(
            key(KeyCode::Backspace),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputCommand,
        );
        assert!(matches!(action, Some(Action::EditorBackspace)));
        let action = handle(
            key(KeyCode::Char('x')),
            &Mode::CommandEditor,
            &default_config(),
            &EditorInputMode::InputCommand,
        );
        assert!(matches!(action, Some(Action::EditorTypeChar('x'))));
    }

    // --- key_to_tmux_string ---

    #[test]
    fn test_key_to_tmux_string() {
        assert_eq!(
            key_to_tmux_string(key(KeyCode::Char('a'))),
            Some("a".into())
        );
        assert_eq!(
            key_to_tmux_string(key(KeyCode::Enter)),
            Some("Enter".into())
        );
        assert_eq!(
            key_to_tmux_string(key(KeyCode::Backspace)),
            Some("BSpace".into())
        );
        assert_eq!(key_to_tmux_string(key(KeyCode::Tab)), Some("Tab".into()));
        assert_eq!(key_to_tmux_string(key(KeyCode::Up)), Some("Up".into()));
        assert_eq!(key_to_tmux_string(key(KeyCode::Down)), Some("Down".into()));
        assert_eq!(key_to_tmux_string(key(KeyCode::Left)), Some("Left".into()));
        assert_eq!(
            key_to_tmux_string(key(KeyCode::Right)),
            Some("Right".into())
        );
        assert_eq!(key_to_tmux_string(key(KeyCode::Home)), Some("Home".into()));
        assert_eq!(key_to_tmux_string(key(KeyCode::End)), Some("End".into()));
        assert_eq!(
            key_to_tmux_string(key(KeyCode::PageUp)),
            Some("PageUp".into())
        );
        assert_eq!(
            key_to_tmux_string(key(KeyCode::PageDown)),
            Some("PageDown".into())
        );
        assert_eq!(key_to_tmux_string(key(KeyCode::Delete)), Some("DC".into()));
        assert_eq!(key_to_tmux_string(ctrl('c')), Some("C-c".into()));
        assert_eq!(key_to_tmux_string(key(KeyCode::Esc)), None);
    }
}
