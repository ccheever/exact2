//! The colour declarations: CSS's `@color-profile` (LLP 1100 D3), the
//! app's `theme` (LLP 1095 D4) and its design, `app.tamber` (LLP 1121 D2).

use crate::{Attr, Span};

/// `color-profile --name src="…" rendering-intent=…`: CSS's `@color-profile`.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorProfileDecl {
    /// The dashed name `color()` refers to, `--` included.
    pub name: String,
    /// The descriptors, literal: `src` and `rendering-intent`.
    pub attrs: Vec<Attr>,
    /// Where.
    pub span: Span,
}

/// `theme`, one `name="colour"` line an entry (LLP 1095 D4): a colour any
/// row takes, another entry's name, or a role's; an entry named like a role
/// overrides it.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDecl {
    /// The entries, in order; each value a string literal.
    pub entries: Vec<Attr>,
    /// Where.
    pub span: Span,
}

/// The app's design (LLP 1121 D2): `app.tamber` beside `app.contract`, or
/// the file `app.json`'s `"tamber"` names, read by the driver. The
/// compiler checks and folds it with `exact-tamber`, as it does an inline
/// `theme`; an app has one or the other.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignSource {
    /// The file, as the driver found it.
    pub path: String,
    /// The file as a component's identity names it (LLP 1121 D15):
    /// relative to the app's directory.
    pub home: String,
    /// Its text.
    pub text: String,
    /// Its source id, as a span names the file it is in.
    pub source_id: u32,
    /// The app beside it has code that can name a token outside its views
    /// (an `app.ts`, a Rust data crate, declared native modules or hatches):
    /// the driver's half of `names_tokens_outside_views` (LLP 1121 D7).
    pub bindable: bool,
}
