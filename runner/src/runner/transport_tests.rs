//! Review round 2, G1: a background stop is the standing commit's, and when
//! `set_page`'s commit is refused with reads owed, the commit that fails
//! those reads and stands makes it.

use crate::transport::tests::{answered, boot_with, voices, SOURCE};
use exact_plan::Value;

#[test]
fn a_background_stop_rides_the_recovery_commit() {
    let source = SOURCE
        .replace(
            "  derive double = bpm * 2\n",
            "  derive double = bpm * 2\n  derive crowdKey = crowded.length == 7 ? 0 / 0 : 1\n",
        )
        .replace(
            "  sequence groove",
            "  task watch when true key=crowdKey\n    after(100000, noop)\n  sequence groove",
        );
    let mut r = boot_with(&source);
    r.act("start", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    // The list read again answers seven steps, which makes a task's key no
    // key: the page's commit is refused, the read failed, and the next
    // commit stands.
    let crowded = (0..r.plan.resources.len())
        .find(|&i| r.plan.str(r.plan.resources[i].name) == "crowded")
        .unwrap();
    r.data().poisoned = true;
    r.force_refresh(crowded);
    r.owe(&[crowded]);
    let page = crate::Page {
        hidden: true,
        suspended: true,
        ..r.page()
    };
    assert!(r.set_page(page).is_ok());
    assert!(r.page().suspended);
    assert!(!r.transport().playing(), "the recovery commit stopped it");
    assert!(voices(&r)
        .iter()
        .all(|v| v.at <= 1000.0 || v.by != crate::sound::By::End));
    r.land_then();
    assert_eq!(answered(&r)[1], Value::str("background"));
}
