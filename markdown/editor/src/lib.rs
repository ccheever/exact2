//! WYSIWYG Markdown editing over the source string.
//!
//! @ref LLP 1045 D1/D5 — the editor's value is the Markdown source and nothing
//! else. Syntax is hidden, never removed: a host draws [`Editor::lines`], one
//! per source line, with [`view::HIDDEN`] segments kept out of layout, so the
//! drawn text is the source and the platform's own typing, composition and
//! spelling work on it directly. What the platform must not decide — where a
//! caret sits around hidden syntax, what Backspace removes, where typed text
//! goes beside a bold word, list Return, pending formats, undo — is decided
//! here, once, for every host.
//!
//! Optional by construction: nothing below the hosts depends on this crate,
//! and the web ships it as its own wasm (`abi`), fetched only when a Markdown
//! textarea mounts. Offsets are UTF-16 code units, as in `exact-markdown`.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod direction;
mod editor;
mod projection;
mod text;
pub mod view;

/// The wasm exports the web host's `markup-editor.js` calls. Only the wasm
/// build has them: a native host that links this crate for
/// [`link_destinations`] exports none of them.
#[cfg(target_arch = "wasm32")]
pub mod abi;

pub use direction::{auto_directions, AUTO_LTR, AUTO_NONE, AUTO_RTL};
pub use editor::{Change, Editor, Facts, Input, Outcome, Selected, SelectionMode};
pub use projection::Deco;
pub use view::Line;

/// Each link's and image's `[` (or `![`) and `](target)` in `styled`, in
/// UTF-16 code units: the destinations a host that hides only those (Apple,
/// LLP 1045 D5) keeps out of layout, with the opener a deletion keeps beside
/// a kept destination. A destination that spans a line break is left shown:
/// hidden, its newline would leave an empty line.
pub fn link_destinations(source: &str, styled: &exact_markdown::Styled) -> Vec<[u32; 4]> {
    let units: Vec<u16> = source.encode_utf16().collect();
    styled
        .links
        .iter()
        .filter(|l| {
            let close = units.get(l.close.start as usize..l.close.end as usize);
            close.is_some_and(|c| !c.iter().any(|&u| u == 10 || u == 13))
        })
        .map(|l| [l.open.start, l.open.end, l.close.start, l.close.end])
        .collect()
}
