use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;
use tokio::process::Command;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Stopped,
}

/// Subprocess-based audio player.
/// Priority: paplay (PipeWire/PulseAudio) → aplay (ALSA) → ffplay
/// No C build dependencies — works on any Lilith/Ubuntu system out of the box.
pub struct AudioPlayer {
    state: Arc<Mutex<PlaybackState>>,
    stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PlaybackState::Idle)),
            stop_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Write WAV bytes to a temp file and play via the best available player.
    /// Non-blocking: returns immediately; playback runs in a spawned task.
    pub async fn play_wav(&self, wav_bytes: Vec<u8>) -> Result<()> {
        // Stop any existing playback
        self.stop();

        // Write to temp file (auto-deleted when handle drops)
        let mut tmp = NamedTempFile::new().context("Creating temp WAV file")?;
        tmp.write_all(&wav_bytes)
            .context("Writing WAV to temp file")?;
        tmp.flush()?;
        let tmp_path = tmp.path().to_path_buf();

        let (stop_tx, stop_rx) = watch::channel(false);
        *self.stop_tx.lock().unwrap() = Some(stop_tx);

        let state = self.state.clone();
        *state.lock().unwrap() = PlaybackState::Playing;

        tokio::spawn(async move {
            // Keep temp file alive for the duration of playback
            let _tmp_guard = tmp;

            let result = play_wav_subprocess(&tmp_path, stop_rx).await;
            if let Err(e) = result {
                tracing::warn!("Audio playback error: {}", e);
            }
            *state.lock().unwrap() = PlaybackState::Idle;
        });

        Ok(())
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.stop_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(true);
            }
        }
        *self.state.lock().unwrap() = PlaybackState::Stopped;
    }

    pub fn is_playing(&self) -> bool {
        *self.state.lock().unwrap() == PlaybackState::Playing
    }

    pub fn is_idle(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            PlaybackState::Idle | PlaybackState::Stopped
        )
    }
}

/// Try players in priority order until one succeeds.
async fn play_wav_subprocess(path: &PathBuf, mut stop_rx: watch::Receiver<bool>) -> Result<()> {
    let path_str = path.to_string_lossy().into_owned();

    // Try each player in order
    for player in &["paplay", "aplay", "ffplay", "mpv"] {
        if !command_exists(player).await {
            continue;
        }

        let args: Vec<&str> = match *player {
            "paplay" => vec![&path_str],
            "aplay" => vec!["-q", &path_str],
            "ffplay" => vec!["-nodisp", "-autoexit", "-loglevel", "quiet", &path_str],
            "mpv" => vec!["--no-video", "--really-quiet", &path_str],
            _ => vec![&path_str],
        };

        let mut child = Command::new(player)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("Spawning {}", player))?;

        tracing::debug!("Playing via {}", player);

        loop {
            tokio::select! {
                status = child.wait() => {
                    match status {
                        Ok(s) if s.success() => return Ok(()),
                        Ok(_) => break, // try next player
                        Err(e) => return Err(e.into()),
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        let _ = child.kill().await;
                        return Ok(());
                    }
                }
            }
        }
    }

    tracing::warn!("No audio player found. Install paplay (PulseAudio/PipeWire) or aplay.");
    Ok(())
}

async fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─── Text chunking ───────────────────────────────────────────────────────────

/// Split long text into ~800-char sentence-boundary chunks for streaming TTS.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for sentence in split_sentences(text) {
        if current.len() + sentence.len() > max_chars && !current.is_empty() {
            chunks.push(current.trim().to_string());
            current.clear();
        }
        current.push_str(&sentence);
        current.push(' ');
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev = ' ';

    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?') && prev != '.' {
            out.push(cur.trim().to_string());
            cur.clear();
        }
        prev = ch;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}
