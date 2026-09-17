# exact-game-audio

The engine owns saved sound descriptions, play events, loop reports and master gain.
This crate reads that state and executes it. Agents and headless Linux use the
zero-sized, discarding `NullOutput`. Tests opt into `RecordingOutput`. Neither opens
a device. Apple FFI, teardown order and the SPSC implementation remain unchanged.

```rust
use exact_game::{audio::{self, Audio, Sounds, Synth}, World};
let mut world = World::new(60, 0);
world.register_audio(); // register before restore too
world.resource_mut::<Sounds>().add("hum", Synth::sine(220.0).seconds(2.0));
let voice = world.play("hum").ui().pitch(0.94).gain(0.8).start();
world.resource_mut::<Audio>().master = 0.5;
audio::stop(&mut world, voice);
```

A play builder commits exactly once, either at `.start()` (returning a `VoiceId`)
or on drop at statement end. Pitch is playback rate, bounded to 0.01..16. The saved
voice holds its definition, rate, lifetime and last known world position. Edits to
a definition affect later plays; an attached one-shot survives despawning its
entity. `at_point(Vec3)` is stationary; `ui()` bypasses spatialization. Games can
set `Audio.master` each tick from `world.args().number("volume")`.

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

The Player cache keeps the current revision per sound plus revisions currently
used by device voices. Web buffers follow that cache and active source references.
Apple retains PCM while commands or voices can reference its raw pointer. A Stop
command's sequence is acknowledged through the return SPSC ring; only the main
thread then releases the Arc. A full return ring coalesces and retries its latest
watermark. Device disposal still joins callbacks before dropping any PCM.

## Outputs

Web constructs an initially suspended `AudioContext`. Invoke and await
`output.unlock(&mut transport)` from a trusted input handler; it awaits the
`resume()` promise and bumps generation only on success. No sources are created
before success, including when the browser suspends again. No forgotten closures.
Each voice connects buffer source → gain → stereo panner → destination. Playback
rate implements pitch; gain and pan use `setTargetAtTime` with a 10 ms time constant.
The Web probe demonstrates this ownership and asynchronous API.

Apple has 32 fixed voice slots. `start_at`/`set`/`stop` enqueue producer-side work;
`flush` retries it (Player calls flush each sync). Pending Set commands coalesce by
identity, latest wins; Start and Stop stay ordered and are never discarded on ring
pressure. A stalled device cannot panic the producer. The host calls
`AppleOutput::suspend()`/`resume()` for interruption notifications, updates playing,
and increments generation after resume. The host still owns AVAudioSession policy.
The callback allocates nothing, locks nothing, and performs no reference counting.
It linearly resamples, ramps stereo gains over 10 ms, maps non-finite samples to
zero, sums linearly and clamps only outside [-1,1]. Quiet/full-scale authored gains
therefore agree with WebAudio instead of being compressed by `x/(1+abs(x))`.

The controls retry queue can grow during an unbounded stream of starts/stops while
a device is stalled; this is the deliberate cost of the requested never-drop rule.
The coalesced Set table is bounded by the Player's selected voices.

## Synthesis and proof

Synthesis uses fixed operation order, libm and a local xorshift32 noise stream.
Oscillators, envelopes and filters are unchanged. Rendered PCM is made finite and
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

| Sound | Previous PCM hash | AU2 PCM hash |
|---|---|---|
| chime | `f674dcb28a6f2d96` | `f674dcb28a6f2d96` |
| footstep | `a0e63ab8aeb11a78` | `a0e63ab8aeb11a78` |
| thud | `61001743ff4a6326` | `61001743ff4a6326` |
| wind | `47138b008334dd51` | `926b35610b65f1ec` |
| night-sting | `91841647939b0610` | `91841647939b0610` |

Only wind is expected to move, due to its new loop overlap. The original one-shot
wind hash and chime hash remain pinned by tests.

AU2 validation (2026-09-17): native build, native and wasm audio clippy with
`-D warnings`, and Apple-output checks for aarch64 macOS/iOS pass. All 21 audio
crate tests and the five engine audio tests pass on both arm64 Mac and x86-64
Linux. Both complete scoped suites report 121 passed, one failed: the Greybox
state golden under `game/games/` still expects `sources: 0` instead of `sources:
[]`; that excluded directory was not edited. All five demo PCM hashes above match
between machines. Audio-file rustfmt, caps and boot pass. The complete package
fmt check flags concurrent presentation-generation edits in engine `sim.rs` and
`world.rs`, outside these audio changes.

Final audio source line counts: engine `audio.rs` 690; executor `lib.rs` 368;
`apple.rs` 634; `web.rs` 135; regression files `player.rs` 530 and `apple_tests.rs`
132. The clock integration adds five lines across `world.rs` and its existing
`begin_tick` hook. No new scripts, checks, device frameworks or sub-agents.
