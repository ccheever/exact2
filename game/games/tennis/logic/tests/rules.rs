//! The rules program on its own, and edited inside a running match.
use exact_game_rules::{Program, Rules};
use tennis_logic::rules::{Match, Side, PLAN};

/// A point to Jev: you serve and miss Jev's return, or Jev serves an ace.
fn far_point(p: &Program, r: &mut Rules) {
    let events: &[&str] = if r.flag("nearServes") {
        &[
            "next",
            "toss",
            "serveBall",
            "landBox",
            "hit",
            "landCourt",
            "landCourt",
        ]
    } else {
        &["next", "toss", "serveBall", "landBox", "landCourt"]
    };
    for event in events {
        p.act(r, event).unwrap();
    }
}

#[test]
fn first_to_four_games() {
    let p = Program::new(PLAN).unwrap();
    let mut r = p.boot().unwrap();
    for _ in 0..16 {
        far_point(&p, &mut r);
    }
    let m = Match::read(&r, 0);
    assert_eq!(m.games, [0, 4]);
    assert_eq!(m.winner, Some(Side::Far));
    assert!(m.call.ends_with(" · Game, set and match Jev"), "{m:?}");
    p.act(&mut r, "next").unwrap();
    let over = r.slots.clone();
    for event in p.events().to_vec() {
        p.act(&mut r, &event).unwrap();
    }
    assert_eq!(r.slots, over, "nothing changes once the match is over");
}

#[test]
fn the_kernels_raise_only_declared_events() {
    let p = Program::new(PLAN).unwrap();
    let mut r = p.boot().unwrap();
    let e = p.act(&mut r, "point").unwrap_err();
    assert!(e.contains("not an event"), "{e}");
}
