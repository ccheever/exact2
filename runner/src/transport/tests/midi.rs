//! MIDI through a runner (LLP 1125 D6–D8): access and its grant, `sendMIDI`
//! and its refusals, a sequence's notes with a track's offset and the seek
//! rule, an edit's and a stop's retractions, overlapping notes, clock out,
//! and `midimessage` with coalescing and every listener's own commit.

use super::compiled;
use crate::midi::{Incoming, MidiOp, Sent};
use crate::{DataError, DataSource, Runner};
use exact_kernel::Kernel;
use exact_plan::Value;

const SOURCE: &str = r#"shape Step
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

fn note(beat: number, n: number, length: number): Step = Step(beat=beat, src="", gain=1, group="", note=n, velocity=100, channel=0, duration=length, haptic="")

component M
  state playing = false
  state bpm = 120
  state take = 0
  state off = 0
  state length = 0.5
  state clockOut = ""
  state heard = 0
  state heard2 = 0
  state beatSeen = -2
  state sum = 0
  state bad = [144, 200, 1]
  resource midi = exactMIDI() as shape MIDI
  action ask
    requestMIDIAccess()
  action send
    sendMIDI("synth", [144, 60, 100, 128, 60, 0], at=performanceNow() + 500)
  action sendLate
    sendMIDI("synth", [176, 7, 100], at=performanceNow() - 10)
  action sendBad
    sendMIDI("synth", bad)
  action sendNowhere
    sendMIDI("nowhere", [144, 60, 100])
  action start
    playing = true
  action halt
    playing = false
  action again
    take = take + 1
  action faster
    bpm = 240
  action early
    off = -100
  action longer
    length = 2
  action withClock
    clockOut = "synth"
  action hear(e: MIDIMessageEvent)
    heard = heard + 1
    beatSeen = e.beat
    sum = sum + (match e.data.at(2) { case some(v) => v, case none => 0 })
  action hear2(e: MIDIMessageEvent)
    heard2 = heard2 + 1
  sequence s when playing key=take
    play([Track(beats=1, loops=0, offset=off, output="synth", steps=[note(0, 60, length)])], bpm=bpm, clockOut=clockOut)
  view
    column midimessage=hear midiinputs=["keys"]
      text `${midi.state} ${midi.outputs.length}`
      column midimessage=hear2 midiinputs=["keys", "pads"]
        text "b"
"#;

struct Granted;

impl DataSource for Granted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("no data source {source}")
    }

    fn grants(&self) -> &str {
        "device.midi purpose.midi"
    }
}

struct Ungranted;

impl DataSource for Ungranted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("no data source {source}")
    }
}

fn boot_with(source: &str) -> Runner<Granted> {
    Runner::boot(
        compiled(source),
        Granted,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A runner with access granted, `synth` an output and `keys`, `pads`
/// inputs.
fn granted(source: &str) -> Runner<Granted> {
    let mut r = boot_with(source);
    let (_, reply) = r
        .midi_agent(r#"{"op":"prefer","midi":{"outputs":["synth"],"inputs":["keys","pads"]}}"#)
        .unwrap();
    reply.unwrap();
    r.act("ask", vec![]).unwrap();
    r.land_then();
    r
}

fn said<D: DataSource>(r: &Runner<D>) -> Vec<String> {
    r.journal()
        .map(|l| l.split_once(' ').map_or(l, |(_, rest)| rest))
        .filter(|l| l.starts_with("midi") || l.starts_with("sequence"))
        .map(str::to_string)
        .collect()
}

fn sent<D: DataSource>(r: &Runner<D>) -> Vec<(String, f64, Option<&'static str>)> {
    r.midi()
        .sent()
        .map(|s: &Sent| (crate::midi::hex(&s.data), s.at, s.by))
        .collect()
}

fn text<D: DataSource>(r: &Runner<D>, name: &str) -> f64 {
    match r.slot(name) {
        Some(Value::Number(n)) => *n,
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn access_needs_the_grant_and_the_agent_answers_a_request() {
    let mut r = Runner::boot(
        compiled(SOURCE),
        Ungranted,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.midi().ports().access.word(), "denied");
    r.act("ask", vec![]).unwrap();
    assert!(said(&r)
        .iter()
        .any(|l| l.contains("grants name no device.midi")));
    let mut r = boot_with(SOURCE);
    assert_eq!(r.midi().ports().access.word(), "prompt");
    r.act("ask", vec![]).unwrap();
    r.land_then();
    match r.resource("midi") {
        Some(Value::Record(f)) => assert_eq!(f[0], Value::str("granted")),
        other => panic!("{other:?}"),
    }
    // A port pulled stays listed as disconnected.
    let mut r = granted(SOURCE);
    r.midi_agent(r#"{"op":"prefer","midi":{"outputs":[]}}"#)
        .unwrap();
    let outputs = &r.midi().ports().outputs;
    assert_eq!((outputs.len(), outputs[0].connected), (1, false));
    // `prefer midi.state denied` is what a request then finds.
    let mut r = boot_with(SOURCE);
    r.midi_agent(r#"{"op":"prefer","midi":{"state":"denied"}}"#)
        .unwrap();
    assert_eq!(r.midi().ports().access.word(), "denied");
}

#[test]
fn send_midi_queues_whole_messages_and_drops_the_rest_by_name() {
    let mut r = granted(SOURCE);
    r.take_midi();
    r.act("send", vec![]).unwrap();
    assert_eq!(
        sent(&r),
        [
            ("90 3c 64".into(), 500.0, None),
            ("80 3c 00".into(), 500.0, None)
        ]
    );
    let ops = r.take_midi();
    assert!(matches!(&ops[0], MidiOp::Send { id: 1, at, .. } if *at == 500.0));
    r.act("sendLate", vec![]).unwrap();
    r.act("sendBad", vec![]).unwrap();
    r.act("sendNowhere", vec![]).unwrap();
    let lines = said(&r);
    assert!(
        lines.contains(&"midi 3 to synth b0 07 64 at 0 (asked -10)".to_string()),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("midi dropped: byte 2 is c8")),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"midi dropped: nowhere is not a connected output (sendMIDI)".to_string())
    );
    assert_eq!(r.midi().waiting(0.0), 1, "the note-off is exempt");
}

#[test]
fn notes_play_on_the_beat_and_one_seek_equals_many() {
    let run = |steps: &[f64]| {
        let mut r = granted(SOURCE);
        r.act("start", vec![]).unwrap();
        for &to in steps {
            r.advance(to).unwrap();
        }
        (sent(&r), said(&r))
    };
    let one = run(&[4000.0]);
    let many = run(&(1..=40).map(|k| 100.0 * k as f64).collect::<Vec<_>>());
    assert_eq!(one, many);
    let ons: Vec<f64> = one
        .0
        .iter()
        .filter(|s| s.0 == "90 3c 64")
        .map(|s| s.1)
        .collect();
    assert_eq!(&ons[..5], [0.0, 500.0, 1000.0, 1500.0, 2000.0]);
    let offs: Vec<f64> = one
        .0
        .iter()
        .filter(|s| s.0 == "80 3c 00")
        .map(|s| s.1)
        .collect();
    assert_eq!(&offs[..2], [250.0, 750.0]);
}

#[test]
fn an_edit_takes_back_waiting_notes_and_keeps_a_handed_note_s_off() {
    let mut r = granted(SOURCE);
    r.act("start", vec![]).unwrap();
    r.advance(980.0).unwrap();
    // Beat 2's note at 1000 is handed (within 50 ms): the edit leaves its
    // note-off at 1250. At 240 the beat reached at 980 is 1.96: beat 2 at
    // 990 is within the hold, played already, so the next note is beat 3 at
    // 1240, which moves that note-off to it.
    r.act("faster", vec![]).unwrap();
    let s = sent(&r);
    assert!(s.contains(&("90 3c 64".into(), 1000.0, None)), "{s:?}");
    assert!(
        s.contains(&("80 3c 00".into(), 1250.0, Some("overlap"))),
        "{s:?}"
    );
    assert!(s.contains(&("80 3c 00".into(), 1240.0, None)), "{s:?}");
    assert!(s.contains(&("90 3c 64".into(), 1240.0, None)), "{s:?}");
    assert!(!s.iter().any(|e| e.1 == 990.0), "{s:?}");
    // An edit before the hold takes the note and its note-off back.
    let mut r = granted(SOURCE);
    r.act("start", vec![]).unwrap();
    r.advance(940.0).unwrap();
    r.act("faster", vec![]).unwrap();
    let s = sent(&r);
    assert!(
        s.contains(&("90 3c 64".into(), 1000.0, Some("edit"))),
        "{s:?}"
    );
    assert!(
        s.contains(&("80 3c 00".into(), 1250.0, Some("edit"))),
        "{s:?}"
    );
    assert!(said(&r).contains(&"midi retracted: 2 (edit)".to_string()));
}

#[test]
fn a_stop_moves_a_handed_note_s_off_to_the_stop() {
    let mut r = granted(SOURCE);
    r.act("longer", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(600.0).unwrap();
    r.act("halt", vec![]).unwrap();
    let s = sent(&r);
    // Beat 0's note-on was handed; its note-off at 1000 moves to 600.
    assert!(
        s.contains(&("80 3c 00".into(), 1000.0, Some("cancelled"))),
        "{s:?}"
    );
    assert!(s.contains(&("80 3c 00".into(), 600.0, None)), "{s:?}");
    assert_eq!(r.midi().waiting(600.0), 0);
}

#[test]
fn a_second_note_on_moves_the_first_note_s_off_before_it() {
    let mut r = granted(SOURCE);
    r.act("longer", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    // Each beat's note lasts two beats: the next one cuts it, off first.
    let s = sent(&r);
    let at500: Vec<&str> = s
        .iter()
        .filter(|e| e.1 == 500.0 && e.2.is_none())
        .map(|e| e.0.as_str())
        .collect();
    assert_eq!(at500, ["80 3c 00", "90 3c 64"], "{s:?}");
    assert!(said(&r).iter().any(|l| l == "midi retracted: 1 (overlap)"));
}

#[test]
fn a_negative_offset_plays_at_the_arming_and_skips_at_an_edit() {
    let mut r = granted(SOURCE);
    r.act("early", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    r.act("start", vec![]).unwrap();
    let s = sent(&r);
    assert_eq!(
        s[0],
        ("90 3c 64".into(), 1000.0, None),
        "clamped to the arming"
    );
    assert_eq!(s[2], ("90 3c 64".into(), 1400.0, None));
}

#[test]
fn clock_out_starts_at_beat_0_and_stops_after_its_last_handed_tick() {
    let mut r = granted(SOURCE);
    r.act("withClock", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    let s = sent(&r);
    assert_eq!(
        s[0],
        ("fa".into(), 0.0, None),
        "Start before the first tick"
    );
    // A second's ticks at 120: 48, 24 a beat.
    assert_eq!(r.midi().clock().1, 48);
    r.advance(510.0).unwrap();
    r.act("again", vec![]).unwrap();
    let lines = said(&r);
    let stop = lines
        .iter()
        .position(|l| l.ends_with("fc at 541.6666666666667") || l.contains(" fc at "));
    let start = lines.iter().rposition(|l| l.contains(" fa at 510"));
    assert!(
        stop.is_some() && start.is_some() && stop < start,
        "{lines:?}"
    );
    // The Stop is no earlier than the last tick handed (510 + 50 holds one at 541.67).
    let stop_at = r.midi().sent().find(|e| e.data == [0xFC]).unwrap().at;
    assert!(stop_at > 510.0 && stop_at <= 560.0, "{stop_at}");
}

#[test]
fn every_listener_hears_a_message_in_its_own_commit_and_controls_coalesce() {
    let mut r = granted(SOURCE);
    let m = |data: &[u8]| Incoming {
        input: "keys".into(),
        time_stamp: 0.0,
        data: data.to_vec(),
    };
    let (receipts, error) = r.midi_input(vec![
        m(&[0xB0, 7, 10]),
        m(&[0xB0, 7, 20]),
        m(&[0xF8]),
        m(&[0x90, 60, 30]),
        m(&[0xB0, 7, 40]),
        m(&[0xB0, 7, 50]),
    ]);
    assert!(error.is_none(), "{error:?}");
    // Three messages after coalescing, each heard by both elements.
    assert_eq!(receipts.len(), 6);
    assert_eq!((text(&r, "heard"), text(&r, "heard2")), (3.0, 3.0));
    assert_eq!(text(&r, "sum"), 20.0 + 30.0 + 50.0);
    assert_eq!(text(&r, "beatSeen"), -1.0, "stopped");
    assert!(said(&r).contains(&"midi coalesced: 2 (keys)".to_string()));
    // `pads` only the inner element hears.
    let (receipts, _) = r.midi_input(vec![Incoming {
        input: "pads".into(),
        time_stamp: 0.0,
        data: vec![0x90, 60, 1],
    }]);
    assert_eq!(receipts.len(), 1);
    r.take_midi();
    assert_eq!(r.midi().open(), ["keys", "pads"]);
    // While playing, a message's beat is the transport's at its stamp.
    r.act("start", vec![]).unwrap();
    r.midi_input(vec![Incoming {
        input: "keys".into(),
        time_stamp: 250.0,
        data: vec![0x90, 61, 1],
    }]);
    assert_eq!(text(&r, "beatSeen"), 0.5);
}
