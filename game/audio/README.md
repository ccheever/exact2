# exact-game-audio

The engine owns saved sound descriptions, play events, loop reports and master gain.
This crate reads that state and executes it. Agents and headless Linux use the
zero-sized, discarding `NullOutput` when explicitly testing the executor. Ordinary
headless simulation needs no Player. `RecordingOutput` records device commands;
neither test output opens a device. NullOutput has zero capacity and skips selection, synthesis and caching.

```rust
use exact_game::{audio::{self, Audio, Synth}, World};
let mut world = World::new(60, 0);
world.sounds([("hum", Synth::sine(220.0).seconds(2.0))]);
let voice = world.play("hum").ui().pitch(0.94).gain(0.8).start();
world.resource_mut::<Audio>().master = 0.5;
audio::stop(&mut world, voice);
```

A definition is a `Synth` or a `Sample`. Square and saw oscillators are band-limited
(PolyBLEP). A sample names a baked `.sound` asset: put `boom.wav` or `music.ogg` in
`art/` ([the baker](../bake/README.md) writes `assets/boom.sound`), declare the name
in `Game::ASSETS` so setup waits for it, and register it in setup:

```rust,ignore
const ASSETS: &'static [&'static str] = &["boom.sound", "music.sound"];
// in setup:
world.sounds([
    ("boom", Sample::new("boom.sound")),
    ("music", Sample::new("music.sound").looped().gain(0.6)),
]);
```

`sounds` reads the delivered asset's frames, rate and channels into the saved
definition, so voice lengths never need the PCM; an undelivered name panics naming
its declaration. One `sounds` call takes one definition type; call it twice to mix.
The PCM stays in the world's asset store, shared with the outputs, and never enters
a save or the hash: a save records the asset's content identity, and restoring
into a world whose `.sound` differs refuses by name. Samples need only
`game.audio: true`, not the model-capable module.

A `#[must_use]` play builder commits only at `.start()` (returning a `VoiceId`).
Dropping it creates no voice or playback journal event. Pitch is playback rate,
bounded to 0.01..16. `pan(p)` is a balance from -1 (left only) through 0 to 1 (right
only), applied after spatial gains. `offset(seconds)` starts part-way in (wrapping
for loops); `fade_in(seconds)` ramps the gain up from zero. A voice of a looping
definition plays until `audio::stop` or `audio::fade(world, id, seconds)`, which
ramps it to silence and then removes it; loops have no 60-second limit. The saved
voice holds its definition, rate, offset, pan, fade, lifetime and last known world
position; saving and restoring mid-sound resumes at the same world-time offset.
Edits to a definition affect later plays; an attached one-shot survives despawning
its entity. `at_point(Vec3)` is stationary; `ui()` bypasses spatialization. Games can
declare `#[live] volume: f64` in the game's Args struct and set `Audio.master`
from `args.volume` in tick.

A setup voice begins at tick 0; a voice authored inside tick N begins at boundary
N+1. A non-saved phase flag at the existing world tick hooks makes this distinction
explicit. Pruning uses that same completed-boundary convention.

Call `audio::step` after game logic. It prunes finished voices, records source
changes and removes extra listeners in entity order. The saved `Audio` resource
holds the last reported source states, so restoring does not repeat loop events.
Journal examples: `sfx chime at lantern-3 gain 0.80`, `sfx music at ui gain 1.00
offset 0.50 fade-in 1.00 loop`, `sfx music fade 0.50`, `sfx blip stop`, `loop wind on
gain 0.30`, `loop wind off`. `state.world.audio.voices` lists sound, place, gain,
began and ends (`null` while looping), plus offset, pan and the current fade gain
when set; `state.world.audio.sources` lists sound, entity, gain and playing.
Voices are ambient and do not block `clock settle`; neither do sources.
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
this shared boundary. Missing ears silence spatial sounds. Local +X is right
and pan is equal-power. UI gains apply equally to both channels on both executors.

`audio::Spatial` controls distance attenuation: `distance_model` is
`DistanceModel::Inverse` (default), `Linear` or `Exponential`; `ref_distance`
defaults to 1, `max_distance` to 10000 and `rolloff_factor` to 1, following
[Web Audio](https://www.w3.org/TR/webaudio-1.0/#enumdef-distancemodeltype).
The default is `1/max(distance,1)` with no 40-unit cutoff. Only the linear model
uses `max_distance`. Set `rolloff_factor: 0.` to retain directional pan without
distance attenuation. Attach `Spatial` to an `AudioSource` or an entity followed
by a voice, or call `world.play("hum").at_point(position).spatial(controls).start()`
to save controls with that voice. Explicit voice controls override the entity's.
An attached voice retains the entity's controls and position when it despawns.

Definitions are immutable shared values with a content revision computed at
registration or restore. A synth-only world saves and hashes exactly as it did
before samples existed. Voices retain their frozen definition; sources
resolve their name in the current registry and require a looping definition. A
non-looping `AudioSource` is refused by sound name at presentation; use `World::play`
for finite sounds, which records a deterministic activation tick. This avoids adding
activation state to attached sources or measuring their offset from world tick zero.
`AudioSource::new("wind").gain(0.3)` starts playing by default. After explicit
`sounds([..])` in setup, `world.play("chime").at(entity)` needs only `&World`,
so it can run while an unrelated component is borrowed.

Two budgets bound PCM. **Sampled sounds: 32 MiB of 16-bit samples resident per
world** (`asset::SOUND_BYTE_BUDGET`). The bake refuses a larger file, and a delivery
that would take the world's resident total past it is refused by name
(`sound … exceeds the 32.0 MiB sound residency budget`), leaving that declared
asset Failed in `state.world.assets` and setup waiting, never half-loaded.
**Synthesis: the Player reserves 32 MiB before rendering** (`samples × 4`), counting
a shared allocation once. It keeps active allocations and allocations still owned
by an output; acknowledgement releases the latter. Registry membership alone does
not retain PCM. Registration validates definitions; the 60-second synth duration
limit already keeps every valid synth below 32 MiB at 48 kHz. The Player checks the
aggregate budget at its actual output rate before rendering. Samples add nothing to
it: their allocation is the world's.
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
operation. `Pcm` is the Player's rendered mono `f32` or a sound asset's interleaved
16-bit frames (mono or stereo); `rate` is the source's own rate and `offset` is in
its frames. Outputs resample to the device. `set(id, left, right)` gains feed both
channels from mono PCM and each channel from its own stereo channel, on both
executors. WebAudio failures return refusal for retry instead of trapping the module.
Each web voice connects buffer source → (channel splitter, for stereo) → left and
right gains → channel merger → destination; the `AudioBuffer` is created at the
source's rate and cached per allocation. Playback rate implements pitch; gains use
`setTargetAtTime` with a 10 ms time constant.

Apple has 32 fixed voice slots. `start`/`set`/`stop` enqueue producer-side work;
`flush` retries it (Player calls flush each sync). Pending Set commands coalesce by
identity, latest wins. Unpublished Start/Stop pairs cancel; published commands
retain their order and PCM until acknowledged. Published starts occupy at most 32
slots, including stopped voices awaiting acknowledgement. Up to 32 newer selected
starts stay in the coalescible producer queue. Across both windows, retained PCM
has a **64 MiB total byte budget** (the synthesis and sound budgets together),
counting each shared allocation once at its own sample width. `Pending::start`
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
It reads `f32` or 16-bit samples (divided by 32768), linearly resamples each channel
from the source rate, ramps stereo gains over 10 ms, maps non-finite samples to
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
own no player or device. Failed device creation retries every 300 live sync frames, with
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

## Proof

The [audio tests](tests) exercise synthesis, output ownership, seek/restore,
sample-rate changes and sampled voices; the aliasing measurement is
`cargo test -p exact-game-audio alias -- --nocapture`. [Greybox](../games/greybox/logic/src/lib.rs)
is the synthesized consumer and [the audio fixture](../games/audio-fixture/logic/src/lib.rs)
the sampled one; their proofs check silent seekable execution, and with
`EXACT_AUDIO_PROBE=1` on the web, the live gesture path and non-zero analyser RMS. Synthetic lifecycle events are not a physical
phone interruption test. Simulation/save pins live in the game's `pins.json`.

For commands and executor selection see [the game map](../README.md); dated
measurements belong in [bench](../bench/README.md) or the [diaries](../diaries/README.md).
