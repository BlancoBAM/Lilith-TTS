// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod i18n;
mod ipc_client;

fn main() -> cosmic::iced::Result {
    // Initialise logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("lilith_tts=info".parse().unwrap())
                .add_directive("tts_core=info".parse().unwrap()),
        )
        .init();

    // Get the system's preferred languages and initialise i18n.
    // DesktopLanguageRequester::requested_languages() returns Vec<LanguageIdentifier>.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    // Start the COSMIC applet event loop.
    // The framework handles panel button rendering and popup window anchoring.
    cosmic::applet::run::<app::LilithApplet>(())
}
