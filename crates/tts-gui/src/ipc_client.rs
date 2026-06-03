// SPDX-License-Identifier: GPL-3.0-only
//! IPC client — sends commands from the applet to the running daemon over a Unix socket.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use tts_core::ipc::{socket_path, IpcAction, IpcMessage};

/// Send an action to the daemon. Fire-and-forget (no reply expected).
pub async fn send(action: IpcAction) -> Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).await.map_err(|e| {
        anyhow::anyhow!(
            "Cannot reach Lilith-TTS daemon ({}). Is it running?\n\
             Start it with: systemctl --user start lilith-tts-daemon\n\
             Error: {}",
            path.display(),
            e
        )
    })?;

    let msg = IpcMessage { action };
    let json = serde_json::to_string(&msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Send an action and wait for a status reply.
pub async fn send_recv(action: IpcAction) -> Result<IpcMessage> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).await?;
    let (reader, mut writer) = stream.into_split();

    let msg = IpcMessage { action };
    let json = serde_json::to_string(&msg)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    let mut lines = BufReader::new(reader).lines();
    if let Some(line) = lines.next_line().await? {
        let reply: IpcMessage = serde_json::from_str(&line)?;
        return Ok(reply);
    }

    anyhow::bail!("Daemon closed connection without reply")
}
