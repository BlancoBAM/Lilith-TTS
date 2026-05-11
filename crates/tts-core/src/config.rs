use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted user configuration at ~/.config/lilith-tts/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Which TTS backend to use
    pub provider: ProviderConfig,
    /// Active voice ID
    pub active_voice: String,
    /// Playback speed multiplier (0.5 – 3.0)
    pub speed: f32,
    /// Pitch multiplier (0.5 – 2.0)
    pub pitch: f32,
    /// Default reading mode
    pub default_mode: ReadMode,
    /// Paths to user-cloned voice reference files
    pub voice_refs_dir: PathBuf,
    /// Whether to skip navigator/toolbar elements during screen reading
    pub smart_skip_ui_chrome: bool,
    /// Elements to explicitly skip by AT-SPI role names
    pub skip_roles: Vec<String>,
    /// Global hotkey sequence (display string only; daemon uses it)
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProviderConfig {
    /// NeuTTS Nano — primary, bundled with Lilith Linux
    /// GGUF format (~120MB at Q4), real-time CPU, voice cloning from 3-15s clips
    NeuTts { model_path: PathBuf },
    /// System TTS via espeak-ng — always-available fallback
    Espeak { voice: String },
    /// Piper TTS binary (local, high-quality neural voices)
    Piper {
        model_path: PathBuf,
        piper_bin: PathBuf,
    },
    /// Crane-OAI HTTP server (Qwen3-TTS, alternative high-quality option)
    Crane { base_url: String, model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    /// Intelligently read focused screen content
    Screen,
    /// Read clipboard contents
    Clipboard,
    /// User-guided selection (click to mark start/end)
    Selection,
    /// Manual text entry
    Manual,
}

impl Default for TtsConfig {
    fn default() -> Self {
        let voice_refs_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("lilith-tts")
            .join("voices");

        Self {
            provider: ProviderConfig::NeuTts {
                model_path: PathBuf::from("/var/lib/lilith/models/neutts-nano-q4.gguf"),
            },
            active_voice: "default".to_string(),
            speed: 1.0,
            pitch: 1.0,
            default_mode: ReadMode::Screen,
            voice_refs_dir,
            smart_skip_ui_chrome: true,
            skip_roles: vec![
                "menu_bar".to_string(),
                "tool_bar".to_string(),
                "navigation".to_string(),
                "banner".to_string(),
                "complementary".to_string(),
                "content_info".to_string(),
                "status_bar".to_string(),
            ],
            hotkey: "Ctrl+T+T+M".to_string(),
        }
    }
}

impl TtsConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("lilith-tts")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Reading config from {}", path.display()))?;
        let cfg: Self = toml::from_str(&raw).with_context(|| "Parsing config TOML")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(&path, raw)?;
        Ok(())
    }
}
