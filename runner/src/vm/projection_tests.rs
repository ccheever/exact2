use super::*;
use exact_plan::{
    asm::{Arg, Asm},
    builder::PlanBuilder,
};

#[test]
fn row_projections_match_the_machine_values_traps_and_reused_storage() {
    assert_eq!(Opcode::LoadItem.operands(), &[Operand::U16]);
    assert_eq!(Opcode::LoadBound.operands(), &[Operand::U16]);
    assert_eq!(Opcode::Field.operands(), &[Operand::U16]);
    let plan = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
        .finish()
        .unwrap();
    let record = Value::record(vec![
        Value::Unit,
        Value::str("shared text wider than the inline string"),
        Value::Number(f64::NAN),
        Value::list(vec![Value::Bool(true)]),
        Value::record(vec![Value::Number(-0.0)]),
    ]);
    let cases = [
        None,
        Some(Value::Unit),
        Some(Value::Bool(false)),
        Some(Value::Number(3.0)),
        Some(Value::str("text")),
        Some(Value::list(vec![])),
        Some(Value::Option(None)),
        Some(Value::some(record.clone())),
        Some(Value::record(vec![])),
        Some(Value::record(vec![Value::Bool(true)])),
        Some(Value::record(vec![Value::Bool(false)])),
        Some(record.clone()),
    ];
    let mut scratch = Scratch::default();
    let mut oracle = Scratch::default();
    for value in cases {
        let frames = [
            Frame {
                item: value.clone(),
                bound: Some(record.clone()),
                ..Frame::default()
            },
            Frame {
                item: Some(record.clone()),
                bound: value,
                ..Frame::default()
            },
        ];
        let env = Env {
            plan: &plan,
            strings: &[],
            router: None,
            lists: None,
            format: None,
            geometry: None,
            slots: &[],
            derives: &[],
            resources: &[],
            params: &[],
            frames: &frames,
            now_ms: 0.0,
            pending_resources: &[],
            failed_resources: &[],
            failed_why: &[],
            pending_mutations: &[],
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        };
        for op in [Opcode::LoadItem, Opcode::LoadBound] {
            for depth in [0u16, 1, 2, u16::MAX] {
                for index in [0u16, 1, 2, 3, 4, 5, u16::MAX] {
                    for code in bodies(op, depth, index) {
                        assert!(field_projection(&code, &env).is_some());
                        let actual = eval_reusing(&code, &env, &[], &mut scratch);
                        let expected = eval_machine(&code, &env, &[], &mut oracle);
                        // Debug equality includes all outcome effects and trap locations,
                        // while treating NaN as the same untouched field in both results.
                        assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "{code:?}");
                        assert!(scratch.machine.stack.is_empty());
                        assert!(scratch.machine.locals.is_empty());
                        assert!(scratch.machine.callbacks.is_empty());
                        assert!(scratch.extents.0.is_empty());
                        assert_eq!(scratch.machine.pos, 0);
                        let actual_value = eval_value_reusing(&code, &env, &mut scratch);
                        assert_eq!(
                            format!("{:?}", actual_value.as_ref()),
                            format!("{:?}", expected.as_ref().map(|out| &out.value)),
                            "value-only {code:?}"
                        );
                        assert!(scratch.machine.stack.is_empty());
                        assert!(scratch.machine.locals.is_empty());
                        assert!(scratch.machine.callbacks.is_empty());
                        assert!(scratch.extents.0.is_empty());
                        assert_eq!(scratch.machine.pos, 0);
                    }
                }
            }
        }
        let code = [
            Opcode::LoadItem as u8,
            0,
            0,
            Opcode::Field as u8,
            1,
            0,
            Opcode::Return as u8,
        ];
        let mut others: Vec<Vec<u8>> = (0..code.len()).map(|n| code[..n].to_vec()).collect();
        others.push([code.as_slice(), &[255]].concat());
        for (at, byte) in [(0, 255), (0, Opcode::LoadIndex as u8), (3, 255), (6, 255)] {
            let mut changed = code.to_vec();
            changed[at] = byte;
            others.push(changed);
        }
        for code in others {
            let actual = eval_reusing(&code, &env, &[], &mut scratch);
            let expected = eval_machine(&code, &env, &[], &mut oracle);
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "{code:?}");
        }
    }
}

// Short reads, local scopes, nested projections, forward jumps and callbacks
// all keep the same absolute instruction locations after the initial field.
fn bodies(op: Opcode, depth: u16, index: u16) -> Vec<Vec<u8>> {
    (0..6)
        .map(|tail| {
            let mut a = Asm::new();
            a.op(op, &[Arg::U16(depth)]).field(index);
            match tail {
                0 => {}
                1 => {
                    a.bind_local()
                        .load_local(0)
                        .load_local(0)
                        .simple(Opcode::Eq)
                        .drop_local();
                }
                2 => {
                    a.field(0);
                }
                3 => {
                    a.bind_local().load_local(0).drop_local();
                }
                4 => {
                    let no = a.label();
                    let end = a.label();
                    a.jump_if_false(no)
                        .number(7.0)
                        .jump(end)
                        .place(no)
                        .number(9.0)
                        .place(end);
                }
                _ => {
                    let end = a.label();
                    a.each_item(Opcode::Map, end)
                        .load_local(0)
                        .simple(Opcode::Not)
                        .place(end);
                }
            }
            a.finish()
        })
        .collect()
}

#[test]
fn projected_string_defaults_match_both_branches_and_all_refusals() {
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let literals =
        ["", "sentinel", "a literal longer than inline text"].map(|text| builder.str(text));
    let replacement = builder.str("replacement text");
    let plan = builder.finish().unwrap();
    let strings: Vec<_> = plan.strings.iter().map(|text| Value::str(text)).collect();
    let mut nontext = strings.clone();
    nontext[literals[0].0 as usize] = Value::Number(7.0);
    let caches = [
        strings.as_slice(),
        &strings[..literals[0].0 as usize],
        &strings[..replacement.0 as usize],
        nontext.as_slice(),
    ];
    let mut scratch = Scratch::default();
    let mut oracle = Scratch::default();
    for value in [
        Value::str(""),
        Value::str("sentinel"),
        Value::str("a literal longer than inline text"),
        Value::str("é🙂"),
        Value::Number(7.0),
        Value::Number(f64::NAN),
        Value::Bool(false),
        Value::Unit,
        Value::NONE,
        Value::some(Value::str("")),
        Value::list(vec![]),
        Value::record(vec![]),
    ] {
        let frame = Frame {
            item: Some(Value::record(vec![value.clone()])),
            bound: Some(Value::record(vec![value.clone()])),
            ..Frame::default()
        };
        let frames = [frame.clone(), frame];
        for strings in caches {
            let env = Env {
                plan: &plan,
                strings,
                router: None,
                lists: None,
                format: None,
                geometry: None,
                slots: &[],
                derives: &[],
                resources: &[],
                params: &[],
                frames: &frames,
                now_ms: 0.0,
                pending_resources: &[],
                failed_resources: &[],
                failed_why: &[],
                pending_mutations: &[],
                store_dependent_derives: &[],
                store_dependent_resources: &[],
            };
            for op in [Opcode::LoadItem, Opcode::LoadBound] {
                for depth in [0, 1, 2] {
                    for index in [0, 1] {
                        for literal in literals {
                            let mut asm = Asm::new();
                            let no = asm.label();
                            let end = asm.label();
                            asm.op(op, &[Arg::U16(depth)])
                                .field(index)
                                .bind_local()
                                .load_local(0)
                                .str(literal)
                                .simple(Opcode::Eq)
                                .jump_if_false(no)
                                .str(replacement)
                                .jump(end)
                                .place(no)
                                .load_local(0)
                                .place(end)
                                .drop_local();
                            let code = asm.finish();
                            if value.is_str()
                                && strings.len() == plan.strings.len()
                                && strings[literal.0 as usize].is_str()
                            {
                                assert!(projected_string_default(&code, &env, &value).is_some());
                            }
                            let mut cases = vec![code.clone()];
                            // Truncation, wrong opcodes, missing strings, bad locals,
                            // and jumps past the end must preserve the trap and pc.
                            for at in 0..code.len() {
                                cases.push(code[..at].to_vec());
                                let mut changed = code.clone();
                                changed[at] = 255;
                                cases.push(changed);
                            }
                            cases.push([code.as_slice(), &[255]].concat());
                            for code in cases {
                                let actual = eval_reusing(&code, &env, &[], &mut scratch);
                                let expected = eval_machine(&code, &env, &[], &mut oracle);
                                assert_eq!(
                                    format!("{actual:?}"),
                                    format!("{expected:?}"),
                                    "{code:?}"
                                );
                                assert!(scratch.machine.stack.is_empty());
                                assert!(scratch.machine.locals.is_empty());
                                assert!(scratch.machine.callbacks.is_empty());
                                assert!(scratch.extents.0.is_empty());
                                assert_eq!(scratch.machine.pos, 0);
                                let actual_value = eval_value_reusing(&code, &env, &mut scratch);
                                assert_eq!(
                                    format!("{:?}", actual_value.as_ref()),
                                    format!("{:?}", expected.as_ref().map(|out| &out.value)),
                                    "value-only {code:?}"
                                );
                                assert!(scratch.machine.stack.is_empty());
                                assert!(scratch.machine.locals.is_empty());
                                assert!(scratch.machine.callbacks.is_empty());
                                assert!(scratch.extents.0.is_empty());
                                assert_eq!(scratch.machine.pos, 0);
                            }
                        }
                    }
                }
            }
        }
    }
}
