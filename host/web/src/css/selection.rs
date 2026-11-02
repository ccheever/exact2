//! An author's `user-select`.
use exact_num::push_text;

/// `auto` is unsaid, as on the native hosts (TextSelectable.swift): the page's
/// stylesheet decides. WebKit parses `user-select` but selects by
/// `-webkit-user-select` alone, so any other value is written in both.
pub(super) fn push(out: &mut String, value: &str) {
    if value != "auto" {
        push_text!(out, "-webkit-user-select:{};user-select:{};", value, value);
    }
}
