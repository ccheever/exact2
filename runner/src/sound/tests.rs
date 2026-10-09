//! The voice table through a runner (LLP 1096 §5): retrigger, past starts,
//! groups by start time, stops, the bound, drops, a refused commit, reload,
//! the seek rule, no wake, and the record.

use crate::sound::{By, SoundOp, Voice};
use crate::{DataError, DataSource, Runner};
use exact_kernel::Kernel;
use exact_plan::Value;
use std::path::PathBuf;

const SOURCE: &str = r#"sound "assets/kick.wav"
sound "assets/hat.wav"

shape Hit
  src: string
  at: number
  gain: number
  group: string

component Kit
  state n = 0
  state ahead = 0
  resource hits = hits(n) as shape list<Hit>
  action three
    playSound("assets/kick.wav")
    playSound("assets/kick.wav")
    playSound("assets/kick.wav")
  action list
    playSounds(hits)
  action past
    playSound("assets/kick.wav", at=performanceNow() - 10)
  action computed(s: string, a: number, g: number)
    playSound(s, at=a == -1 ? 0 / 0 : a, gain=g == -2 ? 1 / 0 : g)
  action count(k: number)
    n = k
  action hat(ms: number)
    playSound("assets/hat.wav", at=performanceNow() + ms, group="hat")
  action open(ms: number)
    playSound("assets/kick.wav", at=performanceNow() + ms, group="hat")
  action stop
    stopSounds()
  action stopHats
    stopSounds(group="hat")
  action bad
    playSound("assets/kick.wav")
    n = 99
  action again
    reload()
  action window
    playSounds(hits)
    ahead = performanceNow() + 100
  task clock mount
    every(25, window)
  view
    text "kit"
"#;

/// The kit's hits: `n` of them, 50 ms apart from the source's `at`, each in
/// group "g"; `99` answers a string, a shape refusal.
struct Kit {
    at: f64,
}

impl DataSource for Kit {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        assert_eq!(source, "hits");
        let n = match args.first() {
            Some(Value::Number(n)) => *n,
            _ => 0.0,
        };
        if n == 99.0 {
            return Ok(Value::str("not a list"));
        }
        Ok(Value::list(
            (0..n as usize)
                .map(|i| {
                    Value::record(vec![
                        Value::str("assets/hat.wav"),
                        Value::Number(self.at + 50.0 * i as f64),
                        Value::Number(1.0),
                        Value::str(""),
                    ])
                })
                .collect(),
        ))
    }
}

/// Write a 16-bit mono WAV of `ms` at 48 kHz.
fn wav(path: &std::path::Path, ms: u32) {
    let frames = 48 * ms;
    let data = frames * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    for x in [1u16, 1] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    for x in [2u16, 16] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    std::fs::write(path, b).unwrap();
}

/// The app's directory: the source and a 250 ms kick and 100 ms hat.
fn app(source: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "exact-sound-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    wav(&dir.join("assets/kick.wav"), 250);
    wav(&dir.join("assets/hat.wav"), 100);
    std::fs::write(dir.join("app.contract"), source).unwrap();
    dir
}

fn boot_with(source: &str, n: f64) -> Runner<Kit> {
    let dir = app(source);
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let mut r = Runner::boot(
        plan,
        Kit { at: 0.0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    if n > 0.0 {
        r.act("count", vec![Value::Number(n)]).unwrap();
    }
    r
}

fn boot() -> Runner<Kit> {
    boot_with(SOURCE, 0.0)
}

fn voices(r: &Runner<Kit>) -> Vec<Voice> {
    r.sounds().voices().cloned().collect()
}

fn ends(r: &Runner<Kit>) -> Vec<(u64, f64, By)> {
    r.sounds().voices().map(|v| (v.id, v.ends, v.by)).collect()
}

fn journal(r: &Runner<Kit>) -> Vec<String> {
    r.journal()
        .filter(|l| l.contains("sound"))
        .map(|l| l.split_once(' ').map_or(l, |(_, rest)| rest).to_string())
        .collect()
}

#[test]
fn every_call_is_a_new_voice() {
    let mut r = boot();
    r.act("three", vec![]).unwrap();
    let v = voices(&r);
    assert_eq!(v.iter().map(|v| v.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(v
        .iter()
        .all(|v| v.at == 0.0 && v.ends == 250.0 && v.by == By::End));
    // A list of three is three voices too, in list order.
    let mut r = boot_with(SOURCE, 3.0);
    r.act("list", vec![]).unwrap();
    let ats: Vec<f64> = voices(&r).iter().map(|v| v.at).collect();
    assert_eq!(ats, [0.0, 50.0, 100.0]);
    // The authored commands still reach the host, which skips them.
    let names: Vec<String> = r.take_commands().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["playSounds"]);
    let ops = r.take_sounds();
    assert_eq!(ops.len(), 3);
    assert!(
        matches!(ops[1], SoundOp::Play { id: 2, sound: 1, at, gain } if at == 50.0 && gain == 1.0)
    );
}

#[test]
fn a_past_start_is_now_and_says_what_was_asked() {
    let mut r = boot();
    r.advance(1000.0).unwrap();
    r.act("past", vec![]).unwrap();
    assert_eq!(voices(&r).last().unwrap().at, 1000.0);
    let lines = journal(&r);
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("assets/kick.wav at 1000 gain 1 (asked 990)")),
        "{lines:?}"
    );
}

#[test]
fn a_group_cuts_by_start_time_whatever_the_order_of_calls() {
    let mut r = boot();
    // 100 ms ahead first, then 50 ms ahead: the earlier start is cut by the later.
    r.act("hat", vec![Value::Number(100.0)]).unwrap();
    r.act("hat", vec![Value::Number(50.0)]).unwrap();
    assert_eq!(ends(&r), [(1, 200.0, By::End), (2, 100.0, By::Group)]);
    // A tie goes to the later call: the earlier never sounds.
    r.act("hat", vec![Value::Number(100.0)]).unwrap();
    assert_eq!(ends(&r)[0], (1, 100.0, By::Cut));
    assert_eq!(ends(&r)[2], (3, 200.0, By::End));
    // Another source in the same group chokes it.
    r.act("open", vec![Value::Number(120.0)]).unwrap();
    assert_eq!(ends(&r)[2], (3, 120.0, By::Group));
    let lines = journal(&r);
    assert!(
        lines.contains(&"sound 2 ends at 100 (group hat)".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"sound 1 ends at 100 (group hat)".to_string()),
        "{lines:?}"
    );
    // The output hears each end that moved earlier, once.
    let ops = r.take_sounds();
    assert!(ops.contains(&SoundOp::End { id: 1, at: 100.0 }));
    assert!(ops.contains(&SoundOp::End { id: 3, at: 120.0 }));
}

#[test]
fn stop_ends_the_sounding_and_cancels_the_waiting() {
    let mut r = boot();
    r.act("three", vec![]).unwrap();
    r.act("hat", vec![Value::Number(500.0)]).unwrap();
    r.advance(100.0).unwrap();
    r.act("stopHats", vec![]).unwrap();
    assert_eq!(ends(&r)[3], (4, 500.0, By::Cancelled));
    assert_eq!(ends(&r)[0], (1, 250.0, By::End));
    r.act("stop", vec![]).unwrap();
    assert_eq!(ends(&r)[0], (1, 100.0, By::Stop));
    let lines = journal(&r);
    assert!(
        lines.contains(&"sounds stopped: 1 (group hat)".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"sounds stopped: 3".to_string()),
        "{lines:?}"
    );
    assert_eq!(r.sounds().live(100.0), 0);
}

#[test]
fn the_thirty_third_live_voice_is_dropped_and_an_ended_one_frees_its_place() {
    let mut r = boot();
    for _ in 0..10 {
        r.act("three", vec![]).unwrap();
    }
    r.act("past", vec![]).unwrap();
    r.act("past", vec![]).unwrap();
    assert_eq!(r.sounds().live(0.0), 32);
    r.act("past", vec![]).unwrap();
    assert_eq!(voices(&r).len(), 32);
    assert!(journal(&r)
        .contains(&"sound dropped: 32 voices sound or wait (assets/kick.wav at 0)".to_string()));
    r.advance(250.0).unwrap();
    r.act("past", vec![]).unwrap();
    assert_eq!(voices(&r).last().unwrap().id, 33);
}

#[test]
fn a_bad_call_is_dropped_and_the_commit_stands() {
    let mut r = boot();
    let call = |r: &mut Runner<Kit>, s: &str, a: f64, g: f64| {
        r.act(
            "computed",
            vec![Value::str(s), Value::Number(a), Value::Number(g)],
        )
        .unwrap();
    };
    call(&mut r, "assets/nope.wav", 0.0, 1.0);
    // NaN and an infinity, as a computation makes them.
    call(&mut r, "assets/kick.wav", -1.0, 1.0);
    call(&mut r, "assets/kick.wav", 0.0, -2.0);
    // A finite gain outside 0–1 is clamped, not dropped.
    call(&mut r, "assets/kick.wav", 0.0, 3.0);
    assert_eq!(voices(&r).len(), 1);
    assert_eq!(voices(&r)[0].gain, 1.0);
    let lines = journal(&r);
    for want in [
        "sound dropped: \"assets/nope.wav\" is not a declared sound",
        "sound dropped: at is not a number (assets/kick.wav)",
        "sound dropped: gain is not a number (assets/kick.wav)",
    ] {
        assert!(lines.iter().any(|l| l == want), "{want}: {lines:?}");
    }
}

#[test]
fn a_refused_commit_schedules_nothing() {
    let mut r = boot();
    assert!(r.act("bad", vec![]).is_err());
    assert!(voices(&r).is_empty());
    assert!(r.take_sounds().is_empty());
    assert!(
        journal(&r).iter().all(|l| !l.starts_with("sound ")),
        "{:?}",
        journal(&r)
    );
}

#[test]
fn reload_ends_every_live_voice() {
    let mut r = boot();
    r.act("three", vec![]).unwrap();
    r.act("hat", vec![Value::Number(500.0)]).unwrap();
    r.take_sounds();
    r.act("again", vec![]).unwrap();
    assert!(journal(&r).contains(&"sounds stopped: 4 (reload)".to_string()));
    let ops = r.take_sounds();
    assert_eq!(ops.len(), 4, "{ops:?}");
    assert!(ops.contains(&SoundOp::End { id: 4, at: 500.0 }));
    // A host's dev reload asks the same of the runner it replaces.
    r.act("three", vec![]).unwrap();
    r.end_sounds();
    assert_eq!(r.sounds().live(0.0), 0);
}

#[test]
fn one_long_seek_and_many_short_ones_give_one_table() {
    let mut one = boot_with(SOURCE, 2.0);
    let mut many = boot_with(SOURCE, 2.0);
    one.advance(60_000.0).unwrap();
    for k in 1..=60 {
        many.advance(1_000.0 * k as f64).unwrap();
    }
    assert_eq!(voices(&one), voices(&many));
    assert_eq!(one.sounds().evicted(), many.sounds().evicted());
    // Each window's hits land at the commit's time: a timer's due time.
    assert_eq!(voices(&one).last().unwrap().at, 60_000.0);
}

#[test]
fn a_voice_is_not_a_timer() {
    let mut r = boot_with(
        &SOURCE.replace("  task clock mount\n    every(25, window)\n", ""),
        0.0,
    );
    assert_eq!(r.timer_due_ms(), None);
    r.act("hat", vec![Value::Number(5_000.0)]).unwrap();
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn the_record_keeps_1024_voices_and_counts_what_it_let_go() {
    let mut r = boot_with(
        &SOURCE.replace("  task clock mount\n    every(25, window)\n", ""),
        0.0,
    );
    for k in 0..400 {
        r.advance(300.0 * k as f64).unwrap();
        r.act("three", vec![]).unwrap();
    }
    assert_eq!(voices(&r).len(), 1024);
    assert_eq!(r.sounds().evicted().0, 1200 - 1024);
    assert_eq!(voices(&r)[0].id, 1200 - 1024 + 1);
    let state = crate::agent::handle(&r, r#"{"op":"state","sounds":"all"}"#);
    assert!(state.contains("\"recorded\":1024"), "{}", &state[..200]);
    let shown = crate::agent::handle(&r, r#"{"op":"state"}"#);
    assert_eq!(shown.matches("\"src\":").count(), 64);
}
