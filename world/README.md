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

Inspection and binary writers expose `stopped()`. Container loops stop after
refusal and `finish()` propagates the error. Record publication admits sequence
backing, strings, numeric vectors and nesting before allocation. Custom Data
writers must honor `stopped` and claim owned decode storage. Allocating defaults,
including skipped fields, must declare `default_size` or implement an admitted
`read_new`; a codec cannot bound arbitrary user code. Derive requires Data + Default
on skipped fields and charges every reset. Enum read_new constructs the selected
variant without invoking an unrelated manual enum Default. Manual enum defaults
charge the largest variant's defaults; unit-default enums charge their largest
inline variant. `Data::inline_size()` separates portable inline storage from
allocating defaults. Box contributes eight inline units; an empty Option does
not recurse into its payload's default. Derive's constant default check refuses
cyclic default-construction dependencies, including aliases and indirect cycles;
recursive enums use a derived unit Default or manual Data admission. Rust expands cfg and
cfg_attr before derive; retained, absent and conditionally skipped fields are tested.
Parser diagnostics still use invocation spans.

Publication decode charges one shared 65,536-unit allowance before children,
strings and object keys allocate; declared sequence lengths preflight it. Logical
depth is 80, leaving room within the 256 codec frames. Root keys separately have
the fixed 256 × 256-byte bound. Error paths truncate at 256 UTF-8 bytes; the small
diagnostic reserve stays bounded independently of rejected payload size.
`bin::read_into` stages a saved copy plus patch under one 256 MiB decode budget;
it costs a complete encode/decode and preserves the destination on decode error.
`World::hash`, `hash::of` and `Hasher::finish` return Result and refuse invalid
values, excessive strings, decode claims and nesting beyond 256 frames.

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

Use the existing Cargo cache and shared target directory. Before a cold build,
check `df -h ~`; stop below 25 GiB free. These counts are allocator calls and
cumulative requested bytes, not resident memory, latency or first pixel.

| Operation | K1d calls / bytes | K1e calls / bytes |
|---|---:|---:|
| Empty World | 1 / 24 | 1 / 24 |
| 100-entity construction | 21 / 41,880 | 21 / 41,880 |
| First tick with publication | 2 / 568 | 2 / 568 |
| 1,000 changing-publication ticks | 3 / 200,704 | 3 / 200,704 |
| Exact 10 KiB restore | 80 / 46,764 | 80 / 46,760 |
| First 32-byte component at slot 199,999 | 4 / 77,536 | 4 / 77,536 |
| 1,000 high-slot remove/reinsert cycles | 0 / 0 | 0 / 0 |
| 1,000 sparse-edit ticks in 200k entities | 0 / 0 | 0 / 0 |
| 1,000 prepared input-heavy ticks | 0 / 0 | 0 / 0 |

Input fixtures prepare event strings, queue and edge buffers before counting;
they assert press/release edges and movement while assets remain pending. Ordinary
ticks perform zero Data writes; three explicit settle ticks perform 800,000 writes
across four boundaries. The 200k owner-removal control reaps interleaved descendants
without visiting ownership scratch, even after recycling the owner's slot.
Hostile nested publications refused after 527–542 requested bytes, versus the
reproduced 3,201,238 bytes for 100,000 Unit children before the fix.

First high-slot insertion deliberately initializes at most 25,000 bytes of
presence and 50,000 bytes of directory on this 64-bit host, plus one value chunk.
Chunks and directory backing retain bounded high-water capacity until replacement.
Nonempty queries cost O(highest-live-slot / 64 × terms + rows); runs cost
O(directory chunks + runs + consumed values). Despawn visits at most 256 columns.
The 200k reverse-insertion/recycled-generation benchmark compares complete storage
implementations over 100 traversals and seven samples, checking every sum.

Final quiet-run medians (ns/row) are diagnostic, not a significance claim:

| Operation | K1d kernel | K1e engine | K1e kernel |
|---|---:|---:|---:|
| Dense query | 1.312 | 1.124 | 1.316 |
| Sparse query | 2.770 | 2.815 | 2.754 |
| Dense runs | 0.695 | 1.025 | 0.697 |
| Sparse runs | 2.933 | 55.953 | 2.937 |

Counts do not regress. Dense query/runs differ +0.3%, sparse runs +0.1%, and sparse
query -0.6%; these timings do not establish a statistically significant change.
A prior run concurrent with other checks was slower (dense query 1.371).

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
```

K1e Miri: 8 x86-64 storage, 13 page/ECS, 8 i686 storage and 1 i686 admission
executions passed, with no UB detected. A real wasm32 build executed through Bun
passes all five frozen continuation boundaries and the 2,232-accept/2,231-refuse
admission boundary. Generated simulations compare 2,048 complete save/load/save
boundaries and hashes. These finite controls do not prove arbitrary manual Data.

The temporary cross-engine consumer compares common Data/hash content and world
bytes, normalizing only the EXGAME header. Both engines use the same reverse
insertion order. Contract conversions remain in game/world-adapter, outside core;
no root crate gained a dependency on the game workspace. F2 consolidation,
nonspatial app startup/artifact evidence, deployed old-save cutover and owner/judge
approval remain separate, unfinished admission work. This host has no GPU adapter,
Chrome or Apple SDK; browser/Apple rendering and first pixel are unverified.

Final kernel/derive/motion tests: 184 passed, two large controls passed separately;
clippy denies warnings and formatting passes. Cross-engine and all six adapter
tests pass. Game workspace: 740 passed, 25 ignored, 18 missing-GPU failures.
Authored-game workspaces: 48 passed, one ignored, three missing-GPU failures.
Bun: 145 passed initially, one skipped, four failures; the lane's long-name literal
false positive was fixed and its check passes, leaving three Chrome failures.
Seven Linux game proofs pass; both Lanterns fixtures execute but lack pins. Cubes
requires Chrome and timed out. Root build/test/clippy require the absent lean
Hermes producer. Caps passes; boot stays at 88,699 JS bytes, 3,468 page bytes,
two pre-pixel modules and one Wasm reference. No game pins changed.

K1e totals 7,479 lines: 6,989 Rust + 466 README + 24 manifests.
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
