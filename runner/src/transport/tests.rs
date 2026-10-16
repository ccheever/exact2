//! The transport through a runner (LLP 1125 §5): the seek rule over
//! polymeter, a count-in, a tempo edit, a key restart, a stop and a frame
//! task; an edit's retraction; tempo without a jump in phase; a dropped
//! step; the waiting cap and the voice bound; `ended=`; `exactTransport`'s
//! answer; a refused commit; a second sequence; `stopSounds`; the wake; and
//! the background stop. Then the review's cases: a refused background
//! stop, a refused commit's derives, a `from` past exact counting, taps far
//! ahead, an end already past, and haptics past the record's bound.

use crate::sound::{By, Voice};
use crate::{DataError, DataSource, Runner};
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use std::path::PathBuf;

pub(crate) const SOURCE: &str = r#"sound "assets/kick.wav"
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

shape Hit
  src: string
  at: number
  gain: number
  group: string

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
  state farN = 0
  resource far = farTaps(farN) as shape Hit[]
  derive double = bpm * 2
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
  action badTempo
    bpm = 150
    take = 0 / 0
  action farFrom
    from = 100000000000000000000
  action late
    from = 8
    loops = 1
  action farOn
    farN = 256
  action tapFar
    playSounds(far)
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

/// `crowd(n)`: 33 kicks at beat 0, or `n` clicks spread over 2 beats; 7
/// steps once `poisoned`.
#[derive(Default)]
pub(crate) struct NoData {
    pub(crate) poisoned: bool,
}

impl DataSource for NoData {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let n = match args.first() {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        // `farTaps(n)`: `n` kicks from 100 s ahead, 300 ms apart.
        if source == "farTaps" {
            let hit = |i: usize| {
                Value::record(vec![
                    Value::str("assets/kick.wav"),
                    Value::Number(100_000.0 + 300.0 * i as f64),
                    Value::Number(1.0),
                    Value::str(""),
                ])
            };
            return Ok(Value::list((0..n).map(hit).collect()));
        }
        assert_eq!(source, "crowd");
        let n = if self.poisoned { 7 } else { n };
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
                    // 2000 haptics over 2 beats, and no sample.
                    2000 => {
                        let Value::Record(f) = step(2.0 * i as f64 / n as f64, "") else {
                            unreachable!()
                        };
                        let mut f = f.to_vec();
                        f[8] = Value::str("selection");
                        Value::record(f)
                    }
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

pub(crate) fn boot_with(source: &str) -> Runner<NoData> {
    Runner::boot(
        compiled(source),
        NoData::default(),
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

pub(crate) fn voices(r: &Runner<NoData>) -> Vec<Voice> {
    r.sounds().voices().cloned().collect()
}

/// The journal's lines the transport and the voice table wrote, unstamped.
pub(crate) fn said(r: &Runner<NoData>) -> Vec<String> {
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

pub(crate) fn answered(r: &Runner<NoData>) -> Vec<Value> {
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

/// Review G1: a background stop whose commit is refused (a task key that is
/// no key while the page is hidden) leaves the sequence playing, its
/// voices, its answer and the output's ops as they were.
#[test]
fn a_refused_background_stop_stops_nothing() {
    let source = SOURCE
        .replace(
            "  resource transport = exactTransport() as shape Transport\n",
            "  resource transport = exactTransport() as shape Transport\n  resource page = exactPage() as shape Page\n",
        )
        .replace(
            "shape Hit\n",
            "shape Page\n  visibilityState: string\n\nshape Hit\n",
        )
        .replace(
            "  sequence groove",
            "  task hidden when page.visibilityState == \"hidden\" key=0 / 0\n    after(1000, noop)\n  sequence groove",
        );
    let mut r = boot_with(&source);
    r.act("start", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    r.take_sounds();
    let before = (voices(&r), r.transport().answer());
    let page = crate::Page {
        hidden: true,
        suspended: true,
        ..r.page()
    };
    assert!(r.set_page(page).is_err());
    assert!(r.transport().playing());
    assert_eq!((voices(&r), r.transport().answer()), before);
    assert!(r.take_sounds().is_empty(), "no End reached the output");
    assert!(!r.page().suspended);
}

/// Review A1: a refused commit with a keyed sequence and no gated task puts
/// back what its settlement published, the derives included.
#[test]
fn a_refused_commit_puts_its_derives_back() {
    let mut r = boot();
    r.act("start", vec![]).unwrap();
    let derives = |r: &Runner<NoData>| {
        let state = crate::agent::handle(r, r#"{"op":"state"}"#);
        let at = state.find("\"double\":").unwrap();
        state[at..at + 13].to_string()
    };
    let before = derives(&r);
    assert!(r.act("badTempo", vec![]).is_err());
    assert_eq!(derives(&r), before);
    assert_eq!(r.transport().answer().bpm, 120.0);
}

/// Review A2: a `from` past 2^31 beats, where a loop's index would stop
/// counting exactly, does not play.
#[test]
fn a_from_past_exact_counting_does_not_play() {
    let mut r = boot();
    r.act("farFrom", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert!(!r.transport().playing());
    assert!(
        said(&r).contains(
            &"sequence groove did not play: from 100000000000000000000 is beyond 2^31 beats"
                .to_string()
        ),
        "{:?}",
        said(&r)
    );
    r.advance(5000.0).unwrap();
}

/// Review A3: 256 taps waiting 100 s ahead do not starve the sequence: a
/// blocked hit is issued at its own time, when it no longer waits.
#[test]
fn taps_far_ahead_do_not_starve_a_sequence() {
    let mut r = boot();
    r.act("farOn", vec![]).unwrap();
    r.act("tapFar", vec![]).unwrap();
    assert_eq!(r.sounds().waiting(0.0), 256);
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert_eq!(r.transport().wake_due(), 500.0);
    r.advance(3000.0).unwrap();
    let hats = at(&r, "assets/hat.wav");
    assert!(hats.contains(&1000.0) && hats.contains(&2000.0), "{hats:?}");
    assert!(at(&r, "assets/kick.wav").contains(&1500.0));
}

/// Review A6: a last track already past at the arming (`from` beyond it)
/// ends at the arming's time, never back in time.
#[test]
fn an_end_already_past_ends_now() {
    let mut r = boot();
    r.advance(1000.0).unwrap();
    r.act("late", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert_eq!(r.transport().ended_due(), 1000.0);
    r.advance(1000.0).unwrap();
    assert_eq!(slot(&r, "endedAt"), 1000.0);
    assert_eq!(r.now_ms(), 1000.0);
}

/// Review A7: haptics still to come are kept past the record's bound, so a
/// stop retracts every one of them.
#[test]
fn a_stop_retracts_every_haptic_still_to_come() {
    let mut r = boot();
    r.act("crowd", vec![Value::Number(2000.0)]).unwrap();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert_eq!(r.sounds().haptics().len(), 2000);
    r.act("halt", vec![]).unwrap();
    assert!(said(&r).contains(&"haptics retracted: 2000 (cancelled)".to_string()));
    assert!(r.sounds().haptics().all(|h| h.by == Some("cancelled")));
}

/// Review rounds 2 and 3, A2: a beat kept across an edit (a long track at
/// a high tempo, then short ones) past exact loop counting drops the track
/// that loops forever, journaled, rather than hang a fill; a finite sibling,
/// never iterated there, plays on and its `ended=` runs.
#[test]
fn a_beat_past_exact_counting_drops_only_the_endless_track() {
    let source = SOURCE
        .replace("  state stray = 0\n", "  state stray = 0\n  state huge = false\n  state split = false\n")
        .replace(
            "  derive tracks = dense > 0 ?",
            "  derive tracks = huge ? [Track(beats=100000000000000, loops=0, offset=0, output=\"\", steps=[hit(0, \"assets/kick.wav\")])] : split ? [Track(beats=1, loops=0, offset=0, output=\"\", steps=[hit(0, \"assets/kick.wav\")]), Track(beats=4, loops=1, offset=0, output=\"\", steps=[hit(0, \"assets/hat.wav\")])] : dense > 0 ?",
        )
        .replace("  action noop\n", "  action goHuge\n    huge = true\n    bpm = 6000000000000000000\n  action small\n    huge = false\n    split = true\n    bpm = 60\n  action noop\n");
    let mut r = boot_with(&source);
    r.act("noCountIn", vec![]).unwrap();
    r.act("goHuge", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    r.act("small", vec![]).unwrap();
    let dropped: Vec<String> = said(&r)
        .into_iter()
        .filter(|l| l.ends_with("is past exact loop counting"))
        .collect();
    assert_eq!(dropped.len(), 1, "the endless track only: {:?}", said(&r));
    assert!(dropped[0].starts_with("sequence groove: a track dropped: beat "));
    // The finite track's last loop is long behind: it ends at the edit's time.
    assert_eq!(r.transport().ended_due(), 1000.0);
    r.advance(1000.0).unwrap();
    assert_eq!(slot(&r, "endedAt"), 1000.0);
    assert_eq!(answered(&r)[1], Value::str("ended"));
}

/// Review round 2, A3 on a live clock: a hit the waiting cap held is
/// issued at its wake even when the wake runs just after its time; only a
/// stall's hits, more than 500 ms past, are skipped.
#[test]
fn a_held_hit_plays_at_a_late_wake_on_a_live_clock() {
    use std::{cell::Cell, rc::Rc};
    let mut r = boot();
    let now = Rc::new(Cell::new(0.0));
    let clock = now.clone();
    r.set_live_clock(Some(Box::new(move || clock.get())));
    r.act("farOn", vec![]).unwrap();
    r.act("tapFar", vec![]).unwrap();
    r.act("noCountIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    for t in [500.01, 1000.01, 1500.01, 2000.01] {
        now.set(t);
        r.advance(t).unwrap();
    }
    let hats: Vec<f64> = voices(&r)
        .iter()
        .filter(|v| v.src == "assets/hat.wav")
        .map(|v| v.at)
        .collect();
    assert!(
        hats.iter().any(|t| (1000.0..1001.0).contains(t)),
        "{hats:?}"
    );
    assert!(
        said(&r).iter().all(|l| !l.contains("late, skipped")),
        "{:?}",
        said(&r)
    );
}

mod clock;
mod midi;
