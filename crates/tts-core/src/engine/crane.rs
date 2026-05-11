use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use serde_json::json;
use std::path::Path;

use super::{AudioBytes, TtsProvider};

/// Talks to a running `crane-oai` server's OpenAI-compatible `/v1/audio/speech`
/// endpoint, which serves Qwen3-TTS with full voice cloning support.
///
/// To run the server:
///   cargo run -p crane-oai --release -- \
///       --model-path /path/to/Qwen3-TTS-12Hz-0.6B-CustomVoice
///
/// Set provider = { type = "Crane", base_url = "http://localhost:8000", model = "Qwen3-TTS" }
/// in ~/.config/lilith-tts/config.toml to activate.
pub struct CraneProvider {
    base_url: String,
    model: String,
    client: Client,
}

impl CraneProvider {
    pub fn new(base_url: String, model: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("Building HTTP client for Crane")?;
        Ok(Self {
            base_url,
            model,
            client,
        })
    }
}

#[async_trait]
impl TtsProvider for CraneProvider {
    fn name(&self) -> &str {
        "crane-qwen3-tts"
    }

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        _pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes> {
        let voice = if voice_id.is_empty() || voice_id == "default" {
            "Chelsie" // a default CustomVoice speaker
        } else {
            voice_id
        };

        let body = json!({
            "model": self.model,
            "input": text,
            "voice": voice,
            "speed": speed,
            "response_format": "wav"
        });

        let url = format!("{}/v1/audio/speech", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("POST to crane-oai /v1/audio/speech")?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            anyhow::bail!("crane-oai returned {}: {}", status, err_body);
        }

        let bytes: Bytes = response.bytes().await.context("Reading audio response")?;
        Ok(bytes.to_vec())
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        // Crane CustomVoice built-in speakers (Qwen3-TTS-CustomVoice)
        Ok(vec![
            "Chelsie".to_string(),
            "Ethan".to_string(),
            "Serena".to_string(),
            "David".to_string(),
            "Rose".to_string(),
            "William".to_string(),
            "Emily".to_string(),
        ])
    }

    fn supports_voice_clone(&self) -> bool {
        true
    }

    /// Clone a voice from a reference WAV file by sending it to the Base model endpoint.
    async fn clone_voice(&self, name: &str, ref_audio: &Path) -> Result<String> {
        let audio_bytes = tokio::fs::read(ref_audio)
            .await
            .context("Reading reference audio")?;
        let url = format!(
            "{}/v1/audio/voice_clone",
            self.base_url.trim_end_matches('/')
        );

        // Multipart upload: name + reference audio file
        let file_name = ref_audio
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reference.wav")
            .to_string();

        let part = reqwest::multipart::Part::bytes(audio_bytes)
            .file_name(file_name)
            .mime_str("audio/wav")?;

        let form = reqwest::multipart::Form::new()
            .text("name", name.to_string())
            .part("reference", part);

        let response = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("Voice clone upload")?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            anyhow::bail!("Voice clone failed {}: {}", status, err_body);
        }

        let json: serde_json::Value = response.json().await?;
        let voice_id = json["voice_id"].as_str().unwrap_or(name).to_string();

        tracing::info!(voice_id, name, "Voice cloned successfully");
        Ok(voice_id)
    }
}
