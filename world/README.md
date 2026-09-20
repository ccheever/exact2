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
A returned tick error stops before reaping or advancing the tick, logs the first failure (the longest UTF-8 prefix within 4,096 bytes), and refuses further driving, saving or input.
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
component values and world size. Input holds at most 16 keys, with 64 action/button/axis entries, and the
pending queue at most 1,024 events. Due batches are preflighted against a stack
array of 16 borrowed key names, then consumed by moving event ownership. No held
strings, maps or edge buffers are cloned per tick. Admission is O(events × (actions + held keys)); edge derivation is
O(events × actions × (actions + buttons + bindings × held keys)).
A 17th distinct held key refuses before consuming any event. Mutable queries still cost their chosen query
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
old/new entries within the shared 65,536-unit publication budget.
`emit(text: impl AsRef<str>) -> Result<(), DataError>` queues up to 1,024 messages of at most
4,096 bytes each; size/count admission precedes copying and refusal preserves pending delivery.

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
`children(owner)` borrows live direct children in ascending slot order (including recycled slots); `parent(entity)` returns the live parent or `None`; both accept names or handles and allocate nothing, with O(log owners + children) and O(log types) work respectively under the existing admission bounds.

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

World saves remain **EXGAME v4**. **EXSIM v10** corrects lost publication delivery:
byte 6 changes 09→0a, the outer sequence count at byte 8 changes 9→10, and one
final boolean byte records pending delivery. Every previous payload field and
hash remains unchanged. Other versions refuse without migration. The frozen v9
inventory normalizes exactly those three changes and checks the new pending bit;
empty, pending and drained delivery have independent restore/from-save/carry tests.

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

1. A `Data` writer emits fields in declaration order and stops container loops on `Writer::stopped()`.
2. Declare conservative portable `inline_size`/`default_size` units, including allocating
   defaults and skipped-field resets; custom allocating `read_new` must claim before allocating.
3. Read through `Reader`, propagate errors, and respect its allocation and nesting checks.

Derive supplies these checks, including skipped defaults; arbitrary manual code is not bounded.
For the hostile inputs measured below, decoding peaks at ≤ the caller's byte budget + 8,192 bytes,
excluding input and existing state; this counts requested heap bytes, not allocator metadata or RSS.
Allocation claims on both encode and decode use `max(portable units, native size)`.
Default-construction charges remain unchanged. Maps charge 64 + key bytes + twice
that value allowance per entry for half-empty nodes, plus one initial 12-value
node allowance for allocation before amortization; the 1 MiB long-key control
exceeded budget + 8,192 by 212 bytes without that initial allowance.
Component registration checks `Layout::array::<C>(64)` and refuses overflow or > 256 MiB.
System out-of-memory may abort, as with ordinary Rust allocation.
`bin::read_into` stages a saved copy and patch under one budget, preserving the destination on error.
Publication decode additionally admits 65,536 shared units and depth 80 before child allocation.
Requests beyond the following work/storage bounds return errors.

| Measured hostile shape | Peak at 1 MiB | Peak at 256 MiB |
|---|---:|---:|
| Huge sequence count / tiny input | 112 | 112 |
| Huge string length | 58 | 58 |
| Wide enum vector | 104 | 104 |
| 4096-aligned struct vector | 104 | 104 |
| 20,000 map keys | 658,369 | 3,741,120 |
| 1,024 boxed 4096-aligned values | 1,024,104 | 4,202,560 |
| 1,024 mapped 4096-aligned values (8-byte keys) | 1,007,376 | 8,606,888 |
| 70,000 boxed / mapped padded values | 104 / 1,007,494 | 266,075,112 / 266,557,860 |
| Refused boxed-resource replacement | 1,504 | 266,076,512 |
| 200,000 entity slots | 744 | 10,622,040 |
| Nested publications, depths 8 / 40 / 81 | 551 | 551 |
| Sparse 1 KiB components, eight types | 1,216 | 253,205,256 |
| Sparse 4096-aligned chunks | 1,216 | 253,394,856 |
| 100,000 allocating skipped defaults | 105 | 4,000,096 |

| Admission | Bound and refusal |
|---|---|
| Entity slots / registered types / query terms | 200,000 / 256 / 8 |
| Structural subscriptions / retained changes | 64 / 4,096; lag returns resync |
| Game and session log retention | 4,096 each; loss/reset is reported |
| Inspection output / log page | 65,536 bytes/visits / at most 512 records |
| Work / busy / derived slots | 64 each; reasons 256 bytes, at most 8 reported |
| Held keys / bindings per action / actions | 16 / 8 / 64; excess refuses before applying input |
| Input edge work per batch | ≤ 16,777,216 binding/key comparisons (2 × 1,024 × 64 × 8 × 16); including action/button/axis/edge scans < 60M string comparisons, each ≤ 128 bytes |
| Input / emitted messages | 1,024 queued each; messages 4,096 bytes |
| Clock advance / settle | 216,000 / 3,600 ticks per request |
| Binary/world/simulation output | Generic 256 MiB; World and Sim 128 MiB |
| Decoder allocation / string / nesting | Cumulative 256 MiB / 1 MiB / 256 |

Portable admission units remain architecture-independent: scalar widths, 24-unit container
headers and derived record/enum declarations. Overflow saturates to refusal; registration
rejects declarations above 256 MiB. Ownership scratch has its separate entity bound. Requests
past admission return errors, except programmer-facing infallible operations
(such as conflicting borrow use) which panic. Journal capacity does not cause mutation refusal. Saved tick and game-journal cursors above 2^62 refuse decode with
`cursor beyond supported range`; 2^62 is accepted. Structural, session and replacement
cursors are unsaved and cannot be supplied by a checkpoint. Runtime game-journal admission uses the same limit before mutation. Live generation `u32::MAX` refuses decode; `log` returns an error before dropping
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
`Sim::clock_ms() -> f64` reads the last accepted caller clock (`caller_us / 1000`), including pauses and rebases, preserved by restore/from_save/carry so hosts can resume from it.

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

Use the existing Cargo cache and shared target directory. Before a cold build,
check `df -h ~`; stop below 25 GiB free. These counts are allocator calls and
cumulative requested bytes, not resident memory, latency or first pixel.

| Operation | Starting `8d24068` calls / bytes | K1f calls / bytes |
|---|---:|---:|
| Empty World | 1 / 24 | 1 / 24 |
| 100-entity construction | 21 / 41,880 | 21 / 41,880 |
| First tick with publication | 2 / 568 | 2 / 568 |
| 1,000 changing-publication ticks | 3 / 200,704 | 3 / 200,704 |
| Exact 10 KiB restore | 80 / 46,760 | 80 / 46,760 |
| First 32-byte component at slot 199,999 | 4 / 77,536 | 4 / 77,536 |
| 1,000 high-slot remove/reinsert cycles | 0 / 0 | 0 / 0 |
| 1,000 sparse-edit ticks in 200k entities | 0 / 0 | 0 / 0 |
| 1,000 prepared input-heavy ticks | 0 / 0 | 0 / 0 |

Input fixtures prepare event strings, queue and edge buffers before counting;
they assert press/release edges and movement while assets remain pending. Ordinary
ticks perform zero Data writes; three explicit settle ticks perform 800,000 writes
across four boundaries. The 200k owner-removal control reaps interleaved descendants
without visiting ownership scratch, even after recycling the owner's slot.
The nested reservation-chain control at depths 8, 40 and 80 now refuses after
33,391 requested bytes; depth 8 previously allocated 262,430 bytes. A valid flat
boundary still loads, including 1,023 siblings plus a nested 1,024th value.

First high-slot insertion deliberately initializes at most 25,000 bytes of
presence and 50,000 bytes of directory on this 64-bit host, plus one value chunk.
Chunks and directory backing retain bounded high-water capacity until replacement.
Nonempty queries cost O(highest-live-slot / 64 × terms + rows); runs cost
O(directory chunks + runs + consumed values). Despawn visits at most 256 columns.
The 200k reverse-insertion/recycled-generation benchmark compares complete storage
implementations over 100 traversals and seven samples, checking every sum.

Final quiet-run medians (ns/row) are diagnostic, not a significance claim:

| Operation | Starting kernel | K1f engine | K1f kernel |
|---|---:|---:|---:|
| Dense query | 1.303 | 1.204 | 1.309 |
| Sparse query | 3.263 | 2.801 | 3.007 |
| Dense runs | 0.693 | 1.029 | 0.694 |
| Sparse runs | 2.937 | 57.809 | 2.932 |

Counts do not regress. Dense query/runs differ +0.5%/+0.1%, sparse runs -0.2%, and
sparse query -7.8%; these timings do not establish a statistically significant
change. Both runs used the existing benchmark and shared target directory; the
starting kernel was copied from `8d24068`, with its dependency version relabeled
only to distinguish the two path packages. No traversal optimization was added.

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
cargo test -p exact-world --test startup -- --nocapture
cargo test -p exact-world -p exact-world-derive -p exact-motion --no-fail-fast
cargo test -p exact-world -- --ignored
cargo clippy -p exact-world -p exact-world-derive -p exact-motion --all-targets -- -D warnings
cargo fmt -p exact-world -p exact-world-derive -p exact-motion -- --check
cargo test --manifest-path game/Cargo.toml -p exact-game --test world_kernel
cargo test --manifest-path game/Cargo.toml -p exact-game --test world_kernel \
  dense_and_sparse_page_throughput -- --ignored --nocapture
cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast
cargo clippy --manifest-path game/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path game/Cargo.toml --all -- --check
(cd game && bun test)
bun game/app/shells.mjs --test
# Every game/games/*/proof.mjs and game/tests/lanterns/proof.mjs takes linux.
```

Miri exercises leases, retained mutable rows, aligned ZSTs, owned/padded values,
replacement, slot reuse, failed readers and selected unwind paths. The new empty
query control uses 321 slots under Miri and 200,000 natively. No unsafe code moved
outside storage; the allocator used by refusal tests is test-only storage support.

```sh
cargo +nightly miri test -p exact-world --lib storage -- --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
cargo +nightly miri test -p exact-world --test pages --test ecs -- --test-threads=1 --skip resource_and_non_state_outputs
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --lib storage -- --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --test data portable_admission
cargo +nightly miri test -p exact-world --test admission -- --test-threads=1
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --test admission -- --test-threads=1
```

K5 retained mutable ZST collect/write control passes x86-64 and i686 under both
Stacked and Tree Borrows; all four runs report `1 passed; 0 failed`, with no pointer changes.

K1f Miri: 8 x86-64 storage, 13 page/ECS, 8 i686 storage, 1 i686 portable-boundary
and 12 x86-64/i686 admission executions passed, with no UB detected (42 total).
A real wasm32 build executed through Bun
passes all five frozen continuation boundaries and the 2,232-accept/2,231-refuse
admission boundary, skipped finite recursion and oversized 32-bit page admission.
Generated simulations compare 2,048 complete save/load/save
boundaries and hashes. These finite controls do not prove arbitrary manual Data.

The temporary cross-engine consumer compares common Data/hash content and world
bytes, normalizing only the EXGAME header. Both engines use the same reverse
insertion order. Contract conversions remain in game/world-adapter, outside core;
no root crate gained a dependency on the game workspace. F2 consolidation,
nonspatial app startup/artifact evidence, deployed old-save cutover and owner/judge
approval remain separate, unfinished admission work. This host has no GPU adapter,
Chrome or Apple SDK; browser/Apple rendering and first pixel are unverified.

Kernel/derive/core-motion tests: 185 passed, two large controls passed separately
after every commit; clippy denies warnings and formatting passes. The 18 reviewed
root motion-dependent tests plus a low-frequency compatibility control pass.
Cross-engine and all six adapter tests pass. Game workspace: 749 passed, 25 ignored,
18 missing-GPU failures; the optional motion module contributes nine passing tests.
Authored-game workspaces: 48 passed, one ignored, three missing-GPU failures.
Bun's full run reached 83 passes and two Chrome failures before an open server
prevented exit. Excluding those two tests completed with 146 passes, one skip and
one further Chrome failure (generated-game web proof): three unavailable checks.
Seven Linux game proofs pass; both Lanterns fixtures execute but lack pins. Cubes
is browser-only, failed process inventory and timed out. Root build/test/clippy require the absent lean
Hermes producer. Caps passes; boot stays at 88,699 JS bytes, 3,468 page bytes,
two pre-pixel modules and one Wasm reference. No game pins changed.

EXGAME v4 and EXSIM v10 are unchanged by K1f. `git diff land/game-next -- motion/`
is empty. Core dependencies are exact-world-derive, libm and ryu; optional
exact-world-motion depends on exact-world and exact-motion. The root workspace
does not include game, and Caltrain's normal dependency tree does not include world.

K1f totals 7,254 lines: 6,738 Rust + 493 README + 23 manifests, down from 7,479.
The extracted optional motion module adds 335 separate lines (326 Rust + 9 manifest)
under the same exclusions; kernel plus module totals 7,589, up 110 from old core alone.
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
