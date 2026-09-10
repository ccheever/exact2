use exact_plan::{builder::PlanBuilder, Code, Plan, RegionKind, TypeKind, Value};

fn fixture() -> PlanBuilder {
    let mut b = PlanBuilder::new(0, 0);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.0));
    let scope = b.region(RegionKind::Scope, None, None, 0, zero, zero, 1).0;
    let resource = b.resource("value", "read", &[], number, None);
    b.set_resource_owner(resource, scope);
    b.set_resource_fallback(resource, zero);
    let action = b.action("tick", &[("capture", number)], &[], zero);
    let timer = b.timer(100, action);
    b.set_timer_owner(timer, scope);
    let args = b.args(&[zero]);
    b.set_timer_args(timer, args);
    b.resource_boot(
        resource,
        &Value::list(vec![Value::str("a")]),
        &Value::list(vec![]),
        &Value::Number(1.0),
        false,
        true,
    );
    b
}

#[test]
fn owned_plan_round_trips_with_pending_boot_seed() {
    let plan = fixture().finish().unwrap();
    assert!(plan.resource_boot[0].pending);
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn malformed_owners_timer_arity_and_boot_seeds_are_refused() {
    let valid = fixture().finish().unwrap();
    let mut invalid = Vec::new();
    let mut p = valid.clone();
    p.regions[0].kind = RegionKind::Each;
    invalid.push(p);
    let mut p = valid.clone();
    p.timers[0].args.len = 0;
    invalid.push(p);
    let mut p = valid.clone();
    p.resource_boot.push(p.resource_boot[0].clone());
    invalid.push(p);
    let mut p = valid.clone();
    p.resources[0].owner = None;
    invalid.push(p);
    let mut p = valid.clone();
    p.resources[0].fallback = Code {
        offset: u32::MAX,
        len: 5,
    };
    invalid.push(p);
    for field in ["path", "args", "value"] {
        let mut b = PlanBuilder::from_plan(valid.clone());
        let bytes = b.data(&Value::Bool(false));
        let mut p = b.finish().unwrap();
        match field {
            "path" => p.resource_boot[0].path = bytes,
            "args" => p.resource_boot[0].args = bytes,
            _ => p.resource_boot[0].value = bytes,
        }
        invalid.push(p);
    }
    let mut b = PlanBuilder::from_plan(valid.clone());
    let bytes = b.data(&Value::list(vec![Value::list(vec![])]));
    let mut p = b.finish().unwrap();
    p.resource_boot[0].path = bytes;
    invalid.push(p);
    for (i, p) in invalid.iter().enumerate() {
        assert!(p.validate().is_err(), "case {i} passed validate");
        assert!(Plan::decode(&p.encode()).is_err(), "case {i} passed decode");
    }
}

#[test]
fn seeds_reject_nonfinite_keys_arguments_and_signed_zero_duplicates() {
    let valid = fixture().finish().unwrap();
    for value in [
        Value::list(vec![Value::Number(f64::NAN)]),
        Value::list(vec![Value::Number(f64::INFINITY)]),
    ] {
        let mut b = PlanBuilder::from_plan(valid.clone());
        let bytes = b.data(&value);
        let mut plan = b.finish().unwrap();
        plan.resource_boot[0].path = bytes;
        assert!(Plan::decode(&plan.encode()).is_err());
        plan.resource_boot[0].args = bytes;
        assert!(plan.validate().is_err());
    }
    let mut b = PlanBuilder::from_plan(valid);
    for zero in [-0.0, 0.0] {
        b.resource_boot(
            exact_plan::ResourcesId(0),
            &Value::list(vec![Value::Number(zero)]),
            &Value::list(vec![]),
            &Value::Number(0.0),
            false,
            false,
        );
    }
    assert!(b.finish().is_err());
}

#[test]
fn mutations_accept_scope_slots_and_slots_reject_other_region_owners() {
    let mut b = fixture();
    let number = b.primitive(TypeKind::Number);
    let option = b.option(number);
    let none = b.constant(&Value::Option(None));
    let slot = b.slot("reply", option, none);
    b.set_slot_owner(slot, exact_plan::RegionsId(0));
    b.mutation("reply", slot, number);
    let plan = b.finish().unwrap();
    assert_eq!(Plan::decode(&plan.encode()).unwrap(), plan);
    let mut b = PlanBuilder::from_plan(plan);
    let region = b.region(RegionKind::When, None, None, 1, none, none, 2).0;
    b.set_slot_owner(slot, region);
    assert!(b.finish().is_err());
}
