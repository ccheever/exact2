# LLP 0095: Text Geometry

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Text, Tooling, Runtime, Layout
**Author:** Charlie Cheever / GPT-5 Codex
**Date:** 2026-03-28
**Revised:** 2026-07-12 (r7: ENG-24486 authority split; ENG-24416 contract reconciliation)
**Related:** RFC 0017, RFC 0026, RFC 0084, RFC 0088, RFC 0096, RFC 0135,
LLP 0160, LLP 0172, LLP 0223, LLP 0297, LLP 0323, LLP 0349,
`docs/fonts-and-assets.md`, `docs/css-dom-restrictions.md`,
`docs/app-runtime-and-dev-server.md`,
`docs/plans/text-geometry-web-first-execution-plan.md`

## Summary

Exact needs a first-class **Text Geometry** subsystem rather than treating text
as an opaque leaf that answers only “what is your width and height at this
width?”

The subsystem is built around prepared paragraphs and reusable geometry
queries:

1. **Prepare once** — parse and shape one canonical paragraph input.
2. **Query many times** — ask for measurement, lines, ranges, hit tests, and
   width search without rebuilding content identity.
3. **Share one paragraph model** across measurement, rendering, selection,
   accessibility, and custom text surfaces.

This RFC owns paragraph identity, inputs, capabilities, query semantics,
accuracy, caching, and platform placement. It does not define a custom layout
ABI. **LLP 0349 owns Programmable Layout** and consumes this service; RFC 0096
owns text composition and explicit custom text surfaces. The dependency is
one-way: Text Geometry is useful and reviewable without either downstream
layer.

## Implementation status (verified on `origin/main`, 2026-07-12)

| Area | State | Evidence |
| --- | --- | --- |
| `@exact/text` package with `ParagraphSpec`, `ParagraphSpan`, `ParagraphRangeAnnotation` (kinds, decoration, interaction, accessibility) as specified | **shipped** | `packages/exact-text/src/paragraph-spec.ts` |
| Geometry query surface: `measure`, `minContentWidth`, `maxContentWidth`, `layoutLines`, `nextLine`, `rangeRects`, `rangeAnnotationsAt`, `hitTest`, plus `layoutWithLineWidths` and width-search | **api-present (web backend behind it; several spec fields unsupported but declared)** — no `lineMetrics` method (subsumed by `layoutLines`; `ParagraphLine` carries width/height/baseline/origin, **no ascent/descent**, and the web baseline is a style heuristic); Apple `layoutWithLineWidths` is still single-width rather than per-line variable width; ENG-24416's capability descriptors now expose and fail closed on unsupported fields and attachment shaping | `packages/exact-text/src/geometry-api.ts`, `native/apple-backend.ts`, `web/pretext-backend.ts` |
| Web backend consumes Pretext directly (`@chenglou/pretext`) — answers OQ 7 in practice | **shipped** | `packages/exact-text/src/web/pretext-backend.ts:9` |
| Apple geometry backend: JS-side prepared object + cache over **stateless host measure/layout calls** — not true native prepared handles | **partial** | `packages/exact-text/src/native/apple-backend.ts:612-740`; LLP 0323:297-300 records the distinction |
| Paragraph lowering from nested `Text` | **partial — lowerer exists, NOT yet the shared paragraph feed**: the renderer keeps the `ParagraphSpec` on the runtime-side node only; the protocol encoder still sends flattened text + scalar props, the kernel reconstructs its own measurement runs, and the Apple presenter rebuilds attributed strings from presenter children. The one-paragraph objective's core (one spec feeding measurement, rendering, selection, geometry) remains open | `packages/exact-renderer/src/text/paragraph-lowering.ts`, `host-ops.ts:1702+`, `protocol/encoder.ts:937+`, `kernel/src/lib.rs:337+`, `ExactNodeRenderSupport.swift:427+` |
| React consumption surface | **shipped** | `packages/exact-react/src/text-geometry.tsx` |
| Range rects / hit testing accuracy | **approximate** — proportional interpolation across the line, both backends (see §Position units and geometry accuracy) | `native/apple-backend.ts:479-528`, `web/pretext-backend.ts:978-1040` |
| Geometry accuracy, UTF-16 unit labels, API stability, and backend capability descriptors | **shipped (incubator contract)** — ENG-24416 added `TEXT_GEOMETRY_API_STABILITY = "incubator"`, branded/labeled UTF-16 offsets, `offsetUnit`, `geometryAccuracy`, field/query capability descriptors, result metadata, and `assertExactClusterGeometry`; both shipped backends honestly declare `approximate`, reject unsupported geometry-affecting inputs, and reject attachment shaping before offsets can drift | `packages/exact-text/src/geometry-api.ts`, `text-position.ts`, backend contract tests |
| Default-`Text` measurement substrate (kernel path) | **superseded here; owned by LLP 0323** — kernel-owned, host-fed segment measurement cache, currently **dev-flag opt-in** (`EXACT_TEXT_SEGMENT_CACHE=1`; no default-on before LLP 0323's per-platform gates — `ExactKernel.swift:384-390`); this RFC's `TextLayoutRef` host-service integration was not built (see §Kernel Integration) | `kernel/src/text_segments.rs`; LLP 0323 §6 |
| Selection consuming prepared-paragraph geometry (this RFC's convergence exit criterion) | **not yet — decision recorded** — shipped selection paints through native text-view hosts; convergence deferred to the LLP 0172 static-text milestone (see §Range Annotations) | RFC 0017 §Highlight paths |
| Threading exception for sync geometry host-calls | **shipped** — allowlisted sync-by-design | `exact-verify.json` `sync-host-call-allowlist` (`apple-backend.ts`, `exact-react/src/text-geometry.tsx`) |

## Motivation

Text is currently split across several partially connected paths:

- the kernel consumes text mainly through measurement;
- native renderers build attributed strings separately;
- selection maintains its own document model and host projections;
- web uses real DOM/CSS for ordinary text;
- the explicit `@exact/text` geometry layer prepares its own paragraphs.

That split is acceptable for ordinary text only while geometry remains a
scalar callback. Rich queries expose four structural problems:

1. The default kernel/host contract cannot answer line, range, or hit-test
   geometry.
2. Measurement, rendering, selection, and accessibility can drift when they
   resolve different paragraph inputs.
3. Apple and LLP 0323 have useful local caches, but there is no shared
   cross-consumer paragraph identity.
4. Approximate geometry can look plausible while being wrong for proportional
   fonts, ligatures, bidi, truncation, or attachments.

The goal is not one universal text engine. The goal is a shared, honest
paragraph contract implemented by platform text engines and safe for explicit
consumers. Layout primitives, custom surfaces, and agent diagnostics then build
on that contract through their own authorities.

## Goals

- Make prepared paragraph geometry a first-class Exact subsystem.
- Define one canonical `ParagraphSpec` and UTF-16 offset domain.
- Support reusable width-independent paragraph identity with cheap relayout.
- Expose geometry richer than scalar measurement.
- Make backend support and geometry accuracy inspectable and fail-closed.
- Keep default `Text` semantic on every platform.
- Converge measurement, rendering, selection, accessibility, and hit testing on
  one paragraph input without forcing one painter or shaper across platforms.
- Provide a stable substrate for LLP 0349 and RFC 0096 without absorbing their
  layout or rendering responsibilities.

## Non-Goals

- Defining custom layout containers, layout worklets, child-frame ownership, or
  layout-cycle policy (LLP 0349).
- Defining region composition or custom text painters (RFC 0096).
- Pixel-identical text across platforms.
- Replacing native text engines with one Exact-owned shaper.
- Running ordinary web text through Taffy or a custom text renderer.
- Turning Taffy into a general relational layout solver.
- Supporting arbitrary inline `View` layout beyond the attributed-text model.
- Freezing the low-level API before the incubator graduation gates are met.

## Design Principles

### 1. Ordinary text stays ordinary

DOM/CSS on web and native attributed text on native remain the default.
Geometry queries are explicit and additive.

### 2. One paragraph input serves every consumer

Measurement, rendering, selection, accessibility, and geometry must not resolve
different text, font, direction, or attachment streams.

### 3. Implementations are platform-specific; the contract is shared

Each host uses its real text engine and declares support and accuracy honestly.
The shared API defines meanings, units, and failure behavior, not identical
glyph placement.

### 4. Width changes are the hot path

Preparation may be expensive. Reflow and repeated queries at new widths should
reuse width-independent paragraph state.

### 5. Approximation never masquerades as interaction fidelity

Approximate geometry is useful for estimation and experiments. Selection,
carets, links, and accessibility geometry require `exact-clusters` and fail
closed otherwise.

### 6. Downstream layers keep separate authorities

LLP 0349 owns programmable layout and RFC 0096 owns composition/painters. This
RFC exposes caller-neutral queries and does not assume either consumer exists.

## Proposed Architecture

Exact has three neighboring responsibilities:

1. **Box layout** — Taffy-backed flex/grid/positioning for ordinary view trees.
2. **Text Geometry (this RFC)** — prepared paragraphs, capabilities, line and
   range queries, hit testing, and width search.
3. **Render surfaces** — semantic DOM, native attributed text, and explicit
   custom surfaces owned by RFC 0096.

LLP 0349 may use Text Geometry to place children, but it cannot change this
service's units, accuracy semantics, or ownership. Text Geometry is not a Taffy
replacement; Taffy is not a paragraph engine.

## The Core Abstraction: Prepared Paragraphs

The central runtime object is a prepared paragraph handle:

```ts
type ParagraphHandle = opaque;
```

A paragraph is prepared from an attributed paragraph spec:

```ts
type ParagraphSpec = {
  spans: ParagraphSpan[];
  ranges?: ParagraphRangeAnnotation[];
  locale?: string;
  baseDirection?: 'ltr' | 'rtl' | 'auto';
  writingMode?: 'horizontal-tb' | 'vertical-rl' | 'vertical-lr';
  lineBreak?: 'normal' | 'strict' | 'loose';
  overflowWrap?: 'normal' | 'break-word' | 'anywhere';
  maxLines?: number;
  truncation?: 'none' | 'head' | 'middle' | 'tail' | 'clip';
};

type ParagraphSpan =
  | {
      type: 'text';
      text: string;
      style: TextStyle;
      metadata?: Readonly<Record<string, unknown>>;
    }
  | {
      type: 'attachment';
      id: string;
      width: number;
      ascent: number;
      descent: number;
      altText?: string;
      metadata?: Readonly<Record<string, unknown>>;
    };

type ParagraphRangeAnnotation = {
  id?: string;
  range: { start: number; end: number };
  kind: ParagraphRangeKind;
  decoration?: ParagraphRangeDecoration;
  interaction?: ParagraphRangeInteraction;
  accessibility?: ParagraphRangeAccessibility;
  metadata?: Readonly<Record<string, unknown>>;
};

type ParagraphRangeKind =
  | 'link'
  | 'inlineCode'
  | 'highlight'
  | 'selection'
  | 'source'
  | (string & {});

type ParagraphRangeDecoration = {
  backgroundColor?: string;
  cornerRadius?: number;
  underline?: TextDecorationStyle;
  squiggle?: TextDecorationStyle;
  zIndex?: 'behindText' | 'overText';
};

type ParagraphRangeInteraction = {
  action: 'activate' | 'select' | (string & {});
  target?: string;
};

type ParagraphRangeAccessibility = {
  role?: 'link' | 'mark' | 'comment' | (string & {});
  label?: string;
};

type TextDecorationStyle = {
  color?: string;
  thickness?: number;
  style?: 'solid' | 'dashed' | 'dotted' | 'wavy';
};
```

The important property is that paragraph preparation is **width-independent**. Width changes do not rebuild the paragraph unless the content or layout-affecting text style changes.

Range annotations are semantic and presentational overlays keyed to paragraph character offsets. They are intentionally separate from shaping inputs: link metadata, inline-code backgrounds, search highlights, source ranges, and selection highlights do not change line breaking or glyph shaping, so they should not invalidate the expensive prepared paragraph unless a future annotation type explicitly declares itself layout-affecting. Span metadata remains useful for facts tied to a single span, but the canonical annotation shape is paragraph-local because real ranges can cross spans, overlap each other, and be transient.

## Query Surface

Prepared paragraphs expose geometry queries richer than scalar measurement:

```ts
type ParagraphMetrics = {
  width: number;
  height: number;
  lineCount: number;
  didTruncate: boolean;
};

type ParagraphLine = {
  index: number;
  textRange: { start: number; end: number };
  width: number;
  height: number;
  baseline: number;
  origin: { x: number; y: number };
};

type LineCursor = opaque;
```

Required v1 queries:

- `measure(maxWidth)` -> width/height/lineCount/truncation
- `minContentWidth()` -> intrinsic min-content width
- `maxContentWidth()` -> intrinsic max-content width
- `layoutLines(maxWidth)` -> array or iterator of line boxes
- `nextLine(cursor, maxWidth)` -> one line at a time
- `rangeRects(start, end, maxWidth)` -> rects for selection/highlight
- `rangeAnnotationsAt(position)` -> active semantic/decorative annotations at a paragraph-local text position
- `hitTest(point, maxWidth)` -> text position / grapheme boundary plus active range annotations
- ~~`lineMetrics(maxWidth)`~~ — **subsumed by `layoutLines`** (disposition
  2026-07-12): per-line width/height/baseline/origin ride `ParagraphLine`;
  no separate method ships or is required. Ascent/descent/leading are NOT
  in `ParagraphLine` today and the web baseline is a style heuristic —
  exact vertical metrics land with the `exact-clusters` accuracy work,
  which decides whether they extend `ParagraphLine` or a metrics query
  returns.

Optional but strongly recommended v1.1 queries:

- `findTightestWidthForLineCount(lineCount, minWidth, maxWidth)`
- `findBalancedWidth(targetHeight | targetLineCount, bounds)`
- `layoutWithLineWidths(getWidthForLineY)`

The last query is what unlocks obstacle-aware and editorial layouts. It must exist somewhere in the system, even if the first public API is only used by internal layout primitives.

### Position units and geometry accuracy

Two contracts every backend must declare, added in the 2026-07 revision
(they were implicit before, and the shipped backends surfaced the gap):

**Position units.** Paragraph-local text offsets (`ParagraphSpec.ranges`,
`textRange` in lines and fragments, `hitTest` results) use **one canonical
public unit: UTF-16 code units**. "Backend-declared" was the r1 wording and
it was wrong for a shared input type: `ParagraphSpec` is one
cross-platform structure, so the same annotation range must identify the
same text on every backend — the unit belongs to the *spec*, not the
backend. UTF-16 is the canonical choice because it is what both shipped
backends already count (the web backend accumulates JS string lengths;
CoreText/AppKit speak UTF-16 natively); a backend whose engine counts
differently converts internally. The kernel selection document counts
graphemes; the **grapheme↔paragraph-offset mapping is owned by the
selection hosts** at their boundary (they already convert
grapheme↔UTF-16 today), per RFC 0017 §Text position domains — no unlabeled
number crosses between the geometry service and the selection system.
ENG-24416 makes the unit executable on the API: offsets use branded
`Utf16Offset` / `Utf16Range` types, results carry `offsetUnit: 'utf16-code-unit'`,
and every backend descriptor declares the same unit.

**Attachments in the canonical stream:** an attachment span contributes
exactly **one U+FFFC (object replacement character) code unit** to the
paragraph's UTF-16 stream; its `altText` lives in the copy/accessibility
plane, never in the offset stream. This makes the same `ParagraphSpec`
yield the same offsets on every backend. Both shipped backends currently
declare attachment shaping unsupported and reject it before layout. That is
the conforming fail-closed posture until a backend can shape U+FFFC without
skewing the canonical stream; `altText` still belongs only to copy and
accessibility.

A shared Unicode fixture corpus (emoji ZWJ, flags,
combining marks, ligatures, mixed bidi, truncation, attachments) pins the
backends and the conversions together (RFC 0017 work C).

**Geometry accuracy.** Each backend (and each prepared paragraph) declares

```ts
geometryAccuracy: 'exact-clusters' | 'approximate'
```

`exact-clusters` means `rangeRects`/`hitTest` are computed from the shaped
line (cluster/glyph geometry after shaping, bidi reordering, truncation,
and attachment placement) — what this RFC promises. `approximate` means
interpolated. **Both shipped backends are `approximate` today**: they
prorate character offsets across the line width
(`intersectRangeRect`/`hitTestLines` in `native/apple-backend.ts:479-528`
and `web/pretext-backend.ts:978-1040`), which is wrong for proportional
fonts, ligatures, and bidi. Approximate geometry remains useful for demos,
masonry estimation, and layout search. The gate is normative: **selection
highlights, carets, link hit-targets, and accessibility geometry must not
consume `approximate` geometry** — those consumers fail closed (keep using
native text-engine geometry) until an `exact-clusters` backend exists on
their platform. Cluster-accurate `rangeRects`/`hitTest` (CTLine/TextKit
per-run geometry on Apple; DOM `Range.getClientRects` or shaped-run data on
web) are the work that flips the flag.

ENG-24416 landed the enforcement mechanism: each backend and prepared
paragraph publishes `geometryAccuracy`, `offsetUnit`, and one field/query
capability descriptor; query results carry the same metadata; and
`assertExactClusterGeometry` is the fail-closed consumer gate. Both shipped
backends remain `approximate`. Unsupported geometry-affecting fields and
attachments now reject before shaping instead of being ignored or changing
the canonical offset stream. RFC 0096 consumes this descriptor rather than
forking a second capability vocabulary.

## Range Annotations and Decorations

Prepared paragraphs should own the range model used for rich inline text behavior. Markdown links, inline-code backgrounds, syntax-token backgrounds, search hits, source maps, AI annotations, and selection highlights are all the same class of data: paragraph-local ranges attached to metadata and optional decoration. They should not each build a parallel interval table or paint layer.

The engine responsibility is:

- preserve the annotations through paragraph preparation without making them shaping inputs
- expose `rangeRects` for any annotation range after final line breaking, bidi reordering, truncation, and attachment placement
- make hit testing return both the text position and the active annotations at that position
- hand range-decoration fragments to the rendering layer defined in RFC 0096

This keeps the high-cost path clean. Shaping and line breaking still depend on text, attachment metrics, and layout-affecting style; ordinary metadata and decorations do not. Selection should be the first internal client of this primitive, so Exact does not grow one highlight system for selection and another for Markdown/code/search.

**Convergence decision (2026-07-12, shared with RFC 0017 §Highlight paths and RFC 0096).** The shipped selection system did not take this path: default-
path selection highlights paint through native text-view selection hosts
(RFC 0017 §Components 4–5), while `RangeDecorationFragment`s exist only on
the RFC 0096 custom-surface path. That is exactly the two-system outcome
this section warns against, so the set now records the decision explicitly
rather than leaving the promise dangling:

- **End state: convergence stands — on identity and geometry, not on one
  painter.** When LLP 0172's static-text milestones move Apple static text
  onto this RFC's paragraph lowering with `exact-clusters` geometry,
  default-path selection highlights become transient `kind: 'selection'`
  range annotations whose **geometry** comes from the shared prepared
  paragraph — selection becomes this primitive's first internal client, as
  designed. The **painter stays surface-specific**: the native text host
  keeps painting the default path (0172's CTLine arm borrows RFC 0096's
  technique without adopting its opt-in surface model), and RFC 0096
  decoration-fragment painters own custom surfaces. RFC 0017 §Highlight
  paths states the same boundary.
- **Interim boundary (accepted, not drifting):** native text views own
  default-path highlight painting; decorations own custom-surface painting;
  neither claims the other's surface. The `geometryAccuracy` gate above is
  the precondition — convergence cannot happen on `approximate` geometry.
- **Owner and date:** the switch of the default path is RFC 0017
  remaining-work G, owned by the author through the LLP 0172 program, with
  a scheduled revisit at the **2026-09-09 checkpoint**. 0172 is currently
  Draft with a stranding notice, and its M2 text calls this RFC's paragraph
  lowering a blocker — that claim needs re-derivation rather than simple
  retirement: the lowerer exists (runtime-side), but it is not yet the
  shared paragraph feed (see the ledger), so M2's real precondition is the
  canonical-feed work, not the lowerer's existence. 0172 reconciles that
  when next touched. Without the dated revisit, the "accepted interim"
  would become permanent by default — the exact failure mode this section
  warns against.

## Backend Placement

The Text Geometry subsystem should be **host-owned**, not kernel-owned.

Rationale:

- web does not use the kernel for ordinary rendering
- platform text engines are host-specific
- rendering and selection already have strong host integration
- Exact should not move shaping logic into Rust just to send results back to host renderers

The kernel becomes a client of the service, not the owner of the service.

### Native

On native platforms:

- iOS/macOS backend: CoreText / TextKit-backed prepared paragraphs
- Android backend: MeasuredText / StaticLayout / platform shaper-backed prepared paragraphs

Prepared paragraphs are cached in a host-side registry keyed by paragraph content and layout-affecting style data.

*Windows (explicit deferral, 2026-07-12):* a DirectWrite-backed geometry
arm is deferred, not omitted by oversight — the Windows host already has a
reusable DirectWrite measurement service (`packages/exact-host-windows/src/windows_host.rs`,
used by LLP 0323's segment path), which is the natural substrate when the
Windows text lane reaches interactive text. Owner: the Windows host lane,
gated on LLP 0323's Windows arm; until then Windows is out of this RFC's
claimed scope.

*Shipped state:* the Apple backend today is a **JS-side prepared object and
cache over stateless host measure/layout calls**
(`packages/exact-text/src/native/apple-backend.ts`) — each query re-enters
the host rather than holding a native prepared-paragraph handle. LLP 0323
records this distinction explicitly ("0095 prepared paragraphs are not
landed natively; the host text-geometry service is stateless measure/layout
host-calls"). True native prepared handles — a host-side framesetter/layout
object with identity that survives across queries — remain this RFC's
open native work, and are also the path to `exact-clusters` accuracy on
Apple.

### Web

On web:

- default `Text` remains semantic DOM/CSS
- the geometry service is used only when some component or internal primitive explicitly opts into geometry queries
- the backend can be Pretext-like or directly built on the same prepare/query model

This preserves Exact's existing "real web on web" position while still allowing geometry-driven features.

## Kernel Integration

**Revised 2026-07-12: the default-text measurement substrate is owned by
LLP 0323, not this RFC.** The original proposal here (native text nodes
lower to a `TextLayoutRef { paragraph_id }`; the kernel queries a host
paragraph service for min/max/measured sizes) was not built. What shipped
instead is LLP 0323's substrate: a **kernel-owned, host-fed segment
measurement cache** (`kernel/src/text_segments.rs`,
`EXACT_TEXT_SEGMENT_CACHE=1`) with descriptor-local rewrap, sitting under
the measurement callback path. LLP 0323 §6 explicitly claims that ownership
boundary and names the merge path: when this RFC's paragraph handles land
natively, the segment cache and the prepared paragraph converge on one
identity.

### Current model (as of 2026-07)

- The measurement callback is no longer the scalar-only channel this RFC
  originally described: it carries styled multi-run measurement
  (ENG-22729), the host keeps framesetter-level caches, and the LLP 0323
  segment cache adds kernel-side prepared-segment identity for default
  `Text`.
- What is still missing — and still this RFC's claim — is **shared
  paragraph identity across measurement, rendering, selection, and
  geometry**: the segment cache serves layout measurement only; rendering
  builds its attributed strings separately; selection paints through
  native text views; the explicit geometry API (`@exact/text`) prepares its
  own paragraphs.

### Re-scoped model

This RFC's kernel story is now scoped to the **explicit geometry layer**:

- `TextLayoutRef`/`paragraph_id` as a kernel-visible concept is deferred
  until the LLP 0323 merge point — the kernel does not grow a second
  paragraph identity while the segment cache is the shipping substrate.
- The convergence target (unchanged in direction): one prepared paragraph
  per text node, shared by the LLP 0323 measurement path, the renderer's
  attributed output (measure == render, LLP 0223/0172), selection range
  geometry (§Range Annotations convergence decision), and the `@exact/text`
  query surface.
- Sequencing: native prepared handles land in the host geometry backend
  first (§Backend Placement), prove `exact-clusters` accuracy, then merge
  with the segment cache per LLP 0323 §6 — not the other way around.

The consequences claimed by the original model (cheaper repeated
measurement; one paragraph serving selection and rendering; advanced-layout
reuse) remain the point of the convergence — they are just reached through
LLP 0323's substrate rather than by replacing it.

## Renderer Integration

Nested `Text` already lowers to attributed content conceptually. That should become the canonical paragraph input for the geometry service.

### Native

On native:

- the renderer resolves nested `Text` trees into a `ParagraphSpec`
- the render surface uses that same spec to build attributed text
- the geometry service prepares the paragraph from that same spec
- selection and range mapping use the same prepared paragraph

### Web

On web:

- default `Text` continues to emit semantic DOM nodes
- when geometry is requested, the renderer also prepares a paragraph spec for the geometry service
- advanced containers can use geometry queries to compute widths/heights/frames while still rendering DOM where appropriate

This avoids the trap of making semantic text and geometry text two unrelated systems.

## Selection, Accessibility, and Hit Testing

This RFC intentionally ties text geometry to selection instead of treating selection as a later add-on.

The paragraph service should own:

- grapheme and line boundary mapping
- range-to-rect projection
- point-to-text-position hit testing
- attachment boundary behavior

Selection should project onto the prepared paragraph, not rebuild a separate best-effort model after rendering. (Status: the shipped cross-view
selection system projects onto the kernel `SelectionDocument` and paints
through native text-view hosts — see the convergence decision in §Range
Annotations for how and when it moves onto prepared paragraphs. The
`geometryAccuracy` gate applies to every consumer named here.)

Accessibility should continue to use native text semantics, but paragraph metadata such as line breaks, truncation state, and attachment alt text should derive from the same paragraph input.

## Threading contract (LLP 0297)

The geometry service is a synchronous query API by design — layout callers
(Taffy measure functions, LLP 0349 programmable-layout consumers, and RFC 0096 composition) need
answers inline. Under the LLP 0297 threading contract that requires an
explicit shape, not an assumption:

- **Sync-by-design, allowlisted.** The shipped Apple backend's host calls
  (`packages/exact-text/src/native/apple-backend.ts`) and the React
  consumption surface (`packages/exact-react/src/text-geometry.tsx`) are
  entries in the LLP 0297 W4b `sync-host-call-allowlist`
  (`exact-verify.json`; "sync-by-design, W3 decision"). New geometry
  entry points that call the host synchronously extend that allowlist
  entry — they do not silently add sync surface.
- **Same-thread queries only.** A prepared paragraph is queried on the
  thread that owns its backend. The web backend is single-threaded by
  nature. The Apple backend's host calls run on the caller's thread against
  thread-safe text engines; nothing in this RFC may introduce a
  cross-thread synchronous wait (kernel↔main or otherwise) — LLP 0297's
  prohibition is absolute for new surfaces.
- **Callback affinity.** Any new host callback this RFC's native work adds
  (prepared-handle create/query/release) declares its thread affinity in
  `docs/callback-affinity.md` before merge, per CLAUDE.md §6.5.
- **Selection interplay.** The selection gesture loop runs main-local over
  the RFC 0017 document mirror and never blocks on the kernel; when
  selection converges onto prepared-paragraph geometry (work G), those
  geometry queries are main-thread host-local calls, preserving that
  property.

## Relationship to Programmable Layout (LLP 0349)

LLP 0349 consumes prepared paragraphs and their query results to inform explicit
layout primitives. It owns execution context, determinism, invalidation,
cycle detection, budgets, child-frame ownership, Taffy interaction, and the
authoring API.

This RFC owns the paragraph service independently of that caller. A geometry
query receives explicit inputs and returns a result with capability, accuracy,
unit, and identity metadata. No custom-layout callback or worklet is defined
here, and accepting this RFC does not accept LLP 0349.

## Consumption Boundary

Ordinary `Text` remains semantic and platform-native. Internal convergence may
reuse prepared paragraph state without changing its authoring API.

Explicit consumers include:

- selection/accessibility geometry once `exact-clusters` is available;
- custom text composition and surfaces (RFC 0096);
- geometry-driven layout primitives (LLP 0349);
- diagnostics such as overflow, line count, range rects, and fit checks.

Every consumer checks the backend descriptor before relying on a feature.
Interaction-critical consumers additionally enforce `exact-clusters`.

## Caching and Invalidation

Prepared paragraphs should be cached by a stable key containing:

- span text
- all layout-affecting text style fields
- locale and directionality inputs
- line break / overflow / truncation policy
- font resolution identity
- backend version that affects shaping or measurement

Width must **not** be part of the prepared paragraph key.

Width-specific query results may have their own secondary caches.

Invalidation must happen when:

- any layout-affecting span/style changes
- a referenced font loads or unloads
- Dynamic Type / text scale changes
- locale or writing mode changes
- backend shaping or measurement configuration changes

## Package and Module Structure

This RFC implies a project restructuring.

Suggested shape:

```text
packages/
  exact-text/
    src/
      paragraph-spec.ts
      geometry-api.ts
      web-backend/
      shared/

  exact-renderer/
    src/
      text/
        paragraph-lowering.ts
        paragraph-cache.ts
        render-surface.ts

kernel/
  src/
    text_layout_ref.rs
    // remove long-term dependency on heuristic scalar-only text measurement
```

On native, the platform hosts own the actual backend implementation. `exact-text` defines the shared API and web backend.

*Shipped state:* `packages/exact-text` exists with this shape —
`paragraph-spec.ts`, `geometry-api.ts`, `composition-api.ts` (RFC 0096),
`web/` (Pretext-backed), `native/apple-backend.ts` — plus
`packages/exact-renderer/src/text/paragraph-lowering.ts` and
`packages/exact-react/src/text-geometry.tsx`. The execution-plan document
(`docs/plans/text-geometry-web-first-execution-plan.md`) still reads as
instructions to *create* this package and its demos; treat it as the
historical plan those artifacts executed, not as remaining work.

### Authoring surface (LLP 0160)

The low-level geometry service predates Contract bindings and currently ships
through `@exact/text`; its React-tier consumers live in
`@exact/react/text-geometry.tsx`. This is a logged legacy exception, not a
precedent for new React-first host capability. Graduation from the incubator
requires a Contract-consumable geometry surface. LLP 0349 owns Contract
bindings for programmable-layout primitives; RFC 0096 owns bindings for
custom text surfaces.

## Public API Direction

The low-level API is deliberately layered and **incubator**:

```ts
const paragraph = await TextGeometry.prepare(spec);
const metrics = paragraph.measure({ maxWidth: 320 });
const lines = paragraph.layoutLines({ maxWidth: 320 });
```

`TEXT_GEOMETRY_API_STABILITY`, branded UTF-16 offsets, capability descriptors,
and result metadata make that status and its position/accuracy contract
machine-readable. Graduation requires at least two healthy backends, exact
cluster geometry for fidelity-critical consumers, a Contract-consumable
surface, and a settled cross-thread artifact ownership model.

Higher-level layout components are not standardized here: LLP 0349 owns layout
primitives, while RFC 0096 owns composed-text components and surfaces. Agent and
devtool consumers may expose overflow, line-count, range, and fit diagnostics
only with the same capability and accuracy labels.

## Web Semantics

This RFC does **not** change the foundational web decision:

- Exact on web continues to use real DOM elements with real CSS by default
- text selection, accessibility, SEO, and browser tools remain first-class

Geometry mode is an additive capability, not a replacement web renderer.

When an advanced layout primitive uses geometry on web, it should still prefer semantic DOM output unless the chosen surface is explicitly canvas/SVG/custom drawing.

## Why This Is Better Than "Just Improve the Measurement Callback"

We could incrementally enrich the current callback:

- add more style fields
- add caching
- maybe add line count

That would help, but it would not solve the actual architectural problem. The real limitation is that the system has no reusable paragraph identity and no place for advanced layout to consume paragraph geometry.

The callback model fundamentally optimizes the wrong thing: one-off scalar measurement.

## Alternatives Considered

### 1. Keep the current callback and add caching

Rejected as the main plan.

This should still happen as a short-term improvement, but it does not provide:

- line geometry
- hit testing
- range rects
- width search
- shared paragraph identity across rendering and selection

### 2. Move all text layout into the Rust kernel

Rejected.

This fights Exact's web architecture, duplicates native text engine behavior, and creates a much harder rendering/selection integration problem.

### 3. Use a single exact text engine on every platform

Rejected for now.

That would be a different company-sized project. It would also sacrifice native fidelity and accessibility advantages that Exact currently benefits from.

### 4. Encode Pretext-like behavior only as a web optimization

Rejected.

The web use case is the inspiration, but the right abstraction is broader: reusable paragraph geometry for all platforms, with different backends.

### 5. Add special-case components without a shared subsystem

Rejected.

That creates five incompatible mini-engines:

- one for auto-grow text
- one for masonry estimation
- one for selection
- one for shrinkwrap
- one for custom drawing

The system would become impossible to maintain.

## Implementation Plan

The original web-first plan is now a historical execution record. Remaining
work is geometry-specific:

### G0 — Contract honesty (shipped in ENG-24416)

- label the API `incubator`;
- brand and publish the UTF-16 offset unit;
- publish backend/paragraph/result accuracy and capabilities;
- reject unsupported geometry-affecting inputs and attachment shaping;
- provide the exact-cluster fail-closed assertion.

### G1 — Canonical paragraph feed

- make nested-text lowering the canonical input for measurement, rendering,
  selection mapping, and explicit geometry;
- eliminate independent run derivation where it can drift;
- keep ordinary rendering behavior unchanged.

Exit: the same paragraph content identity feeds every consumer, with parity
fixtures proving measure/render text and offset equality.

### G2 — Native prepared artifacts and exact clusters

- add true native prepared artifacts on at least one Apple path;
- implement cluster-accurate line/range/hit-test geometry;
- declare callback affinity and avoid every cross-thread synchronous wait;
- choose the immutable artifact/snapshot handoff between runtime-thread
  measurement and main-thread rendering/selection.

Exit: Apple and web both expose honest descriptors; at least one platform
satisfies `exact-clusters`; fidelity-critical consumers can opt in without
approximation.

### G3 — Default-path convergence

- reconcile the native prepared artifact with LLP 0323's default-path segment
  cache rather than growing a second identity;
- move selection geometry onto the canonical paragraph at the LLP 0172/2026-09-09
  checkpoint while retaining surface-specific painters;
- add Contract consumption and agent diagnostics.

Exit: one paragraph object can power measurement, rendering identity,
selection/range geometry, accessibility facts, and diagnostics.

LLP 0349 and RFC 0096 maintain their own implementation phases. Their progress
does not change this RFC's status.

## Risks

### Cross-thread artifact ownership

Layout/measurement runs on the runtime thread while native rendering and
selection are main-owned. A “shared paragraph” cannot mean one thread-unsafe
object read synchronously from both. G2 must choose immutable snapshots or
backend draw references with explicit lifetime and affinity.

### Drift between semantic rendering and geometry

Different font resolution, line-breaking, or attachment streams make geometry
untrustworthy. Canonical input and parity fixtures are merge gates.

### Accuracy overclaim

Both shipped backends remain approximate. Capability metadata is necessary but
not sufficient; fidelity-critical callers must actually invoke the fail-closed
gate.

### Web overreach

Geometry is opt-in. Real DOM/CSS remains the production-web default.

### Public API premature exposure

The API is explicitly incubator until the graduation gates above. New breaking
shapes must update the machine-readable stability contract and migration notes.

## Open Questions

1. **Resolved by ENG-24416:** `@exact/text` is an incubator API; unit,
   capability, and result-shape changes remain allowed until graduation.
2. At the LLP 0323 merge point, does the segment-cache key become the paragraph
   content identity, or does a host paragraph handle supply a subordinate
   segment table?
3. What immutable artifact crosses between runtime-thread measurement and
   main-thread rendering/selection without a synchronous wait?
4. How much line-by-line variable-width geometry belongs in the public
   geometry API versus remaining an internal capability consumed by LLP 0349
   and RFC 0096?
5. What is the first supported attachment shape? Both backends currently reject
   attachment shaping to preserve the one-U+FFFC offset invariant.
6. Should preparation be eager during reconciliation, lazy on first query, or a
   hybrid cache fill?
7. Which decoration fields are guaranteed across platforms before RFC 0096
   surfaces graduate?
8. What exact-cluster source wins per platform (Apple CTLine/TextKit 2; web DOM
   Range versus shaped-run data), and what are its scale costs?
9. When does Windows add a DirectWrite geometry backend beyond LLP 0323's scalar
   measurement service?

## Conclusion

Exact should promote prepared paragraph geometry to a first-class, honest,
platform-backed subsystem shared across measurement, rendering identity,
selection, accessibility, and explicit queries.

That geometry contract stands on its own. LLP 0349 owns the separate
Programmable Layout proposal, and RFC 0096 owns composition and custom text
surfaces; neither expands this RFC's acceptance scope.

## Revision record

- **r7 (2026-07-12, ENG-24486):** retitled and narrowed the document to Text
  Geometry; moved the programmable-layout proposal to Draft LLP 0349; recorded
  ENG-24416's landed unit/accuracy/capability contract and incubator decision;
  made the dependency one-way (LLP 0349 consumes LLP 0095). The real Codex and
  Fable review records for the combined r1-r6 source are preserved in
  `llp/reviews/0095-text-geometry-and-programmable-layout.codex.md` and
  `llp/reviews/0095-text-geometry-and-programmable-layout.fable.md`. They do
  not review or approve the post-review r7 split or Draft LLP 0349.
- **Reviewed r1–r6 source record (preserved verbatim):** 2026-07-12 (r6: Review transition, author-authorized —
Status Draft→Review; the promised accuracy/unit/capability tracking issue
is FILED: **ENG-24416** (the three mechanisms land together); the round-6
rev-pin heading leftover in §Range Annotations fixed. The retitle/scope
split (narrow to "Text Geometry"; companion carries Programmable Layout)
remains the recorded intent for the author's next touch.
 r5, same day: round-5 polish — un-pinned the convergence
cites from "rev 9"; named the tracking vehicle for the unbuilt-normative
accuracy/unit/capability mechanisms (one issue, filed with the OQ 1
incubator decision at the Review transition; the three mechanisms land
together — 0096's composer fail-closed rule dangles until all three
exist).
 r4, same day: round-4 polish — recorded the retitle/scope
intent (the deferred Programmable Layout half still colors the title and
Summary; the author decides at the Review transition whether this document
narrows to "Text Geometry" with the companion split carrying the layout
half); narrowed the Motivation claim to "no *shared cross-consumer*
paragraph identity" (Apple already keeps local attributed-string/
framesetter/CTLine caches); marked the execution-plan pointer as a
superseded historical sequence; flagged OQ 1 (public vs incubator API) as
decision-needed before the unit/accuracy reshape starts.
 r3, same day: round-3 repairs — gave `lineMetrics` a
disposition (subsumed by `layoutLines`; ascent/descent absent from
`ParagraphLine` and owed with the accuracy work); relabeled Phases 0 and 2
partial against their own exit criteria; qualified the paragraph-lowering
ledger row (the lowerer exists runtime-side but is NOT the shared paragraph
feed — kernel measurement and presenter attributed strings still derive
their own runs) and re-derived the 0172-blocker note accordingly; defined
attachment contribution in the canonical UTF-16 stream (one U+FFFC unit);
added the explicit Windows deferral.
 r2, same day: round-2 corrections — adopted
**UTF-16 as the canonical public paragraph-offset unit** (replacing
"backend-declared" for `ParagraphSpec`/geometry offsets, which was
semantically wrong for a shared input type); recorded that the
`geometryAccuracy`/unit declaration mechanism is itself unbuilt normative
work; fixed ledger drift in both directions (no `lineMetrics` method;
`ShrinkwrapText`/`BalancedText`/`FlowText` ARE implemented in
`@exact/react`; LLP 0323's cache is flag-gated opt-in, not a shipping
default); aligned the convergence wording with RFC 0017 (geometry
converges, the painter stays surface-specific) and gave the trigger an
owner and a 2026-09-09 checkpoint revisit; deferred §Programmable Layout to
a companion LLP; added the LLP 0160 authoring-surface note.
 r1, same day: shipped-state reconciliation — Implementation Status ledger;
deferred the default-text measurement substrate to LLP 0323 and re-scoped
§Kernel Integration; position-units + geometry-accuracy contract; LLP 0297
threading section; the selection-highlight convergence decision shared with
RFC 0017/0096)
