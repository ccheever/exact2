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

`scrollLeft` (prop 59, float) is the horizontal counterpart of `scrollTop`: a
changed binding sets that axis after layout, clamped by the host’s content
extent; unrelated updates preserve the reader’s offset.

`autocapitalize` and `autocorrect` (props 57/58, text) carry HTML editing
hints. They remain strings so HTML's case-insensitive aliases and missing,
empty and invalid-value behavior stay the host's. The browser receives real
attributes; native editors apply them without transforming stored text.

Explicit application policies (Messages, 2026-09-09) have no CSS
equivalent: `scrollFollowEnd` (boolean, absent/false by default) keeps a
scroll container at its trailing vertical edge across content/viewport changes
only while the reader is already there. Above the end, web leaves CSS scroll
anchoring to the browser; iOS records a visible descendant and adjusts the offset
by its movement after layout. It keeps that choice between batches until the
reader scrolls; if the node disappears, a surviving visible candidate can hold
the position. With none left, iOS clamps the old offset to the new extent.
An inactive iOS route retains an unpinned offset across a temporary viewport
clamp, restoring it when space permits; a new drag or explicit scroll write wins.
macOS still preserves the numeric offset in all cases. An explicit `scrollTop` assignment wins. Web uses
ResizeObserver and commit boundaries; Apple snapshots before each batch and
restores after layout. The opt-in policy is not a complete native implementation
of CSS `overflow-anchor` selection and suppression rules.
`retainFocus` (prop 55, boolean, absent/false by default) lets a button
retain the existing editing session on a tap, including a button with gesture
handlers and no press action. Messages uses it on bubbles and reaction badges
that open Tapbacks: the panel does not exist yet at pointer down. Web prevents
the pointer’s default focus change; iOS skips resigning the current responder.
It neither focuses a field nor opens a keyboard. Other hosts currently ignore it.
`swipeIndicator` (prop 56, boolean, absent/false by default) marks a direct
child of a `swiperight` target as authored gesture feedback. Web and iOS hold
its opacity and scale between their authored values and 1 as the rightward
offset grows from 0 to the 64-point action threshold, clamped thereafter.
Reversal follows the offset; release/cancellation restores the authored values
through their transitions. Its subtree does not take pointer hits, including at
zero opacity. It has no CSS equivalent and creates no host artwork
or runner gesture state. Other hosts ignore it.
`keyboardDismissMode` (`none`, `on-drag`, `interactive`; absent means `none`)
opts an iOS scroll container into UIKit's keyboard dismissal behavior. Other
hosts retain their platform behavior. Linux does not yet implement either policy.

`scrollbar-width` (bit 88; Messages, 2026-09-09) follows
[CSS Scrollbars](https://www.w3.org/TR/css-scrollbars-1/#scrollbar-width):
`auto` initially, `thin`, or `none`, not inherited. It changes scrollbar
presentation, not overflow, scroll position, snap, or layout. Web emits the CSS
property. iOS uses its already thin native indicators for `auto`/`thin` and
hides both for `none`; macOS uses regular/small scrollers and hides both for
`none`. Clearing the row restores platform defaults. Linux has no scrollbar
painting yet and ignores this row. Messages hides the horizontal timestamp track.

`clip-path` (bit 87) clips painting and platform hit-testing without changing
layout. Its initial value is `none`. The implemented CSS subset is
`path("...")` with explicit absolute `M`, `L`, `Q`, `C`, and `Z` commands,
whitespace/comma-separated finite coordinates in CSS pixels, and nonzero fill.
Other shapes, fill-rule arguments, relative/implicit commands, and adjoining
signed coordinates are rejected. `clip.rs` validates and canonicalizes the value;
the wire carries that CSS string and validates it on decode. Apple receives the
parsed commands, applies a layer mask to the entire subtree, and tests the same
path for pointer hits. Web emits CSS. Linux painting has not implemented it.
Messages uses it for transparent curved tails over the focused reply material.

`navigationKey` and `navigationBack` declare a host navigation container on the
first root. Its direct children carry unique `navigationKey` values; the root's
key selects one, with preceding children retained as its back stack.
`navigationBack` names the HTML `id` of a press control in the active route.
UIKit presents those existing Contract views through `UINavigationController`
and presses that control after a completed native pop; cancellation changes no
Contract state. The browser hides and makes inactive routes inert. This is an
explicit platform navigation policy, not a CSS style or an engine-owned gesture
or interactive transition model. Other hosts currently retain their ordinary
stacked rendering; callers supply opaque, absolutely positioned route surfaces.

`swipeContent` (65), `swipeLeading` (66) and `swipeTrailing` (67) are
explicit native row presentation requests (2026-09-10, Messages). The first
names a descendant's HTML `id`; the others are whitespace-separated control
ids, ordered from the outer action inward. `swipeDestructive` (68, boolean,
absent/false) marks a control's destructive role in that presentation. These
props do not change CSS layout, inheritance or scroll semantics. UIKit supplies
the row's swipe surface (LLP 1008 §9); other hosts keep the authored content and
controls. A full swipe performs the first configured action when it is enabled.
Missing/ambiguous references or invalid row geometry leave the authored fallback
in place and produce a host diagnostic. All references must be within the owning
scroll node, and its content must have that node's width and height.

`emojiPicker` (prop 63, boolean, absent/false by default) is an explicit
selection-input policy on `input`, not an HTML `inputmode` value. A single emoji
grapheme emits `change`; ordinary text and multiple graphemes are refused and
the field is cleared. iOS prefers an enabled emoji keyboard, including its
native search; web and macOS filter entered characters without opening a system
picker. Linux's driver reports unsupported before changing focus or state.
The app keeps `value=""`; its draft belongs to a separate editor. The selection
gate accepts emoji-presentation scalars, or emoji scalars accompanied by VS16
or the keycap mark, within one grapheme. This is a bounded input filter, not a
complete Unicode emoji-sequence validator. It is not inherited.

`contextTarget` on a context-preview node names its source's HTML `id`.
The preview's nearest enclosing absolutely positioned panel aligns that preview
with the source's visible position, clamped inside the viewport and safe areas.
Web and Apple also intersect that vertical range with the panel's containing
region, allowing authored content to reserve space above or below the menu.
The region's bounds do not change the preview's source or text measurement.
The app supplies the preview's matching content and the surrounding controls;
the host supplies placement over scrolled content. This is a context-preview
presentation policy, not CSS anchor positioning. Web and iOS also magnify the
preview by 15%, capped at 26 added points of width, preserving its text layout,
source-facing outside edge and vertical center before viewport clamping. Later
content counteracts the panel's half-height shift so a receipt keeps its
source-relative position. Top-aligned immediate side siblings translate to the
enlarged preview's left or right edge, preserving their authored horizontal gap
without changing the preview's percentage-width basis. Clamping includes the farther extent of the enlarged
preview or following controls; the kernel's layout and authored transform rows
do not change. This follows measured iPhone
17 / iOS 26.5 preview geometry, not a recovered UIKit rounding/animation policy.
`contextMagnify=false` on that preview disables only this host magnification;
alignment, containment and the authored CSS transform still apply. Absent or
true keeps the 15%/26-point rule. The prop is a non-inherited boolean and has
no effect without `contextTarget`. Messages uses false for badge/double-tap
entry and true for long-press entry, including their respective emoji pickers.
macOS implements alignment without magnification; Linux currently retains the
panel's authored position. Messages supplies its outside-dismiss backdrop as an
ordinary press control.

Declared deviations, each because the engine cannot express the CSS value:
`position` has no `static` (Taffy positions an absolute child against its parent,
so `relative` without insets is the closest box; a web host emits `position:
relative` on every node to match); `text_align` defaults to `left`, not `start`,
because logical alignment is not yet lowered. Font matching stops at the nearest
real declared face and never synthesizes weight or style, rather than CSS's initial
`font-synthesis: weight style small-caps`; the compiler diagnoses a literal
weight/style whose declared family lacks the needed face, and the web host emits
`font-synthesis: none` (LLP 1019 §5). A `ScrollView`/`List` scrolls on its
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
- **Spelling-check hint** (2026-09-09, Messages). HTML's enumerated `spellcheck`
  is a string prop, preserving the authored spelling. `NodeRef::spellcheck()`
  returns the nearest explicit hint through logical ancestors: ASCII-case-insensitive
  `true`/`false`, empty true, invalid/missing values inherited without trimming.
  `None` leaves the editor's default in charge. The getter does not modify props;
  native hosts project its result to editors and the web emits the authored attribute.
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
- **The environment** (2026-08-30). A dimension row takes a fourth kind beside
  `auto`, points, and percent: an `env()` length — CSS's
  `env(safe-area-inset-<edge>)` and `calc(env(safe-area-inset-<edge>) ± <n>px)`,
  parsed once in `style.rs` (`Dimension::parse_env`; text on a dimension row is
  that or a rejection; wire kinds 3–6, one per edge, the `f32` the added points;
  no fallback argument, since the host always defines the four). The kernel
  holds one `Env` — the four insets in points, the host's, set with the viewport
  (`Kernel::set_env`; a `reset` keeps it, a rehydration carries it) — and
  resolves every `env()` length against it where the engine style is derived
  (`taffy_style`); `set_env` re-derives and dirties exactly the nodes whose style
  reads an inset (`uses_env`) and says whether any did, so a host lays out only
  when something can move. Zero until the host says otherwise, as a browser
  reports the insets for a page without `viewport-fit=cover` (LLP 1008 §9).
  `tests/env.rs`.

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

Frames retain fractional CSS pixel geometry (Messages, 2026-09-09). Taffy's
whole-point rounding is disabled when constructing the layout tree, including
rebuilds and rehydration. A half-point height edit must move the following row
by half a point, not by a whole point or zero; nested fractional offsets and
intrinsic image ratios must survive publication too. Kernel regressions and a
browser/iPhone fixture cover those cases. Rasterization belongs to the host;
this does not remove rounding inside an injected text measurer or promise
identical floating-point quantization in every browser engine.

**The result-equality gate is a test, from the first commit.**
`tests/layout_equality.rs` mutates a random tree for hundreds of rounds and asserts
the incremental frames equal, bit for bit, both a kernel rehydrated from the
columns and a kernel that replayed every batch from scratch. This is RFC 0491's
Phase-5 exit conjunct (1), moved to day one where it costs nothing.

An engine fault is never a panic (`#![forbid(unsafe_code)]`, no `expect` on the
production path): `LayoutTree` records it, `compute_layout` reports
`LayoutError::Engine`, rebuilds the engine tree from the columns, and retries once.

## 6. Text (WS-E, WS-I)

**Inheritance** (LLP 1035.000 D1–D4, landed 2026-09-09). The schema marks the
rows CSS inherits with `inherited: true` — `text_color`, `font_family`,
`font_size`, `font_weight`, `font_style`, `line_height`, `letter_spacing`,
`font_variant_numeric`, `direction`, `white_space`, `text_align` — and the
generator emits `StyleMask::INHERITED` and `StyleId::inherited()`. One
mechanism serves them all: `NodeRef::computed(id)` is the own row, else the
nearest logical ancestor's for an inherited row, else the initial value;
`source_of(id)` names the node that supplied it; `computed_style(rows)`
resolves a set of rows in one walk; `text_color()` and `text_style()` are
instances. Authored presence stays in the own mask — a computed value is
never written back. A run measures with its computed style (`arena.text_runs`),
so a `text` child without a `font_size` takes its paragraph's, as a `<span>` in
a `<div>`; a paragraph's `direction` and `text_align` inherit into its
measurement too. Invalidation is the kernel's: a write to an inherited row
marks and touches every logical descendant that does not set the row itself
(text rows remeasure its paragraph, the rest repaint), stopping under an
override; a child moved between parents propagates only the rows whose
computed value differs, and an orphan re-attached is re-derived in full. The
receipt therefore names what an inherited change reached; no host re-derives
descendants per frame. A light/dark pair is preserved for the host to resolve.
`kernel/tests/apply.rs` holds colour (reparenting, cleared overrides) and the
text rows (a bare, a bold and a small run; the touched set after an ancestor
change, a move and a clear; an identical write touching nothing).

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

### Platform background materials

`backgroundMaterial` (prop 54) requests a system material, not a CSS blur
radius. `ultra-thin` is consumed by the Messages focused reply thread. UIKit uses
`UIVisualEffectView` with `.systemUltraThinMaterial`, including the platform's
appearance and accessibility adaptation; web approximates the material with
`backdrop-filter: blur(20px) saturate(180%)` and an appearance-aware translucent
fill. `glass` uses `UIGlassEffect(style: .regular)` on iOS 26, falling back
to ultra-thin blur on earlier iOS. Its corner configuration follows the node’s
uniform border radius. Web uses translucent fill, blur, and a light shadow.
Messages uses glass for its composer, header controls, and inbox search. Authored
children go in the effect’s `contentView`; enabled nodes with a press handler use
`UIGlassEffect.isInteractive`. Changing or clearing the material preserves those
children. Glass grouping is not implemented. Other hosts currently leave materials
transparent. This explicit
host policy does not alter the existing CSS `backdrop-blur` style row or claim
pixel parity between a UIKit material and a CSS filter.

### Touch panning directions

`touch-action` (style bit 86, initial `auto`) admits the keyword combinations
listed in `schema.json`. Web emits the CSS declaration unchanged. UIKit tests
the initial pan direction against the hit node's and ancestors' declarations,
through the scroll container, leaving permitted scrolling to `UIScrollView`.
The directional names describe scrolling: a leftward finger movement scrolls
right, so Messages uses `pan-right pan-y` on a replyable bubble. This mapping
was driven against Chrome touch input. Changing the row after recognition does
not change that gesture. Native pinch zoom is not added by this row.
