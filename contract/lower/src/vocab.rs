//! What the compiler admits, listed from its own lookups (LLP 1086 D3).
//!
//! `tags.rs`'s `tag`, `attr`, `renamed` and `html_tag` are `match`
//! expressions with no table to iterate; `build.rs` scans their arms'
//! patterns into candidates, and this module keeps the ones the live lookup
//! admits, so the list cannot claim a name the compiler refuses. The style
//! names are a table, `style_names::STYLE_NAMES` (LLP 1081 D8), read whole. The rules
//! no lookup lists (the `head` fields, the attributes admitted only on some
//! tags, the two open sets) are named here beside them.

use crate::tags::{self, AttrTarget, Tag};
use contract_syntax::{Attr, Expr, Span};
use exact_kernel::{PropId, StyleCodec, StyleId};

/// A prop's literal value grammar, shared by its refusal and `contract vocab`.
pub fn accepts(prop: PropId) -> Option<&'static str> {
    match prop {
        PropId::NavigationDetent => Some("space-separated large, medium, or finite positive point heights; fit-content alone or as fit-content large"),
        _ => None,
    }
}

include!(concat!(env!("OUT_DIR"), "/vocab.rs"));

/// Attributes restricted by tag or collection shape; the `head` fields
/// are [`tags::HEAD_FIELDS`].
pub const CONTEXTUAL: &[(&str, &str)] = &[
    ("sandbox", "`iframe`"),
    ("src", "`iframe` or `video`"),
    ("load", "`iframe`, `image` or a native module"),
    ("message", "`iframe`, `canvas` or a native module"),
    ("document", "`scroll`"),
    (
        "reachstart",
        "`list virtualized=true` with one direct `each`",
    ),
    ("reachend", "`list virtualized=true` with one direct `each`"),
    ("text-transform", "any tag but `input` and `textarea`"),
    ("transform", "SVG elements, including `svg` and SVG `text`"),
    (
        "selectionchange",
        "`text`, plain text `input` or `textarea`",
    ),
    ("beforeinput", "plain text `input` or `textarea`"),
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

/// The compiler and target emitters share the collection-edge refusal.
pub fn collection_edge_refusal(event: &str) -> String {
    format!("`{event}` belongs to a virtualized list (LLP 1010): write `list virtualized=true` with one direct `each`. Only a non-empty collection has edges in its geometric window; a plain list lays out all its children and has no collection window to report an edge. For a plain feed or masonry, use the web's `intersect=` on a marker at the end of the content, after the columns (docs/reference.md, \"Intersection visibility\"):\nview intersect=loadMore intersect-margin=\"0px 0px 800px 0px\"\n  height=1")
}

/// LLP 1055.000 D5's box refusal and the alternatives printed by `contract vocab`.
pub const BOX_TRANSFORM_REFUSAL: &str = "write the individual `translate`, `rotate` and `scale` properties on a box: `transform=\"translate(0px, 25px)\"` becomes `translate=\"0px 25px\"`, and `transform=\"scale(0.8)\"` becomes `scale=0.8`; `transform` on a box is deferred (LLP 1055.000 D5), and is accepted only on SVG elements";

/// What a button with no `buttonStyle` is on each host (LLP 1115.000 D15,
/// D14.5), in one sentence for both places that print it. The hosts hold the
/// defaults themselves: `ButtonFace.unsaid` on Apple's, `bordered` wherever
/// another host reads the prop.
macro_rules! button_default {
    () => {
        "with no `buttonStyle` a button is the platform's default: plain tinted text on iOS (as `UIButton(type: .system)` and SwiftUI's `Button` are), a push button on macOS, bordered on the web (a bare text button there reads as unstyled)"
    };
}

/// Printed under `contract vocab buttonStyle`.
pub const BUTTON_DEFAULT_NOTE: &str = button_default!();

/// What a tag means beyond its node, where the hosts make more of it than
/// the web: `contract vocab <tag>` prints it under the tag.
pub const TAG_NOTES: &[(&str, &str)] = &[
    (
        "header",
        // @ref LLP 1116.003 D2.2, LLP 1115.000 D5.2, D5.3 —
        // NavigationBarIOS.swift's `HeaderShape`, `NavigationRules.bar`.
        "on iOS a route's `header` is its navigation bar when it is the route's first child, is shown, and holds exactly one heading (`text role=\"heading\"`, or a `text` with `aria-level`), whatever the stack's other routes have. The heading is the title, large at `aria-level=1`, else inline; buttons that press or open a popover are the bar's items, leading before the heading and trailing after it; the first text after the heading is the subtitle; one `input type=\"search\"` is the bar's search field and one `role=\"tablist\"` its segmented title; the root's `navigationBack` control is the system Back button. The rest of the header is hidden (warning `lower-header-unplaced`). A header with no heading or with two is no bar and is drawn in the page as written (warning `lower-header-no-bar`). A pushed route with no bar of its own (no header, or one that is no bar) still gets a bar holding the system Back button alone, with no title; a stack's first route with none has no bar. A route that draws its own chrome says `navigationBar=\"none\"`: no bar, and the edge swipe still goes back; without it, a `navigationBack` control drawn in a pushed route's page is a second Back under the bar's (warning `lower-back-twice`). On the Mac the same header is the window's title bar and toolbar (LLP 1115.002 D2): the heading is the window's title, with no large title; the buttons are toolbar items, one whose popover is a menu a menu item; a `role=\"tablist\"` of text tabs is the centred segments; the search input is the toolbar's search field. There a header holding anything a toolbar has no place for (a picture, a second text, a pressable title, a button drawn as a box) is drawn in the page as written, whole. docs/recipes/navigation-stack.md",
    ),
    (
        "button",
        // @ref LLP 1115.000 D15 — the default, then what an author says in its place.
        concat!(
            button_default!(),
            ". Say what the action is and leave the look: `buttonStyle=\"filled\"` on the screen's main action (Save, Add, Send), `\"bordered\"` on one that must read as a button, nothing on a secondary or inline one (Cancel, Edit, an empty state's), and `destructive=true` on one that deletes, which iOS draws in the system red. In a `header` it is a bar item, in a `dialog role=\"alertdialog\"` an alert's action, and as a grouped `list`'s row a cell: each the platform's already. A background, border or radius of your own makes it your box, the same on every host (`contract vocab appearance`)"
        ),
    ),
];

/// A tag's note, if it has one ([`TAG_NOTES`]).
pub fn tag_note(name: &str) -> Option<&'static str> {
    TAG_NOTES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, note)| *note)
}

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

/// Every attribute, by name: the scanned arms of `attr`, and the style
/// names, which are a table (`style_names::STYLE_NAMES`, LLP 1081 D8).
pub fn attrs() -> Vec<(&'static str, AttrTarget)> {
    // A part's declaration (`::placeholder color`) is no attribute to write:
    // `contract vocab ::placeholder` describes the block.
    let styles: Vec<&'static str> = crate::style_names::STYLE_NAMES
        .iter()
        .map(|(n, ..)| *n)
        .filter(|n| !n.starts_with("::"))
        .collect();
    sorted(&[ATTR_CANDIDATES, &styles].concat())
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

/// Whether a style row takes a colour role by name (`-exact-label`, `Canvas`): the colour codecs that read one.
pub fn takes_roles(row: StyleId) -> bool {
    matches!(
        row.codec(),
        StyleCodec::ColorValue | StyleCodec::KeywordColor
    )
}

/// The colour roles the schema's table gives, as written: Exact's (`-exact-<name>`, listed by `<name>`), then CSS's
/// system colours (by their names). `contract vocab background-color` lists them (LLP 1116.009 D6: tip-splitter guessed one).
pub fn color_roles() -> (Vec<&'static str>, Vec<&'static str>) {
    let names = |css: bool| {
        exact_kernel::generated::COLOR_ROLES
            .iter()
            .filter(|r| exact_kernel::style::roles::is_css_system(r) == css)
            .map(|r| r.name)
            .collect()
    };
    (names(false), names(true))
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
        written: true,
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
        assert!(
            ATTR_CANDIDATES.contains(&"press"),
            "attribute `press` missed"
        );
        // Style names are a table, not scanned arms (LLP 1081 D8).
        let listed: Vec<&str> = attrs().into_iter().map(|(n, _)| n).collect();
        for name in [
            "padding",
            "animation-play-state",
            "overscroll-behavior",
            "-exact-press-scale",
        ] {
            assert!(listed.contains(&name), "attribute `{name}` missed");
        }
        assert!(RENAMED_CANDIDATES.contains(&"keyboardAvoiding"));
        assert!(HTML_CANDIDATES.contains(&"h6"));
        assert_eq!(default(StyleId::PaddingTop), Some("0"));
        assert_eq!(default(StyleId::FlexDirection), Some("row"));
    }
}
