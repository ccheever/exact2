//! The plan format: canonical bytes round-trip, loading is a validation pass,
//! and values conform to their shapes or are refused.

use exact_plan::asm::Asm;
use exact_plan::builder::PlanBuilder;
use exact_plan::bytes::Reader;
use exact_plan::{
    BindingKind, BindingsRow, Code, CodeError, EventKind, Opcode, Plan, PlanError, RegionKind,
    Stdlib, TypeKind, Value, FORMAT_DIGEST,
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
    let _res = b.resource("nearby", "stations_near", &[arg], stations, Some(&initial));
    let mut inc = Asm::new();
    inc.load_slot(count)
        .number(1.0)
        .simple(Opcode::Add)
        .store_slot(count);
    let inc = b.code(inc);
    let tick = b.action("tick", &[], &[count], inc);
    b.timer(1000, tick);
    let mut text = Asm::new();
    text.load_derive(doubled).call(Stdlib::ToString);
    let text = b.code(text);
    let root = b.node(0, None, None, 0, &[], &[]);
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
    );
    let mut cond = Asm::new();
    cond.load_slot(count).number(3.0).simple(Opcode::Gt);
    let cond = b.code(cond);
    let unit = b.constant(&Value::Unit);
    let (_region, arms) = b.region(RegionKind::When, Some(root), None, 1, cond, unit, 2);
    b.node(1, None, Some(arms[0]), 0, &[], &[]);
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
}

#[test]
fn a_huge_announced_count_reserves_little_before_it_is_refused() {
    // A header announcing 16M strings over a few bytes: refused by count
    // (count > remaining), and never reserved 16M entries first.
    let plan = sample();
    let mut bytes = plan.encode();
    // strings count sits right after the 36-byte header.
    bytes[36..40].copy_from_slice(&(1u32 << 24).to_le_bytes());
    assert!(matches!(Plan::decode(&bytes), Err(PlanError::BadCount(_))));
}
