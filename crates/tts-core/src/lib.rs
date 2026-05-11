pub mod audio;
pub mod config;
pub mod engine;
pub mod ipc;
pub mod reader;
pub mod voices;

pub use config::TtsConfig;
pub use engine::{TtsEngine, TtsProvider};
pub use ipc::{IpcAction, IpcMessage};
pub use voices::{Voice, VoiceManager};
