use crate::config::Config;
use crate::keys::{self, Action, Mode};
use crate::layouts;
use crate::pane::PaneManager;
use crate::session_picker::SessionPicker;
use crate::source::command::CommandSource;
use crate::source::http::HttpSource;
use crate::source::local_tmux::LocalTmuxSource;
use crate::source::ssh_tmux::{RemoteConfig, SshTmuxSource};
use crate::source::tail::TailSource;
use crate::source::{self, NewPaneRequest, PaneSpec};
use crate::tmux;
use crate::ui;
use anyhow::Result;
use azlin_ssh::SshPool;
use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::sync::Arc;
use std::time::Duration;

pub struct App {
    pub pane_manager: PaneManager,
    pub config: Config,
    pub mode: Mode,
    pub picker: SessionPicker,
    pub should_quit: bool,
    pub tokio_handle: tokio::runtime::Handle,
    pub ssh_pool: Arc<SshPool>,
}

impl App {
    pub fn new(config: Config, tokio_handle: tokio::runtime::Handle) -> Self {
        Self {
            pane_manager: PaneManager::new(),
            config,
            mode: Mode::Normal,
            picker: SessionPicker::new(),
            should_quit: false,
            tokio_handle,
            ssh_pool: Arc::new(SshPool::default()),
        }
    }

    pub fn add_local_tmux(&mut self, name: &str, owned: bool) {
        let source = if owned {
            // Already created externally
            LocalTmuxSource::attach(name.to_string())
        } else {
            LocalTmuxSource::attach(name.to_string())
        };
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
                let source = SshTmuxSource::new(
                    remote,
                    session.clone(),
                    Arc::clone(&self.ssh_pool),
                    &self.tokio_handle,
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

        let source = SshTmuxSource::new(
            remote,
            session.to_string(),
            Arc::clone(&self.ssh_pool),
            &self.tokio_handle,
        );
        self.pane_manager.add(Box::new(source));
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => self.should_quit = true,
            Action::AddPane => {
                self.create_local_tmux(None)?;
            }
            Action::DropPane => {
                // Pane::drop() calls source.cleanup()
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
                    self.picker.refresh_with_remotes(
                        &self.config.remote,
                        &self.config.azlin,
                        &self.ssh_pool,
                        &self.tokio_handle,
                    )?;
                }
                self.mode = Mode::SessionPicker;
            }
            Action::PickerUp => self.picker.select_prev(),
            Action::PickerDown => self.picker.select_next(),
            Action::PickerConfirm => {
                if let Some(session) = self.picker.confirm() {
                    let name = session.name.clone();
                    if let Some(host) = &session.host {
                        // Remote session — find matching remote config
                        if let Some(remote) =
                            self.config.remote.iter().find(|r| r.name == *host).cloned()
                        {
                            let source = SshTmuxSource::new(
                                remote,
                                name,
                                Arc::clone(&self.ssh_pool),
                                &self.tokio_handle,
                            );
                            self.pane_manager.add(Box::new(source));
                        }
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
                    self.picker.refresh_with_remotes(
                        &self.config.remote,
                        &self.config.azlin,
                        &self.ssh_pool,
                        &self.tokio_handle,
                    )?;
                } else {
                    // Even without azlin config, try discovery
                    let azlin_cfg = crate::azlin_integration::AzlinConfig {
                        enabled: true,
                        resource_group: None,
                    };
                    self.picker.refresh_with_remotes(
                        &self.config.remote,
                        &azlin_cfg,
                        &self.ssh_pool,
                        &self.tokio_handle,
                    )?;
                }
                self.mode = Mode::SessionPicker;
            }
            Action::PickerScanAzlin => {
                // Scan azlin VMs and add their sessions to the picker
                let rg = self.config.azlin.resource_group.as_deref();
                let result =
                    self.tokio_handle
                        .block_on(crate::azlin_integration::discover_remote_sessions(
                            &self.ssh_pool,
                            rg,
                        ));
                if let Ok(remote_sessions) = result {
                    // Add to existing picker sessions (avoid duplicates)
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
                        if let Some(remote) =
                            self.config.remote.iter().find(|r| r.name == *host).cloned()
                        {
                            let source = SshTmuxSource::new(
                                remote,
                                name,
                                Arc::clone(&self.ssh_pool),
                                &self.tokio_handle,
                            );
                            self.pane_manager.add(Box::new(source));
                        }
                    } else {
                        self.pane_manager
                            .add(Box::new(LocalTmuxSource::attach(name)));
                    }
                }
                self.mode = Mode::Normal;
            }
        }
        Ok(())
    }
}

/// Run azlin discovery: list all VMs and their tmux sessions, then launch TUI.
pub fn run_azlin(resource_group: Option<String>) -> Result<()> {
    let config = crate::config::load()?;
    let rt = tokio::runtime::Runtime::new()?;
    let handle = rt.handle().clone();
    let ssh_pool = Arc::new(SshPool::default());

    eprintln!("Discovering Azure VMs...");
    let vms = crate::azlin_integration::discover_vms(resource_group.as_deref())?;

    if vms.is_empty() {
        eprintln!("No running VMs found.");
        return Ok(());
    }

    eprintln!(
        "Found {} running VM(s). Listing tmux sessions...",
        vms.len()
    );

    let mut all_sessions = Vec::new();
    for vm in &vms {
        let remote = match crate::azlin_integration::vm_to_remote_config(vm) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Skipping {}: {}", vm.name, e);
                continue;
            }
        };
        let result = handle.block_on(crate::source::ssh_tmux::list_remote_sessions(
            &ssh_pool, &remote,
        ));
        match result {
            Ok(sessions) => {
                for sess in sessions {
                    eprintln!("  {}:{}", vm.name, sess);
                    all_sessions.push((remote.clone(), sess));
                }
            }
            Err(e) => {
                eprintln!("  {}: SSH error: {}", vm.name, e);
            }
        }
    }

    if all_sessions.is_empty() {
        eprintln!("No tmux sessions found on any VM.");
        return Ok(());
    }

    // Launch TUI with all discovered sessions
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config, handle);
    app.ssh_pool = ssh_pool;

    for (remote, session) in &all_sessions {
        let source = SshTmuxSource::new(
            remote.clone(),
            session.clone(),
            Arc::clone(&app.ssh_pool),
            &app.tokio_handle,
        );
        app.pane_manager.add(Box::new(source));
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        let term_size = terminal.size()?;
        let pane_count = app.pane_manager.count();
        if pane_count > 0 {
            let rects = crate::layout::compute(
                pane_count,
                ratatui::layout::Rect::new(
                    0,
                    0,
                    term_size.width,
                    term_size.height.saturating_sub(1),
                ),
            );
            for (i, pane) in app.pane_manager.panes_mut().iter_mut().enumerate() {
                if let Some(rect) = rects.get(i) {
                    let w = rect.width.saturating_sub(2);
                    let h = rect.height.saturating_sub(2);
                    if w > 0 && h > 0 {
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
            if let Event::Key(key) = event::read()? {
                if let Some(action) = keys::handle(key, &app.mode, &app.config) {
                    app.handle_action(action)?;
                }
            }
        }
    }

    terminal::disable_raw_mode()?;
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
    // Create tokio runtime for async SSH operations
    let rt = tokio::runtime::Runtime::new()?;
    let handle = rt.handle().clone();

    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config, handle);

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
            let rects = crate::layout::compute(
                pane_count,
                ratatui::layout::Rect::new(
                    0,
                    0,
                    term_size.width,
                    term_size.height.saturating_sub(1),
                ),
            );
            for (i, pane) in app.pane_manager.panes_mut().iter_mut().enumerate() {
                if let Some(rect) = rects.get(i) {
                    let w = rect.width.saturating_sub(2);
                    let h = rect.height.saturating_sub(2);
                    if w > 0 && h > 0 {
                        if let Ok(content) = pane.source.capture(w, h) {
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

    // Save layout if requested
    if let Some(layout_name) = save_layout {
        let specs: Vec<PaneSpec> = app
            .pane_manager
            .panes()
            .iter()
            .map(|p| p.source.to_spec())
            .collect();
        layouts::save(&layouts::LayoutSpec {
            name: layout_name,
            panes: specs,
        })?;
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
