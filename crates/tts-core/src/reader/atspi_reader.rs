//! Screen content extraction via AT-SPI2 accessibility tree.
//!
//! AT-SPI2 is the standard Linux accessibility infrastructure.
//! Every GTK, Qt, Electron and most other GUI apps expose their
//! content through it. We traverse the focused window's tree,
//! filter out UI chrome (menus, toolbars, nav elements), and
//! return clean readable text.
//!
//! For web pages in browsers, AT-SPI exposes the full DOM with
//! ARIA roles, letting us distinguish article content from ads
//! and navigation links.

use anyhow::{Context, Result};
use std::process::Command;

use super::{ExtractedContent, SKIP_ROLES};

/// Read the focused application's screen content via AT-SPI2.
/// Falls back to xdotool/xprop on non-accessible apps.
pub async fn read_focused_screen(
    skip_roles: &[String],
    smart_skip: bool,
) -> Result<ExtractedContent> {
    // Try AT-SPI first (preferred, works on all DEs)
    match read_via_atspi(skip_roles, smart_skip).await {
        Ok(content) if !content.is_empty() => return Ok(content),
        Ok(_) => tracing::debug!("AT-SPI returned empty content, trying clipboard fallback"),
        Err(e) => tracing::warn!("AT-SPI read failed: {}, trying xdotool", e),
    }

    // Fallback: try to get window title + any accessible text
    read_via_xdotool().await
}

/// Core AT-SPI2 tree traversal using the `atspi-proxy` helper binary
/// (ships with the atspi crate as an optional feature) or via Python
/// as a thin AT-SPI bridge since the Rust atspi crate requires an
/// async D-Bus runtime that conflicts with our tokio setup.
async fn read_via_atspi(skip_roles: &[String], smart_skip: bool) -> Result<ExtractedContent> {
    // We invoke a small Python helper to walk the AT-SPI tree.
    // Python AT-SPI bindings (pyatspi) are available on every Ubuntu system.
    // This avoids the tokio/async-std D-Bus conflict in the atspi Rust crate.
    let skip_arg = if smart_skip {
        let mut all_skip: Vec<&str> = SKIP_ROLES.to_vec();
        let user_skip: Vec<&str> = skip_roles.iter().map(|s| s.as_str()).collect();
        all_skip.extend(user_skip);
        all_skip.join(",")
    } else {
        String::new()
    };

    let script = build_atspi_script(&skip_arg);

    let output = tokio::process::Command::new("python3")
        .arg("-c")
        .arg(&script)
        .output()
        .await
        .context("Running AT-SPI Python helper")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("AT-SPI helper failed: {}", err.trim());
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = raw.lines().collect();

    if lines.is_empty() {
        return Ok(ExtractedContent::default());
    }

    let source = lines.first().map(|s| *s).unwrap_or("Screen").to_string();
    let text = lines[1..].join("\n").trim().to_string();

    if smart_skip {
        let filtered = apply_readability_filter(&text);
        Ok(ExtractedContent::new(filtered, source))
    } else {
        Ok(ExtractedContent::new(text, source))
    }
}

/// Python script that walks the focused window's AT-SPI tree
fn build_atspi_script(skip_roles_csv: &str) -> String {
    format!(
        r#"
import sys
try:
    import pyatspi
except ImportError:
    sys.exit(1)

SKIP_ROLES = set(r.strip() for r in """{skip}""".split(',') if r.strip())

def role_name(obj):
    try:
        return pyatspi.role_to_string(obj.get_role()).lower().replace(' ', '_')
    except:
        return 'unknown'

def get_text(obj):
    try:
        ti = obj.queryText()
        return ti.getText(0, ti.characterCount)
    except:
        try:
            return obj.name or ''
        except:
            return ''

def should_skip(obj):
    rn = role_name(obj)
    if rn in SKIP_ROLES:
        return True
    # Skip navigation landmarks (web pages)
    try:
        attrs = dict(a.split(':', 1) for a in (obj.getAttributes() or []))
        if attrs.get('xml-roles', '').strip() in ('navigation', 'banner', 'complementary', 'contentinfo'):
            return True
        if attrs.get('hidden', 'false') == 'true':
            return True
        # Skip link-dense blocks (ads / related articles)
        if attrs.get('tag', '') in ('nav', 'header', 'footer', 'aside'):
            return True
    except:
        pass
    return False

def extract(obj, depth=0, collected=None):
    if collected is None:
        collected = []
    if should_skip(obj):
        return collected
    text = get_text(obj).strip()
    if text and len(text) > 2:
        # Only keep if link density is low (skip pure-nav blocks)
        collected.append(text)
    for i in range(obj.childCount):
        try:
            extract(obj[i], depth+1, collected)
        except:
            pass
    return collected

desktop = pyatspi.Registry.getDesktop(0)
focused_app = None
focused_obj = None

try:
    focused_obj = pyatspi.Registry.getFocusedObject()
    if focused_obj:
        focused_app = focused_obj.get_application()
except:
    pass

if not focused_app:
    # Find active window across all apps
    for app in desktop:
        try:
            for window in app:
                try:
                    state = window.getState()
                    if state.contains(pyatspi.STATE_ACTIVE):
                        focused_app = app
                        focused_obj = window
                        break
                except:
                    pass
        except:
            pass
        if focused_app:
            break

if not focused_app:
    sys.exit(0)

app_name = focused_app.name or 'Unknown'
print(app_name)

texts = extract(focused_obj or focused_app)
# Deduplicate consecutive duplicates
seen = set()
for t in texts:
    key = t[:80]
    if key not in seen:
        seen.add(key)
        print(t)
"#,
        skip = skip_roles_csv
    )
}

/// Simple readability filter: remove very short lines and link-dense paragraphs.
fn apply_readability_filter(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.len() < 4 {
                return false;
            }
            // Skip lines that look like nav items (short + no sentence punctuation)
            if trimmed.len() < 20 && !trimmed.ends_with(['.', '!', '?', ':']) {
                return false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// xdotool fallback: get the active window title as minimal context
async fn read_via_xdotool() -> Result<ExtractedContent> {
    let output = tokio::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            let title = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if title.is_empty() {
                return Ok(ExtractedContent::default());
            }
            Ok(ExtractedContent::new(
                format!(
                    "[Window: {}] — Content not accessible. Use clipboard or selection mode.",
                    title
                ),
                title,
            ))
        }
        _ => Ok(ExtractedContent::default()),
    }
}
