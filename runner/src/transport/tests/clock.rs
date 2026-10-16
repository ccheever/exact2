//! Clock in through a runner (LLP 1125 D9): the tempo window, Start with a
//! running clock and from rest, a count-in counted in ticks, Stop, Continue
//! and Song Position, a tempo change's re-anchoring and jitter's none, a key
//! restart, the gate while waiting, clock out ignored, the inputs opened,
//! what is dispatched, and replay: the same deliveries give the same record.

use super::compiled;
use crate::midi::MidiOp;
use crate::sound::By;
use crate::transport::clock::{Follow, Heard};
use crate::{DataError, DataSource, Runner};
use exact_kernel::Kernel;
use exact_plan::Value;

const SOURCE: &str = r#"sound "assets/kick.wav"

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

shape Port
  id: string
  name: string
  state: string

shape MIDI
  state: string
  inputs: Port[]
  outputs: Port[]

shape Transport
  state: string
  reason: string
  bpm: number
  anchorAt: number
  anchorBeat: number
  clock: string

fn kick(beat: number): Step = Step(beat=beat, src="assets/kick.wav", gain=1, group="", note=36, velocity=100, channel=9, duration=0.25, haptic="")

component C
  state playing = false
  state take = 0
  state input = "clk"
  state from = 0
  state loops = 0
  state clockOut = ""
  state heard = 0
  state beatSeen = -2
  state endedAt = -1
  resource midi = exactMIDI() as shape MIDI
  resource transport = exactTransport() as shape Transport
  action ask
    requestMIDIAccess()
  action start
    playing = true
  action halt
    playing = false
  action again
    take = take + 1
  action other
    input = "keys"
  action countIn
    from = -4
  action finite
    loops = 2
  action withClockOut
    clockOut = "synth"
  action hear(e: MIDIMessageEvent)
    heard = heard + 1
    beatSeen = e.beat
  action finished
    endedAt = performanceNow()
  sequence s when playing key=take
    play([Track(beats=1, loops=loops, offset=0, output="synth", steps=[kick(0)])], from=from, clock=input, clockOut=clockOut, ended=finished)
  view
    column midimessage=hear midiinputs=["keys"]
      text `${transport.state}`
"#;

struct Granted;

impl DataSource for Granted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("no source {source}")
    }

    fn grants(&self) -> &str {
        "device.midi purpose.midi"
    }
}

/// A runner with access granted: `clk` and `keys` inputs, `synth` an
/// output.
fn boot(source: &str) -> Runner<Granted> {
    let mut r = Runner::boot(
        compiled(source),
        Granted,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    agent(
        &mut r,
        r#"{"op":"prefer","midi":{"outputs":["synth"],"inputs":["clk","keys"]}}"#,
    )
    .unwrap();
    r.act("ask", vec![]).unwrap();
    r.land_then();
    r
}

fn agent(r: &mut Runner<Granted>, json: &str) -> Result<String, String> {
    let (_, reply) = r.midi_agent(json).unwrap();
    r.land_then();
    reply
}

/// One delivery from `clk` at the clock's time: `(stamp, byte)` each.
fn deliver(r: &mut Runner<Granted>, msgs: &[(f64, u8)]) {
    let list: Vec<String> = msgs.iter().map(|(t, b)| format!("[{t},{b}]")).collect();
    let json = format!(
        r#"{{"op":"midi","input":"clk","clock":[{}]}}"#,
        list.join(",")
    );
    agent(r, &json).unwrap();
}

/// One message from `clk`, stamped now.
fn now(r: &mut Runner<Granted>, byte: u8) {
    let t = r.now_ms();
    deliver(r, &[(t, byte)]);
}

/// The driver's `tap midi "clk" clock <bpm> <ms>`: ticks one interval
/// apart from the clock's time, delivered a 50 ms slice at a time.
fn clock(r: &mut Runner<Granted>, bpm: f64, ms: f64) {
    let (start, interval) = (r.now_ms(), 60000.0 / (24.0 * bpm));
    let (mut k, mut edge) = (1.0, start);
    while edge < start + ms {
        edge = (edge + 50.0).min(start + ms);
        let mut slice = Vec::new();
        while start + k * interval <= edge {
            slice.push((start + k * interval, 0xF8));
            k += 1.0;
        }
        r.advance(edge).unwrap();
        deliver(r, &slice);
    }
}

fn said<D: DataSource>(r: &Runner<D>) -> Vec<String> {
    r.journal()
        .map(|l| l.split_once(' ').map_or(l, |(_, rest)| rest))
        .filter(|l| {
            ["sequence", "sound", "midi"]
                .iter()
                .any(|w| l.starts_with(w))
        })
        .map(str::to_string)
        .collect()
}

/// Each kick: its start and how it ended.
fn kicks(r: &Runner<Granted>) -> Vec<(f64, By)> {
    r.sounds().voices().map(|v| (v.at, v.by)).collect()
}

fn answered(r: &Runner<Granted>) -> Vec<Value> {
    match r.resource("transport") {
        Some(Value::Record(f)) => f.to_vec(),
        other => panic!("transport: {other:?}"),
    }
}

fn number(r: &Runner<Granted>, name: &str) -> f64 {
    match r.slot(name) {
        Some(Value::Number(n)) => *n,
        other => panic!("{name}: {other:?}"),
    }
}

fn ticks(from: f64, interval: f64, n: usize) -> Vec<(f64, Vec<u8>)> {
    (0..n)
        .map(|k| (from + interval * k as f64, vec![0xF8]))
        .collect()
}

#[test]
fn the_window_says_a_tempo_from_thirteen_packet_leading_ticks() {
    let mut f = Follow::new("clk", 90.0, 0.0);
    let mut log = Vec::new();
    f.feed(&ticks(0.0, 20.0, 12), "s", &mut log);
    assert_eq!((f.estimate, f.tempo()), (None, 90.0), "the prior until 13");
    f.feed(&ticks(240.0, 20.0, 1), "s", &mut log);
    assert_eq!(f.estimate, Some(125.0));
    // Forty-eight at most; a stamp shared with the tick before (one
    // packet) says nothing.
    f.feed(&ticks(260.0, 25.0, 60), "s", &mut log);
    assert_eq!((f.window(), f.estimate), (48, Some(100.0)));
    let before = f.estimate;
    f.feed(&[(f.last_tick().unwrap(), vec![0xF8])], "s", &mut log);
    assert_eq!(f.estimate, before);
    // A gap past 500 ms, or a stamp back in time, starts the window over.
    f.feed(&ticks(5000.0, 20.0, 1), "s", &mut log);
    assert_eq!(f.window(), 1);
    f.feed(&ticks(5020.0, 20.0, 3), "s", &mut log);
    f.feed(&ticks(4000.0, 20.0, 1), "s", &mut log);
    assert_eq!(f.window(), 1);
    assert!(log.is_empty());
}

#[test]
fn start_falls_on_the_next_tick_or_at_its_own_time() {
    let mut f = Follow::new("clk", 120.0, -4.0);
    let mut log = Vec::new();
    // From rest: Start's own time.
    let h = f.feed(&[(1000.0, vec![0xFA])], "s", &mut log);
    assert_eq!(
        h,
        [Heard::Start {
            at: 1000.0,
            beat: -4.0,
            cont: false
        }]
    );
    // A running clock: the tick after the last one.
    let mut f = Follow::new("clk", 120.0, 0.0);
    let mut msgs = ticks(0.0, 20.0, 20);
    msgs.push((385.0, vec![0xFA]));
    msgs.push((400.0, vec![0xF8]));
    let h = f.feed(&msgs, "s", &mut log);
    assert!(
        matches!(h[0], Heard::Start { at, beat, .. } if at == 400.0 && beat == 0.0),
        "{h:?}"
    );
    assert!(
        matches!(h[1], Heard::Anchor(s) if s.at == 400.0 && s.beat == 0.0 && s.bpm == 125.0),
        "{h:?}"
    );
    // Stop holds where the ticks reached; Song Position while stopped is
    // where Continue resumes, ignored while playing.
    let h = f.feed(
        &[
            (420.0, vec![0xF8]),
            (430.0, vec![0xFC]),
            (440.0, vec![0xFB]),
        ],
        "s",
        &mut log,
    );
    assert_eq!(f.held, None, "Continue takes the held beat");
    assert!(
        matches!(h[2], Heard::Start { beat, cont: true, .. } if beat == 2.0 / 24.0),
        "{h:?}"
    );
    f.feed(&[(450.0, vec![0xF2, 0x10, 0x00])], "s", &mut log);
    assert_eq!(log, ["sequence s: song position ignored while playing"]);
    let h = f.feed(
        &[
            (460.0, vec![0xFC]),
            (470.0, vec![0xF2, 0x10, 0x01]),
            (480.0, vec![0xFB]),
        ],
        "s",
        &mut log,
    );
    // (1 << 7 | 16) sixteenths: 36 beats.
    assert!(
        matches!(h[1], Heard::Start { beat, .. } if beat == 36.0),
        "{h:?}"
    );
}

#[test]
fn a_count_in_counts_ticks_and_the_kick_lands_on_tick_ninety_six() {
    let mut r = boot(SOURCE);
    r.act("countIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.land_then();
    assert_eq!(answered(&r)[0], Value::str("waiting"));
    assert_eq!(answered(&r)[5], Value::str("midi"));
    assert!(r.midi_inputs().contains(&"clk".to_string()));
    assert!(r.take_midi().iter().any(
        |op| matches!(op, MidiOp::Open { inputs, clock } if inputs == &["keys", "clk"] && clock == &["clk"])
    ));
    assert!(kicks(&r).is_empty(), "nothing plays while waiting");
    clock(&mut r, 125.0, 1000.0);
    assert!(kicks(&r).is_empty());
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFA)]);
    // Start comes after the tick at `t`: beat -4 at the next one.
    let start = t + 20.0;
    assert_eq!(answered(&r)[0], Value::str("countin"));
    clock(&mut r, 125.0, 2500.0);
    let at: Vec<f64> = kicks(&r)
        .iter()
        .filter(|k| k.1 == By::End)
        .map(|k| k.0)
        .collect();
    assert_eq!(&at[..2], [start + 96.0 * 20.0, start + 120.0 * 20.0]);
    assert_eq!(answered(&r)[0], Value::str("playing"));
    assert_eq!(answered(&r)[2], Value::Number(125.0));
    assert!(said(&r).contains(&format!("sequence s: clock start at {start}, beat -4")));
    // Nothing re-anchored: a steady clock moves no hit.
    assert!(!said(&r).iter().any(|l| l.contains("re-anchored")));
}

#[test]
fn stop_leaves_it_waiting_and_continue_resumes_where_it_held() {
    let mut r = boot(SOURCE);
    r.act("start", vec![]).unwrap();
    r.land_then();
    clock(&mut r, 125.0, 400.0);
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFA)]);
    clock(&mut r, 125.0, 1000.0);
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFC)]);
    let a = answered(&r);
    assert_eq!(
        (&a[0], &a[1]),
        (&Value::str("waiting"), &Value::str("clock"))
    );
    assert!(said(&r).contains(&"sequence s stopped (clock)".to_string()));
    assert!(
        r.transport().clock_input() == Some("clk"),
        "still following"
    );
    assert!(
        r.midi().sent().all(|s| s.state(t) != "waiting"),
        "nothing left waiting"
    );
    // The FC came with no listener for `clk`: unrecorded.
    assert_eq!(r.midi().received().1, 0);
    // Continue: 50 ticks had come, so beat 50/24 at the next tick.
    clock(&mut r, 125.0, 100.0);
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFB)]);
    assert!(
        said(&r).contains(&format!(
            "sequence s: clock continue at {}, beat 2.0833333333333335",
            t + 20.0
        )),
        "{:?}",
        said(&r)
    );
    // Gate false while waiting: stopped (gate).
    now(&mut r, 0xFC);
    r.act("halt", vec![]).unwrap();
    r.land_then();
    let a = answered(&r);
    assert_eq!(
        (&a[0], &a[1]),
        (&Value::str("stopped"), &Value::str("gate"))
    );
    assert_eq!(r.transport().clock_input(), None);
}

#[test]
fn a_tempo_change_re_anchors_and_jitter_does_not() {
    // A kick every 1/8 beat, so the revocable band always holds one.
    let mut r = boot(&SOURCE.replace("Track(beats=1,", "Track(beats=0.125,"));
    r.act("start", vec![]).unwrap();
    r.land_then();
    clock(&mut r, 125.0, 400.0);
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFA)]);
    // ±1 ms of jitter on a steady 125: nothing moves.
    let mut at = r.now_ms();
    for k in 0..60 {
        at += 20.0;
        let jitter = if k % 2 == 0 { 1.0 } else { -1.0 };
        r.advance(at + 1.0).unwrap();
        deliver(&mut r, &[(at + jitter, 0xF8)]);
    }
    assert!(
        !said(&r).iter().any(|l| l.contains("re-anchored")),
        "{:?}",
        said(&r)
    );
    assert!(!kicks(&r).iter().any(|k| k.1 == By::Clock));
    // Ticks delivered 50 ms apart leave nothing revocable (the lookahead is
    // the batch plus the hold), so a slower clock re-anchors without
    // retracting.
    clock(&mut r, 100.0, 600.0);
    assert!(said(&r).iter().any(|l| l.contains("re-anchored")));
    assert!(!kicks(&r).iter().any(|k| k.1 == By::Clock));
    // Delivered one tick at a time, 125 to 100: the kicks not yet within
    // the hold are taken back by `clock` and issued again on the new grid,
    // and none within it moves.
    let mut at = r.now_ms();
    for _ in 0..120 {
        at += 25.0;
        r.advance(at).unwrap();
        let held: Vec<(f64, By)> = kicks(&r).into_iter().filter(|k| k.0 < at + 50.0).collect();
        deliver(&mut r, &[(at, 0xF8)]);
        let after: Vec<(f64, By)> = kicks(&r).into_iter().take(held.len()).collect();
        assert_eq!(held, after, "within the hold at {at}");
    }
    assert!(kicks(&r).iter().any(|k| k.1 == By::Clock));
    assert!(said(&r)
        .iter()
        .any(|l| l.starts_with("sounds retracted") && l.ends_with("(clock)")));
    // The answer moves by half a beat a minute or more (rulings #7).
    assert!(matches!(answered(&r)[2], Value::Number(b) if (b - 100.0).abs() < 0.5));
    assert_eq!(r.transport().follower().unwrap().estimate, Some(100.0));
    // The last kicks fall 75 ms (1/8 beat) apart.
    let live: Vec<f64> = kicks(&r)
        .iter()
        .filter(|k| k.1 == By::End && k.0 > at - 400.0)
        .map(|k| k.0)
        .collect();
    assert!(live.len() > 3);
    assert!(
        live.windows(2).all(|w| (w[1] - w[0] - 75.0).abs() < 1e-6),
        "{live:?}"
    );
}

#[test]
fn ended_runs_at_the_end_the_clock_predicts() {
    let mut r = boot(SOURCE);
    r.act("finite", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.land_then();
    clock(&mut r, 125.0, 400.0);
    let t = r.now_ms();
    deliver(&mut r, &[(t, 0xFA)]);
    clock(&mut r, 125.0, 950.0);
    r.advance(t + 1500.0).unwrap();
    // Two one-beat loops from the tick after Start, 480 ms a beat.
    assert_eq!(number(&r, "endedAt"), t + 20.0 + 960.0);
    let a = answered(&r);
    assert_eq!(
        (&a[0], &a[1]),
        (&Value::str("stopped"), &Value::str("ended"))
    );
    assert_eq!(r.transport().clock_input(), None);
}

#[test]
fn a_key_restart_waits_again_and_clock_out_is_ignored() {
    let mut r = boot(SOURCE);
    r.act("withClockOut", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.land_then();
    assert!(said(&r).contains(&"sequence s: clock out ignored: it follows clk".to_string()));
    now(&mut r, 0xFA);
    clock(&mut r, 125.0, 500.0);
    assert_eq!(answered(&r)[0], Value::str("playing"));
    assert_eq!(r.midi().clock().1, 0, "no clock out");
    r.act("again", vec![]).unwrap();
    r.land_then();
    assert_eq!(answered(&r)[0], Value::str("waiting"));
    // Another input: a restart too, following it.
    r.act("other", vec![]).unwrap();
    r.land_then();
    assert_eq!(r.transport().clock_input(), Some("keys"));
    assert_eq!(
        agent(&mut r, r#"{"op":"midi","input":"clk","clock":[]}"#).unwrap_err(),
        "tap midi: no sequence follows \"clk\" (clock=)"
    );
}

#[test]
fn transport_messages_are_heard_with_their_beat_and_ticks_never() {
    let source = SOURCE.replace("midiinputs=[\"keys\"]", "midiinputs=[\"clk\"]");
    let mut r = boot(&source);
    r.act("start", vec![]).unwrap();
    r.land_then();
    clock(&mut r, 125.0, 200.0);
    let t = r.now_ms();
    agent(&mut r, r#"{"op":"midi","input":"clk","data":[250]}"#).unwrap();
    assert_eq!(number(&r, "heard"), 1.0);
    // Start at the next tick: its own stamp is a tick's length before beat 0.
    assert!((number(&r, "beatSeen") + 20.0 / 480.0).abs() < 1e-9);
    clock(&mut r, 125.0, 200.0);
    agent(&mut r, r#"{"op":"midi","input":"clk","data":[252]}"#).unwrap();
    assert_eq!((number(&r, "heard"), number(&r, "beatSeen")), (2.0, -1.0));
    let (received, heard) = r.midi().received();
    assert_eq!(heard, 2);
    assert!(received.into_iter().all(|m| m.data != [0xF8]));
    let _ = t;
}

/// The same deliveries into two runners give the same record, and between
/// deliveries one long advance equals many short ones.
#[test]
fn the_same_deliveries_give_the_same_record() {
    let run = |steps: u32| {
        let mut r = boot(SOURCE);
        r.act("countIn", vec![]).unwrap();
        r.act("start", vec![]).unwrap();
        r.land_then();
        clock(&mut r, 125.0, 300.0);
        now(&mut r, 0xFA);
        clock(&mut r, 125.0, 2000.0);
        clock(&mut r, 100.0, 1000.0);
        // A gap: the clock is lost a while, then comes back.
        let t = r.now_ms();
        for k in 1..=steps {
            r.advance(t + 700.0 * f64::from(k) / f64::from(steps))
                .unwrap();
        }
        clock(&mut r, 100.0, 400.0);
        now(&mut r, 0xFC);
        clock(&mut r, 100.0, 200.0);
        now(&mut r, 0xFB);
        clock(&mut r, 100.0, 1000.0);
        let sent: Vec<_> = r
            .midi()
            .sent()
            .map(|s| (crate::midi::hex(&s.data), s.at, s.by))
            .collect();
        (
            kicks(&r),
            sent,
            said(&r),
            answered(&r),
            crate::agent::state(&r),
        )
    };
    let one = run(1);
    assert_eq!(one, run(1));
    assert_eq!(one, run(70));
    assert!(!one.0.is_empty());
}
