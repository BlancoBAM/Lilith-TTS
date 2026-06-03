// SPDX-License-Identifier: GPL-3.0-only

//! Lilith TTS — COSMIC panel applet.
//!
//! Task<M> = iced::Task<cosmic::Action<M>> so Task::perform closures must
//! produce cosmic::Action<Message>, achieved via `.into()` (From<M> for Action<M>).

use std::path::PathBuf;

use cosmic::{
    app::Task,
    cosmic_config::{self, CosmicConfigEntry},
    iced::{window::Id, Alignment, Length, Limits, Subscription},
    prelude::*,
    widget,
};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};

use tts_core::{
    config::{ProviderConfig, ReadMode, TtsConfig},
    ipc::{EngineStatus, IpcAction},
    voices::Voice,
};

use crate::config::Config;

// ─── App ID ──────────────────────────────────────────────────────────────────

pub const APP_ID: &str = "io.lilith.LilithTts";

// ─── Colours ─────────────────────────────────────────────────────────────────

fn crimson() -> cosmic::iced::Color { cosmic::iced::Color::from_rgb(1.0, 0.20, 0.00) }
fn ember()   -> cosmic::iced::Color { cosmic::iced::Color::from_rgb(1.0, 0.40, 0.20) }
fn muted()   -> cosmic::iced::Color { cosmic::iced::Color::from_rgb(0.53, 0.40, 0.33) }
fn success() -> cosmic::iced::Color { cosmic::iced::Color::from_rgb(0.40, 0.90, 0.40) }

// Build a colored text element using widget::text().class()
fn ctext<'a>(content: impl Into<String>, color: cosmic::iced::Color) -> Element<'a, Message> {
    widget::text(content.into())
        .class(cosmic::theme::Text::Color(color))
        .into()
}

// ─── Pages ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Page { Main, Settings }

// ─── ComboBox state ───────────────────────────────────────────────────────────

pub struct VoiceState(widget::combo_box::State<String>);
impl VoiceState {
    fn new(voices: &[Voice]) -> Self {
        Self(widget::combo_box::State::new(
            voices.iter().map(|v| v.display_name.clone()).collect(),
        ))
    }
}

pub struct ProviderState(widget::combo_box::State<String>);
impl ProviderState {
    fn new() -> Self {
        Self(widget::combo_box::State::new(vec![
            "NeuTTS Nano".into(), "espeak-ng".into(), "Piper TTS".into(), "Crane-OAI".into(),
        ]))
    }
}

// ─── Messages ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    SetPage(Page),
    UpdateConfig(Config),

    SetMode(ReadMode),
    VoiceSelected(String),
    VoiceInputChanged(String),
    SpeedChanged(f32),
    PitchChanged(f32),
    TextInputChanged(String),
    Speak,
    Pause,
    Resume,
    Stop,
    StatusUpdated(EngineStatus),
    VoicesLoaded(Vec<Voice>),

    OpenAddVoiceDialog,
    VoiceFileChosen(Option<PathBuf>),
    VoiceNameChanged(String),
    ConfirmAddVoice,
    CancelAddVoice,

    ProviderSelected(String),
    ProviderInputChanged(String),
    ModelPathChanged(String),
    BrowseModelPath,
    ModelPathChosen(Option<PathBuf>),
    DownloadModel,
    DownloadFinished(bool),
    ModelExistsResult(bool),
}

// ─── State ───────────────────────────────────────────────────────────────────

pub struct LilithApplet {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    tts_config: TtsConfig,
    page: Page,
    voices: Vec<Voice>,
    voice_state: VoiceState,
    voice_input: String,
    provider_state: ProviderState,
    speed: f32,
    pitch: f32,
    mode: ReadMode,
    text_input: String,
    status: EngineStatus,
    progress: f32,
    adding_voice: bool,
    new_voice_name: String,
    new_voice_path: Option<PathBuf>,
    model_path_input: String,
    model_exists: bool,
    downloading: bool,
    download_progress: u8,
    selected_provider: String,
}

impl Default for LilithApplet {
    fn default() -> Self {
        let tts_config = TtsConfig::load().unwrap_or_default();
        let model_path_input = match &tts_config.provider {
            ProviderConfig::NeuTts { model_path } |
            ProviderConfig::Piper { model_path, .. } => model_path.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        let selected_provider = match &tts_config.provider {
            ProviderConfig::NeuTts { .. } => "NeuTTS Nano",
            ProviderConfig::Espeak { .. } => "espeak-ng",
            ProviderConfig::Piper { .. }  => "Piper TTS",
            ProviderConfig::Crane { .. }  => "Crane-OAI",
        }.to_string();
        let default_voices = vec![Voice::builtin("default", "Default")];
        let voice_state = VoiceState::new(&default_voices);
        Self {
            core: cosmic::Core::default(),
            popup: None,
            config: Config::default(),
            speed: tts_config.speed,
            pitch: tts_config.pitch,
            mode: tts_config.default_mode.clone(),
            model_path_input,
            selected_provider,
            tts_config,
            page: Page::Main,
            voice_state,
            voices: default_voices,
            voice_input: String::new(),
            provider_state: ProviderState::new(),
            text_input: String::new(),
            status: EngineStatus::Idle,
            progress: 0.0,
            adding_voice: false,
            new_voice_name: String::new(),
            new_voice_path: None,
            model_exists: false,
            downloading: false,
            download_progress: 0,
        }
    }
}

// ─── cosmic::Application ─────────────────────────────────────────────────────

impl cosmic::Application for LilithApplet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core { &self.core }
    fn core_mut(&mut self) -> &mut cosmic::Core { &mut self.core }

    // Task<M> = iced::Task<Action<M>>; closures must produce Action<M> via .into()
    fn init(core: cosmic::Core, _flags: ()) -> (Self, Task<Self::Message>) {
        let mut app = Self::default();
        app.core = core;
        app.config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|ctx| match Config::get_entry(&ctx) {
                Ok(c)      => c,
                Err((_, c)) => c,
            })
            .unwrap_or_default();

        let path = app.model_path_input.clone();
        let check = Task::perform(
            async move { PathBuf::from(&path).exists() },
            |x| Message::ModelExistsResult(x).into(),
        );
        (app, check)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core.applet
            .icon_button("io.lilith.LilithTts-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content: Element<'_, Message> = match self.page {
            Page::Main     => self.view_main(),
            Page::Settings => self.view_settings(),
        };
        self.core.applet.popup_container(content).into()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            // ── Popup ──────────────────────────────────────────────────────────
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut s = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(), new_id, None, None, None,
                    );
                    s.positioner.size_limits = Limits::NONE
                        .max_width(380.0).min_width(320.0)
                        .min_height(200.0).max_height(640.0);
                    let path = self.model_path_input.clone();
                    let check = Task::perform(
                        async move { PathBuf::from(&path).exists() },
                        |x| Message::ModelExistsResult(x).into(),
                    );
                    Task::batch(vec![get_popup(s), check])
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) { self.popup = None; }
            }
            Message::SetPage(p) => self.page = p,
            Message::UpdateConfig(c) => self.config = c,

            // ── TTS Controls ───────────────────────────────────────────────────
            Message::SetMode(mode) => {
                self.mode = mode.clone();
                self.tts_config.default_mode = mode;
                let _ = self.tts_config.save();
            }
            Message::SpeedChanged(v) => {
                self.speed = (v * 10.0).round() / 10.0;
                self.tts_config.speed = self.speed;
                let _ = self.tts_config.save();
            }
            Message::PitchChanged(v) => {
                self.pitch = (v * 10.0).round() / 10.0;
                self.tts_config.pitch = self.pitch;
                let _ = self.tts_config.save();
            }
            Message::TextInputChanged(s) => self.text_input = s,
            Message::VoiceSelected(name) => {
                let id = self.voices.iter().find(|v| v.display_name == name)
                    .map(|v| v.id.clone())
                    .unwrap_or_else(|| name.to_lowercase().replace(' ', "_"));
                self.tts_config.active_voice = id;
                let _ = self.tts_config.save();
            }
            Message::VoiceInputChanged(s) => self.voice_input = s,

            Message::Speak => {
                let action = match &self.mode {
                    ReadMode::Manual if !self.text_input.is_empty() => IpcAction::Speak {
                        text: self.text_input.clone(),
                        speed: self.speed,
                        pitch: self.pitch,
                        voice_id: self.tts_config.active_voice.clone(),
                    },
                    ReadMode::Screen    => IpcAction::ReadScreen,
                    ReadMode::Clipboard => IpcAction::ReadClipboard,
                    ReadMode::Selection => IpcAction::ShowSelectionOverlay,
                    _ => return Task::none(),
                };
                self.status = EngineStatus::Reading;
                return Task::perform(
                    async move { crate::ipc_client::send(action).await },
                    |r| {
                        if let Err(e) = &r { tracing::warn!("IPC: {}", e); }
                        Message::StatusUpdated(EngineStatus::Reading).into()
                    },
                );
            }
            Message::Pause => {
                self.status = EngineStatus::Paused;
                return Task::perform(
                    async { crate::ipc_client::send(IpcAction::Pause).await },
                    |_| Message::StatusUpdated(EngineStatus::Paused).into(),
                );
            }
            Message::Resume => {
                self.status = EngineStatus::Reading;
                return Task::perform(
                    async { crate::ipc_client::send(IpcAction::Resume).await },
                    |_| Message::StatusUpdated(EngineStatus::Reading).into(),
                );
            }
            Message::Stop => {
                self.status = EngineStatus::Idle;
                self.progress = 0.0;
                return Task::perform(
                    async { crate::ipc_client::send(IpcAction::Stop).await },
                    |_| Message::StatusUpdated(EngineStatus::Idle).into(),
                );
            }
            Message::StatusUpdated(s) => self.status = s,
            Message::VoicesLoaded(v) => {
                self.voice_state = VoiceState::new(&v);
                self.voices = v;
            }

            // ── Voice cloning ──────────────────────────────────────────────────
            Message::OpenAddVoiceDialog => {
                self.adding_voice = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                            .set_title("Select Voice Reference (3–15 s)")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |x| Message::VoiceFileChosen(x).into(),
                );
            }
            Message::VoiceFileChosen(p) => self.new_voice_path = p,
            Message::VoiceNameChanged(n) => self.new_voice_name = n,
            Message::CancelAddVoice => {
                self.adding_voice = false;
                self.new_voice_name.clear();
                self.new_voice_path = None;
            }
            Message::ConfirmAddVoice => {
                if let Some(path) = self.new_voice_path.take() {
                    let name = std::mem::take(&mut self.new_voice_name);
                    self.adding_voice = false;
                    return Task::perform(
                        async move {
                            let mut vm = tts_core::voices::VoiceManager::load()?;
                            vm.add_cloned_voice(&name, &path)?;
                            Ok::<Vec<Voice>, anyhow::Error>(vm.voices)
                        },
                        |r| Message::VoicesLoaded(r.unwrap_or_else(|e| {
                            tracing::error!("Clone voice: {}", e);
                            vec![Voice::builtin("default", "Default")]
                        })).into(),
                    );
                }
            }

            // ── Settings ──────────────────────────────────────────────────────
            Message::ProviderSelected(name) => {
                self.selected_provider = name.clone();
                self.tts_config.provider = match name.as_str() {
                    "espeak-ng"  => ProviderConfig::Espeak { voice: "en".into() },
                    "Piper TTS"  => ProviderConfig::Piper {
                        model_path: PathBuf::from(&self.model_path_input),
                        piper_bin: PathBuf::from("piper"),
                    },
                    "Crane-OAI"  => ProviderConfig::Crane {
                        base_url: "http://localhost:8080".into(),
                        model: "qwen3-tts".into(),
                    },
                    _ => ProviderConfig::NeuTts {
                        model_path: PathBuf::from(&self.model_path_input),
                    },
                };
                let _ = self.tts_config.save();
            }
            Message::ProviderInputChanged(_) => {}

            Message::ModelPathChanged(s) => {
                let path = PathBuf::from(&s);
                self.model_path_input = s;
                match &mut self.tts_config.provider {
                    ProviderConfig::NeuTts  { model_path }        => *model_path = path.clone(),
                    ProviderConfig::Piper   { model_path, .. }    => *model_path = path.clone(),
                    _ => {}
                }
                let _ = self.tts_config.save();
                return Task::perform(
                    async move { path.exists() },
                    |x| Message::ModelExistsResult(x).into(),
                );
            }
            Message::BrowseModelPath => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Model file", &["gguf", "bin"])
                            .set_title("Select TTS model file")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |x| Message::ModelPathChosen(x).into(),
                );
            }
            Message::ModelPathChosen(Some(path)) => {
                self.model_exists = path.exists();
                self.model_path_input = path.to_string_lossy().into_owned();
                match &mut self.tts_config.provider {
                    ProviderConfig::NeuTts  { model_path }     => *model_path = path,
                    ProviderConfig::Piper   { model_path, .. } => *model_path = path,
                    _ => {}
                }
                let _ = self.tts_config.save();
            }
            Message::ModelPathChosen(None) => {}

            Message::DownloadModel => {
                self.downloading = true;
                self.download_progress = 0;
                return Task::perform(
                    async { download_neutts_model().await },
                    |x| Message::DownloadFinished(x).into(),
                );
            }
            Message::DownloadFinished(ok) => {
                self.downloading = false;
                if ok {
                    self.model_exists = true;
                    self.download_progress = 100;
                    let dest = "/var/lib/lilith/models/neutts-nano-q4.gguf";
                    self.model_path_input = dest.into();
                    if let ProviderConfig::NeuTts { model_path } = &mut self.tts_config.provider {
                        *model_path = PathBuf::from(dest);
                    }
                    let _ = self.tts_config.save();
                }
            }
            Message::ModelExistsResult(e) => self.model_exists = e,
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|u| Message::UpdateConfig(u.config))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

// ─── Main page ────────────────────────────────────────────────────────────────

impl LilithApplet {
    fn view_main(&self) -> Element<'_, Message> {
        // Header
        let title = ctext("LILITH TTS", crimson());
        let status_col = match &self.status {
            EngineStatus::Idle    => muted(),
            EngineStatus::Reading => ember(),
            EngineStatus::Paused  => crimson(),
            EngineStatus::Error(_)=> cosmic::iced::Color::from_rgb(1.0, 0.3, 0.3),
        };
        let status_txt = widget::text(self.status_label())
            .size(11)
            .class(cosmic::theme::Text::Color(status_col));
        let settings_btn = widget::button::icon(
            widget::icon::from_name("preferences-system-symbolic"),
        ).on_press(Message::SetPage(Page::Settings));

        let header = widget::row::with_children(vec![
            title,
            widget::Space::new().width(Length::Fill).into(),
            status_txt.into(),
            settings_btn.into(),
        ])
        .align_y(Alignment::Center)
        .spacing(8);

        // Model banner
        let uses_neutts = matches!(self.tts_config.provider, ProviderConfig::NeuTts { .. });
        let model_banner: Option<Element<'_, Message>> = if self.downloading {
            Some(
                widget::text(format!("Downloading model… {}%", self.download_progress))
                    .size(12)
                    .class(cosmic::theme::Text::Color(ember()))
                    .into(),
            )
        } else if uses_neutts && !self.model_exists {
            Some(
                widget::row::with_children(vec![
                    widget::text("NeuTTS model missing")
                        .size(12)
                        .class(cosmic::theme::Text::Color(ember()))
                        .into(),
                    widget::button::text("Download").on_press(Message::DownloadModel).into(),
                    widget::button::text("Settings").on_press(Message::SetPage(Page::Settings)).into(),
                ])
                .spacing(4).align_y(Alignment::Center).into(),
            )
        } else { None };

        // Voice selector
        let active_name: Option<String> = self.voices.iter()
            .find(|v| v.id == self.tts_config.active_voice)
            .map(|v| v.display_name.clone());
        let voice_cb = widget::combo_box(
            &self.voice_state.0,
            "Select voice…",
            active_name.as_ref(),
            Message::VoiceSelected,
        )
        .on_input(Message::VoiceInputChanged)
        .width(Length::Fill);

        let voice_row = widget::row::with_children(vec![
            voice_cb.into(),
            widget::button::text("+ Voice").on_press(Message::OpenAddVoiceDialog).into(),
        ])
        .spacing(8).align_y(Alignment::Center);

        // Sliders
        let speed_row = slider_row("Speed", 42.0, self.speed, 0.5, 3.0,
            |v| Message::SpeedChanged(v), format!("{:.1}×", self.speed));
        let pitch_row = slider_row("Pitch", 42.0, self.pitch, 0.5, 2.0,
            |v| Message::PitchChanged(v), format!("{:.1}×", self.pitch));

        // Mode tabs
        let modes = widget::row::with_children(vec![
            mode_tab("Screen",    ReadMode::Screen,    &self.mode),
            mode_tab("Select",    ReadMode::Selection, &self.mode),
            mode_tab("Clipboard", ReadMode::Clipboard, &self.mode),
            mode_tab("Type",      ReadMode::Manual,    &self.mode),
        ]).spacing(4);

        // Progress bar
        let progress_val = if self.status == EngineStatus::Reading { self.progress } else { 0.0 };
        let pbar = widget::determinate_linear(progress_val)
            .width(Length::Fill)
            .girth(Length::Fixed(6.0));

        // Transport
        let speak_btn: Element<'_, Message> = match self.status {
            EngineStatus::Reading => widget::button::suggested("Pause").on_press(Message::Pause).into(),
            EngineStatus::Paused  => widget::button::suggested("Resume").on_press(Message::Resume).into(),
            _                     => widget::button::destructive("SPEAK").on_press(Message::Speak).into(),
        };
        let transport = widget::row::with_children(vec![
            widget::button::standard("Stop").on_press(Message::Stop).into(),
            widget::Space::new().width(Length::Fill).into(),
            speak_btn,
        ]).align_y(Alignment::Center);

        // Manual text input
        let manual_input: Option<Element<'_, Message>> = if matches!(self.mode, ReadMode::Manual) {
            Some(widget::column::with_children(vec![
                widget::divider::horizontal::default().into(),
                widget::text_input("Enter text to speak…", &self.text_input)
                    .on_input(Message::TextInputChanged).size(13).into(),
            ]).spacing(6).into())
        } else { None };

        // Add-voice dialog
        let add_voice_dialog: Option<Element<'_, Message>> = if self.adding_voice {
            let file_label = self.new_voice_path.as_ref()
                .and_then(|p| p.file_name()).and_then(|n| n.to_str())
                .unwrap_or("No file chosen");
            Some(widget::column::with_children(vec![
                widget::text("Clone a Voice").size(13)
                    .class(cosmic::theme::Text::Color(ember())).into(),
                widget::text(format!("Ref: {}", file_label)).size(11)
                    .class(cosmic::theme::Text::Color(muted())).into(),
                widget::text_input("Voice name…", &self.new_voice_name)
                    .on_input(Message::VoiceNameChanged).size(13).into(),
                widget::row::with_children(vec![
                    widget::button::standard("Browse").on_press(Message::OpenAddVoiceDialog).into(),
                    widget::Space::new().width(Length::Fill).into(),
                    widget::button::standard("Cancel").on_press(Message::CancelAddVoice).into(),
                    widget::button::destructive("Clone").on_press(Message::ConfirmAddVoice).into(),
                ]).spacing(8).into(),
            ]).spacing(6).into())
        } else { None };

        // Footer
        let footer = widget::text("Global hotkey: Ctrl + T + T + M")
            .size(10)
            .class(cosmic::theme::Text::Color(muted()));

        // Assemble
        let mut children: Vec<Element<'_, Message>> = vec![
            header.into(),
            widget::divider::horizontal::default().into(),
            voice_row.into(),
            speed_row,
            pitch_row,
            widget::divider::horizontal::default().into(),
            modes.into(),
            pbar.into(),
            transport.into(),
        ];
        if let Some(b) = model_banner   { children.push(b); }
        if let Some(i) = manual_input   { children.push(i); }
        if let Some(d) = add_voice_dialog { children.push(d); }
        children.push(widget::Space::new().height(Length::Fixed(4.0)).into());
        children.push(footer.into());

        widget::column::with_children(children).spacing(8).padding(14).into()
    }

    fn status_label(&self) -> &'static str {
        match self.status {
            EngineStatus::Idle     => "Idle",
            EngineStatus::Reading  => "Reading",
            EngineStatus::Paused   => "Paused",
            EngineStatus::Error(_) => "Error",
        }
    }

    // ─── Settings page ────────────────────────────────────────────────────────

    fn view_settings(&self) -> Element<'_, Message> {
        let back_btn = widget::button::icon(
            widget::icon::from_name("go-previous-symbolic"),
        ).on_press(Message::SetPage(Page::Main));

        let header = widget::row::with_children(vec![
            back_btn.into(),
            ctext("Settings", crimson()),
        ]).spacing(8).align_y(Alignment::Center);

        // Engine picker
        let engine_cb = widget::combo_box(
            &self.provider_state.0,
            "Select engine…",
            Some(&self.selected_provider),
            Message::ProviderSelected,
        )
        .on_input(Message::ProviderInputChanged)
        .width(Length::Fill);

        // Model path row
        let model_row = widget::row::with_children(vec![
            widget::text_input(
                "/var/lib/lilith/models/neutts-nano-q4.gguf",
                &self.model_path_input,
            )
            .on_input(Message::ModelPathChanged).size(12).width(Length::Fill).into(),
            widget::button::standard("…").on_press(Message::BrowseModelPath).into(),
        ]).spacing(6).align_y(Alignment::Center);

        // Model status
        let model_status: Element<'_, Message> = if self.downloading {
            widget::text(format!("Downloading… {}%", self.download_progress))
                .size(12).class(cosmic::theme::Text::Color(ember())).into()
        } else if self.model_exists {
            widget::text("Model found ✓")
                .size(12).class(cosmic::theme::Text::Color(success())).into()
        } else {
            widget::column::with_children(vec![
                widget::text("Model not found at path above.")
                    .size(12).class(cosmic::theme::Text::Color(ember())).into(),
                widget::button::destructive("Download NeuTTS Nano (~120 MB)")
                    .on_press(Message::DownloadModel).into(),
            ]).spacing(4).into()
        };

        // Hotkey info
        let hotkey_section = widget::column::with_children(vec![
            ctext("Global Hotkey", crimson()),
            widget::text("Ctrl + T + T + M").size(14).into(),
            widget::text("Works on X11 and Wayland via /dev/input/*.")
                .size(11).class(cosmic::theme::Text::Color(muted())).into(),
            widget::divider::horizontal::default().into(),
            widget::text("Requires membership in the 'input' group:")
                .size(11).class(cosmic::theme::Text::Color(muted())).into(),
            widget::text("sudo usermod -aG input $USER && newgrp input")
                .size(10).class(cosmic::theme::Text::Color(ember())).into(),
        ]).spacing(4);

        widget::column::with_children(vec![
            header.into(),
            widget::divider::horizontal::default().into(),
            widget::text("Engine").size(12)
                .class(cosmic::theme::Text::Color(muted())).into(),
            engine_cb.into(),
            widget::text("Model path").size(12)
                .class(cosmic::theme::Text::Color(muted())).into(),
            model_row.into(),
            model_status,
            widget::divider::horizontal::default().into(),
            hotkey_section.into(),
        ])
        .spacing(8).padding(14).into()
    }
}

// ─── Widget helpers ───────────────────────────────────────────────────────────

fn slider_row<'a, F>(
    label: &'static str,
    label_w: f32,
    value: f32,
    min: f32,
    max: f32,
    on_change: F,
    value_label: String,
) -> Element<'a, Message>
where
    F: Fn(f32) -> Message + 'a,
{
    widget::row::with_children(vec![
        widget::text(label).size(12).width(Length::Fixed(label_w))
            .class(cosmic::theme::Text::Color(muted())).into(),
        widget::slider(min..=max, value, on_change).step(0.1_f32).width(Length::Fill).into(),
        widget::text(value_label).size(12).width(Length::Fixed(34.0)).into(),
    ])
    .spacing(8).align_y(Alignment::Center).into()
}

fn mode_tab(label: &'static str, mode: ReadMode, current: &ReadMode) -> Element<'static, Message> {
    let active = std::mem::discriminant(&mode) == std::mem::discriminant(current);
    if active {
        widget::button::destructive(label).on_press(Message::SetMode(mode)).into()
    } else {
        widget::button::standard(label).on_press(Message::SetMode(mode)).into()
    }
}

// ─── Model download ───────────────────────────────────────────────────────────

async fn download_neutts_model() -> bool {
    let dest_dir = PathBuf::from("/var/lib/lilith/models");
    let dest_file = dest_dir.join("neutts-nano-q4.gguf");

    if tokio::fs::create_dir_all(&dest_dir).await.is_ok()
        && run_download(&dest_file).await { return true; }

    // Fallback: user-local directory
    let local_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("lilith-tts").join("models");
    let local_file = local_dir.join("neutts-nano-q4.gguf");
    tokio::fs::create_dir_all(&local_dir).await.is_ok() && run_download(&local_file).await
}

async fn run_download(dest: &PathBuf) -> bool {
    let dest_str = dest.to_string_lossy().into_owned();
    let dest_dir = dest.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into());

    // Try huggingface-cli first (handles auth and resuming)
    if let Ok(s) = tokio::process::Command::new("huggingface-cli")
        .args(["download", "neuphonic/neutts-nano-q4", "neutts-nano-q4.gguf",
               "--local-dir", &dest_dir])
        .status().await
    {
        if s.success() {
            tracing::info!("NeuTTS downloaded via huggingface-cli → {}", dest_dir);
            return true;
        }
    }

    // Fall back to curl
    let url = "https://huggingface.co/neuphonic/neutts-nano-q4/resolve/main/neutts-nano-q4.gguf";
    if let Ok(s) = tokio::process::Command::new("curl")
        .args(["-L", "--progress-bar", "-o", &dest_str, url])
        .status().await
    {
        if s.success() {
            tracing::info!("NeuTTS downloaded via curl → {}", dest_str);
            return true;
        }
    }

    tracing::error!("Failed to download NeuTTS Nano model");
    false
}
