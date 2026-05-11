use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A single voice profile (built-in or user-cloned)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub display_name: String,
    pub kind: VoiceKind,
    /// Path to reference WAV (for user-cloned voices)
    pub reference_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKind {
    /// Built-in voice from the TTS provider
    BuiltIn,
    /// User-cloned from a reference audio file
    Cloned,
}

impl Voice {
    pub fn builtin(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: name.into(),
            kind: VoiceKind::BuiltIn,
            reference_path: None,
        }
    }

    pub fn cloned(id: impl Into<String>, name: impl Into<String>, ref_path: PathBuf) -> Self {
        Self {
            id: id.into(),
            display_name: name.into(),
            kind: VoiceKind::Cloned,
            reference_path: Some(ref_path),
        }
    }
}

/// Manages the list of available voices, persisted to disk.
pub struct VoiceManager {
    voices_dir: PathBuf,
    voices_file: PathBuf,
    pub voices: Vec<Voice>,
}

impl VoiceManager {
    pub fn load() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("lilith-tts");
        let voices_dir = config_dir.join("voices");
        let voices_file = config_dir.join("voices.json");

        std::fs::create_dir_all(&voices_dir)?;

        let voices = if voices_file.exists() {
            let raw = std::fs::read_to_string(&voices_file)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            default_voices()
        };

        Ok(Self {
            voices_dir,
            voices_file,
            voices,
        })
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.voices)?;
        std::fs::write(&self.voices_file, json)?;
        Ok(())
    }

    /// Register a new cloned voice from a reference audio file.
    /// The reference file is copied to the voices directory.
    pub fn add_cloned_voice(&mut self, name: &str, ref_audio: &Path) -> Result<Voice> {
        let ext = ref_audio
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav");
        let dest = self.voices_dir.join(format!("{}.{}", name, ext));
        std::fs::copy(ref_audio, &dest)?;

        let id = format!("cloned_{}", name.to_lowercase().replace(' ', "_"));
        let voice = Voice::cloned(&id, name, dest);
        self.voices.push(voice.clone());
        self.save()?;
        Ok(voice)
    }

    pub fn remove_voice(&mut self, voice_id: &str) -> Result<()> {
        if let Some(pos) = self.voices.iter().position(|v| v.id == voice_id) {
            let voice = self.voices.remove(pos);
            // Remove reference file if cloned
            if let Some(ref_path) = voice.reference_path {
                let _ = std::fs::remove_file(ref_path);
            }
            self.save()?;
        }
        Ok(())
    }

    /// Merge in provider-supplied built-in voices (keep user-cloned voices)
    pub fn merge_builtin_voices(&mut self, provider_voices: Vec<String>) {
        // Remove old built-ins
        self.voices.retain(|v| v.kind == VoiceKind::Cloned);
        // Add new built-ins at the front
        let mut builtins: Vec<Voice> = provider_voices
            .into_iter()
            .map(|id| Voice::builtin(id.clone(), title_case(&id)))
            .collect();
        builtins.extend(self.voices.drain(..));
        self.voices = builtins;
    }
}

fn default_voices() -> Vec<Voice> {
    vec![Voice::builtin("default", "Default")]
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
