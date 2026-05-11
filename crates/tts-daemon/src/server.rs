//! Unix domain socket IPC server.
//! The daemon listens on $XDG_RUNTIME_DIR/lilith-tts.sock.
//! The GUI connects as a client and sends/receives JSON IpcMessage objects.

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tts_core::ipc::{socket_path, IpcAction, IpcMessage};

use crate::AppState;

/// Start the Unix socket server. Each GUI connection gets its own task.
pub async fn run_ipc_server(tx: broadcast::Sender<IpcAction>, _state: AppState) -> Result<()> {
    let sock_path = socket_path();

    // Remove stale socket file if it exists
    if sock_path.exists() {
        std::fs::remove_file(&sock_path)?;
    }

    let listener = UnixListener::bind(&sock_path)
        .with_context(|| format!("Binding Unix socket at {}", sock_path.display()))?;

    info!("IPC server listening at {}", sock_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let client_tx = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, client_tx).await {
                        warn!("IPC client error: {}", e);
                    }
                });
            }
            Err(e) => warn!("IPC accept error: {}", e),
        }
    }
}

async fn handle_client(stream: UnixStream, tx: broadcast::Sender<IpcAction>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let msg: IpcMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                warn!("Bad IPC message: {} — {}", e, line);
                continue;
            }
        };

        match &msg.action {
            IpcAction::Shutdown => {
                let _ = tx.send(IpcAction::Shutdown);
                break;
            }
            action => {
                let _ = tx.send(action.clone());
                // Acknowledge
                let ack = serde_json::to_string(&IpcMessage {
                    action: IpcAction::StatusUpdate {
                        status: tts_core::ipc::EngineStatus::Idle,
                        progress: 0.0,
                        current_text: String::new(),
                    },
                })?;
                writer.write_all(ack.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
    }

    Ok(())
}

/// Send a one-shot notification to the GUI by connecting as a client.
pub async fn notify_gui(msg: IpcMessage) -> Result<()> {
    let sock_path = socket_path();
    // The GUI also listens on a separate port (or reverse connection)
    // For simplicity, the GUI polls via its own socket listener.
    // This is a no-op placeholder — the GUI receives events by subscribing
    // to the same broadcast channel internally when embedded.
    let _ = (sock_path, msg);
    Ok(())
}

/// Send a command to the daemon from the GUI process.
pub async fn send_to_daemon(action: IpcAction) -> Result<()> {
    let sock_path = socket_path();
    let mut stream = UnixStream::connect(&sock_path)
        .await
        .with_context(|| "Connecting to daemon socket (is lilith-tts-daemon running?)")?;

    let msg = IpcMessage { action };
    let json = serde_json::to_string(&msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}
