mod app;
mod config;
mod keys;
mod layout;
mod pane;
mod self_update;
mod session_picker;
mod tmux;
mod ui;
mod update_check;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tmuch", about = "TUI tmux multiplexer", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// tmux sessions to attach on startup
    #[arg()]
    sessions: Vec<String>,

    /// Create a new tmux session running this command
    #[arg(short = 'n', long = "new", value_name = "COMMAND")]
    new_commands: Vec<String>,

    /// Override a command binding (e.g., --bind 1="top")
    #[arg(short = 'b', long = "bind", value_name = "KEY=CMD")]
    binds: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Update tmuch to the latest version from GitHub Releases
    #[command(alias = "self-update")]
    Update,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(Commands::Update) = cli.command {
        return self_update::handle_self_update();
    }

    // Non-blocking update check (background, cached)
    update_check::check_for_updates();

    let mut config = config::load()?;

    // Apply CLI bind overrides
    for bind in &cli.binds {
        if let Some((key, cmd)) = bind.split_once('=') {
            if let Some(ch) = key.chars().next() {
                config.bindings.insert(ch, cmd.to_string());
            }
        }
    }

    app::run(config, cli.sessions, cli.new_commands)
}
