use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

use super::{AudioBytes, TtsProvider};

// ─── espeak-ng backend ─────────────────────────────────────────────────────

/// Uses `espeak-ng` via subprocess to generate WAV audio.
/// Works on any Ubuntu system with `espeak-ng` installed (default on Lilith).
pub struct EspeakProvider {
    default_voice: String,
}

impl EspeakProvider {
    pub fn new(default_voice: String) -> Self {
        Self { default_voice }
    }

    fn effective_voice<'a>(&'a self, voice_id: &'a str) -> &'a str {
        if voice_id.is_empty() || voice_id == "default" {
            &self.default_voice
        } else {
            voice_id
        }
    }
}

#[async_trait]
impl TtsProvider for EspeakProvider {
    fn name(&self) -> &str {
        "espeak-ng"
    }

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes> {
        // espeak speed: words-per-minute; default ~175; multiply by speed factor
        let wpm = (175.0 * speed).clamp(50.0, 700.0) as u32;
        // espeak pitch: 0–99; default 50
        let pitch_val = (50.0 * pitch).clamp(0.0, 99.0) as u32;
        let voice = self.effective_voice(voice_id);

        let output = Command::new("espeak-ng")
            .args([
                "--stdout",
                "-v",
                voice,
                "-s",
                &wpm.to_string(),
                "-p",
                &pitch_val.to_string(),
                text,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .context("Spawning espeak-ng")?;

        if !output.status.success() {
            anyhow::bail!("espeak-ng exited with non-zero status");
        }
        Ok(output.stdout)
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        let output = Command::new("espeak-ng")
            .args(["--voices=en"])
            .output()
            .await
            .context("Listing espeak-ng voices")?;

        let voices = String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1) // header row
            .filter_map(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                cols.get(4).map(|v| v.to_string())
            })
            .collect();
        Ok(voices)
    }
}

// ─── Piper TTS backend ─────────────────────────────────────────────────────

/// High-quality neural TTS via the Piper binary.
/// Model files stored in ~/.config/lilith-tts/models/
pub struct PiperProvider {
    piper_bin: PathBuf,
    model_path: PathBuf,
}

impl PiperProvider {
    pub fn new(piper_bin: PathBuf, model_path: PathBuf) -> Self {
        Self {
            piper_bin,
            model_path,
        }
    }
}

#[async_trait]
impl TtsProvider for PiperProvider {
    fn name(&self) -> &str {
        "piper"
    }

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        _pitch: f32,
        _voice_id: &str,
    ) -> Result<AudioBytes> {
        let length_scale = 1.0 / speed; // Piper: higher = slower

        let mut child = Command::new(&self.piper_bin)
            .args([
                "--model",
                self.model_path.to_str().unwrap_or(""),
                "--length-scale",
                &length_scale.to_string(),
                "--output-raw",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Spawning piper")?;

        // Feed text via stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(text.as_bytes()).await?;
        }

        let output = child
            .wait_with_output()
            .await
            .context("Waiting for piper")?;
        if !output.status.success() {
            anyhow::bail!("Piper exited with non-zero status");
        }

        // Piper outputs raw PCM; wrap in WAV
        let pcm_bytes = output.stdout;
        Ok(pcm_to_wav(pcm_bytes, 22050))
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        // Piper voices are individual model files; enumerate from models dir
        let models_dir = self.model_path.parent().unwrap_or(&self.model_path);
        let mut voices = vec![];
        if let Ok(entries) = std::fs::read_dir(models_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |e| e == "onnx") {
                    if let Some(stem) = entry.path().file_stem() {
                        voices.push(stem.to_string_lossy().into_owned());
                    }
                }
            }
        }
        Ok(voices)
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Wrap raw 16-bit LE PCM in a minimal WAV container
fn pcm_to_wav(pcm: Vec<u8>, sample_rate: u32) -> Vec<u8> {
    use std::io::Write;
    let data_len = pcm.len() as u32;
    let file_len = data_len + 36;
    let mut buf = Vec::with_capacity((file_len + 8) as usize);

    // RIFF header
    buf.write_all(b"RIFF").unwrap();
    buf.write_all(&file_len.to_le_bytes()).unwrap();
    buf.write_all(b"WAVE").unwrap();

    // fmt chunk
    buf.write_all(b"fmt ").unwrap();
    buf.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    buf.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
    buf.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    buf.write_all(&sample_rate.to_le_bytes()).unwrap();
    let byte_rate = sample_rate * 2;
    buf.write_all(&byte_rate.to_le_bytes()).unwrap();
    buf.write_all(&2u16.to_le_bytes()).unwrap(); // block align
    buf.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample

    // data chunk
    buf.write_all(b"data").unwrap();
    buf.write_all(&data_len.to_le_bytes()).unwrap();
    buf.extend_from_slice(&pcm);

    buf
}
