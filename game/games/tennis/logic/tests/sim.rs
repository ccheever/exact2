//! Hostless matches. A scripted near player (the same bot the proof drives
//! through the agent API) serves and returns from the world's own contact
//! hint; a scripted brain supplies Jev's answers as the live `plan` argument.
use exact_game::{Sim, Transform, Value};
use tennis_logic::brain::{Brain, Source};
use tennis_logic::players::{Hand, Player, CONTACT};
use tennis_logic::rules::{Match, Phase, Side};
use tennis_logic::{Ball, Options, Tennis};

const TICK: f64 = 1000.0 / 120.0;

fn game(offline: bool) -> Sim<Tennis> {
    Sim::<Tennis>::new(Options {
        seed: 7,
        offline,
        ..Options::default()
    })
    .unwrap()
}

fn bind_plan(game: &mut Sim<Tennis>, plan: &str) {
    game.bind(
        &[
            Value::Number(7.0),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::str(plan),
        ],
        None,
    )
    .unwrap();
}

fn text(game: &Sim<Tennis>, key: &str) -> String {
    let value = game.world().published(key).unwrap();
    value.as_str().unwrap().to_string()
}

fn phase(game: &Sim<Tennis>) -> Match {
    game.world().resource::<Match>().clone()
}

/// The near player as an agent would play it: keys only.
#[derive(Default)]
struct Bot {
    held: Vec<&'static str>,
}
impl Bot {
    fn hold(&mut self, game: &mut Sim<Tennis>, want: &[&'static str]) {
        for key in self.held.clone() {
            if !want.contains(&key) {
                game.key_up(key);
            }
        }
        for key in want {
            if !self.held.contains(key) {
                game.key_down(key);
            }
        }
        self.held = want.to_vec();
    }
    /// One tick of play: serve on cue, run to the hint, swing CONTACT ticks early.
    fn step(&mut self, game: &mut Sim<Tennis>, hand_override: Option<Hand>) {
        let m = phase(game);
        let tick = game.world().tick();
        let p = *game.get::<Player>("near").unwrap();
        let at = game.local_position("near").unwrap();
        let ball = *game.get::<Ball>("ball").unwrap();
        if m.server == Side::Near && m.phase == Phase::Ready && tick >= m.since + 30 {
            self.hold(game, &[]);
            game.tap("KeyJ");
        } else if m.server == Side::Near
            && m.phase == Phase::Toss
            && tick + 1 == m.since + 68 - CONTACT
        {
            game.tap("KeyJ");
        } else if m.phase == Phase::Rally && ball.hitter == Side::Far && p.contact > tick {
            let (dx, dz) = (p.goal.x - at.x, p.goal.z - at.z);
            let mut keys = vec![];
            if p.contact <= tick + CONTACT + 1 {
                // Aim for the open court with the stick through contact.
                let far = game.local_position("far").unwrap();
                keys.push(if far.x > 0.0 { "KeyA" } else { "KeyD" });
            } else if p.contact > tick + 10 {
                if dx > 0.1 {
                    keys.push("KeyD");
                } else if dx < -0.1 {
                    keys.push("KeyA");
                }
                if dz < -0.1 {
                    keys.push("KeyW");
                } else if dz > 0.1 {
                    keys.push("KeyS");
                }
            }
            self.hold(game, &keys);
            if tick + CONTACT + 1 == p.contact {
                let hand = hand_override.unwrap_or(p.hand);
                game.tap(if hand == Hand::Backhand {
                    "KeyK"
                } else {
                    "KeyJ"
                });
            }
        } else {
            self.hold(game, &[]);
        }
        game.run(TICK);
    }
}

#[test]
fn offline_match_plays_serves_rallies_and_scores() {
    let mut game = game(true);
    let mut bot = Bot::default();
    let (mut longest, mut far_forehands, mut far_backhands) = (0, 0, 0);
    for _ in 0..(120 * 90) {
        bot.step(&mut game, None);
        longest = longest.max(phase(&game).rally);
        if phase(&game).phase == Phase::Over {
            break;
        }
    }
    for line in game.world().journal() {
        let text = line.line.clone();
        far_forehands += text.contains("far Forehand") as u32;
        far_backhands += text.contains("far Backhand") as u32;
    }
    let m = phase(&game);
    eprintln!("offline match after {:.0} s: {m:?}", game.world().seconds());
    assert!(m.points_played >= 4, "points were decided: {m:?}");
    assert!(longest >= 4, "a real rally happened (longest {longest})");
    assert!(
        far_forehands > 0 && far_backhands > 0,
        "Jev's player hits both sides"
    );
    let brain = game.world().resource::<Brain>().clone();
    assert_eq!(brain.stats.asked, 0, "offline never asks");
    assert_eq!(brain.last.unwrap().source, Source::Offline);
    assert_eq!(
        game.world().published("jev_state").unwrap().as_str(),
        Some("offline")
    );
}

/// Serve, then run the rally until the near player's next return is planned.
fn rally_until_near_contact(game: &mut Sim<Tennis>, bot: &mut Bot) {
    for _ in 0..2000 {
        bot.step(game, None);
        let p = *game.get::<Player>("near").unwrap();
        let ball = *game.get::<Ball>("ball").unwrap();
        if phase(game).phase == Phase::Rally
            && ball.hitter == Side::Far
            && !ball.serve
            && p.contact > game.world().tick() + 30
        {
            return;
        }
    }
    panic!("no rally ball reached the near player");
}

#[test]
fn forehand_only_on_the_racket_side() {
    let mut game = game(true);
    let mut bot = Bot::default();
    rally_until_near_contact(&mut game, &mut bot);
    let hand = game.get::<Player>("near").unwrap().hand;
    let wrong = if hand == Hand::Forehand {
        Hand::Backhand
    } else {
        Hand::Forehand
    };
    let mut sure = game.save().unwrap();
    let mut other = Sim::<Tennis>::new(Options {
        seed: 7,
        offline: true,
        ..Options::default()
    })
    .unwrap();
    other.restore(&sure).unwrap();
    for _ in 0..200 {
        bot.step(&mut game, None);
    }
    let mut wrong_bot = Bot::default();
    for _ in 0..200 {
        wrong_bot.step(&mut other, Some(wrong));
    }
    let journal = |g: &Sim<Tennis>| {
        g.world()
            .journal()
            .iter()
            .map(|e| e.line.clone())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        journal(&game).contains(&format!("near {hand:?} clean")),
        "{}",
        journal(&game)
    );
    assert!(
        journal(&other).contains(&format!("near {wrong:?} mishit")),
        "{}",
        journal(&other)
    );
    sure.clear();
}

#[test]
fn jev_answer_in_time_is_applied_and_sampled() {
    let mut game = game(false);
    let mut bot = Bot::default();
    // Serve: the toss asks Jev how to return.
    for _ in 0..40 {
        bot.step(&mut game, None);
    }
    let ask = text(&game, "ask");
    assert!(
        ask.contains("\"id\":1") && ask.contains("\"kind\":\"rally\""),
        "{ask}"
    );
    assert!(ask.contains("return of serve"), "{ask}");
    assert_eq!(text(&game, "jev_status"), "Jev thinking…");
    bind_plan(
        &mut game,
        r#"{"id":1,"ok":true,"error":"","shot":{"choice":"slice","confidence":0.5,"p":{"slice":1.0}},"target":{"choice":"backhand","confidence":0.9,"p":{"backhand":0.9,"forehand":0.1}},"aggression":{"score":1.0,"levels":5,"p":{"1":1.0}},"approach":0.0}"#,
    );
    bot.step(&mut game, None);
    let status = text(&game, "jev_status");
    assert!(status.starts_with("plan ready in 0."), "{status}");
    assert_eq!(text(&game, "jev"), "", "the plan is secret until the swing");
    assert_eq!(text(&game, "ask"), "", "an answered question is withdrawn");
    let brain = game.world().resource::<Brain>().clone();
    let copied = brain.answer.as_ref().unwrap().target.p["backhand"];
    assert_eq!(copied, 0.9, "probabilities are copied into saved state");
    for _ in 0..400 {
        bot.step(&mut game, None);
        if game.world().resource::<Brain>().stats.on_time == 1 {
            break;
        }
    }
    let last = game.world().resource::<Brain>().last.clone().unwrap();
    let executed = (last.source, last.shot.as_str(), last.id);
    assert_eq!(executed, (Source::Jev, "slice", 1));
    let line = text(&game, "jev");
    assert!(
        line.starts_with("Jev: slice → your"),
        "revealed at the swing: {line}"
    );
    // Striking, the far player asks at once for its next shot.
    for _ in 0..40 {
        bot.step(&mut game, None);
    }
    let ask = text(&game, "ask");
    assert!(ask.contains("\"id\":2"), "{ask}");
    assert!(
        text(&game, "jev").starts_with("Jev: slice → your"),
        "the line persists"
    );
    assert_eq!(text(&game, "jev_status"), "Jev thinking…");
    assert!(ask.contains("you just hit a slice aimed at"), "{ask}");
}

#[test]
fn late_answer_falls_back_and_stale_answers_are_ignored() {
    let mut game = game(false);
    let mut bot = Bot::default();
    for _ in 0..600 {
        bot.step(&mut game, None);
        if game.world().resource::<Brain>().stats.late == 1 {
            break;
        }
    }
    let brain = game.world().resource::<Brain>().clone();
    assert_eq!(brain.stats.late, 1, "no answer by the commit point");
    assert_eq!(brain.last.as_ref().unwrap().source, Source::Late);
    let line = game
        .world()
        .published("jev")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(line.starts_with("Jev late → fallback: "), "{line}");
    assert_eq!(
        game.world().published("jev_state").unwrap().as_str(),
        Some("late")
    );
    let id = brain.last.as_ref().unwrap().id;
    let journal: Vec<_> = game
        .world()
        .journal()
        .iter()
        .map(|e| format!("{} {}", e.tick, e.line))
        .collect();
    assert!(
        journal
            .iter()
            .any(|l| l.contains(&format!("jev decision {id} late"))),
        "{journal:#?}"
    );
    // The answer turns up after the far player committed: counted, never applied.
    bind_plan(
        &mut game,
        &format!(
            r#"{{"id":{id},"ok":true,"shot":{{"choice":"lob","p":{{"lob":1.0}}}},"target":{{"choice":"body","p":{{"body":1.0}}}}}}"#
        ),
    );
    bot.step(&mut game, None);
    let brain = game.world().resource::<Brain>().clone();
    assert_eq!(
        (brain.stats.late_arrivals, brain.ready.is_none()),
        (1, true)
    );
    // A wrong id is stale.
    bind_plan(&mut game, r#"{"id":99,"ok":true}"#);
    bot.step(&mut game, None);
    assert!(game
        .world()
        .journal()
        .iter()
        .any(|e| e.line.clone().contains("jev answer 99 ignored: stale")));
}

#[test]
fn save_and_restore_mid_rally_replays_exactly() {
    let mut game = game(true);
    let mut bot = Bot::default();
    rally_until_near_contact(&mut game, &mut bot);
    let saved = game.save().unwrap();
    let mut restored = Sim::<Tennis>::new(Options::default()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(restored.world().hash(), game.world().hash());
    let (mut a, mut b) = (Bot::default(), Bot::default());
    a.held = bot.held.clone();
    b.held = bot.held.clone();
    for _ in 0..(120 * 8) {
        a.step(&mut game, None);
        b.step(&mut restored, None);
    }
    assert_eq!(restored.world().hash(), game.world().hash());
    assert_eq!(restored.save().unwrap(), game.save().unwrap());
    assert!(
        phase(&game).rally >= 2 || phase(&game).points_played >= 1,
        "play went on"
    );
}

#[test]
fn same_inputs_same_match() {
    let run = || {
        let mut game = game(true);
        let mut bot = Bot::default();
        for _ in 0..(120 * 20) {
            bot.step(&mut game, None);
        }
        (game.world().hash(), phase(&game).points, phase(&game).games)
    };
    assert_eq!(run(), run());
}

#[test]
fn far_player_lets_out_balls_go_and_calls_them() {
    let mut game = game(true);
    let mut bot = Bot::default();
    rally_until_near_contact(&mut game, &mut bot);
    // Hit a clean forehand with the stick hard right: it sails wide.
    let p = *game.get::<Player>("near").unwrap();
    while game.world().tick() + CONTACT + 1 < p.contact {
        game.run(TICK);
    }
    bot.hold(&mut game, &[]);
    game.key_down("KeyD");
    game.tap(if p.hand == Hand::Backhand {
        "KeyK"
    } else {
        "KeyJ"
    });
    game.run(TICK * 30.0);
    game.key_up("KeyD");
    let before = phase(&game).points;
    game.run(3000.0);
    let m = phase(&game);
    let journal: Vec<_> = game
        .world()
        .journal()
        .iter()
        .map(|e| e.line.clone())
        .collect();
    assert!(
        m.points[1] > before[1] || journal.iter().any(|l| l.contains("point Jev")),
        "{journal:?}"
    );
    let _ = Transform::default();
}
