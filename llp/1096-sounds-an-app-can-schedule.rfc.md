# LLP 1096: Sounds an app can schedule

**Type:** RFC
**Status:** Draft (r1), for review. Admission is the orchestrator's under Charlie's 2026-10-04 delegation (§9 Q1).
**Systems:** Contract compiler (a `sound` declaration; two host commands; `contract/lower/src/sounds.rs`; the test step `expect sound`), Plan (`sounds` table), Runner (new `runner/src/sound.rs`, `take_sounds`, `state.sounds`), JS target (new `host/web-js/sounds.js`), web output shared by both targets (new `host/web/sound-glue.js`), Apple (new `host/apple/soundarm/SoundArm.swift`, `SoundModule.swift`), Linux and Windows (record only), conformance, Lean and difftest (commands only), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-06, stage 2 on 2026-10-07, stage 3 on 2026-10-08–09, stage 4 on 2026-10-09 (§6)
**Amends:** LLP 1042 §8 (a sound effect is no longer an `audio` element's job); `docs/contract-for-agents.md`'s "Ding" recipe; `rules/DEFERRED.md` (D12)
**Related:** LLP 1005 §3 (host commands); LLP 1012 (the agent); LLP 1019 (a declared asset: fonts); LLP 1046.003 AU3 (the game add-on's audio, which stays the game's); LLP 1051 §5.3 (`start(when)` noted); LLP 1070.000 (a command the runner keeps for itself); LLP 1077 D14 (`haptic`, the command this follows); LLP 1092 D7 (gated tasks, which the lookahead clock uses). Diaries: `~/projects/x2apps/drums/DIARY.md` (R1–R3, Top 5 #1, `repros/R1-retrigger`), `snake/DIARY.md` (F1), `trivia/DIARY.md` (F5). External, read 2026-10-04: Web Audio API 1.0 (`AudioBufferSourceNode`, `AudioScheduledSourceNode.start(when)`/`stop(when)`, `GainNode`, `AudioContext.getOutputTimestamp()`); the W3C Audio Session draft (`navigator.audioSession.type`); AVFAudio's `AVAudioEngine`, `AVAudioSourceNode`, `AVAudioPlayerNode.scheduleBuffer(_:at:)`.

## Summary

Exact has one way to make a sound: an `audio` element, which is HTML's media
player. A player is the wrong tool for a drum hit, a click or a game's beep:

- **It cannot retrigger.** A second hit does nothing while `paused` stays
  `false` (drums R1). Drums keeps two elements per track and alternates them.
- **It cannot be scheduled.** A hit starts when a commit lands. The onset
  carries the timer's lateness, the commit and the element's start-up (R3).
- **It needs a polling clock.** Drums ticks every 2 ms, even while stopped (R2).

This RFC adds Web Audio's model in its smallest form: decoded buffers, a
one-shot voice per play, a start time and a gain.

```
sound "assets/kit/kick.wav"
sound "assets/kit/hat.wav"

component Drums
  task clock when playing                 -- LLP 1092 D7
    every(25, tick)
  action tick                             -- schedule what falls due in the next 100 ms
    if nextAt < now() + 100
      playSound("assets/kit/kick.wav", at=nextAt, gain=0.8, group="kick")
      nextAt = nextAt + stepMs(song.tempo)
  action stop
    playing = false
    stopSounds()
  action click
    playSound("assets/kit/hat.wav")       -- now, retriggered on every press
```

- **`sound`** declares a file, as `font` does. The compiler reads its header
  and refuses what no host can play.
- **`playSound`** starts a new voice every time it is called. It takes a time
  on the runner's clock (`now()`'s milliseconds), a gain and a group.
- **`group`** makes a group's voices monophonic, as a drum machine's choke
  group is.
- **`stopSounds`** ends the voices that are sounding and cancels the ones
  still waiting.

The runner keeps the voice table, so what was scheduled, when, is the same on
every host and under the driver's clock. The web and Apple play it: Web
Audio's `start(when)`, and an `AVAudioSourceNode` mixer on `AVAudioEngine`.
Linux and Windows keep the record and play nothing.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `sound "path"`: a declared WAV, read by the compiler | snake F1 | 1 |
| D2 | `playSound(src, at=, gain=, group=)`: one new voice per call | drums R1 | 1 |
| D3 | `at` is runner time; the past means now | drums R3 | 1 |
| D4 | `group=`: monophonic by start time; `stopSounds(group=)` | drums (hat choke, stop) | 1 |
| D5 | The runner's voice table, its bound, its journal and `take_sounds` | — | 1 |
| D6 | Runner time to the speaker, on each host | drums R3 | 2, 3 |
| D7 | The web: one `sound-glue.js` for both targets; activation | trivia F5 | 2 |
| D8 | Apple: a sound arm, `AVAudioEngine` and one source-node mixer | drums | 3 |
| D9 | Linux and Windows: the record, no output | — | 1 |
| D10 | The driver: no output, `state.sounds`, `expect sound` | drums R7, R8 | 1 |
| D11 | Lean and difftest | — | 1 |
| D12 | `rules/DEFERRED.md` | — | 1 |
| D13 | Docs and adoption | drums, snake, trivia | 2–4 |

## 1. Evidence

- **Drums R1 (retrigger).** `media-glue.js`'s `syncPlayback` calls `play()`
  only when the bound `paused` changes against its own last-applied value. It
  never learns that the element ended by itself. A timer's hit is one commit,
  and flipping `paused` back and forth takes two. The repro
  (`drums/repros/R1-retrigger`) made 5 `play()` calls in 4 s where 29 were
  due. The app now carries 16 hidden `audio` elements for 8 tracks
  (`app.contract:418–419`, `shapes.contract:112` `elementOn`).
- **Drums R2 (the clock).** `every(2, tick)` fires forever, playing or not:
  6.9 % CPU on macOS at rest, against 2.5 % at 1000 ms. LLP 1092 D7's gated
  task stops it at rest. A 2 ms poll is still needed while playing, because
  a hit can only start when its commit lands.
- **Drums R3 (timing).** In Chrome, Firefox and WebKit, `play()` lands a
  median 3–4 ms after the grid and at most about 10 ms after it. On macOS,
  hit to `playing` averaged 24–31 ms under the agent (max 56–114). "Nothing
  can be scheduled ahead of time." The playhead never drifts, because steps
  are planned from `now()`. What drifts is each onset.
- **Snake F1.** Snake has a sound toggle and no way to make a sound. Its
  settings read "On · silent build, no audio output".
- **Trivia F5.** A remounted 1×1 `video` playing a generated mp4 stayed
  `paused: true`. Its `play()` came outside the gesture and was dropped with
  no error. The app falls back to `haptic`.
- **Today's code.**
  - `audio` is node type 12, `Video` with `semanticTag` audio
    (`kernel/tables/schema.json:54–58`; `contract/lower/src/media.rs:49–54`).
  - Its players are the web's `<audio>` and Apple's AVPlayer, in an arm
    loaded on demand (`host/apple/Sources/ExactKit/VideoModule.swift:27–31`).
  - Linux reports it unavailable (`host/linux/src/agent.rs:197–209`).
  - Host commands are names in `HOST_COMMANDS`
    (`contract/types/src/checks.rs:740–776`), lowered to `Opcode::Command`
    and journaled after the commit stands (`runner/src/runner/commit.rs:616–651`).
- **The game add-on** (`game/audio`, LLP 1046.003 AU3) has a Web Audio
  output, an Apple AudioUnit output and a WASAPI output. It starts voices
  with `start(0, offset)` on tick boundaries and never at a future `when`
  (`game/audio/src/web.rs:125`). No core crate may depend on it
  (`rules/DEFERRED.md`, the game entry).

## 2. What the web and the platforms offer

- **Web Audio.**
  - An `AudioBuffer` is decoded PCM (`decodeAudioData`).
  - An `AudioBufferSourceNode` plays one buffer once. It is cheap and made per
    play: "a source node can only be started once". Retrigger is therefore a
    new node.
  - `start(when)` and `stop(when)` take the context's clock in seconds. A
    `when` in the past starts at once.
  - `GainNode.gain` is a linear multiplier.
  - `getOutputTimestamp()` maps the context's clock to `performance.now()` at
    the speaker.
  - The context starts `suspended` until a user activation, and `resume()`
    inside a gesture runs it.
  - The usual pattern ("A Tale of Two Clocks") is a coarse timer that
    schedules every note falling in the next window. The audio clock does the
    precise work.
- **Audio Session** (W3C draft, Safari 16.4+). `navigator.audioSession.type`
  is `auto`, `playback`, `ambient` and so on. Under `auto`, Web Audio is
  ambient: it mixes with other apps, and iOS's ring/silent switch mutes it.
- **AVFoundation.**
  - `AVAudioPlayerNode.scheduleBuffer(_:at:)` starts a buffer at an
    `AVAudioTime`, which can be host time. A node plays its buffers one after
    another, so overlapping voices need a pool of nodes. There is no stop at a
    future time, so a choke needs a timer.
  - `AVAudioSourceNode` (macOS 10.15, iOS 13) calls a render block with the
    host time of each buffer's first frame. A mixer in that block places every
    start and stop at the sample, with one node.
- **Linux.** Output means ALSA or PipeWire, which are system libraries the
  pure-Rust host does not link (LLP 1000; `host/linux/Cargo.toml`).

## 3. Decisions

### D1 — `sound "path"`: a declared WAV

```ebnf
sound = "sound" STRING NL ;
```

`sound` is a top-level declaration, a keyword only at the start of a
top-level line, as `font` is (`parser.rs:346`). `parser.rs` is 1,450 lines,
so it gains only that dispatch arm; the declaration's parser is the new
`parser/sounds.rs`. So `state sound = true` (snake) still parses.
The path follows `font`'s rules (`contract/lower/src/fonts.rs:100–147`): it is
under the app's `assets/`, stays inside the app directory, and must be
readable. A module's `sound` lines join its user's.

The new `contract/lower/src/sounds.rs` reads the RIFF header, with no new
dependency, and refuses with the file named:

- **`lower-sound-format`**: anything other than `WAVE` `fmt ` format 1
  (16-bit integer PCM) or format 3 (32-bit float), 1 or 2 channels, at
  8–96 kHz. The message gives the conversion: `afconvert -f WAVE -d LEI16
  in.m4a out.wav`, or `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`.
- **`lower-sound-long`**: longer than 10 s. "A sound is a short clip; play
  longer audio with `audio`."
- **`lower-sound-unreadable`** and **`lower-sound-path`**: as for fonts.

Plan `sounds` gains a row per file: `src` (the path), `frames`, `rate`,
`channels` and a SHA-256 digest. `FORMAT_DIGEST` changes. The runner reads
durations from it (D5). The hosts read the list of files to decode from it
(D7, D8). The files themselves are already in every bundle: the web build
copies `assets/` whole (`host/web/build.mjs:143–147`,
`host/web-js/build.mjs:480`), and so does Apple's
(`host/apple/build.mjs:243–246`).

### D2 — `playSound(src, at=, gain=, group=)`

`playSound` and `stopSounds` join `HOST_COMMANDS`. `command_args`
(`contract/lower/src/expr.rs:15`) gives them fixed positions, as it does for
`scrollIntoView`: `playSound` is `src, at, gain, group`, with `none` for an
omitted argument, and `stopSounds` is `group`.

- **`src`** is a string. A literal that no `sound` declares is refused
  (`type-sound-undeclared`, which names the declared sounds nearest it). A
  computed one is checked by the runner (D5). Drums chooses a track's sound
  from data (`v.file`).
- **`at=`** is a number, in milliseconds on the runner's clock (D3). It
  defaults to the commit's time.
- **`gain=`** is a number, a linear multiplier, clamped to 0–1 as CSS clamps
  `opacity` and as `volume`'s range is (`media.rs:27–42`). It defaults to 1.
  A literal outside 0–1 is refused (`type-play-sound`), as a literal `volume`
  is (`lower-attr-value`, `media.rs:27–42`).
- **`group=`** is a string (D4). With none given, the voice is in no group.

Each call is a new voice: Web Audio's one-shot source, so a retrigger is
simply another call. `playSound` returns nothing and has no handle. Groups do
the stopping (D4). Other options are refused by name (`type-play-sound`),
with the usage in the message.

### D3 — `at` is runner time; the past means now

`at` is read on the clock `now()` reads, so an app schedules with the
arithmetic it already uses for its timers. In a timer's commit, `now()` is the
due time, not the wall time it ran (LLP 1092 D8; `rt.js` `advance`). A step
planned as `nextAt` therefore lands on the grid whatever the timer's
lateness.

- **The effective start** is `max(at, t)`, where `t` is the commit's time.
  Web Audio starts a past `when` at once. A non-finite `at` drops the voice
  and is journaled (D5). It never refuses the commit: a sound is
  presentation, and no state change is lost to one.
- **No wake.** A voice is not a timer. `timer_due_ms` never reports one, so a
  voice waiting a second ahead keeps no host awake, and `clock settle` never
  waits for one (D10).
- **The seek rule holds.** A voice is recorded at the commit that issued it,
  with runner times only. One `advance(60_000)` and sixty `advance(1_000)`s
  give the same table (LLP 1092 D10).

The lookahead pattern is the documented clock (D13): a gated
`every(25, tick)` that schedules each hit whose time falls before
`now() + 100`. The precision is then the audio clock's. A 25 ms tick costs a
twelfth of the commits drums' 2 ms poll makes, and none at rest (LLP 1092 D7).

### D4 — Groups and `stopSounds`

**A group is monophonic by start time.** A voice in group `g` ends at the
first of these:

- its natural end (`at + frames / rate`);
- the start of the next voice in `g`, by start time, with ties going to the
  later call;
- a `stopSounds` that covers it.

This is a drum machine's choke (drums' closed hat cuts its open hat; each of
its tracks cuts its own last hit). It does not depend on the order of calls.
A hit scheduled 100 ms ahead is cut correctly by one scheduled 50 ms ahead in
the same commit. A voice cut before it starts never sounds: its end equals
its start, and it is recorded as cut.

**`stopSounds()`** ends every voice still live at the commit's time `t`.
**`stopSounds(group="hat")`** ends that group's. A sounding voice ends at `t`,
and a waiting voice is cancelled. This is what a Stop button needs once hits
are scheduled ahead: without it, the hits already in the window would still
play.

There is no `at=` on `stopSounds` (§8, Q8).

### D5 — The runner's voice table

The new `runner/src/sound.rs` holds a table of voices:
`{id, sound, at, gain, group, ends, by}`.

- `id` counts up from 1 per boot.
- `by` is how the voice ends: `end`, `group`, `stop` or `cut` (cut before it
  started).
- **When.** The table is updated after a commit stands, from that commit's
  `playSound` and `stopSounds` commands, in the order they were issued. This
  is where the row form of `scrollIntoView` is handled (`commit.rs:616–641`).
  A refused commit issues nothing, and `poison()` clears the table along with
  the commands (`commit.rs:665`).
- **Two channels.** The authored commands stay in `take_commands()` and are
  journaled as `command playSound(…)`, as today. That is what difftest
  observes and what Lean issues (D11). The table's output goes to a separate
  queue, `take_sounds() -> Vec<SoundOp>`:
  - `Play { id, sound, at, gain }`
  - `End { id, at }`, for a voice whose end moved earlier, whether by a
    group or a stop.

  Every host's command switch skips `playSound` and `stopSounds` as "the
  runner's own".
- **Drops**, each journaled, the voice not added, the commit standing:
  - a `src` that no `sound` declares:
    `sound dropped: "assets/x.wav" is not a declared sound`;
  - a non-finite `at`;
  - **the bound:** at most **32 live voices**, sounding or waiting at the
    commit's time, after that commit's cuts. A 33rd is dropped:
    `sound dropped: 32 voices sound or wait (assets/kit/hat.wav at 1250)`.
    The runner knows every voice's end (D1), so the bound is the same on
    every host. Every output keeps at most 32 voices (D7, D8).
- **The journal**, one line per change:
  - `sound 7 assets/kit/kick.wav at 1250 gain 0.8 group kick`
  - `sound 6 ends at 1250 (group kick)`
  - `sounds stopped: 3 (group hat)`

  A start given in the past adds `(asked 1240)`.
- **A dev reload or a `reload`** starts the table over. The host ends every
  sounding voice and journals `sounds stopped: N (reload)`.

The JS target keeps the same table in `host/web-js/sounds.js`, with the same
rules and journal lines (`rt.js` is 1,338 lines, so it gains only the call
site after `command()`, `rt.js:182`).

### D6 — Runner time to the speaker

Each host maps a voice's runner time to its output clock, aiming the onset at
the speaker, not at the mixer:

- **Web.** Runner time is `performance.now()` minus an origin: the wasm
  glue's `t0` (`glue.js:162–169`), and the JS target's `start`
  (`rt.js:1236`). With `{contextTime, performanceTime} =
  ctx.getOutputTimestamp()`:

  ```
  when = contextTime + (origin + at - performanceTime) / 1000
  ```

  Where `getOutputTimestamp` is missing, `currentTime` read beside
  `performance.now()`, plus `outputLatency` (or `baseLatency`), stands in.
- **Apple.** Runner time is `(CACurrentMediaTime() - t0) × 1000`
  (`Session.swift:86–87`). The render block's `AudioTimeStamp.mHostTime`
  converts to the same seconds. A voice's first frame in a buffer is
  `round((t0 + at/1000 - bufferHostSeconds - presentationLatency) × rate)`.
  `presentationLatency` is `AVAudioEngine.outputNode.presentationLatency`.
- **A voice the mapping puts in the past** starts in the next buffer from its
  first frame, as a past `when` does on the web. Its attack is never skipped.

Under the driver the clock is virtual and is not mapped (D10).

### D7 — The web: `sound-glue.js` for both targets

One output file, `host/web/sound-glue.js`, serves the wasm glue and the JS
target. It is loaded after the first frame, and only by a plan whose `sounds`
table is not empty, as `media-glue.js` is (`glue.js:29`). The boot graph does
not change (`bun scripts/boot.mjs`).

- **Load.**
  - It makes one `AudioContext({latencyHint: "interactive"})` and one
    `GainNode` per voice into `destination`.
  - It fetches and `decodeAudioData`s every declared sound, in declaration
    order.
  - It sets `navigator.audioSession.type` where the browser has it (D13 Q5).
- **Activation.** A capture-phase `pointerdown`, `keydown` and `touchend`
  listener on `document` calls `resume()` until the context runs. A tap's
  own commit runs inside its handler on both targets, so the first tap's
  sound plays (trivia F5's case).
- **A voice before activation is dropped**, never queued. A queued voice
  would burst on the first tap. It is journaled through the host's log as
  `sound blocked: the page has had no user activation (assets/x.wav)`.
- **A voice whose sound is still decoding** starts when it is decoded, from
  its first frame. If it is still undecoded at its end, it is dropped and
  journaled.
- **`Play`** makes a source node with `start(when)` (D6). **`End`** calls
  `stop(when)`. Web Audio applies the last `stop` given, and the runner only
  ever moves an end earlier.
- **The wire.** The wasm host gets an op `sound` in its batch
  (`host/web/src/batch.rs`), carrying `take_sounds()`. `host/web/src/host.rs`
  is 1,497 lines and gains one call. The JS target hands its table's ops to
  the same functions.

### D8 — Apple: a sound arm with one mixer

The video arm sets the shape: `host/apple/soundarm/SoundArm.swift` is built
as `libexact_sound.dylib`. `build.mjs` builds it only when the plan declares a
sound, as it does `hasVideo`'s arm (`build.mjs:1043–1044`).
`SoundModule.swift` `dlopen`s it after first pixel. AVFAudio therefore never
enters ExactKit's link graph.

- **Engine.** One `AVAudioEngine` holds one `AVAudioSourceNode`, connected to
  `mainMixerNode` in the output's format. At load, every declared sound is
  read with `AVAudioFile` into a float buffer and converted to that format
  once (`AVAudioConverter`).
- **The mixer.** The render block keeps 32 voice slots, filled from a
  lock-free single-producer ring of `Play` and `End` ops. For each buffer it
  sums every live voice from its first frame (D6) to its end frame, times its
  gain. A slot frees at its end. There is no allocation and no lock in the
  block.
- **Why not `AVAudioPlayerNode`.** `scheduleBuffer(_:at:)` would start a
  voice exactly. But a group's cut and `stopSounds` of a waiting voice need a
  stop at a future time, which a player node does not have, and polyphony
  needs a pool of nodes. A source node gives both at the sample, with one
  node.
- **Session.** On iOS and tvOS, `AVAudioSession` is `.ambient` by default,
  as Safari's Web Audio is under `auto`, and the game add-on's audio is
  (`CanvasSeams.swift:678–688`). `audio_session: "playback"` in `app.json`
  picks `.playback` (Q5). An interruption stops the engine and restarts it
  when the interruption ends. Voices due in between are not played; each is
  journaled `sound skipped: interrupted`.
- **The wire.** The batch gains an op `sound` (`host/apple/src/batch.rs`).
  `Session.swift` (1,486 lines) passes it to `SoundModule` in one line.

### D9 — Linux and Windows: the record, no output

The runner's table, journal and `state.sounds` are there (D5, D10). The
presenter drains `take_sounds()` and plays nothing. `state.sounds.output` is
`"none"`, as `state.media` is `unavailable` (`agent.rs:197–209`). Windows
runs through the Linux presenter and does the same.

A Linux output would link ALSA or PipeWire, or speak ALSA's ioctls in Rust.
Either is a decision about the host's "no system library" line. A Windows
output could reuse `game/audio`'s WASAPI code, but only as an artifact of its
own. Both are deferred to a consumer (§7).

### D10 — The driver: no output, the record, `expect sound`

- **No output under the driver**, on any host. Its clock is virtual: a
  `clock +60000` passes a minute in a moment. Hosts report
  `state.sounds.output: "agent"`. Development without the driver plays (Q6).
- **`state`** gains `sounds`:
  - `voices`: the last 64 voices, each `{id, src, at, gain, group, ends, by}`,
    with `state` derived from the clock (`waiting`, `sounding`, `ended`);
  - `live`: the live count;
  - `output`: the host's facts (`web-audio` with the context's `state` and
    `decoded: "16/16"`, `avaudioengine`, `none`, `agent`), and `late` and
    `blocked` counts.

  It is printed by the new `runner/src/agent/sound.rs` (`agent.rs` is 1,415
  lines) and by `agent.js` from `sounds.js`. No operation is added.
- **`clock settle`** waits for no voice: a voice changes no state and holds
  no request.
- **The test step:**

  ```ebnf
  expect-sound = "expect" "sound" ( "has" | "missing" ) STRING [ "at" NUMBER ] [ "gain" NUMBER ] ;
  ```

  `has` passes when the table holds a voice of that `src`, started at that
  time when `at` is given (the effective start, D3), and with that gain when
  `gain` is given. Cut voices count. Dropped calls do not. `missing` is the
  negation. It mirrors `expect tree has|missing`.

  ```contract-test
  test "a bar at 120 BPM schedules its kicks on the grid"
    tap "play"
    clock +600
    expect sound has "assets/kit/kick.wav" at 0
    expect sound has "assets/kit/kick.wav" at 500
    expect sound missing "assets/kit/kick.wav" at 125
  ```

  The step parser is `contract/syntax/src/parser/steps.rs`. The driver reads
  `state.sounds`. A failure prints the `src`'s voices near the asked time.

### D11 — Lean and difftest

- Lean issues commands as values (`Observe.lean:93–94`). `playSound` and
  `stopSounds` are commands to it, printed with their positional arguments,
  `none` included. The runner keeps them in `take_commands()` (D5), so
  `observe` (`semantics/difftest/src/observe.rs:134`) agrees with no change.
- `contract lean` ignores `sound` declarations. `type-sound-undeclared` is the
  checker's, and Lean's `LowerCheck` does not model files.
- The voice table (ids, cuts, the bound) is not in Lean. It is tested by
  runner tests and by conformance across three runtimes (§5).
- The corpus gains `commands/play-sound.contract`. The generator emits
  `playSound` with literal declared sources.

### D12 — `rules/DEFERRED.md`

Proposed text, recorded in its own commit once the orchestrator decides Q1.
It goes under **Components**, beside `video`:

> **Expanded (LLP 1096; [admitted by the orchestrator under Charlie's
> 2026-10-04 delegation]):** short sounds an action can schedule: a `sound`
> declaration (a WAV in `assets/`, at most 10 s), `playSound(src, at=,
> gain=, group=)` and `stopSounds(group=)`. The runner keeps the voice table.
> The web plays through Web Audio, and Apple through one `AVAudioSourceNode`
> mixer in an arm loaded on demand. Linux and Windows keep the record.
>
> - **Consumers:** drums (R1–R3), snake (F1), trivia (F5).
> - **Unblocks:** a retriggered, sample-accurate hit, without two media
>   elements per voice or a 2 ms poll.
> - **Take:** [Q1].
> - **Still out:** synthesis (tones, oscillators), pitch and rate, pan,
>   loops, fades and gain automation, effects, a handle to one voice, a
>   stop at a future time, compressed formats, sound produced by a data
>   module, microphone input, and output on Linux and Windows (LLP 1096 §7).

No new tag is added. HTML has no element for this; Web Audio is an API, so
its shape here is a command, as `haptic`'s is. The **Components** count is
unchanged.

### D13 — Docs and adoption

**Docs.**

- `contract-for-agents.md` replaces the "Ding" recipe (`:423–441`) with
  `playSound`, and the recipe for the lookahead clock (D3) with drums' tick.
  It keeps one sentence for `audio`: a player for long media (jukebox).
- `contract-grammar.md` gains `sound`, the two commands and `expect sound`.
- `reference.md`'s host table gains a row: web and Apple play, Linux and
  Windows record.
- `agent-pitfalls.md` gains: "a scheduled sound plays even after Stop unless
  the action calls `stopSounds`", and "the first sound on the web needs a
  user activation".
- LLP 1042 §8 gains a pointer here.

**Adoption** (outside the repo, `EXACT_APP_DIR`, driven on the web and macOS):

- **Drums** (stage 2 on the web, stage 3 on macOS):
  - 16 `sound` lines;
  - the 16 `audio` elements, `elementOn` and the alternating `hit` go;
  - `task clock when playing every(25, tick)` replaces `every(2, tick)`, which
    needs LLP 1092 stage 2;
  - `tick` schedules each hit due before `now() + 100` with `at=`, the track's
    `gain` and `group=` set to the track (each track cuts its own last hit,
    as the two elements did), and the closed hat in the open hat's group;
  - `togglePlay`'s stop calls `stopSounds()`;
  - `preview` calls `playSound` now.

  Its tests replace the `hits` derive with `expect sound`.
  `tools/timing-web.mjs` wraps `AudioBufferSourceNode.prototype.start` and
  compares each `when`, mapped back through D6, with the grid.
- **Snake** (stage 2): a WAV each for eat and crash, played when the setting
  is on; the "silent build" label goes.
- **Trivia** (stage 2): its two tones become WAVs played from the answer's
  action. The 1×1 `video` and the `audioFailed` filter go. `haptic` stays
  beside them.

## 4. Effect on each implementation

Every file stays under 1,500 lines: new behaviour goes in new files, and
files near the cap gain call sites only.

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| syntax | `sound` in `parser/sounds.rs` (one arm in `parser.rs`, 1,450); `expect sound` in `parser/steps.rs` | — | — |
| types, lower, plan | `HOST_COMMANDS`, `command_args`, `type-sound-undeclared`, `type-play-sound`; `lower/src/sounds.rs`; plan `sounds` | — | — |
| runner | `sound.rs`, `take_sounds`, a call site in `commit.rs` (1,077); `agent/sound.rs` | — | — |
| JS target | `sounds.js`; one call site in `rt.js`; `agent.js` state | output through `sound-glue.js` | — |
| web wasm | — | `sound-glue.js`; batch op `sound`; one line in `host.rs` (1,497) and `glue.js` (1,495) | — |
| Apple | — | — | `soundarm/SoundArm.swift`, `SoundModule.swift`, batch op, one line in `Session.swift` (1,486), `build.mjs` arm |
| Linux, Windows | drain and record; `state.sounds.output` | — | — |
| driver | `expect sound`; `state.sounds` | — | — |

## 5. Tests

- **Compiler** (`contract/cli/tests/it/sound.rs`, `contract/corpus/rejects.txt`):
  - a declared 16-bit WAV and a float WAV are accepted;
  - each `lower-sound-*` refusal is asserted whole: an mp3, 24-bit PCM,
    three channels, 11 s, outside `assets/`;
  - `type-sound-undeclared` for a literal, and none for a computed `src`;
  - `gain=1.5` refused; an unknown option refused;
  - `sound` as a state name still parses;
  - `expect sound` parses.
- **Plans.** Every in-repo app's plan decodes, with an empty `sounds` table
  and a changed digest.
- **Runner** (`runner/src/sound/tests.rs`):
  - retrigger: three calls in a commit make three voices;
  - a past `at` starts at the commit's time and journals `(asked …)`;
  - a group cuts by start time whatever the order of calls; a tie goes to
    the later call; a voice cut before it starts is `cut`;
  - `stopSounds()` and `stopSounds(group=)` end the sounding and cancel the
    waiting;
  - the 33rd live voice is dropped and journaled, while a voice that ended
    frees its place;
  - an undeclared computed `src` and a NaN `at` are dropped; the commit
    stands;
  - a refused commit issues nothing; poison and reload clear;
  - `take_commands()` still carries the authored commands;
  - one `advance(60_000)` and sixty `advance(1_000)`s give equal tables;
  - `timer_due_ms` ignores voices.
- **Conformance.** `host/web-js/conformance/sounds.contract` and `.steps`
  compare the journal and `state.sounds.voices` across the wasm runner, the
  JS runtime and the Linux host: a lookahead tick, a choke, a stop, the
  bound.
- **Web output** (`host/web/tests/sound-glue.test.mjs`, run in Chrome by the
  async lane):
  - with an `OfflineAudioContext`, a voice at 250 ms renders its first
    non-zero sample at frame `0.25 × rate`, with gain applied;
  - a group cut ends it at the next voice's frame;
  - a voice before activation is blocked and journaled.
- **Apple** (an XCTest on the arm's mixer, offline): fed a host-time stamp,
  the render block places a voice's first frame and its cut at the frames D6
  computes, sums two voices, and frees a slot at its end.
- **Lean.** `lake build`; difftest `corpus` and `random --count 500` (async
  lane).
- **Driven.** D13's adoptions, the five checks after each stage, and drums
  heard on the web and macOS by the implementer, outside the driver.

## 6. Implementation plan

Each commit passes the five checks.

1. **2026-10-06, the model** (D1–D5, D9–D11, D12's record). Compiler, plan,
   runner, the JS target's table, Linux and Windows draining, the driver,
   conformance.
   - **Exit:** §5's compiler, runner and conformance tests; the plans decode.
2. **2026-10-07, the web output** (D6, D7) and the web adoptions (D13).
   - **Exit:** the offline-context test; drums, snake and trivia tests on
     the web with `expect sound`; drums' `timing-web.mjs` showing each `when`
     on the grid; heard in Chrome, Firefox and Safari.
3. **2026-10-08–09, Apple** (D6, D8).
   - **Exit:** the mixer XCTest; drums on macOS and the iOS simulator, heard;
     drums' tests under `test macos`.
   - If the iOS session's interruption handling slips, macOS lands and the
     slipped piece gets a `QUEUE.md` line.
4. **2026-10-09, docs** (D13), after stage 2's adoptions run.

## 7. Deferred, with preconditions

- **Linux output.** Needs a consumer that ships sound on Linux, and a ruling
  on linking ALSA or PipeWire (or a pure-Rust ALSA client) as a separate
  artifact, never a feature on `exact-linux`.
- **Windows output.** Needs a Windows consumer with sound (Skirmish has the
  game's). It would be a separate artifact, its WASAPI code from
  `game/audio/src/windows.rs`.
- **`stopSounds(at=)`, fades, gain automation.** Both outputs could do them
  at the sample (Web Audio's `stop(when)` and `setValueAtTime`; the mixer's
  end frame). Precondition: a consumer, such as a stop at the bar's end or a
  fade out.
- **Pitch and rate, pan, loops.** Precondition: a consumer that a WAV per
  variant cannot serve.
- **Synthesis** (snake F1's `playTone(frequency, ms)`, an `OscillatorNode`).
  A declared WAV beep serves F1. Precondition: a tone whose parameters
  come from state.
- **Compressed formats** (m4a, mp3, ogg). The web and Apple decode them, but
  their durations need the compiler to parse each container, and Firefox
  and Safari disagree on Ogg. Precondition: a consumer whose WAVs are too
  big to bundle.
- **Sound from a data module** (PCM a source computes). Precondition: a
  consumer, and a byte-transfer design (LLP 1027.003 §9's typed buffers).

## 8. Considered, not taken

- **Fixing `audio`'s retrigger instead** (R1's glue change: compare `paused`
  with `el.paused`). It fixes retrigger but still gives no scheduling and no
  choke, and it makes a media element into an instrument.
- **A `sound` element** with a hit counter. Not HTML. It ties a one-shot to a
  node's lifetime, and a scheduled start becomes a prop that is read once.
- **The game add-on's audio crate in the core.** The DEFERRED game entry
  forbids it. Its outputs also start on ticks, not at a `when`.
- **`AVAudioPlayerNode` pools** (D8): no stop at a future time.
- **Queueing a voice until activation** (D7): it bursts on the first tap.
- **Refusing the commit when a voice overflows the bound.** That is LLP 1092
  D4's rule for a write, where dropping loses data. A dropped sound loses
  nothing durable, so it is dropped and journaled, as `postMessage` drops
  past its bound (`presenter.rs`).
- **Output under the driver's `clock +N real`.** That clock moves in 50 ms
  steps, so a mapped voice would jitter by a step. Q6.

## 9. Open questions, with the author's recommendations

1. **The admission and its take.** Recommended take: the `audio` element
   stops being the sound-effect path, and two pieces of owed work come off:
   - the R1 retrigger change to `media-glue.js`, `media.js` and the Apple
     arm (a `paused=false` element that ended plays again on its next hit),
     which is not built;
   - the guide's "Ding" recipe and LLP 1042 §8's sound-effect paragraph, which
     are deleted.

   `audio` stays HTML's player, for jukebox. If the orchestrator rules that
   none of this is doing-list work, the fallback is a waiver with no take, as
   LLP 1092 and LLP 1094 had.
2. **The bound.** 32 live voices. Drums at 200 BPM with a group per track
   peaks below 16. A game with many effects may want 64. The mixer's cost is
   per live voice, so 64 costs little; 32 keeps the record short.
3. **Formats.** 16-bit and float WAV only, for one decoder rule on every
   host and a header the compiler reads in a few lines. Recommended as
   written.
4. **Length.** 10 s, with longer clips going to `audio`. Recommended as
   written.
5. **The session.** Recommended: `.ambient` by default, as Safari's Web
   Audio under `auto`. `app.json` `audio_session: "playback"` (the Audio
   Session API's word, in the manifest's snake case) picks `.playback` on iOS
   and tvOS, and `navigator.audioSession.type = "playback"` where a browser
   has it. Drums wants it: a drum machine the ring switch silences is broken.
   It is app-wide, so it also settles QUEUE's "iOS sound is silenced by the
   ring/silent switch" for an app that sets it. That is the one piece of LLP
   1042 §5's arbiter admitted here.
6. **Output under the driver.** Recommended: none, ever. The record is what a
   test reads, and a developer listens in the dev loop. The alternative,
   playing under `clock +N real`, jitters (§8).
7. **Globs in `sound`** (`sound "assets/kit/*.wav"`). Recommended no for v1:
   drums' 16 lines are clear, and a glob's order would need a rule.
8. **`stopSounds(at=)`.** Recommended deferred (§7). Both outputs could do it
   cheaply once a consumer asks.

## 10. Revisions

- **r1** (2026-10-04): first draft.
