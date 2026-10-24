//! A native module's sources and sends (LLP 1133.001 D1, LLP 1133.002 D13).
//! Contract never declares one: the loader reads them from a package's
//! `exact.json`, and `use` brings them in as it brings a shape.

use crate::{Field, Span, TypeExpr};

/// Whether a module name is read, as a resource reads a source, or sent,
/// as a mutation's `send` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    /// A source: `resource r = name(…)`.
    Source,
    /// A send: `send m = name(…)`.
    Send,
}

/// One source or send a module declares.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    /// The name a call writes: the declared one, or its program-unique name
    /// once scoped.
    pub name: String,
    /// The name the module declares, which the wire carries.
    pub declared: String,
    /// The interface's package name, `@exact/geolocation`.
    pub package: String,
    /// The wire's `extension`, the package's name without its scope.
    pub extension: String,
    /// Read or sent.
    pub kind: ModuleKind,
    /// Its keyword arguments. One typed `option<T>` may be omitted, and then
    /// stays the platform's own default (LLP 1133.002 D2); any other is
    /// required.
    pub options: Vec<Field>,
    /// Each platform's words for it, by the key a call writes (`apple`).
    pub words: Vec<PlatformWords>,
    /// What it answers.
    pub answers: TypeExpr,
    /// A live source: the module announces when its answer changes.
    pub live: bool,
    /// Whether a call may show the platform's prompt: `always`,
    /// `when-prompt` (a source, while the state is `prompt`) or `never`.
    pub request: String,
    /// Its synchronous form's name, where it has one (LLP 1133 D6 rule 4).
    pub sync: Option<String>,
    /// Callable from data code alone (`"contract": false`, LLP 1133.011
    /// D10 item 1): a Contract `use` of it is refused, so its answer never
    /// becomes a mutation's value in `state`.
    pub data_only: bool,
    /// The agent's substitute as the package declares it, a JSON object
    /// (LLP 1133.002 §6): `default`, `from`, `ticket`, `short`, `fill`,
    /// `cancel`, `timeout`, and the interface's `permission`.
    pub substitute: String,
    /// The declaration, in the package's `exact.json`.
    pub span: Span,
}

/// One platform's words on a module name (LLP 1133 D2, tier 2), as the
/// implementation that owns them declares them. Every word is optional, and
/// one a call omits stays distinct from one it writes.
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformWords {
    /// The key a call writes, `apple` in `apple.desiredAccuracy=…`.
    pub platform: String,
    /// The implementation that declares them, `@exact/geolocation/apple`.
    pub package: String,
    /// The words and their types.
    pub words: Vec<Field>,
}
