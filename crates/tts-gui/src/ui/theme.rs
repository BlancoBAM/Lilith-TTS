// SPDX-License-Identifier: GPL-3.0-only

// ─── Lilith TTS Colour Palette ──────────────────────────────────────────────
// Kept here as named constants that can be referenced from app.rs.
// COSMIC applets use the system theme for most widgets; these colours are used
// for custom-styled containers and text where brand identity matters.

use cosmic::iced::Color;

/// Window/popup background — near-black with faint crimson undertone
pub const BG_DEEP: Color = Color {
    r: 0.027,
    g: 0.012,
    b: 0.012,
    a: 1.0,
};

/// Card / section surface
pub const BG_SURFACE: Color = Color {
    r: 0.065,
    g: 0.031,
    b: 0.031,
    a: 1.0,
};

/// Input field / elevated surface
pub const BG_ELEVATED: Color = Color {
    r: 0.102,
    g: 0.039,
    b: 0.039,
    a: 1.0,
};

/// Primary crimson — buttons, active elements
pub const CRIMSON: Color = Color {
    r: 0.800,
    g: 0.102,
    b: 0.000,
    a: 1.0,
};

/// Bright crimson — hover, header text, active waveform peak
pub const CRIMSON_BRIGHT: Color = Color {
    r: 1.000,
    g: 0.200,
    b: 0.000,
    a: 1.0,
};

/// Ember / flame accent — waveform fill, progress, highlights
pub const EMBER: Color = Color {
    r: 1.000,
    g: 0.400,
    b: 0.200,
    a: 1.0,
};

/// Primary text — warm off-white
pub const TEXT_PRIMARY: Color = Color {
    r: 0.910,
    g: 0.831,
    b: 0.769,
    a: 1.0,
};

/// Secondary / muted text — copper-grey
pub const TEXT_MUTED: Color = Color {
    r: 0.533,
    g: 0.400,
    b: 0.333,
    a: 1.0,
};

/// Panel border — dark red
pub const BORDER_COLOR: Color = Color {
    r: 0.239,
    g: 0.067,
    b: 0.031,
    a: 1.0,
};
