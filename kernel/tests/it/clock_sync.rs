//! A commit's clock joins wait for all of its rows (LLP 1055.002 D6,
//! "Within a commit"): `MotionSync::apply` holds them, so the order a
//! commit's nodes are applied in does not pick a phase.

use exact_kernel::MotionSync;
use exact_motion::{Animations, Engine, Keyframes};

fn row(text: &str) -> Animations {
    let pulse = Keyframes::parse("from{opacity:0.4}to{opacity:1}").unwrap();
    let mut a = Animations::parse(text).unwrap();
    a.resolve(|name| (name == "pulse").then_some(&pulse));
    a
}

#[test]
fn a_commit_that_swaps_the_only_member_starts_the_clock_over() {
    let (old, new) = (3, 4);
    let mut e = Engine::new();
    let first = MotionSync {
        clocks: vec![(old, Some("Pending".into()))],
        animations: vec![(old, row("pulse 800ms infinite alternate"))],
        ..Default::default()
    };
    first.apply(&mut e).unwrap();
    e.advance(1.2).unwrap();
    // A created node's row comes before a touched node's: the new member is
    // applied while the old one still plays.
    let second = MotionSync {
        clocks: vec![(new, Some("Pending".into())), (old, Some("Pending".into()))],
        animations: vec![
            (new, row("pulse 800ms 1 alternate")),
            (old, Animations::NONE),
        ],
        ..Default::default()
    };
    second.apply(&mut e).unwrap();
    assert_eq!(
        e.animation_plays(new)[0].start,
        1.2,
        "not 0.0, ended at once"
    );
}
