# Resident-byte subtraction experiment

**Do not adopt this version.** On `core/world-lean`, the requested subtraction
reduces the Data trait and passes the specified hostile shapes, but a derived
allocating default defeats the caller's budget. Mainline was not changed.

Baseline: `b00b96e`. The implementation is split into five commits: real-byte
charges and measured fixtures; decoder-unit removal; encode-claim removal;
trait/derive removal; tests, documentation and this report. EXGAME v4, EXSIM v10,
the frozen continuation inventory and existing pinned hashes are unchanged.

The concrete counterexample needs no manual Data implementation:

```rust
#[derive(Default, Data)]
struct Defaults {
    #[data(skip)]
    bytes: Box<[u8; 32]>,
}
```

A sequence of 100,000 empty records occupies 200,004 wire bytes. Loading it with
`LoadBudget::new(1_048_576)` succeeds, requests 7,200,064 cumulative bytes in
200,002 allocation calls, and peaks at **4,000,096 bytes**. The vector allocation
is charged; construction of each default box and its skipped-field reset are not.
`allocating_derived_defaults_expose_the_experiments_budget_gap` records this
counterexample, including `catch_unwind`; its success is evidence against adoption.
The same counterexample also executes on wasm32. Declaring allocating defaults
trusted does not meet the requested guarantee for ordinary derived Data.

## Size and API

Counts follow the README recipe, including comments and blank lines. The tests
column also counts inline test modules and the excluded allocator/test support.
The measurement report itself is outside that production recipe.

| Lines | Before | After | Change |
|---|---:|---:|---:|
| Kernel production Rust | 6,246 | 6,257 | +11 |
| Derive production Rust | 544 | 474 | −70 |
| All production Rust | 6,790 | 6,731 | −59 |
| README | 512 | 414 | −98 |
| Tests and test support | 6,079 | 6,175 | +96 |
| Manifests | 23 | 23 | 0 |
| Production recipe total | 7,325 | 7,168 | −157 |

`Data` previously exposed `CHECK_DEFAULT_ACYCLIC`, `inline_size`, `default_size`,
`read_new`, `write`, and `read`. It now has these three signatures:

```rust
fn read_new(r: &mut dyn Reader) -> Result<Self, DataError>;
fn write(&self, w: &mut dyn Writer);
fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError>;
```

`Writer::claim_decoded` is removed. `Reader::claim`, `Reader::check_allocation`,
`LoadBudget`, `MAX_LOAD_BYTES`, and public World/Sim signatures are retained.
Skipped fields now need Default rather than Data admission metadata. The adapter's
old claim method becomes a private output-limit helper; its sixfold text-escaping
allowance remains local to Contract conversion. No motion-module API changes.

## Restore and allocation measurements

Both versions used Rust 1.97.0, the same machine and target cache, Cargo's release
profile (opt-level 3, thin LTO, one codegen unit), one warmup and 20 restores.
The median is the average of sorted samples 10 and 11. The generated fixture has
200,000 entities, reverse/interleaved insertion of three component types, a
controller column and pending assets. Every measured restore verifies its save.
These timings are diagnostics on a shared machine, not a significance claim.

| Sim::restore input | Before median | After median |
|---|---:|---:|
| Tally's 3.6 KB save | Unavailable | Unavailable |
| Generated 4,866,081-byte save | 94.666432 ms | 100.822390 ms (+6.50%) |

Tally and its pins/save are absent from this checkout and from the supplied base;
no substitute was labeled Tally and no other lane was consulted.

| Operation: allocator calls / cumulative requested bytes | Before | After |
|---|---:|---:|
| Empty World | 1 / 24 | 1 / 24 |
| 100 entities | 21 / 41,880 | 21 / 41,792 |
| First publishing tick | 2 / 568 | 2 / 568 |
| 1,000 changing-publication ticks | 3 / 200,704 | 3 / 200,704 |
| 10 KiB from-save and existing-Sim restore | 80 / 46,760 | 80 / 46,672 |
| 10 KiB carry | 84 / 46,766 | 84 / 46,678 |
| First component at slot 199,999 | 4 / 77,536 | 4 / 77,536 |
| High-slot churn, sparse ticks, prepared input-heavy ticks | 0 / 0 | 0 / 0 |

The existing query/run implementation and benchmark were untouched. No new query
speed claim is made. Reproduce timing with `cargo test --release -p exact-world
--test restore -- --ignored --nocapture`; the baseline is an unmodified copy of
`b00b96e` with the same benchmark source and workspace release profile.

## Hostile-input measurements

The existing allocator now records net peak requested bytes as well as cumulative
requests. Fixtures retain their pre-existing owners until decoding finishes, so
outgoing-state deallocations cannot hide a candidate's peak. The tests assert
**both** measures ≤ budget + **8,192 bytes**, reserved for bounded diagnostics.
That allowance is independent of input length; allocator metadata/RSS and stack
memory are not requested heap bytes.

| Hostile shape | Peak at 1 MiB | Peak at 256 MiB | Result |
|---|---:|---:|---|
| Huge sequence count, tiny input | 112 | 112 | Err |
| Huge string length | 58 | 58 | Err |
| Wide enum / aligned struct vectors | 104 | 104 | Err before backing allocation |
| Nested publications, worst of depths 8/40/81 | 1,025,520 | 5,188,588 | Shallow control succeeds; larger/deeper cases refuse |
| 20,000 map keys | 107,314 | 3,741,120 | Err / success |
| Sparse 1 KiB components across eight types | 1,128 | 268,278,448 | Err |
| Sparse 4096-aligned component chunks | 1,128 | 268,336,976 | Err |
| 200,000 entity slots | 744 | 10,620,528 | Err / success |

The sparse fixture forges one present value per 64-slot chunk without constructing
its multi-gigabyte decoded shape. Error cases retain the destination; valid string,
vector, map, component and publication controls assert actual returned content.
All these cases run inside `catch_unwind`. A scratch mutant removing vector-growth
claims fails the wide-vector test at **307,500,128 peak bytes** with a 1 MiB budget.
This negative control confirms that the new tests detect removal of byte charging.

CPU still has input and structural bounds: input ≤ 256 MiB (Sim 128 MiB), nesting
≤ 256, strings ≤ 1 MiB, entity slots ≤ 200,000, types ≤ 256, publication nodes
≤ 65,536 and publication string bytes ≤ 65,536. Counts exceeding remaining input
refuse before reservation. Work is bounded by these caps and map/key comparisons;
the resident budget alone does not meter allocation-free traversal. Ordinary tick
and ownership work limits and their explicit refusals remain unchanged.

## Test inventory

Every removed or renamed test from the baseline is listed below. The three
allocating-default guards were substantive safety tests, not unit-arithmetic noise.

| Removed name | Reason / replacement |
|---|---|
| `collection_and_record_admission_uses_fixed_wire_units` | Only asserts deleted trait sizes. |
| `portable_admission_boundary_matches_32_and_64_bit_readers` | The 2,232-unit architecture-invariant threshold no longer describes the contract. |
| `oversized_admission_refuses_without_overflow_on_any_pointer_width` | Artificial default-size declarations and their registration refusal were deleted. |
| `bulk_hash_and_encoder_share_the_decoded_allowance_boundary` | Hashing and encoding no longer claim decode allocation. |
| `encoder_refuses_small_wire_values_with_excessive_decoded_backing` | Replaced by `saving_does_not_preprove_loadability`. |
| `unit_default_enum_charges_its_largest_inline_variant_before_allocation` | Renamed `wide_enum_vector_charges_its_resident_layout_before_allocation`; real-layout refusal retained. |
| `nested_reservation_chain_cannot_spend_outstanding_sibling_allowances` | Reservation units were removed; measured nesting and independent node/text caps replace it. |
| `ownership_scratch_does_not_consume_the_state_decode_allowance` | Replaced by `ownership_scratch_and_validation_share_the_resident_budget`; scratch now belongs to that budget. |
| `nested_box_default_is_preflighted_before_allocating` | This protection was lost; the measured default counterexample records the consequence. |
| `omitted_box_fields_and_container_resets_claim_defaults_before_allocation` | This protection was lost; the measured default counterexample records the consequence. |
| `skipped_defaults_and_manual_enum_defaults_cannot_escape_decode_admission` | Default charging was lost; selected-enum construction still has its own retained control. |
| `writer_accounts_for_skipped_default_resets_before_returning_unreadable_bytes` | Saving no longer pre-proves decode/default costs. |

The derive test also drops four compile-refusal cases tied to
`CHECK_DEFAULT_ACYCLIC`; the rest of its syntax, borrowing and map-key controls stay.
The i686/wasm portable-unit fixture is deleted; meaningful aligned-storage and
frozen-wire checks remain. No frozen bytes, expected hash or game pin was edited.

New tests (including the three replacements above):

- `huge_count_tiny_input_and_huge_string_length`
- `wide_enum_and_padded_struct_vectors`
- `many_map_keys`
- `near_cap_entity_table`
- `sparse_chunks_across_types_charge_real_layouts`
- `nested_publication_peak_is_bounded`
- `publication_nodes_and_text_have_independent_shared_caps`
- `allocating_derived_defaults_expose_the_experiments_budget_gap`
- `selected_enum_reader_avoids_unrelated_manual_default`
- `wide_enum_vector_charges_its_resident_layout_before_allocation`
- `ownership_scratch_and_validation_share_the_resident_budget`
- `saving_does_not_preprove_loadability`
- `restore_median_20` (ignored diagnostic benchmark)

## Accounting that carries real weight

Actual buffer capacity, sparse chunk Layout sizes, directory replacements, boxes,
strings, publication backing, map nodes and journal Vec backing cannot disappear.
Both numeric conversion buffers coexist. Names, duplicate-field marks, registry
copies, name/owner indices, ownership scratch and canonical re-encoding also
allocate during a load, so their costs share the budget. `bin::read_into` includes
its initial encoded staging copy. Growth is cumulatively charged in full, a
conservative bound rather than a refund system; map inserts use a conservative
estimate of two full standard-library B-tree nodes per entry.

Box/chunk and vector/string allocations use fallible allocation and checked
layouts. Stable standard-library Rc and BTreeMap insertion still allocate through
their infallible APIs after budget preflight; this is not protection against system
OOM. Unsafe implementation code remains confined to storage.

Most importantly, default-construction accounting cannot be safely deleted while
retaining the existing derived decode/reset behavior. This branch deletes it to
measure the proposed experiment and consequently fails its required guarantee.
Restoring that protection needs an accounted construction mechanism or a different
default/decode contract; hiding that cost in a new declaration was outside the brief.

## Verification and limitations

- Kernel/derive: **165 passed**, three ignored; both large controls passed separately,
  as did the 20-sample restore benchmark. Clippy with `-D warnings` and formatting pass.
- Miri: six x86-64 storage, six i686 storage, eleven journal, five bulk, six decoder,
  one recursive-box and one atomic `read_into` execution passed: **36 total**. The
  unrelated random-float formatter sweep was stopped and excluded from the selected
  decoder rerun; no touched decoder case was skipped for failure.
- Production wasm32 build passes. A wasm32 cdylib built from the same sources runs
  all eight experiment cases and the five-boundary frozen continuation inventory
  through Bun: **nine exported checks pass**, including reproduction of the gap.
  Native `catch_unwind` checks are also compiled there; a wasm abort is a host trap.
- Game workspace: **749 passed, 25 ignored, 18 missing-GPU failures**. All six
  world-adapter and nine world-motion tests pass. Game workspace clippy and fmt pass.
- Bun: **146 passed, one skipped, three missing-Chrome failures**.
- Linux proofs: six pass (Beacons, Greybox, particles, placement, skinned, sprites).
  Both Lanterns fixtures complete with zero assertions failing but report missing
  pins. Asset-fixture refuses its pre-existing invalid generated-output manifest;
  other games and generated asset policies were outside this task's edit scope.
- Root build/test/clippy are blocked by the absent lean Hermes producer. Root fmt,
  caps and boot pass; boot remains 88,699 JS bytes, 3,468 page bytes, two modules
  and one Wasm reference. No GPU, Chrome or Apple rendering was verified.
- Tally compile, pins and restore timing are unavailable because Tally is absent.

Logs, baseline source, the mutation control and the wasm harness are under
`~/lanes/gamenext/scratch/KL/`; all Cargo builds share this clone's target cache,
with no separate scratch target trees. The wasm harness changes only test entry
attributes/exports in its scratch copy so Bun can call the same test bodies.

## Verdict

The public Data trait is simpler, but production kernel Rust grew by eleven lines.
The lean version is not at least as safe: ordinary derived defaults exceed the
resident budget, and the measured large restore is slower.
Do not adopt this experiment without recovering default-construction accounting.
