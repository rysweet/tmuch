mod app;
mod azlin_integration;
mod config;
mod consts;
mod ipc;
mod keys;
mod layout;
mod layouts;
mod pane;
mod self_update;
mod session_picker;
mod source;
mod theme;
mod tmux;
mod ui;
mod update_check;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

#[derive(Parser)]
#[command(name = "tmuch", about = "TUI tmux multiplexer", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// tmux sessions to attach on startup
    #[arg()]
    sessions: Vec<String>,

    /// Create a new pane (prefix: watch:cmd:ms, tail:path, or plain tmux command)
    #[arg(short = 'n', long = "new", value_name = "COMMAND")]
    new_commands: Vec<String>,

    /// Override a command binding (e.g., --bind 1="top")
    #[arg(short = 'b', long = "bind", value_name = "KEY=CMD")]
    binds: Vec<String>,

    /// Load a named layout
    #[arg(short = 'l', long = "layout", value_name = "NAME")]
    layout: Option<String>,

    /// Save current layout on exit
    #[arg(long = "save-layout", value_name = "NAME")]
    save_layout: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Update tmuch to the latest version from GitHub Releases
    #[command(alias = "self-update")]
    Update,

    /// List available saved layouts
    Layouts,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Discover Azure VMs and their tmux sessions
    Azlin {
        /// Azure resource group to filter VMs
        #[arg(short, long)]
        resource_group: Option<String>,
    },

    /// Send a command to a running tmuch instance
    Ctl {
        /// JSON command to send (e.g. '{"command":"list_panes"}')
        json: String,
    },

    /// List available pane types and widget apps
    Apps,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    match &cli.command {
        Some(Commands::Update) => return self_update::handle_self_update(),
        Some(Commands::Layouts) => {
            let names = layouts::list();
            if names.is_empty() {
                eprintln!("No saved layouts. Use --save-layout NAME to save one.");
            } else {
                for name in names {
                    println!("{}", name);
                }
            }
            return Ok(());
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(*shell, &mut cmd, "tmuch", &mut std::io::stdout());
            return Ok(());
        }
        Some(Commands::Azlin { resource_group }) => {
            return app::run_azlin(resource_group.clone());
        }
        Some(Commands::Ctl { json }) => {
            match ipc::send_command(json) {
                Ok(response) => {
                    println!("{}", response);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        Some(Commands::Apps) => {
            let registry = source::registry::PluginRegistry::new();
            println!("Available pane types:\n");
            for info in registry.list() {
                println!("  {:<12} {}", info.name, info.description);
                println!("  {:<12} Usage: {}", "", info.usage);
                println!();
            }
            println!("\nExample: tmuch -n 'weather:Seattle' -n 'sysinfo:' -n 'snake:'");
            return Ok(());
        }
        None => {}
    }

    // Non-blocking update check (background, cached)
    update_check::check_for_updates();

    let mut config = config::load()?;

    // Config validation warnings
    config::validate_warnings(&config);

    // Apply CLI bind overrides
    for bind in &cli.binds {
        if let Some((key, cmd)) = bind.split_once('=') {
            if let Some(ch) = key.chars().next() {
                config.bindings.insert(ch, cmd.to_string());
            }
        }
    }

    app::run(
        config,
        cli.sessions,
        cli.new_commands,
        cli.layout,
        cli.save_layout,
    )
}
