# Engine programming model

Start with the [runnable example](../README.md#the-programming-model) or the
[starter game](../new/logic/src/lib.rs). Gameplay implements `Game::setup` and
`Game::tick`; components and resources hold its saved state. `World` owns time
and randomness; `Sim` owns input. The renderer interpolates completed ticks.

## State and queries

- `#[derive(Component)]` declares per-entity data; `#[derive(Resource)]` declares
  singleton data. Both use `Data` for saves, hashes and agent JSON.
- Use `spawn_named`, `require::<T>("player")` and `require_mut::<T>("player")` for
  named entities. `get::<T>` returns `Option<Ref<T>>`; dereference the borrow guard
  for value comparisons: `*w.get::<Mesh>("player").unwrap()`. Entity handles work too. Duplicate
  names resolve to the lowest living slot, including after recycling or loading.
- A mutable component lease locks its whole column, including other entities.
  Use a query to borrow several rows. `query::<Q>().one()` returns an optional row,
  refuses a second match and retains its leases for the returned row.
- Use `insert_resource`, `resource`, `resource_mut` and `try_resource` for singleton
  state. Register types that first appear mid-game in `Game::register` so a fresh
  process can restore them.
- `near` and `near_xz` read global poses in entity order, including parented
  entities. `nearest_xz_mut` supplies one entity and its mutable component together.
- `despawn(e)` removes that entity immediately. After the tick, the simulation
  removes its descendants and propagates transforms. `children(e)` scans; a tick
  that needs a child list can keep it in a component. Saved parent cycles refuse
  to load; a runtime cycle drops its highest-index edge and journals the repair.

`w.dt()` is one fixed step. `w.tick_end()` names the endpoint currently being
written; use it when retargeting motion. `w.now()` names the completed boundary.
`local_position` and `global_position` are explicit on both `World` and `Sim`.
Use `rand(range)`, `chance(p)` or `pick(slice)` for individual random draws, or
hold `rng()` for a batch. These all use the same saved random stream.

## Arguments and restart

`Game::Args` is `()` or a struct with `#[derive(Args)]`. Contract binds its names:
`world(seed=7, paused=paused)`. Omitted arguments take Rust defaults; positional
calls follow field order. Invalid arguments refuse before changing the world.

Ordinary argument changes rerun setup. `#[live]` fields reach each tick without
rebuilding; `Game::paused` chooses whether to tick. A `#[restart]` boolean reruns
setup on either edge, as the starter's Restart and Play again buttons do.
Setup cannot fail. Register types selected by setup arguments in `Game::register`;
live arguments must not change the saved schema.

Restore validates and installs saved state without calling setup. `Sim::restore`
uses saved arguments; a canvas uses `Sim::restore_bound` to retain current app
bindings. Keep state that should travel with a save in components or resources,
and app-owned settings in live arguments.

## HUD and events

Publish ordinary scalars with `w.publish("lit", count)`. For a structured HUD,
`w.publish_record(&hud)` accepts a named `Data` record with nested records, lists,
options and scalars. Contract reads the record through `exactSurface("world")`
and validates it against its shape: missing fields default, extra names are
ignored, and wrong kinds refuse by field name. Rust field names are not checked
against the Contract shape at bake time.

Publication delivers and journals only changed values. In records, unit and `None`
publish as `null`; `Some(())` refuses because it is indistinguishable from `None` in
Contract JSON. Record publication rejects enum variants and integers outside
±9,007,199,254,740,991 before publishing any field. Ordinary scalar integer
publication accepts u8/u16/u32 and i8/i16/i32; larger integers can use the checked
record path. See the [HUD example](../README.md#publications-and-events).

`w.emit("won")` queues a string for the canvas's `message=` handler. Undelivered
messages save in order and stay outside the simulation hash. The journal is
telemetry; reading it does not change the world.

## Movement, animation and sound

`w.character("player").step(direction, jump)` moves a `Character` and reports its
actual displacement and contact state. Collision movement uses the separate
[physics](../physics/README.md) `CapsuleController`.

`Follow` initializes after setup and argument rebuilds; restore initializes only
new followers and preserves saved poses. It then steps after each tick. Call
`scene::follow(w)` inside the tick to choose an earlier order; it will not step
twice. Primitive dimensions belong to `Mesh`;
`Collider::of(&mesh)` supplies matching collision geometry.

Retarget `Glow(Tween)` or `Lit(Spring)` once, using `w.tick_end()`. The renderer
samples them between ticks; keep authored emission or light intensity constant.
Negative light overshoot clamps to zero. See [effects](../render/README.md#effects).

Animation order is explicit: `let motion = animation::step(w)`, apply root motion,
then query sockets. The result owns its markers so component writes can follow.
Reverse one-shots start at their end; saved playback state keeps a completed
controller from restarting after restore.
`SocketFollow::new("fox", "head").offset(t)` attaches to a joint while preserving
the saved local transform. `animation::socket(w, target, joint)` reads the current
world-space tick endpoint; displayed attachments use the interpolated local chain.

Emitters form local clouds: moving the emitter moves particles already born.
For sound, `w.play("chime").start()` creates a voice; dropping the play builder
does nothing. Call `audio::step(w)` after game logic. Playback and PCM generation
belong to the separate [audio executor](../audio/README.md).

## Saves and assets

`Sim::save` captures the simulation; `Sim::restore` validates format, game identity,
registered types and assets before replacing anything. Failed restores leave the
receiver intact. Restore reinstates the saved input queue, held keys and touch
viewport; a later host resize replaces the viewport. Agent inspection does not
consume input.

`sim.load_assets(|name| std::fs::read(name))?` loads headless dependencies.
`sim.save()?` checks current mesh roots even before requests are drained and
reports pending or failed declarations by name. Failed cosmetics do not block saves.

Declare a data-authored level with
`const LEVEL: Option<asset::Level> = Some(asset::Level::of::<Island>("island.level.json"))`.
`Island` uses `Data`; the bake validates the JSON and setup reads
`w.level::<Island>("island.level.json").expect("validated level")` after delivery.
JSON levels need no `game.assets` setting. Agent asset state includes their value;
malformed fields report their path.

`w.generated("island.model", mesh_data)` registers immutable geometry during setup.
Clone the returned `Mesh` for repeated props. Saves store names and content identities,
not vertices, so reconstruct from the same level and seed before restoring. Changed
level bytes or generated output refuse restore by name. Keep other generator inputs
in the level or saved setup arguments; identity checks cannot prove a generator is
deterministic. See the [level example](../games/asset-fixture/logic/src/lib.rs).

## Settling and data formats

`Sim::settle` advances world time and reports what remains busy. Components, RNG,
non-ambient resources, springs and explicit `busy(reason)` declarations participate.
Physics remains busy while a dynamic body is awake; settling never forces sleep.
A one-tick advance compares before/after; a longer seek observes its final tick
pair. Zero ticks retain the previous answer. Live ticks do not perform observation.

Mark cosmetic entities `Ambient`. Declare `ambient_resource::<T>()` and
`derived_publication(name)` in setup/register when appropriate; these policies
survive load and must be present in each fresh game. Ambient data still saves and
hashes. Derived publications still deliver, journal and save, but do not hold rest
open; ordinary publications do. Exhausted settling journals the changing reasons.

Agent component/resource JSON represents `Option` as `[]` or `[value]`.
Tuple structs are arrays: `Glow(Tween)` is `[tween]`.
`Vec<u8>`, `Vec<u16>`, `Vec<u32>` and `Vec<f32>` use typed, little-endian bulk saves;
NaNs canonicalize and negative zero survives. Their JSON output is a bytes/hash
summary, while input accepts numeric arrays. Summaries cannot be loaded as data.
Other vectors remain structural sequences.

Loads allow at most 16 Mi entity slots, 64 MiB per string, and 2 GiB of input and
accounted allocations. Custom `Data` readers must account allocations with
`Reader::claim`. Violations return `DataError`; old save formats refuse by name.

[Game commands and host integration](../README.md) ·
[Renderer contract](../render/README.md) · [Measurements](../bench/README.md)
