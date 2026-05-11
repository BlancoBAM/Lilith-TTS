//! Global hotkey detection via Linux evdev.
//!
//! We read directly from /dev/input/event* devices, which works on both
//! X11 and Wayland. The user must be in the `input` group:
//!   sudo usermod -aG input $USER
//!
//! Detected sequence: Ctrl held → T pressed → T pressed → M pressed → Ctrl released
//! All presses must occur within 600ms of each other.

use anyhow::{Context, Result};
use evdev::{Device, EventType, InputEventKind, Key};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use tts_core::ipc::{ActivateMode, IpcAction};

const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(600);

/// Listen on all keyboard input devices for the Ctrl+T+T+M hotkey sequence.
/// Runs on a dedicated blocking thread.
pub fn listen_for_hotkey(tx: broadcast::Sender<IpcAction>) -> Result<()> {
    let devices = find_keyboard_devices()?;

    if devices.is_empty() {
        tracing::warn!(
            "No keyboard input devices found. Ensure user is in the 'input' group.\n\
             Run: sudo usermod -aG input $USER && newgrp input"
        );
        return Ok(());
    }

    tracing::info!(
        "Monitoring {} keyboard device(s) for Ctrl+T+T+M",
        devices.len()
    );

    // Spawn one thread per device (they don't block each other)
    let handles: Vec<_> = devices
        .into_iter()
        .map(|dev| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                monitor_device(dev, tx);
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }

    Ok(())
}

fn find_keyboard_devices() -> Result<Vec<Device>> {
    let mut keyboards = Vec::new();
    for entry in std::fs::read_dir("/dev/input")
        .context("Reading /dev/input")?
        .flatten()
    {
        let path = entry.path();
        if let Ok(device) = Device::open(&path) {
            // Only include devices that have letter keys
            if let Some(keys) = device.supported_keys() {
                if keys.contains(Key::KEY_T) && keys.contains(Key::KEY_M) {
                    keyboards.push(device);
                }
            }
        }
    }
    Ok(keyboards)
}

fn monitor_device(mut device: Device, tx: broadcast::Sender<IpcAction>) {
    let mut ctrl_held = false;
    let mut sequence: Vec<Key> = Vec::new();
    let mut last_key_time = Instant::now();

    loop {
        let events = match device.fetch_events() {
            Ok(e) => e,
            Err(_) => break,
        };

        for event in events {
            if event.event_type() != EventType::KEY {
                continue;
            }

            let value = event.value(); // 0=up, 1=down, 2=repeat
            let InputEventKind::Key(key) = event.kind() else {
                continue;
            };

            // Track Ctrl state
            if matches!(key, Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL) {
                ctrl_held = value > 0;
                if !ctrl_held {
                    // Ctrl released — check if sequence is complete
                    if sequence == [Key::KEY_T, Key::KEY_T, Key::KEY_M] {
                        tracing::info!("Hotkey Ctrl+T+T+M triggered!");
                        let _ = tx.send(IpcAction::Activate {
                            mode: ActivateMode::Screen,
                        });
                    }
                    sequence.clear();
                }
                continue;
            }

            if value == 1 && ctrl_held {
                // Key pressed while Ctrl is held
                let now = Instant::now();
                if now.duration_since(last_key_time) > SEQUENCE_TIMEOUT {
                    sequence.clear();
                }
                last_key_time = now;

                match key {
                    Key::KEY_T | Key::KEY_M => {
                        sequence.push(key);
                        // Cap sequence length to avoid unbounded growth
                        if sequence.len() > 4 {
                            sequence.drain(..sequence.len() - 4);
                        }
                    }
                    _ => {
                        // Any other key resets the sequence
                        sequence.clear();
                    }
                }
            }
        }
    }
}
