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
Finite voices participate in `clock settle`; loops do not keep it running forever.
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

Capacity belongs to the Player. It ranks loops first, then louder voices (maximum
stereo gain), then newer start boundaries, then larger stable identities. It stops
all losers before starting winners; a dropped loop returns at its current phase.
There is no callback stealing. Final per-channel gains are sanitized to 0..4 at
this shared boundary. Missing ears silence spatial sounds. Distance gain is
`1/max(distance,1) * (1-smoothstep(1,40,distance))`; local +X is right and pan is
equal-power. UI gains apply equally to both channels on both executors.

Definitions are immutable shared values with a content revision computed at registration
or restore. Started voices retain that definition. Player reuses selection/PCM scratch,
culls silence and ranks capacity winners before synthesis. Its cache keeps materialized
current definitions plus revisions currently used by device voices. Web buffers follow that cache and active source references.
Apple retains PCM while commands or voices can reference its raw pointer. A Stop
command's sequence is acknowledged through the return SPSC ring; only the main
thread then releases the Arc. A full return ring coalesces and retries its latest
watermark. Device disposal still joins callbacks before dropping any PCM.

## Outputs

Web constructs an initially suspended `AudioContext`. The surface invokes
`Output::unlock` synchronously from key/pointer down; its owned asynchronous resume
completion enables playback, and the next sync bumps the transport generation.
No sources are created before success, including when the browser suspends again.
The explicit probe can also await `output.unlock(&mut transport)`.
Each voice connects buffer source → gain → stereo panner → destination. Playback
rate implements pitch; gain and pan use `setTargetAtTime` with a 10 ms time constant.
The Web probe demonstrates this ownership and asynchronous API.

Apple has 32 fixed voice slots. `start_at`/`set`/`stop` enqueue producer-side work;
`flush` retries it (Player calls flush each sync). Pending Set commands coalesce by
identity, latest wins. Unpublished Start/Stop pairs cancel; published commands
retain their order and PCM until acknowledged. Published starts occupy at most 32
slots, including stopped voices awaiting acknowledgement. Up to 32 newer selected
starts stay in the coalescible producer queue. Across both windows, retained PCM
has a **32 MiB total byte budget**, counting each shared allocation once. `Pending::start`
refuses before adding ownership or a command when the next unique allocation does
not fit. A refusal leaves the Player voice inactive for retry, without changing
transport. Stops release bytes only after acknowledgement (or immediately when
cancelling an unpublished start). The Player cache is separate from this budget.
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
Visible, AudioInterrupted and AudioResumed. `gpu_lifecycle(id, code)` maps codes
0–3 and ignores unknown values. `Surface::clock(bool)` is defaulted and receives
clock ownership on creation and each change. Neither callback advances simulation.
Web delivers visibilitychange/pagehide/pageshow; ExactKit delivers app and iOS
audio-session notifications to every canvas; headless Linux has none to deliver.
The presentation hook combines hidden/interrupted state before suspending output;
resume resets playback from the current tick offset with one readiness epoch bump.
ExactKit recognizes the `exact:audio` surface message and configures/activates one
process audio session only for a live requesting surface (and reactivates it after
an interruption). Other hosts consume the request without app dispatch.

The generated `GameAudio` hook in `game/render/src/lib.rs` still needs three
forwarders (`wants_audio`, `clock`, `suspend`); that file is outside AU3c's supplied
scope. Until that change is authorized, the SurfacePlayer unit regressions pass,
but game surfaces do not yet forward these controls or request the Apple session.

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

AU3 web module size (`web` profile, wasm-bindgen, wasm-opt -Oz; gzip level 9):
693,756 → 818,436 bytes raw; 277,890 → 319,108 bytes gzip. A headed Chrome run
for ten seconds resumed the context and created a wind source with non-zero PCM,
but the audio clock stalled at 5.33 ms and analyser RMS stayed zero. Device output
was **not verified**; the temporary analyser was removed and Chrome closed.

Run `bun game/bench/probes/audio.mjs greybox` after the web proof, or use
`EXACT_AUDIO_PROBE=1 bun game/games/greybox/proof.mjs web`. The shortcut closes the
deterministic session before opening a separate live browser. The probe is under
`game/bench`, excluded from the proof's build-input digest. It asserts context
creation/resume on the first trusted input before the first frame, a running
context with advancing time, nonzero analyser RMS while wind plays, and
suspension/resumption on page lifecycle events. Evidence goes to
`game/games/greybox/artifacts/audio-web.json`; the assertions run each time, with
no dated clock/RMS numbers treated as a contract. Setup and teardown share a
try/finally, kill the owned Chrome process group, await exit, close server
connections with a deadline, and remove the profile.

All synth numbers must be finite. Duration is 0..=60 seconds and sustain 0..=1.
Oscillator gain, frequency, vibrato frequency/depth, ADSR times and filter cutoffs
are nonnegative; slide accepts either sign. Validation recurses through layers and
applies to both saved registry and saved voice definitions; refused loads leave
the world unchanged. A dev carry retains each old voice's definition and overlays
the freshly bound sound registry for subsequent plays.

macOS verification exercises the actual render callback with fixture buffers;
it does **not** open or capture a device. The AU3c macOS greybox proof was attempted
with SDK 26 and a native Swift build wrapper, but SwiftPM failed loading
`BuildServerProtocol` before the host could run. The audio and render Rust libraries
build for `aarch64-apple-ios`; the notification code also compiles with Xcode's
matching iOS compiler. The full iOS host was not linked or driven. The lifecycle
seam supplies interruption handling, which remains unproven on an iOS device.
