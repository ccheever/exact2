//! Contract syntax: the lexer, the AST, and the parser.
//!
//! @ref LLP 1004 D3 (the language basis: Contract v1 Edition 1, scoped to the
//! v1 app's constructs) / LLP 0508 (research)
//!
//! The grammar is indentation-structured. A file is a sequence of `shape` and
//! `component` declarations; a component holds `props`, `state`, `derive`,
//! `resource`, `action`, `task`, and `view` sections; a view is a tree of
//! elements, component uses, and the three region constructs `when`/`else`,
//! `each … in … key=…`, and `match … case some(x) / case none`. Expressions
//! are closed: literals, template strings, names, member access, calls,
//! arithmetic, comparison, boolean logic, the conditional operator, `some`,
//! `none`, and inline `match`.
//!
//! Every node carries a [`Span`]; every rejection is a [`SyntaxError`] with
//! one stable id and a span. Nothing here knows about types or the plan.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod ast;
mod clock;
pub mod fmt;
pub mod idioms;
pub mod inline;
pub mod lexer;
pub mod parser;
pub mod scope;
mod share;
mod spans;

pub use ast::*;
pub use clock::{resolve_clock_timelines, resolve_clock_timelines_in};
pub use inline::{
    expand, expand_all, expand_checked, expand_mapped, expand_typed, hygiene, inline, Expanded,
    Instance, Owner,
};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{parse, parse_source, parse_source_all, SyntaxError};
pub use share::share_calls;
pub use spans::VisitSpans;

/// A token range in one source file. Lines and byte columns are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Line.
    pub line: u32,
    /// Inclusive byte column.
    pub col: u32,
    /// Exclusive byte column; equal to `col` for a structural position.
    pub end_col: u32,
    /// File identity assigned by the loader; zero for a standalone source.
    pub source_id: u32,
}

impl Span {
    /// An empty position in a standalone source.
    pub const fn point(line: u32, col: u32) -> Self {
        Self {
            line,
            col,
            end_col: col,
            source_id: 0,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// The commands a host answers (LLP 1005 §3): every name an action body may
/// call. The web host's `command` op, the Apple session's queue, and the Linux
/// presenter's `run_commands` match these by name; any other name would reach
/// them and be refused there, silently to the author, so the type pass
/// refuses it. It lives here, not in the type pass, because expansion reads
/// it too: a statement naming one is the command, even inside an action of
/// the same name, never a call of that action (LLP 1089 D1).
pub const HOST_COMMANDS: &[&str] = &[
    "blur",
    "copyText",
    "deliveryActivate",
    "deliveryCheck",
    "focus",
    "format",
    // @ref LLP 1077 D14 — `haptic("success" | "warning" | "error" | …)`.
    "haptic",
    "openURL",
    // `reload()`: the development host boots the app again, as its dev
    // menu's Reload does; a host without a dev menu refuses it.
    "reload",
    "selectText",
    // `el.setSelectionRange(start, end, direction)` on a text field by its
    // `id` (x2apps codeedit #2): `setSelectionRange("editor", 4, 4)`.
    "setSelectionRange",
    "setScheme",
    // @ref LLP 1069.002 D2 — `HTMLInputElement.showPicker()` on a file input.
    "showPicker",
    "share",
    // Local notifications by the Notification API's names (rules/DEFERRED.md,
    // 2026-10-04): `showNotification(title=, body=, tag=, showTrigger=)` and
    // `closeNotification(tag)`.
    "showNotification",
    "closeNotification",
    // @ref LLP 1069.010 D3 — export: the host copies an `app:/` file out.
    "saveFile",
    // @ref LLP 1069.010 D2 — the File System Access API's pickers.
    "showOpenFilePicker",
    "showDirectoryPicker",
    "showSaveFilePicker",
    // @ref LLP 1070.000 — a virtualized list's row brought into view, by key.
    "scrollIntoView",
    // The inverse of a canvas's `message=`: `postMessage(text, "world")` queues
    // text into the surface of that name, delivered in order, never coalesced.
    "postMessage",
    // `event.preventDefault()` for the `key` or `wheel` event that ran the
    // action: the host skips its default action, the key's or the scroll;
    // for `beforeunload`, the window stays open (docs/contract-grammar.md#events).
    "preventDefault",
    // `event.stopPropagation()` for the same event: no ancestor's `key`
    // handler hears it, and its default still happens (files diary F8).
    "stopPropagation",
    // `window.close()` (studio diary R17): the window closes without asking
    // its `beforeunload` again — what an app calls once its own "Save
    // changes?" is answered. As every host command keeps its name (LLP 1089
    // D1), an action prop named `close` is bound, never called.
    "close",
];
