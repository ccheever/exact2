//! What a `for` key reads: a derive, a resource and the pending states
//! invalidate it; a router read is pure and an action parameter opaque.

use super::*;

fn key_reuse_dependency_plan(which: u8) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.));
    b.slot("dep", number, zero);
    let derive = b.derive("derived", number, zero);
    let resource = b.resource("answer", "answer", &[], number, Some(&Value::Number(0.)));
    let optional = b.option(number);
    let none = b.constant(&Value::NONE);
    let reply = b.slot("reply", optional, none);
    let mutation = b.mutation("write", reply, number);
    let subject = b.constant(&values(1));
    let key = code(&mut b, |a| match which {
        0 => {
            a.load_derive(derive);
        }
        1 => {
            a.load_resource(resource);
        }
        2 => {
            a.pending_resource(resource);
        }
        3 => {
            a.pending_mutation(mutation);
        }
        4 => {
            a.simple(Opcode::Unit).call(exact_plan::Stdlib::Depth);
        }
        5 => {
            a.load_param(0);
        }
        _ => unreachable!(),
    });
    b.region(RegionKind::Each, None, None, 0, subject, key, 1);
    b.finish().unwrap()
}

#[test]
fn key_reads_track_derive_resource_and_pending_variants() {
    for kind in 0..4 {
        let p = key_reuse_dependency_plan(kind);
        let sites = crate::instance::SiteIndex::new(&p);
        let slots = [Value::Number(0.)];
        let before = [Some(Value::Number(1.))];
        let after = [Some(Value::Number(2.))];
        let held_before = [Some(crate::held::Held::new(Value::Number(1.)))];
        let held_after = [Some(crate::held::Held::new(Value::Number(2.)))];
        let pending = [false];
        let changed_pending = [true];
        let mut input = env(&p, &slots);
        input.derives = &before;
        input.resources = &held_before;
        input.pending_resources = &pending;
        input.pending_mutations = &pending;
        let seen = crate::instance::deps::Seen::of(&input);
        let key = &sites.deps.keys[0];
        assert!(!sites.deps.changed(&seen, &input).intersects(&key.bits));
        match kind {
            0 => input.derives = &after,
            1 => input.resources = &held_after,
            2 => input.pending_resources = &changed_pending,
            3 => input.pending_mutations = &changed_pending,
            _ => unreachable!(),
        }
        assert!(
            sites.deps.changed(&seen, &input).intersects(&key.bits),
            "dependency {kind}"
        );
    }
}

#[test]
fn a_router_read_is_pure_and_an_action_parameter_is_opaque() {
    let sites = |kind| crate::instance::SiteIndex::new(&key_reuse_dependency_plan(kind));
    assert!(sites(4).deps.keys[0].constant());
    assert!(sites(5).deps.keys[0].opaque);
}
