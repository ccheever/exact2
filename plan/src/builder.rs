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
use std::hash::{BuildHasherDefault, Hasher};

/// A plan under construction.
#[derive(Debug, Default)]
pub struct PlanBuilder {
    plan: Plan,
    interned: HashMap<String, StrId, BuildHasherDefault<BodyHasher>>,
    /// Every code body appended so far, by content: an identical body is
    /// shared. Each reference is validated on its own (`check_code`), so a
    /// shared body is indistinguishable from a copy to every reader.
    bodies: HashMap<Vec<u8>, Code, BuildHasherDefault<BodyHasher>>,
}

/// A multiply-rotate hash for the pools' dedup tables: code bodies this
/// builder assembled and the plan's own strings, so no key is adversarial
/// and SipHash's cost buys nothing.
#[derive(Default)]
struct BodyHasher(u64);

impl Hasher for BodyHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.mix(u64::from_le_bytes(chunk.try_into().unwrap()));
        }
        let mut tail = [0u8; 8];
        tail[..chunks.remainder().len()].copy_from_slice(chunks.remainder());
        self.mix(u64::from_le_bytes(tail) ^ (bytes.len() as u64) << 56);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl BodyHasher {
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
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
            interned: HashMap::default(),
            bodies: HashMap::with_capacity_and_hasher(256, Default::default()),
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
        PlanBuilder {
            plan,
            interned,
            bodies: HashMap::default(),
        }
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

    /// Append an assembled code body, or return the identical one already
    /// in the pool.
    pub fn code(&mut self, asm: Asm) -> Code {
        let bytes = asm.finish();
        if let Some(code) = self.bodies.get(&bytes) {
            return *code;
        }
        let code = Code {
            offset: self.plan.code.len() as u32,
            len: bytes.len() as u32,
        };
        self.plan.code.extend_from_slice(&bytes);
        self.bodies.insert(bytes, code);
        code
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
            Value::HeapStr(_) | Value::InlineStr(_) => {
                let id = self.str(v.as_str().unwrap_or_default());
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
        self.plan.data.to_mut().extend_from_slice(&bytes);
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

    /// A `@keyframes` rule, its body as CSS text (LLP 1055 D5).
    pub fn keyframes(&mut self, name: &str, css: &str) -> KeyframesId {
        let (name, css) = (self.str(name), self.str(css));
        self.plan.keyframes.push(KeyframesRow { name, css });
        KeyframesId(self.plan.keyframes.len() as u32 - 1)
    }

    /// An ordered CSS font fallback list (one to 64 members).
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
            late: false,
        });
        SlotsId(self.plan.slots.len() as u32 - 1)
    }

    /// A route in declaration order. @ref LLP 1038 D2.
    pub fn route(
        &mut self,
        name: &str,
        pattern: &str,
        parent: Option<RoutesId>,
        tab: bool,
        notfound: bool,
    ) -> RoutesId {
        let name = self.str(name);
        let pattern = self.str(pattern);
        self.plan.routes.push(RoutesRow {
            name,
            pattern,
            parent,
            tab,
            notfound,
            render: RenderPolicy::Client,
            activate: ActivatePolicy::Inferred,
            paint: PaintPolicy::Settled,
            pages: StrId(0),
            pages_args: Bytes { offset: 0, len: 0 },
        });
        let id = RoutesId(self.plan.routes.len() as u32 - 1);
        self.plan.routes[id.0 as usize].pages = self.str("");
        self.plan.routes[id.0 as usize].pages_args = self.no_data();
        id
    }

    /// The source that lists a parameterized route's pages, with its
    /// arguments (LLP 1048.000 D2).
    pub fn set_route_pages(&mut self, id: RoutesId, source: &str, args: &[Value]) {
        let source = self.str(source);
        let args = self.data(&Value::list(args.to_vec()));
        let row = &mut self.plan.routes[id.0 as usize];
        row.pages = source;
        row.pages_args = args;
    }

    /// A route's declared render, activation and first-paint policies (LLP
    /// 1048.003 D5, LLP 1048.005); a route declares none is `client`,
    /// activation inferred, painted settled.
    pub fn set_route_policy(
        &mut self,
        id: RoutesId,
        render: RenderPolicy,
        activate: ActivatePolicy,
        paint: PaintPolicy,
    ) {
        let row = &mut self.plan.routes[id.0 as usize];
        row.render = render;
        row.activate = activate;
        row.paint = paint;
    }

    /// The root slot filled with the launch location. @ref LLP 1038 D2/D5.
    pub fn set_router(&mut self, slot: SlotsId) {
        self.plan.router = Some(slot);
    }

    /// One locale's texts, `(key, text)` sorted by key; the first locale is
    /// the base. @ref LLP 1060 D3.
    pub fn locale(&mut self, name: &str, rtl: bool, texts: &[(&str, &str)]) -> LocalesId {
        let start = self.plan.texts.len() as u32;
        for (key, text) in texts {
            let key = self.str(key);
            let text = self.str(text);
            self.plan.texts.push(TextsRow { key, text });
        }
        let name = self.str(name);
        self.plan.locales.push(LocalesRow {
            name,
            rtl,
            texts: TextsRange {
                start,
                len: texts.len() as u32,
            },
        });
        LocalesId(self.plan.locales.len() as u32 - 1)
    }

    /// The root slot holding the resolved locale. @ref LLP 1060 D4.
    pub fn set_locale(&mut self, slot: SlotsId) {
        self.plan.locale = Some(slot);
    }

    /// A derive.
    pub fn derive(&mut self, name: &str, ty: TypesId, body: Code) -> DerivesId {
        let name = self.str(name);
        self.plan.derives.push(DerivesRow { name, ty, body });
        DerivesId(self.plan.derives.len() as u32 - 1)
    }

    /// A resource: a data source name, argument expressions, its shape, and
    /// an optional compiled initial value. With non-empty arguments, also call
    /// `set_resource_initial_args` before `finish` (LLP 1038 D5).
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
        let initial_args = if initial.len > 0 && args.len == 0 {
            self.data(&Value::list(Vec::new()))
        } else {
            self.no_data()
        };
        let name = self.str(name);
        let source = self.str(source);
        self.plan.resources.push(ResourcesRow {
            name,
            source,
            args,
            context: 0,
            ty,
            initial,
            initial_args,
            reader: false,
            placeholder: None,
            placeholder_value: self.no_data(),
        });
        ResourcesId(self.plan.resources.len() as u32 - 1)
    }

    /// A mutation (LLP 1016): the `option<T>` slot its reply fills, and `T`;
    /// the source is named at each `send`.
    pub fn mutation(&mut self, name: &str, slot: SlotsId, ty: TypesId) -> MutationsId {
        let name = self.str(name);
        self.plan.mutations.push(MutationsRow {
            name,
            slot,
            ty,
            refreshes: MutationRefreshesRange { start: 0, len: 0 },
            then: None,
        });
        MutationsId(self.plan.mutations.len() as u32 - 1)
    }

    /// The action run after each of `mutation`'s answers lands.
    pub fn set_mutation_then(&mut self, mutation: MutationsId, action: ActionsId) {
        self.plan.mutations[mutation.0 as usize].then = Some(action);
    }

    /// The resources a send to `mutation` refreshes (LLP 1054.000.000 D1),
    /// once its resources exist.
    pub fn mutation_refreshes(&mut self, mutation: MutationsId, resources: &[ResourcesId]) {
        let start = self.plan.mutation_refreshes.len() as u32;
        for r in resources {
            self.plan
                .mutation_refreshes
                .push(MutationRefreshesRow { resource: *r });
        }
        self.plan.mutations[mutation.0 as usize].refreshes = MutationRefreshesRange {
            start,
            len: resources.len() as u32,
        };
    }

    /// A data-source signature (LLP 1027 D2): the parameter types and the
    /// result type every `resource` and `send` naming `name` agree on.
    pub fn source(&mut self, name: &str, params: &[TypesId], ty: TypesId) -> SourcesId {
        let start = self.plan.source_params.len() as u32;
        for p in params {
            self.plan.source_params.push(SourceParamsRow { ty: *p });
        }
        let name = self.str(name);
        self.plan.sources.push(SourcesRow {
            name,
            params: SourceParamsRange {
                start,
                len: params.len() as u32,
            },
            ty,
        });
        SourcesId(self.plan.sources.len() as u32 - 1)
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

    /// A timer that dispatches `action` at boot+`interval_ms`, then every
    /// `interval_ms` — or, when `once`, never again.
    pub fn timer(&mut self, interval_ms: u32, action: ActionsId, once: bool) -> TimersId {
        self.plan.timers.push(TimersRow {
            interval_ms,
            action,
            once,
            frame: false,
        });
        TimersId(self.plan.timers.len() as u32 - 1)
    }

    /// A frame task (LLP 1073): dispatches `action` once per presented
    /// frame, or per virtual frame on a seek.
    pub fn frame_timer(&mut self, action: ActionsId) -> TimersId {
        self.plan.timers.push(TimersRow {
            interval_ms: 0,
            action,
            once: false,
            frame: true,
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
    pub fn surface(&mut self, name: &str, args: &[(&str, Code)]) -> SurfacesId {
        let start = self.plan.surface_args.len() as u32;
        for (name, expr) in args {
            let name = self.str(name);
            self.plan
                .surface_args
                .push(SurfaceArgsRow { name, expr: *expr });
        }
        let args = SurfaceArgsRange {
            start,
            len: args.len() as u32,
        };
        let name = self.str(name);
        let mode = if args.len != 0
            && self
                .plan
                .str(self.plan.surface_args[args.start as usize].name)
                .is_empty()
        {
            SurfaceArgsMode::Positional
        } else {
            SurfaceArgsMode::Named
        };
        self.plan.surfaces.push(SurfacesRow { name, args, mode });
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

    /// Make a slot owned by an arm's instance — an `each` row, a `when` or
    /// `match` arm: one value per instance, created with it, read and
    /// written through its frame (LLP 1017 P4c).
    pub fn set_slot_owner(&mut self, id: SlotsId, arm: ArmsId) {
        self.plan.slots[id.0 as usize].owner = Some(arm);
    }

    /// Make a root slot initialized at the boot render, after boot
    /// settlement: a child's state used outside every region.
    pub fn set_slot_late(&mut self, id: SlotsId) {
        self.plan.slots[id.0 as usize].late = true;
    }

    /// Replace a derive's body.
    pub fn set_derive_body(&mut self, id: DerivesId, body: Code) {
        self.plan.derives[id.0 as usize].body = body;
    }

    /// Replace a resource's arguments.
    pub fn set_resource_args(&mut self, id: ResourcesId, args: ArgsRange) {
        self.plan.resources[id.0 as usize].args = args;
    }

    /// Set the trailing request-context count, after installing arguments.
    /// @ref LLP 1027.005 D6 — zero by default; validated against `args.len`.
    pub fn set_resource_context(&mut self, id: ResourcesId, context: u16) {
        self.plan.resources[id.0 as usize].context = context;
    }

    /// A declared `else empty(…)`'s constant (LLP 1054.000.002 D3).
    pub fn set_resource_placeholder_value(&mut self, id: ResourcesId, v: &Value) {
        let bytes = self.data(v);
        self.plan.resources[id.0 as usize].placeholder_value = bytes;
    }

    /// Replace a resource's compiled initial value.
    pub fn set_resource_initial(&mut self, id: ResourcesId, v: &Value) {
        let bytes = self.data(v);
        self.plan.resources[id.0 as usize].initial = bytes;
    }

    /// The evaluated arguments beside a compiled value, encoded as a list.
    /// @ref LLP 1038 D5 — bake's cache key, including an empty argument list.
    pub fn set_resource_initial_args(&mut self, id: ResourcesId, args: &[Value]) {
        let bytes = self.data(&Value::list(args.to_vec()));
        self.plan.resources[id.0 as usize].initial_args = bytes;
    }

    /// The row whose value shows while `id`'s source hasn't answered (LLP
    /// 1048.003 D6).
    pub fn set_resource_placeholder(&mut self, id: ResourcesId, placeholder: ResourcesId) {
        self.plan.resources[id.0 as usize].placeholder = Some(placeholder);
    }

    /// Mark a resource as one the bake found consulting the store (LLP 1027
    /// D4): its compiled value is the empty-store placeholder.
    pub fn set_resource_reader(&mut self, id: ResourcesId, reader: bool) {
        self.plan.resources[id.0 as usize].reader = reader;
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
