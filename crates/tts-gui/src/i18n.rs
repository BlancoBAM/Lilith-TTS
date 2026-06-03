// SPDX-License-Identifier: GPL-3.0-only

//! Localisation support — matches cosmic-applet-template pattern exactly.

use std::sync::LazyLock;

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    unic_langid::LanguageIdentifier,
    DefaultLocalizer, LanguageLoader, Localizer,
};
use rust_embed::RustEmbed;

/// Apply the system's requested languages to the fl!() macro.
pub fn init(requested_languages: &[LanguageIdentifier]) {
    if let Err(why) = localizer().select(requested_languages) {
        eprintln!("error while loading fluent localizations: {why}");
    }
}

/// Returns the Localizer used to localise strings from the i18n/ directory.
#[must_use]
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error loading fallback language");

    loader
});

/// Look up a localised string by its Fluent message ID.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($key:literal = $value:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($key = $value),*)
    }};
}
