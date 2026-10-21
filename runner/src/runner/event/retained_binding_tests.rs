use super::*;
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, SlotsId, Stdlib, TypesId};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, super::super::DataError> {
        panic!("unexpected query {source}")
    }
}

fn fixture(
    build: impl FnOnce(&mut PlanBuilder, SlotsId) -> (Vec<(TypesId, Code)>, Asm),
) -> Runner<NoData> {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let empty = b.constant(&Value::str(""));
    let output = b.slot("out", string, empty);
    let (arguments, action) = build(&mut b, output);
    let names: Vec<_> = (0..arguments.len()).map(|i| format!("arg{i}")).collect();
    let params: Vec<_> = arguments
        .iter()
        .zip(&names)
        .map(|((ty, _), name)| (name.as_str(), *ty))
        .collect();
    let code = b.code(action);
    let action = b.action("set", &params, &[output], code);
    let args: Vec<_> = arguments.iter().map(|(_, code)| *code).collect();
    b.node(
        NodeType::Pressable as u8,
        None,
        None,
        0,
        &[],
        &[(EventKind::Press, action, &args)],
        None,
    );
    Runner::boot(
        b.finish().unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn capture(r: &Runner<NoData>) -> Result<ActionBinding, ActionBindingRefusal> {
    let key = r.kernel.arena().key_of(r.kernel.roots()[0]).unwrap();
    r.capture_action_binding(key, EventKind::Press)
}

fn literals(count: usize, len: usize) -> Runner<NoData> {
    fixture(|b, out| {
        let ty = b.primitive(TypeKind::String);
        let args = (0..count)
            .map(|_| (ty, b.constant(&Value::str(&"x".repeat(len)))))
            .collect();
        let mut action = Asm::new();
        action.load_param(0).store_slot(out);
        (args, action)
    })
}

#[test]
fn argument_count_and_aggregate_string_boundaries() {
    assert!(capture(&literals(8, 0)).is_ok());
    assert!(matches!(
        capture(&literals(9, 0)),
        Err(ActionBindingRefusal::Limit)
    ));
    assert!(capture(&literals(4, 1024)).is_ok());
    assert!(matches!(
        capture(&literals(5, 1024)),
        Err(ActionBindingRefusal::Limit)
    ));
    assert!(matches!(
        capture(&literals(1, 1025)),
        Err(ActionBindingRefusal::Limit)
    ));
}

#[test]
fn direct_nested_fields_stop_at_eight_without_copying_parent_records() {
    for depth in [8, 9] {
        let mut r = fixture(|b, out| {
            let string = b.primitive(TypeKind::String);
            let mut ty = string;
            let mut initial = Asm::new();
            initial.str(b.str("projected"));
            for i in 0..depth {
                ty = b.record(&format!("Depth{i}"), &[("value", ty)]);
                initial.record(ty);
            }
            let initial = b.code(initial);
            let root = b.slot("nested", ty, initial);
            let mut arg = Asm::new();
            arg.load_slot(root);
            for _ in 0..depth {
                arg.field(0);
            }
            let mut action = Asm::new();
            action.load_param(0).store_slot(out);
            (vec![(string, b.code(arg))], action)
        });
        if depth == 8 {
            let binding = capture(&r).unwrap();
            r.dispatch_bound(&binding, Event::Press).unwrap();
            assert_eq!(r.slot("out"), Some(&Value::str("projected")));
        } else {
            assert!(matches!(capture(&r), Err(ActionBindingRefusal::Limit)));
        }
    }
}

#[test]
fn rich_projection_refuses_and_captured_scalar_does_not_pin_parent() {
    let mut n = 0;
    for rich in [Value::NONE, Value::list(vec![]), Value::record(vec![])] {
        assert_eq!(
            BindingScalar::capture(&rich, &mut n),
            Err(ActionBindingRefusal::Unsupported)
        );
    }
    // Longer than inline text, so the parent shares an allocation.
    let text = exact_plan::Str::from("retained-id-of-a-parent-row");
    let parent = Value::record(vec![
        Value::from(text.clone()),
        Value::list(vec![Value::Unit; 1024]),
    ]);
    let Value::Record(fields) = &parent else {
        unreachable!()
    };
    let before = exact_plan::Str::strong_count(&text);
    let scalar = BindingScalar::capture(&fields[0], &mut n).unwrap();
    assert_eq!(exact_plan::Str::strong_count(&text), before);
    drop(parent);
    assert_eq!(exact_plan::Str::strong_count(&text), 1);
    assert_eq!(
        scalar,
        BindingScalar::String("retained-id-of-a-parent-row".into())
    );
    assert_ne!(
        BindingScalar::capture(&Value::Number(0.0), &mut n),
        BindingScalar::capture(&Value::Number(-0.0), &mut n)
    );
    assert!(BindingScalar::capture(&Value::Number(f64::NAN), &mut n).is_err());
}

#[test]
fn projection_calls_and_rich_values_are_not_evaluated_by_capture() {
    for call in [false, true] {
        let r = fixture(|b, _| {
            let ty = b.primitive(TypeKind::Number);
            let ty = if call { ty } else { b.list(ty) };
            let mut arg = Asm::new();
            if call {
                arg.call(Stdlib::PerformanceNow);
            } else {
                arg.number(1.0).list(1);
            }
            (vec![(ty, b.code(arg))], Asm::new())
        });
        let before = r.journal().count();
        assert!(matches!(
            capture(&r),
            Err(ActionBindingRefusal::Unsupported)
        ));
        assert_eq!(r.journal().count(), before);
        assert_eq!(r.now_ms(), 0.0);
    }
}

#[test]
fn direct_root_projection_tracks_current_scalar_and_poison_refuses() {
    let mut r = fixture(|b, out| {
        let ty = b.primitive(TypeKind::String);
        let mut arg = Asm::new();
        arg.load_slot(out);
        let mut action = Asm::new();
        action.load_param(0).store_slot(out);
        (vec![(ty, b.code(arg))], action)
    });
    let binding = capture(&r).unwrap();
    r.act("set", vec![Value::str("changed")]).unwrap();
    assert_eq!(
        r.validate_action_binding(&binding, EventKind::Press),
        Err(ActionBindingRefusal::Stale)
    );
    r.poisoned = true;
    assert!(matches!(capture(&r), Err(ActionBindingRefusal::Poisoned)));
}

#[test]
fn action_ambient_reads_commands_and_control_flow_refuse() {
    for mode in 0..4 {
        let r = fixture(|b, out| {
            let mut action = Asm::new();
            match mode {
                0 => {
                    action.load_slot(out).store_slot(out);
                }
                1 => {
                    action.call(Stdlib::PerformanceNow).simple(Opcode::Pop);
                }
                2 => {
                    let end = action.label();
                    action.jump(end).place(end);
                }
                _ => {
                    let name = b.str("effect");
                    action.command(name, 0);
                }
            }
            (vec![], action)
        });
        assert!(matches!(
            capture(&r),
            Err(ActionBindingRefusal::Unsupported)
        ));
        assert_eq!(r.slot("out"), Some(&Value::str("")));
    }
}

#[test]
fn action_actual_store_and_stack_limits_are_finite() {
    for stores in [8, 9] {
        let r = fixture(|b, out| {
            let mut action = Asm::new();
            let s = b.str("x");
            for _ in 0..stores {
                action.str(s).store_slot(out);
            }
            (vec![], action)
        });
        if stores == 8 {
            assert!(capture(&r).is_ok());
        } else {
            assert!(matches!(capture(&r), Err(ActionBindingRefusal::Limit)));
        }
    }
    let r = fixture(|_, _| {
        let mut action = Asm::new();
        for _ in 0..9 {
            action.simple(Opcode::Unit);
        }
        (vec![], action)
    });
    assert!(matches!(capture(&r), Err(ActionBindingRefusal::Limit)));
}
