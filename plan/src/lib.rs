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
pub mod shared;
pub mod strings;
pub mod value;

/// Generated from `tables/format.json`.
#[allow(missing_docs, clippy::all)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/format.rs"));
}

pub use generated::*;
pub use shared::{HeapStr, InlineStr, Items, Str};
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
    /// A frame timer (LLP 1073) carries no interval and repeats.
    FrameTimer {
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
    /// A family repeats one static `(weight, italic)` coordinate.
    DuplicateFace {
        family: u32,
        face: u32,
        weight: u16,
        italic: bool,
    },
    /// A CSS fallback stack requires one to 64 members.
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
    /// A node or region nests deeper than [`MAX_SITE_DEPTH`], or its parents
    /// form a cycle; realization recurses once per level.
    SiteTooDeep {
        table: &'static str,
        row: u32,
    },
    /// A type contains itself; every type-directed walk must end.
    TypeCycle {
        ty: u32,
    },
}

/// How deeply sites may nest: nodes inside nodes and region arms.
pub const MAX_SITE_DEPTH: usize = 256;

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
            // @ref LLP 1048.000 D2 — a pages source's arguments are a list.
            let listed = matches!(
                Value::from_bytes(self.bytes(route.pages_args)),
                Ok(Value::List(_))
            );
            if self.str(route.pages).is_empty() != (route.pages_args.len == 0)
                || (route.pages_args.len > 0 && !listed)
            {
                return Err(bad("pages_args"));
            }
        }
        if let Some(id) = self.router {
            let slot = self.slot(id);
            if self.type_(slot.ty).kind != TypeKind::Record || slot.owner.is_some() || slot.late {
                return Err(PlanError::BadReference {
                    table: "header",
                    row: 0,
                    field: "router",
                });
            }
        }
        // A late slot is a root slot: an owned one is its instance's.
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.late && (slot.owner.is_some() || self.locale == Some(SlotsId(i as u32))) {
                return Err(PlanError::BadReference {
                    table: "slots",
                    row: i as u32,
                    field: "late",
                });
            }
        }
        // @ref LLP 1048.003 D6 — a placeholder row is another row of the
        // same type with no placeholder of its own.
        for (i, resource) in self.resources.iter().enumerate() {
            let Some(p) = resource.placeholder else {
                continue;
            };
            let row = &self.resources[p.0 as usize];
            if p.0 as usize == i || row.ty != resource.ty || row.placeholder.is_some() {
                return Err(PlanError::BadReference {
                    table: "resources",
                    row: i as u32,
                    field: "placeholder",
                });
            }
        }
        // @ref LLP 1038 D5 — compiled data and its argument list form one cache entry.
        for (i, resource) in self.resources.iter().enumerate() {
            // @ref LLP 1027.005 D6 — the identity is the remaining prefix.
            if u32::from(resource.context) > resource.args.len {
                return Err(PlanError::BadReference {
                    table: "resources",
                    row: i as u32,
                    field: "context",
                });
            }
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
        for (i, surface) in self.surfaces.iter().enumerate() {
            let expected = if surface.args.len != 0
                && self
                    .str(self.surface_arg(surface.args.iter().next().unwrap()).name)
                    .is_empty()
            {
                SurfaceArgsMode::Positional
            } else {
                SurfaceArgsMode::Named
            };
            if surface.mode != expected {
                return Err(PlanError::BadReference {
                    table: "surfaces",
                    row: i as u32,
                    field: "mode",
                });
            }
            // A surface's argument names are few: a scan, not a tree's code.
            let mut names: Vec<&str> = Vec::new();
            let mut positional = false;
            for id in surface.args.iter() {
                let name = self.str(self.surface_arg(id).name);
                positional |= name.is_empty();
                let repeated = !name.is_empty() && names.contains(&name);
                if !name.is_empty() && !repeated {
                    names.push(name);
                }
                if repeated || (positional && !names.is_empty()) {
                    return Err(PlanError::BadReference {
                        table: "surfaces",
                        row: i as u32,
                        field: "args",
                    });
                }
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
            if t.frame && (t.interval_ms != 0 || t.once) {
                return Err(PlanError::FrameTimer { timer: i as u32 });
            }
            if !t.frame && t.interval_ms == 0 {
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
            // A family's faces are few: a scan, not a hash table's code.
            let mut coordinates = Vec::new();
            for face_id in family.faces.iter() {
                let face = self.face(face_id);
                let coordinate = (face.weight, face.italic);
                if coordinates.contains(&coordinate) {
                    return Err(PlanError::DuplicateFace {
                        family: i as u32,
                        face: face_id.0,
                        weight: face.weight,
                        italic: face.italic,
                    });
                }
                coordinates.push(coordinate);
            }
        }
        if self.stacks.len() > u16::MAX as usize + 1 {
            return Err(PlanError::TooManyStacks(self.stacks.len()));
        }
        for (i, stack) in self.stacks.iter().enumerate() {
            if !(1..=64).contains(&stack.members.len) {
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
        self.validate_texts()?;
        self.validate_site_depth()?;
        self.validate_acyclic_types()
    }

    /// Every node and region sits at most [`MAX_SITE_DEPTH`] levels below
    /// the root, following node parents and arm regions (no recursion here).
    fn validate_site_depth(&self) -> Result<(), PlanError> {
        // Sites: nodes first, then regions. 0 = unknown, u32::MAX = on the path.
        let first_region = self.nodes.len();
        let mut depth = vec![0u32; first_region + self.regions.len()];
        let up = |site: usize| -> Option<usize> {
            let (parent, arm) = match site.checked_sub(first_region) {
                None => (self.nodes[site].parent, self.nodes[site].arm),
                Some(r) => (self.regions[r].parent, self.regions[r].arm),
            };
            match (parent, arm) {
                (Some(node), _) => Some(node.0 as usize),
                (None, Some(arm)) => Some(first_region + self.arm(arm).region.0 as usize),
                (None, None) => None,
            }
        };
        let refuse = |site: usize| match site.checked_sub(first_region) {
            None => PlanError::SiteTooDeep {
                table: "nodes",
                row: site as u32,
            },
            Some(r) => PlanError::SiteTooDeep {
                table: "regions",
                row: r as u32,
            },
        };
        let mut path = Vec::new();
        for start in 0..depth.len() {
            let mut site = Some(start);
            while let Some(s) = site.filter(|s| depth[*s] == 0) {
                if path.len() >= MAX_SITE_DEPTH {
                    return Err(refuse(start));
                }
                depth[s] = u32::MAX;
                path.push(s);
                site = up(s);
            }
            let mut below = match site {
                None => 0,
                Some(s) if depth[s] == u32::MAX => return Err(refuse(s)),
                Some(s) => depth[s],
            };
            while let Some(s) = path.pop() {
                below += 1;
                if below as usize > MAX_SITE_DEPTH {
                    return Err(refuse(s));
                }
                depth[s] = below;
            }
        }
        Ok(())
    }

    /// No type reaches itself through an element or a field.
    fn validate_acyclic_types(&self) -> Result<(), PlanError> {
        // 0 unvisited, 1 on the path, 2 done; an explicit stack of (type, next edge).
        let mut state = vec![0u8; self.types.len()];
        for start in 0..self.types.len() {
            if state[start] != 0 {
                continue;
            }
            let mut stack = vec![(start, 0usize)];
            state[start] = 1;
            while let Some((ty, edge)) = stack.last_mut() {
                let row = &self.types[*ty];
                let edges = row.elem.map_or(0, |_| 1) + row.fields.len as usize;
                if *edge == edges {
                    state[*ty] = 2;
                    stack.pop();
                    continue;
                }
                let next = match (row.elem, *edge) {
                    (Some(elem), 0) => elem.0 as usize,
                    (elem, e) => {
                        let field = row.fields.start as usize + e - usize::from(elem.is_some());
                        self.fields[field].ty.0 as usize
                    }
                };
                *edge += 1;
                match state[next] {
                    0 => {
                        state[next] = 1;
                        stack.push((next, 0));
                    }
                    1 => return Err(PlanError::TypeCycle { ty: next as u32 }),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// The region whose instance frame holds an owned slot's value: its
    /// owning arm's region. `None` for a root slot.
    pub fn owner_region(&self, slot: &SlotsRow) -> Option<RegionsId> {
        slot.owner.map(|arm| self.arm(arm).region)
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

    /// This plan without its resources' compiled values (`initial` and
    /// `initial_args`), the data pool rebuilt to hold only what its other
    /// rows still name. It is what a data module binds with: a source reads
    /// the plan's declarations there, never the bake's answers, which can be
    /// most of a baked plan's bytes.
    pub fn without_compiled_values(&self) -> Plan {
        let mut plan = self.clone();
        for row in &mut plan.resources {
            row.initial = Bytes::default();
            row.initial_args = Bytes::default();
        }
        let mut data = Vec::new();
        let mut moved = std::collections::HashMap::new();
        plan.each_bytes_mut(&mut |b: &mut Bytes| {
            if b.len == 0 {
                *b = Bytes::default();
                return;
            }
            let offset = *moved.entry((b.offset, b.len)).or_insert_with(|| {
                let at = data.len() as u32;
                data.extend_from_slice(&self.data[b.offset as usize..(b.offset + b.len) as usize]);
                at
            });
            b.offset = offset;
        });
        plan.data = std::borrow::Cow::Owned(data);
        plan
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

/// Sources answered by the runner, with a result shape selected by each reader.
pub const RUNNER_OWNED_SOURCES: &[&str] = &[
    "exactDelivery",
    "exactViewport",
    "exactPage",
    "exactSurface",
    "exactTime",
];

/// Whether the runner answers `name` ([`RUNNER_OWNED_SOURCES`]).
pub fn runner_owned_source(name: &str) -> bool {
    RUNNER_OWNED_SOURCES.contains(&name)
}

impl Plan {
    /// The bit a platform holds in a `hatches` row's mask (LLP
    /// 1075.003.000.001 §4.3), in the order the manifest's names are listed.
    pub fn hatch_platform_bit(platform: &str) -> u16 {
        ["ios", "tvos", "macos", "web", "linux", "windows", "android"]
            .iter()
            .position(|p| *p == platform)
            .map_or(0, |i| 1 << i)
    }

    /// Declare a hatch word and the platforms that handle it, after
    /// lowering: the row comes from `app.json`, not from Contract.
    pub fn add_hatch(&mut self, word: &str, platforms: u16) {
        let id = match self.strings.iter().position(|s| s == word) {
            Some(i) => StrId(i as u32),
            None => {
                self.strings.push(word.to_string());
                StrId(self.strings.len() as u32 - 1)
            }
        };
        self.hatches.push(HatchesRow {
            word: id,
            platforms,
        });
    }

    /// Whether `platform`'s module handles `word`, as the plan says it: true
    /// for a plan that lists no words at all (one baked before the row, or an
    /// app that declares none), since then only the module can say.
    pub fn handles_hatch(&self, word: &str, platform: &str) -> bool {
        if self.hatches.is_empty() {
            return true;
        }
        let bit = Self::hatch_platform_bit(platform);
        self.hatches
            .iter()
            .any(|row| self.str(row.word) == word && row.platforms & bit != 0)
    }
}
