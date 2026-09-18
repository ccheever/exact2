# exact-game-audio

The engine owns saved sound descriptions, play events, loop reports and master gain.
This crate reads that state and executes it. Agents and headless Linux use the
zero-sized, discarding `NullOutput` when explicitly testing the executor. Ordinary
headless simulation needs no Player. `RecordingOutput` records device commands;
neither test output opens a device. NullOutput has zero capacity and skips selection, synthesis and caching.

```rust
use exact_game::{audio::{self, Audio, Sounds, Synth}, World};
let mut world = World::new(60, 0);
world.register_audio(); // register before restore too
world.resource_mut::<Sounds>().add("hum", Synth::sine(220.0).seconds(2.0));
let voice = world.play("hum").ui().pitch(0.94).gain(0.8).start();
world.resource_mut::<Audio>().master = 0.5;
audio::stop(&mut world, voice);
```

A `#[must_use]` play builder commits only at `.start()` (returning a `VoiceId`).
Dropping it creates no voice or playback journal event. Pitch is playback rate, bounded to 0.01..16. The saved
voice holds its definition, rate, lifetime and last known world position. Edits to
a definition affect later plays; an attached one-shot survives despawning its
entity. `at_point(Vec3)` is stationary; `ui()` bypasses spatialization. Games can
declare `#[live] volume: f64` in the game's Args struct and set `Audio.master`
from `args.volume` in tick.

A setup voice begins at tick 0; a voice authored inside tick N begins at boundary
N+1. A non-saved phase flag at the existing world tick hooks makes this distinction
explicit. Pruning uses that same completed-boundary convention.

Call `audio::step` after game logic. It prunes finished voices, records source
changes and removes extra listeners in entity order. The saved `Audio` resource
holds the last reported source states, so restoring does not repeat loop events.
Journal examples: `sfx chime at lantern-3 gain 0.80`, `loop wind on gain 0.30`,
`loop wind off`. `state.world.audio.sources` lists sound, entity, gain and playing.
Finite voices are ambient and do not block `clock settle`; neither do loops.
Invalid source gains clamp to 0..4 (non-finite becomes zero), with at most one
refusal per offending source identity. Play and master gains use the same bound.

The frame owner constructs `Player::new(output, 48000)` and passes explicit state:

```rust,ignore
player.sync(world, Listener::from_world(world), transport);
```

`Transport { generation: u64, playing: bool }` is owned by the host. Increment its
generation on every seek, restore, world rebuild and successful output unlock.
Set playing false while paused. Each sync reads it: a changed epoch stops all
outputs and starts current voices at world-time offsets; pause stops outputs and
resume resynchronizes. There is no manual Player reset. A fresh player behaves
the same way. Ended PCM never starts, including the rounded final lifetime tick.
Offsets include pitch. Loops use world-time phase modulo their rendered length.

Capacity belongs to the output. Player ranks loops first, then louder voices
(maximum stereo gain), then newer start boundaries, then larger stable identities.
It stops priority losers before starts and walks candidates until the output has
accepted `capacity()` voices. A budget or device refusal cannot starve a smaller,
lower-priority sound, except while a preferred candidate waits for stopped PCM to
be acknowledged: non-preferred candidates stay stopped so they cannot reclaim that
capacity. A dropped loop returns at its current phase.
There is no callback stealing. Final per-channel gains are sanitized to 0..4 at
this shared boundary. Missing ears silence spatial sounds. Distance gain is
`1/max(distance,1) * (1-smoothstep(1,40,distance))`; local +X is right and pan is
equal-power. UI gains apply equally to both channels on both executors.

Definitions are immutable shared values with a content revision computed at
registration or restore. Finite voices retain their frozen definition; sources
resolve their name in the current registry and require a looping definition. A
non-looping `AudioSource` is refused by sound name at presentation; use `World::play`
for finite sounds, which records a deterministic activation tick. This avoids adding
activation state to attached sources or measuring their offset from world tick zero.
`AudioSource::new("wind").gain(0.3)` starts playing by default. After explicit
`register_audio()` in setup, `world.play("chime").at(entity)` needs only `&World`,
so it can run while an unrelated component is borrowed.

Player reserves a **32 MiB PCM budget before synthesis** (`samples × 4`), counting
a shared allocation once. It keeps active allocations and allocations still owned
by an output; acknowledgement releases the latter. Registry membership alone does
not retain PCM. Registration validates definitions; the 60-second duration limit
already keeps every valid definition below 32 MiB at 48 kHz. The Player checks the
aggregate budget at its actual output rate before rendering.
Web buffers follow active source references; a failed start leaves no cached PCM.
Apple retains PCM while commands or voices can reference its raw pointer. A Stop
command's sequence is acknowledged through the return SPSC ring; only the main
thread then releases the Arc. A full return ring coalesces and retries its latest
watermark. Device disposal still joins callbacks before dropping any PCM.

## Outputs

Web may construct `AudioContext` on the first live frame or an earlier gesture.
`Output::unlock` calls `resume()` synchronously on the first trusted key/pointer
down stack, never for a seekable surface and never twice while activation is pending
or the output is ready. Its owned async completion enables playback; the next sync
bumps the transport generation. Lifecycle resumption can request activation again.
The probe uses this production path and observes readiness. There is no second async
unlock API. A late resume completion suspends again if the surface became hidden.

`Output::start(id, pcm, rate, looping, offset, pitch) -> bool` is the single start
operation. WebAudio failures return refusal for retry instead of trapping the module.
Each voice connects buffer source → gain → stereo panner → destination. Playback
rate implements pitch; gain and pan use `setTargetAtTime` with a 10 ms time constant.

Apple has 32 fixed voice slots. `start`/`set`/`stop` enqueue producer-side work;
`flush` retries it (Player calls flush each sync). Pending Set commands coalesce by
identity, latest wins. Unpublished Start/Stop pairs cancel; published commands
retain their order and PCM until acknowledged. Published starts occupy at most 32
slots, including stopped voices awaiting acknowledgement. Up to 32 newer selected
starts stay in the coalescible producer queue. Across both windows, retained PCM
has a **32 MiB total byte budget**, counting each shared allocation once. `Pending::start`
refuses before adding ownership or a command when the next unique allocation does
not fit. A refusal leaves the Player voice inactive for retry, without changing
transport. Stops release bytes only after acknowledgement (or immediately when
cancelling an unpublished start). Player accounts for these same retained allocations until acknowledgement. An
unpublished same-id replacement subtracts its releasable allocation before checking;
a published replacement cannot release bytes until the callback acknowledges it.
Stops can pass unpublished starts to release capacity; sequence watermarks are
assigned at publication. A full mixer leaves a start unconsumed and unacknowledged.
Finite voices retire on the exact terminal sample step, including silent callbacks.
The callback allocates nothing, locks nothing, and performs no reference counting.
It linearly resamples, ramps stereo gains over 10 ms, maps non-finite samples to
zero, sums linearly and clamps only outside [-1,1]. Quiet/full-scale authored gains
therefore agree with WebAudio instead of being compressed by `x/(1+abs(x))`.

The callback drains commands even on null/unsupported output layouts. It always
advances the mixer for the full quantum, writing interleaved or planar stereo and
discarding samples while silencing other layouts.
The coalesced Set table is bounded by the Player's selected voices.


## Game surfaces

Set `"audio": true` beside `game.crate` and `game.type` in the app manifest. The
synthesized GPU shell adds the audio dependency and invokes
`exact_game_render::module!(Game, audio)`. The silent form uses `WorldSurface<G, ()>`;
the audio form supplies a small generic presentation hook, so render has no audio
dependency and no feature switch. No adapter crate is needed.

The surface syncs after each render's simulation advance, with its presentation
generation and `!paused`. `SurfacePlayer` owns a separate live/seekable clock flag:
a live input can construct WebAudio and invoke resume on the trusted gesture's
stack before any frame; a seekable input constructs nothing. Seekable frames close
any previous device. Native headless modules start seekable; other native targets
use NullOutput. Failed device creation retries every 300 live sync frames, with
one warning per surface; seekable frames preserve that cooldown.

The GPU seam is `Surface::lifecycle(Lifecycle)` (default no-op), with Hidden,
Visible, Interrupted and Resumed. The latter pair means an external interruption of
a surface's device work, applicable to video decoding or a chart ticker as well. `gpu_lifecycle(id, code)` maps codes
0–3 and ignores unknown values. `Surface::clock(bool)` is defaulted and receives
clock ownership on creation and each change. Neither callback advances simulation.
Web delivers visibilitychange/pagehide/pageshow; persisted pageshow restores
visibility even when `document.hidden` lags, and a later visibilitychange corrects
it. Web has no external-interruption source. ExactKit delivers aggregate visibility
and iOS audio-session interruptions on main; headless Linux has none to deliver.
macOS hide/occlusion stops work; losing focus alone does not hide a visible window.
iOS requires an active app and a mounted window. New surfaces receive the same
aggregate, with notifications delivering only transitions.
The presentation hook combines hidden/interrupted state before suspending output;
resume resets playback from the current tick offset with one readiness epoch bump.
ExactKit recognizes the `exact:audio` surface message and configures/activates one
process audio session only for a live requesting surface. Failed activation retries
at 300 live display-link frames, or on a new Visible transition or explicit gesture;
failed activation never emits Resumed. An interruption ending without `shouldResume`
blocks automatic resume process-wide, including later lifecycles and automatic
`exact:audio` requests. A subsequent Hidden → Visible transition or trusted key/pointer
down can resume. ExactView window attach/detach refreshes aggregate visibility.
Other hosts consume
the request without app dispatch. SurfacePlayer returns lifecycle failures and the
GameAudio forwarder reports them. Failed AudioUnit Stop disposes the device so work
ends; failed Start drops it into the same 300-live-frame retry as creation failures.

`module!(Game, audio)` forwards `wants_audio`, `clock`, `suspend`, `unlock` and `sync`
through GameAudio in `game/render/src/lib.rs`; these forwarders are wired.

## Synthesis and proof

`exact_game_audio::render(&Synth, sample_rate)` generates PCM here, beside playback.
The engine retains saved definitions, validation, durations, voices and events.
Synthesis uses fixed operation order, libm and a local xorshift32 noise stream. Rendered PCM is made finite and
clamped to ±4 before caching. `Synth::looped()` adds a 10 ms equal-power overlap of
tail into head, removes that overlapped tail, and leaves one-shot renders alone.
The demo uses it for wind. The seam now connects adjacent samples from the original
tail. The loop is 480 samples shorter at 48 kHz; its period is the rendered length.

```sh
cd game
EXACT_UPDATE_TRUST=development cargo build -p exact-game -p exact-game-audio
EXACT_UPDATE_TRUST=development cargo test -p exact-game -p exact-game-audio --no-fail-fast
EXACT_UPDATE_TRUST=development cargo clippy -p exact-game -p exact-game-audio --all-targets -- -D warnings
EXACT_UPDATE_TRUST=development cargo clippy -p exact-game-audio --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo fmt -p exact-game -p exact-game-audio --check
EXACT_UPDATE_TRUST=development cargo check -p exact-game-audio --target aarch64-apple-ios
EXACT_UPDATE_TRUST=development cargo check -p exact-game-audio --target aarch64-apple-darwin
cargo run -p exact-game-audio --example demo -- /private/tmp/exact-au2-demo
```

The demo writes 16-bit mono WAV. Optional `--play` uses the real macOS output;
validation does not claim listening or device interruption testing. Regression
cases are marked AU2.1–13 in `tests/player.rs` and `src/apple_tests.rs`; callback
and queue scenarios use the real mixer without opening a device. The Player cases
use RecordingOutput. The existing million-transfer SPSC ordering test is retained.

| Sound | Current PCM hash |
|---|---|
| chime | `b0df0c973190c087` |
| footstep | `3fb464e28571d7c1` |
| thud | `212e79c768c1fa8b` |
| looped wind | `0a81380038622d05` |
| night-sting | `bde9e602c3ce2406` |

D1b typed-bulk cards agree on arm64 macOS, x86-64 Linux and Chrome 153 wasm
(2026-09-17), for all five demo sounds. The browser ran the demo definitions
through a temporary wasm card and chime through `null_probe`'s `chime_hash()`
export; no audio device is opened. The chime pin is in
`src/synth_tests.rs`; the unlooped wind pin is `c72651fc30eafc30` in `tests/player.rs`.
Typed bulk f32 hash framing changed these hashes without changing the generated samples.

AU3 diagnostic (64 distinct two-second voices, macOS arm64 dev profile):

| Output | Cold sync before → after | Warm sync before → after | PCM cached before → after |
|---|---|---|---|
| Null | 75.020 ms → 0.010 ms | 66.648 µs → 0.028 µs | 64 → 0 |
| Capacity 32 | 74.481 ms → 39.084 ms | 64.624 µs → 7.226 µs | 64 → 32 |

Run `cargo test -p exact-game-audio --test player sixty_four_voice_sync_timing -- --ignored --nocapture`.
The timing is diagnostic, not a threshold. Greybox's 1.5 s forward pin is
`0x0f14b8b231091d12`, position `(0, 0.9, -5.3666644)`, matching the current
native golden and AU3c web deterministic proof.

Run `bun game/bench/probes/audio.mjs greybox` after the web proof, or use
`EXACT_AUDIO_PROBE=1 bun game/games/greybox/proof.mjs web`. The shortcut closes the
deterministic session before opening a separate live browser. The probe is under
`game/bench`, excluded from the proof's build-input digest. It verifies both orderings:
a completed live frame before input, and a fresh production surface created during
a trusted gesture before its first frame. Neither rAF nor ResizeObserver callbacks
are delayed. Both invoke resume exactly once on the trusted stack. It records context
construction and input-to-next-frame costs, observes running device time and nonzero
wind analyser RMS, and injects a refused start to prove retry without a wasm trap.

Evidence goes to `game/games/greybox/artifacts/audio-web.json`. Synthetic page events
prove suspension/resumption and simulation isolation; the separate listener fixture
checks persisted pageshow with `document.hidden` still true. This is not a claim that
Chrome actually admitted the page into bfcache. Teardown owns and awaits its browser
process group, closes server connections, and removes the profile, including injected
setup failures. Greybox, Beacons and the asset fixture pass on web with unchanged pins.

All synth numbers must be finite. Duration is 0..=60 seconds and sustain 0..=1.
Oscillator gain, frequency, vibrato frequency/depth, ADSR times and filter cutoffs
are nonnegative; slide accepts either sign. Validation recurses through layers and
applies to both saved registry and saved voice definitions; refused loads leave
the world unchanged. Opening a `.world` restores saved `Sounds`, including runtime-registered names,
and saved finite-voice definitions. `Sim::restore_bound` owns only app binding policy.
The restore seam distinguishes Open from Carry:
GameAudio overlays fresh setup definitions only on Carry, preserving runtime names;
finite voices retain frozen definitions. Opening a file keeps its saved registry.

Apple callback tests use the real mixer with fixture buffers and open no device.
Compiled macOS Swift fixtures call `interruption(began:shouldResume:)` directly
on the main thread; they do not post `AVAudioSession.interruptionNotification`
from a background queue. They check aggregate visibility, activation retry,
`shouldResume`, recovery broadcast to live lifecycles and the first silent gesture.
AU3d built and linked the macOS `ExactMac` Swift product with a temporary native-build
wrapper. The one greybox macOS proof attempt then failed before launch: the WebKit
helper selected SDK 27 with Swift 6.3.3. There is no macOS device-output claim.
Audio/render Rust libraries and the Swift `ExactKit` target build for iOS; iOS was
not driven and interruption handling remains unproven on a device.

Owed: real-device Apple interruption and multi-display sweeps; WebAudio resume-failure propagation. The fixtures do not establish these claims (`review-F2f-sol.md`, `CanvasSeams.swift:507`; `review-F2f-grok.md`, `game/audio/src/web.rs:47–50,117–119`). The authoring regression now holds a mutable `Transform` borrow across `play().at(entity).start()`. Finite attached sources are skipped and journal a named refusal once in debug and release, without a panic.
