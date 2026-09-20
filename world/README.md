# exact-world

`exact-world` is an optional, deterministic, nonspatial world kernel. It owns typed
state, entity lifetimes, fixed ticks, input, saves and publications. Games and
optional modules supply behavior. Ordinary Exact apps do not acquire it, and no
root package depends on the separate `game/` workspace.

## A small lifecycle

Register saved types before constructing entities. Keep continuation state in
components, resources and arguments; the game implementation itself is stateless.

Use `?` to propagate kernel errors from `setup` and `tick`.
A returned setup error refuses construction or restart.
A returned tick error stops before reaping or advancing the tick, logs the first failure, and refuses further driving, saving or input.
Bounded state, tree and log inspection remain available until restore, carry or a bind restart installs healthy state.
Never `.unwrap()` a kernel `Result` in game code: on wasm a panic aborts the module and every world in it.

```rust
use exact_world::*;

#[derive(Default, Component)]
struct Counter { points: u32 }

#[derive(Default, Args)]
struct Options { #[live] increment: u32 }

struct Board;
impl Game for Board {
    const ID: &'static str = "readme-board";
    type Args = Options;

    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        w.register::<Counter>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &Options) -> Result<(), DataError> {
        w.spawn_named("counter", Counter::default())?;
        Ok(())
    }
    fn tick(w: &mut World, _: &Input, args: &Options) -> Result<(), DataError> {
        if let Some(mut counter) = w.get_mut::<Counter>("counter") {
            counter.points += args.increment;
        }
        Ok(())
    }
}

let mut sim = Sim::<Board>::new(Options { increment: 2 })?;
let entity = sim.world().named("counter").unwrap();
let consumer = sim.world_mut().subscribe_changes()?;

assert_eq!(sim.run(17.)?, 1);
assert_eq!(sim.world().get::<Counter>(entity).unwrap().points, 2);
assert_eq!(sim.world().observation(), None); // ordinary ticks do not observe Data

// Structural replacement produces a generation-bearing change for subscribers.
sim.world_mut().insert(entity, Counter { points: 10 })?;
let through = {
    let batch = sim.world().changes(&consumer)?;
    assert!(!batch.resync);
    assert_eq!(batch.events.len(), 1);
    batch.next
};
sim.world_mut().acknowledge_changes(&consumer, through)?;

let bytes = sim.save()?;
let restored = Sim::<Board>::from_save(&bytes)?;
assert_eq!(restored.save()?, bytes);
assert_eq!(restored.world().hash(), sim.world().hash());

// This is a live argument: binding it preserves the world and its clock.
sim.bind(Options { increment: 0 })?;
assert_eq!(sim.settle(2)?, 1);
assert_eq!(sim.world().observation(), Some(true));

// Logs use an independent, unsaved cursor, including session-only messages.
sim.world().session_log("adapter attached")?;
let page = sim.world().logs(LogCursor::default())?;
assert!(page.entries.contains("adapter attached"));
let _next_page = sim.world().logs(page.next)?;
# Ok::<(), DataError>(())
```

This example is compiled as a doctest. Run it with
`cargo test -p exact-world --doc` from the repository root.

## Invariants and cost

### State, identity and borrowing

Data fields traverse in declaration order. Storage traverses in type-name order;
queries return ascending entity slots. Empty required columns and empty worlds
prepare zero query words; empty page iteration also short-circuits. Hashing and binary encoding canonicalize
NaNs and preserve signed zero. Semantic Data has no interior mutability. Manual
implementations must obey the visitor and admission contracts below. Explicit
`hash() -> Result<u64, DataError>` always walks current Data; it no longer caches by the mutation epoch.

`Entity` contains a slot and generation. Reusing the lowest free slot does not
revive an old handle. `entity_at(index: usize) -> Option<Entity>` checks the slot
and returns its current live incarnation; dead and out-of-range indices return
`None`. Names select the lowest live matching slot. `resolve` accepts a name,
`#12`, or `name#12`; explicit index selectors take precedence over names.

Storage names admit 1–256 UTF-8 bytes. Registration is explicit and fallible. A short-name collision refuses and names
both full Rust types; stale-handle `insert` returns `Err`.
`insert` returns `Ok(true)` for new membership and `Ok(false)` for replacement. `register::<C>()` and
`register_resource::<R>()` are idempotent; component hooks run once per registry,
including recursive registration. A returned hook error restores all type
declarations made by that call and its dependencies; retry runs the hook again.
Hooks declare types only; arbitrary gameplay effects are not rolled back.
Rollback takes a fixed 256-byte snapshot only when a hook declares a dependency;
empty hooks do no snapshot scan or allocation. Type count bounds nested work. Panics still poison. `Game::register` receives borrowed setup/restart
arguments, without formatting them. Insertion requires declared types; `set_parent`
implicitly declares the kernel-owned Parent after validating the edge.
Registration and setup must not keep hidden continuation state.
The resource name `Rng` is reserved for the built-in generator; wrap its Data in a
game-named resource to save an additional independent stream.

Shared and exclusive column leases enforce aliasing. Mutable queries are sealed;
optional terms, filters, tuple joins and retained row guards preserve disjointness.
`pages::<C>()` holds a shared lease. Its `runs()` yield only initialized contiguous
values, never holes or padding. Page masks are read-only. World identity,
replacement, component revision/membership and page generations support external
caches. Mutable access conservatively invalidates revisions even without assignment.

A panicking structural mutation poisons its world; discard it. A panicking tick
once gameplay has started poisons its driver and World, so neither continuation
boundary can save partial logic. A healthy Sim checkpoint can recover it. Installation swaps the complete driver
before dropping outgoing state; a destructor panic propagates with the incoming
state healthy and consistent. Live bind invalidates observation before old Args drop. Input admission happens before the boundary changes. Earlier
successful ticks remain committed if a later tick in the request is refused.

### Ordinary ticks and explicit observation

An ordinary tick performs **zero component visitor/hash calls**. Its kernel cost
is O(game-touched work + bounded input bookkeeping), independent of untouched
component values and world size. Input sets have at most 64 entries and the
pending queue at most 1,024 events. Due batches are preflighted against a stack
array of 64 borrowed key names, then consumed by moving event ownership. No held
strings, maps or edge buffers are cloned per tick. Admission is O(events × 64);
edge derivation is O(events × actions × held keys), with each factor explicitly
bounded. A 65th distinct held key refuses before consuming any event. Mutable queries still cost their chosen query
traversal; this is work the game requested. Changing publications also creates
saved journal events. `Paranoid::Save` and `FreshGame` explicitly add full saves,
validation and reconstruction, and do not have the ordinary-tick cost.

`observation() -> Option<bool>` distinguishes an unobserved boundary (`None`),
observed change (`Some(false)`) and an observed unchanged tick (`Some(true)`).
Mutable access, live argument binding and held-input reconciliation invalidate the report.
Driving with no tick or applied input and draining an empty message queue preserve it. `quiescent()` requires
an observed unchanged boundary and a current settle deadline.

`settle(max_ticks)` samples the initial boundary once, then each completed
boundary once. It excludes Ambient components/resources from rest observation,
while they remain saved and hashed. Pending or failed external work refuses
settle immediately; virtual time cannot finish I/O. `Work::Deadline(tick)` declares
future simulation work. `busy(reason)` lasts one tick. Repeating the same work or
publication value does not replace it or invalidate the world.

`sample() -> Result<Sample, DataError>` explicitly reports the observation hash,
hashed bytes and visited components. It admits at most 32 MiB of hashed Data and
1,000,000 live-component/slot probes, refusing during traversal. Missing
memberships do not consume that allowance; at most 200,000 × 256 membership tests
are possible under the entity/type bounds. This costs O(slots × storage
nonempty types + admitted Data), even for mostly empty worlds. Built-in visitors stop at
refusal; manual writers must honor `stopped()`. Empty retained columns do not consume probes or take part in traversal. Ordinary
ticks never sample. `report(&mut dyn Writer) -> Result<(), DataError>` exposes
observation, up to eight busy reasons and eight named Work entries, plus truncation;
it visits at most 64 admitted entries, each with at most 256 bytes of text.

`visit(Option<Entity>, &mut dyn Writer)` traverses erased components (`Some`)
or resources (`None`) by saved type name. `candidate()` makes an isolated exact
copy. `Candidate::edit(entity, name, bytes)` applies canonical field patches only
to that copy; a failed edit poisons it. `commit(self, &mut World)` validates
ownership and health before adoption. It refuses a different destination or a
changed source boundary (including completed ticks, applied input and delivery) before mutation.
Erased Parent edits refuse; ownership changes use `set_parent`. Publications, their pending flag, messages,
and saved game history survive an untouched commit; session/structural replacement
effects follow the ordinary adoption contract. Save/decode budgets apply to candidate
creation; each edit admits at most 256 MiB of bytes and decoded allocation.
`resource_revision::<R>() -> Option<u64>` is local to R; compare revisions only
within one `replacement()` generation.

Publications are kernel `Published` Data (scalars, options, lists, positional
records and named objects), preserving the existing saved tags. `publications()`
borrows the map; `take_published()` returns an owned map only when pending.
Contract conversion and `publish_record(world, &data)` now live in
`game/world-adapter/src/publication.rs`, compiled only in the game workspace.
The kernel has no `exact-plan` dependency or Value re-export. The adapter exposes
`args::decode_args::<A>(&[Value])`, `argument_values(&A)` and `from_values::<G>`;
conversion uses Data, preserves omitted defaults and checks Contract numeric range.
Publication map keys and journal keys share `Rc<str>` allocations. Owned delivery
still returns String keys. `publish(key, value) -> Result<(), DataError>` and
`publish_batch(BTreeMap<String, Published>) -> Result`
admits all keys, values and event cursors before changing anything; the adapter
uses it for complete record updates. A single changed publication validates only
its old/new values, using the retained aggregate cost; batches visit at most 512
old/new entries within shared caps of 65,536 nodes and 65,536 string bytes.
`emit(text) -> Result<(), DataError>` queues up to 1,024 messages of at most
4,096 bytes each; refusal preserves pending delivery.

### Ownership and structural consumers

Parent is an ownership edge, independent of spatial transforms. Only
`set_parent` changes it. Parent insertion rejects stale handles and cycles. Each ancestry check admits
at most 256 edges, returning an error past that work bound before mutation. This
bounds natural chain construction; it is not a global maximum depth, since reverse
construction can form deeper valid chains. Validation still walks each edge once.
`despawn(Entity) -> Result<bool, DataError>` and
`remove::<C>(Entity) -> Result<Option<C>, DataError>` refuse exhausted cursors
before mutation; stale/absent targets return Ok(false)/Ok(None).
Despawn removes the entity immediately; descendants leave at the next reap in
ascending slot order. Reusing a dead parent's slot cannot rescue descendants.

A derived reverse-child index makes unrelated despawn O(log owners). Owner
removal queues only its immediate children; reaping follows only that orphan
subtree, preflights all removals, then removes in slot order. At most 200,000
entries are visited; excess work refuses explicitly. Cost is O(subtree × log slots
+ subtree × registered columns), independent of unrelated entities. Exact save
validation still uses a linear three-color ownership walk with bounded scratch.

The structural journal has a separate consumer model from saved game logs:

- `subscribe_changes() -> Result<ChangeConsumer, DataError>` starts at the current
  boundary. At most 64 subscriptions may be live.
- `changes(&consumer) -> Result<Changes<'_>, DataError>` returns `next`, `resync`
  and an exact-size `events` iterator. Reading does not acknowledge anything.
- `acknowledge_changes(&consumer, through)` advances only that consumer.
  Retention follows the minimum acknowledgement.
- At most 4,096 changes are retained. A lagging reader gets `resync: true` and an
  empty iterator; rebuild derived state and acknowledge `next`. A slow reader
  never causes spawn, replacement or reaping to fail.
- Without subscribers, the structural consumer deque allocates and retains no events. Dropped
  subscription claims are pruned at the next edit, subscription or acknowledgement.
  The deque's allocated capacity may remain available for reuse. Saved game logs
  independently retain deterministic structure events.
- Suffix reads use a deque range at the cursor offset, without filtering history.
  Subscription validation is bounded by 64 handles; pruning is amortized over the
  released events. Replacement transfers subscriptions and appends Reset.

Saved game events retain their original indices. `logs(LogCursor)` merges them
with unsaved session messages using independent offsets and a replacement token.
`Logs` contains `next`, `reset`, `truncated` and JSON `entries`. A page admits at
most 512 records and 65,536 escaped output bytes *before* formatting. Long messages
produce smaller pages. More than 512 session-only messages remain pageable even
when no game event occurs. Each stream retains at most 4,096 entries. Restore
reanchors retained session messages after restored game history and reports reset.

### Saves, codecs and admission

World saves use **EXGAME v4** and simulation saves use **EXSIM v10**.
Other versions refuse without migration. Frozen checkpoints pin bytes and hashes.

`World::load/carry` on a Sim-owned world refuses; use the owning Sim.
Exact restore reconstructs a candidate and requires canonical bytes to reproduce
exactly before installation. `carry` deliberately permits field adaptation and
returns whether canonical content changed; Sim carry also preserves current live
arguments and rejects setup drift. Restore never runs gameplay setup on the
restoring world. Derived slots, observation, structural subscriptions and session
cursors are not saved continuation state.

`Reader<'data>` borrows field and variant identities from the input. Duplicate
tracking reuses a field stack and per-name depth marks; nested records restore
outer marks. Repeated literal name definitions cannot bypass duplicate rejection.
Map Data admits only String and Rc<str> keys, whose complete
meaning survives record-name encoding. Custom key conversions are refused by a
sealed bound. Owned map keys claim storage; decoder accounting stays cumulative.
Sequences reserve their admitted known length once.

`Writer::field` and `variant` take static names; dynamic map names use `key`.
`Writer::bytes(Bulk<'_>)` receives borrowed U8/U16/U32/F32 slices. Numeric chunks
use at most 1,024 stack bytes; there is no payload-sized conversion vector in
hashing, saving or inspection. Tags, little-endian order and float normalization
remain unchanged. Numeric decoding still owns both raw and converted buffers;
its cumulative budget counts both.

Inspection and binary writers expose `stopped()`; container loops stop on refusal.
A load shares one cumulative budget for requested allocation bytes, including validation.
Charges use capacity × `size_of`, checked chunk layouts, string bytes and map node estimates.
Growth charges the new allocation in full; old and new backing can coexist.
Manual Data readers that allocate must call `r.claim(real_bytes)` before allocating.
Defaults remain trusted code: even derived allocating defaults can escape this budget.
Errors preserve the destination; saving checks output bounds without pre-proving loadability.
`Paranoid` provides the functional save/load proof; refusal can differ by architecture.

| Admission | Bound and refusal |
|---|---|
| Entity slots / registered types / query terms | 200,000 / 256 / 8 |
| Structural subscriptions / retained changes | 64 / 4,096; lag returns resync |
| Game and session log retention | 4,096 each; loss/reset is reported |
| Inspection output / log page | 65,536 bytes/visits / at most 512 records |
| Work / busy / derived slots | 64 each; reasons 256 bytes, at most 8 reported |
| Input / emitted messages | 1,024 queued each; messages 4,096 bytes |
| Clock advance / settle | 216,000 / 3,600 ticks per request |
| Binary/world/simulation output | Generic 256 MiB; World and Sim 128 MiB |
| Input bytes | 256 MiB; Sim 128 MiB |
| Requested decoder allocation / string / nesting | Cumulative 256 MiB / 1 MiB / 256 |
| Sequence lengths | At most remaining input bytes before reservation |
| Publication nodes / string bytes / logical depth | 65,536 / 65,536 / 80; root keys ≤ 256 × 256 bytes |

Requests
past these bounds return errors, except programmer-facing infallible operations
(such as conflicting borrow use) which panic. Journal capacity does not cause mutation refusal. Saved tick and game-journal cursors above 2^62 refuse decode with
`cursor beyond supported range`; 2^62 is accepted. Structural, session and replacement
cursors are unsaved and cannot be supplied by a checkpoint. Runtime overflow checks
remain defensive guards. Live generation `u32::MAX` refuses decode; `log` returns an error before dropping
retained events. A dead exhausted slot is retired permanently. Spawn selects the lowest reusable
free slot, skipping at most 200,000 retired indices, then appends if capacity
permits. Retirement is saved in the existing generation/free fields (no wire change).

### Clock, arguments and scalar helpers

`run` and `advance_to` share one caller clock. Each nonnegative millisecond delta
rounds separately to the nearest microsecond, with half-microseconds rounded up.
Sub-microsecond deltas are not accumulated before rounding. `alpha_inputs()` still
returns `(tick, numerator, 1_000_000)`; typed clock/phase result types are deferred.
Nonnumeric # suffixes resolve as names. Both paused and playing input use half-open timestamp boundaries. Clock rebasing
and settle range-check in widened arithmetic before mutation, in debug and release.
Pause advances caller time and reconciles held input without simulation ticks or
retained resume edges. `reconcile_input(clock_ms, held)` atomically replaces held
state, clears edges/queued input and rebases caller time without a tick.

Args fields default to Setup. `#[live]` changes subsequent ticks; either boolean
`#[restart]` edge reconstructs the world. Registration sees only setup/restart
fields. Derive accepts unqualified and std/core-qualified portable scalars; nonfinite values and oversized text refuse
before mutation. Full-width integers remain typed core values; Contract safe-range
checks belong to the adapter. Input exposes key, held,
pressed, released and scalar axis state. Axis declarations still require a key
pair; all declared keys share the runtime 128-byte bound. An analog-only declaration is deferred. `stick_axis` uses the
60-point contact-offset rule; contact ownership belongs to a module.

Scalar math re-exports libm's f32 functions directly. Angles are radians;
`round` rounds halfway away from zero. `lerp` permits extrapolation and
`wrap_angle` returns [-pi, pi). `ease` uses a nonnegative time constant and
snaps within 1e-4; zero lag arrives immediately. `Rng::next_u32` and `next_f32`
are the deterministic RNG primitives. Saved Spring/Tween and smoothstep live
in the optional `game/world-motion` module; core has no motion dependency or
per-value settlement hook. Modules declare animation work with `Work::Deadline`.
Derived caches use 64 independent lazy cells, with at most 64 type-ID comparisons;
replacement clears them. Reborrowing the same T exclusively still refuses.

## Measurements and reproduction

The resident-byte experiment is **not ready to adopt**. A 200,004-byte input
containing 100,000 empty records with a derived, skipped `Box<[u8; 32]>` field
succeeds under a 1 MiB budget but peaks at 4,000,096 requested bytes on x86-64.
Default construction and skipped-field resets are outside reader allocation sites.
The counterexample is executable in `storage::resident`; a passing counterexample
test confirms the gap, not the desired safety guarantee. Full measurements,
deletions and verification limitations are in [the experiment report](KL.md).

The other hostile fixtures assert both peak and cumulative requested allocations
≤ the supplied budget + 8,192 diagnostic bytes. They cover forged counts, huge
strings, wide enum/struct vectors, nested publications, many map keys, 200k slots,
and sparse 1 KiB/4096-aligned chunks across types, at 1 MiB and 256 MiB budgets.
Each runs inside `catch_unwind`, checks unchanged destinations on error, and has
successful controls. The wasm32 harness executes the same cases and frozen saves;
wasm aborts surface as host traps, since that target cannot unwind panics.

The input cap and structural caps bound parser work independently of allocations:
O(input bytes + admitted values + slots × registered types), with logarithmic map
operations and key comparisons. Ordinary ticks retain their existing bounds and
allocation counts. No query or run implementation or benchmark changed.

Use the existing Cargo cache and shared target directory. Before a cold build,
check `df -h ~`; stop below 25 GiB free. Basic reproduction:

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
cargo test -p exact-world -p exact-world-derive --no-fail-fast
cargo test -p exact-world --test ecs --test kernel -- --ignored
cargo test -p exact-world --lib resident -- --nocapture
cargo test -p exact-world --test startup -- --nocapture
cargo test --release -p exact-world --test restore -- --ignored --nocapture
cargo clippy -p exact-world -p exact-world-derive --all-targets -- -D warnings
cargo fmt -p exact-world -p exact-world-derive -- --check
cargo build -p exact-world --target wasm32-unknown-unknown
cargo +nightly miri test -p exact-world --lib storage::raw::tests -- \
  --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
cargo +nightly miri test -p exact-world --lib world::journal -- --test-threads=1
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --lib \
  storage::raw::tests -- --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast
cargo clippy --manifest-path game/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path game/Cargo.toml --all -- --check
(cd game && bun test)
```

The production ceiling is 7,500 handwritten lines: all production Rust under
world/ including derive, this README and both manifests. Comments and blank lines
count. Only tests and test-only allocator support are excluded. Reproduce with:

```sh
python3 - <<'PYCOUNT'
import re
from pathlib import Path
rust = 0
for path in sorted(Path('world').rglob('*.rs')):
    if 'tests' in path.parts:
        continue
    source = path.read_text()
    if source.startswith('#![cfg(test)]'):
        continue
    source = re.sub(r'^#\[cfg\(test\)\]\nmod \w+ \{.*?^\}', '', source,
                    flags=re.M | re.S)
    rust += len(source.splitlines())
readme = len(Path('world/README.md').read_text().splitlines())
manifests = sum(len(p.read_text().splitlines()) for p in
                [Path('world/Cargo.toml'), Path('world/derive/Cargo.toml')])
print('Rust', rust, 'README', readme, 'manifests', manifests,
      'ALL HANDWRITTEN', rust + readme + manifests)
PYCOUNT
```
