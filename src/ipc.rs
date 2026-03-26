use crate::source::PaneSpec;
use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;

/// Commands that can be sent via IPC.
#[derive(Debug)]
pub enum IpcCommand {
    ListPanes,
    AddPane(PaneSpec),
    RemovePane(u32),
    FocusPane(u32),
    Split { direction: String, spec: PaneSpec },
    Maximize(u32),
    SendKeys { id: u32, keys: String },
    Quit,
}

/// Response channel: the IPC thread sends commands and receives responses.
pub struct IpcMessage {
    pub command: IpcCommand,
    pub response_tx: mpsc::Sender<String>,
}

pub struct IpcServer {
    socket_path: PathBuf,
}

impl IpcServer {
    /// Start the IPC server on a background thread.
    /// Returns the server handle and the receiver for the main event loop.
    pub fn start(tx: mpsc::Sender<IpcMessage>) -> Result<Self> {
        let pid = std::process::id();
        let socket_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let socket_path = socket_dir.join(format!("tmuch-{}.sock", pid));

        // Remove stale socket
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)?;

        // Restrict socket permissions to owner only
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
        }

        listener.set_nonblocking(false)?;

        let path_clone = socket_path.clone();
        let tx_clone = tx;

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let tx = tx_clone.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(&stream);
                    let mut writer = &stream;
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        match parse_ipc_command(&line) {
                            Ok(cmd) => {
                                let (resp_tx, resp_rx) = mpsc::channel();
                                let msg = IpcMessage {
                                    command: cmd,
                                    response_tx: resp_tx,
                                };
                                if tx.send(msg).is_err() {
                                    break;
                                }
                                // Wait for response from the main thread
                                let response = resp_rx
                                    .recv_timeout(std::time::Duration::from_secs(5))
                                    .unwrap_or_else(|_| {
                                        r#"{"ok":false,"error":"timeout"}"#.to_string()
                                    });
                                let _ = writeln!(writer, "{}", response);
                            }
                            Err(e) => {
                                let resp = serde_json::json!({
                                    "ok": false,
                                    "error": format!("parse error: {}", e)
                                });
                                let _ = writeln!(writer, "{}", resp);
                            }
                        }
                    }
                });
            }
            let _ = std::fs::remove_file(&path_clone);
        });

        Ok(Self { socket_path })
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn parse_ipc_command(json: &str) -> Result<IpcCommand> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let cmd_type = v
        .get("command")
        .and_then(|c| c.as_str())
        .unwrap_or_default();

    match cmd_type {
        "list_panes" => Ok(IpcCommand::ListPanes),
        "add_pane" => {
            let spec: PaneSpec = serde_json::from_value(
                v.get("spec")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("missing 'spec'"))?,
            )?;
            Ok(IpcCommand::AddPane(spec))
        }
        "remove_pane" => {
            let id = v
                .get("id")
                .and_then(|i| i.as_u64())
                .ok_or_else(|| anyhow::anyhow!("missing 'id'"))? as u32;
            Ok(IpcCommand::RemovePane(id))
        }
        "focus_pane" => {
            let id = v
                .get("id")
                .and_then(|i| i.as_u64())
                .ok_or_else(|| anyhow::anyhow!("missing 'id'"))? as u32;
            Ok(IpcCommand::FocusPane(id))
        }
        "split" => {
            let direction = v
                .get("direction")
                .and_then(|d| d.as_str())
                .unwrap_or("vertical")
                .to_string();
            let spec: PaneSpec = serde_json::from_value(
                v.get("spec")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("missing 'spec'"))?,
            )?;
            Ok(IpcCommand::Split { direction, spec })
        }
        "maximize" => {
            let id = v
                .get("id")
                .and_then(|i| i.as_u64())
                .ok_or_else(|| anyhow::anyhow!("missing 'id'"))? as u32;
            Ok(IpcCommand::Maximize(id))
        }
        "send_keys" => {
            let id = v
                .get("id")
                .and_then(|i| i.as_u64())
                .ok_or_else(|| anyhow::anyhow!("missing 'id'"))? as u32;
            let keys = v
                .get("keys")
                .and_then(|k| k.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing 'keys'"))?
                .to_string();
            Ok(IpcCommand::SendKeys { id, keys })
        }
        "quit" => Ok(IpcCommand::Quit),
        _ => Err(anyhow::anyhow!("unknown command: {}", cmd_type)),
    }
}

/// Connect to a running tmuch instance and send a command.
pub fn send_command(json: &str) -> Result<String> {
    use std::os::unix::net::UnixStream;

    // Find the first tmuch socket
    let socket_path = find_socket()?;

    let mut stream =
        UnixStream::connect(&socket_path).map_err(|e| anyhow::anyhow!("connect: {}", e))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    writeln!(stream, "{}", json)?;
    stream.flush()?;

    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            return Ok(line);
        }
    }

    Err(anyhow::anyhow!("no response from tmuch"))
}

fn find_socket() -> Result<PathBuf> {
    // Search XDG_RUNTIME_DIR first, then fall back to /tmp
    let dirs_to_search: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            dirs.push(PathBuf::from(xdg));
        }
        dirs.push(PathBuf::from("/tmp"));
        dirs
    };

    for dir in &dirs_to_search {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("tmuch-") && name.ends_with(".sock") {
                    return Ok(entry.path());
                }
            }
        }
    }
    Err(anyhow::anyhow!(
        "no running tmuch instance found (no tmuch-*.sock)"
    ))
}
