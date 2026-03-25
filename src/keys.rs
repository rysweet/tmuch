use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    PaneFocused,
    SessionPicker,
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
}

pub fn handle(event: KeyEvent, mode: &Mode, config: &Config) -> Option<Action> {
    match mode {
        Mode::Normal => handle_normal(event, config),
        Mode::PaneFocused => handle_pane_focused(event),
        Mode::SessionPicker => handle_picker(event),
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
            _ => None,
        };
    }

    match event.code {
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrev),
        KeyCode::Enter => Some(Action::EnterPaneMode),
        KeyCode::Char('q') => Some(Action::Quit),
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
