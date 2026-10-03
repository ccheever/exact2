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
- `query::<Q>().one()` returns an optional row, refuses a second match and keeps
  its lease for the returned row. [Borrowing](#borrowing) is per row.
- Use `insert_resource`, `resource`, `resource_mut` and `try_resource` for singleton
  state. Register types that first appear mid-game in `Game::register` so a fresh
  process can restore them. The engine's scene components (`Transform`, `Parent`,
  `Mesh`, `Material`, `Camera`, lights, `Visible`, `Ambient`, `Follow`, `Glow`,
  `Lit`) are registered by every world.
- `near` and `near_xz` read global poses in entity order, including parented
  entities. `nearest_xz_mut` supplies one entity and its mutable component together.
- `despawn(e)` removes that entity immediately. After the tick, the simulation
  removes its descendants and propagates transforms. Both cost what changed:
  after the first propagation they visit only despawned parents and the
  `Parent`/`Transform` pages written, and recompute only the subtrees under rows
  whose values differ, so a static hierarchy costs nothing per tick.
  `children(e)` scans; a tick that needs a child list can keep it in a component.
  Saved parent cycles refuse to load; a runtime cycle drops its highest-index edge
  and journals the repair.
- `pose_cursor()` and `poses_changed_since(cursor)` name the blocks of `PAGE`
  entities whose local `Transform` rows were written or whose propagated global
  poses changed since the cursor was taken: physics scenes, render feeds and other
  derived caches skip the rest. A block may hold no changed pose; an unreported
  block holds none. A cursor from before a load or restore reports every block.

`w.dt()` is one fixed step. `w.tick_end()` names the endpoint currently being
written; use it when retargeting motion. `w.now()` names the completed boundary.
`local_position` and `global_position` are explicit on both `World` and `Sim`.
Use `rand(range)`, `chance(p)` or `pick(slice)` for individual random draws, or
hold `rng()` for a batch. These all use the same saved random stream.

## Borrowing

Borrows are checked per row at run time. Each lease covers the rows it can hand out:

| Lease | Rows | Kind |
|---|---|---|
| `get`, `require` | one entity's row, while the guard lives | shared |
| `get_mut`, `require_mut`, `nearest_xz_mut` | one entity's row, while the guard lives | exclusive |
| `query::<Q>()` | every row it matches, after `with`/`without`, from its first iteration until it and every row guard it yielded drop | per term |
| `local_position`, `global_position`, `near*` | each row they read, for the read only | shared |
| `pages::<C>()`, `hash`, `save`, inspection | every row of the component | shared |

Different rows never conflict: read the player inside a loop over the enemies, or
hold one entity's `RefMut` while changing another's. One row takes any number of
shared leases or a single exclusive one. Constructing a query takes no lease;
its first `iter`, `one` or `for` does, so builder filters narrow it. A query over
every `Transform` includes the player's row: read the player before that loop, or
exclude it with `.with::<Enemy>()`/`.without::<Player>()`. Two queries may overlap
only where both are shared.

A conflict panics, and release and web builds abort, with the component, the entity's
name and handle, both leases and both callers:

```text
borrow conflict on Transform of `player` (#0, generation 0)
  requested: shared borrow of one row (get/require/position), at src/lib.rs:42:15
  held by:   exclusive query (&mut Transform), taken at src/lib.rs:41:29
```

A live lease records only its kind and its caller's location. Nothing is marked
per row, so iterating a query costs no more than the join itself. A single-row guard
pushes and pops one record on its column; position reads copy the row out and only
check for an exclusive lease. A leaked guard or query (`mem::forget`) keeps its rows
refused; after the next spawn, despawn, insert or remove, a leaked query holds every
row of its components, because its matched rows can no longer be known. The design and its
measurements are in [LLP 1046.003 §Row leases](../../llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23).

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

The other way, Contract's `postMessage("world", "buy carrot")` posts text into
the surface of that name. Each message is input stamped at the call: the next
tick reads it in `input.messages()`, in arrival order, never coalesced (two presses
between ticks are two messages). A paused world holds them for its first tick
after the pause. A message waiting for its tick saves with the input queue; one
delivered is gone. Messages are at most 64 KiB; keep data in the world and post
commands. `Sim::post(text)` does the same in tests.

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

Attach `animation::Layers(vec![animation::Layer::new(Animation::play("hit").once())
.additive().weight(0.5).mask(["spine"])])` beside the base `Animation`, `Blend`
or `Animator`. Layers run in order before IK; masks include the named nodes and
their descendants. Override layers blend only the clip's authored channels;
additive layers apply deltas from the imported bind pose. Each layer saves its
own clock and marker state. Weight zero advances that clock without changing the
pose or emitting markers. Root motion belongs to the base controller. Layers
can also stand alone over the bind pose. Unskinned animated mesh nodes follow
the same interpolated hierarchy as skinned nodes; adding an artificial skin is
unnecessary.
`SocketFollow::new("fox", "head").offset(t)` attaches to a joint while preserving
the saved local transform. `animation::socket(w, target, joint)` reads the current
world-space tick endpoint; displayed attachments use the interpolated local chain.

Emitters form local clouds: moving the emitter moves particles already born.
For sound, `w.sounds([..])` registers synthesized (`Synth`) or sampled (`Sample`)
definitions in setup; a sample names a `.sound` asset declared in `Game::ASSETS`, and
registration saves its frames, rate and channels. `w.play("chime").start()` creates
a voice with optional gain, pitch, pan, start offset and fade-in; dropping the play
builder does nothing. A looping definition plays until `audio::stop` or
`audio::fade`. Call `audio::step(w)` after game logic. Voices and definitions are
saved and hashed; PCM is delivery, outside both. Playback, PCM generation and the
budgets belong to the separate [audio executor](../audio/README.md).

## Saves and assets

`Sim::save` captures the simulation; `Sim::restore` validates format, game identity,
registered types and assets before replacing anything. Failed restores leave the
receiver intact. Restore reinstates the saved input queue, held keys and touch
viewport; a later host resize replaces the viewport. Agent inspection does not
consume input.

`sim.load_assets(|name| std::fs::read(name))?` loads headless dependencies, including
declared `.sound` assets, which the primitive module also decodes; a save records
their content identity, as it does a level's.
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
Observation and `World::hash` are paged: each re-reads only the component pages
written (or inserted into) since it last looked, the entity-table pages a spawn or
despawn touched, and resources whose revision moved, so an observed tick costs
what changed rather than the world (0.1 ms at 216k entities, against ~90 ms).
The hash is a stream over page digests in type-name and page order; it is the
same on every host and for a world freshly loaded from the same save.

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

World saves (EXGAME v4) are columnar: the entity table and each component
storage store their rows' shape (tags, field and variant names, sequence lengths)
once and their scalars as run-length or dictionary-coded columns, so repeated
values cost nothing per entity and no name or entity key is repeated per row.
Grow a Garden's scale world saves in about 9 bytes per entity, against 247 in v3.
Resources save as their values. The encoding is byte-identical on every host.

Loads allow at most 16 Mi entity slots, 64 MiB per string, and 2 GiB of input and
accounted allocations. Custom `Data` readers must account allocations with
`Reader::claim`. Violations return `DataError`; old save formats refuse by name.

[Game commands and host integration](../README.md) ·
[Renderer contract](../render/README.md) · [Measurements](../bench/README.md)
