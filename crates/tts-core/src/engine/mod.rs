pub mod crane;
pub mod espeak;
pub mod neutts;

use anyhow::Result;
use async_trait::async_trait;

/// Opaque audio output — raw WAV bytes ready for rodio
pub type AudioBytes = Vec<u8>;

/// Implemented by each TTS backend
#[async_trait]
pub trait TtsProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes>;

    async fn list_voices(&self) -> Result<Vec<String>>;

    fn supports_voice_clone(&self) -> bool {
        false
    }

    async fn clone_voice(&self, _name: &str, _ref_audio: &std::path::Path) -> Result<String> {
        anyhow::bail!("Voice cloning not supported by this provider")
    }
}

/// Central engine that delegates to a `TtsProvider`
pub struct TtsEngine {
    pub provider: Box<dyn TtsProvider>,
}

impl TtsEngine {
    pub fn new(provider: impl TtsProvider + 'static) -> Self {
        Self {
            provider: Box::new(provider),
        }
    }

    pub async fn synthesize(
        &self,
        text: &str,
        speed: f32,
        pitch: f32,
        voice_id: &str,
    ) -> Result<AudioBytes> {
        tracing::info!(
            provider = self.provider.name(),
            chars = text.len(),
            speed,
            "Synthesizing"
        );
        self.provider.synthesize(text, speed, pitch, voice_id).await
    }

    pub async fn list_voices(&self) -> Result<Vec<String>> {
        self.provider.list_voices().await
    }

    pub fn supports_voice_clone(&self) -> bool {
        self.provider.supports_voice_clone()
    }

    pub async fn clone_voice(&self, name: &str, ref_audio: &std::path::Path) -> Result<String> {
        self.provider.clone_voice(name, ref_audio).await
    }
}

/// Build the active TtsEngine from config
pub async fn build_from_config(cfg: &crate::config::TtsConfig) -> Result<TtsEngine> {
    use crate::config::ProviderConfig;
    match &cfg.provider {
        ProviderConfig::NeuTts { model_path } => Ok(TtsEngine::new(neutts::NeuTtsProvider::new(
            model_path.clone(),
        ))),
        ProviderConfig::Espeak { voice } => {
            Ok(TtsEngine::new(espeak::EspeakProvider::new(voice.clone())))
        }
        ProviderConfig::Piper {
            model_path,
            piper_bin,
        } => Ok(TtsEngine::new(espeak::PiperProvider::new(
            piper_bin.clone(),
            model_path.clone(),
        ))),
        ProviderConfig::Crane { base_url, model } => Ok(TtsEngine::new(crane::CraneProvider::new(
            base_url.clone(),
            model.clone(),
        )?)),
    }
}
