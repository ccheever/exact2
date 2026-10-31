//! Spelling suggestions for an unknown attribute or tag, from the
//! compiler's own lookups (`tags::attr`, `tags::tag`).
use crate::tags::{attr, tag, AttrTarget};

/// Suggest one unambiguous single-edit spelling from the existing attribute
/// lookup. No second vocabulary is maintained, and this never admits an alias.
pub(crate) fn similar_attr(name: &str, style_only: bool) -> Option<String> {
    similar(name, |candidate| match attr(candidate) {
        // Lowering's own row for `:active`'s opacity (LLP 1121 D8).
        _ if candidate == crate::states::PRESS_OPACITY_TO => false,
        Some(AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand) => true,
        Some(_) => !style_only,
        None => false,
    })
}
/// The same for a tag, from the tag lookup.
pub(crate) fn similar_tag(name: &str) -> Option<String> {
    similar(name, |candidate| tag(candidate).is_some())
}

/// One unambiguous single-edit spelling of `name` that `admitted` accepts,
/// found by trying every edit against the lookup itself.
fn similar(name: &str, admitted: impl Fn(&str) -> bool) -> Option<String> {
    if !name.is_ascii() || !(3..=64).contains(&name.len()) {
        return None;
    }
    let mut found: Option<String> = None;
    let mut consider = |bytes: &[u8]| {
        let candidate = std::str::from_utf8(bytes).expect("ASCII spelling edits");
        if candidate == name {
            return true;
        }
        if admitted(candidate) {
            if found.as_deref().is_some_and(|old| old != candidate) {
                return false;
            }
            found = Some(candidate.to_owned());
        }
        true
    };
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut candidate = name.as_bytes().to_vec();
    for index in 0..name.len() {
        let original = candidate.remove(index);
        if !consider(&candidate) {
            return None;
        }
        candidate.insert(index, original);
        for &letter in LETTERS {
            candidate[index] = letter;
            if !consider(&candidate) {
                return None;
            }
        }
        candidate[index] = original;
        if index + 1 < name.len() {
            candidate.swap(index, index + 1);
            if !consider(&candidate) {
                return None;
            }
            candidate.swap(index, index + 1);
        }
    }
    for index in 0..=name.len() {
        candidate.insert(index, b'a');
        for &letter in LETTERS {
            candidate[index] = letter;
            if !consider(&candidate) {
                return None;
            }
        }
        candidate.remove(index);
    }
    found
}
