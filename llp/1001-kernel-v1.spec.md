# LLP 1001: Kernel v1 — what `exact-kernel` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Kernel, Wire, Layout, Text, Export
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1000, RFC 0491, LLP 0507, LLP 0486, LLP 0487, LLP 0488, LLP 0297, LLP 0382

## Summary

`kernel/` is RFC 0491's end state built fresh, scoped by `rules/NOT-DOING.md`: a
copied-in, borrow-parsed binary command stream with transactional apply, Taffy
layout, and columnar binary exports. Nine workstreams in exact1; here they are the
shape of one small crate. This document records the decisions the code embodies so
a reader can tell a choice from an accident. Where this document and the code
disagree, the code and its tests are the authority and this document is stale.

Under `rules/RULES.md` this spec is legitimate because the implementer and date
are on it and the code exists. It is not a promise about anything not yet built.

## 1. One declaration authority (WS-B)

`kernel/tables/schema.json` is the only place node types, props, style rows, enum
vocabularies, and opcodes are declared. `kernel/build.rs` generates from it — into
`OUT_DIR`, never committed — the Rust enums, `StyleProps` and its mask, the
patch/clear operations, the wire codec for style rows, and `SCHEMA_DIGEST`
(domain-separated SHA-256 of the canonical JSON, first 8 bytes, little-endian).
Every EXWF frame carries the digest; a producer generated from a different table is
refused at decode (`DecodeError::SchemaDigestMismatch`).

The generator fails closed: a row it cannot validate is a build error. A second
copy of any table anywhere is a defect — the TypeScript encoder, when it exists,
reads the same file.

**Defaults live in the table, once, and they are CSS's.** The style table's
`default` column is both the storage initializer and the semantic default; there is
no separate profile authority yet (0486's layout profile becomes relevant when a
second engine exists). Per `rules/RULES.md` §Scope the web is the standard: a bare
node is `display: block`, `box-sizing: content-box`, `flex-direction: row`,
`flex-shrink: 1`, `align-items: stretch` — what a bare `<div>` does — and rows
carry CSS names and vocabularies (`object_fit: fill|contain|cover|none|scale-down`,
`text_overflow: clip|ellipsis`, `line_clamp`). Block layout brings CSS margin
collapsing with it; flex and grid are opt-in per node. (An earlier draft chose
React Native's defaults; Charlie reversed that on 2026-08-28.)

**Motion rows (2026-08-28, LLP 1002/1003).** The animatable rows carry CSS's
individual transform property names — `translate` (vec2), `scale`, `rotate`
(degrees) — beside `opacity`, and a `transition` row (codec `transitions`, bit 82)
carries CSS `transition` declarations. The row's type is `exact_motion::Transitions`;
the kernel owns its bytes (`wire/codec.rs`) and depends on `exact-motion` for the
type, which is the only dependency edge between the two crates. `Kernel::motion_sync`
restates a commit for the engine (LLP 1003 §6).

Declared deviations, each because the engine cannot express the CSS value:
`position` has no `static` (Taffy positions an absolute child against its parent,
so `relative` without insets is the closest box; a web host emits `position:
relative` on every node to match); `text_align` defaults to `left`, not `start`,
because logical alignment is not yet lowered. A `ScrollView`/`List` scrolls on its
block axis unless the producer sets `overflow_y` — the only per-tag default,
applied in `StyleProps::to_taffy` (a scroll container is `overflow: auto` on the
web). An `Image` is a replaced element: the host reports its intrinsic size
(`Kernel::set_intrinsic_size`, the bitmap's pixel counts one-for-one as points,
after the image loads; before that each unknown axis measures 0, so a `width`
row still sizes the box), the node is measured from it, and it keeps its ratio
unless an `aspect_ratio` row is set — one dimension given, the other follows,
and min/max resolve by CSS 2.1 §10.4's table (Taffy patch 5). Declared: in
*block* flow Taffy stretches an auto-width image to its container where CSS
would use the intrinsic width (in a stretching flex column both stretch, by
ratio); the ratio still holds (`kernel/tests/image.rs`; LLP 1011).

## 2. The data model (WS-A)

- **Typed props.** `PropId` is a generated `#[repr(u16)]` enum; every prop declares
  a `PropKind` (`str | bool | int | float`). A boolean is a boolean on the wire and
  in storage; `"true"` is unrepresentable. A kind mismatch is a decode rejection
  (`PropKindMismatch`) or, for in-process ops, an apply rejection.
- **Columnar arena** (`arena.rs`). Nodes are slots; every attribute is a column;
  topology is index-based (`parents`, `children`); destroyed slots go on a free
  list. Frames and Taffy handles are *derived* columns — rehydration is
  columns-plus-rebuild (`LayoutTree::rebuild`, `Kernel::rehydrate`), never
  serialized engine state.
- **Identity.** A producer names nodes by a wire-local `ViewId` (u32, unique among
  live nodes in one kernel). Inside, a `NodeKey { index, generation }` names one
  allocation; generations start at 1 and bump on slot reuse, so a stale key never
  resolves (`arena::tests::keys_fail_closed_after_reuse`). Receipts and agent refs
  carry keys, never bare ids. `CreateView` on a live id of the same type is a
  no-op (0507 §5.4 `LiveNoop`); of another type, a rejection; after a destroy in
  the same batch, allocate-after-destroy with a fresh generation.

  *Deliberately not yet built:* the six-field raw address of 0507 §5.1
  (`ProducerId`, `ExecutionGeneration`, `rootId`, `rootIncarnation`, …). v1 has one
  producer per kernel and one incarnation counter (`Kernel::incarnation`, bumped by
  `reset`). The fields are added when a second producer or HMR exists to need them.

- **Selector index.** `testId` is a kernel citizen: an exact-value multimap,
  maintained on set/clear/destroy/reset, returned in structural tree order
  (`Kernel::find_by_test_id`).

## 3. One write path (WS-D, WS-F, 0507 §4)

Two ingress forms, one engine: EXWF byte frames (`Kernel::apply_frame`) and
in-process structured apply (`Kernel::apply(root_id, batch, &[Op])`). There is no
direct setter. Both enter `txn::apply`, which **validates the whole batch against a
staged view** (arena plus the batch's own earlier ops) **before writing anything**.
A rejection is a typed `ApplyError` and the arena, the layout engine, the selector
index, the epoch, and every receipt are exactly as they were —
`tests/apply.rs::every_rejection_class_leaves_the_kernel_untouched` proves it per
class by comparing the EXNODE export before and after.

The closed op list (revision 1): `CreateView`, `DestroyView` (**subtree** — the
WS-I correction, from day one), `SetProp`, `ClearProp`, `SetStyle` (a **masked
patch**), `ClearStyle` (a mask), `SetChildren`, `AttachRoot`. There is no
`ComputeLayout` op: layout is a host call (`Kernel::compute_layout`), because the
host owns the frame clock. A root with `width: auto` fills the width it is
offered — CSS's block rule, which Taffy does not apply to a root — and stays
as tall as its content: the page a viewport scrolls (added 2026-08-29 when the
Apple host's first bare-root fixture laid out 89 pt wide; a root's engine
style is re-derived on `AttachRoot`).

`SetChildren` rejects: duplicate children, self-child, a root as a child, a cycle
(child is an ancestor of the parent), children on a leaf type, and a non-`Text`
child under a `Text` (a text node's children are its inline runs; anything else
would be a node the engine never lays out — `InlineRunNotText`). A child listed
under a new parent is reparented; children dropped from a list become detached
(live, no parent, in no root's layout) — not destroyed. Every number in a style
patch must be finite on both ingress paths (`DecodeError::NonFinite`,
`ApplyError::NonFiniteStyle`); NaN never reaches a frame.

The apply phase assumes only what validation established. If that assumption ever
fails it stops and returns `ApplyError::Internal` — typed and loud, never a silent
skip and never a panic. That error names a kernel defect: earlier ops in the batch
were applied and no receipt was published, so a host `reset()`s and re-snapshots.
There is no `unwrap`/`expect` on the production path.

## 4. EXWF frame revision 1 (`wire/frame.rs`)

40-byte header: magic `EXWF`, revision u16, header_len u16, frame_len u32, root_id
u32, batch u64, schema digest u64, flags u32, reserved u32. 16-byte op header,
8-aligned: opcode u16, flags u16, view_id u32, payload_len u32, reserved u32; the
payload is zero-padded to 8. **There is no op count on the wire**; a consumer
iterates by validated lengths until `frame_len`. Reserved fields must be zero.
Every malformation names a `DecodeError`; a frame that fails anywhere applies
nothing (`tests/wire.rs::a_malformed_frame_applies_nothing`).

Value grammars: dimension = kind byte (0 auto, 1 points, 2 percent) + f32;
**percent is authored 0–100** on the wire and in storage and converted to Taffy's
fraction exactly once (`style.rs`); `auto` is admitted per row (`admitsAuto`) and is
a rejection elsewhere (`AutoNotAdmitted`); colors are `0xRRGGBBAA`; grid tracks are
a closed six-kind grammar (fr, points, percent, auto, min-content, max-content, ≤32
tracks); enum bytes outside their vocabulary are rejected, never defaulted.

## 5. Layout proportional to change (WS-H)

Per-node flags (`STYLE_DIRTY`, `TEXT_DIRTY`, `CHILDREN_DIRTY`, `PROPS_DIRTY`,
`PAINT_DIRTY`, `GEOMETRY_CHANGED`, `CREATED`), one published epoch
(`Kernel::epoch`, bumped only by a commit that changed something), and a
`CommitReceipt` per batch (`created`, `destroyed`, `touched`, `layout_invalidated`)
retained in a 64-deep ring. `compute_layout(root, offer)` runs Taffy over that
root, publishes absolute frames, and returns a `LayoutReceipt` naming exactly the
nodes whose frame bits changed — the changed-geometry receipt.

**The result-equality gate is a test, from the first commit.**
`tests/layout_equality.rs` mutates a random tree for hundreds of rounds and asserts
the incremental frames equal, bit for bit, both a kernel rehydrated from the
columns and a kernel that replayed every batch from scratch. This is RFC 0491's
Phase-5 exit conjunct (1), moved to day one where it costs nothing.

An engine fault is never a panic (`#![forbid(unsafe_code)]`, no `expect` on the
production path): `LayoutTree` records it, `compute_layout` reports
`LayoutError::Engine`, rebuilds the engine tree from the columns, and retries once.

## 6. Text (WS-E, WS-I)

Text measurement is a **per-kernel injected trait object** (`Box<dyn TextMeasurer>`),
never a process-global callback. The kernel hands the measurer a paragraph as
ordered `TextRun`s: a `Text` with its own `text` prop is one run; otherwise its
`Text` children are its runs, flattened in order — one structural traversal
(`NodeArena::text_runs`), the WS-I `text_fragments()` IR. Inline runs are measured
with their owning paragraph and have no geometry of their own; editing a run marks
the owner dirty (`measure_owner`). `TextInput` measures its `value` or
`placeholder`. `MonospaceMeasurer` is the deterministic reference measurer for
tests and headless hosts.

## 7. EXNODE export (WS-C)

One crossing per sync. `Kernel::rows` is the typed in-process projection; `Kernel::
export` is the binary envelope: 40-byte header (magic `EXNO`, version, total length,
root id, epoch, node count), a section directory with an FNV-1a-32 checksum per
section, and three 8-aligned sections — 32-byte node rows (id, parent, generation,
type, flags, depth, frame), per-node masked style patches, per-node typed props.
`export::decode` validates every length and checksum before adopting anything, and
never allocates from a count (sections, rows, children) it has not first bounded
by the bytes actually present — with the arithmetic in `u64`, so a 32-bit wasm
target cannot overflow on a hostile length either.

## 8. Errors

One convention (`error.rs`): `Result` with a typed error naming the exact
condition. `DecodeError` for the wire, `ApplyError` for rejected batches,
`LayoutError` for layout, `KernelError` as the union. No status integers.

## 9. Not in v1 (and where each is declared)

Selection, semantics/accessibility tree, islands, SVG rasterization, crash
capsules, a module registry, virtualized lists, portals, choice layout (0487's
operator — the corpus stays research until a producer needs it), the direction
truth table (RTL box layout), the 0507 raw-address namespace, EXWF extension
chunks, event frames, the wasm host interface. Each is either on
`rules/NOT-DOING.md` or waits for the consumer that would make its spec
transcription rather than speculation. (The C ABI found its consumer on
2026-08-29: the Apple host, LLP 1008 §4.)

## 10. Checks that hold this

`cargo test -p exact-kernel` (unit + `tests/{apply,wire,export,layout_equality}.rs`),
`cargo clippy -p exact-kernel --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo build -p exact-kernel --target wasm32-unknown-unknown`, `node scripts/caps.mjs`.
Every source file is under 1,500 lines; the largest is the generator.
