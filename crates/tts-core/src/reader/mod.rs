pub mod atspi_reader;
pub mod clipboard;
pub mod web;

use anyhow::Result;

/// Extracted readable text with metadata
#[derive(Debug, Clone, Default)]
pub struct ExtractedContent {
    pub text: String,
    /// Source description for UI display
    pub source: String,
    /// Estimated word count
    pub word_count: usize,
}

impl ExtractedContent {
    pub fn new(text: impl Into<String>, source: impl Into<String>) -> Self {
        let text = text.into();
        let word_count = text.split_whitespace().count();
        Self {
            text,
            source: source.into(),
            word_count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Roles we always skip when extracting screen content
pub const SKIP_ROLES: &[&str] = &[
    "menu_bar",
    "menu",
    "menu_item",
    "tool_bar",
    "status_bar",
    "scroll_bar",
    "separator",
    "combo_box",
    "push_button",
    "toggle_button",
    "check_box",
    "radio_button",
    "slider",
    "spin_button",
    "progress_bar",
    "table_column_header",
    "page_tab_list",
    "page_tab",
    "tree_item", // sidebar items
    "unknown",
];

/// Roles that carry readable content
pub const CONTENT_ROLES: &[&str] = &[
    "paragraph",
    "heading",
    "text",
    "label", // only if not in toolbar/nav
    "list",
    "list_item",
    "block_quote",
    "document_web",
    "document",
    "article",
    "section",
    "static_text",
];
