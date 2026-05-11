//! Web content extraction from HTML — strips navigation, ads, footers,
//! related article links, and other non-article UI elements.

use anyhow::Result;
use scraper::{Html, Selector};

use super::ExtractedContent;

/// Tags we always strip from the DOM before reading
const STRIP_TAGS: &[&str] = &[
    "nav",
    "header",
    "footer",
    "aside",
    "script",
    "style",
    "noscript",
    "iframe",
    "form",
    "button",
    "input",
    "select",
    "textarea",
    "[role=\"navigation\"]",
    "[role=\"banner\"]",
    "[role=\"contentinfo\"]",
    "[role=\"complementary\"]",
    "[aria-hidden=\"true\"]",
    ".advertisement",
    ".ad",
    ".ads",
    ".sidebar",
    ".cookie-notice",
    ".related-articles",
    ".recommended",
    ".social-share",
];

/// Extract the main readable content from an HTML string.
/// Uses a readability-like heuristic: find the element with the highest
/// text-to-link ratio and most paragraph content.
pub fn extract_from_html(html: &str, url: &str) -> Result<ExtractedContent> {
    let document = Html::parse_document(html);

    // Try <article> first — most well-structured pages use it
    if let Ok(article_sel) = Selector::parse("article") {
        if let Some(article) = document.select(&article_sel).next() {
            let text = extract_text_from_element(article);
            if text.split_whitespace().count() > 50 {
                let source = url_to_source(url);
                return Ok(ExtractedContent::new(clean_text(&text), source));
            }
        }
    }

    // Try <main> role
    for selector_str in &[
        "main",
        "[role=\"main\"]",
        "#content",
        ".content",
        ".article-body",
        ".post-content",
        ".entry-content",
    ] {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&sel).next() {
                let text = extract_text_from_element(el);
                if text.split_whitespace().count() > 50 {
                    let source = url_to_source(url);
                    return Ok(ExtractedContent::new(clean_text(&text), source));
                }
            }
        }
    }

    // Fallback: score all divs by text density, pick the winner
    let best = score_and_pick(&document);
    let source = url_to_source(url);
    Ok(ExtractedContent::new(clean_text(&best), source))
}

/// Extract visible text from an element, respecting block structure.
fn extract_text_from_element(element: scraper::ElementRef) -> String {
    let mut parts: Vec<String> = Vec::new();

    for node in element.descendants() {
        if let Some(el) = scraper::ElementRef::wrap(node) {
            let tag = el.value().name().to_lowercase();
            // Skip non-content elements
            if matches!(
                tag.as_str(),
                "nav"
                    | "aside"
                    | "footer"
                    | "script"
                    | "style"
                    | "button"
                    | "input"
                    | "select"
                    | "form"
                    | "iframe"
                    | "noscript"
                    | "figcaption"
            ) {
                continue;
            }
            // Check aria-hidden
            if el.value().attr("aria-hidden") == Some("true") {
                continue;
            }
            // Check for ad/nav classes
            let class = el.value().attr("class").unwrap_or("");
            if is_ad_or_nav_class(class) {
                continue;
            }
        } else if let Some(text) = node.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                parts.push(t.to_string());
            }
        }
    }

    parts.join(" ")
}

fn is_ad_or_nav_class(class: &str) -> bool {
    let class = class.to_lowercase();
    [
        "ad",
        "advertisement",
        "sidebar",
        "nav",
        "navigation",
        "related",
        "recommend",
        "social",
        "share",
        "cookie",
        "popup",
        "modal",
        "banner",
        "promo",
        "sponsored",
    ]
    .iter()
    .any(|&kw| class.contains(kw))
}

/// Score divs by text density and return the best candidate's text
fn score_and_pick(document: &Html) -> String {
    if let Ok(sel) = Selector::parse("div, section") {
        let mut best_text = String::new();
        let mut best_score = 0usize;

        for el in document.select(&sel) {
            let text = extract_text_from_element(el);
            let words = text.split_whitespace().count();
            // Penalize link-heavy blocks
            let link_count = el.select(&Selector::parse("a").unwrap()).count();
            let score = words.saturating_sub(link_count * 3);
            if score > best_score {
                best_score = score;
                best_text = text;
            }
        }
        return best_text;
    }
    String::new()
}

/// Clean extracted text: collapse whitespace, remove duplicate lines
fn clean_text(text: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(|l| l.trim())
        .filter(|l| {
            if l.len() < 3 {
                return false;
            }
            // Deduplicate repeated phrases
            let key: String = l.chars().take(60).collect();
            seen.insert(key)
        })
        .collect::<Vec<_>>()
        .join("\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn url_to_source(url: &str) -> String {
    if url.is_empty() {
        return "Web page".to_string();
    }
    // Extract domain from URL for display
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}
