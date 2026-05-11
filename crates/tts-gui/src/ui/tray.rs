//! System tray icon via the StatusNotifierItem D-Bus protocol (ksni).
//! Works on COSMIC, KDE Plasma, and GNOME (with AppIndicator extension).

use ksni::Tray;
use tts_core::ipc::IpcAction;

pub struct LilithTrayIcon {
    pub action_sender: tokio::sync::mpsc::UnboundedSender<TrayEvent>,
}

#[derive(Debug)]
pub enum TrayEvent {
    ShowWindow,
    ReadScreen,
    ReadClipboard,
    Quit,
}

impl Tray for LilithTrayIcon {
    fn id(&self) -> String {
        "lilith-tts".to_string()
    }

    fn title(&self) -> String {
        "Lilith TTS".to_string()
    }

    fn icon_name(&self) -> String {
        // Use the installed icon; falls back to audio-headphones
        "lilith-tts".to_string()
    }

    fn icon_theme_path(&self) -> String {
        "/usr/share/icons/hicolor".to_string()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "lilith-tts".to_string(),
            icon_pixmap: vec![],
            title: "Lilith TTS".to_string(),
            description: "Text-to-Speech for Lilith Linux\nCtrl+T+T+M to activate".to_string(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "🌙 Open TTS Panel".to_string(),
                activate: Box::new(|tray: &mut LilithTrayIcon| {
                    let _ = tray.action_sender.send(TrayEvent::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "▶ Read Screen".to_string(),
                activate: Box::new(|tray: &mut LilithTrayIcon| {
                    let _ = tray.action_sender.send(TrayEvent::ReadScreen);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "📋 Read Clipboard".to_string(),
                activate: Box::new(|tray: &mut LilithTrayIcon| {
                    let _ = tray.action_sender.send(TrayEvent::ReadClipboard);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "✕ Quit".to_string(),
                activate: Box::new(|tray: &mut LilithTrayIcon| {
                    let _ = tray.action_sender.send(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the tray icon in a background thread.
/// Returns a channel sender for tray menu events.
pub fn spawn_tray() -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<TrayEvent>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let tray = LilithTrayIcon { action_sender: tx };

    // ksni blocks internally; run in its own thread
    std::thread::spawn(move || {
        let mut service = ksni::TrayService::new(tray);
        service.spawn();
    });

    Ok(rx)
}
