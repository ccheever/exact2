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
  state from = 0
  state floodN = 0
  resource midi = exactMIDI() as shape MIDI
  resource many = flood(floodN) as shape number[]
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
  action countIn
    from = -1
  action laterOff
    off = 100
  action shorter
    length = 0.05
  action slower
    bpm = 30
  action sendPair
    sendMIDI("synth", [144, 61, 100], at=performanceNow() + 100)
    sendMIDI("synth", [128, 61, 0], at=performanceNow() + 500)
  action sendStart
    sendMIDI("synth", [250], at=performanceNow() + 10000)
  action reloadIt
    reload()
  action floodOn
    floodN = 1024
  action flood
    sendMIDI("synth", many, at=performanceNow() + 10000)
  action hear(e: MIDIMessageEvent)
    heard = heard + 1
    beatSeen = e.beat
    sum = sum + (match e.data.at(2) { case some(v) => v, case none => 0 })
  action hear2(e: MIDIMessageEvent)
    heard2 = heard2 + 1
  sequence s when playing key=take
    play([Track(beats=1, loops=0, offset=off, output="synth", steps=[note(0, 60, length)])], bpm=bpm, from=from, clockOut=clockOut)
  view
    column midimessage=hear midiinputs=["keys"]
      text `${midi.state} ${midi.outputs.length}`
      column midimessage=hear2 midiinputs=["keys", "pads"]
        text "b"
"#;

struct Granted;

impl DataSource for Granted {
    /// `flood(n)`: `n` note-ons' bytes.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        assert_eq!(source, "flood");
        let n = match args.first() {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let bytes = [144.0, 60.0, 100.0].map(Value::Number);
        Ok(Value::list(
            bytes.iter().cycle().take(3 * n).cloned().collect(),
        ))
    }

    fn grants(&self) -> &str {
        "device.midi purpose.midi"
    }
}

struct Ungranted;

impl DataSource for Ungranted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        assert_eq!(source, "flood");
        Ok(Value::list(Vec::new()))
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
    let (_, reply) = r
        .midi_agent(r#"{"op":"prefer","midi":{"outputs":[]}}"#)
        .unwrap();
    reply.unwrap();
    let outputs = &r.midi().ports().outputs;
    assert_eq!((outputs.len(), outputs[0].connected), (1, false));
    // `prefer midi.state denied` is what a request then finds.
    let mut r = boot_with(SOURCE);
    let (_, reply) = r
        .midi_agent(r#"{"op":"prefer","midi":{"state":"denied"}}"#)
        .unwrap();
    reply.unwrap();
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
    let start = lines.iter().rposition(|l| l.contains(" fa at "));
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

/// Review round 1, A1: a `reload` while a note sounds ends it: its
/// note-off is handed at once and nothing is left waiting.
#[test]
fn a_reload_ends_every_sounding_note() {
    let mut r = granted(SOURCE);
    r.act("longer", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(600.0).unwrap();
    r.act("reloadIt", vec![]).unwrap();
    let s = sent(&r);
    // Beat 1's note-on at 500 was handed; its note-off at 1500 moves to 600.
    assert!(s.contains(&("80 3c 00".into(), 600.0, None)), "{s:?}");
    assert_eq!(r.midi().waiting(600.0), 0, "{s:?}");
    assert!(r
        .midi()
        .sent()
        .all(|e| e.by.is_some() || e.at - 50.0 <= 600.0));
}

/// Review round 1, A2: a queue full of messages far ahead does not block a
/// note that is due, and the clock moves on.
#[test]
fn a_full_queue_never_blocks_a_due_note() {
    let mut r = granted(SOURCE);
    r.act("floodOn", vec![]).unwrap();
    r.act("flood", vec![]).unwrap();
    assert_eq!(r.midi().waiting(0.0), 1024);
    r.act("start", vec![]).unwrap();
    r.advance(2000.0).unwrap();
    // The record keeps the 1,024 waiting; the journal has every note.
    let ons: Vec<String> = said(&r)
        .into_iter()
        .filter(|l| l.contains("90 3c 64 at ") && !l.ends_with("at 10000"))
        .collect();
    assert!(ons.len() >= 4, "{ons:?}");
    assert!(
        ons[0].ends_with("90 3c 64 at 0") && ons[3].ends_with("90 3c 64 at 1500"),
        "{ons:?}"
    );
}

/// Review round 1, A3: a restart's Start reaches the device after the old
/// clock's Stop, in time order, not only in the journal's.
#[test]
fn a_restart_starts_after_its_stop() {
    let mut r = granted(SOURCE);
    r.act("withClock", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(510.0).unwrap();
    r.act("again", vec![]).unwrap();
    let live: Vec<(String, f64)> = sent(&r)
        .into_iter()
        .filter(|e| e.2.is_none())
        .map(|e| (e.0, e.1))
        .collect();
    let stop = live.iter().find(|e| e.0 == "fc").expect("a Stop").1;
    let start = live
        .iter()
        .filter(|e| e.0 == "fa")
        .map(|e| e.1)
        .next_back()
        .unwrap();
    assert!(start >= stop, "Start {start} before Stop {stop}: {live:?}");
    assert!(stop > 510.0);
}

/// Review round 1, A4: a Start an edit takes back before it is handed is
/// sent again.
#[test]
fn a_start_taken_back_is_sent_again() {
    let mut r = granted(SOURCE);
    r.act("withClock", vec![]).unwrap();
    r.act("countIn", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert!(sent(&r).contains(&("fa".into(), 500.0, None)));
    r.advance(100.0).unwrap();
    r.act("faster", vec![]).unwrap();
    let s = sent(&r);
    assert!(s.contains(&("fa".into(), 500.0, Some("edit"))), "{s:?}");
    // At 240 the beat reached at 100 is -0.8: beat 0 falls at 300.
    assert!(s.contains(&("fa".into(), 300.0, None)), "{s:?}");
}

/// Review round 2: a full queue does not hang a note a track's positive
/// offset keeps beyond the hold; its fill wakes when the note enters the
/// hold, never again where it stood.
#[test]
fn a_full_queue_and_a_positive_offset_still_move_on() {
    let mut r = granted(SOURCE);
    r.act("floodOn", vec![]).unwrap();
    r.act("flood", vec![]).unwrap();
    r.act("laterOff", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    assert!(
        r.transport().wake_due() > 0.0,
        "{}",
        r.transport().wake_due()
    );
    r.advance(2000.0).unwrap();
    let ons: Vec<String> = said(&r)
        .into_iter()
        .filter(|l| l.contains("90 3c 64 at ") && !l.ends_with("at 10000"))
        .collect();
    assert!(ons.len() >= 3, "{ons:?}");
    assert!(
        ons[0].ends_with("90 3c 64 at 100") && ons[2].ends_with("90 3c 64 at 1100"),
        "{ons:?}"
    );
}

/// Review round 2: a runner ended on a live clock ends at the host's time,
/// not its last commit's: a note-on the output handed since is not taken
/// back, and its note-off comes after it.
#[test]
fn ending_a_runner_takes_the_host_s_time() {
    let mut r = granted(SOURCE);
    r.act("sendPair", vec![]).unwrap();
    r.set_live_clock(Some(Box::new(|| 60.0)));
    r.end_sounds();
    let s = sent(&r);
    assert!(
        s.contains(&("90 3d 64".into(), 100.0, None)),
        "the handed note-on stays: {s:?}"
    );
    assert!(
        s.contains(&("80 3d 00".into(), 500.0, Some("cancelled"))),
        "{s:?}"
    );
    assert!(
        s.contains(&("80 3d 00".into(), 110.0, None)),
        "after its note-on: {s:?}"
    );
}

/// Review round 2: a Start the app sent itself, taken back at a stop, does
/// not stand for the clock's: the clock still sends Stop.
#[test]
fn an_app_s_own_start_does_not_suppress_the_clock_s_stop() {
    let mut r = granted(SOURCE);
    r.act("withClock", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.act("sendStart", vec![]).unwrap();
    r.advance(100.0).unwrap();
    r.act("halt", vec![]).unwrap();
    let s = sent(&r);
    assert!(s.iter().any(|e| e.0 == "fc" && e.2.is_none()), "{s:?}");
}

/// Review round 3: an edit while a full queue parks a note the fill never
/// issued still issues it: an output the fill did not reach is not taken
/// for one handed.
#[test]
fn an_edit_keeps_a_note_a_full_queue_parked() {
    let mut r = granted(SOURCE);
    r.act("floodOn", vec![]).unwrap();
    r.act("flood", vec![]).unwrap();
    r.act("laterOff", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(10.0).unwrap();
    r.act("faster", vec![]).unwrap();
    r.advance(2000.0).unwrap();
    let ons: Vec<String> = said(&r)
        .into_iter()
        .filter(|l| l.contains("90 3c 64 at ") && !l.ends_with("at 10000"))
        .collect();
    assert!(
        ons.first().is_some_and(|l| l.ends_with("90 3c 64 at 100")),
        "{ons:?}"
    );
}

/// The final confirmation of round 3: a parked note issued after a tempo
/// edit ends in the segment its end falls in, not the newest: at 120 BPM a
/// 0.05-beat note at offset 100 sounds from 100 to 125 though the tempo
/// moved to 30 at 40.
#[test]
fn a_parked_note_ends_in_the_segment_its_end_falls_in() {
    let mut r = granted(SOURCE);
    r.act("floodOn", vec![]).unwrap();
    r.act("flood", vec![]).unwrap();
    r.act("laterOff", vec![]).unwrap();
    r.act("shorter", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.advance(40.0).unwrap();
    r.act("slower", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    // The record keeps the waiting flood; the journal keeps the note.
    let notes: Vec<String> = said(&r)
        .into_iter()
        .filter(|l| l.contains(" 3c ") && !l.ends_with("at 10000"))
        .collect();
    assert!(
        notes.iter().any(|l| l.ends_with("90 3c 64 at 100")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|l| l.ends_with("80 3c 00 at 125")),
        "{notes:?}"
    );
}

/// [`SOURCE`] reading `exactMIDI().sent`, with sends at chosen times.
fn sent_source() -> String {
    SOURCE
        .replace(
            "shape MIDI\n  state: string\n",
            "shape Message\n  output: string\n  data: number[]\n  timeStamp: number\n\nshape MIDI\n  state: string\n  sent: Message[]\n",
        )
        .replace("`${midi.state} ${midi.outputs.length}`", "`${midi.state} ${midi.sent.length}`")
        .replace(
            "  action ask\n",
            "  action sendLater\n    sendMIDI(\"synth\", [144, 61, 100], at=performanceNow() + 1000)\n  action sendSooner\n    sendMIDI(\"synth\", [144, 62, 100], at=performanceNow() + 10)\n  action ownTick\n    sendMIDI(\"synth\", [248], at=performanceNow() + 100)\n  action ask\n",
        )
}

/// `exactMIDI().sent` as each message's bytes and time.
fn listed(r: &Runner<Granted>) -> Vec<(Vec<f64>, f64)> {
    let Some(Value::Record(f)) = r.resource("midi") else {
        panic!()
    };
    let Value::List(items) = &f[1] else {
        panic!("{:?}", f[1])
    };
    items
        .iter()
        .map(|m| match m {
            Value::Record(m) => {
                assert_eq!(m[0], Value::str("synth"));
                let Value::List(d) = &m[1] else { panic!() };
                let d = d.iter().map(|b| match b {
                    Value::Number(n) => *n,
                    _ => panic!(),
                });
                let Value::Number(at) = m[2] else { panic!() };
                (d.collect(), at)
            }
            other => panic!("{other:?}"),
        })
        .collect()
}

/// `exactMIDI().sent` (euxy's activity log): what went out by now, oldest
/// first, as `output`, `data` and `timeStamp`, never a clock tick or a
/// message taken back. Each message's time answers its reader again, with
/// no app action; a plan that declares no `sent` is answered no more often.
#[test]
fn sent_lists_what_went_out_and_is_answered_as_each_goes() {
    let mut r = granted(&sent_source());
    r.act("withClock", vec![]).unwrap();
    r.act("start", vec![]).unwrap();
    r.land_then();
    // Start and the first note-on went at 0; the note-off at 250 has not.
    assert_eq!(
        listed(&r),
        [(vec![250.0], 0.0), (vec![144.0, 60.0, 100.0], 0.0)]
    );
    let landed = r.advance(260.0).unwrap();
    assert_eq!(landed.len(), 1, "the note-off's time is one answer");
    assert_eq!(listed(&r).last(), Some(&(vec![128.0, 60.0, 0.0], 250.0)));
    // A stop takes back what waits: the next note-on never goes, and Stop
    // goes after the last clock tick handed.
    r.act("halt", vec![]).unwrap();
    r.advance(2000.0).unwrap();
    let went = listed(&r);
    assert_eq!(went.len(), 4, "{went:?}");
    assert!(went[3].0 == [252.0] && went[3].1 > 260.0, "{went:?}");
    assert!(!went.iter().any(|(_, at)| *at == 500.0), "{went:?}");
    // Without `sent`, nothing is answered as messages go.
    let mut r = granted(SOURCE);
    r.act("start", vec![]).unwrap();
    r.land_then();
    assert!(r.advance(260.0).unwrap().is_empty());
}

/// `sent` is in the order messages went (time, then id), not the order
/// they were queued; it outlives the queue's record; and Timing Clock is
/// never listed nor answered for, whoever sent it (review round 1).
#[test]
fn sent_is_in_time_order_outlives_the_queue_and_leaves_out_clock() {
    let mut r = granted(&sent_source());
    r.act("sendLater", vec![]).unwrap();
    r.act("sendSooner", vec![]).unwrap();
    r.advance(1000.0).unwrap();
    assert_eq!(
        listed(&r),
        [
            (vec![144.0, 62.0, 100.0], 10.0),
            (vec![144.0, 61.0, 100.0], 1000.0)
        ]
    );
    // An app's own Timing Clock is neither listed nor an answer.
    r.act("ownTick", vec![]).unwrap();
    assert!(r.advance(1200.0).unwrap().is_empty());
    assert_eq!(listed(&r).len(), 2);
    // 1,024 messages queued ten seconds on push the sent ones out of the
    // queue's record; `sent` keeps them.
    r.act("floodOn", vec![]).unwrap();
    r.act("flood", vec![]).unwrap();
    assert!(
        r.midi().sent().all(|s| s.at > 1200.0),
        "the record let them go"
    );
    assert_eq!(listed(&r).len(), 2);
    // Of what went, the last 128 by time.
    r.advance(12_000.0).unwrap();
    let went = listed(&r);
    assert_eq!(went.len(), 128);
    assert!(
        went.iter().all(|(_, at)| *at == 11_200.0),
        "{:?}",
        &went[..2]
    );
}

/// LLP 1125 D6 (2026-10-20, second amendment): `""` is no output. What a
/// sequence plays to it and what `sendMIDI("")` sends is queued, taken back
/// and listed in `sent` as any message is, with `output: ""`, whether or
/// not MIDI access was asked for, and no host is handed it: euxy's activity
/// log with no device. A named output that is not connected still drops.
#[test]
fn no_output_is_listed_and_handed_to_no_host() {
    let source = sent_source()
        .replace("output=\"synth\"", "output=\"\"")
        .replace(
            "  action ask\n",
            "  action sendNone\n    sendMIDI(\"\", [176, 123, 0])\n  action ask\n",
        );
    let mut r = boot_with(&source);
    assert_eq!(r.midi().ports().access.word(), "prompt");
    r.act("start", vec![]).unwrap();
    r.land_then();
    r.advance(600.0).unwrap();
    r.act("halt", vec![]).unwrap();
    r.act("sendNone", vec![]).unwrap();
    r.advance(700.0).unwrap();
    let Some(Value::Record(f)) = r.resource("midi") else {
        panic!()
    };
    let Value::List(items) = &f[1] else {
        panic!("{:?}", f[1])
    };
    let went: Vec<(String, String, f64)> = items
        .iter()
        .map(|m| match m {
            Value::Record(m) => {
                let Value::List(d) = &m[1] else { panic!() };
                let d: Vec<u8> = d
                    .iter()
                    .map(|b| match b {
                        Value::Number(n) => *n as u8,
                        _ => panic!(),
                    })
                    .collect();
                let Value::Number(at) = m[2] else { panic!() };
                (m[0].text().to_string(), crate::midi::hex(&d), at)
            }
            other => panic!("{other:?}"),
        })
        .collect();
    let at = |hex: &str, at: f64| ("".to_string(), hex.to_string(), at);
    // Two beats' notes, the second's note-off moved to the stop, then the
    // app's control change; the note-on at 1000 never went.
    assert_eq!(
        went,
        [
            at("90 3c 64", 0.0),
            at("80 3c 00", 250.0),
            at("90 3c 64", 500.0),
            at("80 3c 00", 600.0),
            at("b0 7b 00", 600.0)
        ]
    );
    assert!(
        r.take_midi()
            .iter()
            .all(|op| !matches!(op, MidiOp::Send { .. } | MidiOp::Retract { .. })),
        "no host is handed or asked to take back a message to no output"
    );
    let lines = said(&r);
    assert!(
        lines.contains(&"midi 1 to no output 90 3c 64 at 0".to_string()),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("notes not sent")),
        "{lines:?}"
    );
}
