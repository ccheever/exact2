//! A retained subtree moved between content owners (LLP 1123 D9, D12): the
//! presenter is sent its style as its new owner resolves it, though nothing
//! it inherits changed.

use super::Host;
use exact_kernel::{MonospaceMeasurer, Op, OwnerValues};
use exact_runner::{DataError, DataSource, Timed, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

/// The radius the batch last sends `id`.
fn radius(batch: &str, id: u32) -> serde_json::Value {
    let batch: serde_json::Value = serde_json::from_str(batch).unwrap();
    let ops = batch["ops"].as_array().unwrap();
    let op = ops
        .iter()
        .rev()
        .find(|o| o["op"] == "style" && o["id"] == id);
    op.map_or(serde_json::Value::Null, |o| {
        o["style"]["border_radius_top_left"].clone()
    })
}

#[test]
fn a_subtree_moved_to_another_owner_is_sent_its_lengths_by_it() {
    let plan = contract::compile(
        r#"component App
  view
    row width="100%" height="100%"
      column testId="tall" width="50%" height="100%"
        box testId="card" width=40 height=40 border-radius="10cqh"
          box testId="inner" width=20 height=20 border-radius="10cqh"
      column testId="short" width="50%" height="100%"
"#,
    )
    .unwrap()
    .encode();
    let measurer = Box::new(MonospaceMeasurer::default());
    let (mut host, _) = Host::boot(&plan, NoData, measurer, 800., 600.).unwrap();
    let id = |host: &Host<NoData>, name: &str| {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let (tall, short) = (id(&host, "tall"), id(&host, "short"));
    let (card, inner) = (id(&host, "card"), id(&host, "inner"));
    let owner = |height| OwnerValues {
        cols: 1,
        rows: 1,
        size: (400.0, height),
        ..OwnerValues::default()
    };
    let first = host.set_claims(&crate::Claims {
        owners: &[(tall, Some(owner(300.0))), (short, Some(owner(100.0)))],
        ..crate::Claims::default()
    });
    assert_eq!(radius(&first, card), 30.0, "{first}");
    // A producer moves the card, as kept, into the other owner.
    let receipt = (host.runner.kernel_mut())
        .apply(
            0,
            99,
            &[Op::SetChildren {
                id: short,
                children: vec![card],
            }],
        )
        .unwrap();
    let moved = host.commit(&[Timed { at_ms: 0., receipt }], None);
    assert_eq!(radius(&moved, card), 10.0, "{moved}");
    assert_eq!(radius(&moved, inner), 10.0, "{moved}");
}
