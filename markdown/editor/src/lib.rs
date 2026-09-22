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

mod editor;
mod projection;
mod text;
pub mod view;

/// The wasm exports the web host's `markup-editor.js` calls.
pub mod abi;

pub use editor::{Change, Editor, Facts, Input, Outcome, Selected};
pub use projection::Deco;
pub use view::Line;
