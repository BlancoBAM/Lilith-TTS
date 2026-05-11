use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use super::{AudioBytes, TtsProvider};

/// Neuphonic NeuTTS Nano — GGUF-format TTS via llama.cpp compatible inference.
///
/// Model: neutts-nano-q4.gguf (~229M params, ~120MB at Q4)
/// Bundled with Lilith Linux at: /var/lib/lilith/models/neutts-nano-q4.gguf
///
/// Features:
///   • Real-time or faster-than-real-time CPU inference
///   • Instant voice cloning from 3–15 second reference audio clips
///   • NeuCodec neural audio codec for high-quality output
///   • espeak-ng for phonemization (already required by TTS)
///
/// This provider shells out to the `neutts` CLI (if available) or uses the
/// llama.cpp server API endpoint, keeping the Rust build clean of C++ FFI.
pub struct NeuTtsProvider {
    model_path: PathBuf,
    /// Path to `neutts` CLI binary (installed by Lilith)
    neutts_bin: PathBuf,
}

impl NeuTtsProvider {
    pub fn new(model_path: PathBuf) -> Self {
        // Try the Lilith system location, then local cargo install
        let neutts_bin = which_bin("neutts")
            .or_else(|| which_bin("neutts-cli"))
            .unwrap_or_else(|| PathBuf::from("/usr/local/bin/neutts"));

        Self {
            model_path,
            neutts_bin,
        }
    }

    /// Use the llama.cpp server HTTP API as an alternative to the CLI
    pub fn new_with_server(model_path: PathBuf, server_url: &str) -> NeuTtsServerProvider {
        NeuTtsServerProvider {
            model_path,
            server_url: server_url.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for NeuTtsProvider {
    fn name(&self) -> &str {
        "neutts-nano"
    }

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        _pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes> {
        // neutts CLI: neutts --model <path> --speed <f> [--voice <id>] --text <text> --output -
        let mut args = vec![
            "--model".to_string(),
            self.model_path.to_string_lossy().into_owned(),
            "--speed".to_string(),
            speed.to_string(),
            "--output".to_string(),
            "-".to_string(), // stdout
        ];

        if !voice_id.is_empty() && voice_id != "default" {
            args.push("--voice".to_string());
            args.push(voice_id.to_string());
        }

        args.push("--text".to_string());
        args.push(text.to_string());

        let output = Command::new(&self.neutts_bin)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .context("Spawning neutts CLI")?;

        if !output.status.success() {
            anyhow::bail!(
                "neutts exited with status {}",
                output.status.code().unwrap_or(-1)
            );
        }

        Ok(output.stdout) // neutts outputs WAV directly
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        let output = Command::new(&self.neutts_bin)
            .args(["--list-voices"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                let voices = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.trim().to_string())
                    .collect();
                Ok(voices)
            }
            _ => {
                // Fallback: list voice ref files in the voices directory
                Ok(list_voice_refs())
            }
        }
    }

    fn supports_voice_clone(&self) -> bool {
        true
    }

    /// Clone a voice by providing a 3–15 second clean WAV reference clip.
    /// The reference is registered and a voice ID returned for future use.
    async fn clone_voice(&self, name: &str, ref_audio: &Path) -> Result<String> {
        let voices_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("lilith-tts")
            .join("voices");

        std::fs::create_dir_all(&voices_dir)?;

        // Copy reference audio to voices directory
        let dest = voices_dir.join(format!("{}.wav", name));
        tokio::fs::copy(ref_audio, &dest)
            .await
            .context("Copying reference audio to voice library")?;

        // Register with neutts (if CLI supports it)
        let reg_output = Command::new(&self.neutts_bin)
            .args([
                "--register-voice",
                "--name",
                name,
                "--reference",
                dest.to_str().unwrap_or(""),
            ])
            .output()
            .await;

        match reg_output {
            Ok(o) if o.status.success() => {
                tracing::info!(name, "Voice registered with neutts");
            }
            _ => {
                // Fallback: just store the reference file; neutts accepts --voice <path>
                tracing::info!(name, "Voice reference stored (path-based cloning)");
            }
        }

        Ok(name.to_string())
    }
}

// ─── llama.cpp server API variant ─────────────────────────────────────────

/// Uses the llama.cpp TTS server HTTP endpoint.
/// Useful when running NeuTTS via `llama-server` for multi-client access.
pub struct NeuTtsServerProvider {
    pub model_path: PathBuf,
    pub server_url: String,
    pub client: reqwest::Client,
}

#[async_trait]
impl TtsProvider for NeuTtsServerProvider {
    fn name(&self) -> &str {
        "neutts-server"
    }

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        _pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes> {
        use serde_json::json;

        let body = json!({
            "input": text,
            "voice": if voice_id.is_empty() || voice_id == "default" { "default" } else { voice_id },
            "speed": speed,
            "response_format": "wav"
        });

        let url = format!("{}/v1/audio/speech", self.server_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("POST to llama.cpp TTS server")?;

        if !resp.status().is_success() {
            anyhow::bail!("llama.cpp TTS server error: {}", resp.status());
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        Ok(list_voice_refs())
    }

    fn supports_voice_clone(&self) -> bool {
        true
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn which_bin(name: &str) -> Option<PathBuf> {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
}

fn list_voice_refs() -> Vec<String> {
    let voices_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("lilith-tts")
        .join("voices");

    std::fs::read_dir(&voices_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "wav" || x == "mp3") {
                p.file_stem().map(|s| s.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect()
}
