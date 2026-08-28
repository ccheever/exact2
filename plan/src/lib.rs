//! The plan format.
//!
//! @ref LLP 1004 D2 (one table authority) / D3 (the refusal tuple)
//! @ref LLP 0485 (the flat plan; research)
//!
//! A plan is the compiled form of a Contract program: relational tables
//! (slots, derives, resources, actions, timers, nodes, bindings, handlers,
//! regions) plus three pools (strings, bytecode, data). Nothing walks it; the
//! runner indexes into it. Everything about its shape is declared once in
//! `tables/format.json` and generated here by `build.rs`:
//!
//! - the row structs and the [`Plan`] container ([`generated`]);
//! - the canonical encoder and the validating decoder — **loading is a
//!   validation pass, never trusted indexing** (0485 §3.4): every index, range,
//!   and code reference is checked before a decoded plan is returned;
//! - the expression VM's [`Opcode`] set and the [`Stdlib`] roster;
//! - [`FORMAT_DIGEST`], which every encoded plan carries and every reader
//!   compares.
//!
//! This crate depends on nothing and declares no kernel vocabulary: plan rows
//! carry the kernel's ordinals as numbers, and `exact-kernel` gives them
//! meaning in the compiler and the runner.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod asm;
pub mod builder;
pub mod bytes;
pub mod value;

/// Generated from `tables/format.json`.
#[allow(missing_docs, clippy::all)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/format.rs"));
}

pub use generated::*;
pub use value::Value;

/// Interned string index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StrId(pub u32);

/// A bytecode range in the code pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Code {
    /// Byte offset.
    pub offset: u32,
    /// Byte length.
    pub len: u32,
}

/// A byte range in the data pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bytes {
    /// Byte offset.
    pub offset: u32,
    /// Byte length.
    pub len: u32,
}

/// Why a plan (or a value) was refused. Nothing was adopted.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    Truncated {
        needed: usize,
        available: usize,
    },
    BadMagic,
    UnsupportedVersion(u32),
    FormatDigestMismatch {
        expected: u64,
        actual: u64,
    },
    BadCount(u32),
    BadUtf8,
    TrailingBytes(usize),
    UnknownEnum {
        table: &'static str,
        field: &'static str,
        value: u8,
    },
    BadReference {
        table: &'static str,
        row: u32,
        field: &'static str,
    },
    BadCode {
        table: &'static str,
        row: u32,
        field: &'static str,
        error: CodeError,
    },
    UnknownValueTag(u8),
    NonFiniteValue,
    ValueTooDeep,
    /// A region's arm count does not match its kind (`when`/`match` 2, `each` 1).
    RegionArms {
        region: u32,
        kind: RegionKind,
        arms: u32,
    },
    /// An arm's back-reference names a region that does not own it.
    ArmOwner {
        arm: u32,
    },
    /// A timer with a zero interval would never make progress.
    ZeroInterval {
        timer: u32,
    },
}

/// Why a code range was refused.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeError {
    Empty,
    Truncated {
        pc: usize,
    },
    UnknownOpcode {
        pc: usize,
        byte: u8,
    },
    NonFinite {
        pc: usize,
    },
    BadIndex {
        pc: usize,
        table: &'static str,
        index: u32,
    },
    BadEnum {
        pc: usize,
        name: &'static str,
        value: u8,
    },
    NoReturn,
    /// A jump that is backward or not on an instruction boundary.
    BadJump {
        pc: usize,
        target: u32,
    },
}

impl CodeError {
    /// Attach the row this code belongs to.
    pub fn at(self, table: &'static str, row: u32, field: &'static str) -> PlanError {
        PlanError::BadCode {
            table,
            row,
            field,
            error: self,
        }
    }
}

impl Plan {
    /// The rules the row codecs cannot express: region topology and timer
    /// progress. Called by the generated `validate`, so every loaded or built
    /// plan has passed it.
    pub fn validate_semantics(&self) -> Result<(), PlanError> {
        for (i, r) in self.regions.iter().enumerate() {
            let want = match r.kind {
                RegionKind::When | RegionKind::Match => 2,
                RegionKind::Each => 1,
            };
            if r.arms.len != want {
                return Err(PlanError::RegionArms {
                    region: i as u32,
                    kind: r.kind,
                    arms: r.arms.len,
                });
            }
            for a in r.arms.iter() {
                if self.arm(a).region.0 as usize != i {
                    return Err(PlanError::ArmOwner { arm: a.0 });
                }
            }
        }
        for (i, t) in self.timers.iter().enumerate() {
            if t.interval_ms == 0 {
                return Err(PlanError::ZeroInterval { timer: i as u32 });
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PlanError {}
