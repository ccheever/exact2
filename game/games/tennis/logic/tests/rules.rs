//! The rules program on its own, and edited inside a running match.
use exact_game_rules::{Program, Rules};
use tennis_logic::rules::{Match, Phase, Side, PLAN};

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

/// What `semantics/Apps/Proofs/TennisRules.lean` does not prove yet, checked
/// over random event sequences on the shipped VM: nobody passes the games to
/// win, the winner is exactly who reached them, and a decided match only
/// shows its call or ends.
#[test]
fn random_events_keep_the_match_sane() {
    let p = Program::new(PLAN).unwrap();
    let events = p.events().to_vec();
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut ended = 0;
    for _ in 0..600 {
        let mut r = p.boot().unwrap();
        for _ in 0..600 {
            // Mostly an event the phase can use, sometimes any at all.
            let fits: &[&str] = match r.text("phase") {
                "ready" => &["toss"],
                "toss" => &["serveBall", "serveBall", "serveBall", "dropToss"],
                "rally" => &[
                    "hit",
                    "net",
                    "clip",
                    "landOwn",
                    "landOut",
                    "landCourt",
                    "landBox",
                ],
                _ => &["next"],
            };
            let roll = next();
            let event = if roll % 10 == 0 {
                events[(roll / 10 % events.len() as u64) as usize].as_str()
            } else {
                fits[(roll / 10 % fits.len() as u64) as usize]
            };
            let was_over = r.text("phase") == "over";
            p.act(&mut r, event).unwrap();
            let m = Match::read(&r, 0);
            let to_win = m.first_to();
            assert!(m.games[0] <= to_win && m.games[1] <= to_win, "{m:?}");
            match m.winner {
                None => assert!(m.games[0] < to_win && m.games[1] < to_win, "{m:?}"),
                Some(Side::Near) => assert!(m.games[0] == to_win && m.games[1] < to_win, "{m:?}"),
                Some(Side::Far) => assert!(m.games[1] == to_win && m.games[0] < to_win, "{m:?}"),
            }
            if m.winner.is_some() {
                assert!(matches!(m.phase, Phase::Dead | Phase::Over), "{m:?}");
            }
            if was_over {
                assert_eq!(m.phase, Phase::Over);
            }
            assert!(
                ["0", "15", "30", "40", "AD"].contains(&m.labels().0.as_str()),
                "{m:?}"
            );
        }
        ended += (r.text("phase") == "over") as u32;
    }
    assert!(ended > 0, "some random matches end");
}
