# exact-world

`exact-world` is an optional, deterministic, nonspatial world kernel. It owns typed
state, entity lifetimes, fixed ticks, input, saves and publications. Games and
optional modules supply behavior. Ordinary Exact apps do not acquire it, and no
root package depends on the separate `game/` workspace.

## A small lifecycle

Register saved types before constructing entities. Keep continuation state in
components, resources and arguments; the game implementation itself is stateless.

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
    fn setup(w: &mut World, _: &Options) {
        w.spawn_named("counter", Counter::default()).unwrap();
    }
    fn tick(w: &mut World, _: &Input, args: &Options) {
        w.get_mut::<Counter>("counter").unwrap().points += args.increment;
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
queries return ascending entity slots. Hashing and binary encoding canonicalize
NaNs and preserve signed zero. Semantic Data has no interior mutability. Manual
implementations must obey the visitor and admission contracts below. Explicit
`hash()` always walks current Data; it no longer caches by the mutation epoch.

`Entity` contains a slot and generation. Reusing the lowest free slot does not
revive an old handle. `entity_at(index: usize) -> Option<Entity>` checks the slot
and returns its current live incarnation; dead and out-of-range indices return
`None`. Names select the lowest live matching slot. `resolve` accepts a name,
`#12`, or `name#12`; explicit index selectors take precedence over names.

Registration is explicit and fallible. A short-name collision refuses and names
both full Rust types; stale-handle `insert` returns `Err`. `register::<C>()` and
`register_resource::<R>()` are idempotent; component hooks run once per registry,
including recursive registration. `Game::register` receives borrowed setup/restart
arguments, without formatting them. Insertion never registers types implicitly.
Registration and setup must not keep hidden continuation state.

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
Mutable access, live argument binding and held-input reconciliation invalidate the report. `quiescent()` requires
an observed unchanged boundary and a current settle deadline.

`settle(max_ticks)` samples the initial boundary once, then each completed
boundary once. It excludes Ambient components/resources from rest observation,
while they remain saved and hashed. Pending or failed external work refuses
settle immediately; virtual time cannot finish I/O. `Work::Deadline(tick)` declares
future simulation work. `busy(reason)` lasts one tick. Repeating the same work or
publication value does not replace it or invalidate the world.

`sample() -> Result<Sample, DataError>` explicitly reports the observation hash,
hashed bytes and visited components. It admits at most 32 MiB of hashed Data and
1,000,000 live-column/slot probes, refusing during traversal. This costs O(slots × storage
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
changed source boundary (including caller clock, input and delivery) before mutation.
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

### Ownership and structural consumers

Parent is an ownership edge, independent of spatial transforms. Only
`set_parent` changes it. Parent insertion rejects stale handles and cycles. Each ancestry check admits
at most 256 edges, returning an error past that work bound before mutation. This
bounds natural chain construction; it is not a global maximum depth, since reverse
construction can form deeper valid chains. Validation still walks each edge once.
Despawn removes the entity immediately; descendants leave at the next reap in
ascending slot order. Reusing a dead parent's slot cannot rescue descendants.

A derived owner-count index makes an unrelated despawn O(log owners), without
an ownership scan. Only removal of an entity with children arms reaping; valid
spawns, Parent edits and unchanged ticks do not trigger it. A necessary reap reuses one byte per slot
of scratch. Its three-color walk follows each edge at most twice through the
Parent query: O(slots), plus removal work. Scratch
is bounded to 200,000 bytes of initialized status entries; allocation capacity
can retain the vector's bounded high-water growth. Independent validation uses
the same scratch. Every orphan generation is checked before removal starts.

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

World saves remain **EXGAME v4**; simulation saves remain **EXSIM v10**. This lane
changes neither format nor generic Data/hash tags. Empty columns are omitted.
EXSIM includes caller time separately from simulation time. Other envelope
versions are refused, without migration.

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
Owned map keys still claim storage, and decoder accounting stays cumulative.
Sequences reserve their admitted known length once.

`Writer::field` and `variant` take static names; dynamic map names use `key`.
`Writer::bytes(Bulk<'_>)` receives borrowed U8/U16/U32/F32 slices. Numeric chunks
use at most 1,024 stack bytes; there is no payload-sized conversion vector in
hashing, saving or inspection. Tags, little-endian order and float normalization
remain unchanged. Numeric decoding still owns both raw and converted buffers;
its cumulative budget counts both.

Inspection and binary writers expose `stopped()`. Container loops stop after
refusal and `finish()` propagates the error. Record publication admits sequence
backing, strings, numeric vectors and nesting before allocation. Custom Data
writers must honor `stopped` and claim owned decode storage. Allocating defaults,
including skipped fields, must declare `default_size` or implement an admitted
`read_new`; a codec cannot bound arbitrary user code.

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
| Decoder allocation / string / nesting | Cumulative 256 MiB / 1 MiB / 256 |

Admission uses architecture-independent wire units, not `size_of`: scalar widths,
24 units for text/container headers, 16 plus fields for derived records, and fixed
metadata/chunk accounting. `Data::default_size()` declares those units, including
allocating defaults; manual implementations must supply conservative fixed values.
This is a wire/work allowance, not a resident-memory measurement of arbitrary Rust
layouts. Native and 32-bit Miri readers share the same 2,232-unit boundary fixture. Ownership scratch has its separate entity bound. Requests
past admission return errors, except programmer-facing infallible operations
(such as conflicting borrow use) which panic. Journal capacity does not cause mutation refusal. Exhausted journal cursors and
live generation `u32::MAX` refuse decode; `log` returns an error before dropping
retained events. A dead exhausted slot is retired permanently. Spawn selects the lowest reusable
free slot, skipping at most 200,000 retired indices, then appends if capacity
permits. Retirement is saved in the existing generation/free fields (no wire change).

### Clock, arguments and scalar helpers

`run` and `advance_to` share one caller clock. Each nonnegative millisecond delta
rounds separately to the nearest microsecond, with half-microseconds rounded up.
Sub-microsecond deltas are not accumulated before rounding. `alpha_inputs()` still
returns `(tick, numerator, 1_000_000)`; typed clock/phase result types are deferred.
Both paused and playing input use half-open timestamp boundaries. Clock rebasing
and settle range-check in widened arithmetic before mutation, in debug and release.
Pause advances caller time and reconciles held input without simulation ticks or
retained resume edges. `reconcile_input(clock_ms, held)` atomically replaces held
state, clears edges/queued input and rebases caller time without a tick.

Args fields default to Setup. `#[live]` changes subsequent ticks; either boolean
`#[restart]` edge reconstructs the world. Registration sees only setup/restart
fields. Derive supports typed portable scalars; nonfinite values and oversized text refuse
before mutation. Full-width integers remain typed core values; Contract safe-range
checks belong to the adapter. Input exposes key, held,
pressed, released and scalar axis state. Axis declarations still require a key
pair; a dedicated analog-only declaration is deferred. `stick_axis` uses the
60-point contact-offset rule; contact ownership belongs to a module.

`Spring` and `Tween` are saved scalar motion over exact-motion. Tween uses its
cubic easing; Spring state is private and rejects nonfinite scalar input. Tween, Spring and
SpringConfig writers refuse invalid values using `Writer::reject`; successful
built-in saves survive their corresponding reader. Scalar
math re-exports libm's f32 functions directly. Angles are radians; `round` rounds
halfway away from zero. `lerp` permits extrapolation, `smoothstep` clamps between
distinct increasing edges, and `wrap_angle` returns [-pi, pi). `ease` uses a
nonnegative time constant and snaps within 1e-4; zero lag arrives immediately.
`Rng::next_u32` and `next_f32` are the deterministic RNG primitives.

## Measurements and reproduction

Use the pinned Bun and the existing Cargo caches. Do not create another target
directory. Before a cold build, check `df -h ~`; stop below 25 GiB free.

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH
export EXACT_UPDATE_TRUST=development
cargo test -p exact-world --test startup -- --nocapture
cargo test -p exact-world --no-fail-fast
cargo test -p exact-world -- --ignored
cargo clippy -p exact-world -p exact-world-derive --all-targets -- -D warnings
cargo fmt -p exact-world -p exact-world-derive -- --check
```

Counts below are allocation calls / cumulative requested bytes, including realloc
requests, on this Linux 64-bit builder. Fixtures are constructed before measuring
activation or restore. The test repeats restore and asserts identical counts.
These are allocation/work counts, not first-pixel or GPU startup measurements.
The input fixture establishes held state and reusable edge buffers and constructs
all event strings/queue backing before counting the 1,000 consuming ticks. It
checks 251 press/release edges and nonzero movement while assets remain pending;
removing event delivery fails the test. The old clone path fails the zero-allocation
assertion with 68,751 allocations. New input-state capacity and event creation are
not claimed to be allocation-free.

| Fixture | K1c | K1d |
|---|---:|---:|
| Empty World | 1 / 24 | **1 / 24** |
| 100-entity construction | 21 / 41,968 | **21 / 41,880** |
| First tick, including first publication | 3 / 640 | **2 / 568** |
| 1,000 ticks with sparse edits among 200,000 entities | 0 / 0 | **0 / 0**, zero component visitor calls |
| 1,000 ticks with 64 held keys, an axis and 1,000 queued events | 0 / 0 | **0 / 0** |
| 1,000 ticks changing a publication every tick | 1,003 / 204,704 | **3 / 200,704** |
| Exact 10,240-byte restore | 80 / 46,873 | **80 / 46,764** |
| First component at slot 199,999 | 4 / 77,536 | **4 / 77,536** |
| 1,000 empty-column remove/reinsert cycles at slot 199,999 | 2,000 / 77,048,000 | **0 / 0** |

The 1,000-publication case now pays only for three journal capacity growths. This is game-produced output, not hidden component observation.
The 200,000-entity counter test subsequently performs three settle ticks and
counts exactly 800,000 component writes: four boundaries, including the initial
sample. That positive control catches an observer that simply returns nothing.
Unchanged ownership reaping allocates nothing and leaves its status scratch
unvisited; reverse chains, parent-slot recycling and later reuse exercise the
unfavorable case. Spawning into a valid hierarchy also performs no ownership walk.

Relative to lane 1 (1,038 / 106,671), restore allocation calls remain **13×** lower
and requested bytes **2.28×** lower. K1d preserves the 80 allocation calls.
The histogram printed by the test locates the remaining requests, including the
full canonical comparison encoding, the Blob, event backing and value chunk.
Thirty-two nine-byte strings are saved CellValue event names. Sharing publication
keys removes their steady-state allocations without changing any Data/hash tags.

Hashing a 4 MB numeric vector allocates **0 / 0**. Inspection of a 65,536-byte
numeric vector allocates under 1,024 bytes; an oversized vector refuses before
conversion. A million nested inspection values stop before 6,000 element visits.
An already-refused inspection incurs zero further allocations. Unchanged work
and publication updates allocate nothing across 1,000 repetitions and preserve
mutation/journal generations. Clock tests distinguish per-delta rounding from
accumulating fractional microseconds.

### Dense and sparse bulk readers

```sh
cargo test --manifest-path game/Cargo.toml -p exact-game --test world_kernel \
  dense_and_sparse_page_throughput -- --ignored --nocapture
```

The fixture builds a temporary **release** consumer using the shared root target.
It compares complete storage implementations, not an isolated page-size switch.
Both have 200,000 slots, reversed insertion, every eleventh slot recycled, a
40-byte component, and 100 traversals. Sparse rows occur at stride 97: 2,062 live
values. Counts/sums are checked every traversal; clocks are diagnostic samples.

Medians of seven samples, each with 100 traversals, on this same builder:

| Shape / operation, ns/row | K1c engine | K1c kernel | K1d engine | K1d kernel |
|---|---:|---:|---:|---:|
| Dense query | 1.211 | 1.309 | 1.121 | 1.312 |
| Sparse query | 2.798 | 2.981 | 2.804 | 2.770 |
| Dense runs | 1.034 | 0.704 | 1.026 | 0.695 |
| Sparse runs | 58.212 | 2.910 | 56.018 | 2.933 |

Kernel dense queries changed +0.2%, sparse queries -7.1%, dense runs -1.3%,
and sparse runs +0.8%. These are timing samples, not a significance claim. The
paired kernel/engine query ratios are 1.170× dense and 0.988× sparse. Both query
targets pass; allocation and work bounds have no regression.

Presence occupies a flat, geometrically grown array, capped at **25,000 bytes**
for 200,000 slots. A direct chunk directory shares its allocation (50,000 bytes
at capacity on this 64-bit machine). New empty columns allocate neither. Once
allocated, metadata and value chunks retain their bounded high-water backing until
World destruction/replacement. Removal clears presence and drops the value without
a directory scan; public page iteration and save accounting skip empty chunks. Only metadata is zeroed. Each 64-value chunk is
allocated uninitialized and owns exactly the slots whose presence bits are set.
Chunks keep separate write generations; typed runs skip holes with bit operations.

64 values retain small-world allocation: a 40-byte component requests **2,560 B**
per chunk (100 entities: 5,120 B); a 4-byte component requests **256 B** (100:
512 B). A 1,024-value allocation would request 40,960/4,096 B immediately. Query
joins scan flat words without chunk lookups; only matched words fetch pointers.
The bounds remain 200,000 slots, eight terms and four filters; excess world/query
admission refuses. Query work is O(slots/64 × terms + returned rows); runs cost
O(directory chunks + present runs + values actually consumed).

Counted construction is **21 / 41,880**, restore **80 / 46,764**, and first
component at slot 199,999 is **4 / 77,536**. The last byte count rises deliberately
with flat metadata; it still allocates only one value chunk. Empty World, first
tick, publication ticks and 1,000 sparse-edit ticks retain their prior counts.

### Parity and environment checks

```sh
cargo test --manifest-path game/Cargo.toml -p exact-game --test world_kernel
cargo test --manifest-path game/Cargo.toml -p exact-world-adapter
cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast
cargo clippy --manifest-path game/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path game/Cargo.toml --all -- --check
(cd game && bun test)
bun game/app/shells.mjs --test
# Run each game/games/*/proof.mjs with linux, and game/tests/lanterns/proof.mjs linux.
```

The cross-engine fixture compares common World hashes, canonical world bytes and
generic Data bytes. It covers recycled slots, floating-point edge cases, resources,
delivery and temporary-column churn in the new kernel. It uses the **same reverse
component insertion order** in both implementations; it does not compare opposite
insertion histories across engines. It normalizes only the EXGAME header version;
the old engine's unpopulated column history is compared with new-kernel
create/remove history. The separate 200k churn test checks history independence.

K1d ran 123 kernel tests successfully (including the doctest and compile probes),
plus both ignored large controls separately. Cross-engine and all five frozen
continuation boundaries pass unchanged. Game workspace: 740 pass, 25 ignored,
18 fail exclusively for the missing GPU adapter. Authored-game tests: 48 pass,
one ignored, three GPU failures. Bun: 146 pass, one skipped, three Chrome-dependent
failures. Game and kernel clippy with warnings denied and formatting pass.

Linux proofs pass for asset-fixture, Beacons, Greybox, particles-fixture,
placement-fixture, skinned-fixture and sprites-fixture. Both Lanterns fixtures
run but refuse empty pins; no parity claim is made. The cubes benchmark invokes
Chrome even with `linux` and reached its 300-second external deadline.
Root workspace build/test/clippy are blocked by the absent lean Hermes producer
for weatherlight/fieldnotes. Caps passes all 785 sources; boot reports 88,699 JS
bytes, 3,468 page bytes, two pre-pixel JS modules and one Wasm reference.
The box has no GPU adapter, Chrome or Apple SDK. Rendering, GPU residency, real
browser/Apple execution and first pixel cannot be certified here. F2 consolidation,
complete-consumer artifact measurements and owner/judge acceptance remain open.

### Unsafe boundary verification

Nightly `1.100.0-nightly (feaadeeac 2026-09-19)` with Miri installed successfully.
The final storage command passed seven tests; query/page commands passed thirteen.
The same seven storage tests also passed under i686 Miri, plus its near-limit
admission test. A direct wasm32 build executed through Bun preserved all five
complete frozen checkpoint boundaries and matched native/i686 admission exactly:
2,232 units accepts and 2,231 refuses. The redundant full i686 Miri continuation
run was cancelled after this real wasm result; it is not counted as a pass.
No undefined behavior was detected. This includes aligned ZST retained rows,
lease unwind, padded/owned moves, slot reuse, rejecting readers and panicking drops.
The 200k directory stress stays native; Miri's churn/overalignment sizes are reduced
while still crossing chunks. The equal-address ZST allegation was not reproduced.

```sh
cargo +nightly miri test -p exact-world --lib storage -- --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
cargo +nightly miri test -p exact-world --test pages --test ecs -- --test-threads=1 --skip resource_and_non_state_outputs
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --test data portable_admission
cargo +nightly miri test --target i686-unknown-linux-gnu -p exact-world --lib storage -- --test-threads=1 --skip presence_is_flat_bounded_and_values_stay_lazy_after_churn
```

The raw query `Fetch` trait and reference constructors are crate-private. External
compile probes refuse both direct construction and a generic attempt to mint a
`'static` row. Unsafe code remains confined to `world/src/storage/`.

Derives require `Default` on every field, including skipped fields, even when the
outer type supplies its own manual Default. Tuple/container replacement and enum
switching construct field defaults. The compile harness checks a skipped
`NoDefault` field and requires the explicit diagnostic "Data fields require Default,
including skipped fields", naming its type. Field-spanned parser diagnostics remain
deferred; this lane does not introduce a second parser or weaken the bounds.

## Public surface and remaining work

The core authoring APIs remain `spawn`, `spawn_named`, `insert`, `remove`,
`despawn`, typed component/resource leases, ordered queries and pages. Mutation
admission is explicit and fallible where declared; name/generation validation
precedes structural commit. `entity_at` now provides checked page-index lookup.
For shared state use resources; `derived<T>()` is unsaved and cleared on replacement.
Its 64 lazily initialized cells borrow independently; lookup inspects at most 64
type IDs. Reborrowing the same T exclusively still refuses.
For delivery use kernel `publish`, borrowed `publications`, `take_published`,
`emit` and `take_messages`. The redundant cloning `published(key)` getter was
removed; borrow `publications().get(key)` instead. Contract `publish_record` and
value/message conversion belong to `exact-world-adapter::publication`. The world
hash excludes delivery queues; Sim saves retain them.

The merged log cursor replaces `journal(since)`, and structural batches expose
`next` instead of the removed global `change_cursor()`. The `tree` and `children`
collection wrappers are removed: adapters can bound `entities().take(512)` and
query Parent explicitly. Public method descriptions are consolidated in this
included README; storage safety and mutation invariants remain beside the code.

The external seam is shipped and exercised by five public-only tests: erased
component/resource visitation, candidate edits and atomic commit/refusal, bounded
observation (including mostly empty maximal worlds), and resource revisions.
The original three prototype tests landed with two additional adverse controls.
Conversion of named Data records, positional Contract values and the untagged
Contract message envelope moved to the real `game/world-adapter` crate. That
adapter enforces conversion size/depth limits and is tested outside the root
workspace, including byte/hash parity against the old engine.

A kinds/space author can register types, hold independent structural cursors, use
safe typed pages and resolve indices to live handles. Reload can use `visit`,
`candidate`, `Candidate::edit` and `Candidate::commit`; merge policy remains
external. Typed clock/phase results, query `one()` and analog-only declarations
remain deferred under the production ceiling.

`tests/fixtures/k1b-continuation.bin` was produced from commit `428436d` with the
fixture in `tests/continuation.rs`. Five boundaries compare complete EXGAME v4
and EXSIM v9 bytes plus World hashes after reversed insertion, recycled slots,
resource changes, ownership, publications, pending assets and queued input. The
separate cross-engine test still proves common grammar/hash parity.

The ceiling is **7,500 lines for all handwritten kernel material**: production
Rust in world/ and world/derive, README and both manifests. Tests alone are excluded;
comments and blank lines count. K1d totals 7,311: 6,732 Rust + 555 README + 24 manifests.
Reproduce the honest total with:

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

EXSIM v10 corrects lost publication delivery: byte 6 changes 09→0a, the outer
sequence count at byte 8 changes 9→10, and one final boolean byte records pending
delivery. EXGAME v4, hashes and all prior payload fields are unchanged. The frozen
v9 inventory normalizes precisely these bytes; empty/pending/drained restore,
from-save and carry have independent controls. v9 refuses; no migration ships.

`bin::read_into(bytes, &mut value)` stages a saved copy and patch under one
256 MiB decode allowance, replacing only on successful completion. This explicit
patch path costs a complete encode/decode of the existing value.
