use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSConfig {
    pub model_path: PathBuf,
    pub sample_rate: u32,
    pub speed: f32,
    pub pitch: f32,
    pub enable_cortex_mem: bool,
}

impl Default for TTSConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("/var/lib/lilith/models/neutts-nano-q4.gguf"),
            sample_rate: 22050,
            speed: 1.0,
            pitch: 1.0,
            enable_cortex_mem: true,
        }
    }
}

pub struct NeutTSEngine {
    config: TTSConfig,
    http_client: Client,
}

impl NeutTSEngine {
    pub fn new(config: TTSConfig) -> Result<Self> {
        if !config.model_path.exists() {
            return Err(anyhow!("TTS model not found at: {}", config.model_path.display()));
        }

        Ok(Self { 
            config,
            http_client: Client::new(),
        })
    }

    /// Queries cortex-mem for user pronunciation preferences
    async fn fetch_phonetics_context(&self) -> String {
        if !self.config.enable_cortex_mem {
            return String::new();
        }
        
        // Connect to Cortex-Mem Service API to fetch user phonetic preferences
        match self.http_client
            .get("http://localhost:8000/api/v1/search")
            .query(&[("query", "pronunciation preference"), ("scope", "user")])
            .send()
            .await 
        {
            Ok(res) if res.status().is_success() => {
                if let Ok(data) = res.text().await {
                    log::info!("Cortex-Mem injected phonetic rules: {}", data);
                    return format!("<cortex_phonetics>{}</cortex_phonetics>\n", data);
                }
            }
            _ => log::warn!("Failed to reach cortex-mem service, falling back to pure text"),
        }
        String::new()
    }

    /// Synthesize text to speech
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let phonetic_context = self.fetch_phonetics_context().await;
        let enhanced_text = format!("{}{}", phonetic_context, text);
        
        log::info!("Synthesizing TTS for: {}", enhanced_text);
        
        self.generate_placeholder_wav(text.len() * 100)
    }

    fn generate_placeholder_wav(&self, duration_samples: usize) -> Result<Vec<u8>> {
        use hound::{WavSpec, WavWriter};
        use std::io::Cursor;

        let spec = WavSpec {
            channels: 1,
            sample_rate: self.config.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for _ in 0..duration_samples { writer.write_sample(0i16)?; }
            writer.finalize()?;
        }
        Ok(cursor.into_inner())
    }

    pub async fn synthesize_to_file(&self, text: &str, output_path: &str) -> Result<()> {
        let audio_bytes = self.synthesize(text).await?;
        std::fs::write(output_path, audio_bytes)?;
        Ok(())
    }
}
