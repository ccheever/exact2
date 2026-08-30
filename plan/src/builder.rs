//! Builds a plan row by row: string interning, code and data pools, and typed
//! row insertion returning validated ids.
//!
//! The compiler's lowering and the hand-built test plans both go through
//! this, so a plan is constructed in exactly one way and `Plan::validate`
//! is the one gate before encoding.

use crate::asm::Asm;
use crate::generated::*;
use crate::value::Value;
use crate::{Bytes, Code, Plan, PlanError, StrId};
use std::collections::HashMap;

/// A plan under construction.
#[derive(Debug, Default)]
pub struct PlanBuilder {
    plan: Plan,
    interned: HashMap<String, StrId>,
}

impl PlanBuilder {
    /// Empty, with the identities every plan must carry.
    pub fn new(kernel_schema_digest: u64, compiler_identity: u64) -> PlanBuilder {
        let mut builder = PlanBuilder {
            plan: Plan {
                kernel_schema_digest,
                compiler_identity,
                ..Plan::default()
            },
            interned: HashMap::new(),
        };
        // These eight distinct stack ids are part of the plan vocabulary.
        // A host may resolve several of them to one installed face; their
        // identities never collapse in the plan (LLP 1019 D3).
        for kind in [
            StackMemberKind::SystemUi,
            StackMemberKind::UiSansSerif,
            StackMemberKind::SansSerif,
            StackMemberKind::UiSerif,
            StackMemberKind::Serif,
            StackMemberKind::UiMonospace,
            StackMemberKind::Monospace,
            StackMemberKind::UiRounded,
        ] {
            builder.font_stack(&[(kind, None)]);
        }
        builder
    }

    /// Continue building from an existing plan (the bake rewrites resource data).
    pub fn from_plan(plan: Plan) -> PlanBuilder {
        let interned = plan
            .strings
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), StrId(i as u32)))
            .collect();
        PlanBuilder { plan, interned }
    }

    /// Intern a string.
    pub fn str(&mut self, s: &str) -> StrId {
        if let Some(id) = self.interned.get(s) {
            return *id;
        }
        let id = StrId(self.plan.strings.len() as u32);
        self.plan.strings.push(s.to_string());
        self.interned.insert(s.to_string(), id);
        id
    }

    /// Append an assembled code body.
    pub fn code(&mut self, asm: Asm) -> Code {
        let bytes = asm.finish();
        let offset = self.plan.code.len() as u32;
        self.plan.code.extend_from_slice(&bytes);
        Code {
            offset,
            len: bytes.len() as u32,
        }
    }

    /// A one-value code body: the value's literal, then `Return`.
    pub fn constant(&mut self, v: &Value) -> Code {
        let mut asm = Asm::new();
        self.push_literal(&mut asm, v);
        self.code(asm)
    }

    fn push_literal(&mut self, asm: &mut Asm, v: &Value) {
        match v {
            Value::Number(n) => {
                asm.number(*n);
            }
            Value::Bool(b) => {
                asm.bool(*b);
            }
            Value::Str(s) => {
                let id = self.str(s);
                asm.str(id);
            }
            Value::Unit => {
                asm.simple(Opcode::Unit);
            }
            Value::Option(None) => {
                asm.simple(Opcode::None);
            }
            Value::Option(Some(inner)) => {
                self.push_literal(asm, inner);
                asm.simple(Opcode::Some);
            }
            Value::List(items) => {
                for item in items.iter() {
                    self.push_literal(asm, item);
                }
                asm.list(items.len() as u32);
            }
            Value::Record(_) => panic!("a record literal needs its type; use Asm::record"),
        }
    }

    /// Append a value to the data pool.
    pub fn data(&mut self, v: &Value) -> Bytes {
        let bytes = v.to_bytes();
        let offset = self.plan.data.len() as u32;
        self.plan.data.extend_from_slice(&bytes);
        Bytes {
            offset,
            len: bytes.len() as u32,
        }
    }

    /// An empty data reference.
    pub fn no_data(&self) -> Bytes {
        Bytes {
            offset: self.plan.data.len() as u32,
            len: 0,
        }
    }

    /// A primitive type row.
    pub fn primitive(&mut self, kind: TypeKind) -> TypesId {
        assert!(matches!(
            kind,
            TypeKind::Number | TypeKind::Bool | TypeKind::String | TypeKind::Unit
        ));
        let name = self.str(kind.name());
        self.push_type(TypesRow {
            kind,
            elem: None,
            fields: FieldsRange::default(),
            name,
        })
    }

    /// `Option<elem>`.
    pub fn option(&mut self, elem: TypesId) -> TypesId {
        let name = self.str("option");
        self.push_type(TypesRow {
            kind: TypeKind::Option,
            elem: Some(elem),
            fields: FieldsRange::default(),
            name,
        })
    }

    /// `List<elem>`.
    pub fn list(&mut self, elem: TypesId) -> TypesId {
        let name = self.str("list");
        self.push_type(TypesRow {
            kind: TypeKind::List,
            elem: Some(elem),
            fields: FieldsRange::default(),
            name,
        })
    }

    /// A record type with named fields in order.
    pub fn record(&mut self, name: &str, fields: &[(&str, TypesId)]) -> TypesId {
        let start = self.plan.fields.len() as u32;
        for (fname, ty) in fields {
            let name = self.str(fname);
            self.plan.fields.push(FieldsRow { name, ty: *ty });
        }
        let name = self.str(name);
        self.push_type(TypesRow {
            kind: TypeKind::Record,
            elem: None,
            fields: FieldsRange {
                start,
                len: fields.len() as u32,
            },
            name,
        })
    }

    fn push_type(&mut self, row: TypesRow) -> TypesId {
        // Structural interning keeps equal types equal by id, which is what
        // `Record` opcodes and `conforms` compare against.
        if let Some(i) = self.plan.types.iter().position(|t| *t == row) {
            return TypesId(i as u32);
        }
        self.plan.types.push(row);
        TypesId(self.plan.types.len() as u32 - 1)
    }

    /// A declared static font family and its faces (LLP 1019 D1–D2).
    pub fn font_family(&mut self, name: &str, faces: &[(&str, u16, bool)]) -> FamiliesId {
        let start = self.plan.faces.len() as u32;
        for (source, weight, italic) in faces {
            let source = self.str(source);
            self.plan.faces.push(FacesRow {
                source,
                weight: *weight,
                italic: *italic,
            });
        }
        let name = self.str(name);
        self.plan.families.push(FamiliesRow {
            name,
            faces: FacesRange {
                start,
                len: faces.len() as u32,
            },
        });
        FamiliesId(self.plan.families.len() as u32 - 1)
    }

    /// An ordered font stack. v1's semantic validator accepts one member;
    /// the range keeps the format additive for authored cascade later.
    pub fn font_stack(&mut self, members: &[(StackMemberKind, Option<FamiliesId>)]) -> StacksId {
        let start = self.plan.stack_members.len() as u32;
        for (kind, family) in members {
            self.plan.stack_members.push(StackMembersRow {
                kind: *kind,
                family: *family,
            });
        }
        self.plan.stacks.push(StacksRow {
            members: StackMembersRange {
                start,
                len: members.len() as u32,
            },
        });
        StacksId(self.plan.stacks.len() as u32 - 1)
    }

    /// A state slot.
    pub fn slot(&mut self, name: &str, ty: TypesId, init: Code) -> SlotsId {
        let name = self.str(name);
        self.plan.slots.push(SlotsRow {
            name,
            ty,
            init,
            owner: None,
        });
        SlotsId(self.plan.slots.len() as u32 - 1)
    }

    /// A derive.
    pub fn derive(&mut self, name: &str, ty: TypesId, body: Code) -> DerivesId {
        let name = self.str(name);
        self.plan.derives.push(DerivesRow { name, ty, body });
        DerivesId(self.plan.derives.len() as u32 - 1)
    }

    /// A resource: a data source name, argument expressions, its shape, and
    /// an optional compiled initial value.
    pub fn resource(
        &mut self,
        name: &str,
        source: &str,
        args: &[Code],
        ty: TypesId,
        initial: Option<&Value>,
    ) -> ResourcesId {
        let args = self.args(args);
        let initial = match initial {
            Some(v) => self.data(v),
            None => self.no_data(),
        };
        let name = self.str(name);
        let source = self.str(source);
        self.plan.resources.push(ResourcesRow {
            name,
            source,
            args,
            ty,
            initial,
        });
        ResourcesId(self.plan.resources.len() as u32 - 1)
    }

    /// A mutation (LLP 1016): the `option<T>` slot its reply fills, and `T`;
    /// the source is named at each `send`.
    pub fn mutation(&mut self, name: &str, slot: SlotsId, ty: TypesId) -> MutationsId {
        let name = self.str(name);
        self.plan.mutations.push(MutationsRow { name, slot, ty });
        MutationsId(self.plan.mutations.len() as u32 - 1)
    }

    /// A run of argument expressions.
    pub fn args(&mut self, exprs: &[Code]) -> ArgsRange {
        let start = self.plan.args.len() as u32;
        for expr in exprs {
            self.plan.args.push(ArgsRow { expr: *expr });
        }
        ArgsRange {
            start,
            len: exprs.len() as u32,
        }
    }

    /// An action: parameters, the slots it writes, and its body.
    pub fn action(
        &mut self,
        name: &str,
        params: &[(&str, TypesId)],
        writes: &[SlotsId],
        body: Code,
    ) -> ActionsId {
        let pstart = self.plan.params.len() as u32;
        for (pname, ty) in params {
            let name = self.str(pname);
            self.plan.params.push(ParamsRow { name, ty: *ty });
        }
        let wstart = self.plan.writes.len() as u32;
        for slot in writes {
            self.plan.writes.push(WritesRow { slot: *slot });
        }
        let name = self.str(name);
        self.plan.actions.push(ActionsRow {
            name,
            params: ParamsRange {
                start: pstart,
                len: params.len() as u32,
            },
            writes: WritesRange {
                start: wstart,
                len: writes.len() as u32,
            },
            body,
        });
        ActionsId(self.plan.actions.len() as u32 - 1)
    }

    /// A timer that dispatches `action` every `interval_ms`.
    pub fn timer(&mut self, interval_ms: u32, action: ActionsId) -> TimersId {
        self.plan.timers.push(TimersRow {
            interval_ms,
            action,
        });
        TimersId(self.plan.timers.len() as u32 - 1)
    }

    /// A region under `parent` (or an arm root when `parent` is `None`),
    /// with `arm_count` arms allocated. Returns the region and its arms.
    #[allow(clippy::too_many_arguments)] // mirrors the row: kind, site (parent, arm, order), two codes, arms
    pub fn region(
        &mut self,
        kind: RegionKind,
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        order: u32,
        subject: Code,
        key: Code,
        arm_count: u32,
    ) -> (RegionsId, Vec<ArmsId>) {
        let region = RegionsId(self.plan.regions.len() as u32);
        let start = self.plan.arms.len() as u32;
        let mut arms = Vec::with_capacity(arm_count as usize);
        for _ in 0..arm_count {
            self.plan.arms.push(ArmsRow { region });
            arms.push(ArmsId(self.plan.arms.len() as u32 - 1));
        }
        self.plan.regions.push(RegionsRow {
            kind,
            parent,
            arm,
            order,
            subject,
            key,
            arms: ArmsRange {
                start,
                len: arm_count,
            },
        });
        (region, arms)
    }

    /// A canvas node's surface binding: the surface's name in the app's GPU
    /// module and its argument expressions (LLP 1009 D3).
    pub fn surface(&mut self, name: &str, args: &[Code]) -> SurfacesId {
        let args = self.args(args);
        let name = self.str(name);
        self.plan.surfaces.push(SurfacesRow { name, args });
        SurfacesId(self.plan.surfaces.len() as u32 - 1)
    }

    /// A node with its bindings, handlers, and (for a canvas) its surface.
    #[allow(clippy::too_many_arguments)]
    pub fn node(
        &mut self,
        node_type: u8,
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        order: u32,
        bindings: &[BindingsRow],
        handlers: &[(EventKind, ActionsId, &[Code])],
        surface: Option<SurfacesId>,
    ) -> NodesId {
        let bstart = self.plan.bindings.len() as u32;
        self.plan.bindings.extend_from_slice(bindings);
        let hstart = self.plan.handlers.len() as u32;
        for (event, action, args) in handlers {
            let args = self.args(args);
            self.plan.handlers.push(HandlersRow {
                event: *event,
                action: *action,
                args,
            });
        }
        self.plan.nodes.push(NodesRow {
            node_type,
            parent,
            arm,
            order,
            bindings: BindingsRange {
                start: bstart,
                len: bindings.len() as u32,
            },
            handlers: HandlersRange {
                start: hstart,
                len: handlers.len() as u32,
            },
            surface,
        });
        NodesId(self.plan.nodes.len() as u32 - 1)
    }

    /// Replace a slot's initializer (declared first, filled once every id exists).
    pub fn set_slot_init(&mut self, id: SlotsId, init: Code) {
        self.plan.slots[id.0 as usize].init = init;
    }

    /// Make a slot a row slot of an `each` region: one value per keyed row,
    /// read and written through the row's frame (LLP 1017 P4c).
    pub fn set_slot_owner(&mut self, id: SlotsId, region: RegionsId) {
        self.plan.slots[id.0 as usize].owner = Some(region);
    }

    /// Replace a derive's body.
    pub fn set_derive_body(&mut self, id: DerivesId, body: Code) {
        self.plan.derives[id.0 as usize].body = body;
    }

    /// Replace a resource's arguments.
    pub fn set_resource_args(&mut self, id: ResourcesId, args: ArgsRange) {
        self.plan.resources[id.0 as usize].args = args;
    }

    /// Replace a resource's compiled initial value.
    pub fn set_resource_initial(&mut self, id: ResourcesId, v: &Value) {
        let bytes = self.data(v);
        self.plan.resources[id.0 as usize].initial = bytes;
    }

    /// Replace an action's body.
    pub fn set_action_body(&mut self, id: ActionsId, body: Code) {
        self.plan.actions[id.0 as usize].body = body;
    }

    /// The plan, validated.
    pub fn finish(self) -> Result<Plan, PlanError> {
        self.plan.validate()?;
        Ok(self.plan)
    }

    /// Read access while building.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
}
