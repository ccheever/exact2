//! The transport through a runner (LLP 1125 §5): the seek rule over
//! polymeter, a count-in, a tempo edit, a key restart, a stop and a frame
//! task; an edit's retraction; tempo without a jump in phase; a dropped
//! step; the waiting cap and the voice bound; `ended=`; `exactTransport`'s
//! answer; a refused commit; a second sequence; `stopSounds`; the wake; and
//! the background stop.

use crate::sound::{By, Voice};
use crate::{DataError, DataSource, Runner};
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use std::path::PathBuf;

const SOURCE: &str = r#"sound "assets/kick.wav"
sound "assets/hat.wav"
sound "assets/click.wav"

shape Step
  beat: number
  src: string
  gain: number
  group: string
  note: number
  velocity: number
  channel: number
  duration: number
  haptic: string

shape Track
  beats: number
  loops: number
  offset: number
  output: string
  steps: Step[]

shape Transport
  state: string
  reason: string
  bpm: number
  anchorAt: number
  anchorBeat: number
  clock: string

fn hit(beat: number, src: string): Step = Step(beat=beat, src=src, gain=1, group="", note=-1, velocity=100, channel=0, duration=0.25, haptic="")
fn buzz(beat: number): Step = Step(beat=beat, src="", gain=1, group="", note=-1, velocity=100, channel=0, duration=0.25, haptic="impact-light")

component Seq
  state playing = false
  state bpm = 120
  state take = 0
  state loops = 0
  state from = -4
  state stray = 0
  state dense = 0
  state endedAt = -1
  state other = false
  resource transport = exactTransport() as shape Transport
  resource crowded = crowd(dense) as shape Step[]
  derive tracks = dense > 0 ? [Track(beats=dense == 33 ? 4 : 2, loops=0, offset=0, output="", steps=crowded)] : [Track(beats=3, loops=loops, offset=0, output="", steps=[hit(0, "assets/kick.wav"), buzz(1)]), Track(beats=4, loops=loops, offset=0, output="", steps=[hit(0, "assets/hat.wav"), hit(2, "assets/hat.wav"), hit(stray, "assets/hat.wav")])]
  action start
    playing = true
  action halt
    playing = false
  action toggle
    playing = !playing
  action faster
    bpm = 150
  action again
    take = take + 1
  action stepOut
    stray = 7
  action finite
    loops = 2
  action noCountIn
    from = 0
  action crowd(n: number)
    dense = n
  action badKey
    playing = true
    take = 0 / 0
  action finished
    endedAt = performanceNow()
  action quiet
    stopSounds()
  action second
    other = true
  action noop
    take = take
  sequence groove when playing key=take
    play(tracks, bpm=bpm, from=from, countIn=[hit(0, "assets/hat.wav"), hit(1, "assets/hat.wav"), hit(2, "assets/hat.wav"), hit(3, "assets/hat.wav")], ended=finished)
  sequence other when other
    play([Track(beats=1, loops=0, offset=0, output="", steps=[hit(0, "assets/kick.wav")])], bpm=60)
  view
    text `${transport.state}`
"#;

/// The timeline the seek rule drives: a tempo edit, a key restart, a stop
/// and a frame task, each its own task.
const TIMELINE: &str = "  task faster mount\n    after(30000, faster)\n  task again mount\n    after(40000, again)\n  task halt mount\n    after(50000, halt)\n  task draw when playing\n    every(frame, noop)\n";

/// `crowd(n)`: 33 kicks at beat 0, or `n` clicks spread over 2 beats.
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        assert_eq!(source, "crowd");
        let n = match args.first() {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let step = |beat: f64, src: &str| {
            let f = [beat, 0.0, 1.0, 0.0, -1.0, 100.0, 0.0, 0.25, 0.0];
            let mut v: Vec<Value> = f.iter().map(|n| Value::Number(*n)).collect();
            v[1] = Value::str(src);
            v[3] = Value::str("");
            v[8] = Value::str("");
            Value::record(v)
        };
        Ok(Value::list(
            (0..n)
                .map(|i| match n {
                    33 => step(0.0, "assets/kick.wav"),
                    _ => step(2.0 * i as f64 / n as f64, "assets/click.wav"),
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

/// The plan of `source` with a 250 ms kick, a 100 ms hat and a 2 ms
/// click, compiled once per source for the whole binary.
fn compiled(source: &str) -> Plan {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    static PLANS: std::sync::Mutex<Vec<(String, Plan)>> = std::sync::Mutex::new(Vec::new());
    let mut plans = PLANS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, plan)) = plans.iter().find(|(s, _)| s == source) {
        return plan.clone();
    }
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "exact-transport-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    wav(&dir.join("assets/kick.wav"), 250);
    wav(&dir.join("assets/hat.wav"), 100);
    wav(&dir.join("assets/click.wav"), 2);
    std::fs::write(dir.join("app.contract"), source).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    plans.push((source.to_owned(), plan.clone()));
    plan
}

fn boot_with(source: &str) -> Runner<NoData> {
    Runner::boot(
        compiled(source),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn boot() -> Runner<NoData> {
    boot_with(SOURCE)
}

fn timeline() -> String {
    SOURCE.replace("  sequence groove", &format!("{TIMELINE}  sequence groove"))
}

fn voices(r: &Runner<NoData>) -> Vec<Voice> {
    r.sounds().voices().cloned().collect()
}

/// The journal's lines the transport and the voice table wrote, unstamped.
fn said(r: &Runner<NoData>) -> Vec<String> {
    r.journal()
        .map(|l| l.split_once(' ').map_or(l, |(_, rest)| rest))
        .filter(|l| {
            ["sound", "sequence", "haptic"]
                .iter()
                .any(|w| l.starts_with(w))
        })
        .map(str::to_string)
        .collect()
}

fn at(r: &Runner<NoData>, src: &str) -> Vec<f64> {
    r.sounds()
        .voices()
        .filter(|v| v.src == src && v.by != By::Edit)
        .map(|v| v.at)
        .collect()
}

fn slot(r: &Runner<NoData>, name: &str) -> f64 {
    match r.slot(name) {
        Some(Value::Number(n)) => *n,
        other => panic!("{name}: {other:?}"),
    }
}

fn answered(r: &Runner<NoData>) -> Vec<Value> {
    match r.resource("transport") {
        Some(Value::Record(f)) => f.to_vec(),
        other => panic!("transport: {other:?}"),
    }
}

#[test]
fn one_long_seek_and_many_short_ones_give_one_record() {
    let source = timeline();
    let run = |steps: &[f64]| {
        let mut r = boot_with(&source);
        r.act("start", vec![]).unwrap();
        let mut lines = Vec::new();
        let mut seen = 0;
        for &to in steps {
            r.advance(to).unwrap();
            // Read the ring as it goes: one long seek fills it differently.
            let all = said(&r);
            lines.extend(all[seen.min(all.len())..].iter().cloned());
            seen = all.len();
        }
        let v: Vec<_> = voices(&r)
            .into_iter()
            .map(|v| (v.id, v.src, v.at, v.ends, v.by))
            .collect();
        let h: Vec<_> = r.sounds().haptics().cloned().collect();
        (v, h, lines, r.transport().answer())
    };
    let one = run(&[60_000.0]);
    let many = run(&(1..=60).map(|k| 1000.0 * k as f64).collect::<Vec<_>>());
    assert_eq!(one.0, many.0, "voices");
    assert_eq!(one.1, many.1, "haptics");
    assert_eq!(one.3, many.3, "the answer");
    // Polymeter: the kick every 3 beats and the hats every 2, from beat 0
    // at 2000 (four beats of count-in at 120), up to the tempo edit.
    let kicks: Vec<f64> = one
        .0
        .iter()
        .filter(|v| v.1 == "assets/kick.wav" && v.4 != By::Edit)
        .map(|v| v.2)
        .take(4)
        .collect();
    assert_eq!(kicks, [2000.0, 3500.0, 5000.0, 6500.0]);
    let hats: Vec<f64> = one
        .0
        .iter()
        .filter(|v| v.1 == "assets/hat.wav" && v.4 != By::Edit)
        .map(|v| v.2)
        .take(7)
        .collect();
    assert_eq!(hats, [0.0, 500.0, 1000.0, 1500.0, 2000.0, 2000.0, 3000.0]);
    // Nothing sounds after the stop at 50000.
    assert!(one
        .0
        .iter()
        .filter(|v| v.4 != By::Edit && v.4 != By::Cancelled)
        .all(|v| v.2 < 50_000.0));
    assert!(
        one.2.iter().any(|l| l == "sequence groove edited: bpm 150"),
        "{:?}",
        one.2
    );
    assert!(one.2.iter().any(|l| l == "sequence groove stopped (gate)"));
}

#[test]
fn an_edit_retracts_what_has_not_started_and_expands_from_its_commit() {
    let mut r = boot();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(1750.0).unwrap();
    // Issued to 2500 by the fill at 1500: two hats and a haptic at 2000
    // wait. The edit drops the stray hat.
    r.act("stepOut", vec![]).unwrap();
    let edited: Vec<_> = voices(&r)
        .into_iter()
        .filter(|v| v.by == By::Edit)
        .collect();
    assert_eq!(edited.len(), 2);
    assert!(edited.iter().all(|v| v.at == 2000.0 && v.ends == v.at));
    assert!(voices(&r).iter().any(|v| v.at == 1500.0 && v.by == By::End));
    // The stray step at beat 7 is outside its track's 4 beats: dropped and said once.
    let lines = said(&r);
    let dropped = "sequence groove: track 2 step 3 dropped: beat 7 is outside [0, 4)";
    assert_eq!(
        lines.iter().filter(|l| *l == dropped).count(),
        1,
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("sounds retracted: ")),
        "{lines:?}"
    );
    r.advance(3000.0).unwrap();
    assert_eq!(lines.iter().filter(|l| *l == dropped).count(), 1);
    // One hat at 2000 is issued again after the edit, and the haptic.
    assert_eq!(
        at(&r, "assets/hat.wav")
            .iter()
            .filter(|t| **t == 2000.0)
            .count(),
        1
    );
    assert!(r
        .sounds()
        .haptics()
        .any(|h| h.at == 2000.0 && h.by == Some("edit")));
    assert!(r
        .sounds()
        .haptics()
        .any(|h| h.at == 2000.0 && h.state(3000.0) == "played"));
}

#[test]
fn a_tempo_moves_without_a_jump_in_phase() {
    let mut r = boot();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(1250.0).unwrap();
    r.act("faster", vec![]).unwrap();
    r.advance(5000.0).unwrap();
    // Beat 2.5 at 1250; at 150 a beat is 400 ms: beat 3 (the kick) at 1450,
    // beat 4 (a hat) at 1850, beat 6 at 2650.
    let kicks = at(&r, "assets/kick.wav");
    assert_eq!(&kicks[..3], [0.0, 1450.0, 2650.0]);
    assert!(at(&r, "assets/hat.wav").contains(&1850.0));
    let (segments, recorded) = r.transport().segments();
    assert_eq!(recorded, 2);
    let last = segments.last().unwrap();
    assert_eq!((last.at, last.beat, last.bpm), (1250.0, 2.5, 150.0));
}

#[test]
fn the_waiting_cap_stops_expanding_early_and_drops_nothing() {
    let mut r = boot();
    r.act("crowd", vec![Value::Number(300.0)]).unwrap();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert_eq!(r.sounds().waiting(0.0), 256);
    // It wakes when the first waiting voice starts.
    let first = 1000.0 / 300.0;
    assert_eq!(r.transport().wake_due(), first);
    r.advance(1000.0).unwrap();
    let mut clicks = at(&r, "assets/click.wav");
    clicks.retain(|t| *t < 1000.0);
    assert_eq!(clicks.len(), 300, "every hit of the first second, once");
    assert!(clicks.windows(2).all(|w| w[0] < w[1]));
    assert!(
        said(&r).iter().all(|l| !l.starts_with("sound dropped")),
        "{:?}",
        said(&r)
    );
}

#[test]
fn the_thirty_third_voice_sounding_at_a_start_is_dropped() {
    let mut r = boot();
    r.act("crowd", vec![Value::Number(33.0)]).unwrap();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    let at0 = voices(&r).iter().filter(|v| v.at == 0.0).count();
    assert_eq!(at0, 32);
    assert!(said(&r).contains(
        &"sound dropped: 32 voices sound at its start (assets/kick.wav at 0)".to_string()
    ));
}

#[test]
fn ended_runs_at_the_last_track_s_end() {
    let mut r = boot();
    r.act("finite", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    // Count-in to 2000; the longer track, twice 4 beats, ends at 6000.
    assert_eq!(r.transport().ended_due(), 6000.0);
    r.advance(10_000.0).unwrap();
    assert_eq!(slot(&r, "endedAt"), 6000.0);
    assert_eq!(answered(&r)[0], Value::str("stopped"));
    assert_eq!(answered(&r)[1], Value::str("ended"));
    // The gate still holds: it stays stopped.
    r.advance(20_000.0).unwrap();
    assert!(voices(&r).iter().all(|v| v.at < 6000.0));
}

#[test]
fn exact_transport_is_answered_by_a_commit_of_its_own() {
    let mut r = boot();
    assert_eq!(answered(&r)[0], Value::str("stopped"));
    r.act("start", vec![]).unwrap();
    // One commit behind, as an answer's `then`: the next commit answers.
    assert_eq!(answered(&r)[0], Value::str("stopped"));
    r.land_then().error.map_or(Ok(()), Err).unwrap();
    let a = answered(&r);
    assert_eq!(
        a,
        [
            Value::str("countin"),
            Value::str(""),
            Value::Number(120.0),
            Value::Number(0.0),
            Value::Number(-4.0),
            Value::str("internal")
        ]
    );
    // Beat 0 at 2000: playing.
    r.advance(2000.0).unwrap();
    assert_eq!(answered(&r)[0], Value::str("playing"));
}

#[test]
fn a_refused_commit_changes_nothing_that_sounds() {
    let mut r = boot();
    assert!(r.act("badKey", vec![]).is_err());
    assert!(voices(&r).is_empty());
    assert!(r.take_sounds().is_empty());
    assert!(!r.transport().playing());
}

#[test]
fn a_second_sequence_does_not_play_while_one_does() {
    let mut r = boot();
    r.act("start", vec![]).unwrap();
    r.act("second", vec![]).unwrap();
    assert!(
        said(&r).contains(&"sequence other did not play: sequence groove is playing".to_string())
    );
    r.act("halt", vec![]).unwrap();
    // Its gate held while the first played: it waits for a new arming.
    assert!(!r.transport().playing());
}

#[test]
fn stop_sounds_stops_what_sounds_and_the_next_steps_play() {
    let mut r = boot();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(100.0).unwrap();
    r.act("quiet", vec![]).unwrap();
    let v = voices(&r);
    let kick = v.iter().find(|v| v.src == "assets/kick.wav").unwrap();
    assert_eq!((kick.by, kick.ends), (By::Stop, 100.0));
    // The hat at 1000 waits at 600: it is the sequence's next step.
    r.advance(600.0).unwrap();
    r.act("quiet", vec![]).unwrap();
    let hat = voices(&r).into_iter().find(|v| v.at == 1000.0).unwrap();
    assert_eq!(hat.by, By::End);
    r.advance(3000.0).unwrap();
    assert!(at(&r, "assets/hat.wav").contains(&2000.0));
}

#[test]
fn the_wake_is_a_timer_the_settle_does_not_wait_for() {
    let mut r = boot_with(
        &SOURCE
            .replace(
                "  resource transport = exactTransport() as shape Transport\n",
                "",
            )
            .replace("${transport.state}", "seq"),
    );
    assert_eq!(r.timer_due_ms(), None);
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(500.0));
    // A fill makes no commit.
    let a = r.advance(500.0).unwrap();
    assert!(a.is_empty());
    assert_eq!(r.transport().issued_to(), 1500.0);
}

#[test]
fn the_background_stops_it_until_its_gate_turns_true_again() {
    let mut r = boot();
    r.act("start", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    let page = crate::Page {
        suspended: true,
        ..r.page()
    };
    assert!(r.set_page(page).unwrap().is_some());
    assert!(!r.transport().playing());
    r.land_then();
    assert_eq!(answered(&r)[1], Value::str("background"));
    let waiting = voices(&r)
        .iter()
        .filter(|v| v.at > 1000.0 && v.by != By::Cancelled)
        .count();
    assert_eq!(waiting, 0);
    let page = crate::Page {
        suspended: false,
        ..r.page()
    };
    r.set_page(page).unwrap();
    r.advance(3000.0).unwrap();
    assert!(
        !r.transport().playing(),
        "latched until the gate turns true again"
    );
    r.act("toggle", vec![]).unwrap();
    r.act("toggle", vec![]).unwrap();
    assert!(r.transport().playing());
}
