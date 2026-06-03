// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// Persisted applet-level state (via cosmic-settings-daemon).
/// Intentionally minimal — the full TTS config lives in TtsConfig (tts-core).
#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq, Serialize, Deserialize)]
#[version = 1]
pub struct Config {
    /// Whether to show the settings panel on next open (persists user intent)
    pub show_settings: bool,
}
