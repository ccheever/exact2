//! A `markup="markdown"` text's source into the paragraph's runs (LLP 1045
//! D3, D4): `exact_markdown::pieces`, the expansion every host measures and
//! paints, so the measured paragraph is the painted one. Headings are bigger
//! and bolder, code is `ui-monospace`, a list item's runs carry its indent
//! and hung marker (`shaping`'s line insets), and links keep a target the
//! presenter follows.
use super::Run;
use exact_markdown::Role;

/// `ui-monospace`: one of the eight generic stacks at fixed ids (`Plan`).
const MONOSPACE: u16 = 5;
/// A marker's or a quote's ink: the web's `opacity: 0.62`.
pub const QUIET: u8 = 1;
/// A followed link's underline.
pub const UNDERLINE: u8 = 2;
/// `~~strike~~`.
pub const STRIKE: u8 = 4;

/// A link's target as the reader follows it: an absolute path is a location
/// in the app (no document base is needed for it), and `http`, `https`,
/// `mailto` and `tel` leave it; anything else is inert, as on the web and
/// Apple.
pub fn target(href: &str) -> Option<&str> {
    let href = href.trim();
    if href.starts_with('/') && !href.starts_with("//") {
        return Some(href);
    }
    let scheme = href.split_once(':')?.0;
    ["http", "https", "mailto", "tel"]
        .iter()
        .any(|s| s.eq_ignore_ascii_case(scheme))
        .then_some(href)
}

/// Expand `source`, the node's one run of Markdown, over its own style.
pub fn expand(source: &Run) -> Vec<Run> {
    exact_markdown::pieces(&source.text)
        .into_iter()
        .map(|p| {
            let href = target(&p.href).unwrap_or_default().to_string();
            let flag = |on: bool, bit: u8| if on { bit } else { 0 };
            let mark = flag(matches!(p.role, Role::Marker | Role::Quote), QUIET)
                | flag(p.role == Role::Link && !href.is_empty(), UNDERLINE)
                | flag(p.strike, STRIKE);
            // A short line between blocks keeps the node's line height from
            // stretching it back to a full line (as Apple's runs).
            let gap = p.scale < 1.0 && p.text == "\n";
            Run {
                size: source.size * p.scale,
                weight: if p.weight != 0 {
                    p.weight
                } else {
                    source.weight
                },
                family: if p.mono { MONOSPACE } else { source.family },
                italic: source.italic || p.italic,
                line_height: source.line_height.filter(|_| !gap),
                letter_spacing: source.letter_spacing,
                font_variant_numeric: source.font_variant_numeric,
                indent: p.indent,
                hang: p.hang,
                mark,
                href,
                text: p.text,
            }
        })
        .collect()
}
