use crate::app::App;
use crate::config::Config;
use crate::ipc::IpcServer;
use crate::keys::{self, Mode};
use crate::layouts;
use crate::source::clock::ClockSource;
use crate::source::command::CommandSource;
use crate::source::http::HttpSource;
use crate::source::snake::SnakeSource;
use crate::source::sparkline_monitor::SparklineSource;
use crate::source::sysinfo::SysInfoSource;
use crate::source::tail::TailSource;
use crate::source::weather::WeatherSource;
use crate::source::{self, NewPaneRequest, PaneSpec};
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
    let mouse_enabled = config.display.mouse;
    if mouse_enabled {
        io::stdout().execute(EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    // Create SshSubprocessSource panes (uses system ssh)
    for session_info in &discovered {
        let vm_name = session_info.host.as_deref().unwrap_or("unknown");
        let vm = vms.iter().find(|v| v.name == vm_name);
        if let Some(vm) = vm {
            if let Ok(remote) = crate::azlin_integration::vm_to_remote_config(vm) {
                let source = crate::source::ssh_subprocess::from_remote_config(
                    &remote,
                    session_info.name.clone(),
                );
                app.pane_manager.add(Box::new(source));
            }
        }
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        capture_pane_content(&mut app, &terminal)?;
        app.spinner_tick = app.spinner_tick.wrapping_add(1);

        // Check for background task completion
        check_bg_result(&mut app);

        terminal.draw(|frame| ui::draw(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(poll_duration)? {
            let term_size = terminal.size()?;
            let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));
            handle_event(&mut app, event::read()?, main_area)?;
            // Drain all remaining queued events this iteration
            while event::poll(Duration::ZERO)? {
                handle_event(&mut app, event::read()?, main_area)?;
                if app.should_quit {
                    break;
                }
            }
        }
    }

    terminal::disable_raw_mode()?;
    if mouse_enabled {
        io::stdout().execute(DisableMouseCapture)?;
    }
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
    let mouse_enabled = config.display.mouse;
    if mouse_enabled {
        io::stdout().execute(EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    // Start IPC server
    let (ipc_tx, ipc_rx) = mpsc::channel();
    let _ipc_server = match IpcServer::start(ipc_tx) {
        Ok(server) => Some(server),
        Err(e) => {
            eprintln!("IPC: {}", e);
            None
        }
    };

    // Load layout if specified
    if let Some(layout_name) = &layout {
        let spec = layouts::load(layout_name)?;
        for pane_spec in &spec.panes {
            app.add_from_spec(pane_spec)?;
        }
    } else {
        setup_initial_panes(&mut app, &initial_sessions, &new_commands)?;
    }

    // Auto-discover azlin VMs in background on startup
    if app.config.azlin.enabled && app.config.azlin.auto_discover {
        crate::dlog!("Starting background azlin discovery on startup...");
        app.busy = Some("Discovering Azure VMs...".into());

        let (tx, rx) = std::sync::mpsc::channel();
        app.bg_result = Some(rx);

        let rg = app.config.azlin.resource_group.clone();
        let remotes = app.config.remote.clone();
        let azlin_cfg = app.config.azlin.clone();

        std::thread::spawn(move || {
            let mut sessions = crate::tmux::list_sessions().unwrap_or_default();
            for remote in &remotes {
                if let Ok(names) = crate::source::ssh_subprocess::list_remote_sessions(remote) {
                    for name in names {
                        sessions.push(crate::tmux::SessionInfo {
                            name,
                            attached: false,
                            host: Some(remote.name.clone()),
                        });
                    }
                }
            }
            if azlin_cfg.enabled || rg.is_some() {
                if let Ok(remote_sessions) =
                    crate::azlin_integration::discover_remote_sessions_sync(rg.as_deref())
                {
                    sessions.extend(remote_sessions);
                }
            }
            let _ = tx.send(crate::app::BgTaskResult::AzlinSessionsSilent(sessions));
        });
    }

    let poll_duration = Duration::from_millis(app.config.display.poll_interval_ms);

    loop {
        capture_pane_content(&mut app, &terminal)?;
        app.spinner_tick = app.spinner_tick.wrapping_add(1);
        check_bg_result(&mut app);
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
            handle_event(&mut app, event::read()?, main_area)?;
            // Drain all remaining queued events this iteration
            while event::poll(Duration::ZERO)? {
                handle_event(&mut app, event::read()?, main_area)?;
                if app.should_quit {
                    break;
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
            .map(|(_, p)| p.source.to_spec())
            .collect();
        layouts::save(&layouts::LayoutSpec {
            name: layout_name,
            panes: specs,
        })?;
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    if mouse_enabled {
        io::stdout().execute(DisableMouseCapture)?;
    }
    io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

/// Check if a background task has completed.
fn check_bg_result(app: &mut App) {
    let completed = if let Some(ref rx) = app.bg_result {
        rx.try_recv().ok()
    } else {
        None
    };

    if let Some(result) = completed {
        match result {
            crate::app::BgTaskResult::AzlinSessionsShowPicker(sessions) => {
                crate::dlog!(
                    "Discovery complete: {} sessions — opening picker",
                    sessions.len()
                );
                app.picker.sessions = sessions;
                app.busy = None;
                app.bg_result = None;
                app.mode = crate::keys::Mode::SessionPicker;
            }
            crate::app::BgTaskResult::AzlinSessionsSilent(sessions) => {
                crate::dlog!(
                    "Background discovery complete: {} sessions ready (Ctrl-S to view)",
                    sessions.len()
                );
                app.picker.sessions = sessions;
                app.busy = None;
                app.bg_result = None;
                // Don't change mode — user can open picker when ready
            }
        }
    }
}

/// Capture content from all visible panes.
fn capture_pane_content(
    app: &mut App,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    let term_size = terminal.size()?;
    let pane_count = app.pane_manager.count();
    if pane_count == 0 {
        return Ok(());
    }

    let main_area = Rect::new(0, 1, term_size.width, term_size.height.saturating_sub(2));

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

    Ok(())
}

/// Handle a single crossterm event.
fn handle_event(app: &mut App, ev: Event, main_area: Rect) -> Result<()> {
    match ev {
        Event::Key(key) => {
            crate::dlog!("key: {:?}", key);

            // If busy, Esc cancels the background operation
            if app.busy.is_some() {
                if key.code == crossterm::event::KeyCode::Esc {
                    crate::dlog!("Cancelling background operation");
                    app.busy = None;
                    app.bg_result = None;
                    return Ok(());
                }
                // Ignore other keys while busy
                return Ok(());
            }

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
        Event::Resize(_, _) => {
            // Terminal resized — next draw loop iteration will pick up new size
        }
        _ => {}
    }
    Ok(())
}

/// Set up initial panes from CLI arguments.
fn setup_initial_panes(
    app: &mut App,
    initial_sessions: &[String],
    new_commands: &[String],
) -> Result<()> {
    // Attach initial sessions (supports user@host:session syntax for remote)
    for name in initial_sessions {
        if name.contains(':') {
            if let Err(e) = app.attach_remote(name) {
                eprintln!("warning: remote session '{}': {}", name, e);
            }
        } else if tmux::session_exists(name) {
            app.add_local_tmux(name, false);
        }
    }

    // Create panes for new commands (supports watch:/tail: prefixes)
    for cmd in new_commands {
        add_pane_from_request(app, source::parse_new_arg(cmd))?;
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

    Ok(())
}

/// Add a pane from a NewPaneRequest.
pub fn add_pane_from_request(app: &mut App, request: NewPaneRequest) -> Result<()> {
    match request {
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
        NewPaneRequest::DebugLog => {
            let source = crate::source::debug_log::DebugLogSource::new();
            app.pane_manager.add(Box::new(source));
        }
    }
    Ok(())
}
