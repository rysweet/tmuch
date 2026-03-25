mod app;
mod config;
mod keys;
mod layout;
mod pane;
mod session_picker;
mod tmux;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "tmuch", about = "TUI tmux multiplexer", version)]
struct Cli {
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
