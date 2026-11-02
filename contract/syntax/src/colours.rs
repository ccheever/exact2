//! The colour declarations: CSS's `@color-profile`, the
//! app's `theme` and its design, `app.tamber`.

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

/// `theme`, one `name="colour"` line an entry: a colour any
/// row takes, another entry's name, or a role's; an entry named like a role
/// overrides it.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDecl {
    /// The entries, in order; each value a string literal.
    pub entries: Vec<Attr>,
    /// Where.
    pub span: Span,
}

/// The app's design: `app.tamber` beside `app.contract`, or
/// the file `app.json`'s `"tamber"` names, read by the driver. The
/// compiler checks and folds it with `exact-tamber`, as it does an inline
/// `theme`; an app has one or the other.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignSource {
    /// The file, as the driver found it.
    pub path: String,
    /// The file as a component's identity names it:
    /// relative to the app's directory.
    pub home: String,
    /// Its text.
    pub text: String,
    /// Its source id, as a span names the file it is in.
    pub source_id: u32,
    /// The app beside it has code that can name a token outside its views
    /// (an `app.ts`, a Rust data crate, declared native modules or hatches):
    /// the driver's half of `names_tokens_outside_views`.
    pub bindable: bool,
    /// The package whose design this is, by its
    /// `package.json` name (`@exact/reading`); none for the app's.
    pub package: Option<String>,
    /// The package's directory, where its `assets/` are (an icon's SVG is
    /// read under the assets of the root whose file writes it); none for
    /// the app's, whose `assets/` are the app's.
    pub root: Option<String>,
}
