//! Where a warning stands among a build's (LLP 1116 D6), source order kept
//! within a rank: first what changes what the app shows, or how a person
//! reaches it, on its native targets (a header item the bar leaves out, a
//! button VoiceOver cannot name, a scroller that grows, a flex property on a
//! block); then the rest; then naming advice that changes nothing shown;
//! last a symbol blank on the web alone. The euxy port's
//! `lower-header-unplaced` was one of 43, after 13 aliases. Lowering keeps
//! its first [`crate::MAX_WARNINGS`] by this order, and `contract` orders a
//! build's warnings by it, so a warning that changes the app is never the
//! one the cap drops.

use crate::LowerError;

/// The warning `id`, saying `message`, by its rank: 0 first.
pub fn rank(id: &str, message: &str) -> u8 {
    if web_only(id, message) {
        return 3;
    }
    match id {
        "lower-header-unplaced"
        | "lower-header-nested"
        | "lower-header-routeless"
        | "lower-header-no-bar"
        | "lower-env-in-owner"
        | "lower-navigation-key"
        | "lower-screen-by-state"
        | "lower-button-unlabelled"
        | "lower-nested-button"
        | "lower-round-button-label"
        | "lower-dialog-role"
        | "lower-level-without-heading"
        | "lower-scroll-grows"
        | "lower-flex-on-block"
        | "lower-width-padding"
        | "lower-swipe-wiring"
        | "lower-row-delete-button"
        | "lower-delete-row"
        | "lower-sound-undeclared"
        | "lower-symbol-tint" => 0,
        "lower-symbol-alias" => 2,
        _ => 1,
    }
}

/// A warning only the web's picture shows: an SF Symbol drawn blank where
/// the web is the one target besides Apple's (with Linux too, it is blank on
/// a native host). A build's text says these in one line.
pub fn web_only(id: &str, message: &str) -> bool {
    // Its first use names the targets, `ships web (…) and linux (…), where
    // it is blank`; a later one, `blank on web too`.
    let shipped = message
        .split_once(" ships ")
        .and_then(|(_, rest)| rest.split_once(", where it is blank"))
        .map(|(targets, _)| targets);
    id == "lower-symbol-platform"
        && match shipped {
            Some(targets) => targets.starts_with("web (") && !targets.contains(") and "),
            None => message.contains(" blank on web too"),
        }
}

/// The first [`crate::MAX_WARNINGS`] of `out` by rank, source order kept
/// within one.
pub(crate) fn keep_first(out: &mut Vec<LowerError>) {
    out.sort_by_key(|w| rank(w.id, &w.message));
    out.truncate(crate::MAX_WARNINGS);
}
