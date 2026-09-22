//! Markdown for reading and editing, over the source string itself.
//!
//! @ref LLP 1045 D1, D2 — the source is the only value. [`style`] says how to
//! draw it (spans, paragraph styles, the marker ranges to hide), [`edit`] turns
//! a formatting command into ordinary text replacements, and [`segments`] cuts
//! a document into the text a single node paints and the blocks text cannot be.
//! Reading is `style(source, None)`; editing passes the selection, and a
//! construct the selection touches keeps its markers visible.
//!
//! Ranges in and out are UTF-16 code units: what TextKit, CoreText and the DOM
//! count in. Pure: no I/O, no host, no dependency.
//!
//! The dialect, exactly (LLP 1045 D2): ATX headings without closing hashes;
//! `*` `_` emphasis with CommonMark's flanking rules; code spans without
//! edge-space normalization; `~~` strikethrough (one tilde stays literal);
//! inline links, autolinks, bare `http(s)` URLs; backslash escapes; fenced
//! code outside quotes; `- * +` and `1.` `1)` lists with `[ ]` tasks; quotes
//! by depth; rules; GFM pipe tables with the alignment row ignored; footnote
//! references and one-paragraph definitions; figures. Not setext headings,
//! indented code, reference links, HTML (inert text) or nested block trees.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod block;
mod edit;
mod inline;
mod offsets;
mod segment;
mod style;

pub use edit::{edit, Command, Edit};
pub use segment::{embed, excerpt, plain, segments, video, Embed, Provider, Segment};
pub use style::{style, Footnote, Paragraph, ParagraphKind, Replaced, Replacement, Span, Styled};

/// Bold.
pub const BOLD: u8 = 1;
/// Italic.
pub const ITALIC: u8 = 2;
/// Inline code.
pub const CODE: u8 = 4;
/// Strikethrough.
pub const STRIKE: u8 = 8;
/// A link; the span's `href` is its target.
pub const LINK: u8 = 16;
/// An image's alternative text; the span's `href` is its source.
pub const IMAGE: u8 = 32;
/// Markdown syntax left visible because the selection touches it; drawn dimmed.
pub const MARKER: u8 = 64;

/// A half-open range of UTF-16 code units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    /// First code unit.
    pub start: u32,
    /// One past the last code unit.
    pub end: u32,
}

impl Range {
    /// The range from `start` to `end`.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// An empty range: a caret.
    pub fn caret(at: u32) -> Self {
        Self { start: at, end: at }
    }
}
