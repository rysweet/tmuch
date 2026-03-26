use crate::config::Config;
use crate::layout::PaneId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    PaneFocused,
    SessionPicker,
    CommandEditor,
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
    OpenCommandEditor,
    EditorUp,
    EditorDown,
    EditorDelete,
    EditorClose,
    #[allow(dead_code)]
    FocusPane(PaneId),
    SplitVertical,
    SplitHorizontal,
    ToggleMaximize,
    SwapPane,
}

pub fn handle(event: KeyEvent, mode: &Mode, config: &Config) -> Option<Action> {
    match mode {
        Mode::Normal => handle_normal(event, config),
        Mode::PaneFocused => handle_pane_focused(event),
        Mode::SessionPicker => handle_picker(event),
        Mode::CommandEditor => handle_command_editor(event),
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
            KeyCode::Char('z') => Some(Action::DiscoverAzlin),
            KeyCode::Char('e') => Some(Action::OpenCommandEditor),
            KeyCode::Char('v') => Some(Action::SplitVertical),
            KeyCode::Char('h') => Some(Action::SplitHorizontal),
            KeyCode::Char('f') => Some(Action::ToggleMaximize),
            KeyCode::Char('x') => Some(Action::SwapPane),
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

fn handle_command_editor(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc => Some(Action::EditorClose),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::EditorUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::EditorDown),
        KeyCode::Char('d') => Some(Action::EditorDelete),
        KeyCode::Char('a') => None, // no-op: hint says edit config file
        _ => None,
    }
}

fn key_to_tmux_string(event: KeyEvent) -> Option<String> {
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
