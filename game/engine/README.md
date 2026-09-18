# Engine programming model

- A tick calls ordinary functions; all saved state lives in components and resources.
- `#[derive(Component)]` names per-entity data; `#[derive(Resource)]` names singleton
  data. Register resource types for loading with `register_resource::<T>()`.
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
`Follow` is saved data, stepped explicitly by `scene::follow`; `math::ease` uses a
portable exponential and snaps within 1e-4 so settling is finite.

Mesh dimensions are authored once; see `Mesh` for conventions and constructors.
The renderer shares unit geometry per primitive kind; dimensions are instance data.
Unchanged transform pages avoid writes but changed column revisions still cause scans. `Collider::of(&mesh)` supplies matching primitive geometry.

Opaque vectors (`Vec<u8>`, `Vec<u16>`, `Vec<u32>`, `Vec<f32>`) use length-prefixed
bytes. Numeric bulk payloads are little-endian; f32 NaNs canonicalize, negative
zero survives. JSON exposes only `{"bytes":n,"hash":"0x…"}`; these summaries cannot
be loaded as data. Other vectors remain structural sequences. Bulk readers claim
both payload and decoded allocations; custom readers must preserve this accounting.

World containers are EXGAME v2; Sim containers are EXSIM v4. Older containers are
refused by name, without migration. Sim's encoded world is one bytes field.
Undelivered `emit` messages remain saved, in order, but are excluded from the
simulation hash. Host draining never changes that hash. Restore retains Input's
viewport for headless touch continuation; a later host resize replaces it.

Storage revisions/membership serve rendering; the shared mutation epoch invalidates
quiescence on every mutable lease. There is no last-changed-tick API. `Play` is a
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
