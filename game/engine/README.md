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

The [complete first game](../README.md#the-programming-model) is this crate's
runnable doc-test. `Game::Args` is a struct with `#[derive(Args)]`: declaration order
is positional order, `#[live]` avoids rebuilding, decoding refuses before mutation.
Setup cannot fail. Setup, paused and tick receive typed arguments; Sim retains the
bound values for saves and agent state.

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

World containers are EXGAME v3; Sim containers are EXSIM v5. Older containers are
refused by name, without migration. Sim's encoded world is one bytes field.
Undelivered `emit` messages remain saved, in order, but are excluded from the
simulation hash. Host draining never changes that hash. Restore retains Input's
viewport for headless touch continuation; a later host resize replaces it.

Storage revisions/membership serve derived caches. Every mutable row lease marks
its page; query iteration marks once per visited page and caches its backing pointer,
including filtered, optional and owning iteration. Insert/remove/load mark writes too.
The shared mutation epoch still invalidates quiescence on every mutable lease. There is no last-changed-tick API. `Play` is a
must-use builder: `.start()` creates a voice. PCM generation belongs to `game/audio`.

D1 measurements (2026-09-17, arm64 M5 Max, release; five frozen-snapshot
save/restore samples, median milliseconds):

| Sim | Before bytes | Current bytes | Save ms before → after | Load ms before → after |
|---|---:|---:|---:|---:|
| 2,000-box pour, 238 physics steps | 52,033,602 | 11,187,756 | 96.446 → 6.704 | 283.536 → 11.371 |
| Greybox setup, seed 7 | 5,376 | 3,003 | — | — |
| Beacons setup, seed 7 | 7,655 | 4,225 | — | — |

The pile fixture freezes the tenth active snapshot, already refreshed, in a Sim
container; timings include binary encoding/decoding and atomic restore, excluding
physics stepping and dirty-snapshot refresh. It retains all 10,030,624 physics
snapshot bytes (including the eight-byte physics marker), maps and components.
Run `PILE_TICKS=238 cargo run -p exact-game-physics --release --example pile -- 2000 --save`.
`cargo run -p exact-game --release --example parity` prints the game cards/sizes.

Greybox's standalone GPU crate (`--profile web`, raw / gzip -9 bytes) is
1,677,842 / 455,002 → **1,676,464 / 454,447**. This measures the simulation
and renderer artifact before host bindgen/optimization, not an engine-only library; unused synthesis was already
removed by linking, so moving its source is not claimed as a wasm saving.


P1 measurements, 2026-09-17, release; median of three runs, with each run's load1.
`examples/churn` retains its median-of-five rotation sampling within each run.
The final iteration chunk caches its page pointer as well as marking its generation;
there is no per-row generation store. CPU runs on Linux are the primary comparison.

| Churn ns/entity/tick | Before | Before load1 | After | After load1 |
|---|---:|---|---:|---|
| Mac, 100k | 2.50 | 19.84/22.50/22.50 | 1.99 | 20.12/20.12/19.15 |
| Mac, 500k | 2.44 | 19.84/22.50/22.50 | 1.97 | 20.12/20.12/19.15 |
| Linux, 100k | 5.74 | 1.26/1.24/1.22 | 5.22 | 1.67/1.62/1.57 |
| Linux, 500k | 5.79 | 1.26/1.24/1.22 | 5.27 | 1.67/1.62/1.57 |

Greybox seed 7 setup retains **309,641 → 301,713 bytes** on both architectures.
This is requested live heap allocation plus `size_of::<World>()`, not process RSS:
heap **308,873 → 301,025**, inline **768 → 688**. It includes the new per-page
metadata; component pages still dominate this eight-entity world. All three runs
reported the same byte counts. Load1: Mac before 21.58/21.58/21.58, after 19.15/19.15/19.15;
Linux before 3.32/3.32/3.32, after 1.85/1.85/1.85.
The RNG is this fixture's singleton; ordinary Resource cells use the same storage.
Run `cargo run --release -p exact-game --example memory` or `--example churn`.


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
The template's non-live `restart_generation` is incremented to reconstruct setup
through the existing argument-binding path.


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
without holding playback leases. Registration installs no callback. Attachments
choose their joint with `SocketFollow::new("fox", "head").offset(t)` and never
write simulation transforms. `animation::socket(w, target, joint)` returns the current
world-space tick endpoint; the renderer composes displayed attachments from the
interpolated local chain. `tick_end()` returns `Now` for the boundary being authored;
use it for root-motion time, springs and HUD publication, retaining `now()` for the
completed boundary. `Sim` and `World` expose explicit `local_position`/`global_position`.
Character contact includes actual displacement; collision movement uses the separate
physics `CapsuleController` handle so both component types coexist.
