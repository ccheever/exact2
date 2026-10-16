# LLP 1125: A transport, MIDI, and the small pieces apps hand-roll

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built until Charlie has read it)
**Status:** Draft r1, 2026-10-16. Not reviewed.
**Systems:** Contract compiler (a `sequence` schedule for `task`; `sendMIDI`; `midimessage`; `wakelock`; `qrPath`; `share`'s `files=`), Plan (`sequences`, `midi` use), Runner (new transport record beside `runner/src/sound.rs`; `exactTransport()` and `exactMIDI()` reserved sources; the journal), the sound arm (`host/apple/soundarm/`, `host/web/sound-glue.js`, `host/web-js/sounds.js`), a new MIDI arm (Apple `ExactMIDI`, web `midi-glue.js`), Share (`runner/src/share.rs`, `Share.swift`), the agent (`state.transport`, `state.midi`, `state.wakelock`, test steps), `runner/src/device.rs` (one grant row)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-16
**Implementer:** unassigned until ruled; §7 stages the work so each piece can get its own lane
**Amends (if accepted):** LLP 1096 §7 (pitch, pan and on-demand loading leave the deferred list; D9 here), LLP 1069.003 D7 (files in a share, with an element's picture as a file, D12), LLP 1069 §5 #10 (keep-awake gets its consumer, D10), `docs/recipes/native-views.md:56–58` (the time rule names the transport as the exception for timed output), `rules/DEFERRED.md` (§8, question 1)
**Related:** LLP 1096 (sounds: the voice table, groups, D6's runner time to the speaker, D8's one session owner); LLP 1098 (the media session: events on an element by the web API's names); LLP 1092 D7–D10 (gated tasks, arming from the commit, the seek rule); LLP 1073 (`every(frame)`); LLP 1077 D14 (`haptic`, `press-haptic`); LLP 1069.000 (reserved sources for device facts); LLP 1069.007 (capabilities in the agent's hands; streams excluded, §5 here); LLP 1069.008 (grants to permissions); LLP 1069.009 (replayable inputs); LLP 1047 and 1047.001 (linked and loaded tiers; a Swift target per capability); LLP 1067.000 (one native module per app); LLP 1124 (host extensions; why these are not, §6); LLP 1112 (pictures of a state, parked; not this). The consumer: euxy, a Euclidean MIDI sequencer (`~/projects/euxy-src/euxy`), and its exact2 port (`~/projects/euxy-ports/exact2/euxy`, `FRICTION.md`, `NOTES.md`, `modules/apple/`). External, read 2026-10-16: Web MIDI API (W3C WD), Screen Wake Lock API (W3C CR), Web Share API Level 2 (`files`), Web Audio API 1.1 (`playbackRate`, `StereoPannerNode`), Apple CoreMIDI (`MIDIClientCreateWithBlock`, `MIDISendEventList`, `kMIDIPropertyUniqueID`), `UIApplication.isIdleTimerDisabled`, IOKit `IOPMAssertionCreateWithName`, Android `View.setKeepScreenOn` and `android.media.midi`, `qrcode` 1.5.4 (npm), ISO/IEC 18004:2015.

## Summary

Each of the three euxy ports (SwiftUI, UIKit, exact2) wrote the same
things by hand. The exact2 port carries 1,607 lines of Swift in
`modules/apple/`. Most of them are a lookahead scheduler, CoreMIDI, a
sampler and a share card. This RFC proposes those pieces as part of exact2's
standard vocabulary, built so that a drum machine, a metronome, a rhythm game,
a practice timer, a MIDI controller surface or a ticket app would use them as
they are:

| | Piece | What the author writes | Where it runs | Stage |
|---|---|---|---|---|
| D1 | Packaging | nothing: each piece is linked or loaded only when the plan uses it | — | — |
| D2–D5 | **Transport** | `task groove when playing` / `sequence(tracks, bpm=…)`: a pattern, not a scheduler | the sound arm's own thread, on the audio clock | 2 |
| D6–D8 | **MIDI** | `exactMIDI()` (ports), `sendMIDI(output, data, at=)`, `midimessage=`, and MIDI steps in a sequence | a new MIDI arm (CoreMIDI, Web MIDI) | 3 |
| D9 | **Sampler gaps** | `playbackRate=`, `pan=`, sounds decoded on demand, `loadSounds(srcs)` | the sound arm | 2 |
| — | Haptics | already there (LLP 1077 D14); a step can fire one (D3) | — | 2 |
| D10 | **Keep-awake** | `wakelock="screen"` on an element | each host's own call | 1 |
| D11 | **QR** | `qrPath(text, errorCorrectionLevel="M")`, the path for an `svg` | pure, in the runner and the JS target | 1 |
| D12 | **Image share** | `share(files=["#card"])`: an element's picture, shared as a PNG | each host's share sheet | 1 |

**The transport is the main proposal (D2–D5).** Today an app keeps time with
LLP 1096's window clock: a 25 ms timer whose action schedules the next
100 ms of hits. That works while the runner is responsive, but each tick is a
commit on the main thread. It stops when the app is in the background, and it
cannot follow a clock that arrives 48 times a second. So euxy's port ran its
own `DispatchSourceTimer` and broke the native-views rule that time is the
session's (FRICTION.md:124–126). Under this RFC the app **declares what should
play**, as a value computed from state. The host plays it on its own clock,
sample-accurate, with no commit per tick. The runner keeps a record that is a
pure function of that value and its clock, so the driver and tests see every
hit exactly, with no audio.

## 1. Evidence

**What euxy is.** A Euclidean MIDI sequencer for the Teenage Engineering
OP–XY (README). Each pattern is a set of lanes. Each lane is a Euclidean
rhythm with its own length and resolution (1/4 to 1/32), playing one note on
one MIDI channel. Lanes of different lengths run against each other
(polymeter). It sends MIDI clock and notes over CoreMIDI. It can also follow
the device's clock, with a count-in. With no device, it plays built-in 808,
909 and analog kits. It shares a pattern as a link and as a card with a QR code.

**What the exact2 port wrote** (`~/projects/euxy-ports/exact2/euxy/modules/apple/`):

| File | Lines | What it does | What replaces it here |
|---|---|---|---|
| `Engine.swift` | 555 | A two-clocks scheduler: 24 PPQN, 100 ms lookahead, a 25 ms `DispatchSourceTimer` on its own queue. Clock out; following an external clock with a count-in; tempo from a 48-tick window; panic; beat haptics by `asyncAfter`; a 3 s device poll; keep-awake | D2–D5, D7, D10 |
| `MidiManager.swift` | 134 | CoreMIDI on its own run loop: ports, connect, timestamped send, receive stamped with `CACurrentMediaTime` | D6–D8 |
| `Sampler.swift` | 192 | 24 `AVAudioPlayerNode`s round robin, `scheduleBuffer(at:)` at a host time, three kits preloaded | LLP 1096 plus D9 |
| `ShareCard.swift` | 179 | `CIQRCodeGenerator` (the wrong QR version, below), `UIGraphicsImageRenderer`, a hand-presented `UIActivityViewController` | D11, D12 |
| `Euxy.swift` | 547 | The `<euxy-engine>` and `<share-card>` tags; every lane and device list encoded into strings, because a prop is a string and Contract cannot flatten lists (FRICTION §6–7) | mostly D2 (a pattern is a typed value, not a prop string) |

**The porter's notes**, quoted from `FRICTION.md`:

- §9 (124–126): "The `native` time guidance (`context.frames`/`after`, never a
  DispatchSource) does not fit an audio/MIDI scheduler that must keep real time
  while the UI is idle; the engine uses a `DispatchSourceTimer` on its own
  queue, the views use `context.frames`."
- §9 (129–131): "`share(...)` takes text/URLs", so the card's PNG went through
  a hand-written `UIActivityViewController`.
- §1: once an app declared `sound`s, exact opened the output at boot and the
  app crashed on every launch on a busy simulator. That is fixed (LLP 1096
  r5: the output opens at the first voice and survives failing to open). The
  port had already moved to its own sampler.

**What the original needed that is easy to miss:**

- **Polymeter.** Each lane wraps at its own length (`src/core/euclid.ts`
  `laneStepAt`). One loop over the least common multiple of the lane lengths
  can be thousands of steps, so a pattern is a set of tracks, each with its own
  loop.
- **Following a clock.** On Start (0xFA) the engine resets and swallows a
  count-in of `countInBeats × 24` ticks. On Stop (0xFC) after recording it
  ends the take. Each Clock (0xF8) fires its tick at once, with no lookahead.
  Tempo comes from a rolling window of 48 tick timestamps, taken in the native
  read callback. An earlier average taken in JavaScript read 120 BPM as about
  226, because stalls bunched the ticks (`engine.ts:85–100`).
- **Device lists.** CoreMIDI's setup notifications arrive only on the run loop
  that was current when the client was created. Its first call is a
  synchronous XPC that blocks about 600 ms when MIDIServer cold-starts
  (`MidiModule.swift:5–55`). Endpoint refs change on replug. euxy also polls
  every 3 s because notifications were missed on hardware
  (`runtime.ts:162–182`).
- **Keep-awake** is held while playing *or* while waiting for an external
  clock (`engine.ts:174–195`).
- **QR.** The original uses `qrcode` 1.5.4 with a byte segment and level H
  (`share-card.tsx:119–127`). `CIQRCodeGenerator` segments the text itself and
  chose version 14 where `qrcode` chose 15
  (`euxy-ports/swiftui/euxy/FRICTION.md:103–107`). The SwiftUI port ported the
  encoder (228 lines, bit-identical under Bun). The exact2 port's dots differ.

**What exact2 already has**, which the porter's list overstates:

- **Choke groups and per-voice gain** are LLP 1096 D2 and D4: `group=` makes
  a group monophonic by start time, and `gain=` is per voice. euxy's own
  sampler had neither choke groups nor pitch; gain was velocity squared.
- **Haptics** are LLP 1077 D14, on every host that has them:
  `haptic("selection" | "impact-light" … | "success" | "warning" | "error")`
  from an action, and `press-haptic` at touch-down. What euxy also does is a
  haptic on the beat, which belongs to the transport (D3).
- **Timed one-shots** are LLP 1096: `at=` on the runner's clock, mapped to the
  speaker within a few microseconds on Apple (1096 §6, As built).

## 2. Who else would use them

The test for each piece is a second app that is not euxy:

- **Transport:** a metronome that keeps time with the screen locked (a
  `playback` session, D5); a drum machine or step sequencer; a rhythm game
  whose notes must land on the beat however busy the frame is; an
  interval-training timer whose beeps and haptics land on the second; a
  language or music practice app that plays along.
- **MIDI:** any app that a keyboard or pad controller can play or steer (a
  piano tutor, a lighting desk, a slide clicker, a soundboard), and any app
  that drives hardware (euxy, a patch librarian without sysex, a MIDI
  monitor).
- **Sampler gaps:** a chromatic instrument playing one sample at many pitches;
  a game varying a footstep's pitch; stereo placement in a game or an
  instrument; an app whose kits are too many to decode at launch.
- **Keep-awake:** a recipe screen, a presentation timer, turn-by-turn steps,
  a boarding pass, a workout. Expo's survey listed it as cheap with no consumer
  yet (LLP 1069 §5 #10).
- **QR:** tickets, Wi-Fi sharing, pairing a second device, a share card, an
  invite.
- **Image share:** any "share this as a picture" card: a score, a stats
  summary, a quote, a receipt.

## 3. Design

### D1 — Core vocabulary, linked or loaded by use; not extensions

Each piece is part of exact2's own vocabulary, as `sound` and `share` are.
None is a cargo feature, and an app that uses none of them carries none of
their code (LLP 1047):

| Piece | Tier (LLP 1047 D1) | Apple | Web | Linux, Windows | Android |
|---|---|---|---|---|---|
| Transport | loaded, in the sound arm | `libexact_sound` / `ExactSound.framework` gains the scheduler | `sound-glue.js` gains it | the record, no output (as LLP 1096 D9) | — |
| MIDI | loaded, a new arm | `ExactMIDI` target, linking CoreMIDI only when used (LLP 1047.001 D4) | `midi-glue.js`, after paint, only for a plan that uses MIDI | the record; ALSA sequencer deferred | `android.media.midi`, deferred to an Android consumer |
| Sampler gaps | the sound arm | the C mixer | `sound-glue.js` | the record | — |
| Keep-awake | core (one call) | `isIdleTimerDisabled`; macOS `IOPMAssertionCreateWithName` | `navigator.wakeLock` | none (journaled) | `View.setKeepScreenOn` |
| QR | linked by use | the runner's Rust | a JS module loaded with the first plan that uses it | the runner's Rust | the runner's Rust |
| Image share | `Capability::Share` | `UIActivityViewController` / `NSSharingServicePicker` with a PNG | `navigator.share({files})` | refused, `unavailable` | `ACTION_SEND` with `image/png` |

**Why core vocabulary and not LLP 1124 extensions.** An extension is the
right home for a device value that only some apps want and that has no
timing (battery, attitude). These pieces differ in three ways:

- **They share one clock.** A step plays a sample and sends a MIDI note at
  the same instant. That instant must hold whatever the main thread is doing,
  and it is runner time mapped to the output clock (LLP 1096 D6). An extension
  answers through the native slot on the main thread. It cannot reach the
  sound arm's realtime mixer, so the two outputs could not share a scheduler.
- **The agent must see them.** The driver plays no audio and sends no MIDI.
  It reads the record (`state.sounds`, `expect sound`). That record exists
  only if the runner knows what was scheduled.
- **MIDI input is a stream of events that must each arrive.** LLP 1124 §5
  refuses value streams, and its sources are pulls (LLP 1016.002 D1).

If 1124 is accepted, a MIDI device list alone would fit it. Splitting that
list from the sends would give an app two ways to name one port, so this RFC
keeps MIDI whole.

### D2 — `sequence`: a schedule the host plays

`sequence` is a third schedule for a gated task, beside `after` and `every`
(LLP 1092 D7). The gate says *whether* it plays; the arguments say *what*.
The runner and the hosts do the rest:

```
sound "assets/kits/808/kick.wav"
sound "assets/kits/808/hat-closed.wav"
sound "assets/kits/808/hat-open.wav"

shape Step
  beat: number        -- from the track's start, in quarter notes
  src: string         -- a declared sound, or "" for none
  gain: number
  group: string
  note: number        -- a MIDI note 0–127, or -1 for none (D7)
  velocity: number    -- 1–127
  channel: number     -- 0–15
  length: number      -- beats until the note-off
  haptic: string      -- a haptic kind (LLP 1077 D14), or ""

shape Track
  length: number      -- beats; the track loops at this length
  output: string      -- a MIDI output's id for its notes, or ""
  steps: Step[]

component Euxy
  state playing = false
  state bpm = 120
  state take = 0
  task groove when playing key=take
    sequence(tracks(lanes, kit, output), bpm=bpm, from=-4, countIn=clicks(4))
  view
    button (playing ? "Stop" : "Play") press=toggle
```

- **The gate starts it.** The transport arms from the commit that made the
  gate true (LLP 1092 D8). Beat `from` (default 0) falls at that commit's
  time, so the press's own commit is the downbeat. A changed `key=` restarts
  it from `from`. A gate turning false stops it (D4).
- **A track loops at its own length.** `length: 3.5` against `length: 4` is
  polymeter, with no common loop computed. A step outside `[0, length)` is
  dropped and journaled. Steps at one beat play in list order.
- **`from=` below 0 is a count-in.** Beats in `[from, 0)` play `countIn=`, a
  list of steps played once, with beat 0 at the downbeat. Tracks start at 0.
- **Beats, not ticks.** A step's time is in quarter notes, as Ableton Link and
  Tone.js's `Transport` count. A sixteenth is `0.25` and a triplet eighth is
  `1/3`. MIDI's 24 PPQN appears only where MIDI needs it: clock out and clock
  in (D7).
- **One step can do all three.** A sample (LLP 1096's `src`, `gain`, `group`,
  and D9's `playbackRate`, `pan`), a MIDI note on the track's `output`, and a
  haptic. Contract shapes are flat, so "none" is `""` or `-1`, as `group` is
  today. A spec may instead propose optional fields (`option<number>`).
- **One sequence plays at a time.** A second live `sequence` task is refused
  at bake (`analyze-sequence-twice`) when both gates can hold together, and
  journaled at run time otherwise.

### D3 — What the runner keeps, and edits while playing

The runner keeps a **transport record** beside the voice table
(`runner/src/sound.rs`): the arming time, the anchor `(at, beat)`, the tempo
and a digest of the tracks. Beat `b` falls at runner time
`anchor.at + (b − anchor.beat) × 60000 / bpm`. Every hit is a pure function of
this record. The host is given the record and expands it; the runner expands
the same record for the driver (D5).

**The horizon.** A host schedules ahead, so a step due within the next moment
has already been handed to the audio output or CoreMIDI. An edit at commit
time `t` therefore applies to steps at or after **`t + 100 ms`**, on every
host and in the runner's record. Steps before that play as they were. 100 ms
is the window clock's lookahead (LLP 1096 D3), less than a sixteenth at
150 BPM. One constant on every host keeps the record identical across hosts.

- **New tracks** (a lane muted, a step toggled, a kit changed) replace the old
  from `t + 100`. The beat continues; nothing restarts.
- **A new `bpm`** re-anchors at `t + 100`: `anchor.beat` becomes the beat
  then, and `anchor.at` becomes `t + 100`. Tempo moves without a jump in phase.
- **A stop is now** (D4), not at the horizon.

**`exactTransport()`** is a reserved source (LLP 1069.000's shape):

```
resource transport = exactTransport() as shape Transport
-- { state: "stopped" | "waiting" | "countin" | "playing",
--   bpm: number, anchorAt: number, anchorBeat: number, clock: "internal" | "midi" }
```

A view that shows the playhead computes the beat from it, in a frame task
gated on visibility. A frame commits only while the screen shows the
playhead. Audio never depends on those commits:

```
task playhead when transport.state == "playing" and page.visibilityState == "visible"
  every(frame, sample)
action sample
  shownAt = performanceNow()
fn beatAt(t: Transport, ms: number): number
  t.anchorBeat + (ms - t.anchorAt) * t.bpm / 60000
```

**A step's haptic** plays at its time on the main thread: `asyncAfter` to the
step's host time on Apple, as euxy's port does. UIKit's feedback generators
cannot be scheduled ahead. A few milliseconds of main-thread lateness is below
what a hand feels. The web plays it through `navigator.vibrate` where it
exists, as `haptic` does.

### D4 — Stopping, the session, and the background

- **The gate turning false stops it at that commit's time.** Voices the
  transport started are ended, as `stopSounds()` ends them (LLP 1096 D4,
  `by: stop`). Every MIDI note it turned on gets its note-off, and each
  channel it used gets CC 123 (All Notes Off), as euxy's `panic()` does. Clock
  out sends Stop (0xFC).
- **`stopSounds()` also stops the transport's voices**, but not the transport.
  The next step plays.
- **The audio session is LLP 1096 D8's**: one owner, `ambient` by default,
  `playback` from `app.json`. A `playback` app with `audio` in
  `host.ios.backgroundModes` keeps playing with the screen locked. The
  scheduler is a host thread, not the runner, so it keeps running while the
  runner sleeps. This is the metronome case. An `ambient` app is suspended
  with its process, and the transport resumes on the beat it would have
  reached. It does not catch up: missed steps are not played late (LLP 1073
  D2's rule for frames).

### D5 — The hosts, and the driver

**Apple.** The sound arm gains one scheduler thread (QoS
`userInteractive`, a 10 ms wake). Each wake expands the steps that fall in the
next 100 ms. It pushes their voices into the mixer's existing SPSC ring at
frame-accurate starts, which the render callback already places (LLP 1096 D6).
It hands their MIDI to the MIDI arm with host-time stamps (D7), and their
haptics to the main queue. The thread is the host's, inside a host arm. LLP
1029.000 §4's ban is on threads an app's module owns. The native-views recipe
keeps its rule for views and gains one line: timed output goes through the
transport.

**Web.** The glue runs the same expansion. Its wake comes from a dedicated
Worker's `setInterval`, posting to the page, because the browser throttles a
hidden page's own timers to about once a second. Steps become
`AudioBufferSourceNode.start(when)` through D6's mapping, and
`MIDIOutput.send(data, timestamp)` with `performance.now()` stamps. A
main-thread stall longer than the 100 ms window makes those steps late. An
AudioWorklet mixer would remove that limit for sound but not for MIDI, since
Web MIDI is main-thread only. That mixer is deferred until a measurement shows
late steps (§9).

**The driver.** Nothing plays and nothing is sent. The runner expands the
record as its clock advances. Each sample step becomes a voice in
`state.sounds` (LLP 1096 D10), so `expect sound has "assets/kits/808/kick.wav"
at 500` works unchanged. Each MIDI step becomes a line in `state.midi.sent`.
Steps are not timers: `clock settle` never waits on one, and one
`advance(60_000)` gives the same record as sixty `advance(1_000)`s (LLP 1092
D10). The record keeps its last 1,024 entries.

### D6 — `exactMIDI()`: the ports, by Web MIDI's names

```
resource midi = exactMIDI() as shape MIDI
-- { state: "prompt" | "granted" | "denied" | "unavailable",
--   inputs: MIDIPort[], outputs: MIDIPort[] }
-- MIDIPort: { id, name, manufacturer, state: "connected" | "disconnected" }
```

- **Field names are Web MIDI's** (`MIDIAccess.inputs`, `MIDIPort.id`, `.name`,
  `.manufacturer`, `.state`).
- **`id` is stable across replug.** On Apple it is the endpoint's
  `kMIDIPropertyUniqueID`, not the endpoint ref, which changes on replug (§1).
  Chrome's ids are stable per device.
- **It changes by itself.** The host announces it, and the runner asks again
  (LLP 1016.002). On Apple the MIDI arm creates its client on its own thread,
  with its own run loop, so setup notifications arrive and the cold XPC never
  blocks the main thread. It re-enumerates when the app returns to the
  foreground, and on a 3 s watchdog while a port is in use. The watchdog stays
  because euxy saw notifications missed on hardware. It lives in the arm, so no
  app writes it again. The web listens to `MIDIAccess.onstatechange`.
- **Permission.** On the web, `requestMIDIAccess()` asks. It is a host command
  an action calls, and its outcome moves `state`. Apple asks for nothing, so
  `state` is `granted` once the arm has loaded. The grant line is
  `device.midi <strings-key>` (LLP 1069.008 D1: `midi` is the Permissions API
  name). On the web it maps to `Permissions-Policy: midi`; Apple has no row.
  SysEx (`{sysex: true}`, a separate prompt) is deferred.
- **Where there is none**, `state` is `unavailable`. WebKit, and so every iOS
  browser, has no Web MIDI. Firefox gates it behind a site-permission add-on.

### D7 — Sending, clock out, clock in

- **`sendMIDI(output, data, at=)`** is a host command, named like LLP 1096's
  `playSound`. `output` is a port id and `data` is a `number[]` of MIDI 1.0
  bytes, as `MIDIOutput.send(data, timestamp)` takes. `at=` is runner time
  (default: the commit's), mapped to `MIDITimeStamp` (host ticks) on Apple and
  to `performance.now()` on the web, as LLP 1096 D6 maps voices. A message
  that is not well-formed MIDI 1.0 (a missing data byte, a SysEx) is dropped
  and journaled. The commit stands. On Apple the bytes go out as a MIDI 1.0
  `MIDIEventList` (`MIDISendEventList`, the API that replaces packet lists).
- **Notes from a sequence** go to the track's `output`: a note-on at the
  step's time and a note-off `length` beats later. A note still sounding when
  the transport stops gets its note-off then (D4).
- **`clockOut=<output id>`** on `sequence` sends Timing Clock (0xF8) 24 times
  per beat, stamped ahead like the notes. It sends Start (0xFA) at beat 0, and
  Continue (0xFB) with a Song Position Pointer when a `key=` restart resumes
  rather than restarts (question 4). It sends Stop (0xFC) at the stop.
- **`clock=<input id>`** follows that input instead of `bpm=`:
  - The transport is `waiting` until the input sends Start (0xFA). Start puts
    beat `from` at that message's timestamp, as euxy swallows its count-in.
  - Each Clock (0xF8) moves the position by 1/24 of a beat. The arm fires a
    step due at a tick at once, with no lookahead, because the device decides
    when the tick comes.
  - The tempo comes from the arm's 48-tick window, taken from CoreMIDI's own
    timestamps (§1). The runner's anchor is updated when the tempo moves by
    0.5 BPM or more, at most every 500 ms (euxy's thresholds). That is a few
    commits a second, never 48.
  - Stop (0xFC) moves `state` to `stopped`. Whether a stop ends a take is the
    app's rule: it hears Stop as a `midimessage` (D8) and sets its own state.
  - Continue resumes. Song Position is honoured, though euxy ignores it.

### D8 — Receiving: `midimessage`

`midimessage` is an event, named as Web MIDI's `MIDIInput` fires it. Every
element that declares it hears it, as `beforeunload` is heard
(`docs/contract-grammar.md:1164`):

```
view
  column midimessage=heard
    ...
action heard(e: MIDIMessageEvent)
  -- e.input: string, e.data: number[], e.timeStamp: number (runner ms), e.beat: number
```

- **Inputs open while a handler is mounted.** Every connected input is open
  then (`MIDIPortConnectSource`; the web's `onmidimessage`). An app filters on
  `e.input`.
- **Each message is one dispatch, in order.** None is coalesced, because a
  note-off must never be lost. Messages that arrive between commits are
  dispatched one after another. A burst is bounded at 256 pending, and the
  257th is dropped and journaled: a controller's knob sweep can exceed what a
  commit per message keeps up with.
- **Clock and Active Sensing (0xF8, 0xFE) are not dispatched.** They feed
  `clock=` (D7) and would be 48 or more commits a second. Start, Continue and
  Stop are dispatched.
- **`timeStamp` is the arm's**, taken in CoreMIDI's read callback (or Web
  MIDI's `event.timeStamp`) and moved to runner time. It is not the commit's
  time. **`e.beat`** is the transport's beat at that time, or `-1` when it is
  stopped. A recorder quantises with a `fn` of `e.beat`, with no clock math.

### D9 — The sampler gaps

LLP 1096 has groups, gain and timed one-shots. The rest of what instruments
and games ask:

- **`playbackRate=`** (Web Audio's `AudioBufferSourceNode.playbackRate`),
  0.25–4, default 1. Pitch moves with it, as on a tape or a hardware sampler. A
  semitone is `2^(n/12)`. The web sets the node's parameter. The C mixer reads
  at a fractional position with linear interpolation; cubic is a spec
  question. The voice's end becomes `start + duration / rate`, so groups and
  `state.sounds` stay exact.
- **`pan=`** (Web Audio's `StereoPannerNode.pan`), −1 to 1, default 0. Its law
  is the spec's equal-power law, which the mixer copies.
- **Both are fields a hit may carry**, after `src`, `at`, `gain` and `group`
  in `playSounds`'s shape, and in a sequence's `Step`. A shape without them
  plays at rate 1, centred.
- **Decoding on demand.** Today every declared sound is decoded when the
  output opens (LLP 1096 D7, D8). Three 24-slot kits would mean 72 decodes
  before the first hit. Instead:
  - A sound is decoded at its first use.
  - `loadSounds(srcs)` decodes a list ahead, which is what euxy does when a kit
    is chosen. It runs off the main thread on Apple, and through
    `decodeAudioData` on the web.
  - `state.sounds` reports each sound's `loaded` state.
  - A voice whose sound is still decoding behaves as LLP 1096 D7 says: it
    starts when decoded, or is dropped at its end.
  - Whether the first `sound` declarations stay eager below some total size is
    question 6.

### D10 — Keep-awake: `wakelock="screen"`

```
column wakelock=(playing or waitingForClock ? "screen" : "none")
```

While any mounted element says `wakelock="screen"`, the display does not dim
or lock for idleness. The value is the Screen Wake Lock API's one type
(`navigator.wakeLock.request("screen")`). An attribute fits better than a pair
of commands, for two reasons:

- **It cannot leak.** A screen that unmounts releases its lock, as a recipe
  screen should. With commands, a forgotten release keeps the phone awake.
- **It is the platform's own shape.** Android's is a view attribute,
  `View.setKeepScreenOn` (`android:keepScreenOn`). A video's
  `preventsDisplaySleepDuringVideoPlayback` is already per element in exact2
  (LLP 1042).

Per host:

- **iOS:** `UIApplication.isIdleTimerDisabled`, set while any element holds
  it.
- **macOS:** `IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep)`,
  released with the last holder.
- **Web:** `navigator.wakeLock.request("screen")`. The browser releases it
  when the page is hidden, so the glue requests it again on `visibilitychange`
  (the spec's own example). Where the API is missing it is journaled once.
- **Android:** `setKeepScreenOn` on the root view.
- **Linux:** journaled as unavailable.
- **The driver:** `state.wakelock` is `{held, by: [node ids]}`. The iOS agent
  keeps disabling the idle timer for its own reasons (`AgentIOS.swift:24–26`),
  separately.

### D11 — QR: `qrPath(text, …)`, matching `qrcode`

`qrPath(text, errorCorrectionLevel="M", margin=4)` is a pure built-in
function, like `format`. It returns an SVG path in module units, one square
per dark module, and `qrSize(...)`, with the same arguments, returns the side
in modules, margin included:

```
svg viewBox=("0 0 " + qrSize(link, errorCorrectionLevel="H") + " " + qrSize(link, errorCorrectionLevel="H"))
  path d=qrPath(link, errorCorrectionLevel="H")
```

- **The output is `qrcode` 1.5.4's.** For a byte segment and a level, it
  matches `QRCode.create([{data, mode: "byte"}], {errorCorrectionLevel})`
  bit for bit: the same smallest version, the same mask by ISO/IEC 18004's
  penalty, and the same modules. Byte mode is always used, so the text is
  never segmented differently, which is where Core Image differs (§1). The
  argument names are `qrcode`'s. A conformance corpus checks the Rust
  encoder against `qrcode` under Bun on every change to either.
- **Level H with a covered centre** is how euxy draws its glyph in the middle.
  The app paints a plate over the centre. Level H recovers up to about 30 %
  of the codewords, which is why such codes still scan.
- **Rounded or spaced dots** (euxy's 84 % rounded rects) are drawing, not
  encoding. `qrPath` could take a `module` style (question 7). Without it the
  app can still lay out the dots as `rect`s from a module list, at the cost of
  one node per module.
- **Where it runs.** The runner's Rust, linked when the plan uses it. On the
  JS target it is a module loaded with the first plan that uses it, compiled
  from the same Rust or written as a port held to the same corpus (question
  7). LLP 1027.000's purity rules hold: same arguments, same path.

### D12 — Sharing an image: `share(files=[…])`, and an element as a file

LLP 1069.003 D7 already designs `files=` as a later slice: `app:/` paths
behind `fs.read`, each turned into the host's own type. This RFC brings that
slice in, with one addition: **an entry `"#id"` names an element, and the
host shares a PNG of it as it is drawn now.**

```
action shareCard
  share(title=pattern.name, url=link, files=["#card"])
view
  column id="card" ...   -- the card, drawn as the app draws anything else
```

- **The picture is the host's own drawing**, at the display's scale. Apple
  renders the element's layer (`UIGraphicsImageRenderer`, as euxy's port
  does). Android draws the view into a bitmap. The web serialises the
  element's subtree into an SVG `foreignObject` and draws it to a canvas.
  Cross-origin images in it are left out and journaled, because they would
  taint the canvas. The Linux CPU painter could serve a Linux share later.
- **Nothing is written that the app can read**, so a picture needs no
  `fs.read` grant. Only an `app:/` path does. The PNG lives in the host's
  temporary space until the sheet closes.
- **The web needs `navigator.canShare({files})`.** Desktop Chrome and Firefox
  share no files. `exactPage().canShare` becomes `canShareFiles` as well
  (question 8). The browser still asks for a user gesture.
- **Not an image API.** There is no `toBlob` and no image the app can read
  back. A picture the app keeps or uploads is LLP 1112's territory, which stays
  parked.

## 4. The agent and tests

| Piece | `state` | Drive | Test step |
|---|---|---|---|
| Transport | `state.transport` (the record, D3), and its voices in `state.sounds` | `clock +N` moves it; nothing plays | `expect sound … at N` (unchanged); `expect midi sent "<id>" 90 3c 64 at N` |
| MIDI ports | `state.midi.inputs/outputs` | `prefer midi.outputs=[…]` (LLP 1069.007 D3's `prefer`, a new name) | — |
| MIDI in | `state.midi.received` | `tap midi "<input>" 90 3c 64`, as LLP 1098 addressed the media session with `tap … mediasession` | the same, in a test |
| Clock in | `state.transport.clock` | `tap midi "<input>" fa`, then `clock +N` ticks it at the `prefer`red tempo | — |
| Keep-awake | `state.wakelock` | — | `expect wakelock held` |
| QR | — (a pure value) | — | `expect` on the path, as for any value |
| Image share | a held `share` ticket (as today), with `files` | `tap @N shared` | as LLP 1069.003 |

**MIDI input against LLP 1069.007.** That LLP says a stream "of events that
must each arrive" does not fit its rule yet. This RFC proposes the rule for
this one stream. An input message is a `dispatch` (LLP 1069.009 D1). The
driver's `tap midi` makes exactly that call with the arm absent, and a
recorded tape replays it. No eleventh operation is added.

## 5. Effect on each implementation

- **Contract compiler:**
  - `sequence` as a schedule (the task grammar, `docs/contract-grammar.md:225`);
  - `Step` and `Track` checked structurally, as `playSounds`'s hits are;
  - `sendMIDI`, `requestMIDIAccess` and `loadSounds` join `HOST_COMMANDS`
    (`contract/syntax/src/lib.rs:87`);
  - `midimessage` joins the event table;
  - `wakelock` joins the attributes;
  - `qrPath` and `qrSize` join the roster;
  - `share` gains `files=`.
- **Plan:** a `sequences` table; use bits for MIDI, wake lock and QR
  (`runner/src/uses.rs`).
- **Runner:** the transport record and its expansion for the driver; two
  reserved sources; the MIDI record; dispatch of `midimessage` with its bound.
  `runner/src/sound.rs` is 394 lines, so the transport is a new
  `runner/src/transport.rs`.
- **Lean and difftest:** the commands and the event only, as for LLP 1096
  (D11 there). The transport's expansion is a pure function, which difftest
  can compare between the runner and the JS target.
- **Apple:**
  - the scheduler in `soundarm/`;
  - rate and pan in `sound_render.c`;
  - a new `ExactMIDI` target;
  - the wake lock in `Session.swift`;
  - the PNG in `Share.swift`.
- **Web:** the scheduler and its Worker in `sound-glue.js`; `midi-glue.js`;
  the wake lock and file share in the glue; the QR module.
- **Linux, Windows:** the record. Keep-awake and image share are refused by
  name.

## 6. Considered, not taken

- **Keep the window clock in Contract** (LLP 1096 D3). It stays for a few
  hits. As a transport it costs a commit every 25 ms on the main thread. It
  stops in the background and is late when the runner is. It cannot follow a
  clock at 24 PPQN.
- **A scheduler in a data module on a worker** (LLP 1027.002). Data modules
  refuse timers by design (LLP 1027.000 D3), and the module would still reach
  the outputs only through commits.
- **Host extensions (LLP 1124)** and **each app's own module (LLP 1067.000)**,
  which is today's state: see D1. Each app writes CoreMIDI again, and the
  agent can drive none of it.
- **Imperative transport commands** (`startTransport`, `setPattern`,
  `stopTransport`). They need a handle, and a forgotten stop keeps playing.
  A gated task is already how exact2 says "while".
- **A pattern in ticks.** The author would multiply by 24 everywhere. Beats
  are what musicians and Link count.
- **One loop over the least common multiple** of the track lengths. See
  polymeter in §1.
- **Core Image's QR.** It segments the text itself and its version differs
  from the web's most common library (§1).
- **`keepAwake()` / `allowSleep()` commands.** They leak (D10).
- **An AudioWorklet mixer now.** No measurement shows the main-thread
  lookahead missing yet (§9).

## 7. Stages

Each stage stands alone and has a consumer. Euxy's exact2 port is
re-written against each one, deleting the Swift that the stage replaces.

1. **The small pieces** (each a day or two; independent):
   - `wakelock` (D10);
   - `qrPath` with its corpus against `qrcode` (D11);
   - `share(files=)` with element pictures (D12).
   The port deletes `ShareCard.swift` and the idle-timer lines in
   `Engine.swift`.
2. **The transport and the sampler gaps** (D2–D5, D9): the runner record, the
   Apple scheduler, the web scheduler, `playbackRate`, `pan` and decoding on
   demand. The port's sounds and beat haptics move to `sequence`, and it
   deletes `Sampler.swift` and the scheduler in `Engine.swift`.
3. **MIDI** (D6–D8): the arm, ports, sends, clock out, clock in and
   `midimessage`. The port deletes `MidiManager.swift` and the rest of
   `Engine.swift`. `Euxy.swift` keeps only what its views draw.

The bar for stage 2 is LLP 1096 §6's device check, applied to a sequence:
onsets on the grid within the mixer's own precision on a Mac and an iPhone,
with the main thread blocked for 80 ms by a test action. The bar for stage 3
is euxy against a real OP–XY: clock out, notes, and following its clock
through a count-in and a take, as `README.md`'s demo shows.

## 8. `rules/DEFERRED.md`

This admits a transport, MIDI and four small pieces. Charlie's 2026-10-15
instruction ("keep making improvements to exact2 until porting euxy … is
higher fidelity than the others", recorded in `DEFERRED.md` with no take)
covers making the euxy port faithful. It does not obviously cover a general
MIDI capability. Question 1 asks.

**Still out after this RFC:**

- SysEx;
- Bluetooth MIDI pairing UI (`CABTMIDICentralViewController`);
- MIDI 2.0 messages;
- Ableton Link;
- swing and groove templates;
- Linux, Windows and Android output;
- synthesis;
- compressed sound formats;
- a picture the app keeps;
- Core Haptics patterns (euxy's charge ramp).

## 9. Questions for Charlie

1. **Admission and take.** Background: `DEFERRED.md` admits a capability only
   with a reason and something taken off, or a waiver. The euxy instruction
   is close to a waiver but is about fidelity, not a capability every app
   gets. **Recommendation:** admit stages 1 and 2 under the euxy instruction,
   since each is what the port lacks and each has the second consumers in §2.
   Rule on MIDI (stage 3) separately, with Linux MIDI as the stated
   non-goal. · **Confidence:** medium, ~65%. · **Why not higher:** whether the
   instruction covers MIDI is yours to say. If it does, admit all three
   stages.
2. **A schedule on `task`, or a new declaration.** Background: a gated task
   already means "while this holds, keep doing this", and the transport needs
   exactly that: arm from the commit, re-arm on a key, stop at the gate. A
   separate `transport` declaration would be a second way to say it.
   **Recommendation:** `sequence` as a third schedule (D2). · **Confidence:**
   high, ~80%. · **Why not higher:** a task's other schedules call an action;
   this one calls none. If that difference reads as a different thing, a
   declaration named `sequence` with the same gate grammar is the fallback.
3. **The 100 ms horizon for edits** (D3). Background: an edit cannot reach a
   step already handed to the speaker or CoreMIDI. Fixing the horizon at one
   constant keeps the record the same on every host; a host-chosen horizon
   would let Apple use 30 ms but make `state.sounds` differ by host.
   **Recommendation:** 100 ms everywhere. · **Confidence:** medium-high,
   ~70%. · **Why not higher:** a live-performance app might feel 100 ms on
   a mute. Then the next step is 50 ms on Apple only, with the driver
   simulating Apple's.
4. **`key=` re-arm: restart or continue.** Background: today a changed key
   re-arms from scratch. For clock out, a resume is Continue plus Song
   Position, which a drum machine follows; a restart is Start. **Recommendation:**
   a key change restarts (Start); a separate `resume` (the gate turning true
   with `from=` set to the old beat) sends Continue. · **Confidence:** medium,
   ~60%. · **Why not higher:** euxy has "Pause/Continue" and "Reset", and I
   have not confirmed which the OP–XY expects after Pause.
5. **One flat `Step`, or kinds** (D2). Background: Contract shapes are flat,
   so a step that may be a sample, a note or a haptic carries all of them,
   with `""` or `-1` for none. Optional fields would read better but cost the
   checker work. **Recommendation:** flat now, as `playSounds` is.
   · **Confidence:** medium, ~60%. · **Why not higher:** if `option<…>` fields are cheap to check structurally, a spec should use them.
6. **Decoding: on demand always, or eager below a size** (D9). Background:
   eager decoding makes the first hit instant; on-demand keeps launch and
   memory flat for big libraries. `loadSounds` covers the instant case.
   **Recommendation:** on demand always, with `loadSounds`.
   · **Confidence:** medium-high, ~70%. · **Why not higher:** a game with
   ten short sounds would need to call `loadSounds` at mount, which is a new
   thing to forget.
7. **QR's scope** (D11). Background: matching `qrcode` means one encoder
   held to a corpus. Drawing styles (rounded, spaced dots) are what card
   designers ask next. **Recommendation:** square modules only. Rust in the
   runner, and the same Rust compiled into the JS target's module rather than a
   second port. · **Confidence:** medium, ~65%. · **Why not higher:** the JS
   target's runtime budget is ~20 KB, and a compiled encoder may not fit as a
   loaded module. A hand port with the corpus is then cheaper.
8. **`canShareFiles`** (D12). Background: desktop browsers share text but not
   files, so a share-card button must know. **Recommendation:** add it to
   `exactPage()`. · **Confidence:** high, ~85%.
9. **The web's MIDI reach.** Background: no WebKit browser has Web MIDI, so
   euxy on an iPhone's Safari has none. Native iOS has CoreMIDI.
   **Recommendation:** accept it as a declared limit: `state: "unavailable"`,
   with the app hiding its MIDI UI. · **Confidence:** high, ~90%.
