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
  and debug builds refuse an ambiguous second entity. `children(e)` scans; tools
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

Seekable advances observe the final tick's component changes, excluding `Ambient`
entities and resources. `changing` names up to eight components in storage order.
One-tick advances compare before/after; zero ticks retain the previous answer.
Live ticks do no observation. Springs and `busy(&self, reason)` also participate.
`Follow` is saved data, stepped explicitly by `scene::follow`; `math::ease` uses a
portable exponential and snaps within 1e-4 so settling is finite.

Mesh dimensions are authored once; see `Mesh` for conventions and constructors.
The renderer caches up to 4,096 distinct mesh values and still uploads transforms
as unchanged pages. `Collider::of(&mesh)` supplies matching primitive geometry.
