# exact-game-audio

Synthesized mono PCM and spatial stereo playback, with no codec, DSP library or
native audio framework crate. Linux is headless and uses `NullOutput`. An agent
always uses `NullOutput`, on every platform: never construct a device output there.

The engine owns `Synth`, `Sounds`, `Voice`/`Voices`, `AudioSource`, `AudioListener`,
play events and the agent's audio section. This crate only reads the world. Its
output cache, device handles and playback cursors are not simulation state.

```rust
use exact_game::{audio::{self, Sounds, Synth}, World};
let mut world = World::new(60, 0);
world.register_audio(); // setup; also registers the types needed for restore
world.resource_mut::<Sounds>().add("chime",
    Synth::sine(880.0).decay(0.6).seconds(0.8)
        .layer(Synth::sine(1320.0).gain(0.4)));
world.play("chime").ui().gain(0.8);
audio::step(&mut world); // after game logic, each fixed tick
```

`audio::step` is the integration hook for the Sim/game tick owner; this lane does
not edit Sim. It prunes voices at their exclusive `ends` tick and removes extra
listeners in entity order, logging one refusal per removed listener. Finite voices
participate in `clock settle`; loops do not keep settle running forever. Journal
lines use the world's existing `t=… tick=…` prefix, followed by
`sfx chime at lantern-3 gain 0.80`. Reads never open a device or produce samples.

A play builder commits on drop at statement end. `at(entity)` follows the entity's
world position, `at_point(Vec3)` is stationary, and `ui()` bypasses spatialization.
Register sound definitions and audio types in setup before restoring a save.

The frame owner constructs `Player::new(output, 48000)` and calls
`player.sync(world, Listener::from_world(world))`. No listener silences spatial
sounds; UI sounds still play. Distance gain is `1/max(distance,1)` multiplied by
`1-smoothstep(1,40,distance)`. Listener local +X is right; equal-power stereo pan is
computed from the unit direction. A fresh player resumes saved voices at
`floor((tick-began)*sample_rate/hz)`. Ended PCM never starts, even in the rounded-up
last lifetime tick. Call `player.reset()` on an explicit forward seek or restore
when reusing a player; backward ticks reset automatically. Loops use world-time
phase modulo their PCM length. Edits to a sound definition invalidate its cache key.

`Output::start_at` extends start/set/stop for restore offsets. Its default makes a
rotated loop or trimmed one-shot; all three supplied outputs override it, retaining
the original PCM. The cache keeps each authored sound revision until player drop.

## Synth conventions

Durations and ADSR stages are seconds. `seconds` includes release; release begins
at `max(0, seconds-release)` from the envelope's level at that instant. Zero-length
stages are instantaneous. Each layer starts at zero and has its own envelope,
filters and gain; the final length is the longest layer. Layers sum without
normalization. Waveforms are intentionally simple and not band-limited. Noise is a
fixed xorshift32 stream local to rendering, never the world's RNG. Cutoffs of zero
disable the one-pole filters. Rendering uses `exact_game::math` throughout.

## Outputs

Web: one initially suspended `AudioContext`. Call `WebOutput::unlock()` directly
from the first input event. Each voice connects buffer source → gain → stereo
panner → destination. PCM is copied into a cached `AudioBuffer` once per rendered
sound revision. WebAudio errors on start panic with context; construct/unlock
return errors. The added `web-sys` features are:

- `AudioContext`, `BaseAudioContext`, `AudioDestinationNode`
- `AudioNode`, `AudioParam`, `AudioBuffer`, `AudioBufferSourceNode`
- `AudioScheduledSourceNode`, `GainNode`, `StereoPannerNode`

Apple: hand-declared AudioToolbox FFI in `src/apple.rs`, the crate's sole unsafe
module. A fixed 32-voice mixer linearly resamples to the device rate and soft-clips
with `x/(1+abs(x))`. The oldest output ID is stolen when all slots are occupied.
The main thread sends commands through a bounded 1024-entry lock-free SPSC ring;
overflow is an explicit panic rather than blocking either thread. The callback
uses no locks, allocation, deallocation, or reference counting. PCM allocations
are retained on the main thread until device disposal, so the callback only reads
stable pointers. The host owns iOS AVAudioSession policy and interruptions.

## Proof and artifacts (2026-09-17)

```sh
cd game
EXACT_UPDATE_TRUST=development cargo build -p exact-game -p exact-game-audio
EXACT_UPDATE_TRUST=development cargo test -p exact-game -p exact-game-audio --no-fail-fast
EXACT_UPDATE_TRUST=development cargo clippy -p exact-game -p exact-game-audio --all-targets -- -D warnings
cargo fmt -p exact-game -p exact-game-audio --check
cargo run -p exact-game-audio --example demo -- /private/tmp/exact-au1-demo
```

The demo writes 16-bit mono WAV with a 44-byte header; all synthesis is f32 before
quantization. Add `--play` on macOS for two seconds per sound through the real
output. Device playback was **not run** in the sandbox; no claim of listening.

| WAV | RMS (float PCM) | Peak | Engine PCM hash |
|---|---:|---:|---|
| chime | 0.191519 | 0.849471 | `f674dcb28a6f2d96` |
| footstep | 0.067314 | 0.382348 | `a0e63ab8aeb11a78` |
| thud | 0.278227 | 0.798393 | `61001743ff4a6326` |
| wind | 0.049679 | 0.205073 | `47138b008334dd51` |
| night-sting | 0.141875 | 0.640189 | `91841647939b0610` |

All five hashes match on arm64 macOS and the x86-64 Linux builder. The chime hash
is pinned in the engine unit test. Its unwindowed FFT power centroid from the WAV
is **941.998 Hz**; its two partials predict **941.982 Hz**, weighting 880/1320 Hz
by gain² × (attack+decay)/3. Hann-windowed analysis gives 881.202 Hz because the
1320 Hz layer decays much earlier; that window is not the full sound's weighting.

Tests cover oscillator closed forms, the noise sequence, ADSR, DC/Nyquist filter
response, journal formatting, lifetime and listener refusal, saved playback
offsets, pose/pan/attenuation, source start/stop/cache reuse, linear resampling and
soft clipping, and one million ordered SPSC transfers across two threads.

The complete engine test run currently has two Greybox golden failures involving
world hashes from concurrent engine work; the audio-specific tests pass on both
hosts. The audio section's expected `sources: 0` field is updated in its goldens.

Wasm size probes (the repository's `web` profile, pre-wasm-bindgen `.wasm` files,
gzip level 9 with zero timestamp):

```sh
cargo build -p exact-game-audio --example web_probe --example null_probe \
  --profile web --target wasm32-unknown-unknown
```

| Probe | Raw bytes | Gzip bytes |
|---|---:|---:|
| `Player<WebOutput>` | 526,285 | 145,005 |
| equivalent `Player<NullOutput>` | 509,811 | 137,866 |
| difference | **16,474** | **7,139** |

These include the synth, world data and wasm-bindgen metadata, not only the output.
They are raw linked probes, not post-bindgen browser payloads; the installed CLI is
0.2.127 and the workspace resolves bindings 0.2.128. Neither was installed or
replaced just for this measurement.
