use anyhow::{Context, Result};
use arboard::Clipboard;

use super::ExtractedContent;

/// Read the current clipboard contents as text.
pub fn read_clipboard() -> Result<ExtractedContent> {
    let mut clipboard = Clipboard::new().context("Opening clipboard")?;
    let text = clipboard.get_text().context("Reading clipboard text")?;

    if text.trim().is_empty() {
        anyhow::bail!("Clipboard is empty");
    }

    Ok(ExtractedContent::new(text, "Clipboard"))
}
