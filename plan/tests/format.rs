//! The plan format: canonical bytes round-trip, loading is a validation pass,
//! and values conform to their shapes or are refused.

use exact_plan::asm::Asm;
use exact_plan::builder::PlanBuilder;
use exact_plan::bytes::Reader;
use exact_plan::{
    BindingKind, BindingsRow, Code, CodeError, EventKind, Opcode, Plan, PlanError, RegionKind,
    StackMemberKind, Stdlib, TypeKind, Value, FORMAT_DIGEST,
};

/// A small but complete plan: a counter slot, a derive, a resource, an
/// action with a write, a timer, a root node with a text binding, and a
/// `when` region with two arms.
fn sample() -> Plan {
    let mut b = PlanBuilder::new(0xdead_beef, 0x1234);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let station = b.record("Station", &[("id", string), ("name", string)]);
    let stations = b.list(station);
    let zero = b.constant(&Value::Number(0.0));
    let count = b.slot("count", number, zero);
    let mut body = Asm::new();
    body.load_slot(count).number(2.0).simple(Opcode::Mul);
    let body = b.code(body);
    let doubled = b.derive("doubled", number, body);
    let arg = b.constant(&Value::str("mv"));
    let initial = Value::list(vec![Value::record(vec![
        Value::str("mv"),
        Value::str("Mountain View"),
    ])]);
    let res = b.resource("nearby", "stations_near", &[arg], stations, Some(&initial));
    b.set_resource_initial_args(res, &[Value::str("mv")]);
    let mut inc = Asm::new();
    inc.load_slot(count)
        .number(1.0)
        .simple(Opcode::Add)
        .store_slot(count);
    let inc = b.code(inc);
    let tick = b.action("tick", &[], &[count], inc);
    b.timer(1000, tick, false);
    let mut text = Asm::new();
    text.load_derive(doubled).call(Stdlib::ToString);
    let text = b.code(text);
    let root = b.node(0, None, None, 0, &[], &[], None);
    let _label = b.node(
        1,
        Some(root),
        None,
        0,
        &[BindingsRow {
            kind: BindingKind::Prop,
            id: 0,
            expr: text,
        }],
        &[(EventKind::Press, tick, &[])],
        None,
    );
    let mut cond = Asm::new();
    cond.load_slot(count).number(3.0).simple(Opcode::Gt);
    let cond = b.code(cond);
    let unit = b.constant(&Value::Unit);
    let (_region, arms) = b.region(RegionKind::When, Some(root), None, 1, cond, unit, 2);
    b.node(1, None, Some(arms[0]), 0, &[], &[], None);
    b.finish().unwrap()
}

#[test]
fn a_plan_round_trips_and_its_bytes_are_canonical() {
    let plan = sample();
    let bytes = plan.encode();
    assert_eq!(&bytes[..4], b"EXPL");
    let decoded = Plan::decode(&bytes).unwrap();
    assert_eq!(decoded, plan);
    assert_eq!(decoded.encode(), bytes, "re-encoding is byte-identical");
    assert_eq!(sample().encode(), bytes, "building twice is byte-identical");
}

#[test]
fn font_tables_round_trip_and_validate_their_identity_graph() {
    let mut b = PlanBuilder::new(0xdead_beef, 0x1234);
    let family = b.font_family(
        "Fixture Sans",
        &[
            ("assets/FixtureSans.ttf", 400, false),
            ("assets/FixtureSans-Bold.ttf", 700, false),
        ],
    );
    let stack = b.font_stack(&[(StackMemberKind::Family, Some(family))]);
    let plan = b.finish().unwrap();
    assert_eq!(stack.0, 8, "the eight generic stacks keep their low ids");
    assert_eq!(plan.families.len(), 1);
    assert_eq!(plan.faces.len(), 2);
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);

    let mut duplicate_coordinate = plan.clone();
    duplicate_coordinate.faces[1].weight = 400;
    assert_eq!(
        duplicate_coordinate.validate(),
        Err(PlanError::DuplicateFace {
            family: 0,
            face: 1,
            weight: 400,
            italic: false,
        })
    );

    let mut dangling = plan.clone();
    dangling.stack_members[8].family = Some(exact_plan::FamiliesId(99));
    assert_eq!(
        dangling.validate(),
        Err(PlanError::BadReference {
            table: "stack_members",
            row: 8,
            field: "family",
        })
    );

    let mut not_a_family = plan;
    not_a_family.stack_members[8].kind = StackMemberKind::Serif;
    assert_eq!(
        not_a_family.validate(),
        Err(PlanError::StackMember { member: 8 })
    );
}

#[test]
fn font_sources_are_portable_local_relative_paths() {
    let build = |source: &str| {
        let mut b = PlanBuilder::new(0xdead_beef, 0x1234);
        let family = b.font_family("Fixture Sans", &[(source, 400, false)]);
        b.font_stack(&[(StackMemberKind::Family, Some(family))]);
        b.finish()
    };
    assert!(build("assets/Fixture Sans.ttf").is_ok());
    for source in [
        "",
        "/font.ttf",
        "//host/font.ttf",
        "https://example.invalid/font.ttf",
        "data:font/ttf,bytes",
        "../font.ttf",
        "assets/../font.ttf",
        "./font.ttf",
        "assets/font.ttf?version=1",
        "assets/font.ttf#face",
        "assets\\font.ttf",
        "assets/%2e%2e/font.ttf",
        "assets//font.ttf",
    ] {
        assert_eq!(
            build(source),
            Err(PlanError::FaceSource { face: 0 }),
            "accepted {source:?}"
        );
    }
}

#[test]
fn host_event_kinds_round_trip_through_the_enum_codec() {
    for event in [
        EventKind::Load,
        EventKind::Message,
        EventKind::Contextmenu,
        EventKind::Dblclick,
        EventKind::Swiperight,
    ] {
        let mut plan = sample();
        plan.handlers[0].event = event;
        let decoded = Plan::decode(&plan.encode()).unwrap();
        assert_eq!(decoded.handlers[0].event, event);
    }
}

#[test]
fn loading_is_a_validation_pass() {
    let plan = sample();
    let good = plan.encode();

    // A different format digest is refused before any table is read.
    let mut bad = good.clone();
    bad[8..16].copy_from_slice(&(FORMAT_DIGEST ^ 1).to_le_bytes());
    assert!(matches!(
        Plan::decode(&bad),
        Err(PlanError::FormatDigestMismatch { .. })
    ));

    // Truncation anywhere is a typed refusal, never a partial plan.
    for cut in [0, 3, 20, good.len() / 2, good.len() - 1] {
        assert!(Plan::decode(&good[..cut]).is_err(), "cut at {cut}");
    }

    // Trailing bytes are refused.
    let mut long = good.clone();
    long.push(0);
    assert_eq!(Plan::decode(&long), Err(PlanError::TrailingBytes(1)));

    // A dangling row reference is named by table, row, and field.
    let mut dangling = plan.clone();
    dangling.derives[0].ty = exact_plan::TypesId(999);
    assert_eq!(
        dangling.validate(),
        Err(PlanError::BadReference {
            table: "derives",
            row: 0,
            field: "ty"
        })
    );
    let bytes = dangling.encode();
    assert!(
        Plan::decode(&bytes).is_err(),
        "encoded dangling reference is refused on load"
    );

    // A code body that indexes a missing slot is refused by pc and table.
    let mut bad_code = plan.clone();
    let start = bad_code.code.len() as u32;
    bad_code
        .code
        .extend_from_slice(&[Opcode::LoadSlot as u8, 7, 0, 0, 0, Opcode::Return as u8]);
    bad_code.derives[0].body = Code {
        offset: start,
        len: 6,
    };
    assert_eq!(
        bad_code.validate(),
        Err(PlanError::BadCode {
            table: "derives",
            row: 0,
            field: "body",
            error: CodeError::BadIndex {
                pc: 0,
                table: "slots",
                index: 7
            }
        })
    );

    // A body without a terminating Return is refused.
    let mut no_return = plan.clone();
    let start = no_return.code.len() as u32;
    no_return.code.push(Opcode::Unit as u8);
    no_return.derives[0].body = Code {
        offset: start,
        len: 1,
    };
    assert!(matches!(
        no_return.validate(),
        Err(PlanError::BadCode {
            error: CodeError::NoReturn,
            ..
        })
    ));

    // An unknown opcode byte is refused.
    let mut unknown = plan.clone();
    let start = unknown.code.len() as u32;
    unknown.code.extend_from_slice(&[250, Opcode::Return as u8]);
    unknown.derives[0].body = Code {
        offset: start,
        len: 2,
    };
    assert!(matches!(
        unknown.validate(),
        Err(PlanError::BadCode {
            error: CodeError::UnknownOpcode { pc: 0, byte: 250 },
            ..
        })
    ));
}

#[test]
fn mutations_require_a_global_option_slot_of_their_result_type() {
    let mut b = PlanBuilder::new(0xdead_beef, 0x1234);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let optional_number = b.option(number);
    let none = b.constant(&Value::NONE);
    let slot = b.slot("result", optional_number, none);
    let unit = b.constant(&Value::Unit);
    let (owner, _) = b.region(RegionKind::Each, None, None, 0, unit, unit, 1);
    b.mutation("save", slot, number);
    let valid = b.finish().unwrap();

    let assert_refused = |plan: Plan, error: PlanError| {
        assert_eq!(plan.validate(), Err(error.clone()));
        assert_eq!(
            PlanBuilder::from_plan(plan.clone()).finish(),
            Err(error.clone()),
            "the builder gate must enforce the semantic relation"
        );
        assert_eq!(
            Plan::decode(&plan.encode()),
            Err(error),
            "the decoder gate must enforce the semantic relation"
        );
    };

    let mut wrong_inner = valid.clone();
    wrong_inner.mutations[0].ty = string;
    assert_refused(
        wrong_inner,
        PlanError::MutationSlotType {
            mutation: 0,
            slot: 0,
        },
    );

    let mut non_option = valid.clone();
    non_option.slots[0].ty = number;
    assert_refused(
        non_option,
        PlanError::MutationSlotType {
            mutation: 0,
            slot: 0,
        },
    );

    let mut row_owned = valid;
    row_owned.slots[0].owner = Some(owner);
    assert_refused(
        row_owned,
        PlanError::MutationSlotOwned {
            mutation: 0,
            slot: 0,
        },
    );
}

#[test]
fn values_round_trip_and_conform_to_shapes() {
    let plan = sample();
    let station = exact_plan::TypesId(2);
    let stations = exact_plan::TypesId(3);
    let v = Value::list(vec![
        Value::record(vec![Value::str("a"), Value::str("A")]),
        Value::record(vec![Value::str("b"), Value::str("B")]),
    ]);
    let bytes = v.to_bytes();
    assert_eq!(Value::from_bytes(&bytes).unwrap(), v);
    assert!(v.conforms(&plan, stations));
    assert!(!v.conforms(&plan, station), "a list is not a record");
    let wrong = Value::list(vec![Value::record(vec![
        Value::Number(1.0),
        Value::str("A"),
    ])]);
    assert!(
        !wrong.conforms(&plan, stations),
        "a number where a string field is declared"
    );
    assert!(!Value::some(Value::Number(1.0)).conforms(&plan, exact_plan::TypesId(0)));

    // The resource's compiled initial value decodes from the data pool and conforms.
    let res = plan.resource(exact_plan::ResourcesId(0));
    let initial = Value::from_bytes(plan.bytes(res.initial)).unwrap();
    assert!(initial.conforms(&plan, res.ty));

    // Hostile value bytes are refused, never trusted.
    assert_eq!(Value::from_bytes(&[9]), Err(PlanError::UnknownValueTag(9)));
    assert!(
        Value::from_bytes(&[0, 0, 0, 0, 0, 0, 0, 0xf8, 0x7f]).is_err(),
        "NaN is refused"
    );
    let mut deep = Vec::new();
    deep.resize(100, 5);
    deep.push(3);
    assert_eq!(Value::from_bytes(&deep), Err(PlanError::ValueTooDeep));
    let mut r = Reader::new(&[6, 0xff, 0xff, 0xff, 0x7f]);
    assert!(
        matches!(Value::decode(&mut r), Err(PlanError::BadCount(_))),
        "a huge count is refused before allocation"
    );
}

#[test]
fn the_assembler_resolves_forward_jumps() {
    let mut asm = Asm::new();
    let else_ = asm.label();
    let end = asm.label();
    asm.bool(false)
        .jump_if_false(else_)
        .number(1.0)
        .jump(end)
        .place(else_)
        .number(2.0)
        .place(end);
    let bytes = asm.finish();
    // Bool(0) = 2 bytes; JumpIfFalse = 5; Number = 9; Jump = 5; -> else_ at 21, end at 30.
    assert_eq!(bytes[2], Opcode::JumpIfFalse as u8);
    assert_eq!(u32::from_le_bytes(bytes[3..7].try_into().unwrap()), 21);
    assert_eq!(bytes[16], Opcode::Jump as u8);
    assert_eq!(u32::from_le_bytes(bytes[17..21].try_into().unwrap()), 30);
    assert_eq!(*bytes.last().unwrap(), Opcode::Return as u8);
}

#[test]
fn control_flow_is_forward_only_and_instruction_aligned() {
    let plan = sample();
    // A backward jump (`Jump 0; Return`) would loop forever at boot.
    let mut looping = plan.clone();
    let start = looping.code.len() as u32;
    looping
        .code
        .extend_from_slice(&[Opcode::Jump as u8, 0, 0, 0, 0, Opcode::Return as u8]);
    looping.slots[0].init = Code {
        offset: start,
        len: 6,
    };
    assert!(matches!(
        looping.validate(),
        Err(PlanError::BadCode {
            error: CodeError::BadJump { pc: 0, target: 0 },
            ..
        })
    ));
    // A jump into the middle of an instruction.
    let mut misaligned = plan.clone();
    let start = misaligned.code.len() as u32;
    // Jump 7 lands inside the Number operand (Number is at 5, 9 bytes long).
    let mut body = vec![Opcode::Jump as u8, 7, 0, 0, 0, Opcode::Number as u8];
    body.extend_from_slice(&1.0f64.to_le_bytes());
    body.push(Opcode::Return as u8);
    let len = body.len() as u32;
    misaligned.code.extend_from_slice(&body);
    misaligned.slots[0].init = Code { offset: start, len };
    assert!(matches!(
        misaligned.validate(),
        Err(PlanError::BadCode {
            error: CodeError::BadJump { pc: 0, target: 7 },
            ..
        })
    ));
    // A forward jump to the Return is fine.
    let mut fine = plan.clone();
    let start = fine.code.len() as u32;
    fine.code
        .extend_from_slice(&[Opcode::Jump as u8, 5, 0, 0, 0, Opcode::Return as u8]);
    fine.slots[0].init = Code {
        offset: start,
        len: 6,
    };
    assert_eq!(fine.validate(), Ok(()));
}

#[test]
fn region_topology_and_timer_progress_are_validated() {
    let plan = sample();
    let mut no_arms = plan.clone();
    no_arms.regions[0].arms.len = 0;
    assert!(matches!(
        no_arms.validate(),
        Err(PlanError::RegionArms {
            region: 0,
            kind: RegionKind::When,
            arms: 0
        })
    ));
    let mut stolen = plan.clone();
    stolen.arms[0].region = exact_plan::RegionsId(0);
    stolen.regions.push(exact_plan::RegionsRow {
        kind: RegionKind::Each,
        parent: None,
        arm: None,
        order: 9,
        subject: plan.regions[0].subject,
        key: plan.regions[0].key,
        arms: exact_plan::ArmsRange { start: 0, len: 1 },
    });
    assert!(matches!(
        stolen.validate(),
        Err(PlanError::ArmOwner { arm: 0 })
    ));
    let mut stuck = plan.clone();
    stuck.timers[0].interval_ms = 0;
    assert_eq!(stuck.validate(), Err(PlanError::ZeroInterval { timer: 0 }));
    // A one-shot timer is refused the same way: `after(0, x)` would fire at boot.
    stuck.timers[0].once = true;
    assert_eq!(stuck.validate(), Err(PlanError::ZeroInterval { timer: 0 }));
}

#[test]
fn a_huge_announced_count_reserves_little_before_it_is_refused() {
    // A header announcing 16M strings over a few bytes: refused by count
    // (count > remaining), and never reserved 16M entries first.
    let plan = sample();
    let mut bytes = plan.encode();
    // strings count sits right after the 44-byte header.
    bytes[44..48].copy_from_slice(&(1u32 << 24).to_le_bytes());
    assert!(matches!(Plan::decode(&bytes), Err(PlanError::BadCount(_))));
}

// @ref LLP 1038 D2/D3/D5 — new rows, header link, cache key, and typed roster.
#[test]
fn a_placeholder_is_another_row_of_the_same_type_without_its_own() {
    // @ref LLP 1048.003 D6
    let mut b = PlanBuilder::from_plan(sample());
    let nearby = exact_plan::ResourcesId(0);
    let ty = b.plan().resources[0].ty;
    let other = b.resource("nearby#else", "no_stations", &[], ty, None);
    b.set_resource_placeholder(nearby, other);
    let plan = b.finish().unwrap();
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);
    assert_eq!(plan.resources[0].placeholder, Some(other));
    let refused = |bad: &Plan| {
        matches!(
            bad.validate(),
            Err(PlanError::BadReference {
                table: "resources",
                field: "placeholder",
                ..
            })
        )
    };
    let mut own = plan.clone();
    own.resources[0].placeholder = Some(nearby);
    assert!(refused(&own));
    let mut chained = plan.clone();
    chained.resources[1].placeholder = Some(nearby);
    assert!(refused(&chained));
    let mut typed = plan.clone();
    typed.resources[1].ty = plan.slots[0].ty;
    assert!(refused(&typed));
}

#[test]
fn router_format_round_trips_and_checks_semantic_links() {
    let mut b = PlanBuilder::from_plan(sample());
    let string = b.primitive(TypeKind::String);
    let ty = b.record("Router", &[("tab", string)]);
    let init = b.constant(&Value::Unit);
    let nav = b.slot("nav", ty, init);
    b.set_router(nav);
    let root = b.route("home", "/", None, true, false);
    b.route("thread", "/t/:thread", Some(root), false, false);
    b.route("notfound", "", None, false, true);
    let plan = b.finish().unwrap();
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);
    assert_eq!(plan.router, Some(nav));
    assert_eq!(
        Value::from_bytes(plan.bytes(plan.resources[0].initial_args)).unwrap(),
        Value::list(vec![Value::str("mv")])
    );
    for (parent, field) in [
        (Some(exact_plan::RoutesId(1)), "parent"),
        (Some(exact_plan::RoutesId(2)), "parent"),
    ] {
        let mut bad = plan.clone();
        bad.routes[1].parent = parent;
        assert!(
            matches!(Plan::decode(&bad.encode()), Err(PlanError::BadReference {table:"routes", row:1, field:f}) if f == field)
        );
    }
    let mut bad = plan.clone();
    bad.routes[1].notfound = true;
    assert!(matches!(
        bad.validate(),
        Err(PlanError::BadReference {
            table: "routes",
            field: "notfound",
            ..
        })
    ));
    let mut bad = plan.clone();
    bad.router = Some(exact_plan::SlotsId(999));
    assert!(matches!(
        Plan::decode(&bad.encode()),
        Err(PlanError::BadReference {
            table: "header",
            field: "router",
            ..
        })
    ));
    let mut bad = plan.clone();
    bad.router = Some(exact_plan::SlotsId(0));
    assert!(matches!(
        bad.validate(),
        Err(PlanError::BadReference {
            table: "header",
            field: "router",
            ..
        })
    ));
    let mut bad = plan.clone();
    bad.resources[0].initial_args.len = 0;
    assert!(matches!(
        bad.validate(),
        Err(PlanError::BadReference {
            table: "resources",
            field: "initial_args",
            ..
        })
    ));
    for (name, params, result) in [
        ("open", vec!["Router", "string"], "Router"),
        ("push", vec!["Router", "string"], "Router"),
        ("replace", vec!["Router", "string"], "Router"),
        ("back", vec!["Router"], "Router"),
        ("select", vec!["Router", "string"], "Router"),
        ("go", vec!["Router", "string"], "Router"),
        ("stack", vec!["Router"], "list<Entry>"),
        ("top", vec!["Router"], "Entry"),
        ("depth", vec!["Router"], "number"),
        ("params", vec!["Router", "string"], "list<string>"),
        ("searchParam", vec!["Entry", "string"], "string"),
        ("encodeURIComponent", vec!["string"], "string"),
    ] {
        let f = Stdlib::from_name(name).unwrap();
        assert_eq!(f.params(), params);
        assert_eq!(f.returns(), result);
        assert_eq!(Stdlib::from_wire(f as u8), Some(f));
    }
    assert_eq!(Stdlib::Now as u8, 0);
    assert_eq!(Stdlib::Min as u8, 10);
}

#[test]
fn sites_nest_at_most_max_site_depth_and_types_never_contain_themselves() {
    use exact_plan::MAX_SITE_DEPTH;
    let deep = |levels: usize| {
        let mut b = PlanBuilder::new(0, 1);
        let mut parent = None;
        for _ in 0..levels {
            parent = Some(b.node(0, parent, None, 0, &[], &[], None));
        }
        b.finish()
    };
    assert!(deep(MAX_SITE_DEPTH).is_ok());
    assert!(matches!(
        deep(MAX_SITE_DEPTH + 1),
        Err(PlanError::SiteTooDeep { table: "nodes", .. })
    ));
    let mut cycle = deep(2).unwrap();
    cycle.nodes[0].parent = Some(exact_plan::NodesId(1));
    assert!(matches!(
        cycle.validate(),
        Err(PlanError::SiteTooDeep { .. })
    ));

    let mut b = PlanBuilder::new(0, 1);
    let number = b.primitive(TypeKind::Number);
    let list = b.list(number);
    let row = b.record("Row", &[("n", number), ("items", list)]);
    let mut plan = b.finish().unwrap();
    plan.types[list.0 as usize].elem = Some(row);
    assert!(matches!(plan.validate(), Err(PlanError::TypeCycle { .. })));
}
