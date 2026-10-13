//! @ref LLP 1115.000 D6 (amended 2026-10-13, Charlie's ruling) — a default
//! yields to the author's version of the same state: a disabled custom
//! pressable dims on the hosts that dim it, unless its author styles its
//! own disabled state. Decided here, once, from the element as written:
//! Contract marks the node (`disabledStyled`), and every host that dims
//! reads the mark.
//!
//! A node styles its own disabled state when it has `disabled` and
//!
//! - writes `opacity`, whatever its value: the author said how faint it is;
//! - or writes a colour, a background, a border colour or a filter whose
//!   expression reads a name its `disabled` expression reads (a slot, a
//!   derive, a prop, a local): the look changes with what disables it.
//!
//! A literal `disabled` reads nothing, so only an `opacity` marks it.

use contract_syntax::Attr;

use crate::warnings::screens::idents;

/// The paint a disabled look is drawn in.
const PAINT: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "border-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "filter",
];

/// Whether the element, as its classes expanded it, styles its own disabled state.
pub(crate) fn styled(attrs: &[Attr]) -> bool {
    let Some(disabled) = attrs.iter().rev().find(|a| a.name == "disabled") else {
        return false;
    };
    if attrs.iter().any(|a| a.name == "opacity") {
        return true;
    }
    let mut reads = Vec::new();
    idents(&disabled.value, &mut reads);
    !reads.is_empty()
        && attrs
            .iter()
            .filter(|a| PAINT.contains(&a.name.as_str()))
            .any(|a| {
                let mut paint = Vec::new();
                idents(&a.value, &mut paint);
                paint.iter().any(|n| reads.contains(n))
            })
}
