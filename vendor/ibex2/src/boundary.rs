//! The host-call boundary.
//!
//! LLP 0059.000 §1: one surface, primitives and handles only, no JSON, and the
//! capability check before dispatch.
//!
//! **A clarification this module had to settle.** §1.2 says host calls are
//! async by default, with the sync exceptions named in §2 (`localStorage`,
//! `performance.now`). Read literally that makes `new URL()` and
//! `TextEncoder.encode()` asynchronous, which is impossible — they are
//! synchronous web APIs and no application can await them. The rule being
//! expressed is LLP 0057 §3's: *no new synchronous cross-thread call surface*.
//! A **pure** op (the Summary's first shape) computes on the calling thread and
//! never leaves it, so it is not a cross-thread call and returns directly.
//! Async-by-default governs the **delegating** and **ambient** shapes, which do
//! leave the thread. Everything in this module is pure; nothing here may block.
//!
//! @ref LLP 0059.000#1-the-host-call-boundary — the surface this implements
//! @ref LLP 0057#3-the-boundary — Rust owns semantics, the platform owns transport

use crate::grant::{GrantSet, Operation};

/// An **argument** crossing into Rust.
///
/// Borrowed, and deliberately so: LLP 0059.000 §1.2 requires an `ArrayBuffer`
/// argument to reach Rust as a span over the engine's own allocation rather
/// than a copy of it. `HostArg` is the type that makes copying the arguments
/// impossible rather than merely discouraged — there is no owned byte variant
/// to copy into.
///
/// The lifetime is the duration of one host call and not one instant longer.
#[derive(Debug, Clone, PartialEq)]
pub enum HostArg<'a> {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    /// Borrowed from the engine's UTF-8 rendering of a JS string. The engine
    /// allocated that rendering (§1.2: strings cost one copy no boundary can
    /// remove); Rust does not add a second.
    Str(&'a str),
    /// Borrowed directly from the engine's `ArrayBuffer` storage. No copy.
    Bytes(&'a [u8]),
}

impl<'a> HostArg<'a> {
    pub fn as_str(&self) -> Option<&'a str> {
        match self {
            HostArg::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        match self {
            HostArg::Bytes(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            HostArg::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

/// A **result** crossing back out to the engine.
///
/// Owned, because ownership is what transfers: a `Bytes` result becomes the
/// backing store of a JavaScript `ArrayBuffer` directly, and the engine frees
/// it through the boundary when the object is collected (§1.2, outbound). There
/// is deliberately no `Object` or `Json` variant: a boundary that serializes
/// has traded parse time for call time, which §1.1 forbids.
#[derive(Debug, Clone, PartialEq)]
pub enum HostValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Bytes(Vec<u8>),
}

impl HostValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            HostValue::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            HostValue::Bytes(value) => Some(value),
            _ => None,
        }
    }
}

/// What a refused or failed host call reports.
///
/// The taxonomy is Rust-owned so failures are identical on every platform and
/// every engine (LLP 0057 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The operation was not admitted by the caller's grants. Carries no detail
    /// about what *would* have been admitted.
    Denied { capability: &'static str },
    /// The arguments did not match the operation's contract.
    InvalidArgument(String),
    /// The operation's own error, e.g. malformed base64 or an unparseable URL.
    Failed(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Denied { capability } => write!(f, "denied: {capability}"),
            HostError::InvalidArgument(detail) => write!(f, "invalid argument: {detail}"),
            HostError::Failed(detail) => write!(f, "{detail}"),
        }
    }
}

/// Check an operation against the grants the calling binding carries.
///
/// This is the whole of LLP 0060 §4: one chokepoint, taking the grant as an
/// argument because authority is carried rather than inferred. There is
/// deliberately no way to call this without supplying a `GrantSet` — no ambient
/// lookup, no stack walk, no thread-local.
pub fn admit(grants: &GrantSet, operation: &Operation) -> Result<(), HostError> {
    if grants.permits(operation) {
        return Ok(());
    }
    Err(HostError::Denied {
        capability: capability_name(operation),
    })
}

fn capability_name(operation: &Operation) -> &'static str {
    match operation {
        Operation::Fetch { .. } => "net.fetch",
        Operation::WebSocket { .. } => "net.websocket",
        Operation::FsRead { .. } => "fs.read",
        Operation::FsWrite { .. } => "fs.write",
        Operation::EnvRead { .. } => "env.read",
        Operation::SqliteOpen { .. } => "sqlite.open",
        Operation::SecretKeep { .. } => "secret.keep",
        Operation::StorageKv { .. } => "storage.kv",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grant::{Grant, Origin};

    #[test]
    fn admit_reports_the_capability_that_was_missing() {
        let grants = GrantSet::none();
        let op = Operation::Fetch {
            origin: Origin::new("https", "example.com", 443),
        };
        assert_eq!(
            admit(&grants, &op),
            Err(HostError::Denied {
                capability: "net.fetch"
            })
        );
    }

    #[test]
    fn admit_passes_a_granted_operation() {
        let origin = Origin::new("https", "example.com", 443);
        let grants = GrantSet::none().with(Grant::Fetch(origin.clone()));
        assert_eq!(admit(&grants, &Operation::Fetch { origin }), Ok(()));
    }

    #[test]
    fn a_denial_does_not_leak_what_would_have_been_admitted() {
        let grants =
            GrantSet::none().with(Grant::Fetch(Origin::new("https", "secret.internal", 443)));
        let denied = admit(
            &grants,
            &Operation::Fetch {
                origin: Origin::new("https", "example.com", 443),
            },
        )
        .unwrap_err();
        assert_eq!(format!("{denied}"), "denied: net.fetch");
    }
}
