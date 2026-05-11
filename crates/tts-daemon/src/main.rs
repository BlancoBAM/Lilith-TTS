mod hotkey;
mod server;

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tts_core::{
    audio::{chunk_text, AudioPlayer},
    config::TtsConfig,
    engine,
    ipc::{socket_path, EngineStatus, IpcAction, IpcMessage},
    reader::{atspi_reader, clipboard},
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Mutex<TtsConfig>>,
    pub status: Arc<Mutex<EngineStatus>>,
    pub cancel_token: Arc<Mutex<Option<CancellationToken>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tts_daemon=info".parse().unwrap())
                .add_directive("tts_core=info".parse().unwrap()),
        )
        .init();

    info!("Lilith-TTS daemon starting");

    // Ensure single instance via lock file
    let lock_path = std::env::temp_dir().join("lilith-tts-daemon.lock");
    let lock_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&lock_path)
        .context("Opening lock file")?;
    use fs2::FileExt;
    if lock_file.try_lock_exclusive().is_err() {
        eprintln!("Lilith-TTS daemon is already running.");
        return Ok(());
    }

    let config = Arc::new(Mutex::new(TtsConfig::load()?));
    let status = Arc::new(Mutex::new(EngineStatus::Idle));
    let cancel_token = Arc::new(Mutex::new(None));

    let state = AppState {
        config: config.clone(),
        status: status.clone(),
        cancel_token: cancel_token.clone(),
    };

    // Channel: hotkey → engine actions
    let (action_tx, mut action_rx) = broadcast::channel::<IpcAction>(32);

    // Spawn hotkey listener thread
    let hotkey_tx = action_tx.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = hotkey::listen_for_hotkey(hotkey_tx) {
            tracing::error!("Hotkey listener error: {}", e);
        }
    });

    // Spawn IPC server (Unix socket)
    let ipc_tx = action_tx.clone();
    let ipc_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_ipc_server(ipc_tx, ipc_state).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    info!("Daemon ready. Listening for hotkey Ctrl+T+T+M ...");

    // Main action dispatch loop
    loop {
        let action = match action_rx.recv().await {
            Ok(a) => a,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Dropped {} actions (overloaded)", n);
                continue;
            }
            Err(_) => break,
        };

        let config_snapshot = config.lock().await.clone();

        match action {
            IpcAction::ReadScreen => {
                let skip_roles = config_snapshot.skip_roles.clone();
                let smart = config_snapshot.smart_skip_ui_chrome;
                let cfg = config_snapshot.clone();
                let st = status.clone();
                let ct = cancel_token.clone();

                tokio::spawn(async move {
                    // Cancel any previous task
                    if let Some(token) = ct.lock().await.take() {
                        token.cancel();
                    }
                    let token = CancellationToken::new();
                    *ct.lock().await = Some(token.clone());

                    *st.lock().await = EngineStatus::Reading;
                    match atspi_reader::read_focused_screen(&skip_roles, smart).await {
                        Ok(content) if !content.is_empty() => {
                            info!(
                                "Screen read: {} words from {}",
                                content.word_count, content.source
                            );
                            if let Err(e) = speak_text(&content.text, &cfg, token).await {
                                tracing::error!("TTS error: {}", e);
                            }
                        }
                        Ok(_) => {
                            tracing::warn!(
                                "Screen read returned empty — sending ShowSelectionOverlay"
                            );
                            // Notify GUI to show selection mode
                            let _ = server::notify_gui(IpcMessage {
                                action: IpcAction::ShowSelectionOverlay,
                            })
                            .await;
                        }
                        Err(e) => tracing::error!("Screen read error: {}", e),
                    }
                    *st.lock().await = EngineStatus::Idle;
                });
            }

            IpcAction::ReadClipboard => {
                let cfg = config_snapshot.clone();
                let st = status.clone();
                let ct = cancel_token.clone();

                tokio::spawn(async move {
                    if let Some(token) = ct.lock().await.take() {
                        token.cancel();
                    }
                    let token = CancellationToken::new();
                    *ct.lock().await = Some(token.clone());

                    *st.lock().await = EngineStatus::Reading;
                    match clipboard::read_clipboard() {
                        Ok(content) => {
                            if let Err(e) = speak_text(&content.text, &cfg, token).await {
                                tracing::error!("TTS clipboard error: {}", e);
                            }
                        }
                        Err(e) => tracing::error!("Clipboard read error: {}", e),
                    }
                    *st.lock().await = EngineStatus::Idle;
                });
            }

            IpcAction::Speak {
                text,
                speed,
                pitch,
                voice_id,
            } => {
                let mut cfg = config_snapshot.clone();
                cfg.speed = speed;
                cfg.pitch = pitch;
                cfg.active_voice = voice_id;
                let st = status.clone();
                let ct = cancel_token.clone();

                tokio::spawn(async move {
                    if let Some(token) = ct.lock().await.take() {
                        token.cancel();
                    }
                    let token = CancellationToken::new();
                    *ct.lock().await = Some(token.clone());

                    *st.lock().await = EngineStatus::Reading;
                    if let Err(e) = speak_text(&text, &cfg, token).await {
                        tracing::error!("TTS speak error: {}", e);
                    }
                    *st.lock().await = EngineStatus::Idle;
                });
            }

            IpcAction::Activate { mode } => {
                use tts_core::ipc::ActivateMode;
                let gui_action = match mode {
                    ActivateMode::Screen => IpcAction::ReadScreen,
                    ActivateMode::Clipboard => IpcAction::ReadClipboard,
                    ActivateMode::Selection => IpcAction::ShowSelectionOverlay,
                };
                // Forward to self to trigger the right handler, also tell GUI to show
                let _ = action_tx.send(gui_action);
                let _ = server::notify_gui(IpcMessage {
                    action: IpcAction::Activate {
                        mode: tts_core::ipc::ActivateMode::Screen,
                    },
                })
                .await;
            }

            IpcAction::Stop => {
                *status.lock().await = EngineStatus::Idle;
                if let Some(token) = cancel_token.lock().await.take() {
                    token.cancel();
                }
            }

            IpcAction::Shutdown => {
                info!("Shutdown requested");
                break;
            }

            _ => {}
        }
    }

    info!("Daemon shutting down");
    Ok(())
}

/// Synthesize and play text using the configured TTS engine.
/// Streams chunk-by-chunk: synthesize → play → wait → next chunk.
async fn speak_text(text: &str, cfg: &TtsConfig, token: CancellationToken) -> Result<()> {
    let engine = engine::build_from_config(cfg).await?;
    let player = AudioPlayer::new();

    let chunks = chunk_text(text, 800);
    for chunk in chunks {
        if token.is_cancelled() {
            player.stop();
            return Ok(());
        }

        let audio = engine
            .synthesize(&chunk, cfg.speed, cfg.pitch, &cfg.active_voice)
            .await?;

        if token.is_cancelled() {
            player.stop();
            return Ok(());
        }

        player.play_wav(audio).await?;

        while !player.is_idle() {
            if token.is_cancelled() {
                player.stop();
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
    Ok(())
}
