use crate::config::Config;
use crate::keys::{self, Action, Mode};
use crate::pane::PaneManager;
use crate::session_picker::SessionPicker;
use crate::tmux;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

pub struct App {
    pub pane_manager: PaneManager,
    pub config: Config,
    pub mode: Mode,
    pub picker: SessionPicker,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            pane_manager: PaneManager::new(),
            config,
            mode: Mode::Normal,
            picker: SessionPicker::new(),
            should_quit: false,
        }
    }

    pub fn attach_session(&mut self, name: &str, owned: bool) {
        self.pane_manager.add(name.to_string(), owned);
    }

    pub fn create_and_attach(&mut self, command: Option<&str>) -> Result<()> {
        let name = tmux::generate_session_name();
        tmux::create_session(&name, command)?;
        self.pane_manager.add(name, true);
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => self.should_quit = true,
            Action::AddPane => {
                self.create_and_attach(None)?;
            }
            Action::DropPane => {
                if let Some(pane) = self.pane_manager.remove_focused() {
                    if pane.owned {
                        let _ = tmux::kill_session(&pane.session_name);
                    }
                }
            }
            Action::FocusNext => self.pane_manager.focus_next(),
            Action::FocusPrev => self.pane_manager.focus_prev(),
            Action::EnterPaneMode => {
                if self.pane_manager.focused().is_some() {
                    self.mode = Mode::PaneFocused;
                }
            }
            Action::ExitPaneMode => {
                self.mode = Mode::Normal;
            }
            Action::OpenSessionPicker => {
                self.picker.refresh()?;
                self.mode = Mode::SessionPicker;
            }
            Action::PickerUp => self.picker.select_prev(),
            Action::PickerDown => self.picker.select_next(),
            Action::PickerConfirm => {
                if let Some(session) = self.picker.confirm() {
                    let name = session.name.clone();
                    self.pane_manager.add(name, false);
                }
                self.mode = Mode::Normal;
            }
            Action::PickerCancel => {
                self.mode = Mode::Normal;
            }
            Action::RunBinding(cmd) => {
                let name = tmux::generate_session_name();
                tmux::create_session(&name, Some(&cmd))?;
                self.pane_manager.add(name, true);
            }
            Action::SendKeys(keys) => {
                if let Some(pane) = self.pane_manager.focused() {
                    let _ = tmux::send_keys(&pane.session_name, &keys);
                }
            }
        }
        Ok(())
    }

}

pub fn run(config: Config, initial_sessions: Vec<String>, new_commands: Vec<String>) -> Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    // Attach initial sessions
    for name in &initial_sessions {
        if tmux::session_exists(name) {
            app.attach_session(name, false);
        } else {
            eprintln!("warning: tmux session '{}' not found, skipping", name);
        }
    }

    // Create sessions for new commands
    for cmd in &new_commands {
        app.create_and_attach(Some(cmd))?;
    }

    // If no panes, open session picker or create a default
    if app.pane_manager.is_empty() {
        let sessions = tmux::list_sessions()?;
        if sessions.is_empty() {
            // Create a default session
            app.create_and_attach(None)?;
        } else {
            // Open picker
            app.picker.refresh()?;
            app.mode = Mode::SessionPicker;
        }
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        // Capture pane content with actual terminal dimensions
        let term_size = terminal.size()?;
        let pane_count = app.pane_manager.count();
        if pane_count > 0 {
            let rects = crate::layout::compute(pane_count, ratatui::layout::Rect::new(
                0, 0, term_size.width, term_size.height.saturating_sub(1),
            ));
            for (i, pane) in app.pane_manager.panes_mut().iter_mut().enumerate() {
                if let Some(rect) = rects.get(i) {
                    // Inner area (minus borders)
                    let w = rect.width.saturating_sub(2);
                    let h = rect.height.saturating_sub(2);
                    if w > 0 && h > 0 {
                        if let Ok(content) = tmux::capture_pane(&pane.session_name, w, h) {
                            pane.content = content;
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

        // Handle events
        if event::poll(poll_duration)? {
            if let Event::Key(key) = event::read()? {
                if let Some(action) = keys::handle(key, &app.mode, &app.config) {
                    app.handle_action(action)?;
                }
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
