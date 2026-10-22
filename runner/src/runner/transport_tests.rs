//! Review round 2, G1: a background stop is the standing commit's, and when
//! `set_page`'s commit is refused with reads owed, the commit that fails
//! those reads and stands makes it. Whether the plan keeps MIDI, which
//! `state` shows, is walked for once.

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

/// Whether a plan keeps MIDI decodes all of its code: the first `state`
/// read makes the answer, and every read after takes it from the runner.
#[test]
fn the_agent_walks_the_plan_for_midi_once() {
    const MIDI: &str = ",\"midi\":{\"state\"";
    let r = boot_with(SOURCE);
    assert!(r.keeps_midi.get().is_none(), "booting walks no code for it");
    assert!(crate::agent::handle(&r, r#"{"op":"state"}"#).contains(MIDI));
    assert_eq!(r.keeps_midi.get(), Some(&true));
    // Its sequence keeps a queue; a read told otherwise shows none, so no
    // read walks the code again.
    let r = boot_with(SOURCE);
    r.keeps_midi.set(false).unwrap();
    assert!(!crate::agent::handle(&r, r#"{"op":"state"}"#).contains(MIDI));
}
