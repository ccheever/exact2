//! A kept answer whose identity reads the zone's offset (LLP 1027.005 D3,
//! LLP 1027.000.000): the Bluesky clone's Discover, kept at -420. A host
//! that boots under the date (`crate::time::launching`) admits it from the
//! first frame; a boot on zero, or in another zone, refuses it.
use super::*;
use crate::time::{launching, WallTime};
use crate::{Answer, Store};
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, TypeKind, Value};

struct Source;

impl DataSource for Source {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        unreachable!("asked only at readiness")
    }

    /// Not ready at boot, as a store-reading source is not: the first frame
    /// is the kept answer or the placeholder.
    fn ready(&self) -> bool {
        false
    }

    fn grants(&self) -> &str {
        "net.fetch https://example.test"
    }
}

/// `time = exactTime()` and `feeds(time.utcOffset)`.
fn plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    let shape = b.record("Time", &[("epochAtZero", number), ("utcOffset", number)]);
    let time = b.resource("time", crate::time::SOURCE, &[], shape, None);
    let mut k = Asm::new();
    k.load_resource(time).field(1);
    let k = b.code(k);
    let feeds = b.resource("feeds", "feeds", &[k], string, Some(&Value::str("loading")));
    b.set_resource_reader(feeds, true);
    // As baked: the date unknown, the offset zero.
    b.set_resource_initial_args(feeds, &[Value::Number(0.)]);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}

fn launch(time: Option<WallTime>) -> Runner<Source> {
    let kept = (
        kept::kept_name("feeds"),
        kept::encode(&[Value::Number(-420.)], &Value::str("feeds at -420")),
    );
    launching(time, || {
        Runner::boot_stored(
            plan(),
            Source,
            Kernel::with_monospace(),
            vec![kept],
            Default::default(),
            "/",
        )
    })
    .unwrap()
}

fn at(utc_offset: f64) -> Option<WallTime> {
    Some(WallTime {
        epoch_at_zero: 1_790_000_000_000.0,
        utc_offset,
    })
}

#[test]
fn a_kept_answer_keyed_on_the_offset_is_admitted_when_the_boot_knows_it() {
    let r = launch(at(-420.));
    assert_eq!(r.resource("feeds"), Some(&Value::str("feeds at -420")));
    assert!(r
        .journal()
        .any(|l| l.contains("feeds shows its kept answer from the last session, kept for [-420]")));
    // On zero (a host that says the date only after the boot), or in
    // another zone, the identity differs: refused.
    for time in [None, at(60.)] {
        let r = launch(time);
        assert_eq!(r.resource("feeds"), Some(&Value::str("loading")));
        assert!(!r.journal().any(|l| l.contains("shows its kept answer")));
    }
}
