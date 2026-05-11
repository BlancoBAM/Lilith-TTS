use iced::widget::{
    button, column, container, horizontal_rule, pick_list, row, slider, text, text_input,
    vertical_space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Task};

use tts_core::{
    config::{ReadMode, TtsConfig},
    ipc::{EngineStatus, IpcAction},
    voices::Voice,
};

use crate::ui::theme::{
    BG_DEEP, BG_ELEVATED, BG_SURFACE, BORDER, CRIMSON, CRIMSON_BRIGHT, EMBER, TEXT_MUTED,
    TEXT_PRIMARY,
};

// ─── Messages ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    Speak,
    Pause,
    Resume,
    Stop,
    SetMode(ReadMode),
    VoiceSelected(String),
    OpenAddVoiceDialog,
    VoiceFileChosen(Option<std::path::PathBuf>),
    VoiceNameChanged(String),
    ConfirmAddVoice,
    SpeedChanged(f32),
    PitchChanged(f32),
    TextInputChanged(String),
    StatusUpdated(EngineStatus),
    VoicesLoaded(Vec<Voice>),
}

// ─── App State ──────────────────────────────────────────────────────────────

pub struct LilithTtsApp {
    config: TtsConfig,
    voices: Vec<Voice>,
    speed: f32,
    pitch: f32,
    mode: ReadMode,
    text_input: String,
    status: EngineStatus,
    progress: f32,
    adding_voice: bool,
    new_voice_name: String,
    new_voice_path: Option<std::path::PathBuf>,
}

impl Default for LilithTtsApp {
    fn default() -> Self {
        let config = TtsConfig::load().unwrap_or_default();
        Self {
            speed: config.speed,
            pitch: config.pitch,
            mode: config.default_mode.clone(),
            voices: vec![Voice::builtin("default", "Default")],
            config,
            text_input: String::new(),
            status: EngineStatus::Idle,
            progress: 0.0,
            adding_voice: false,
            new_voice_name: String::new(),
            new_voice_path: None,
        }
    }
}

impl LilithTtsApp {
    pub fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetMode(mode) => self.mode = mode,

            Message::SpeedChanged(v) => {
                self.speed = (v * 10.0).round() / 10.0;
                self.config.speed = self.speed;
                let _ = self.config.save();
            }

            Message::PitchChanged(v) => {
                self.pitch = (v * 10.0).round() / 10.0;
                self.config.pitch = self.pitch;
                let _ = self.config.save();
            }

            Message::TextInputChanged(s) => self.text_input = s,

            Message::VoiceSelected(name) => {
                // Map display name back to id
                let id = self
                    .voices
                    .iter()
                    .find(|v| v.display_name == name)
                    .map(|v| v.id.clone())
                    .unwrap_or_else(|| name.to_lowercase().replace(' ', "_"));
                self.config.active_voice = id;
                let _ = self.config.save();
            }

            Message::Speak => {
                self.status = EngineStatus::Reading;
                let action = match self.mode {
                    ReadMode::Manual if !self.text_input.is_empty() => IpcAction::Speak {
                        text: self.text_input.clone(),
                        speed: self.speed,
                        pitch: self.pitch,
                        voice_id: self.config.active_voice.clone(),
                    },
                    ReadMode::Screen => IpcAction::ReadScreen,
                    ReadMode::Clipboard => IpcAction::ReadClipboard,
                    ReadMode::Selection => IpcAction::ShowSelectionOverlay,
                    _ => return Task::none(),
                };
                return Task::perform(async move { crate::ipc_client::send(action).await }, |_| {
                    Message::StatusUpdated(EngineStatus::Reading)
                });
            }

            Message::Pause => {
                self.status = EngineStatus::Paused;
                let _ = tokio::spawn(crate::ipc_client::send(IpcAction::Pause));
            }

            Message::Resume => {
                self.status = EngineStatus::Reading;
                let _ = tokio::spawn(crate::ipc_client::send(IpcAction::Resume));
            }

            Message::Stop => {
                self.status = EngineStatus::Idle;
                self.progress = 0.0;
                let _ = tokio::spawn(crate::ipc_client::send(IpcAction::Stop));
            }

            Message::StatusUpdated(s) => self.status = s,

            Message::VoicesLoaded(voices) => self.voices = voices,

            Message::OpenAddVoiceDialog => {
                self.adding_voice = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                            .set_title("Select Voice Reference (3–15 seconds)")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    Message::VoiceFileChosen,
                );
            }

            Message::VoiceFileChosen(path) => self.new_voice_path = path,

            Message::VoiceNameChanged(name) => self.new_voice_name = name,

            Message::ConfirmAddVoice => {
                if let (Some(path), name) = (
                    self.new_voice_path.take(),
                    std::mem::take(&mut self.new_voice_name),
                ) {
                    self.adding_voice = false;
                    return Task::perform(
                        async move {
                            let mut vm = tts_core::voices::VoiceManager::load()?;
                            vm.add_cloned_voice(&name, &path)?;
                            Ok::<Vec<Voice>, anyhow::Error>(vm.voices)
                        },
                        |r| Message::VoicesLoaded(r.unwrap_or_default()),
                    );
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        // ── Header ───────────────────────────────────────────────────────────
        let header = row![
            text("🌙  LILITH TTS").size(17).color(CRIMSON_BRIGHT),
            iced::widget::Space::with_width(Length::Fill),
            status_indicator(&self.status),
        ]
        .align_y(Alignment::Center);

        // ── Voice selector ───────────────────────────────────────────────────
        let voice_names: Vec<String> = self.voices.iter().map(|v| v.display_name.clone()).collect();
        let active_name = self
            .voices
            .iter()
            .find(|v| v.id == self.config.active_voice)
            .map(|v| v.display_name.clone());

        let voice_row = row![
            pick_list(voice_names, active_name, Message::VoiceSelected)
                .width(Length::Fill)
                .placeholder("Select voice..."),
            button(text("＋ Voice").size(12))
                .on_press(Message::OpenAddVoiceDialog)
                .style(btn_secondary()),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // ── Sliders ──────────────────────────────────────────────────────────
        let speed_row = row![
            text("Speed").size(12).color(TEXT_MUTED).width(44),
            slider(0.5..=3.0, self.speed, Message::SpeedChanged)
                .step(0.1)
                .width(Length::Fill),
            text(format!("{:.1}×", self.speed))
                .size(12)
                .color(TEXT_PRIMARY)
                .width(34),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let pitch_row = row![
            text("Pitch").size(12).color(TEXT_MUTED).width(44),
            slider(0.5..=2.0, self.pitch, Message::PitchChanged)
                .step(0.1)
                .width(Length::Fill),
            text(format!("{:.1}×", self.pitch))
                .size(12)
                .color(TEXT_PRIMARY)
                .width(34),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // ── Mode tabs ────────────────────────────────────────────────────────
        let modes = row![
            mode_btn("Screen", ReadMode::Screen, &self.mode),
            mode_btn("Select", ReadMode::Selection, &self.mode),
            mode_btn("📋 Clip", ReadMode::Clipboard, &self.mode),
            mode_btn("Type", ReadMode::Manual, &self.mode),
        ]
        .spacing(4);

        // ── Waveform / progress bar ───────────────────────────────────────────
        let bar_w = (self.progress * 310.0).clamp(0.0, 310.0);
        let bar_color = if self.status == EngineStatus::Reading {
            EMBER
        } else {
            BORDER
        };

        let wave_bar = container(
            row![
                text("🔊").size(13).color(CRIMSON_BRIGHT),
                iced::widget::Space::with_width(4),
                container(iced::widget::Space::with_width(bar_w as u16))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(bar_color)),
                        border: Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .height(8),
            ]
            .align_y(Alignment::Center),
        )
        .style(|_| container::Style {
            background: Some(Background::Color(BG_SURFACE)),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .padding(Padding::new(8.0))
        .width(Length::Fill);

        // ── Transport controls ────────────────────────────────────────────────
        let speak_btn = match self.status {
            EngineStatus::Reading => button(text("⏸  Pause").size(14))
                .on_press(Message::Pause)
                .style(btn_primary()),
            EngineStatus::Paused => button(text("▶  Resume").size(14))
                .on_press(Message::Resume)
                .style(btn_primary()),
            _ => button(text("▶   SPEAK").size(14))
                .on_press(Message::Speak)
                .style(btn_primary()),
        };

        let transport = row![
            button(text("■ Stop").size(12))
                .on_press(Message::Stop)
                .style(btn_secondary()),
            iced::widget::Space::with_width(Length::Fill),
            speak_btn,
        ]
        .align_y(Alignment::Center);

        // ── Manual text input (only in Manual mode) ───────────────────────────
        let manual: Element<Message> = if matches!(self.mode, ReadMode::Manual) {
            column![
                horizontal_rule(1),
                text_input("Enter text to read aloud...", &self.text_input)
                    .on_input(Message::TextInputChanged)
                    .size(14),
            ]
            .spacing(8)
            .into()
        } else {
            column![].into()
        };

        // ── Add-voice dialog ──────────────────────────────────────────────────
        let add_voice: Element<Message> = if self.adding_voice {
            let file_label = self
                .new_voice_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("No file chosen");

            column![
                horizontal_rule(1),
                text("Clone a Voice").size(13).color(EMBER),
                text(format!("Reference: {}", file_label))
                    .size(11)
                    .color(TEXT_MUTED),
                text_input("Voice name...", &self.new_voice_name)
                    .on_input(Message::VoiceNameChanged)
                    .size(13),
                row![
                    button(text("Browse").size(12))
                        .on_press(Message::OpenAddVoiceDialog)
                        .style(btn_secondary()),
                    iced::widget::Space::with_width(Length::Fill),
                    button(text("Clone").size(12))
                        .on_press(Message::ConfirmAddVoice)
                        .style(btn_primary()),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .into()
        } else {
            column![].into()
        };

        // ── Footer hint ───────────────────────────────────────────────────────
        let hint = text("Ctrl + T + T + M  to activate globally")
            .size(10)
            .color(TEXT_MUTED);

        // ── Assemble ──────────────────────────────────────────────────────────
        let content = column![
            header,
            horizontal_rule(1),
            voice_row,
            speed_row,
            pitch_row,
            horizontal_rule(1),
            modes,
            wave_bar,
            transport,
            manual,
            add_voice,
            vertical_space(),
            hint,
        ]
        .spacing(10)
        .padding(Padding::new(14.0));

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_DEEP)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

// ─── Widget helpers ──────────────────────────────────────────────────────────

fn status_indicator(status: &EngineStatus) -> Element<'static, Message> {
    let (sym, col) = match status {
        EngineStatus::Idle => ("◉ Idle", TEXT_MUTED),
        EngineStatus::Reading => ("◉ Reading", EMBER),
        EngineStatus::Paused => ("◉ Paused", CRIMSON),
        EngineStatus::Error(_) => ("⚠ Error", Color::from_rgb(1.0, 0.27, 0.27)),
    };
    text(sym).size(11).color(col).into()
}

fn mode_btn(label: &'static str, mode: ReadMode, current: &ReadMode) -> Element<'static, Message> {
    let active = std::mem::discriminant(&mode) == std::mem::discriminant(current);
    button(
        text(label)
            .size(12)
            .color(if active { BG_DEEP } else { TEXT_PRIMARY }),
    )
    .on_press(Message::SetMode(mode))
    .style(btn_tab_style(active))
    .into()
}

// ─── Inline button styles ────────────────────────────────────────────────────
// iced 0.13 style closures: |theme: &Theme, status: Status| -> button::Style

// No double imports needed here

fn btn_primary() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => CRIMSON_BRIGHT,
            button::Status::Pressed => Color {
                r: CRIMSON.r * 0.8,
                g: CRIMSON.g * 0.8,
                b: CRIMSON.b * 0.8,
                a: 1.0,
            },
            _ => CRIMSON,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn btn_secondary() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => BG_ELEVATED,
            _ => BG_SURFACE,
        })),
        text_color: TEXT_PRIMARY,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn btn_tab_style(active: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        if active {
            button::Style {
                background: Some(Background::Color(CRIMSON_BRIGHT)),
                text_color: BG_DEEP,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered => BG_ELEVATED,
                    _ => BG_SURFACE,
                })),
                text_color: TEXT_MUTED,
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        }
    }
}
