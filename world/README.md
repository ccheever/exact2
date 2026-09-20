# exact-world
An optional, nonspatial deterministic kernel. Root packages never depend on `game/`; ordinary apps do not acquire this dependency. `exact-world-derive` has no dependencies and implements `Data`, `Component`, `Resource`, and `Args` without syn. The generic state, codecs, leases and RNG were ported from `exact-game`; the cross-workspace test checks identical hashes and binary bytes with opposite insertion histories, recycled slots, floating-point edge cases, resources and delivery data.

```rust
use exact_world::*;
#[derive(Default, Component)] struct Counter(u32);
#[derive(Default, Args)] struct Options { #[live] increment: u32 }
struct Board;
impl Game for Board {
    const ID: &'static str = "readme-board";
    type Args = Options;
    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        w.register::<Counter>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &Options) { w.spawn_named("counter", Counter(0)).unwrap(); }
    fn tick(w: &mut World, _: &Input, a: &Options) { w.get_mut::<Counter>("counter").unwrap().0 += a.increment; }
}
let mut sim = Sim::<Board>::new(Options { increment: 2 })?;
assert_eq!(sim.run(17.)?, 1);
let saved = sim.save()?;
let restored = Sim::<Board>::from_save(&saved)?;
assert_eq!(restored.world().hash(), sim.world().hash());
let pages = restored.world().pages::<Counter>();
let page = pages.iter().next().unwrap();
assert_eq!(page.runs().next().unwrap().1[0].0, 2);
# Ok::<(), DataError>(())
```

| Public API (method signatures omit receivers) | Contract |
|---|---|
| `World::new(hz: u32, seed: u64) -> World`; `spawn(bundle: impl Bundle) -> Result<Entity, DataError>`; `spawn_named(name: impl AsRef<str>, bundle: impl Bundle) -> Result<Entity, DataError>`; `despawn(Entity) -> bool` | Lowest free slot; names resolve lowest live slot; generation rejects stale handles. Tuples implement Bundle. |
| `insert<C: Component>(Entity, C) -> Result<bool, DataError>`; `remove<C>(Entity) -> Option<C>`; `get<C>(impl Target) -> Option<Ref<C>>`; `get_mut<C>(impl Target) -> Option<RefMut<C>>`; `insert_resource<R: Resource>(R) -> Result<(), DataError>`; `resource<R>() -> Ref<R>`; `resource_mut<R>() -> RefMut<R>` | Column leases reject aliasing. `try_resource`, `has`, `named`, `resolve`, `name`, `entities` provide typed lookup. |
| `try_query<Q: Query>() -> Result<QueryBorrow<Q>, String>`; `QueryBorrow::get(Entity) -> Option<Q::Item>`; `pages<C>() -> Pages<C>`; `Page::runs() -> impl Iterator<Item=(u32, &[C])>` | Ordered sealed joins, optional terms, with/without/with_any filters, entity-bearing borrowed or guarded iteration; safe typed runs exclude holes and padding. |
| `id() -> WorldId`; `replacement()`, `revision<C>()`, `membership<C>()`, `entities_revision()`, `mutation_epoch() -> u64`; `Page { first, generation, .. }`; `Page::mask() -> &[u64]` | Cache keys include world identity/replacement; value generations conservatively advance on mutable access, membership only on structural edits. |
| `set_parent(Entity, Option<Entity>) -> Result<(), DataError>`; `children(Entity) -> Vec<Entity>`; `validate()`, `reap_orphans() -> Result<(), DataError>` | Independent cycle validation; children and deferred destruction use ascending slot order; Parent cannot be mutated through ordinary leases. |
| `change_cursor() -> u64`; `changes(since: u64) -> Result<impl Iterator<Item=&Change>, DataError>`; `consume_changes(through: u64) -> Result<(), DataError>` | Retained across ticks; generation-bearing spawn/despawn/insert/replace/remove/reparent/reset events. Consumers acknowledge their minimum cursor. |
| `work(&str, Work) -> Result<(), DataError>`; `clear_work(&str)`; `busy(&'static str) -> Result<(), DataError>`; `readiness() -> Readiness`; `settle_tick() -> Option<u64>`; `quiescent() -> bool`; `derived<T: Default + 'static>() -> RefMut<T>` | Ready/Pending/Failed reasons and simulation Deadline data; busy expires each tick. Derived slots are unsaved and cleared on replacement. Ambient still saves/hashes but is excluded from rest observation. |
| `Sim::<G>::new(G::Args)`, `from_save(&[u8]) -> Result<Sim<G>, DataError>`; `bind(G::Args)`, `input(InputEvent)`, `restore(&[u8]) -> Result<(), DataError>`; `carry(&[u8]) -> Result<bool, DataError>` | Game has ID, HZ, ACTIONS, typed Args, register/setup/tick and optional validate/paused. Args fields default to setup; live updates ticks; either restart edge reconstructs. Carry requires compatible setup and preserves current live args. |
| `run(elapsed_ms: f64)`, `advance_to(clock_ms: f64)`, `settle(max_ticks: u32) -> Result<u64, DataError>`; `alpha_inputs() -> (u64,u32,u32)`; `paranoid(Paranoid) -> Self` | Passive fixed ticks; alpha is tick plus numerator/1,000,000. Save/FreshGame reconstruct after every tick, with no setup/asset rebuilding. Timestamped Key/Action/Axis/Blur events; Input key/held/pressed/released/axis; `stick_axis([f32;2],[f32;2]) -> Result<[f32;2],DataError>` preserves the 60-pixel contact-offset rule. |
| `hash() -> u64`; `World::save() -> Result<Vec<u8>, DataError>`; `World::load(&[u8]) -> Result<(),DataError>`; `Sim::save() -> Result<Vec<u8>,DataError>`; `Data::{write(&mut dyn Writer), read(&mut dyn Reader) -> Result<(),DataError>}` | World EXGAME v4 omits empty columns. Sim EXSIM v9 saves caller time separately. Both refuse other envelope versions. Binary/streaming hash share Data traversal; json is bounded output only. `bin::{to_vec,from_slice,read_into}` and `hash::of` work on generic Data. |
| `publish(&str, impl Into<Published>)`; `publish_record(&impl Data)`; `published(&str) -> Option<Value>`; `take_published() -> Result<Option<String>,DataError>`; `emit(impl Into<String>)`; `take_messages() -> Vec<String>` | Positional Contract records remain distinct from lists and named publication objects. Game logs/publications/input/delivery survive Sim checkpoints. |
| `state(Entity)`, `logs(since: u64)`, `publications() -> Result<String,DataError>`; `tree() -> (Vec<(Entity,Option<&str>,Option<Entity>)>,usize)`; `log(&str)`, `session_log(&str) -> Result<(),DataError>`; `journal(since: u64) -> Vec<Event>` | Game journal is saved by Sim, excluded from World hash. Session messages are unsaved, anchored before the next game cursor, merged in order by logs; replacement reanchors retained session messages after restored history. Structural Reset never enters saved history. |
| `Spring::new(f32)`, `set_target(Now,f32)`, `value(Now) -> f32`; `Tween::new(f32)`, `to(Now,f32,f32)`, `value(Now) -> f32`; `rng() -> RefMut<Rng>` | Saved scalar motion over exact-motion; Tween uses its cubic easing, no private smoothstep. RNG and libm scalar math retain deterministic operations. |

State invariants: declaration-order fields, type-name-order storage, entity-order joins, canonical NaNs, signed-zero preservation; no semantic interior mutability in Data. Static descriptors and idempotent registration run each component hook once per registry, including recursive registration. Queues/scratch start empty; arguments are borrowed typed scalars at registration, never formatted. Restore streams one candidate in identity → typed args → registered storage names and contents → driver/delivery/journal order, using one cumulative 256 MiB allocation budget; all validation precedes commit. User Game/Data implementations must honor this contract and bound their own work.
Work bounds: 200,000 entity slots, 256 registered types, 8 query terms, 1,000,000 retained structural events, 4,096 game and session journal entries, 512 inspection rows, 64 work reasons (256 bytes each, 8 reported), 64 derived types, 1,024 input events and messages (4,096 bytes/message), 65,536 inspection output bytes (traversal refusal remains lane 2), depth 256, 1 MiB decoded strings, 128 MiB Sim save, 216,000 ticks/advance and 3,600 ticks/settle. External requests and spawn/insert return explicit errors; despawn/remove panic before exceeding journal capacity. Ownership validation/reaping is O(slots + edges); query membership O(slot words × terms × log allocated pages), observation/hash O(slots × registered types + visited Data), journal consumption O(retained events), decode O(admitted bytes/values). External pending/failed readiness refuses settle immediately; future simulation work must declare deadlines.
Pages contain 64 slots: a 40-byte component first allocates 2,560 bytes versus the predecessor's 40,960 (16× smaller); only inserted values are initialized, with no page-wide zero fill. At 200,000 dense entities this means 3,125 pages rather than 196, trading more upload runs for small-world startup. Counting-allocator ceilings (allocation calls / cumulative requested bytes, including realloc requests): empty World **1 / 24**; 100-entity Sim construction **28 / 70,848**; its first tick **3 / 640**; exactly 10,240-byte restore with 32 entities, journal and binary resource **1,038 / 106,671**. Counts repeat exactly; activation and restore are measured separately from fixture creation.
K1b validation: 79 passing tests including the README doctest, plus 19 compile-refusal cases in the derive test; two ignored controls explicitly run and passing (200k interleaved/churn and full capacity); cross-engine hash/byte comparison; compile refusal fixtures and this doctest. World and game clippy and formatting pass; the kernel also builds for wasm32-unknown-unknown. The following wasm formatter measurements are the pre-K1b baseline, not remeasured artifact admission. Ryu's reachable nonspatial wasm probe (release z, fat LTO, one codegen unit, stripped; gzip level 9, mtime 0): std 226,787 / 83,475 bytes, Ryu 209,642 / 77,396 bytes. This is a formatter admission comparison, not the complete adapter artifact required for later system admission; the adapter is outside K1.
Environment verification: game workspace 734 passed, 18 GPU-adapter failures, 24 ignored; game clippy and formatting pass. Seven consumer Linux proofs pass; Both consumer and separate 1,210-checkpoint Lanterns proofs have zero failures but no pins, so are unverified. The cubes benchmark proof requires Chrome even when given linux and could not run here. Bun completed with 146 passing, 1 skipped and 3 browser failures without Chrome. Authored game shell tests had 48 passing, 3 GPU failures and 1 ignored. The pre-K1b Caltrain Linux smoke stopped because view 20 was covered at the tap point; it was not repeated in this lane. Its dependency graph excludes both world crates. Root build/test/clippy stop at missing lean Hermes TypeScript bakes; caps and boot pass. The pre-K1b metrics snapshot refused the non-Git ibex dependency. Web/Apple/GPU execution and full adapter startup measurements remain unverified here.
K1b counts production Rust only: **6,498 lines**, excluding tests and cfg(test)
modules, with every source file below 1,500 lines. README and manifests do not
count against this task's ceiling. Reproduce from the repository root:
```sh
python3 - <<'PYCOUNT'
import re
from pathlib import Path
total = 0
for p in sorted(Path('world').rglob('*.rs')):
    if 'tests' in p.parts: continue
    s = p.read_text()
    if s.startswith('#![cfg(test)]'): continue
    s = re.sub(r'^#\[cfg\(test\)\]\nmod \w+ \{.*?^\}', '', s, flags=re.M|re.S)
    total += len(s.splitlines())
print('PRODUCTION RUST', total)
PYCOUNT
```

Registration is explicit: `register::<C>()` and `register_resource::<R>()` return
`Result<&mut World, DataError>` and are idempotent. `Game::register` and
`Component::register` return `Result<(), DataError>`. Declare every saved type in
`Game::register`; construction and every reconstruction invoke it with saved setup
arguments. Insertion never registers. Missing declarations name the type in a
`DataError`. Bundle spawning checks every declaration and the complete structural
journal requirement before allocating an entity. A panic in a structural mutation
or a failed registration hook poisons the world; discard it. A tick failing after
gameplay begins poisons the driver, preventing retry of partially executed logic.
Input refusal happens before the tick and leaves its boundary unchanged. Earlier
completed ticks in a multi-tick request remain committed on a later refusal.

`run` and `advance_to` use one caller clock. Pause advances that clock and reconciles
held input without advancing simulation time or retaining edges for resume.
`reconcile_input(clock_ms, held: &[InputEvent]) -> Result<(), DataError>` replaces
held state, clears edges and queued input, and rebases caller time without a tick;
its complete-state batch admits at most 1,024 events. Each delta rounds separately
to microseconds. `world_mut` remains available for authoring, but save and clock
operations refuse a replaced World whose tick/rate contradict the driver.

`bin::to_vec`, `Encoder::finish`, and `World::save` now return `Result<Vec<u8>,
DataError>`. Encoders refuse oversized strings, nesting, output and conservative
decoded-allocation estimates before output growth; collections stop on refusal.
World/Sim output is capped at 128 MiB, generic binary output at 256 MiB. Decode
owns a 256 MiB state allowance; ownership scratch is separately bounded to 200,000
slots (at most 2.6 MB on this 64-bit host). Capacity admission accounts native
layouts; the bytes and hashes themselves remain portable.

`Data::read_new` admits construction before calling defaults; boxed values claim
before allocation. Derives expose `default_size` recursively through fields.
Custom allocating defaults, including skipped fields, must supply an adequate `default_size` in a manual
Data implementation, or override `read_new` and claim before allocation. Manual
writers must account owned decode storage through `Writer::claim_decoded` and
honor `stopped`; arbitrary user code cannot be bounded by a codec.

Exact loading requires the complete canonical saved grammar to reproduce byte
for byte, including fields, value tags, storage roles, arguments and driver data.
This refuses silent defaulting, skipping, or lossy conversion before installation.
`World::carry` and `Sim::carry` explicitly permit field adaptation and return true
when the reconstructed canonical content differs; Sim also preserves current live
arguments and refuses setup drift. Exactness concerns saved content: compatible
Rust representations with identical canonical encoding remain interchangeable.
The extra validation pass raises the measured 10 KiB restore to 1,038 allocations /
106,671 requested bytes; eliminating metadata churn remains lane 2 work.

To keep production Rust under 6,500 lines, this revision deletes the duplicate
`Data::moving` traversal (use `settle_tick`), the redundant Sim name/role schema
table (storage maps and exact canonical comparison validate the same information),
unused `Component::SAVED_FIELDS`, `Sim::from_values`, `Now::seconds`, `World::{count,dt,seconds,seed,tick_end}`, required-value
lookup wrappers and Target labels, `Query::names`, `QueryBorrow::{one,matching_count}`,
and range/chance/pick RNG conveniences. Use `get`/`get_mut`, joined iteration,
`Args::decode`, and `Rng::{next_u32,next_f32}` directly. No state is removed and
common Data/hash encodings remain unchanged.

EXGAME v3 → v4 removes empty component columns from saves and hashes because
creation/removal history is not continuation state. EXSIM v8 → v9 adds the caller
clock required to discard paused time and removes the duplicate name/role schema
list; the nine-item envelope retains typed storage maps and exact comparison.
Generic Data tags, scalar encoding, NaN canonicalization, signed zero and the hash
algorithm remain unchanged. The cross-engine test normalizes only the world
version header and compares temporary-column churn against identical content.
First insertion at slot 199,999 requests four allocations / 3,064 bytes (previously
seven / 152,592); page metadata work is O(log allocated pages), with one page
allocated. Ordinary observation/reaping, streaming numeric payloads, journal
consumer ownership and metadata allocation on restore remain lane 2 work.

Live ticks do not observe Data. `World::observation() -> Option<bool>` returns
`None` at an unobserved boundary, `Some(false)` after observed change, and
`Some(true)` after an observed unchanged tick. `settle` samples its initial
boundary and each subsequent boundary once. Ordinary ticks visit zero component
values except those gameplay touches; ownership work is addressed separately.
The 200,000-entity startup regression counts zero writes across 1,000 live ticks
and 800,000 writes across three explicitly observed ticks (four boundaries).

Ownership reaping gates on entity and Parent revisions. Unchanged ticks do no
slot walk and allocate nothing. Changed boundaries reuse one byte per slot of
scratch; each edge is followed at most twice through the Parent query (a page
lookup costs O(log allocated pages)). Validation shares that scratch. A reversed
chain at slot 199,999 and generation recycling exercise the destructive case.

Restore now borrows field/variant names from the input through `Reader<'data>`.
Duplicate tracking reuses a stack and per-name depth marks; nested records restore
outer marks, including hostile repeated literal name definitions. Encoder field
and variant names are static; dynamic map names use `Writer::key`. Sequences
reserve their admitted length once. The exact 10 KiB restore costs **80 allocations /
47,297 requested bytes**, down from 1,038 / 106,671 (13× fewer calls). Remaining
allocations are saved state/storage/journal strings, codec metadata and canonical
re-encoding. Byte requests are not 10× smaller: decoded storage and the 10 KiB
canonical comparison buffer are still owned. No wire/hash tags changed.

Structural consumers use `subscribe_changes() -> Result<ChangeConsumer, DataError>`,
`changes(&consumer) -> Result<Changes<'_>, DataError>` and
`acknowledge_changes(&consumer, through)`. A batch has `next`, `resync` and an
exact-size `events` iterator. There are at most 64 subscribers and 4,096 retained
changes. No subscriber means no structural events allocated or retained. Dropped
subscriptions are pruned on the next structural edit/subscription/acknowledgement.
The minimum acknowledgement releases history; lag beyond the retained suffix
returns an empty batch with `resync: true`. Rebuild derived state, then acknowledge
`next`. Restore transfers subscriptions and appends Reset; gameplay edits never
fail because a subscriber is slow. Reads index directly into the retained deque.
Portable scalar math functions now directly re-export their libm implementations;
this removes forwarding wrappers without changing signatures or arithmetic.

`Writer::bytes(Bulk<'_>)` borrows typed U8/U16/U32/F32 slices. `Bulk::chunks`
streams at most 1,024 bytes of stack conversion space, preserving little-endian
bytes and canonical NaNs. Hashing a four-megabyte numeric vector allocates zero
bytes; inspecting a 65,536-byte numeric vector allocates under 1,024 bytes. The
cross-engine fixture rechecks all four bulk tags, generic Data bytes and world
hash/content parity. It uses the same reverse component insertion order in both
engines; it does not prove opposite insertion histories between engines.

Inspection and binary sinks expose `Writer::stopped`; container loops stop on
refusal. Record publication also admits strings, numeric vectors, sequence backing
and nesting before allocating. A million nested values stops inspection before
6,000 element visits, and an already-refused inspection performs zero subsequent
allocations. Public method descriptions are consolidated here (included in rustdoc);
storage safety and mutation contracts remain beside their implementations.

Merged logs use `logs(LogCursor::default()) -> Result<Logs, DataError>`, then
`logs(page.next)`. The opaque, unsaved cursor tracks replacement plus independent
game/session offsets. `Logs` reports `reset`, `truncated`, `next` and JSON `entries`.
A page admits at most 512 records and 65,536 escaped output bytes before visiting
records; smaller pages accommodate long messages. Saved game event indices and
EXSIM bytes do not change. Session-only output progresses across any number of
pages up to the retained 4,096 entries; overflow and restore are explicit.

The merged cursor replaces `journal(since)`; structural consumers read `batch.next` instead of the removed global `change_cursor()`. The `tree` and `children` collection
wrappers are removed: bounded adapters use `entities().take(512)` and query Parent
explicitly. These avoid a second log-reading protocol and convenience collectors
that duplicate the existing iterator APIs. Public argument/math/motion descriptions
are consolidated in this included README; arithmetic and binding behavior are unchanged.

The tested item-8 prototype is deferred: its 125 added production lines do not fit
the 6,500 ceiling. It provides erased component/resource visitors, isolated candidate
patching and commit validation, and bounded observation reports. The local patch
and its public-API-only fixture are in `~/lanes/gamenext/scratch/K1b-2/external-seams.patch`.
Moving the 125-line Contract record publication adapter (`RecordWriter` and
`publish_record`) to a future adapter crate would fund it; this run preserves that API.

Unchanged publications return before validation of the retained publication set;
unchanged work entries return before allocation or replacement. A 1,000-iteration
counting test records zero allocations and unchanged mutation/journal generations.
The comparative throughput fixture is reproducible with
`cargo test --manifest-path game/Cargo.toml -p exact-game --test world_kernel dense_and_sparse_page_throughput -- --ignored --nocapture`.
It builds its generated consumer in release and shares the root target directory.
At 200,000 slots: dense query 1.202/1.754 ns per row (engine1024/kernel64), dense
runs 1.027/0.923; sparse query 2.833/30.737, sparse runs 58.278/33.816. Sparse means
2,062 rows spread at stride 97 after slot recycling; insertion is reversed. This
compares complete storage implementations, not page size alone. For a workload
dominated by bulk sparse queries I would choose 1,024 slots with the engine's flat
mask lookup; 64 remains a startup trade, not a universal performance win.
