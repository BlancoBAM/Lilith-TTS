use serde::{Deserialize, Serialize};

/// Messages sent over the Unix socket between daemon and GUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub action: IpcAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcAction {
    /// Hotkey fired — show popup or begin reading
    Activate { mode: ActivateMode },
    /// GUI requests engine to speak text
    Speak {
        text: String,
        speed: f32,
        pitch: f32,
        voice_id: String,
    },
    /// GUI requests reading clipboard
    ReadClipboard,
    /// GUI requests reading focused screen content
    ReadScreen,
    /// Pause current playback
    Pause,
    /// Resume paused playback
    Resume,
    /// Stop all playback
    Stop,
    /// Engine reports status update back to GUI
    StatusUpdate {
        status: EngineStatus,
        progress: f32, // 0.0 – 1.0
        current_text: String,
    },
    /// Request GUI to show the selection overlay
    ShowSelectionOverlay,
    /// User confirmed a text selection in overlay
    SelectionConfirmed { text: String },
    /// Graceful shutdown
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivateMode {
    Screen,
    Clipboard,
    Selection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EngineStatus {
    Idle,
    Reading,
    Paused,
    Error(String),
}

/// Path to the Unix domain socket file
pub fn socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    runtime_dir.join("lilith-tts.sock")
}
