# LLP 1096: Sounds an app can schedule

**Type:** RFC
**Status:** Accepted (r4, by the orchestrator under Charlie's delegation after three review rounds; Grok 4.7 only — Codex budget exhausted; round-3 findings folded unreviewed — the implementation review checks them).
- r1 (`88c2f191a`) was reviewed by Grok 4.7 (xhigh) with two scopes: semantics and web fidelity (`llp/reviews/1096-r1.grok-a.md`, READY WITH CHANGES) and implementation (`llp/reviews/1096-r1.grok-b.md`, NOT READY).
- r2 (`b51ca7d3b`) had a delta review (`llp/reviews/1096-r2.grok.md`, NOT READY: five MATERIAL, six MINOR, five NIT).
- r3 (`cd9a371dd`) had the final round (`llp/reviews/1096-r3.grok.md`, NOT READY: two MATERIAL, three MINOR, four NIT).
- r4 folds round 3's fixes as given, with no further review (§10).
- The orchestrator decided r1's open questions under Charlie's 2026-10-04 delegation (§9). The admission is recorded in `rules/DEFERRED.md` in its own commit.
**Systems:** Contract compiler (a `sound` declaration; three host commands; `contract/lower/src/sounds.rs`; the test step `expect sound`), Plan (`sounds` table), bake receipt (`loads`), Runner (new `runner/src/sound.rs`, `take_sounds`, `state.sounds`), JS target (new `host/web-js/sounds.js`), web output shared by both targets (new `host/web/sound-glue.js`), Apple (new `host/apple/soundarm/`, `SoundModule.swift`, `AudioSession.swift`), Linux and Windows (record only), conformance, Lean and difftest (commands only), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3, r4)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-06, stage 2 on 2026-10-07, stage 3 on 2026-10-08–09, stage 4 on 2026-10-09 (§6)
**Amends:** LLP 1042 §8 (a sound effect is no longer an `audio` element's job); LLP 1046.003 AU3 (`CanvasAudio` stops choosing the session category, D8); `docs/contract-for-agents.md`'s "Ding" recipe and its named-argument sentence (`:222`); `rules/DEFERRED.md` (D12)
**Related:** LLP 1005 §3 (host commands); LLP 1012 (the agent); LLP 1019 (a declared asset: fonts); LLP 1051 §5.3 (`start(when)` noted); LLP 1070.000 (a command the runner keeps for itself); LLP 1077 D14 (`haptic`, the command this follows); LLP 1089 D1 (where `HOST_COMMANDS` lives after its stage 1); LLP 1092 D7, D8 (gated tasks, which the lookahead clock uses). Diaries: `~/projects/x2apps/drums/DIARY.md` (R1–R3, Top 5 #1, `repros/R1-retrigger`), `snake/DIARY.md` (F1), `trivia/DIARY.md` (F5). External, read 2026-10-04: Web Audio API 1.1 (`AudioBufferSourceNode`, `AudioScheduledSourceNode.start(when)`/`stop(when)`, `GainNode`, `AudioContext.getOutputTimestamp()`); the W3C Audio Session draft (`navigator.audioSession.type`); AVFAudio's `AVAudioEngine`, `AVAudioSourceNode`, `AVAudioPlayerNode`, `AVAudioNode.outputPresentationLatency`.

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

shape Hit
  src: string
  at: number
  gain: number
  group: string

component Drums
  task clock when playing                 -- LLP 1092 D7
    every(25, tick)
  action start                            -- the downbeat and the open window, from the press itself
    playing = true
    playSounds(hitsBetween(song, now(), now() + 100))
    scheduledTo = now() + 100
  action tick                             -- each tick schedules the next window
    playSounds(hitsBetween(song, scheduledTo, now() + 100))
    scheduledTo = now() + 100
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
- **`playSounds`** does the same for a list of hits. Contract actions have no
  loop, so a window of hits is a list a `fn` computes.
- **`group`** makes a group's voices monophonic, as a drum machine's choke
  group is.
- **`stopSounds`** ends the voices that are sounding and cancels the ones
  still waiting.

The runner keeps the voice table, so what was scheduled, when, is the same on
every host and under the driver's clock. The web and Apple play it: Web
Audio's `start(when)`, and a C render callback behind an `AVAudioSourceNode`
on `AVAudioEngine`. Linux and Windows keep the record and play nothing.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `sound "path"`: a declared WAV, read by the compiler | snake F1 | 1 |
| D2 | `playSound(src, at=, gain=, group=)`, `playSounds(hits)` | drums R1 | 1 |
| D3 | `at` is runner time; the past means now; the window clock | drums R2, R3 | 1 |
| D4 | `group=`: monophonic by start time; `stopSounds(group=)` | drums (hat choke, stop) | 1 |
| D5 | The runner's voice table, its bound, its journal and `take_sounds` | — | 1 |
| D6 | Runner time to the speaker, on each host | drums R3 | 2, 3 |
| D7 | The web: one `sound-glue.js` for both targets; activation | trivia F5 | 2 |
| D8 | Apple: a sound arm with one mixer; one session owner | drums | 3 |
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
  can be scheduled ahead of time."
- **Snake F1.** Snake has a sound toggle and no way to make a sound.
- **Trivia F5.** A remounted 1×1 `video` playing a generated mp4 stayed
  `paused: true`. Its `play()` came outside the gesture and was dropped with
  no error.
- **Today's code.**
  - `audio` is node type 12, `Video` with `semanticTag` audio
    (`kernel/tables/schema.json:54–58`; `contract/lower/src/media.rs:49–54`).
  - Its players are the web's `<audio>` and Apple's AVPlayer, in an arm
    loaded on demand (`host/apple/Sources/ExactKit/VideoModule.swift:27–31`).
  - Linux reports it unavailable (`host/linux/src/agent.rs:197–209`).
  - Host commands are names in `HOST_COMMANDS`
    (`contract/types/src/checks.rs:744–778` today; LLP 1089 D1 moves the
    list into `contract-syntax`). They are lowered to `Opcode::Command` and
    journaled after the commit stands (`runner/src/runner/commit.rs:616–651`).
- **The game add-on** (`game/audio`, LLP 1046.003 AU3) has a Web Audio, an
  Apple AudioUnit and a WASAPI output. It starts voices with `start(0,
  offset)` on tick boundaries and never at a future `when`
  (`game/audio/src/web.rs:125`). No core crate may depend on it.
- **Web Audio, measured for r2** (Playwright 1.63; Chrome 154 through
  `channel: 'chrome'`, Firefox and WebKit through Playwright's builds; a
  fresh context, no gesture):
  - `start(-0.01)` throws `RangeError` in all three.
  - The first `getOutputTimestamp()` is `{0, 0}` in Chrome and WebKit, and
    `contextTime` 0 with a live `performanceTime` in Firefox.
  - In an `OfflineAudioContext` at 48 kHz, `start(0.25)` first sounds at frame
    12000 in all three. `stop(0.5)` then `stop(0.3)` sounds last at frame
    14399, so the last `stop` wins.
- **Web Audio at the press, measured for r3** (the same browsers; a
  capture-phase `pointerdown` on a Playwright click):
  - `stop()` on a source node never started throws `InvalidStateError` in all
    three, and `start(0)` succeeds.
  - The pairs at the press depend on the autoplay policy, and the two probes
    disagree.
    - **The round-3 review's probe** (autoplay blocked, `resume()` on a
      capture-phase `pointerdown`) found live pairs. Chrome was `running`
      with `{contextTime: 0, performanceTime: 232}`. WebKit was `{0, 0}` and
      `suspended` before the gesture, and already `running` with
      `{0.0187, 116}` on the handler's first line. Firefox at construction
      was `suspended` with `{0, 146}`.
    - **The author's r3 probe** (the context made before the gesture,
      autoplay allowed) saw `{0, 0}` at the press in Chrome and WebKit.

    The normative branch is `performanceTime > 0` (D6). `{0, 0}` is the
    pre-gesture hold.

## 2. What the web and the platforms offer

- **Web Audio.**
  - An `AudioBuffer` is decoded PCM (`decodeAudioData`). An
    `AudioBufferSourceNode` plays one buffer once, so a retrigger is a new
    node.
  - `start(when)` and `stop(when)` take the context's clock in seconds. A
    `when` in `[0, currentTime)` starts at once; a negative `when` throws.
  - `GainNode.gain` is a linear multiplier.
  - `getOutputTimestamp()` maps the context's clock to `performance.now()` at
    the speaker, once the graph has rendered a block.
  - The context starts `suspended` until a user activation.
  - The usual pattern ("A Tale of Two Clocks") is a coarse timer that
    schedules every note falling in the next window.
- **Audio Session** (W3C draft, Safari 16.4+). `navigator.audioSession.type`
  is `auto`, `playback`, `ambient` and so on. Under `auto`, Web Audio is
  ambient: the ring/silent switch mutes it.
- **AVFoundation.**
  - `AVAudioPlayerNode.scheduleBuffer(_:at:)` starts a buffer at a host time.
    A node plays its buffers one after another, so overlap needs a pool of
    nodes. For a buffer already scheduled, `stop()` takes no time, clears
    every scheduled event and resets the node's timeline. A choke that
    arrives on a later tick has no stop at a future sample.
  - `AVAudioSourceNode` calls a render block for each buffer. The block that
    is safe on the realtime thread is Swift-unavailable in the macOS/iOS 27
    SDK (`AVAudioSourceNode.h:69–72`, `:82–88`). A C callback compiled into
    the arm is.
  - `AVAudioNode.outputPresentationLatency` is a node's delay to the output:
    the output's presentation latency plus the processing downstream of the
    node.
- **Linux.** Output means ALSA or PipeWire, which are system libraries the
  pure-Rust host does not link (LLP 1000; `host/linux/Cargo.toml`).

## 3. Decisions

### D1 — `sound "path"`: a declared WAV

```ebnf
sound = "sound" STRING NL ;
```

`sound` is a top-level declaration, a keyword only at the start of a
top-level line, as `font` is (`parser.rs:346`). So `state sound = true`
(snake) still parses. `parser.rs` is 1,450 lines, so it gains only the
dispatch arm and the keyword in its "expected …" message (`:392–398`). The
declaration's parser is the new `parser/sounds.rs`.

The path follows `font`'s rules (`contract/lower/src/fonts.rs:100–147`): it is
under the app's `assets/`, stays inside the app directory, and must be
readable. A module's `sound` lines join its user's.

The new `contract/lower/src/sounds.rs` walks the RIFF chunks until `data`,
skipping any chunk it does not need (`fact`, `LIST`, `bext`). It needs no new
dependency. It refuses with the file named:

- **`lower-sound-format`**: anything other than `WAVE` with a `fmt ` of
  format 1 (16-bit integer PCM) or format 3 (32-bit float), 1 or 2 channels,
  at 8–96 kHz, or `WAVE_FORMAT_EXTENSIBLE` naming one of those. The message
  gives the conversion: `afconvert -f WAVE -d LEI16 in.m4a out.wav`, or
  `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`.
- **`lower-sound-long`**: longer than 10 s. "A sound is a short clip; play
  longer audio with `audio`."
- **`lower-sound-unreadable`** and **`lower-sound-path`**: as for fonts.

Plan `sounds` gains a row per file: `src`, `frames`, `rate` (the file's),
`channels` and a SHA-256 digest. `FORMAT_DIGEST` changes. The runner reads
durations in seconds, `frames / rate` (D5). The hosts read the list of files
to decode from it (D7, D8).

The files are already in every bundle: the web build copies `assets/` whole
(`host/web/build.mjs:143–147`, `host/web-js/build.mjs:480`), and so does
Apple's (`host/apple/build.mjs:243–246`).

### D2 — `playSound`, `playSounds`, `stopSounds`

The three names join `HOST_COMMANDS`, wherever the list lives when stage 1
lands: `contract/types/src/checks.rs:744` today, or `contract-syntax` after
LLP 1089 stage 1. LLP 1089's expander therefore sees them as commands, and a
call keeps the command name. An action may not be named after one, as for
any command.

**Checking.** Each command gets an early checker in `check_command`, as
`share` has (`checks.rs:1057–1058`). The generic loop's `infer` would refuse
a named argument as a canvas binding (`type-named-argument`,
`contract/types/src/lib.rs:683–688`). The refusal id is `type-play-sound`, and
its message gives the usage. If the three checkers and the list's
structural check do not fit `checks.rs` (1,413 lines) under the cap, they go
in a new `contract/types/src/checks/sounds.rs`, and `check_command` gains
only the dispatch.

- **`playSound(src, at=, gain=, group=)`**:
  - `src` is a string. A literal that no `sound` declares is refused
    (`type-sound-undeclared`, which names the declared sounds nearest it). A
    computed one is checked by the runner (D5); drums chooses a track's
    sound from data.
  - `at=` is a number, in milliseconds on the runner's clock (D3). It
    defaults to the commit's time.
  - `gain=` is a number, a linear multiplier, 0–1, defaulting to 1. A
    literal outside 0–1 is refused, as a literal `volume` is
    (`media.rs:27–42`). A computed one is clamped to 0–1 by the runner
    before it is stored (D5), so `state.sounds` and the speaker agree. This
    is unlike the media glue, which refuses an out-of-range bound `volume`
    with `invalid-value` (`media-glue.js:52`).
  - `group=` is a string (D4). With none given, or `""`, the voice is in no
    group.
- **`playSounds(hits)`**: `hits` is a list of records with the fields
  `src: string`, `at: number`, `gain: number` and `group: string`, checked
  structurally. The app declares the shape (the Summary's `Hit`). Each item
  is one `playSound`, in list order.
- **`stopSounds(group=)`**: `group` is optional (D4).

**Lowering.** `command_args` (`contract/lower/src/expr.rs:15`) gives fixed
positions, with `none` for an omitted argument, as for `scrollIntoView`:
- `playSound`: `src, at, gain, group`;
- `playSounds`: `hits`;
- `stopSounds`: `group`.

An omitted `at` or `gain` is pushed as `none` typed `Option<Number>`, not
through `compile_or_none`'s `Option<String>` (`expr.rs:54–66`).

Each item of each call is a new voice: Web Audio's one-shot source, so a
retrigger is simply another call. There is no handle; groups do the stopping
(D4). `contract-for-agents.md:222` names these commands beside `share` and
`scrollIntoView` as taking named arguments.

### D3 — `at` is runner time; the past means now; the window clock

`at` is read on the clock `now()` reads. In a timer's commit, `now()` is the
due time, not the wall time it ran (LLP 1092 D8; `rt.js` `advance`). A hit
planned at `t` therefore lands on the grid, whatever the timer's lateness.

- **The effective start** is `max(at, t)`, where `t` is the commit's time.
- **A non-finite `at` or `gain`** drops that voice with a journal line (D5).
  It never refuses the commit: a sound is presentation, and no state change
  is lost to one.
- **No wake.** A voice is not a timer. `timer_due_ms` never reports one, and
  `clock settle` never waits for one (D10).
- **The seek rule holds.** A voice is recorded at the commit that issued it,
  with runner times only. One `advance(60_000)` and sixty `advance(1_000)`s
  give the same table (LLP 1092 D10).

**The window clock** is the documented pattern (D13):

- **The downbeat comes from the press.** The action that starts playback
  schedules the first window, `[now(), now() + 100)`. A gated `every(25, …)`
  first fires 25 ms after its gate opens (LLP 1092 D8), so a downbeat left to
  the first tick would already be 25 ms late.
- **Each tick schedules the next window,** `[scheduledTo, now() + 100)`.
  `scheduledTo` is the end of the last window scheduled. The hits are a list
  from a `fn`, so a step shorter than the tick never falls behind: every hit
  in the window is in the list.
- **The precision is the audio clock's.** A 25 ms tick makes a twelfth of the
  commits that drums' 2 ms poll makes, and none at rest.

### D4 — Groups and `stopSounds`

**A group is monophonic by start time.** A voice in group `g` ends at the
first of these:

- its natural end: its effective start (D3) plus `1000 × frames / rate`;
- the start of the next voice in `g`, by start time, with ties going to the
  later call;
- a `stopSounds` that covers it.

This is a drum machine's choke. It does not depend on the order of calls. A
hit scheduled 100 ms ahead is cut correctly by one scheduled 50 ms ahead in
the same commit.

**How a voice ended** is its `by`:

| `by` | Meaning |
|---|---|
| `end` | It played to its natural end. |
| `group` | A later voice in its group cut it after it started. |
| `cut` | A later voice in its group cut it before it started; it never sounds, and its end equals its start. |
| `stop` | `stopSounds` ended it while it sounded. |
| `cancelled` | `stopSounds` came at or before its start; it never sounds. |

**`stopSounds()`** ends every voice still live at the commit's time `t`, and
**`stopSounds(group="hat")`** ends that group's. A Stop button needs it once
hits are scheduled ahead: without it, the hits already in the window would
still play. There is no `at=` on `stopSounds` (§7).

### D5 — The runner's voice table

The new `runner/src/sound.rs` holds a table of voices:
`{id, sound, at, gain, group, ends, by}`. `id` counts up from 1 per boot.

**When it is updated.** Only after the commit stands. The table changes only
after `update()` (`commit.rs:658`) has returned ok. Inside the `if
result.is_ok()` block where today's command journal is written
(`commit.rs:661–665`), the runner applies that commit's `playSound`,
`playSounds` and `stopSounds` commands, in the order they were issued. It
logs their `sound …` lines and queues the `SoundOp`s there. On the failure
path, and in `poison()` (`commit.rs:675–683`, `commands.clear()` at `:678`), any speculative sound records
are dropped next to `commands.clear()`.

**Two channels.**
- The authored commands stay in `take_commands()` and are journaled as
  `command playSound(…)`. That is what difftest observes and what Lean issues
  (D11).
- The table's output goes to a separate queue, `take_sounds() ->
  Vec<SoundOp>`:
  - `Play { id, sound, at, gain }`
  - `End { id, at }`, for a voice whose end moved earlier, whether by a group,
    a stop or a reload.

Every host skips the three authored names as "the runner's own", before any
refusal or warning (D7–D9).

**Drops.** Each is journaled; the voice is not added and the commit stands.
- An undeclared `src`: `sound dropped: "assets/x.wav" is not a declared
  sound`.
- A non-finite `at`: `sound dropped: at is not a number (assets/kit/hat.wav)`.
- A non-finite `gain`: `sound dropped: gain is not a number
  (assets/kit/hat.wav)`. A finite `gain` outside 0–1 is clamped (D2), not
  dropped.
- **The bound.** At most **32 live voices**, sounding or waiting at the
  commit's time, after that commit's cuts. The 33rd is dropped:
  `sound dropped: 32 voices sound or wait (assets/kit/hat.wav at 1250)`. The
  runner knows every voice's end, so the bound is the same on every host.

**The journal**, one line per change. A start given in the past adds
`(asked 1240)`.
- `sound 7 assets/kit/kick.wav at 1250 gain 0.8 group kick`
- `sound 6 ends at 1250 (group kick)`
- `sounds stopped: 3 (group hat)`

**The record.** The table keeps the last 1,024 voices for the driver (D10).

**Reload.** A dev reload or a `reload` ends every live voice: `End { id, at:
t }` for each, journaled `sounds stopped: N (reload)`. The table then starts
over. Hosts handle the `reload` command itself as today
(`Session.swift:1018–1022`). The `End`s reach the output through
`take_sounds()` before the restart.

**The JS target** keeps the same table in `host/web-js/sounds.js`, with the
same rules and journal lines. `rt.js` is 1,338 lines and gains two lines:
- in `command()` (`rt.js:222–226`), after its existing `command …` journal
  line, the three names are **recorded** in `sounds.js`'s pending list and
  return before the `Hosts` lookup. So they are never `refused:`, and nothing
  is applied there;
- after the commit's commands run (`rt.js:182`), on the success path only,
  `sounds.js` **applies** the pending list once and clears it. This is the
  only apply, so a voice gets one `id`.
- `agent.js`'s `state` reply (`:339–356`) includes `sounds.js`'s `{voices,
  live, recorded, output}`. Under the driver `output` is `"agent"` (D10).

### D6 — Runner time to the speaker

Each host maps a voice's runner time to its output clock, aiming the onset at
the speaker.

**The web.** Runner time is `performance.now()` minus an origin: the wasm
glue's `t0` (`glue.js:162–169`), and the JS target's `start` (`rt.js:1236`).

- **While the timestamp is live** (`performanceTime > 0`, whatever the
  context's `state`), with `{contextTime, performanceTime} =
  getOutputTimestamp()`:

  ```
  when = max(0, contextTime + (origin + at - performanceTime) / 1000)
  ```

  `max(0, …)` keeps a late voice from throwing (§1's probe). A `when` below
  `currentTime` starts at once, attack included. Every `start` and `stop`
  takes the clamped value.
  - Firefox's press is this case: the context is still `suspended`, with
    `contextTime` 0 and a live `performanceTime` (round-2 review, finding 2).
    The formula schedules it without waiting for `running`, and a due voice
    is `start(0)`.
- **Before the timestamp is live** (`{0, 0}`: Chrome and WebKit at the press,
  and WebKit even after `resume()` resolves, §1):
  - A voice that is due or past starts with `start(0)`.
  - A future voice is **held** as a record `{id, at, stopAt}` with no node.
    An `End` for a held voice only sets `stopAt`. `stop()` on a node that
    was never started throws `InvalidStateError` in all three engines (§1),
    so `stop` is never called on one.
  - A held voice leaves the held set on whichever path takes it first, and
    it is started at most once:
    - when the timestamp goes live, it is started by the formula;
    - or, if its runner time arrives first, it is started then with
      `start(0)`, by a timer in the glue.

    On either path, it is stopped at `stopAt` if one is set. Its start is
    skipped altogether when `stopAt` is not after that start. `stop(-0.01)`
    throws, so a `stopAt` is clamped as a `when` is.
- **Every browser measured has `getOutputTimestamp`.** Where one does not,
  `contextTime` is taken as `currentTime - outputLatency`, Firefox's measured
  gap. In Chrome the gap also includes `baseLatency`, and WebKit's gap was its
  `baseLatency`. Both are named in the code, not added.

**Apple.** Runner time is `(CACurrentMediaTime() - t0) × 1000`
(`Session.swift:86–87`).

- The source node is connected to `mainMixerNode` in the output's format, so
  nothing converts downstream of it. A sample time downstream of a converter
  is not valid (`AVAudioSourceNode.h:26–27`).
- The render callback's `AudioTimeStamp.mHostTime` is host ticks. It is
  converted to seconds on `CACurrentMediaTime`'s timebase
  (`AudioConvertHostTimeToNanos(mHostTime) / 1e9`), giving
  `bufferHostSeconds`.
- The header calls it "the HAL time at which the audio data will be
  rendered" (`AVAudioSourceNode.h:26`). That is when this node renders the
  buffer, and the buffer is heard `outputPresentationLatency` later (the
  source node's: the mixer, the output's processing and the device). Its
  maximum delay downstream of the node is `outputPresentationLatency`
  (`AVAudioNode.h:218–228`), and subtracting it aims the speaker at
  `t0 + at/1000`. The term is dropped only if the stage-3 device check (§5)
  finds the onset early by exactly that term: then `mHostTime` already
  included it. A late onset fails the check.
- A voice's first frame in the buffer is
  `round((t0 + at/1000 - (bufferHostSeconds + outputPresentationLatency)) × outputRate)`,
  where `outputRate` is the output format's sample rate. The file's rate is
  used only for durations (D1).
- The voice's end frame is its first frame plus the converted buffer's
  `frameLength`, cut earlier by an `End`.

**A voice the mapping puts in the past** starts in the next buffer, from its
first frame. Its attack is never skipped.

Under the driver the clock is virtual and is not mapped (D10).

### D7 — The web: `sound-glue.js` for both targets

One output file, `host/web/sound-glue.js`, serves the wasm glue and the JS
target.

**Loading.**
- It is loaded after first paint, and only by a plan whose `sounds` table is
  not empty, through `loadAfterPaint('sound-glue.js', 'installSound')`
  (`glue.js:92–100`).
- It assigns `globalThis.exact.installSound`, as `media-glue.js` assigns
  `installMedia` (`media-glue.js:62`).
- The JS target imports it after two animation frames, as `media.js` does
  (`host/web-js/media.js:32`). `host/web-js/build.mjs:266` adds it to the
  copied host files.
- The boot graph does not change (`scripts/boot.mjs:13`, `:176`).

**What it does.**
- **Load.** One `AudioContext({latencyHint: "interactive"})`, and one
  `GainNode` per voice into `destination`. It fetches and `decodeAudioData`s
  every declared sound, in declaration order.
- **The session.** The web build writes `app.json`'s `audio_session` as
  `data-audio-session` on `#exact-root`. The glue sets
  `navigator.audioSession.type` to it only when `navigator.audioSession`
  exists (WebKit has it; Chrome and Firefox do not, §1).
  - **Every writer of the root div carries it:**
    - the shell div, `host/web/index.html:112` and
      `host/web-js/build.mjs:394`;
    - the render host's rebuilt div, `page.rs:85–87`;
    - `render.mjs:64`'s rebuild;
    - `direct.rs:81`'s `<div id="exact-root" data-boot="">`.
  - **The exact-string cuts** that find the shell div include the
    attribute in their needles: `page.rs:34–39` and `render.mjs:35`.
- **Activation.** A capture-phase `pointerdown`, `keydown` and `touchend`
  listener on `document` calls `resume()` until the context runs. A press
  after the glue has loaded runs its commit inside its own handler on both
  targets, so that press's sound plays (trivia F5).
- **Before activation, a voice is dropped**, never queued (a queue would
  burst on the first tap). It is journaled through the host's log as
  `sound blocked: the page has had no user activation (assets/x.wav)`. A tap
  captured before boot and replayed by `rt.js:1264–1269` is a `click()`, not
  a user activation. Its voice is blocked and journaled, and the boot path
  (`capture.js`, pinned by `scripts/boot.mjs:20–21`) does not change.
- **A voice whose sound is still decoding** starts when it is decoded, from
  its first frame. If it is still undecoded at its end, it is dropped and
  journaled.
- **`Play`** makes a source node and calls `start(when)` (D6), unless the
  voice is held. **`End`** calls `stop(when)` on a started node, or sets a
  held voice's `stopAt` (D6). The last `stop` wins in all three engines (§1),
  and the runner only ever moves an end earlier.

**The wire.**
- **The wasm host.** `host/web/src/host.rs` drains `take_sounds()` at both of
  its command drains (`:552`, `:1019`), into a batch op `sound`
  (`host/web/src/batch.rs`). That is one line each; the file goes from 1,497
  to 1,499. `glue.js`'s command switch skips the three names before its
  `unknown command` warning (`glue.js:820`). Its `sound` op hands the ops to
  `installSound`.
- **The JS target** hands its table's ops to the same functions.
- **Host facts** (`state.sounds.output`) are filled in each host's existing
  state overlay outside the driver only: `glue.js` case `"state"` (`:1099`),
  which starts from the runner's reply, and `agent.js` case `'state'`
  (`:339`), which also carries the table (D5). Under the driver the overlay
  leaves `output: "agent"` alone.

### D8 — Apple: a sound arm with one mixer, and one session owner

**The arm.** `host/apple/soundarm/` holds `SoundArm.swift` and
`sound_render.c`, built together as `libexact_sound.dylib`.
- `SoundModule.swift` `dlopen`s it after first pixel, split as the video
  arm's loader is:
  - **On macOS** (`#if os(macOS)`), the path is the executable's directory
    plus `/libexact_sound.dylib`, as `VideoModule.swift:26–27` loads the video
    arm (and `WebModule.swift:150–151` the web arm).
    `build.mjs:1213`'s `loaded` gains the sound dylib, so the bundle copy
    (`:1268`) and codesign (`:1278`) include it.
  - **On iOS and tvOS** (`#else`), the path is `embeddedModule(framework:
    "ExactSound", dylib: "libexact_sound.dylib")` (`VideoModule.swift:29`,
    `:303–312`; `embeddedModule` is `#if !os(macOS)`).
- The engine and the mixer stay in the dylib. The session stays in ExactKit,
  as `CanvasAudio`'s already is.
- `sound_render.c` exports `exact_sound_render` and `exact_sound_push`,
  declared to Swift through `sound_render.h`, passed to the arm compile with
  `-import-objc-header`.

**The production gate.**
- `bake/src/receipt.rs:536–548` adds `"sound"` to `loads` when the plan's
  `sounds` table is not empty.
- `host/apple/build.mjs` builds and carries the arm on `carries('sound')`
  (`:979–982`). It adds `'sound'` beside the four names in `leftOut`, the load
  list, and both copy sites (`:1213–1218`, `:1316`).
- The arm compile (`:1023–1044`) takes the arm's C file and its header
  beside its Swift file.
- An `.ipa` wraps it as a framework: `build.mjs:1350`'s list gains
  `[soundLoadName, 'ExactSound']` beside `ExactWeb` and `ExactVideo`. A loose
  dylib in an `.ipa` is ITMS-90171 (`build.mjs:363`,
  `VideoModule.swift:303–307`).

Without this, a settled production build (release, store level 0) would ship
no arm, because a `sound` declaration is not a node.

**The engine.**
- One `AVAudioEngine` holds one `AVAudioSourceNode`, built with the C render
  callback and connected to `mainMixerNode` in the output's format (D6).
- At load, every declared sound is read with `AVAudioFile` and converted once
  (`AVAudioConverter`) to that format. A mono file is copied to every output
  channel. A stereo file going to a mono output is averaged.

**The mixer** (`sound_render.c`).
- It has 32 voice slots, filled from a single-producer, single-consumer ring
  of ops. Swift only pushes `Play` and `End` ops onto the ring.
- The ring holds 256 ops, room for 32 `Play`s and 32 `End`s several times
  over. A full ring drops the op and counts it. The count is read by the
  arm's state and journaled `sound dropped: the mixer's queue is full`.
- An `End` for a slot already freed is ignored.
- For each buffer, the mixer first applies every op in the ring, so a
  `Play` and an `End` that arrive for the same buffer both take effect in
  it. It then sums every live voice from its first frame to its end frame
  (D6), times its gain. A slot frees at its end.
- There is no allocation and no lock in the callback.

**The session: one owner.**
- `AudioSession.swift` in ExactKit is the only `setCategory` caller. The
  whole file is `#if os(iOS) || os(tvOS)`, as the current caller is
  (`CanvasSeams.swift:668–684`). `AVAudioSession` is unavailable on macOS
  (`AVAudioSession.h:31`, `:38`, `:46`).
- **On the Mac** there is no session: the arm starts `AVAudioEngine` with no
  category, and `audio_session` has no effect there.
- It reads `audio_session` from the manifest, which the bake plumbs into the
  iOS `Info.plist` as `ExactAudioSession`, the way `backgroundModes` is
  (`build.mjs:403`).
- **`audio_session` is a manifest key.**
  - `scripts/app.schema.json` gains it at the root, beside the other root
    keys. The root is `additionalProperties: false` (`:707`), and
    `scripts/app.mjs:575` refuses a manifest that does not conform.
  - `bake/src/compat.rs:333–337`'s `capabilities` object gains it beside
    `backgroundModes`, so changing it moves the compatibility id.
- It is activated by the sound arm's load, the video arm's first sound-bearing
  item, and `CanvasAudio.activate` (`CanvasSeams.swift:668–684`). That last
  one stops calling `setCategory(.ambient)` and asks the owner instead.
- The default is `.ambient`; `audio_session: "playback"` is `.playback`. This
  holds for an app with no `sound` too, since the owner is ExactKit's. That
  settles QUEUE's "iOS sound is silenced by the ring/silent switch" for a
  video-only or `audio`-only app that sets it.
- **Interruptions.** An interruption stops the engine, and it restarts when
  the interruption ends. Voices due in between are not played; each is
  journaled `sound skipped: interrupted`.

**The wire.**
- `host/apple/src/host.rs` drains `take_sounds()` at both of its command
  drains (`:468`, `:1292`) into a batch op `sound`
  (`host/apple/src/batch.rs`).
- `Session.swift`'s command loop skips the three names before an unmatched
  command reaches the delegate (`Session.swift:1067–1068`). It passes the
  `sound` op to `SoundModule` in one line (the file is 1,486).
- `Agent.swift` case `"state"` (`:206–218`) fills `state.sounds.output`.

### D9 — Linux and Windows: the record, no output

- The runner's table, journal and `state.sounds` are there (D5, D10).
- The presenter's command match skips the three names before `unknown
  command` (`host/linux/src/presenter.rs:517`). It drains `take_sounds()` and
  plays nothing. The file is 1,488 lines and gains two.
- Outside the driver, `state.sounds.output` is `"none"`. Under the driver it
  is `"agent"`, as on every host (D10).
- Windows runs through the Linux presenter and does the same.

A Linux output would link ALSA or PipeWire, or speak ALSA's ioctls in Rust.
Either is a decision about the host's "no system library" line. A Windows
output could reuse `game/audio`'s WASAPI code as an artifact of its own. Both
are deferred to a consumer (§7).

### D10 — The driver: no output, the record, `expect sound`

- **No output under the driver**, on any host. Its clock is virtual: a
  `clock +60000` passes a minute in a moment. Every host reports
  `state.sounds.output: "agent"` under the driver. Development without the
  driver plays.
- **`state.sounds`**:
  - `voices`: the last 64 voices, each `{id, src, at, gain, group, ends, by}`,
    with `state` derived from the clock (`waiting`, `sounding`, `ended`);
  - `live`: the live count;
  - `recorded`: how many voices the runner's record holds (at most 1,024,
    D5);
  - `output`.

  The runner prints `voices`, `live`, `recorded`, and `output: "agent"` under
  the driver. This is done by the new `runner/src/agent/sound.rs`;
  `agent.rs` is 1,415 lines. Outside the driver, each playing host fills
  `output` in its overlay (D7, D8): `web-audio` with the context's `state`
  and `decoded: "16/16"`, or `avaudioengine`, and the `blocked`, `late` and
  `dropped` counts. No operation is added.
- **`clock settle`** waits for no voice: a voice changes no state and holds
  no request.
- **The test step:**

  ```ebnf
  expect-sound = "expect" "sound" ( "has" | "missing" ) STRING
                 [ "at" NUMBER ] [ "gain" NUMBER ] [ "ends" NUMBER ] [ "by" IDENT ] ;
  ```

  - `has` passes when the runner's record holds a voice of that `src` that
    matches every clause given: its effective start for `at`, its gain, its
    end, and its `by` (D4).
  - `missing` is the negation. It mirrors `expect tree has|missing`.
  - `state`'s `sounds.voices` is only the last 64. For this step the driver
    asks `state` for the whole record (`sounds: "all"`). That is a form of
    `state`, not an eleventh operation. The `state` arm passes the request
    on (`runner/src/agent.rs:31`, today `state(runner)`), and
    `agent/sound.rs` reads `sounds` from it.
  - If the asked time is older than the record's first voice, the step fails
    and says so: `voices before t=… are no longer recorded`.
  - Dropped calls are not voices.

  ```contract-test
  test "kicks land on the grid, and Stop cancels the window already scheduled"
    tap "play"
    expect sound has "assets/kit/kick.wav" at 0
    expect sound missing "assets/kit/kick.wav" at 125
    clock +450
    tap "play"
    expect sound has "assets/kit/kick.wav" at 500 by cancelled
  ```

  The second tap runs drums' `togglePlay`, whose stop branch calls
  `stopSounds()`; the 25 ms tick at 450 had scheduled the window to 550.

- **The step reaches four places**, each named in §4:
  - the parser, `contract/syntax/src/parser/steps.rs:423–447`;
  - the step's JSON encoding, `contract/cli/src/lib.rs:602–605`;
  - the driver, `scripts/agent-test.mjs:133–162`;
  - difftest's script reader, `semantics/difftest/src/script.rs:166–173`.
    Its `other` arm returns an error today (`:169–173`). A new arm for
    `expect sound` `continue`s, which works because that match sits in the
    `for` loop.

  The corpus case (D11) has no `expect sound`.

### D11 — Lean and difftest

- Lean issues commands as values (`Observe.lean:93–94`). `playSound`,
  `playSounds` and `stopSounds` are commands to it, printed with their
  positional arguments, `none` included. The runner keeps them in
  `take_commands()` (D5), so `observe`
  (`semantics/difftest/src/observe.rs:134`) agrees with no change.
- `contract lean` ignores `sound` declarations. `type-sound-undeclared` is the
  checker's, and Lean's `LowerCheck` does not model files.
- The voice table (ids, cuts, the bound) is not in Lean. It is tested by
  runner tests and by conformance across three runtimes (§5).
- The corpus gains `commands/play-sound.contract`. The generator emits the
  three commands with literal declared sources.

### D12 — `rules/DEFERRED.md`

Recorded in its own commit, under **Components** beside `video`:

> **Expanded (LLP 1096; admitted by the orchestrator under Charlie's
> 2026-10-04 delegation, "make decisions without me"):** short sounds an
> action can schedule: a `sound` declaration (a WAV in `assets/`, at most 10
> s), `playSound(src, at=, gain=, group=)`, `playSounds(hits)` and
> `stopSounds(group=)`. The runner keeps the voice table. The web plays
> through Web Audio, and Apple through one mixer in an arm loaded on demand.
> Linux and Windows keep the record. `app.json`'s `audio_session` picks the
> Apple session.
>
> - **Consumers:** drums (R1–R3), snake (F1), trivia (F5).
> - **Unblocks:** a retriggered, sample-accurate hit, without two media
>   elements per voice or a 2 ms poll.
> - **Take (offered in LLP 1096 §9 Q1, accepted by the orchestrator):** the
>   `audio` element stops being the sound-effect path. R1's retrigger change
>   to the media glue is not built, and the guide's "Ding" recipe and LLP
>   1042 §8's sound-effect paragraph are deleted. Round-1 review A held that
>   neither is doing-list work. To the extent it is not, the admission stands
>   on the orchestrator's waiver under the same delegation.
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
  `playSound`, and adds the window clock (D3) with drums' `start` and
  `tick`. It keeps one sentence for `audio`: a player for long media
  (jukebox). `:222` names the three commands' named arguments.
- `contract-grammar.md` gains `sound`, the three commands and `expect sound`.
- `reference.md`'s host table gains a row: web and Apple play, Linux and
  Windows record. Its manifest section gains `audio_session`.
- `agent-pitfalls.md` gains three entries:
  - "a scheduled sound plays even after Stop unless the action calls
    `stopSounds`";
  - "the first sound on the web needs a user activation";
  - "schedule the downbeat from the press, not the first tick".
- LLP 1042 §8 gains a pointer here.

**Adoption** (outside the repo, `EXACT_APP_DIR`, driven on the web and macOS).

- **Drums** (stage 2 on the web, stage 3 on macOS):
  - 16 `sound` lines;
  - the 16 `audio` elements, `elementOn` and the alternating `hit` go;
  - a `fn hitsBetween(song, from, to)` returns the window's `Hit`s, with
    the track's gain and its group (each track cuts its own last hit, as the
    two elements did; the closed hat joins the open hat's group);
  - `togglePlay` schedules the first window and stops with `stopSounds()`;
  - `preview` calls `playSound` now;
  - The Summary's gated `task clock when playing`, an indented `every(25,
    tick)` block (LLP 1092 D7), replaces `every(2, tick)`. This needs LLP
    1092 stage 2. Until it lands, drums keeps `every(2, tick)`, and
    its `tick` schedules the window whenever `now() + 100` passes
    `scheduledTo`.

  Its tests replace the `hits` derive with `expect sound`.
  `tools/timing-web.mjs` wraps `AudioBufferSourceNode.prototype.start` and
  compares each `when`, mapped back through D6, with the grid.
- **Snake** (stage 2): a WAV each for eat and crash, played when the setting
  is on. The "silent build" label goes.
- **Trivia** (stage 2): its two tones become WAVs played from the answer's
  action. The 1×1 `video` and the `audioFailed` filter go. `haptic` stays.
  Its confetti canvas no longer sets the session category (D8).

## 4. Effect on each implementation

Every file stays under 1,500 lines: new behaviour goes in new files, and
files near the cap gain only call sites.

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| syntax | `parser/sounds.rs`; the arm and message in `parser.rs` (1,450); `expect sound` in `parser/steps.rs` | — | — |
| types, lower, plan | three names in `HOST_COMMANDS` (wherever LLP 1089 leaves it); early checkers; `command_args`; `lower/src/sounds.rs`; plan `sounds` | — | — |
| bake | — | — | `receipt.rs` `loads` gains `"sound"` |
| runner | `sound.rs`, `take_sounds`, the `is_ok()` block and `poison()` in `commit.rs` (1,090); `agent/sound.rs` | — | — |
| JS target | `sounds.js`; two lines in `rt.js`; `agent.js` state | `sound-glue.js` import, `build.mjs:266` | — |
| web wasm | — | `sound-glue.js`; `data-audio-session` on every root-div writer and in both cut needles; batch op; two drains in `host.rs` (1,497); the skip and state in `glue.js` (1,495) | — |
| Apple | — | — | `soundarm/` (Swift, C and header), `SoundModule.swift` (a macOS and an iOS/tvOS path), `AudioSession.swift` (iOS and tvOS only); `CanvasSeams.swift`; two drains in `host.rs` (1,465); the skip and the op in `Session.swift` (1,486); `Agent.swift` state; `build.mjs` gate, compile, `loaded`, copies and `wrapFramework`; `app.schema.json` and `compat.rs` for `audio_session` |
| Linux, Windows | the skip and the drain in `presenter.rs` (1,488) | — | — |
| driver, tests | `contract/cli/src/lib.rs` encoding; `scripts/agent-test.mjs`; `semantics/difftest/src/script.rs`'s `continue` arm; the `state` arm in `runner/src/agent.rs` | — | — |

## 5. Tests

- **Compiler** (`contract/cli/tests/it/sound.rs`, `contract/corpus/rejects.txt`):
  - a declared 16-bit WAV, a float WAV with a `fact` chunk and a `LIST` chunk
    before `data`, and an extensible WAV are accepted;
  - each `lower-sound-*` refusal is asserted whole: an mp3, 24-bit PCM,
    three channels, 11 s, outside `assets/`;
  - `type-sound-undeclared` for a literal, and none for a computed `src`;
  - `playSound("…", at=…, gain=…, group=…)` and `stopSounds(group="hat")`
    pass the checker;
  - `gain=1.5` refused; an unknown option refused;
  - `playSounds` with a list missing `at`, or with `gain` as a string,
    refused;
  - `sound` as a state name still parses;
  - `expect sound` parses and encodes.
- **Plans.** Every in-repo app's plan decodes, with an empty `sounds` table
  and a changed digest.
- **Bake.** A plan with a `sound` has `"sound"` in `loads`; one without has
  none.
- **Runner** (`runner/src/sound/tests.rs`):
  - retrigger: three calls in a commit make three voices, and a
    `playSounds` of three makes the same;
  - a past `at` starts at the commit's time and journals `(asked …)`;
  - a group cuts by start time whatever the order of calls; a tie goes to
    the later call; each `by` is produced;
  - `stopSounds()` and `stopSounds(group=)` end the sounding and cancel the
    waiting;
  - the 33rd live voice is dropped and journaled, while a voice that ended
    frees its place;
  - an undeclared computed `src`, a NaN `at` and a NaN `gain` are dropped
    with their lines, and the commit stands;
  - a refused commit (a settlement failure after a `playSound`) issues no
    sound line and no `SoundOp`; poison drops them; reload ends every live
    voice;
  - `take_commands()` still carries the authored commands;
  - one `advance(60_000)` and sixty `advance(1_000)`s give equal tables;
  - `timer_due_ms` ignores voices;
  - the record keeps 1,024 voices and reports eviction.
- **Conformance.** `host/web-js/conformance/sounds.contract` and `.steps`
  compare the journal and `state.sounds` across the wasm runner, the JS
  runtime and the Linux host, all under the driver (`output: "agent"`): a
  window clock, a choke, a stop, the bound, no `refused:` line on the JS
  target.
- **Web output** (`host/web/tests/sound-glue.test.mjs`, in Chrome, Firefox
  and WebKit by the async lane):
  - with an `OfflineAudioContext` at 48 kHz, a voice at 250 ms renders its
    first non-zero sample at frame 12000, with gain applied;
  - a group cut ends it at the next voice's frame;
  - a voice mapped 40 ms into the past starts at once and does not throw;
  - with a `{0, 0}` timestamp, a due voice starts with `start(0)`, a future
    one is held with no node, an `End` during the hold sets `stopAt` and calls
    no `stop`, and the held voice starts and stops by the formula once the
    timestamp is live, or by the glue's timer if its time comes first;
  - in Firefox's suspended state with a live `performanceTime`, the formula
    schedules the press's window;
  - with `data-audio-session="playback"`, WebKit's `navigator.audioSession.type`
    is `"playback"`, and the other two set nothing;
  - a voice before activation is blocked and journaled.
- **Apple.**
  - An XCTest renders the C callback offline. Given a timestamp, it places a
    voice's first frame and its cut at the frames D6 computes, at an output
    rate (48 kHz) that differs from the file's (44.1 kHz). It sums two
    voices, frees a slot at its end, ignores an `End` for a freed slot, and
    counts a full ring.
  - A device check in stage 3. A tap on `mainMixerNode`, the node connected
    to the output (`AVAudioNode.h:100–103`), finds the first non-zero sample
    of a voice scheduled at `at`, at `frameIndex` in its buffer.
    - Its **speaker time** is the tap buffer's `AVAudioTime.hostTime`
      (converted as D6 converts), plus `frameIndex / sampleRate`, plus the
      presentation latency still downstream of the tap: the output node's
      processing and the device.
    - The check passes only when that time is within 1 ms of
      `t0 + at/1000`.
    - D6's source term is dropped only when the measured onset is early by
      exactly that term. A late onset fails the check.

    Drums is also heard on a Mac and an iPhone.
  - An `.ipa` of drums carries `Frameworks/ExactSound.framework`, and no loose
    `libexact_sound.dylib`.
  - ExactKit builds for macOS with `AudioSession.swift` present.
  - A production build (release, store level 0) of drums carries the sound
    arm; one of an app with no sound does not. The loose
    `libexact_sound.dylib` is the Mac and simulator product, and
    `ExactSound.framework` is the `.ipa`'s (`wrapFramework`,
    `build.mjs:364–375`).
- **Lean.** `lake build`; difftest `corpus` and `random --count 500` (async
  lane).
- **Driven.** D13's adoptions, and the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks.

1. **2026-10-06, the model** (D1–D5, D9–D11, D12's record). Compiler, plan,
   runner, the JS target's table, Linux and Windows draining, the driver,
   conformance.
   - **Exit:** §5's compiler, runner and conformance tests; the plans decode.
2. **2026-10-07, the web output** (D6, D7) and the web adoptions (D13).
   - **Exit:** the web output tests in all three engines; drums, snake and
     trivia tests on the web with `expect sound`; drums' `timing-web.mjs`
     showing each `when` on the grid; heard in Chrome, Firefox and Safari.
3. **2026-10-08–09, Apple** (D6, D8, the bake gate).
   - **Exit:** the mixer XCTest, the device check, the production-build
     check; drums on macOS and the iOS simulator, heard; drums' tests under
     `test macos`.
   - If the iOS session's interruption handling slips, macOS lands and the
     slipped piece gets a `QUEUE.md` line.
4. **2026-10-09, docs** (D13), after stage 2's adoptions run.

## 7. Deferred, with preconditions

- **Linux output.** Needs a consumer that ships sound on Linux, and a ruling
  on linking ALSA or PipeWire (or a pure-Rust ALSA client) as a separate
  artifact, never a feature on `exact-linux`.
- **Windows output.** Needs a Windows consumer with sound. It would be a
  separate artifact, its WASAPI code from `game/audio/src/windows.rs`.
- **`stopSounds(at=)`, fades, gain automation.** Both outputs could do them
  at the sample (Web Audio's `stop(when)` and `setValueAtTime`; the mixer's
  end frame). Needs a consumer, such as a stop at the bar's end or a fade.
- **Pitch and rate, pan, loops.** Needs a consumer that a WAV per variant
  cannot serve.
- **Synthesis** (snake F1's `playTone(frequency, ms)`, an `OscillatorNode`).
  A declared WAV beep serves F1. Needs a tone whose parameters come from
  state.
- **Compressed formats** (m4a, mp3, ogg). Needs a consumer whose WAVs are too
  big to bundle, and a duration reader per container.
- **Sound from a data module** (PCM a source computes). Needs a consumer, and
  a byte-transfer design (LLP 1027.003 §9's typed buffers).

## 8. Considered, not taken

- **Fixing `audio`'s retrigger instead** (R1's glue change). It fixes
  retrigger but gives no scheduling and no choke, and it makes a media
  element into an instrument.
- **A `sound` element** with a hit counter. Not HTML. It ties a one-shot to a
  node's lifetime.
- **The game add-on's audio crate in the core.** The DEFERRED game entry
  forbids it, and its outputs start on ticks, not at a `when`.
- **`AVAudioPlayerNode` pools** (D8). A buffer already scheduled cannot be
  stopped at a future sample. `scheduleSegment` can end a voice whose cut is
  known when it is scheduled, but not a choke that arrives on a later tick.
- **A Swift render block.** The realtime-safe initializer is
  Swift-unavailable (§2).
- **Queueing a voice until activation** (D7): it bursts on the first tap.
- **Refusing the commit when a voice overflows the bound.** That is LLP 1092
  D4's rule for a write, where dropping loses data. A dropped sound loses
  nothing durable, so it is dropped and journaled, as `postMessage` drops
  past its bound.
- **Output under the driver's `clock +N real`.** That clock moves in 50 ms
  steps, so a mapped voice would jitter by a step.
- **A fixed unroll of `playSound`s per tick** (round-1 review A's
  alternative). With no loop, it caps the hits per window at the unroll, and
  every track multiplies it. `playSounds` takes the list a `fn` computes.

## 9. Questions decided (the orchestrator, for Charlie, 2026-10-04)

Every r1 recommendation was accepted:

1. **Admission:** by the take offered, recorded with the waiver that covers
   it (D12, and round-1 review A's finding 3).
2. **The bound:** 32 live voices.
3. **Formats:** 16-bit and float WAV.
4. **Length:** 10 s.
5. **The session:** `.ambient` by default; `audio_session: "playback"` (D8).
6. **Output under the driver:** none.
7. **Globs in `sound`:** no.
8. **`stopSounds(at=)`:** deferred (§7).

No question is open in r3.

## 10. Revisions

- **r4** (2026-10-04, accepted). Grok 4.7 xhigh's final review of r3
  (`llp/reviews/1096-r3.grok.md`, NOT READY: two MATERIAL, three MINOR,
  four NIT). Three rounds are done, so its fixes are folded as given,
  unreviewed; the implementation review checks them. The code confirmed each
  one (`VideoModule.swift:22–30`, `build.mjs:1210–1216`,
  `script.rs:166–174`).
  - **D8:** the macOS loader; the dylib in `loaded`; `audio_session` in the
    schema and the compatibility id.
  - **D6, §5:** the device check taps `mainMixerNode` and adds the frame
    offset and the downstream latency; the source term is dropped only on
    an early onset, and a late one fails.
  - **D6:** one exit for a held voice.
  - **D7:** `data-audio-session` on every root-div writer and in both cut
    needles.
  - **D10:** the `state` arm passes `sounds: "all"`; difftest's `continue`
    arm.
  - **§1:** both press probes stated.
  - **§5:** the Mac and `.ipa` artifact names.

- **r3** (2026-10-04, round 2 of 3). It resolves Grok 4.7 xhigh's delta
  review of r2 (`llp/reviews/1096-r2.grok.md`, NOT READY). Each finding was
  checked against the code. Findings 1 and 2 were re-measured at the press in
  Chrome, Firefox and WebKit (§1).
  - **D6, D7:** a held voice is a record with no node, and an `End` only sets
    its `stopAt`; "live" means a live `performanceTime` whatever the
    context's state, which covers Firefox's press; a timer starts a held
    voice whose time comes first; Apple's host ticks are converted.
  - **D8:** `.ipa` framework wrapping and `embeddedModule`;
    `AudioSession.swift` is iOS and tvOS only; the C header; ring ops applied
    before mixing.
  - **D5, D10:** the JS `state` reply carries the table; one apply in
    `rt.js`; current `commit.rs` locators; ends from the effective start; the
    runner clamps a computed gain.
  - **D7:** `data-audio-session` feeds `navigator.audioSession.type`.
  - **D13:** the adoption points at LLP 1092's block form.
  - **§5:** the device check measured at the output node.

- **r2** (2026-10-04, round 1 of 3). It resolves both Grok 4.7 xhigh reviews
  of r1, whose dispositions are in `llp/reviews/1096-r1.grok-{a,b}.md`. Each
  finding was checked against the code. Review A's Web Audio finding was
  re-measured in Chrome, Firefox and WebKit (§1).
  - **D6:** a clamped `when`; the `{0, 0}` timestamp rule; the latency
    fallback corrected; Apple's onset indexed at the output rate after
    `outputPresentationLatency`.
  - **D3:** the downbeat from the press; the window clock.
  - **D2:** `playSounds(hits)`; early checkers; typed `none`s; the list's home
    after LLP 1089.
  - **D5:** the table applied inside the commit's `is_ok()` block; drops for
    a non-finite `gain`; reload's `End`s; a 1,024-voice record.
  - **D4:** `by` named for every ending.
  - **D7, D8, D9:** every host's skip, both drains, the loaders, the state
    overlays.
  - **D8:** the bake's `loads` and the production gate; a C render callback
    and its ring; one ExactKit session owner.
  - **D10:** `output: "agent"` on every host under the driver; `expect sound`
    with `ends` and `by`, read from the record; the four places the step
    reaches.
  - **D12:** the admission as recorded.
  - **D13:** the fallback until LLP 1092 stage 2.
- **r1** (2026-10-04): first draft.
