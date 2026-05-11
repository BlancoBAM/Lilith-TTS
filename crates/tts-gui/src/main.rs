mod app;
mod ipc_client;
mod ui;

use anyhow::Result;
use app::LilithTtsApp;
use iced::{window, Size};

use tts_core::ipc::IpcAction;
use ui::tray::{spawn_tray, TrayEvent};

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tts_gui=info".parse().unwrap()),
        )
        .init();

    // Spawn system tray icon (best-effort; not all compositors support SNI)
    if let Err(e) = try_spawn_tray() {
        tracing::warn!("Tray icon unavailable: {}", e);
    }

    // Borderless always-on-top popup window
    let window_settings = window::Settings {
        size: Size::new(400.0, 560.0),
        position: window::Position::Specific(iced::Point::new(20.0, 40.0)),
        resizable: false,
        decorations: false,
        transparent: false,
        level: window::Level::AlwaysOnTop,
        ..Default::default()
    };

    iced::application("Lilith TTS", LilithTtsApp::update, LilithTtsApp::view)
        .window(window_settings)
        .theme(|_| iced::Theme::Dark)
        .run_with(LilithTtsApp::new)
}

fn try_spawn_tray() -> Result<()> {
    let mut rx = spawn_tray()?;

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            while let Some(event) = rx.recv().await {
                let action = match event {
                    TrayEvent::ShowWindow => continue,
                    TrayEvent::ReadScreen => IpcAction::ReadScreen,
                    TrayEvent::ReadClipboard => IpcAction::ReadClipboard,
                    TrayEvent::Quit => IpcAction::Shutdown,
                };
                if let Err(e) = ipc_client::send(action).await {
                    tracing::warn!("Tray→daemon: {}", e);
                }
            }
        });
    });

    Ok(())
}
