//! What the compiler admits, listed from its own lookups (LLP 1086 D3).
//!
//! `tags.rs`'s `tag`, `attr`, `renamed` and `html_tag` are `match`
//! expressions with no table to iterate; `build.rs` scans their arms'
//! patterns into candidates, and this module keeps the ones the live lookup
//! admits, so the list cannot claim a name the compiler refuses. The rules
//! no lookup lists (the `head` fields, the attributes admitted only on some
//! tags, the two open sets) are named here beside them.

use crate::tags::{self, AttrTarget, Tag};
use contract_syntax::{Attr, Expr, Span};
use exact_kernel::{StyleCodec, StyleId};

include!(concat!(env!("OUT_DIR"), "/vocab.rs"));

/// Attributes legal only on some tags, as `lib.rs`'s `lower-attr-tag`
/// refusals decide; the `head` fields are [`tags::HEAD_FIELDS`].
pub const CONTEXTUAL: &[(&str, &str)] = &[
    ("sandbox", "`iframe`"),
    ("src", "`iframe` or `video`"),
    ("load", "`iframe`, `image` or a native module"),
    ("message", "`iframe`, `canvas` or a native module"),
    ("document", "`scroll`"),
    ("reachstart", "`list`"),
    ("reachend", "`list`"),
    ("text-transform", "any tag but `input` and `textarea`"),
    ("selectionchange", "`text`"),
    ("name", "`input type=\"radio\"`"),
    ("markup", "`text` or `textarea`"),
    // @ref LLP 1098 D1, D2 — the media session's, `media::check_session`.
    ("metadata", "`audio` or `video`"),
    ("seekbackward", "an `audio` or `video` with `metadata=`"),
    ("seekforward", "an `audio` or `video` with `metadata=`"),
    ("seekto", "an `audio` or `video` with `metadata=`"),
    ("previoustrack", "an `audio` or `video` with `metadata=`"),
    ("nexttrack", "an `audio` or `video` with `metadata=`"),
    ("stop", "an `audio` or `video` with `metadata=`"),
    (
        "seekbackwardOffset",
        "an `audio` or `video` with `metadata=`",
    ),
    (
        "seekforwardOffset",
        "an `audio` or `video` with `metadata=`",
    ),
];

/// The open set of tags: a hyphenated name is a native module.
pub const MODULE_NOTE: &str = "a hyphenated tag (`map-view`) is a native module, admitted when app.json's `modules` declares it; its other attributes are the module's props";

/// The open set of attributes: `data-*`.
pub const DATA_NOTE: &str = "`data-<word>` is the app's own word on any element, admitted when app.json's `data` declares the word";

fn sorted(names: &[&'static str]) -> Vec<&'static str> {
    let mut names = names.to_vec();
    names.sort_unstable();
    names.dedup();
    names
}

/// Every built-in tag, by name.
pub fn tags() -> Vec<(&'static str, Tag)> {
    sorted(TAG_CANDIDATES)
        .into_iter()
        .filter_map(|n| tags::tag(n).map(|t| (n, t)))
        .collect()
}

/// Every attribute, by name.
pub fn attrs() -> Vec<(&'static str, AttrTarget)> {
    sorted(ATTR_CANDIDATES)
        .into_iter()
        .filter_map(|n| tags::attr(n).map(|t| (n, t)))
        .collect()
}

/// Every old spelling with the name it became.
pub fn renamed() -> Vec<(&'static str, &'static str)> {
    sorted(RENAMED_CANDIDATES)
        .into_iter()
        .filter_map(|n| tags::renamed(n).map(|t| (n, t)))
        .collect()
}

/// Every HTML element Contract spells differently, with what to write.
pub fn html_tags() -> Vec<(&'static str, &'static str)> {
    sorted(HTML_CANDIDATES)
        .into_iter()
        .filter_map(|n| tags::html_tag(n).map(|t| (n, t)))
        .collect()
}

/// The longhand rows a CSS shorthand attribute (`border`, `border-top`, …,
/// `text-decoration`) sets, in the order it lowers them.
pub fn shorthand_rows(name: &str) -> &'static [StyleId] {
    crate::shorthands::rows(name)
}

/// A style row's wire codec, by the schema's spelling (`dimension`,
/// `colorValue`, `enum`).
pub fn codec(row: StyleId) -> String {
    let name = format!("{:?}", row.codec());
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// What a `transition` names (`transition-property`, LLP 1002): `all`,
/// `border-color`, and each property the engine moves, a path's `d` among
/// them (LLP 1055.000 D15).
pub fn transition_properties() -> Vec<&'static str> {
    use exact_motion::Property;
    let mut names = vec!["all", "border-color"];
    names.extend(
        Property::ALL
            .into_iter()
            .chain([Property::D])
            .filter(|p| *p != Property::ShadowColor)
            .map(|p| p.name()),
    );
    names
}

/// A style row's default as the schema writes it, if it declares one; a
/// colour's zero is `transparent`.
pub fn default(row: StyleId) -> Option<&'static str> {
    let default = STYLE_DEFAULTS
        .iter()
        .find(|(field, _)| *field == row.name())
        .map(|(_, default)| *default)?;
    let colour = matches!(row.codec(), StyleCodec::ColorValue | StyleCodec::Rgba8);
    Some(if colour && default == "0" {
        "transparent"
    } else {
        default
    })
}

/// Whether `name` is in one of the open sets: a `data-*` attribute or a
/// native module's tag. A built-in name is not (`align-items` is hyphenated).
pub fn open_set(name: &str) -> Option<&'static str> {
    if name.starts_with("data-") {
        Some(DATA_NOTE)
    } else if crate::native::is_module_tag(name)
        && crate::lint::fragmentation(name).is_none()
        // An `aria-*` name is an attribute ARIA has or does not, never a module.
        && !name.starts_with("aria-")
    {
        Some(MODULE_NOTE)
    } else {
        None
    }
}

/// What the compiler says of a name that is neither a tag nor an attribute:
/// its own hints for an unknown tag and an unknown attribute (a renamed
/// spelling, an HTML element's Contract name, "did you mean").
pub fn refusal(name: &str) -> String {
    let span = Span::default();
    let tag = crate::lint::unknown_tag(name, span).message;
    let attr = Attr {
        name: name.into(),
        value: Expr::Bool(true, span),
        span,
    };
    let attr = crate::lint::unknown_attr("view", &attr).message;
    let mut hints: Vec<&str> = Vec::new();
    for (message, prefix) in [
        (&tag, format!("unknown tag `{name}`")),
        (&attr, format!("`view` has no attribute `{name}`")),
    ] {
        let hint = message.strip_prefix(&prefix).unwrap_or(message);
        if !hint.is_empty() && !hints.contains(&hint) {
            hints.push(hint);
        }
    }
    format!("`{name}` is not a tag or an attribute{}", hints.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every scanned spelling resolves, or is a refusal the compiler names.
    #[test]
    fn every_candidate_resolves_or_is_a_refusal() {
        for name in TAG_CANDIDATES {
            assert!(
                tags::tag(name).is_some(),
                "tag `{name}` is scanned but refused"
            );
        }
        for name in ATTR_CANDIDATES {
            assert!(
                tags::attr(name).is_some(),
                "attribute `{name}` is scanned but refused"
            );
        }
        for name in RENAMED_CANDIDATES {
            assert!(tags::attr(name).is_none(), "renamed `{name}` is admitted");
            assert!(
                tags::renamed(name).is_some() || matches!(*name, "className" | "class" | "style"),
                "`{name}` is scanned in `renamed` but names nothing"
            );
        }
        for name in HTML_CANDIDATES {
            assert!(tags::tag(name).is_none(), "HTML `{name}` is admitted");
            assert!(tags::html_tag(name).is_some(), "`{name}` names nothing");
        }
    }

    /// Arm bodies are skipped: their value literals are never candidates.
    #[test]
    fn the_scan_reads_patterns_only() {
        for value in ["flex", "pre-wrap", "absolute", "font-size", "hidden"] {
            assert!(
                !TAG_CANDIDATES.contains(&value),
                "`{value}` scanned as a tag"
            );
            assert!(
                !RENAMED_CANDIDATES.contains(&value),
                "`{value}` scanned as renamed"
            );
        }
        for name in ["column", "feSpotLight", "rect", "clipPath", "head"] {
            assert!(TAG_CANDIDATES.contains(&name), "tag `{name}` missed");
        }
        for name in [
            "padding",
            "press",
            "animation-play-state",
            "overscroll-behavior",
        ] {
            assert!(ATTR_CANDIDATES.contains(&name), "attribute `{name}` missed");
        }
        assert!(RENAMED_CANDIDATES.contains(&"keyboardAvoiding"));
        assert!(HTML_CANDIDATES.contains(&"h6"));
        assert_eq!(default(StyleId::PaddingTop), Some("0"));
        assert_eq!(default(StyleId::FlexDirection), Some("row"));
    }
}
