//! An author's `user-select`.
use exact_num::push_text;

/// `auto` is unsaid, as on the native hosts (TextSelectable.swift): the page's
/// sheet decides. WebKit 27.2 selects by its prefixed property, so a value is
/// written in both.
pub(super) fn push(out: &mut String, value: &str) {
    if value != "auto" {
        push_text!(out, "-webkit-user-select:{};user-select:{};", value, value);
    }
}
