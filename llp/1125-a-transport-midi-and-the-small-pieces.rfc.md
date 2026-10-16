# LLP 1125: A transport, MIDI, and the small pieces apps hand-roll

**Type:** RFC
**Status:** Accepted r2, 2026-10-16, by the orchestrator under Charlie's instruction of that day ("once happy with it, go ahead and implement"). r1 (`404863b35`) was reviewed blind by three families, each asked for direction and fundamentals only: GPT-6 Astra (max), Grok 4.7 (xhigh) and Muse Wonder Full 1.0 (xhigh), all RIGHT DIRECTION WITH CHANGES (`llp/reviews/1125-a-transport-midi-and-the-small-pieces.{astra,grok,muse}.md`). r2 takes their findings as §R records.
**Systems:** Contract compiler (a `sequence` declaration; `sendMIDI`, `requestMIDIAccess`; `midimessage`, `midiinputs`; `wakelock`; `playbackRate=` and `pan=` on hits; `share`'s `files=`), Plan (`sequences`; use bits), Runner (new `runner/src/transport.rs` beside `sound.rs`; the voice bound; `exactTransport()` and `exactMIDI()`; a MIDI queue and record; clock ticks as an input), JS target (`host/web-js/sounds.js` and a transport twin), the sound arm (`host/apple/soundarm/`, `host/web/sound-glue.js`: rate, pan, capacity), a new MIDI arm (Apple `ExactMIDI`, web `midi-glue.js`), the wake lock on each host, Share (`runner/src/share.rs`, `Share.swift`), the agent (`state.transport`, `state.midi`, `state.wakelock`; test steps), `runner/src/device.rs` (one grant row)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-16
**Revised:** 2026-10-16 (r2: one blind round, §R)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, from 2026-10-16, in §7's stages
**Amends:** LLP 1096 D5 (the voice bound counts voices sounding at a voice's start, D3 here) and §7 (`playbackRate`, `pan` leave the deferred list, D10); LLP 1069.003 D7 (files in a share, D12); LLP 1069 §5 #10 (keep-awake, D11); LLP 1069.007 (one stream, MIDI input, gets its rule, §4); `docs/recipes/native-views.md:56–58` (timed output goes through a sequence); `rules/DEFERRED.md` (§8)
**Related:** LLP 1096 (sounds: the voice table, groups, runner time to the speaker, one session owner); LLP 1098 (events on an element by the web API's names); LLP 1092 D7–D10 (gated tasks, arming from the commit, the seek rule); LLP 1073 (`every(frame)`; no catch-up); LLP 1077 D14 (`haptic`); LLP 1069.000 (reserved sources); LLP 1069.008 (grants to permissions); LLP 1069.009 (replayable inputs); LLP 1047 and 1047.001 (linked and loaded tiers); LLP 1067.000 (one native module per app); LLP 1124 (host extensions, D1). The consumer: euxy, a Euclidean MIDI sequencer (`~/projects/euxy-src/euxy`), and its exact2 port (`~/projects/euxy-ports/exact2/euxy`: `FRICTION.md`, `NOTES.md`, `modules/apple/`). External, read 2026-10-16: Web MIDI API (W3C WD; Chromium's `midi_output.idl` leaves `clear()` unimplemented), Screen Wake Lock API, Web Share API Level 2 (`files`), Web Audio API 1.1 (`playbackRate`, `StereoPannerNode`), CoreMIDI (`MIDIClientCreateWithBlock`, `MIDISendEventList`, `MIDIFlushOutput`, `kMIDIPropertyUniqueID`), `UIApplication.isIdleTimerDisabled`, `IOPMAssertionCreateWithName`, Android `View.setKeepScreenOn`.

## Summary

Each of the three euxy ports (SwiftUI, UIKit, exact2) wrote the same things
by hand. The exact2 port carries 1,607 lines of Swift in `modules/apple/`,
most of them a lookahead scheduler, CoreMIDI, a sampler and a share card.
This RFC makes those pieces exact2's own, shaped so that a drum machine, a
metronome, a rhythm game, an interval timer, a MIDI controller surface or a
share card in any app uses them unchanged:

| | Piece | What the author writes | Stage |
|---|---|---|---|
| D2–D5 | **A sequence** | `sequence groove when playing` / `play(tracks, bpm=…)`: what should play, as a value | 2 |
| D6–D9 | **MIDI** | `exactMIDI()`, `sendMIDI(output, data, at=)`, `midimessage=`, MIDI steps, clock out and in | 3, 4 |
| D10 | **Sampler controls** | `playbackRate=` and `pan=` on a hit | 1 |
| D11 | **Keep-awake** | `wakelock="screen"` on an element | 1 |
| D12 | **Files and pictures in a share** | `share(files=["app:/…", "#card"])` | 5 |
| — | Haptics | already there (LLP 1077 D14); a step can carry one (D2) | 2 |

**A sequence is the main proposal.** Today an app keeps time with LLP 1096's
window clock: a 25 ms timer whose action schedules the next 100 ms of hits.
Each tick is an app commit, the pattern logic is the app's, a track of its own
length needs the app's arithmetic, and nothing follows an external clock. So
euxy's port ran its own `DispatchSourceTimer` (FRICTION.md:124–126). Under
this RFC the app **declares what should play**, and the runner expands it into
the voice table and a MIDI queue a second ahead. The runner remains the single
owner of what sounds, so choke groups, the voice bound and `stopSounds` work
the same for a sequence as for a tap. An edit ends what has not started and
re-expands from the edit's commit. No app action runs per step, and the record
is a pure function of the sequence's history and the clock. The driver and
tests therefore see every hit, exactly, with no audio.

## 1. Evidence

**What euxy is.** A Euclidean MIDI sequencer for the Teenage Engineering OP–XY.
Each pattern is a set of lanes. Each lane is a Euclidean rhythm with its own
length and resolution, playing one note on one MIDI channel, so lanes of
different lengths run against each other (polymeter). It sends clock and
notes over CoreMIDI, or follows the device's clock with a count-in. With no
device it plays built-in kits. It shares a pattern as a link and as a card
with a QR code.

**What the exact2 port wrote** (`modules/apple/`):

| File | Lines | What it does | Replaced by |
|---|---|---|---|
| `Engine.swift` | 555 | Two-clocks scheduler (24 PPQN, 100 ms lookahead, 25 ms `DispatchSourceTimer`), clock out, following a clock with a count-in, tempo from a 48-tick window, panic, beat haptics, a 3 s device poll, keep-awake | D2–D5, D7–D9, D11 |
| `MidiManager.swift` | 134 | CoreMIDI on its own run loop: ports, timestamped send, receive stamped in the read callback | D6–D8 |
| `Sampler.swift` | 192 | 24 `AVAudioPlayerNode`s round robin, `scheduleBuffer(at:)`, three kits preloaded | LLP 1096 and D2 |
| `ShareCard.swift` | 179 | `CIQRCodeGenerator`, `UIGraphicsImageRenderer`, a hand-presented `UIActivityViewController` | D12; QR stays app code (§6) |
| `Euxy.swift` | 547 | The `<euxy-engine>` tag: every lane and device list encoded into strings, because props are strings (FRICTION §6–7) | mostly D2: a pattern is a typed value |

**The porter's notes** (`FRICTION.md`):

- "The `native` time guidance (`context.frames`/`after`, never a
  DispatchSource) does not fit an audio/MIDI scheduler that must keep real time
  while the UI is idle" (§9).
- "`share(...)` takes text/URLs" (§9).

**What the original needs that is easy to miss:**

- **Polymeter.** Each lane wraps at its own length (`euclid.ts`
  `laneStepAt`). One loop over the least common multiple of the lane lengths
  can be thousands of steps long.
- **Following a clock.** On Start (0xFA) the engine resets and swallows a
  count-in of `countInBeats × 24` ticks. Each Clock (0xF8) fires its tick at
  once. Tempo comes from a 48-tick window of timestamps taken in the native
  read callback. An average taken in JavaScript read 120 BPM as about 226,
  because stalls bunched the ticks (`engine.ts:85–100`).
- **Devices.** CoreMIDI's setup notifications arrive only on the run loop that
  was current when the client was made. Its first call can block about 600 ms
  on a cold MIDIServer (`MidiModule.swift:5–55`). Endpoint refs change on
  replug, and euxy polls every 3 s because notifications were missed on
  hardware (`runtime.ts:162–182`).
- **Backgrounding.** euxy deliberately stops when the app leaves the
  foreground. It sends Stop, panics and rewinds, because a device left
  following a dead clock wedges (`engine.ts:184–235`).
- **A latency offset per output**, −120 to +120 ms, set by the user
  (`Engine.swift:303`).
- **Keep-awake** is held while playing *or* while waiting for an external
  clock (`engine.ts:174–195`).

**What exact2 already has:**

- **Choke groups and per-voice gain** are LLP 1096 D2 and D4. euxy's own
  sampler had neither choke groups nor pitch.
- **Haptics** are LLP 1077 D14 on every host that has them.
- **Timed one-shots** are LLP 1096, mapped to the speaker within a few
  microseconds on Apple (1096 §6, As built).
- **Apple's presenter has one `UIView` per kernel node**
  (`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1`), so an element's
  picture is the host's own drawing (D12).

## 2. Who else uses them

- **A sequence:** a metronome; a drum machine or step sequencer; a rhythm
  game whose notes land on the beat however busy the frame is; an
  interval-training timer whose beeps and haptics land on the second (finite
  tracks and `ended=`, D2); a practice app that plays along.
- **MIDI:** apps a controller steers (a lighting desk, a slide clicker, a
  soundboard), apps that drive hardware (euxy, a MIDI monitor), and recording
  what a player played. A low-latency instrument is not claimed: an input goes
  through a commit before it sounds (D8).
- **Sampler controls:** a chromatic instrument from one sample; a game varying
  a footstep's pitch; stereo placement.
- **Keep-awake:** a recipe screen, a presentation timer, turn-by-turn steps, a
  boarding pass, a workout. LLP 1069 §5 #10 listed it with no consumer yet.
- **Files and pictures in a share:** a score card, a stats summary, a quote, a
  backup (LLP 1069.003 D7's own consumer).

## 3. Design

### D1 — Exact2's vocabulary, linked or loaded by use

Each piece is exact2's own, as `sound` and `share` are. None is a cargo
feature, and an app that uses none of them carries none of their code (LLP
1047):

| Piece | Runner | Apple | Web | Linux, Windows |
|---|---|---|---|---|
| Sequence | `transport.rs`, expanding into `sound.rs` | the sound arm, unchanged but for capacity | `sound-glue.js`, unchanged but for capacity | the record |
| MIDI | a MIDI queue and record | `ExactMIDI`, linking CoreMIDI only when used (LLP 1047.001 D4) | `midi-glue.js`, after paint, for a plan that uses MIDI | the record |
| Sampler controls | the voice's two fields | the C mixer | the glue | the record |
| Keep-awake | a node row | `isIdleTimerDisabled`; macOS `IOPMAssertionCreateWithName` | `navigator.wakeLock` | journaled |
| Share files | `share.rs` | the share sheet with files | `navigator.share({files})` | refused, `unavailable` |

**Why not host extensions (LLP 1124) or each app's own module.** An extension
fits a device value with no timing. These pieces differ in three ways:

- **They share one clock.** A step plays a sample, sends a note and fires a
  haptic at one instant, mapped from runner time (LLP 1096 D6).
- **The agent must see them.** The driver plays nothing and reads the record.
- **MIDI input is a stream of events that must each arrive.** LLP 1124 §5
  refuses value streams.

### D2 — `sequence`: what should play, declared

`sequence` is a root declaration with a task's gate grammar (LLP 1092 D7). It
holds one `play(...)`:

```
sound "assets/kits/808/kick.wav"
sound "assets/kits/808/hat-closed.wav"

shape Step
  beat: number        -- from the track's start, in quarter notes
  src: string         -- a declared sound, or "" for none
  gain: number
  group: string
  note: number        -- a MIDI note 0–127, or -1 for none (D7)
  velocity: number    -- 1–127
  channel: number     -- 0–15
  duration: number    -- beats until the note-off (`length` is a list's, As built)
  haptic: string      -- a haptic kind (LLP 1077 D14), or ""

shape Track
  beats: number       -- the track loops at this many beats
  loops: number       -- times it plays; 0 is forever
  offset: number      -- milliseconds added to its times: an output's latency
  output: string      -- a MIDI output's id for its notes, or ""
  steps: Step[]

component Euxy
  state playing = false
  state bpm = 120
  state take = 0
  sequence groove when playing key=take
    play(tracks(lanes, kit, output), bpm=bpm, from=-4, countIn=clicks(4), ended=finished)
```

- **The gate.** The sequence arms from the commit that made the gate true
  (LLP 1092 D8), and beat `from` (default 0) falls at that commit's time. A
  changed `key=` restarts it from `from`. The gate turning false stops it
  (D4). The gate and key follow a task's rules, including that neither may
  read `performanceNow()`.
- **The arguments are re-read after every commit that stands**, and compared
  by value. An equal value changes nothing; a different one is an edit (D3).
  A refused commit changes nothing that sounds.
- **A track loops at its own length**, `loops` times or forever. That gives
  polymeter with no common loop. A step outside `[0, beats)` is dropped and
  journaled. Steps at one beat play in list order.
- **`from=` below 0 is a count-in.** `countIn=` is a list of steps played
  once over `[from, 0)`. Tracks start at beat 0.
- **`ended=`** names an action the runner runs when the last finite track
  finishes, at that exact time, as a timer runs (`performanceNow()` is the
  end). With a track that loops forever, it never runs. This is how an
  interval timer or a one-shot chart learns it is over without polling, and
  the end lands on the runner's clock, not on a late commit.
- **Beats, not ticks.** A sixteenth is `0.25`, a triplet eighth `1/3`. MIDI's
  24 PPQN appears only in clock out and in (D7, D9).
- **A step can carry a sample, a MIDI note and a haptic.** Shapes are flat, so
  "none" is `""` or `-1`, as `group` is today. A step's sample may also carry
  D10's `playbackRate` and `pan`.
- **One sequence plays at a time.** A second one arming while one plays is
  journaled and does not play.

### D3 — The runner expands it into the voice table

The runner keeps a **transport record** (`runner/src/transport.rs`): its
segments, each `(at, beat, bpm, tracks)`, and its state. Beat `b` falls at
`at + (b − beat) × 60000 / bpm` in the segment that holds it.

**Expansion.** The runner issues the sequence's steps into the existing voice
table (`runner/src/sound.rs`) as voices, into a MIDI queue (D7) and as timed
haptics, up to **1000 ms ahead** of its clock. It refills when less than
500 ms remain, on a wake of its own: `timer_due_ms` reports it, but it runs no
app action, makes no commit and renders nothing. `clock settle` does not wait
for it. A host whose main thread stalls for up to 500 ms therefore loses no
step. The lookahead is one constant on every host and under the driver, so the
record is the same everywhere.

**One owner.** The sound arm and the web glue receive `Play` and `End` as they
do today. No host expands a pattern. Choke groups, the voice bound and
`stopSounds` decide sequence voices and tapped voices by one rule, in one
place per runtime: the runner and its JS-target twin, `sounds.js`.

**Edits at the commit.** An edit at commit time `t` ends every voice the
sequence issued that has not started by `t`. Each ends as `by: edit`, never
sounds, and leaves the voice bound. The new value is then expanded from `t`.
Voices already sounding play on. A new `bpm` starts a new segment at `t`, at
the beat reached then, so tempo moves without a jump in phase. LLP 1096's
`End` already retracts a future voice on both outputs. On Apple the mixer
moves its end; on the web it is `stop(when)` or a held voice that never
starts.

**The bound.** LLP 1096 D5 bounded the voices that sound or wait at a
commit's time. A second of lookahead makes "wait" the wrong thing to count,
so the bound becomes: **at most 32 voices sounding at a voice's start**. It is
the same on every host and does not depend on the lookahead. A sequence that
asks for more polyphony has its extra voices dropped and journaled, as today.
Separately, at most 256 voices may be issued and not yet started. When that
cap is reached, expansion stops early and wakes sooner; nothing is dropped.
The Apple mixer's slots and ring grow to match (`sound_render.h`).

**`exactTransport()`** is a reserved source:

```
resource transport = exactTransport() as shape Transport
-- { state: "stopped" | "waiting" | "countin" | "playing",
--   reason: string, bpm: number, anchorAt: number, anchorBeat: number,
--   clock: "internal" | "midi" }
```

A playhead is drawn from it in a frame task gated on visibility (LLP 1073),
with `beat = anchorBeat + (performanceNow() − anchorAt) × bpm / 60000`. A
frame commits only while the playhead is shown, and sound never waits on
those commits.

**Haptic steps** play on the main thread at their time. UIKit's feedback
generators cannot be scheduled ahead, so a blocked main thread makes them
late. The journal counts late haptics.

**The seek rule** holds for an internal clock (LLP 1092 D10): the record is a
function of the segments and the clock, so one `advance(60_000)` and sixty
`advance(1_000)`s give the same voices. Clock in (D9) has its own record.

### D4 — Stopping, the session, and the background

- **The gate turning false stops it at that commit's time.** Unstarted
  sequence voices end as `cancelled`, and sounding ones as `stop`, as
  `stopSounds()` ends them (LLP 1096 D4). Queued MIDI not yet handed to the
  output is discarded. Every note the sequence turned on gets its note-off,
  stamped no earlier than its note-on, so a note already handed out still ends
  (D7). Clock out sends Stop. No CC 123: other notes on a shared channel are
  not the sequence's to silence.
- **`stopSounds()` ends the sequence's voices too**, but not the sequence.
  Its next steps play.
- **Leaving the foreground stops it** by default, as euxy does. When the host
  is about to suspend the app, the runner stops the sequence as if its gate
  had turned false, and `exactTransport()` reports `stopped` with reason
  `background`. The app sees that and sets its own state. A device following
  the clock gets Stop rather than a dead clock.
- **A `playback` session keeps playing.** With `audio_session: "playback"`
  and `audio` in `host.ios.backgroundModes` (LLP 1096 D8), iOS keeps the
  process running, the main run loop keeps the runner's wake firing, and the
  sequence plays on with the screen locked. That is the metronome case. A
  browser page is not suspended when hidden, so on the web it plays on.
  Chromium does not throttle an audible page's timers.

### D5 — The driver

Nothing plays and nothing is sent. The runner expands as its clock advances,
so each sample step is a voice in `state.sounds` (LLP 1096 D10) and
`expect sound has "assets/kits/808/kick.wav" at 500` works unchanged. Each
MIDI step is a line in `state.midi.sent`, and `state.transport` is the record.
The record keeps its last 1,024 entries.

### D6 — `exactMIDI()`: the ports, by Web MIDI's names

```
resource midi = exactMIDI() as shape MIDI
-- { state: "prompt" | "granted" | "denied" | "unavailable",
--   inputs: MIDIPort[], outputs: MIDIPort[] }
-- MIDIPort: { id, name, manufacturer, state: "connected" | "disconnected" }
```

- **Names are Web MIDI's** (`MIDIAccess.inputs`, `MIDIPort.id` and so on).
- **`id` is stable across replug**: on Apple, `kMIDIPropertyUniqueID`, not the
  endpoint ref.
- **It changes by itself** (LLP 1016.002). On Apple the arm makes its client on
  its own thread with its own run loop, so notifications arrive and the cold
  XPC never blocks the main thread. It re-enumerates on foreground, and on a
  3 s watchdog while a port is in use, because euxy saw notifications missed on
  hardware. The web listens to `onstatechange`.
- **Permission.** `requestMIDIAccess()` is a host command. On the web it asks;
  Apple asks for nothing. The grant line is `device.midi <strings-key>` (LLP
  1069.008 D1), which maps to `Permissions-Policy: midi`. SysEx is out.
- **Where there is none**, `state` is `unavailable`. That includes WebKit, and
  so every iOS browser.

### D7 — Sending and clock out

- **`sendMIDI(output, data, at=)`** is a host command. `data` is a `number[]`
  of MIDI 1.0 bytes, as `MIDIOutput.send(data, timestamp)` takes. `at=` is
  runner time and defaults to the commit's. A message that is not well-formed
  MIDI 1.0 is dropped and journaled; SysEx is not sent.
- **The runner's MIDI queue** holds every timed message: `sendMIDI`'s and a
  sequence's notes, each with an id. Hosts take them as `take_midi()` ops, as
  sounds are taken.
- **Hosts hand a message to the output at most 50 ms before it is due**, and
  hold it until then. CoreMIDI could take it a second early and flush, but
  `MIDIFlushOutput` empties a whole destination. Chromium has no
  `MIDIOutput.clear()`. Holding is the one rule both can keep. A retraction
  (an edit, a stop) removes held messages. A message already handed out plays,
  and its note-off follows it. On Apple the holding runs on the arm's thread,
  so main-thread stalls do not delay it. On the web it runs on the page's
  timer, so a stall of more than 50 ms makes those notes late; that limit is
  declared.
- **Track `offset`** shifts a track's times, so a sample and a hardware synth
  can be lined up by ear, as euxy's latency slider does.
- **`clockOut=<output id>`** on `play` sends Timing Clock (0xF8) 24 times per
  beat through the same queue. It sends Start (0xFA) at beat 0 and Stop (0xFC)
  at the stop. Continue with a Song Position Pointer is a later addition.

### D8 — Receiving: `midimessage`

`midimessage` is an event named as `MIDIInput` fires it. Every mounted element
that declares it hears it, as `beforeunload` is heard
(`docs/contract-grammar.md:1164`):

```
column midimessage=heard midiinputs=[inputId]
action heard(e: MIDIMessageEvent)
  -- e.input: string, e.data: number[], e.timeStamp: number (runner ms), e.beat: number
```

- **`midiinputs=`** lists the inputs to open. An app that lists none hears none.
- **Notes and transport messages are never dropped or reordered.** These are
  note on and off, Start, Continue, Stop and Song Position.
- **Continuous messages coalesce.** Control Change, pitch bend, channel
  pressure and polyphonic aftertouch keep only their latest value between
  commits, per input, channel and controller or note. A fader sweep is then one
  commit per frame, not hundreds.
- **Clock and Active Sensing are not dispatched.** They feed clock in (D9).
- **One dispatch per message**, in arrival order. Each mounted handler hears it
  in that one commit.
- **`timeStamp`** is the arm's (CoreMIDI's read callback, or the web event's),
  moved to runner time. **`e.beat`** is the transport's beat at that time, or
  `-1` when stopped. A recorder quantises from `e.beat` alone.

### D9 — Clock in

`clock=<input id>` on `play` follows that input instead of `bpm=`:

- **The ticks are an input.** The arm batches Clock, Start, Continue, Stop and
  Song Position messages with their timestamps, at most every 50 ms. It hands
  them to the runner as one call, which LLP 1069.009's tape records. The
  record is a function of that tick history, so a replay reproduces jitter,
  missing ticks and stops.
- **The runner estimates tempo** from a 48-tick window of those timestamps,
  euxy's estimator moved where it can be replayed. The anchor is re-taken
  each batch.
- **Steps are issued from the predicted clock.** Under clock in, the lookahead
  is the batch interval plus 50 ms, and a voice whose predicted time moves by
  more than 2 ms on a new batch is retracted and re-issued.
- **Start** puts beat `from` at its timestamp (the count-in). `state` is
  `waiting` until then. **Stop** stops the sequence as D4 does, with reason
  `clock`. The app hears Stop as a `midimessage` and decides whether the take
  ended.
- **The driver** sends ticks with `tap midi "<input>" clock <bpm> <ms>`. That
  call is the same runner input, so a drive and its replay agree.

### D10 — Sampler controls

- **`playbackRate=`** (Web Audio's `AudioBufferSourceNode.playbackRate`),
  0.25–4, default 1. Pitch moves with it, as on tape. The C mixer reads at a
  fractional position with linear interpolation. A voice lasts `duration /
  rate`, so groups, the bound and `state.sounds` stay exact.
- **`pan=`** (Web Audio's `StereoPannerNode.pan`), −1 to 1, default 0,
  equal-power as the spec defines it.
- **Both are optional on a hit.** They come after `src`, `at`, `gain` and
  `group` in `playSounds`'s shape and in a `Step`. A shape without them plays
  at rate 1, centred. A literal outside range is refused; a computed one is
  clamped, as `gain` is.
- **Decoding stays eager** (LLP 1096 D7, D8). On-demand decoding needs a
  readiness the app can wait on before its downbeat, and a buffer handoff the
  render thread can take mid-playback. It is deferred to a consumer whose kits
  are too large to decode at the first voice.

### D11 — Keep-awake: `wakelock="screen"`

```
column wakelock=(playing or waitingForClock ? "screen" : "none")
```

While any element that is mounted and displayed says `wakelock="screen"`, the
display does not dim or lock for idleness. Displayed means no ancestor is
`display: none`, so a screen kept mounted in a hidden tab panel holds nothing.
The value is the Screen Wake Lock API's one type. An attribute fits better
than commands, for two reasons:

- **It cannot leak.** A screen that unmounts or hides releases its lock.
- **It is the platform's own shape.** Android's is a view attribute,
  `View.setKeepScreenOn`.

Per host:

- **iOS:** `isIdleTimerDisabled` while any element holds it.
- **macOS:** an `IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep)`
  assertion, held while any element holds it.
- **Web:** `navigator.wakeLock.request("screen")`. The glue requests it again
  on `visibilitychange`, because a hidden page loses it.
- **Linux:** journaled as unavailable.
- **The driver:** `state.wakelock` is `{held, by}`.

### D12 — Files and an element's picture in a share

LLP 1069.003 D7's `files=` comes in: `app:/` paths behind `fs.read`, each
turned into the host's own type. An entry `"#id"` names an element: the host
shares a PNG of it as it is drawn now.

```
action shareCard
  share(title=pattern.name, url=link, files=["#card"])
```

- **On Apple and Android it is the host's own drawing**: the node's view
  rendered at the display's scale. Apple's presenter has one view per node (§1).
  The PNG lives in the host's temporary space until the sheet closes. Nothing is
  written that the app can read, so a picture needs no grant.
- **On the web a picture is refused by name** (`share: refused: a picture of
  an element is not available on the web`), and `exactPage().canSharePictures`
  is false. A serialised SVG `foreignObject` carries no stylesheet, fonts,
  images or canvases, so it would not be the element as drawn. A web picture
  waits for a capture that is, such as the HTML-in-Canvas proposal.
- **Paths on the web** go through `navigator.canShare({files})`;
  `exactPage().canShareFiles` says whether they can.

## 4. The agent and tests

| Piece | `state` | Drive | Test step |
|---|---|---|---|
| Sequence | `state.transport`; its voices in `state.sounds` | `clock +N` | `expect sound … at N`; `expect midi sent "<id>" 90 3c 64 at N` |
| MIDI ports | `state.midi.inputs`, `outputs` | `prefer midi.outputs=[…]` | — |
| MIDI in | `state.midi.received` | `tap midi "<input>" 90 3c 64` (LLP 1098's `tap … mediasession` addressing) | the same |
| Clock in | `state.transport` | `tap midi "<input>" clock 120 2000` | the same |
| Keep-awake | `state.wakelock` | — | `expect wakelock held` |
| Share files | a held `share` ticket with `files` | `tap @N shared` | as LLP 1069.003 |

**MIDI input against LLP 1069.007.** That LLP excludes streams "of events that
must each arrive". Its rule for this one is the following. A message is a
`dispatch` (LLP 1069.009 D1), and a clock batch is one runner call.
`tap midi` makes those calls with the arm absent, and a tape replays them. No
eleventh operation is added.

## 5. Effect on each implementation

- **Contract:**
  - the `sequence` declaration;
  - `Step` and `Track` checked structurally;
  - `sendMIDI` and `requestMIDIAccess` in `HOST_COMMANDS`;
  - `midimessage` among the events;
  - `midiinputs` and `wakelock` among the attributes;
  - `playbackRate` and `pan` on hits;
  - `files=` on `share`.
- **Runner:**
  - `transport.rs`;
  - `sound.rs`'s bound and the `edit` ending;
  - the MIDI queue and record;
  - two reserved sources;
  - the clock-batch call;
  - the background stop.
- **Lean and difftest:** the commands and the event, as LLP 1096 D11 added
  its commands. The expansion is a pure function that difftest can compare
  between the runner and the JS target.
- **Apple:**
  - capacity, rate and pan in `soundarm/`;
  - `ExactMIDI`;
  - the wake lock;
  - pictures in `Share.swift`.
- **Web:**
  - capacity, rate and pan in `sound-glue.js`;
  - `midi-glue.js`;
  - the wake lock;
  - file share.
- **Linux, Windows:** the record.

## 6. Considered, not taken

- **The host expands the pattern** on its own thread (r1). It makes a second
  producer into Apple's single-producer ring. It splits voice semantics between
  the runner and four expansions (Rust, the JS twin, Swift, the web glue), so
  groups and the bound between a sequence and a tap would be undefined. With
  the runner a second ahead, only a main-thread stall over 500 ms costs a step.
- **A fixed 100 ms edit horizon** (r1). Voices can be retracted, so an edit
  applies at its commit. Only the 50 ms hold before a MIDI send is
  irrevocable.
- **The window clock as the transport** (LLP 1096 D3). It stays for a few hits.
  As a transport it costs an app commit every 25 ms and leaves polymeter and
  clock in to every app.
- **A `sequence` schedule on `task`** (r1). The gate grammar is the same, but
  a task fires an action and captures literals, while a sequence's arguments
  are re-read after every commit. Two of three reviewers preferred the
  declaration.
- **QR in exact2.** A QR code needs no clock and no platform, so it is data
  code. Two of three reviewers, and this revision, keep it in the app: a port
  of `qrcode`'s encoder in a data module (the SwiftUI port's is 228 lines),
  until a library can be shared through a package. Grok held that one encoder,
  held to `qrcode` bit for bit, is worth centralising because Core Image
  disagreed across ports.
- **Unconditional CC 123 on stop.** It silences notes the sequence did not
  play.
- **Dropping an input message past a bound.** It can drop a note-off.
- **An AudioWorklet mixer.** It is not needed once the runner schedules a
  second ahead.

## 7. Stages

Each stage stands alone, has a consumer, and lands after Astra and Grok review
its diff.

1. **Sampler controls and keep-awake** (D10, D11).
2. **The sequence** with sample and haptic steps (D2–D5): the runner,
   `exactTransport()`, the JS twin, capacity on both outputs, the background
   stop, and the driver. Its consumer is a metronome and drum machine (an
   example app), and euxy's kit playback.
3. **MIDI** (D6–D8): ports, `sendMIDI`, MIDI steps and `offset`, clock out,
   `midimessage`. With stage 2, euxy deletes its scheduler, `Sampler.swift` and
   `MidiManager.swift`.
4. **Clock in** (D9).
5. **Files and pictures in a share** (D12). euxy deletes `ShareCard.swift`'s
   share path.

The bar for stage 2 is LLP 1096 §6's device check, applied to a sequence:
onsets on the grid within the mixer's precision on a Mac and an iPhone, while
a test action blocks the main thread for 300 ms. The bar for stages 3 and 4
is euxy against a real OP–XY: clock out and notes, then following its clock
through a count-in and a take.

## 8. `rules/DEFERRED.md`

The admission is Charlie's 2026-10-16 instruction ("once happy with it, go
ahead and implement"), recorded as an expansion with no take offered.

**Still out:**

- on-demand decoding (D10);
- web pictures of an element (D12);
- QR in exact2 (§6);
- SysEx;
- Bluetooth MIDI pairing;
- MIDI 2.0;
- Continue and Song Position out;
- Ableton Link;
- swing;
- Linux, Windows and Android output;
- synthesis;
- compressed formats;
- Core Haptics patterns.

## 9. Questions decided (the orchestrator, for Charlie, 2026-10-16)

1. **Admission:** by the instruction (§8). The three reviewers agreed the
   2026-10-15 euxy entry, which named chrome, did not cover this.
2. **Surface:** a `sequence` declaration with a task's gate grammar (§6).
3. **Edits:** at the commit, by retraction; no horizon (D3).
4. **`key=`:** restarts, sending Start. Continue is deferred (§8).
5. **`Step`:** flat, as `playSounds`'s hits are.
6. **Decoding:** eager; on demand is deferred (D10).
7. **QR:** app code (§6).
8. **`canShareFiles`, `canSharePictures`:** both on `exactPage()`.
9. **Web MIDI's reach:** a declared limit; WebKit is `unavailable`.

## R. Round 1, and what r2 changed

Each finding was checked against the code or the platform before it was
taken.

| Finding | Reviewers | r2 |
|---|---|---|
| The host expanding into Apple's single-producer ring, with voice semantics split from the runner's table | Muse 1, Astra 3, Grok 3 | The runner expands; hosts unchanged (D3, §6) |
| The 100 ms horizon: no slack, and retraction already exists | Muse 2, Astra 4, Grok 2 | Edits at the commit; 1 s lookahead (D3) |
| Clock in not reproducible from tempo updates | Astra 1, Muse 4, Grok 4 | Tick batches are a recorded input (D9) |
| Stop cannot recall queued MIDI; Chromium lacks `clear()`; CC 123 too broad | Astra 2 | Hold within 50 ms; note-offs after their note-ons; no CC 123 (D4, D7) |
| The 257th message dropped can be a note-off; controller floods | Grok 1, Astra 7, Muse 8 | Notes never dropped; continuous messages coalesce; `midiinputs=` (D8) |
| `sequence` is not a schedule: its arguments are re-read | Muse 3, Grok 5 | A declaration; the re-read rule normative (D2) |
| Endless loops only; no end signal | Astra 6, Muse 7, Grok 5 | `loops` and `ended=` (D2) |
| Background resume contradicts euxy, which stops | Muse 5, Astra 6, Grok 4 | Stop on suspension by default; `playback` plays on (D4) |
| No per-output latency offset | Muse 6 | Track `offset` (D2, D7) |
| On-demand decoding regresses the first hit and lacks readiness | Muse 9, Astra 8, Grok 7 | Eager stays; on demand deferred (D10) |
| QR is the wrong layer | Muse 10, Astra 9; Grok dissents | App code (§6) |
| `foreignObject` is not the element as drawn | Muse 11, Astra 10, Grok 6 | Web pictures refused by name (D12) |
| Apple has no per-element layer | Muse 11 | Not taken: one `UIView` per node (`NodeViewIOS.swift:1`) |
| Stage 2 cannot delete euxy's scheduler before MIDI | Muse 12, Astra 11 | Stage 2's consumer is a metronome and drum machine; euxy deletes after stage 3 (§7) |
| The euxy waiver names chrome | all three | Admitted by the 2026-10-16 instruction (§8) |
| Keep-awake leaks in a mounted hidden screen | Muse 13 | Mounted and displayed (D11) |
| The undecidable bake check for two sequences | Muse 7 | A run-time rule (D2) |
| Main-queue haptics can be late by a whole stall | Astra 5 | Stated; late haptics are counted (D3) |

## As built (stage 2, 2026-10-16)

D2–D5 with sample and haptic steps, as written, except where this says
otherwise.

- **Compiler.** `sequence NAME (mount | when cond [key=expr] | key=expr)`
  holds one `play(...)` (`contract-sequence-body`); the gate grammar is the
  task's, one `Parser::gate`. The type pass checks a track's and a step's
  shapes whole and in order, as `playSounds`'s hit (`type-sequence-play`),
  and the options' types; `bpm=` is required and a literal one above 0. A
  child's sequence is `type-child-resource`; a gate, key or argument that
  reads `performanceNow()` is `analyze-task-gate-clock` or
  `analyze-sequence-clock`; `ended=` names a parameterless action.
  `clock=` and `clockOut=` are refused until their stages
  (`lower-sequence-clock`, `lower-sequence-clock-out`). Plan `sequences`
  carries the gate, the key and each argument as code (an omitted one a
  constant) and `ended`. `exactTransport` is a runner-owned source;
  `bake-transport-field` refuses a field it does not answer. Fifteen reject
  fixtures name the new ids.
- **Deviation: `length` is two names.** Contract refuses a field named
  `length` (`syntax-length-field`: `.length` is a list's), so a track's
  `length` is `beats` and a step's `length` is `duration`. Everything else
  is D2's, in D2's order (the rulings' nine fields, then optionally
  `playbackRate` and `pan`).
- **Runner** (`runner/src/transport.rs`, `runner/src/runner/transport.rs`).
  The gate step reads each sequence after the tasks' inputs, inside the
  rollback; the transport takes the reading in `conclude`'s standing arm, so
  a refused commit changes nothing. Its fill and the count-in's end are one
  wake (`wake_due`), chosen in `advance_within` after everything due at the
  same time; its end is a timer whose index follows every task's; the
  `exactTransport` answer is a `commit_again` of its readers chosen as a
  `then` after every mutation's. Each window is `[issued_to, at + 1000)`,
  refilled at `issued_to − 500`. Hits sort by time, then track (the
  count-in first), then step. `stood` arms on a gate turning true or a key
  changing, so a sequence stopped by its end, the background or a bad tempo
  stays stopped until then.
- **Rules beyond the text** (each the simplest deterministic one, in the
  module's comment):
  - `stopSounds()` stops a sequence's sounding voices and leaves those not
    yet started: they are its next steps (D4), and re-issuing identical
    voices would only renumber them (rulings #3).
  - A `bpm` that is not a positive number, or a `from` that is not finite,
    stops the sequence (reason `bpm`) and is journaled; the gate must turn
    true again.
  - A track is dropped and journaled when its length is not positive, its
    `loops` is not a whole number, or it would loop more than 1,000 times a
    second at its tempo: the bound that keeps a fill's work finite. A step
    is dropped when its beat is outside its track (or `[0, −from)` for the
    count-in), its sound is undeclared, its haptic kind unknown, or its
    gain, rate or pan not a number; a step with neither a sound nor a
    haptic plays nothing and is left out unsaid. Each read of a changed
    value journals its drops once.
  - Arguments compare as values, numbers bit for bit (`Object.is` in the
    twin), a shared list at once; a changed value that reads as the same
    play is no edit. A new segment starts only when `bpm` changes.
  - At the waiting cap the fill stops at that hit's time, remembering how
    many hits at that time it issued, and wakes at the first waiting
    voice's start.
  - Haptics are not voices: they count toward neither bound, are recorded
    (1,024) and journaled `haptic N kind at T`, and an edit or stop
    retracts the ones still to come (`haptics retracted: N (edit)`).
  - The waiting cap (256) applies to taps: the 257th waiting is dropped,
    `sound dropped: 256 voices wait (…)` (rulings #4). The 33rd voice
    sounding at a start is `sound dropped: 32 voices sound at its start (…)`.
  - A track's `offset` and `output` and a step's MIDI fields are read and
    typed and play nothing until stage 3 (rulings #8); the live clock skips
    a hit already past at a fill and journals the count (rulings #7).
  - `exactTransport` while stopped answers the last segment's `bpm` and
    anchor (0 before any), and `reason` `""` before anything played.
- **The background.** `Page` gains bit 7, `suspended`, which no `exactPage`
  field reads. Apple sets it on iOS and tvOS while hidden unless the
  session is `playback` with `audio` in `UIBackgroundModes`; a rising bit
  while a sequence plays stops it (reason `background`) in a commit that
  carries its ends. A hidden Apple session's wake within a few frames is
  now a timer, not the display link, which does not fire there: without it
  a `playback` metronome would stop filling with the screen locked.
- **JS target** (`host/web-js/transport.js`, `sounds.js`). The twin keeps
  the runner's rules line for line: its wake is a `hatch` in `clock.timers`,
  its end a timer with `i: Infinity` (a hatch too without `ended=`), and the
  answer a pseudo-mutation kept last in `Mutations`. `rt.js` changed three
  lines and gained none. The module is imported only by a plan that
  declares a sequence or reads `exactTransport`.
- **Hosts.** Apple's mixer has 288 slots and a 1,024-op ring and frees a
  voice retracted before it starts at once; `SoundOutput` plays a haptic on
  the main queue at its time without opening the output, and counts those
  more than 10 ms late (`hapticsLate`). The web glue lets a retracted node go
  and plays a haptic as `navigator.vibrate` on a timer. Linux drops the ops.
- **The driver.** `state.transport` (only for a plan with a sequence):
  `state`, `reason`, `sequence`, `bpm`, `anchorAt`, `anchorBeat`, `clock`,
  `issuedTo`, the last 16 segments, `recorded`, and the last 64 haptics with
  their states. `expect sound … by edit` parses. No driver operation or
  page fact was added; on iOS `prefer visibility-state=hidden` reaches the
  background stop.
- **Consumer.** `apps/metronome`: a tempo stepper, an accent on beat 1, a
  count-in, a haptic on each beat and a four-track drum pattern (tracks of
  4, 4, 1 and 3 beats), with a playhead from `exactTransport()` in a frame
  task gated on playing; six WAVs, 11.6 KB in all.
- **Tests.** `runner/src/transport/tests.rs` (13): the seek rule over a
  count-in, polymeter, a tempo edit by a timer, a key restart, a stop and a
  frame task (voices, haptics, journal and answer equal); an edit's
  retraction; tempo without a jump in phase; a dropped step journaled once;
  the waiting cap; the 33rd voice; `ended=` at its exact time; the answer
  commit; a refused commit; a second sequence; `stopSounds`; the wake; the
  background latch. A Linux agent test: `clock settle` does not wait for a
  fill. `SoundMixerTests` (288 slots, a retraction frees its slot, a
  1,024-op ring); `sound-glue.test.mjs` (a retraction is silent and lets
  its node go; haptics timed, cancelled and counted late).
- **Measured.** Conformance `transport` (a count-in, polymeter, a haptic,
  a dropped step, an edit, a tempo change, `stopSounds`, a key restart, the
  gate's stop and `ended=`): 25 of 25 steps equal on wasm, JS and Linux,
  `sequence`, `haptic` and `sound` journal lines and `state.transport`
  included; `sounds` 30/30, `gates` 19/19, `frames` 9/9, `clock` 10/10 on all
  three. The metronome's five tests pass on the web (JS target), macOS and the
  iOS 27 simulator.
- **After review** (Astra, Grok): a background stop is applied once its
  commit stands; a refused commit puts back its settled derives for a plan
  with a sequence; `from` must be within 2^31 beats and a track at least
  2^-20 beats long, so a loop's index stays an exact whole number; an end
  already past is the arming's or edit's time; an end that runs `ended=`
  is held at the timer limit like a task's timer; a hit blocked by the
  waiting cap is due no later than its own time (taps far ahead do not
  starve it); haptics still to come outlive the record's bound; the JS
  twin reads every gate before arming any. On Apple a stop frees its
  voices' slots for the Plays behind it in the same render, and ops the
  1,024-op ring has no room for wait, in order, in `SoundOutput`'s backlog
  (`exact_sound_room`), so an advance across several fills drops none.
  Conformance `transport` gained a `from` past exact counting and an end
  already past: 34 of 34 equal.
- **After review round 2:** a fill stops the sequence (reason `bpm`, the
  beat journaled) when a track's loop index would pass 2^52, which a beat
  kept across an edit can reach where `from` and a length cannot; on a
  live clock only a hit more than 500 ms past is skipped, so a held hit's
  wake, just after its time, still plays it; the background stop rides
  whichever commit of the `set_page` call stands (a recovery commit after
  owed reads included); the JS twin gives each arming its own end entry,
  so an `ended=` that starts another sequence is still counted as a timer.
  On Apple a voice stopped before its first render gives its slot up, and a
  boot's stop-all waits its turn in the backlog.
- **After review round 3:** the 2^52 check drops only a track that loops
  forever (or as many times as that) and would pass it, journaled, and the
  rest play on, so a finite track's `ended=` still runs. On Apple a Play
  that finds no free slot takes one whose voice ends inside the buffer,
  mixing that voice's remaining frames first (a sum, so it sounds as it
  would have): retriggers cut within one buffer, however many rounds, fit
  the runner's bounds, 288 slots.
- **Not done here.** The device check (onsets on the grid with the main
  thread blocked for 300 ms) was not run; nothing in this lane played to a
  speaker. MIDI steps, `offset` and clock out are stage 3; `waiting` and
  clock in stage 4. A plan whose sequence has haptics and no declared sound
  sends its haptic ops through the sound batch, which the Apple bake's
  sound arm need not load for; that path was not driven.

## As built (stage 3, 2026-10-16)

D6–D8, MIDI steps, track `offset` and `clockOut=` (D2, D7), as written,
except where this says otherwise; clock in (D9) stays refused
(`lower-sequence-clock`). The orchestrator's fifteen rulings for the stage
are binding and two of them amend the text:

- **D8, "each mounted handler hears it in that one commit":** each
  listening element hears a message in a commit of its own, in tree order, at
  the same instant (as `beforeunload` is heard). A refusal in one does not
  stop the next, nor the next message (a note-off still arrives).
- **D1, "`ExactMIDI`, linking CoreMIDI only when used":** Apple's MIDI is a
  loaded arm, `libexact_midi.dylib` (the `ExactMIDI` framework on iOS), as the
  sound arm is, carried by use: reading `exactMIDI`, `sendMIDI` or
  `requestMIDIAccess`, a `midimessage` handler, or `play` with `clockOut=`
  (the bake's `loads` names `midi`). A sequence's notes alone load nothing:
  without access no note is sent.

- **Compiler.** `requestMIDIAccess()` and `sendMIDI(output, data, at=)` are
  host commands the runner keeps (`type-request-midi-access`,
  `type-send-midi`, which holds a literal `data` to MIDI 1.0); `midimessage`
  is an event whose action may take `MIDIMessageEvent {input, data,
  timeStamp, beat}`; `midiinputs=[…]` is the kernel prop `midiInputs` (269),
  newline-joined (`onMIDIMessage`, `midiInputs` are refused with these
  names); `exactMIDI` is a runner-owned source whose fields and port fields
  the bake checks (`bake-midi-field`); the grant row `device.midi` maps to
  `Permissions-Policy: midi` and asks for nothing on Apple. Contract has no
  hex literals, so an app writes `[144, 60, 100]`.
- **Runner** (`runner/src/midi.rs`, `runner/src/runner/midi.rs`). One queue
  of timed messages, a sequence's notes and ticks included; a message is
  *handed* once `at − 50 ≤ t`, a rule rather than a host's report, and a
  retraction takes back only what is not handed. At most 1,024 wait;
  note-offs, Stop and ticks are exempt. Ports merge: one seen stays listed as
  `disconnected` for the session. A host's report re-answers the `exactMIDI`
  readers in one commit. On a seek clock (the driver, a test) a request is
  answered by the runner from the drive's `prefer midi.*` (granted unless it
  names a state) in a commit of its own, as `exactTransport`'s answer is;
  on a live clock it is a `request` op to the host. Without the grant the
  state is `denied` and the request is journaled by name. Input deliveries
  coalesce Control Change, pitch bend and both pressures within one delivery,
  never across a note or transport message of the same input and channel;
  `e.beat` is −1 only while stopped (an app reads `exactTransport().state`
  to tell a count-in's negative beats apart).
- **Sequence.** A step whose `note` is not −1 on a track with an `output` is
  a note-on (`0x90 | channel`) and a note-off `duration` beats later. An edit
  at `t` takes back the waiting messages whose step is at or after `t`, but a
  note-off whose note-on was handed stays; a stop takes back every waiting
  one and moves such a note-off to the later of `t` and its note-on; a second
  note-on of one output, channel and note moves the earlier note's waiting
  note-off to it, sent first. A track's `offset` shifts its samples', notes'
  and haptics' times, never which hits a window holds: at the arming an
  output before the commit plays at it, at an edit it is skipped (it played
  already), on a live clock one past at a fill is late. Clock out ticks on
  the beats `k/24` from the arming's (the count-in's included), sends Start
  before the first tick at a beat ≥ 0, Stop at a stop no earlier than the
  last tick handed, Stop then Start at a key restart, and Stop at the end
  (`ended=`, whose waiting note-offs stay). Ticks are counted in
  `state.midi.clock`, not listed.
- **Rules beyond the text and the rulings** (each the simplest deterministic
  one, in the module's comment):
  - An edit's MIDI starts 50 ms after it: a note due within the hold counts
    as played, since re-issuing it would double a note already handed.
  - Retraction is keyed by the step's time, not the output's, for voices,
    haptics and MIDI alike, so an unchanged positive offset loses nothing at
    an edit. An edit of `offset` itself may play a step twice or not at all
    within the change's size: a declared limit (rulings #8).
  - An overlap moves only a waiting note-off; a handed one is left.
  - Start is sent only once the clock reaches a beat ≥ 0, and Stop only
    after a Start. A `clockOut` changed while playing stops the old output's
    clock; the new one starts with Start before its next tick at or after
    beat 0. Clock out above 1,000 ticks a second (a tempo over 2,500) is
    dropped and journaled.
  - A step's MIDI fields are checked only when `note` is not −1 (whole
    0–127, velocity 1–127, channel 0–15, duration above 0) and a track whose
    `offset` is beyond ±500 ms is dropped, each journaled; notes the value
    cannot send (no access, an output not connected) are journaled once per
    value read and skipped.
  - Any channel message that is not continuous bounds its channel's
    coalescing; any system message bounds its input's.
  - `tap midi`'s bytes may hold several messages, one delivery, so a drive
    coalesces as an arm's delivery does.
  - A dev reload ends the queue as a stop does, then hands every other
    waiting note-off at once.
  - `state.midi` (and the JS target's midi.js) is there for a plan that uses
    MIDI or declares a sequence (`uses::keeps_midi`).
  - The journal's lines avoid the word "refused", which the differential
    driver reads as a refused commit.
- **JS target** (`host/web-js/midi.js`): the runner's rules line for line;
  `onMIDI` registers listeners as the view is built, `Sounds.midiApply`
  applies the commit's MIDI commands after the voice table's, and the grants,
  which come with the data module and may land after the first frame, ask
  the readers again when they do. Conformance found two of sounds.js's own:
  it now applies only its three commands and says `state.sounds` only for a
  plan that declares a sound, as the runner does.
- **Hosts.** Every host takes a `{"op":"midi","boot"?,"ops":[send, retract,
  open, request]}` batch op and hands back the ports or one delivery of
  input through one export, `exact_midi`. Apple: CoreMIDI on the arm's own
  thread and run loop, made at the first request (no CoreMIDI client before
  it: `prompt`), ports by `kMIDIPropertyUniqueID`, sends held on that thread
  and sent with `MIDISendEventList` in host time (never `MIDIFlushOutput`),
  inputs stamped from the packet, a 3 s watchdog while a port is in use and
  re-enumeration at the foreground; tvOS is `unavailable`. Web:
  midi-glue.js for both targets, `granted` without asking when the
  permission holds, held sends on the page's timer (late ones counted),
  input batched per frame. Linux and Windows keep the record; off the agent a
  request finds `unavailable`. No MIDI loads under the agent.
- **The driver.** `prefer midi.state|inputs|outputs`, `tap midi "<input>"
  <hex…>`, and the test steps `prefer midi.…` (a step beyond the RFC: a
  test needs ports), `tap midi …` and `expect midi sent|missing "<output>"
  <hex…> [at N]`, a byte two hex digits rebuilt from the tokens that touch.
- **Consumer.** The metronome gains a MIDI output picker (a `select` of
  `exactMIDI().outputs`), "Send MIDI clock", and General MIDI percussion notes
  on channel 10 for the clicks (wood blocks 76, 77) and the drums (36, 38, 42,
  39) when an output is chosen; it grants `device.midi` and gains a Linux host
  so conformance can drive MIDI on all three runners.
- **Measured.** Conformance `midi` (ports, a request, `sendMIDI`'s `at=`, a
  late and a malformed one, notes on a connected and a missing output, clock
  out, a tempo and an offset edit, a key restart, the gate's stop, `ended=`,
  a coalesced delivery heard by two elements, an output pulled): 35 of 35
  steps equal on wasm, JS and Linux, journal lines and `state.midi` included;
  `transport` 34/34, `sounds` 33/33, `gates` 19/19, `frames` 9/9, `clock`
  10/10 still equal. The metronome's eight tests pass on the web, macOS, the
  iOS 27 simulator and Linux. The macOS loopback XCTest (virtual endpoints)
  passes: a send arrives on its time and never early, a retraction holds, an
  input is stamped right, a replugged source keeps its id; a probe measured a
  send 0.06 ms after its `at`.
- **After review** (Astra, Grok; round 1): a `reload` command ends the
  queue as a stop does, every waiting note-off handed at once, in the runner
  and the JS twin; a host that replaces its runner (a boot, a new
  generation) first ends the old runner's voices and notes and sends that
  batch before the new boot's, and a boot or a session's end settles what
  the output still holds: a note-off or Stop goes now, the rest is let go
  (Apple's arm, midi-glue.js). A note-on due within the hold takes no
  waiting place, so a full queue never blocks a note that is due (a fill
  blocked at it had woken at its own time forever). No tick goes before the
  last Stop's time, so a restart's Start reaches the device after the old
  clock's Stop; a waiting Start an edit takes back is sent again. Apple: the
  setup notifications enumerate on the arm's thread, where every other
  change to the ports runs; the receive block only copies messages into a
  preallocated lock-free ring (`midiarm/midi_ring.h`) that the arm's thread
  drains; inputs a listener opened before the request are listened to once
  it is granted. The JS twin answers `exactTransport` before `exactMIDI` at
  one time, as the runner does (conformance `midi-order`), and a listener's
  refusal is `tap midi`'s refusal there too. midi-glue.js journals a refused
  request as `access denied`.
  Round 2: a fill a full queue blocks wakes when the blocked output can take
  a place (its track's offset counted), never again where it stood; a
  runner ended for its replacement takes the host's clock first; only the
  clock's own Start, taken back, is sent again; a note-off or Stop sent at
  teardown is stamped no earlier than its note-on or the output's last
  message; Apple's input ring keeps its last 1,024 places for notes and
  transport messages.
  Round 3: a teardown note-off is stamped no earlier than the latest
  note-on of its note handed, whichever was handed last; a note-on the
  runner takes back after Apple's arm or midi-glue.js handed it (the
  runner's clock behind theirs) is ended by a note-off at once, after it;
  an edit fills from where a full queue parked the fill when that is
  before its time, hits timed by the segment each falls in, and takes for
  handed (the within-hold skip) only what a fill issued, by beat.
  After the rounds (the orchestrator): a parked note's note-off is timed
  by the segment its end beat falls in, not the newest. Still open, both
  found by the last confirmation and left by the three-round rule
  (`QUEUE.md`): Apple's arm and midi-glue.js keep about a second of handed
  note-on ids, so a retraction delayed past that leaves its note sounding;
  and an edit at the very beat a full queue parked the fill keeps that
  beat's skip, so a note already retracted there is not issued again.
- **Not done here.** No real device or IAC bus was available, so nothing was
  sent to hardware from the Mac, and Chrome's Web MIDI was not tried by
  hand. The watchdog and iOS foreground re-enumeration are not exercised by a
  test. The Lean semantics were not run (not installed); the corpus's
  `send-midi` agrees between the runner and the JS target. euxy's port has
  not yet deleted its scheduler or `MidiManager.swift`.

## As built (stage 5, 2026-10-16)

D12 as written, except where this says otherwise.

- **Compiler.** `share` takes `files=`, a list of strings, beside
  `title=`, `text=` and `url=`; one of `text`, `url` and `files` is
  required. A literal entry is an `app:/` path or a non-empty `"#id"`, and
  a literal `files=[]` is refused (`type-share-argument`, three new reject
  fixtures). Lowering orders the arguments `(title, text, url, files)`, so
  every host's `share` now has four. A literal `"#id"` that no element of
  the whole expanded view carries is `lower-share-picture`; where any
  element's id is computed, a node is not expanded, or a `markdown` draws
  elements of its own, the check stands back and the runner refuses a
  missing id when the share runs.
- **Runner** (`runner/src/share.rs`). One rule on every host, before the
  agent's hold: an `app:/` entry must be a plain app path that the app's
  `fs.read` grant covers (`covered`, as `saveFile` checks it; LLP 1069.008
  D3), and an `#id` must name an element in the kernel now (`no element
  with id "card"`). What a host cannot share is a `Reach`, refused by name
  before the hold too, since the host would refuse it for real: the web's
  pictures, Android's files. The present reply carries the files in order,
  `{"path":…}` or `{"id":…,"view":N}`; the hold's summary gains `files`
  (always present, `[]` when none).
- **Page facts.** `exactPage()` gains `canShareFiles` (bit 8) and
  `canSharePictures` (bit 9). iOS and macOS: both true (tvOS: false). The
  web: `canShareFiles` is `navigator.canShare({files})` for a one-byte
  `text/plain` File, read once; `canSharePictures` is false. Linux and
  Android: false. Under the agent both are true on every native host, as
  `canShare` is (the substitute answers there); on the web `canShareFiles`
  is true and `canSharePictures` false, since the web refuses a picture
  under the agent as well. `prefer can-share-files` and
  `prefer can-share-pictures` set them on every host.
- **Apple** (`Share.swift`). A path goes as a copy the library makes
  (`shareFile`, `host/apple/src/picker.rs`), read as `readFile` reads it:
  `fs.read` admitted by the grant parser, every component opened relative
  to the roots' descriptors with `O_NOFOLLOW`, a regular file only; a link
  anywhere in the path is refused (`… Too many levels of symbolic links`).
  A picture is `ShareSheet.picture(of:)`: on iOS
  `UIGraphicsImageRenderer` at the view's `displayScale`, drawing with
  `drawHierarchy(afterScreenUpdates: false)` on screen and `layer.render`
  off it; on macOS `bitmapImageRepForCachingDisplay` and `cacheDisplay`,
  at the window's backing scale. Entry `n` is written as `n/<name>` (the
  file's last segment, or `<id>.png`) in a fresh temporary directory, so
  two entries whose names come out alike stay two files; the directory is
  removed when the sheet reports its outcome or fails to present. A picture
  is journaled `share: picture of #card: W×H px, N bytes`; a node with no
  view or no area is `share: refused: #card is not drawn`.
- **Deviation: `EXACT_DEVICE=real` now reaches `share` on Apple.** The
  session asked the runner with `agent` set to agent mode, so a real sheet
  could not be seen under the driver; it now sends `PageFacts.substituted`,
  which LLP 1069.007 D8 already defines.
- **Web.** The runner (wasm glue) or `files.js` (JS target) rules; then
  `picker-glue.js`'s `share` reads each file (a picked File, else the app's
  store) into a `File` named by its last segment and typed by its
  extension, and calls `navigator.share` when `navigator.canShare` takes
  them, else `share: refused: this browser cannot share these files`. A
  share without files still calls `navigator.share` synchronously in the
  dispatch.
- **Deviation: the web reads files after the press.** Reading the store is
  asynchronous, so `navigator.share` runs after awaits, inside the press's
  transient activation rather than synchronously in it. Chromium keeps
  activation for about five seconds; a browser whose window is shorter, or
  a large file, gets `share: refused: NotAllowedError`. Chromium's
  `canShare` also refuses types outside its share list (JSON among them),
  so Fieldnotes' backup is refused there by name.
- **Ruling: Android refuses files** (`share: refused: files are not
  carried to Android's share sheet yet`). Carrying them needs a
  `FileProvider` in the Canvas host's manifest, the presenter's app roots
  mapped to content URIs, and a picture from the Canvas renderer rather than
  a platform view: not a contained change. Text and links still go to the
  Sharesheet. Desktop Linux refuses `unavailable` as before.
- **Consumers.** Metronome: a tempo card (`id="card"`: the tempo and the
  pattern) and **Share as image**, `share(title=, text=, files=["#card"])`,
  shown where `canSharePictures` holds. Fieldnotes: **Share backup file…**
  beside Export, `files=["app:/data/backups/fieldnotes.json"]`, where
  `canShareFiles` holds.
- **Tests.** `runner/src/share.rs` (files alone, strings, the JSON
  door's list); `contract/cli/tests/it/share.rs` (the four-argument order;
  Apple presents a picture by its view and a granted path in order; the
  web refuses the picture, under the agent too; an ungranted path is
  refused everywhere; the hold records the entries; the JSON reply; the
  compile-time `#id`); `host/web-js/files.test.js` (the JS twin's hold and
  refusals; `picker-glue.js` with a fake navigator: typed Files, `shared`,
  `dismissed`, `NotAllowedError`, `canShare` false, no store);
  `host/web/agent.test.mjs` (bits 8 and 9, `canShare({files})`);
  `ShareMacTests`/`ShareIOSTests.testPicture` (a red node drawn to a PNG of
  its points times the display's scale, red at its centre; no area, no
  picture).
- **Measured.** Driven under the agent: Metronome on macOS, the iOS 27
  simulator and the web (`prefer can-share-pictures true` then the button:
  the web's refusal by name); Fieldnotes on the web (the hold lists
  `files: ["app:/data/backups/fieldnotes.json"]`). With `EXACT_DEVICE=real`
  (on the simulator through `SIMCTL_CHILD_EXACT_DEVICE`): macOS drew the
  card at 139×94 px (a scale-1 window), the iOS simulator at 497×339 px
  (3×), and both presented with no refusal.
- **Not done here.** No one picked an activity in a real sheet, so the
  outcome lines of a file share were not seen; the window screenshots do not
  include the sheet. A real browser's `navigator.share({files})` was not
  run (the glue was tested with a fake navigator). A scratch directory
  whose sheet is still open when the app quits stays in the temporary
  directory until the system clears it. euxy's `ShareCard.swift` was not
  touched (its port is outside this repo). The wasm glue's share path
  (`host/web/glue.js`) was tested through the runner's JSON door, not
  driven in a page. `contract-difftest quick` was not run: Lean is not
  installed on this machine.
- **After review** (Astra, Grok). Apple shared the URL `appFile` joined,
  which follows a link, so `fs.read app:/data` could put a file outside
  the grant on the sheet; it now shares the confined copy above (tests: a
  leaf link, an ancestor link, and a file replaced by a link before the
  read, in `picker.rs`; driven on macOS: a backup swapped for a link while
  the app ran was refused). The web reads through the app's IndexedDB
  store, keyed by the normalised path, which has no links to follow. The
  runner's `covered` read a grant line by whitespace, so `fs.read
  "app:/data/a b"` admitted nothing in Rust while the browser admitted it;
  it now parses each line with `exact_grants` and asks `permits`, which
  `saveFile` shares (pinned against the browser's form: `save_file.rs`
  checks the parsed JSON byte for byte, and `files.test.js` runs the real
  `coversPath` over it). Two pictures whose ids sanitise alike
  (`#card.one`, `#card-one`) wrote one file; each entry now has its own
  folder.
- **After review round 2.** The share's copy reopened the roots by name,
  following a link at or above a root; the roots' handles are now pinned
  when storage is configured (`set_roots`) and a share reads only through
  them (test: a data root swapped for a link after pinning still yields
  the pinned tree's file). A grant declaration with any line the parser
  refuses now admits nothing, in the runner as in the browser (parity
  pinned on both sides). The copy runs on the main thread, so it is
  bounded: a file over 64 MB (`SHARE_COPY_MAX`) is refused by name
  (`larger than the 64 MB a share copies`), and one that grows past the
  bound while copied is refused and its copy removed.
- **After review round 3.** The pinned handles were one per process, so
  in the sample host's two sessions (LLP 1031) the second's storage
  replaced the first's and a share in the first read the second's tree.
  They are now kept per app, by the roots' paths, and a share looks up the
  roots of its own session's app (test: two pinned trees, each share reads
  its own; roots never pinned have no files).
