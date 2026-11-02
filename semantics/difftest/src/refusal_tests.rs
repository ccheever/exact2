use crate::{observe, oracle::Oracle, refusal_class};
use exact_plan::{asm::Asm, builder::PlanBuilder, ActionsId, SlotsId, Value};

const APP: &str = "component App\n  state n = 1\n  derive inverse = 1 / n\n  action take(value: number)\n    n = value\n  view\n    button \"take\" press=take(1) testId=\"take\"\n";

#[test]
fn wrong_types_fail_and_nonfinite_boundaries_are_values() {
    for (value, class) in [
        (Value::Bool(true), "type"),
        (Value::Number(f64::INFINITY), "value"),
    ] {
        let plan = contract::compile(APP).unwrap();
        let mut r = observe::boot(plan.clone(), Oracle::new(&plan, 1)).unwrap();
        let error = r.act("take", vec![value.clone()]).unwrap_err();
        assert_eq!(refusal_class(&format!("{error:?}")), class);
        assert_eq!(r.slot("n"), Some(&Value::Number(1.0)));

        // A malformed plan reaches the write boundary that the source checker rejects.
        let mut builder = PlanBuilder::from_plan(plan);
        let slot = SlotsId(
            builder
                .plan()
                .slots
                .iter()
                .position(|s| builder.plan().str(s.name) == "n")
                .unwrap() as u32,
        );
        let mut body = Asm::new();
        match value {
            Value::Bool(v) => {
                body.bool(v);
            }
            Value::Number(_) => {
                body.number(1.0).number(0.0).simple(exact_plan::Opcode::Div);
            }
            _ => unreachable!(),
        }
        body.store_slot(slot);
        let code = builder.code(body);
        builder.set_action_body(ActionsId(0), code);
        let plan = builder.finish().unwrap();
        let mut r = observe::boot(plan.clone(), Oracle::new(&plan, 1)).unwrap();
        let error = r.act("take", vec![Value::Number(1.0)]).unwrap_err();
        assert_eq!(refusal_class(&format!("{error:?}")), class);
        assert_eq!(r.slot("n"), Some(&Value::Number(1.0)));

        let mut builder = PlanBuilder::from_plan(plan);
        let init = if matches!(value, Value::Bool(_)) {
            builder.constant(&value)
        } else {
            let mut expr = Asm::new();
            expr.number(1.0).number(0.0).simple(exact_plan::Opcode::Div);
            builder.code(expr)
        };
        let mut derive_builder = PlanBuilder::from_plan(builder.plan().clone());
        derive_builder.set_derive_body(exact_plan::DerivesId(0), init);
        let derive_plan = derive_builder.finish().unwrap();
        let error = observe::boot(derive_plan.clone(), Oracle::new(&derive_plan, 1))
            .err()
            .unwrap();
        assert_eq!(refusal_class(&format!("{error:?}")), class);
        if class == "type" {
            assert!(format!("{error:?}").contains("DeriveType"));
        }
        builder.set_slot_init(slot, init);
        let plan = builder.finish().unwrap();
        let error = observe::boot(plan.clone(), Oracle::new(&plan, 1))
            .err()
            .unwrap();
        assert_eq!(refusal_class(&format!("{error:?}")), class);
    }
    assert!(contract::compile(&APP.replace("n = value", "n = true")).is_err());

    let plan = contract::compile(APP).unwrap();
    let mut r = observe::boot(plan.clone(), Oracle::new(&plan, 1)).unwrap();
    let error = r.act("take", vec![Value::Number(0.0)]).unwrap_err();
    assert!(format!("{error:?}").contains("NonFiniteValue"));
    assert_eq!(refusal_class(&format!("{error:?}")), "value");
    assert_eq!(r.slot("n"), Some(&Value::Number(1.0)));
    let error = r.act("take", vec![]).unwrap_err();
    assert_eq!(refusal_class(&format!("{error:?}")), "world");
}

#[test]
fn curried_failures_keep_the_inner_class_and_omitted_commits_are_counted() {
    for (inner, class) in [
        ("Trap(Arity { pc: 2, expected: 1, actual: 0 })", "type"),
        ("Trap(UnwrapNone { pc: 0 })", "app"),
        ("NonFiniteValue { name: \"n\" }", "value"),
    ] {
        assert_eq!(
            refusal_class(&format!(
                "HandlerArguments {{ action: \"go\", error: {inner} }}"
            )),
            class
        );
    }
    let mut census = crate::census::Census::default();
    let lines = ["== land", "outcome ok", observe::OMITTED_COMMIT].map(str::to_string);
    assert!(census.observe(&lines).is_empty());
    assert_eq!(census.omitted, 1);
}
