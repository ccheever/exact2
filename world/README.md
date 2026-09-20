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
    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) { w.register::<Counter>(); }
    fn setup(w: &mut World, _: &Options) { w.spawn_named("counter", Counter(0)); }
    fn tick(w: &mut World, _: &Input, a: &Options) { w.require_mut::<Counter>("counter").0 += a.increment; }
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
| `World::new(hz: u32, seed: u64) -> World`; `spawn(bundle: impl Bundle) -> Entity`; `spawn_named(name: impl AsRef<str>, bundle: impl Bundle) -> Entity`; `despawn(Entity) -> bool` | Lowest free slot; names resolve lowest live slot; generation rejects stale handles. Tuples implement Bundle. |
| `insert<C: Component>(Entity, C) -> bool`; `remove<C>(Entity) -> Option<C>`; `get<C>(impl Target) -> Option<Ref<C>>`; `get_mut<C>(impl Target) -> Option<RefMut<C>>`; `insert_resource<R: Resource>(R)`; `resource<R>() -> Ref<R>`; `resource_mut<R>() -> RefMut<R>` | Column leases reject aliasing. `require`, `require_mut`, `try_resource`, `has`, `named`, `resolve`, `name`, `entities`, `count` provide typed lookup. |
| `try_query<Q: Query>() -> Result<QueryBorrow<Q>, String>`; `QueryBorrow::get(Entity) -> Option<Q::Item>`; `matching_count(&World) -> Result<(usize, Option<Entity>), String>`; `pages<C>() -> Pages<C>`; `Page::runs() -> impl Iterator<Item=(u32, &[C])>` | Ordered sealed joins, optional terms, with/without/with_any filters, entity-bearing borrowed or guarded iteration; safe typed runs exclude holes and padding. |
| `id() -> WorldId`; `replacement()`, `revision<C>()`, `membership<C>()`, `entities_revision()`, `mutation_epoch() -> u64`; `Page { first, generation, mask, .. }` | Cache keys include world identity/replacement; value generations conservatively advance on mutable access, membership only on structural edits. |
| `set_parent(Entity, Option<Entity>) -> Result<(), DataError>`; `children(Entity) -> Vec<Entity>`; `validate()`, `reap_orphans() -> Result<(), DataError>` | Independent cycle validation; children and deferred destruction use ascending slot order; Parent cannot be mutated through ordinary leases. |
| `change_cursor() -> u64`; `changes(since: u64) -> Result<impl Iterator<Item=&Change>, DataError>`; `consume_changes(through: u64) -> Result<(), DataError>` | Retained across ticks; generation-bearing spawn/despawn/insert/replace/remove/reparent/reset events. Consumers acknowledge their minimum cursor. |
| `work(&str, Work) -> Result<(), DataError>`; `clear_work(&str)`; `busy(&'static str) -> Result<(), DataError>`; `readiness() -> Readiness`; `settle_tick() -> Option<u64>`; `quiescent() -> bool`; `derived<T: Default + 'static>() -> RefMut<T>` | Ready/Pending/Failed reasons and simulation Deadline data; busy expires each tick. Derived slots are unsaved and cleared on replacement. Ambient still saves/hashes but is excluded from rest observation. |
| `Sim::<G>::new(G::Args)`, `from_values(&[Value])`, `from_save(&[u8]) -> Result<Sim<G>, DataError>`; `bind(G::Args)`, `input(InputEvent)`, `restore(&[u8])`, `carry(&[u8]) -> Result<(), DataError>` | Game has ID, HZ, ACTIONS, typed Args, register/setup/tick and optional validate/paused. Args fields default to setup; live updates ticks; either restart edge reconstructs. Carry requires compatible setup and preserves current live args. |
| `run(elapsed_ms: f64)`, `advance_to(clock_ms: f64)`, `settle(max_ticks: u32) -> Result<u64, DataError>`; `alpha_inputs() -> (u64,u32,u32)`; `paranoid(Paranoid) -> Self` | Passive fixed ticks; alpha is tick plus numerator/1,000,000. Save/FreshGame reconstruct after every tick, with no setup/asset rebuilding. Timestamped Key/Action/Axis/Blur events; Input key/held/pressed/released/axis; `stick_axis([f32;2],[f32;2]) -> Result<[f32;2],DataError>` preserves the 60-pixel contact-offset rule. |
| `hash() -> u64`; `World::save() -> Vec<u8>`; `World::load(&[u8]) -> Result<(),DataError>`; `Sim::save() -> Result<Vec<u8>,DataError>`; `Data::{write(&mut dyn Writer), read(&mut dyn Reader) -> Result<(),DataError>}` | World EXGAME v3 remains byte-compatible. Sim EXSIM v8 refuses every other version. Binary/streaming hash share Data traversal; json is bounded output only. `bin::{to_vec,from_slice,read_into}` and `hash::of` work on generic Data. |
| `publish(&str, impl Into<Published>)`; `publish_record(&impl Data)`; `published(&str) -> Option<Value>`; `take_published() -> Result<Option<String>,DataError>`; `emit(impl Into<String>)`; `take_messages() -> Vec<String>` | Positional Contract records remain distinct from lists and named publication objects. Game logs/publications/input/delivery survive Sim checkpoints. |
| `state(Entity)`, `logs(since: u64)`, `publications() -> Result<String,DataError>`; `tree() -> (Vec<(Entity,Option<&str>,Option<Entity>)>,usize)`; `log(&str)`, `session_log(&str) -> Result<(),DataError>`; `journal(since: u64) -> Vec<Event>` | Game journal is saved by Sim, excluded from World hash. Session messages are unsaved, anchored before the next game cursor, merged in order by logs; replacement reanchors retained session messages after restored history. Structural Reset never enters saved history. |
| `Spring::new(f32)`, `set_target(Now,f32)`, `value(Now) -> f32`; `Tween::new(f32)`, `to(Now,f32,f32)`, `value(Now) -> f32`; `rng() -> RefMut<Rng>`; `rand(Range<T>) -> T` | Saved scalar motion over exact-motion; Tween uses its cubic easing, no private smoothstep. RNG and libm scalar math retain deterministic operations. |

State invariants: declaration-order fields, type-name-order storage, entity-order joins, canonical NaNs, signed-zero preservation; no semantic interior mutability in Data. Static descriptors and idempotent registration run each component hook once per registry, including recursive registration. Queues/scratch start empty; arguments are borrowed typed scalars at registration, never formatted. Restore streams one candidate in identity → typed args → registered schema → components → driver/delivery/journal order, using one cumulative 256 MiB allocation budget; all validation precedes commit. User Game/Data implementations must honor this contract and bound their own work.
Work bounds: 200,000 entity slots, 256 registered types, 8 query terms, 1,000,000 retained structural events, 4,096 game and session journal entries, 512 inspection rows, 64 work reasons (256 bytes each, 8 reported), 64 derived types, 1,024 input events and messages (4,096 bytes/message), 65,536 inspection bytes/visits, depth 256, 1 MiB decoded strings, 128 MiB Sim save, 216,000 ticks/advance and 3,600 ticks/settle. External requests return explicit errors; trusted mutation APIs panic before exceeding capacity. Ownership validation/reaping is O(slots + edges); queries O(slots × terms), observation/hash O(slots × registered types + visited Data), journal consumption O(retained events), decode O(admitted bytes/values). External pending/failed readiness refuses settle immediately; future simulation work must declare deadlines.
Pages contain 64 slots: a 40-byte component first allocates 2,560 bytes versus the predecessor's 40,960 (16× smaller); only inserted values are initialized, with no page-wide zero fill. At 200,000 dense entities this means 3,125 pages rather than 196, trading more upload runs for small-world startup. Counting-allocator ceilings (allocation calls / cumulative requested bytes, including realloc requests): empty World **1 / 24**; 100-entity Sim construction **31 / 70,568**; its first tick **3 / 640**; exactly 10,240-byte restore with 32 entities, journal and binary resource **1,000 / 77,986**. Counts repeat exactly; activation and restore are measured separately from fixture creation.
Validation: 51 passing tests including the README doctest, plus 18 compile-refusal cases in the derive test; explicit 200k interleaved/churn and full-capacity negative controls; cross-engine hash/byte comparison; compile refusal fixtures and this doctest. Warm test time 1.287 s (`cargo test -p exact-world -p exact-world-derive`) and clippy 0.170 s (`cargo clippy -p exact-world -p exact-world-derive --all-targets -- -D warnings`). `cargo build -p exact-world --target wasm32-unknown-unknown` passes. Ryu's reachable nonspatial wasm probe (release z, fat LTO, one codegen unit, stripped; gzip level 9, mtime 0): std 226,787 / 83,475 bytes, Ryu 209,642 / 77,396 bytes. This is a formatter admission comparison, not the complete adapter artifact required for later system admission; the adapter is outside K1.
Environment verification: game workspace 734 passed, 18 GPU-adapter failures, 24 ignored; game clippy and formatting pass. Seven Linux proofs pass; Lanterns has zero failures but no pins, so is unverified. Bun reached 131 passing and 4 failing tests without Chrome, then was stopped after hanging. Caltrain Linux release builds; its smoke stops because view 20 is covered at the tap point. Its dependency graph excludes both world crates. Root build/test/clippy stop at missing lean Hermes TypeScript bakes; caps and boot pass. Metrics snapshot refuses the non-Git ibex dependency. Web/Apple/GPU execution and full adapter startup measurements remain unverified here.
Count every handwritten file, including this README and manifests; exclude only tests and cfg(test) modules. 6,500 total (6,403 production Rust); no generated files. Exact command, run from the repository root:
```sh
python3 - <<'PY'
import re
from pathlib import Path
total = 0
for p in sorted(Path('world').rglob('*')):
    if not p.is_file() or 'tests' in p.parts: continue
    s = p.read_text()
    if s.startswith('#![cfg(test)]'): continue
    s = re.sub(r'^#\[cfg\(test\)\]\nmod \w+ \{.*?^\}', '', s, flags=re.M|re.S)
    n = len(s.splitlines()); total += n
    print(f'{n:4} {p}')
print('TOTAL', total)
PY
```
Per-file counts below use paths relative to `world/`; all source files are below 1,500 lines.
| File / lines | File / lines | File / lines | File / lines | File / lines |
|---|---|---|---|---|
| `Cargo.toml` 16 | `README.md` 72 | `derive/Cargo.toml` 9 | `derive/src/lib.rs` 532 | `src/args.rs` 164 |
| `src/data/bin.rs` 462 | `src/data/hash.rs` 148 | `src/data/impls.rs` 295 | `src/data/json.rs` 181 | `src/data/limits.rs` 136 |
| `src/data/mod.rs` 212 | `src/data/text.rs` 15 | `src/input.rs` 253 | `src/lib.rs` 34 | `src/math.rs` 107 |
| `src/rng.rs` 103 | `src/sim.rs` 368 | `src/spring.rs` 133 | `src/storage/cell.rs` 134 | `src/storage/mod.rs` 206 |
| `src/storage/pages.rs` 70 | `src/storage/query.rs` 479 | `src/storage/raw.rs` 364 | `src/tween.rs` 111 | `src/values.rs` 322 |
| `src/world/inspect.rs` 225 | `src/world/journal.rs` 230 | `src/world/mod.rs` 862 | `src/world/ownership.rs` 109 | `src/world/save.rs` 148 |
