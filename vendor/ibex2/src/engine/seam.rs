//! The engine seam.
//!
//! LLP 0058 states what a JavaScript engine must provide to sit in the slot
//! below a Rust standard library. This module is where that contract becomes
//! a type. Hermes is the default and the only planned implementation; the
//! trait exists so engine assumptions cannot leak into the standard library,
//! not because swapping will be cheap.
//!
//! **This is deliberately a sketch.** The one method that decides whether the
//! whole program is viable — how a Rust future completing off-thread resolves
//! into the engine's job queue with correct ordering — is LLP 0058 OQ1 and is
//! not specified yet. Writing a plausible-looking signature for it now would
//! be inventing an answer to the hardest open question in the design. The
//! spike answers it against a real engine first; the trait follows.
//!
//! @ref LLP 0058#1-what-an-engine-must-provide — the four requirements below

/// What an engine must supply for a Rust standard library to sit on top of it.
///
/// The four requirements of LLP 0058 §1, in decreasing order of how much they
/// constrain the choice:
///
/// 1. **Ahead-of-time bytecode that is actually shipped.** Bytecode evaluation
///    is flat in graph size where source parse is linear in bytes; an engine
///    without a real AOT story cannot meet a startup budget measured in tens
///    of milliseconds. This dominates every other consideration.
/// 2. **Host functions over primitives and handles.** The entire standard
///    library reaches JavaScript through these. An embedding API that forces
///    serialization at the boundary makes LLP 0057 §3's boundary rule
///    unimplementable.
/// 3. **A job queue that can be interleaved with.** See the module note above.
/// 4. **Enough of the language** for the application tier — a real threshold,
///    lower than total conformance, and not near zero.
pub trait Engine {
    /// Ahead-of-time compiled bytecode, ready to evaluate without parsing.
    type Bytecode;

    /// A value living in the engine's heap.
    type Value;

    /// Whatever the engine reports when evaluation fails.
    type Error;

    /// Evaluate a prepared bytecode module.
    ///
    /// Requirement 1. Source evaluation is deliberately absent: LLP 0060 D4
    /// closes dynamic code at construction, and a runtime that compiles no
    /// source needs no way to ask an engine to.
    fn evaluate(&mut self, bytecode: &Self::Bytecode) -> Result<Self::Value, Self::Error>;
}
