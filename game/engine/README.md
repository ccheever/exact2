# Engine programming model

- A tick calls ordinary functions; all saved state lives in components and resources.
- `#[derive(Component)]` names per-entity data; `#[derive(Resource)]` names singleton
  data. Resources and the RNG occupy singleton cells, not component pages; they
  retain the same Data save/hash/JSON representation and resource ambient opt-out.
  Register resource types for loading with `register_resource::<T>()`.
- `despawn(e)` removes only `e`. Descendants leave at the end of the tick: after
  `Game::tick`, `Sim` reaps dead-parent children in entity order, repeating for
  orphaned chains, then propagates transforms.
- Roots read their local `Transform` directly. `propagate` resolves only entities
  with `Parent`; a runtime cycle loses its highest-index edge and journals once.
  A save containing a cycle is refused.
- The renderer owns interpolation. `World::fresh()` lists entities spawned or
  teleported since the current tick began; `Sim` clears it at tick start. Entries
  include their generation and may refer to an entity that has since died.
- `get_mut::<T>` locks the whole column, including other entities. Use a query for
  multiple rows. `query::<Q>().one()` keeps those leases alive for its returned row
  and all builds refuse an ambiguous second entity. `children(e)` scans; tools
  use it, while a tick that needs children keeps them in a component.
- `rand(range)`, `chance(p)` and `pick(slice)` release the random lease before
  returning. `rng()` keeps a lease for bulk draws from the same saved stream.
- Agent component/resource JSON encodes `Option` as `[]` or `[value]`.
- Saves are limited to 16 Mi entity slots, 64 MiB per string, and 2 GiB of input
  and accounted decoded allocations; violations return `DataError`. Custom `Data`
  readers must account their allocations through `Reader::claim` too.
  The binary decoder borrows internal field/variant names from its input; values
  returned through `Reader` remain owned, with the same allocation accounting.
  Dynamic `Value` fields stream their existing variant tags directly to save,
  hash and JSON writers, without constructing a second owned value tree.

The [small example](../README.md#the-programming-model) is this crate's
runnable doc-test. Only rustdoc includes the guide; editing it does not rebuild
the runtime. `Game::Args` is a struct with `#[derive(Args)]`: a canvas can bind
`world(seed=7, paused=paused)`, with omitted fields taking Rust defaults.
Declaration order is positional order, `#[live]` avoids rebuilding, and decoding
refuses before mutation. Named canvas bindings resolve to this same typed path.
Setup cannot fail. Setup, paused and tick receive typed arguments. Sim keeps one
validated argument JSON representation for saves, bound restore and agent state.
Restore retains assets and the decoded input queue, validates the typed world once,
and replaces the receiver only after validation. It does not call setup.
Register types first spawned mid-game or selected by setup arguments in
`Game::register`. That hook receives named setup/restart arguments; live fields
cannot shape the saved schema.
The authored three-way merge follows typed decode, using a separate fresh tick-zero
world when the current authored base is unavailable. No candidate commits on refusal.

Seekable advances observe the final tick's components, RNG and non-ambient
resources, excluding `Ambient` entities. `changing` names up to eight components in storage order.
One-tick advances compare before/after; zero ticks retain the previous answer.
Live ticks do no observation. Springs and `busy(&self, reason)` also participate.
`Follow` is saved data. The engine places followers after setup and setup-argument
rebuilds, and initializes new followers on restore; games call `scene::follow` in
`tick` where following should happen. `math::ease` uses a
portable exponential and snaps within 1e-4 so settling is finite.

Mesh dimensions are authored once; see `Mesh` for conventions and constructors.
The renderer shares unit geometry per primitive kind; dimensions are instance data.
Page write generations let unchanged transform pages skip reading/hashing their bytes. `Collider::of(&mesh)` supplies matching primitive geometry.

Opaque vectors (`Vec<u8>`, `Vec<u16>`, `Vec<u32>`, `Vec<f32>`) use length-prefixed
bytes. Numeric bulk payloads are little-endian; f32 NaNs canonicalize, negative
zero survives. Each bulk kind has a distinct binary and hash tag, even when empty.
JSON exposes only `{"bytes":n,"hash":"0x…"}`; summaries and numeric arrays cannot
be loaded as data. Other vectors remain structural sequences. Bulk readers claim
the destination byte size once, including conversion from the temporary payload;
custom readers must preserve this accounting.

World containers are EXGAME v3; Sim containers are EXSIM v7. Older containers are
refused by name, without migration. Sim's encoded world and authored initializer are separate bytes fields; saved base
construction arguments are distinct from current live bindings. A bound Carry
three-way merges the initializer only after typed world decode and exact clock
validation. Open retains saved world Data. Older EXSIM5/6 saves require recreation.
Save headers and payloads are written into the same buffer; returned vectors may
retain spare capacity.
Undelivered `emit` messages remain saved, in order, but are excluded from the
simulation hash. Host draining never changes that hash. Restore retains Input's
viewport for headless touch continuation; a later host resize replaces it.
Input clones share immutable action declarations. Keys, contacts and action edges
remain independently owned; empty declarations need no shared allocation. This
does not change the author API or saved input representation.
Agent state projects pending input once, using Input's own event handling for
keys and contacts; inspection does not advance or consume the queue.
Restore refuses unsorted or duplicate held keys before committing the new state;
ordinary input updates preserve that order.

Storage revisions/membership serve derived caches. Every mutable row lease marks
its page; query iteration marks once per visited page and caches its backing pointer,
including filtered, optional and owning iteration. Insert/remove/load mark writes too.
The shared mutation epoch still invalidates quiescence on every mutable lease. There is no last-changed-tick API. `Play` is a
must-use builder: `.start()` creates a voice. PCM generation belongs to `game/audio`.

Publish a HUD with `w.publish_record(&Hud { beacons })` and a normal `Data` derive;
Contract checks types when applying a surface record; missing fields default and
extra names are ignored. Rust record field names are not checked at bake time.
Unit fields publish as `null`; safe integers include ±9,007,199,254,740,991 and
out-of-range u64/i64 values refuse before any publication. `None::<()>` is `null`;
`Some(())` is explicitly refused because Contract JSON cannot distinguish it. Scalar
`publish` remains available. `near`/`near_xz` return global poses in entity order,
including translated or rotated parents. Mandatory queries use
`query.one().expect("one player")`. Native spatial tests use
`sim.layout("player").unwrap().screen.center()` and `sim.pick(point)`.
`sim.load_assets(|name| std::fs::read(name))?` drains headless dependencies;
`sim.save()?` checks current mesh roots before any request drain and returns
named pending assets or failed declarations; failed cosmetics do not block saving. The game-save
migration hook is gone: the format and game identity are checked before loading.
The template uses a `#[restart]` argument to reconstruct setup.

Asset delivery state is owned only when needed. Rebuilt worlds share immutable
asset maps; the first write detaches them. Reads remain direct, and the final
owner releases its models. Primitive worlds borrow an empty view without linking
model cloning or destruction. Save bytes and host delivery APIs are unchanged.


Erased component pages own values through typed descriptor operations: `read`/
`write` moves, `replace` swaps, and `drop_in_place`, instantiated for the component
at registration. Allocation and masks remain erased. Padded owned components and
zero-sized components with destructors exercise insert, replacement, removal and
load; these ordinary tests do not lock typed moves against byte-copy regressions
without Miri. A non-ZST constructor-ID companion checks that replacement drops the old
instance and removal transfers the new one. That companion is intentionally not
a ZST: an actual ZST cannot carry an instance ID, and an external ID queue would
merely assume the move ordering it claimed to test; the existing panic-on-drop test protects occupied-slot ownership. Loading
an opposite-kind registration names the registered kind and the setup declaration
required to load it. The storage tests are suitable for Miri; neither installed
nightly on the R2 machine includes Miri, so that run remains owed.

Reverse one-shot initialization belongs to the controller: standalone Animation
saves an explicit `sampled` boolean (false before its first successful sample); Animator uses its own elapsed clock. Installing either on an
existing socket pose starts at the end. Both clocks are saved, so restore does not
restart a completed controller. Pose inspection validates both history lengths.


Animation is explicit: `let motion = animation::step(w)` samples once; its owned
result lets the game consume markers and call `transform.translate_local(...)`
without holding playback leases. Controller registration never steps animation.
SocketFollow registration installs pose resolution; worlds without attachments do
not link it. Followers compose saved local poses with the current owner and offset,
including skipped animation ticks. Attachments
choose their joint with `SocketFollow::new("fox", "head").offset(t)` and never
write simulation transforms. `animation::socket(w, target, joint)` returns the current
world-space tick endpoint; the renderer composes displayed attachments from the
interpolated local chain. `tick_end()` returns `Now` for the boundary being authored;
use it for root-motion time, springs and HUD publication, retaining `now()` for the
completed boundary. `Sim` and `World` expose explicit `local_position`/`global_position`.
Character contact includes actual displacement; collision movement uses the separate
physics `CapsuleController` handle so both component types coexist.

Measurements and artifact sizes belong in the [bench README](../bench/README.md);
working commands and the module/executor boundary are in the [game map](../README.md).

Emitters form local clouds: displayed particles follow the emitter's current
transform, so moving an emitter moves particles already born. They are not
world-space trails. A future trail extension would save a birth transform for
each emission and use that transform when deriving its particles; neither line
of that extension is implemented. Random stride and consumption stay fixed.
