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
    /// A mutation's result slot is row-owned, but mutation results are global.
    MutationSlotOwned {
        mutation: u32,
        slot: u32,
    },
    /// A mutation's result slot is not `option<T>` for the mutation's `T`.
    MutationSlotType {
        mutation: u32,
        slot: u32,
    },
    /// A static face weight is outside CSS Fonts' 1–1000 domain.
    FaceWeight {
        face: u32,
        weight: u16,
    },
    /// A face source is not a portable local relative path.
    FaceSource {
        face: u32,
    },
    /// A declared family has no face.
    EmptyFamily {
        family: u32,
    },
    /// A family repeats one static `(weight, italic)` coordinate.
    DuplicateFace {
        family: u32,
        face: u32,
        weight: u16,
        italic: bool,
    },
    /// v1 accepts one member per stack (the table remains a range).
    StackMembers {
        stack: u32,
        members: u32,
    },
    /// A stack member is either a family reference or one generic, not both.
    StackMember {
        member: u32,
    },
    /// The eight generic stacks at ids 0–7 are missing or out of order.
    GenericStacks,
    /// A `u16` kernel row cannot carry this many stack ids.
    TooManyStacks(usize),
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
        // @ref LLP 1038 D2 — declaration order and the one root router slot.
        let mut notfound = false;
        for (i, route) in self.routes.iter().enumerate() {
            let bad = |field| PlanError::BadReference {
                table: "routes",
                row: i as u32,
                field,
            };
            if route.parent.is_some_and(|parent| parent.0 as usize >= i) {
                return Err(bad("parent"));
            }
            if route.notfound && notfound {
                return Err(bad("notfound"));
            }
            notfound |= route.notfound;
        }
        if let Some(id) = self.router {
            let slot = self.slot(id);
            if self.type_(slot.ty).kind != TypeKind::Record || slot.owner.is_some() {
                return Err(PlanError::BadReference {
                    table: "header",
                    row: 0,
                    field: "router",
                });
            }
        }
        // @ref LLP 1038 D5 — compiled data and its argument list form one cache entry.
        for (i, resource) in self.resources.iter().enumerate() {
            let valid = if resource.initial.len == 0 {
                resource.initial_args.len == 0
            } else {
                matches!(Value::from_bytes(self.bytes(resource.initial_args)), Ok(Value::List(args)) if args.len() == resource.args.len as usize)
            };
            if !valid {
                return Err(PlanError::BadReference {
                    table: "resources",
                    row: i as u32,
                    field: "initial_args",
                });
            }
        }
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
        for i in 0..self.mutations.len() {
            self.validate_mutation_slot(MutationsId(i as u32))?;
        }
        for (i, face) in self.faces.iter().enumerate() {
            if !is_portable_asset_path(self.str(face.source)) {
                return Err(PlanError::FaceSource { face: i as u32 });
            }
            if !(1..=1000).contains(&face.weight) {
                return Err(PlanError::FaceWeight {
                    face: i as u32,
                    weight: face.weight,
                });
            }
        }
        for (i, family) in self.families.iter().enumerate() {
            if family.faces.len == 0 {
                return Err(PlanError::EmptyFamily { family: i as u32 });
            }
            let mut coordinates = std::collections::HashSet::new();
            for face_id in family.faces.iter() {
                let face = self.face(face_id);
                if !coordinates.insert((face.weight, face.italic)) {
                    return Err(PlanError::DuplicateFace {
                        family: i as u32,
                        face: face_id.0,
                        weight: face.weight,
                        italic: face.italic,
                    });
                }
            }
        }
        if self.stacks.len() > u16::MAX as usize + 1 {
            return Err(PlanError::TooManyStacks(self.stacks.len()));
        }
        for (i, stack) in self.stacks.iter().enumerate() {
            if stack.members.len != 1 {
                return Err(PlanError::StackMembers {
                    stack: i as u32,
                    members: stack.members.len,
                });
            }
        }
        for (i, member) in self.stack_members.iter().enumerate() {
            let valid = match member.kind {
                StackMemberKind::Family => member.family.is_some(),
                _ => member.family.is_none(),
            };
            if !valid {
                return Err(PlanError::StackMember { member: i as u32 });
            }
        }
        let builtins = [
            StackMemberKind::SystemUi,
            StackMemberKind::UiSansSerif,
            StackMemberKind::SansSerif,
            StackMemberKind::UiSerif,
            StackMemberKind::Serif,
            StackMemberKind::UiMonospace,
            StackMemberKind::Monospace,
            StackMemberKind::UiRounded,
        ];
        if self.stacks.len() < builtins.len()
            || builtins.iter().enumerate().any(|(i, kind)| {
                let stack = &self.stacks[i];
                let member = &self.stack_members[stack.members.start as usize];
                member.kind != *kind || member.family.is_some()
            })
        {
            return Err(PlanError::GenericStacks);
        }
        Ok(())
    }

    /// Validate the slot relation a mutation assignment relies on at runtime.
    pub fn validate_mutation_slot(&self, mutation: MutationsId) -> Result<(), PlanError> {
        let row = self.mutation(mutation);
        let slot = self.slot(row.slot);
        if slot.owner.is_some() {
            return Err(PlanError::MutationSlotOwned {
                mutation: mutation.0,
                slot: row.slot.0,
            });
        }
        let slot_ty = self.type_(slot.ty);
        if slot_ty.kind != TypeKind::Option || slot_ty.elem != Some(row.ty) {
            return Err(PlanError::MutationSlotType {
                mutation: mutation.0,
                slot: row.slot.0,
            });
        }
        Ok(())
    }
}

/// Whether a plan asset source is a portable local relative path.
///
/// Paths use `/`-separated non-empty segments. Schemes, authorities, roots,
/// queries, fragments, parent/current traversal, URL escapes, backslashes,
/// and control characters are refused before any host resolves the source.
pub fn is_portable_asset_path(source: &str) -> bool {
    if source.is_empty()
        || source.starts_with('/')
        || source.contains([':', '?', '#', '%', '\\'])
        || source.bytes().any(|byte| byte < b' ' || byte == 0x7f)
    {
        return false;
    }
    source
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PlanError {}
