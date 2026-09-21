# LLP 1010: Scrolling v1 — what scrolls, who scrolls it, as built

**Type:** Spec
**Status:** Review (llp-review, one round on r1, 2026-08-29 at Charlie's request: codex NOT READY (7 MATERIAL, 2 MINOR), grok NOT READY (3 MATERIAL, 5 MINOR) — every finding folded into the code or declared in r2, dispositions in `llp/reviews/1010-scrolling-v1.{codex,grok}.md`; **r2 is unreviewed**. The code was reviewed the same day (`llp/reviews/code-2026-08-29-scrolling.{codex,grok}.md`), likewise folded.)
**Revised:** 2026-08-29 (r2 — the spec-review fold: the overflow computation made symmetric and its `auto` stand-in named; sizing vs contribution stated correctly; the root rule's border-box/min-max deviation declared; `content` floored with end padding; the page extent declared; §4 split into asserted and observed, with the smoke now asserting limits and the root's width; §5 split into built-untested and not built, with the `List` windowing seam stated as insufficient; LLP 1008 §5 pointed here.)
**Systems:** Kernel (node types, overflow rows, layout), Contract (`scroll` tag), Web host, Apple host
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Implementer:** Claude (Fable 5); the macOS behavior landed 2026-08-29 (this document transcribes it)
**Windowing work:** Codex, started 2026-09-14 at Charlie's direction ("ok do what you think" after the list-memory assessment). §6 records the staged implementation; fixed-height runner/web landed 2026-09-17, measured heights and Apple geometry 2026-09-18 through Markdown. Linux geometry, image budgets and Messages acceptance remain open.
**Related:** LLP 1001 §1 (the `ScrollView`/`List` default — the only per-tag default — and the root-width rule), LLP 1002 D4 ("scroll always wins": the platform recognizes and scrolls; the engine follows), LLP 1007 §1 (the web host's `<div data-scroll>`), LLP 1008 §1, §5 (the window as a viewport over a document; the chaining scroll view), `rules/RULES.md` §The web is the standard, `rules/NOT-DOING.md` §Components (no virtualList v2) and §Motion (scroll-vs-pan arbitration is the platform's)

## Summary

Scrolling is a **protocol fact and a host behavior, never a module**. The
kernel says what scrolls: a `ScrollView` or `List` node is a scroll
container on its block axis unless its `overflow_y` row says otherwise; its
children are laid out in its content space; the kernel publishes their
frames and **holds no scroll offset**. Each host scrolls it with the
platform's own scroller — the browser on the web, `NSScrollView` on macOS —
and the page itself scrolls the way a browser's document does: the window
is a viewport over a content-sized root. Where AppKit's defaults differ
from CSS, the macOS presenter makes them agree: a wheel a scroll node cannot
use chains to what contains it (`overscroll-behavior: auto`), and a nested
scroll view scrolls even when AppKit declines to move it. Every claim below
is held by a smoke that synthesizes wheel events and reads the scroll
positions back, on the Caltrain app and on a fixture whose list and page
both overflow. Where this document and the code disagree, the code and its
tests are the authority.

## 1. The kernel: what scrolls (`schema.json`, `style.rs`, `node.rs`)

- Node types `ScrollView` (id 3) and `List` (id 4) **scroll by default on
  the block axis** — y, hard-coded: `NodeType::scrolls_by_default` →
  `StyleProps::to_taffy` sets Taffy `overflow.y = Scroll` when the
  `overflow_y` row is unset (LLP 1001 §1's one per-tag default; on the web
  the same node is `overflow: auto`). `overflow_x` follows its row, or the
  next bullet.
- Style rows `overflow_x`, `overflow_y` (`visible | hidden | scroll`) are
  layout rows. They do not change how the node's own size is found — an
  auto height still comes from its children, on any overflow value — they
  change what the node **contributes and clips**: a `scroll`/`hidden`
  node's overflowing content does not enlarge its ancestors and is clipped
  at its box, so a node scrolls only when its used size is constrained
  (a set height, or a flex item under a constrained parent) and its content
  exceeds it. The Caltrain app's `scroll` node has neither (its parent is a
  block with auto height), so it never scrolls and the page does — as on
  the web. `scrollbar_width` is 0: scrollbars never take layout space, as
  overlay scrollers on macOS and the web's default on macOS do not.
- **The kernel holds no scroll offset.** Layout places children in the
  container's content coordinates from its top-left; a child's absolute
  frame is where it is at scroll offset zero. Scroll position is host
  state, never plan state, never a style row — "the platform recognizes;
  the engine follows" (LLP 1002 D4), and scroll always wins.
- **A `visible` axis beside a non-visible one becomes scrollable**, either
  way round — CSS Overflow §3 computes it to `auto`; the schema has no
  `auto`, so `scroll` stands in (Taffy sizes them the same). A default
  `ScrollView` is therefore a scroll container on both axes in the kernel
  and `overflow: auto` on the web; `overflow_x: hidden` alone makes y
  scrollable, not hidden.
- **A root fills the width it is offered** and is as tall as its content:
  `taffy_style` makes a root's `width: auto` into `100%` under border-box
  sizing, so the border box — padding and border inside it — is the offer,
  CSS's block rule, which Taffy does not apply to a root; re-derived on
  `AttachRoot`, since a node is styled before it is a root. It is what makes
  the page a document a viewport scrolls over rather than a box shrunk to
  its content (the first bare-root fixture laid out 89 pt wide, then 444
  under `100%` alone). Declared: a root's margins are not subtracted; an
  absolutely positioned root is exempt; and because the rule switches the
  root to border-box, a root's `min_width`/`max_width` constrain its border
  box rather than CSS's content box.
- **The kernel publishes scrollable overflow**: Taffy's `content_size` per
  node, on `NodeRef.content` — the content's extent in the node's own
  space, every descendant's overflow included. It is not quite CSS's
  `scrollWidth`/`scrollHeight`: Taffy's block containers do not always
  count end-edge padding (its flex containers do), so a host floors it
  with the direct children's extent plus the end padding, and with the
  node's own box. The page's extent is the hosts' own (§3).
- Contract's `scroll` tag is `ScrollView` with no fixed rows. The
  `overflow` attribute sets both rows; per-axis `overflowX`/`overflowY`
  attributes are not exposed yet (the rows exist).

## 2. The web host: the browser scrolls (`host/web`)

A `ScrollView` or `List` becomes `<div data-scroll="true">` — the
attribute is keyed by node type (`scrolls_by_default`), unlike the macOS
presenter — and the page's stylesheet says `[data-scroll="true"] {
overflow: auto; }`, both axes, which is what the kernel's pairing gives
the default case; an explicit `overflow_x`/`overflow_y` row lowers by name
as an inline declaration and wins over the stylesheet. Every node is
`position: relative` by a page rule (LLP 1001 §1's declared baseline; an
absolute node's inline style wins). The browser lays out, clips, scrolls,
chains at edges, restores nothing across a reload, and the window scrolls
the document as for any page. Ordinary scroll containers remain platform-owned. The explicit windowed `list` also reports its actual geometry to the runner (§6.2); this does not dispatch an application action.

## 3. The macOS host: `NSScrollView`, made to agree with CSS (`host/apple`)

**The page.** The window's content view is an `NSScrollView` (the
viewport); its document is a flipped view holding the roots, sized after
every batch to the roots' extent and never smaller than the viewport
(`Presenter.fitDocument`). A root taller than the window scrolls the way a
browser's document does.

**A scroll container.** The host writes each node's **effective
overflow** into its style dictionary (`overflow_x`/`overflow_y`, the
kernel's rule: a `ScrollView`/`List` scrolls on y unless its row says
otherwise, an unset x follows a non-visible y), and the presenter acts on
that, never on the node's kind: an axis that is `scroll` makes the node a
`ChainingScrollView` scrolling that axis, `hidden` clips
(`clipsToBounds`), `visible` does nothing. Its flipped document view holds
the children and is sized by the host's **`content` op** — the kernel's
`content` extent, at least the node's own size — after every layout,
emitted only when it changed. Layout frames are parent-relative, so
children land in the document at the coordinates the kernel gave them.
Scrollers are overlay; content insets are zero; `scrollbar_width` is 0 in
the kernel to match.

**The page's extent** is `max(viewport, union of the roots' frames)`
(`fitDocument`), not the kernel's `content`: a root's frame is its layout
height, the in-flow page; descendants overflowing a root's frame
(`overflow: visible` past it, absolute children) are outside the page's
extent on macOS, where CSS's document `scrollHeight` includes them — a
declared deviation until `fitDocument` reads the root's `content`.

**Chaining** (`ChainingScrollView.scrollWheel`): deltas are points when
the event has precise deltas (a trackpad) and lines × 40 pt otherwise (a
wheel; a browser-like tick — the *inner* consumer's constant; the page's
scroll view is a plain `NSScrollView` with AppKit's own line size). Per
axis, the view can take the delta when that axis scrolls, its document
overflows on it, and it is not already at that edge. The event's
**dominant axis** (the larger delta; a tie is vertical) decides who owns
it: if the view can take that axis, it scrolls itself by every axis
it can take — clamped, applied directly to the clip view, **never through
AppKit** — otherwise the whole event goes to the next responder, up
through the node views to the page's scroll view. This is CSS's
`overscroll-behavior: auto`, and it is not AppKit's default, which swallows
the event: on the first try the Caltrain app's `scroll` node — which does
not overflow, because its parent is a block with auto height and so the
*page* is what overflows, exactly as in the browser — trapped every wheel.
Scrolling itself rather than through AppKit was the second finding: a
nested `NSScrollView` may move the *enclosing* view or animate later for
an event handed to `super`, so any "did it move?" fallback can double a
delta; taking the delta directly is deterministic. What is given up: AppKit's
rubber-banding on inner scroll nodes (momentum still arrives, as further
events, and is applied the same way).

## 4. Held by

**The harness.** `host/apple/smoke.mjs` runs the app in smoke mode, where
`main.swift` synthesizes wheel events (`CGEvent` → `NSEvent`, phase-less —
a gesture's phases put the top-level scroll view into a tracking loop that
a synchronous `sendEvent` cannot feed) over the first `scroll`/`list` node
and prints a `scroll:` line: the page's and the node's positions after one
event to the page's scroll view directly, one to the node's, one to the
hit-tested view (`hitTest` then `scrollWheel`, the two things
`NSWindow.sendEvent` does), then two hundred more, with each view's limit,
whether the page ever moved while the node could still scroll, and the
viewport's width.

**What the smoke asserts** (the CI claims), for the hit-view delivery and
the two hundred: when the node overflows — one event moves the node and
not the page; the node keeps moving; it ends **at its limit** (not short
of it); the page never moves before that limit; then the page moves. When
the node does not overflow — the one event moves the page and not the
node; the page ends at its limit. In both — the root is exactly as wide as
the viewport (the kernel's root rule). The direct deliveries are printed,
not asserted.

**Observed** (one run each, 2026-08-29; a transcript, not a check): on
**the Caltrain app** `page 120→2348 (limit 2348)`, `inner 0→0`; on
**`contract/corpus/scroll.contract`** (a 200 pt list of forty rows on a
page of forty more; booted with `EXACT_PLAN=<plan>`, which runs any
compiled contract without a rebuild) `inner 0→652 (limit 652)` for an 852 pt
document in 200 pt, `page 40→471 (limit 471)`, root `420` wide in a `420`
viewport (it was 89, then 444, before the root rule and its border-box
form).

**Declared:** the events are synthesized and phase-less; a real trackpad
— phases, momentum — is exercised only by a hand. The fixture is run by
hand (`EXACT_PLAN`); the default smoke runs the app. The kernel's overflow
pairing and content-size column are held by unit tests in
`kernel/src/style.rs` and `cargo test --workspace` (171 tests,
2026-08-29).

## 5. Original omissions, and the current windowing boundary

**2026-09-14 correction:** the omission inventory below is the 2026-08-29
snapshot. Scroll events now carry `scrollLeft`/`scrollTop` to the runner on
web and Apple; programmatic offsets, per-axis overflow, overscroll policy,
and the admitted scroll-snap form have also landed. `scrollCommand` was
replaced by offset props. None of those changes implements windowing.
The current code is `host/web/glue.js`, `NodeViewIOS.swift`,
`NodeViewMac.swift`, and `runner/src/runner.rs`'s `Event::Scroll`.

**Built, untested:** keyboard and space-bar scrolling on the page (AppKit's);
horizontal scrolling on macOS (the code path takes x; the fixture is
vertical); `overflow: hidden` on the web (clips through the same rows; no
fixture); `hidden` and `visible` on a `ScrollView` on macOS (clip-only and
no scroller, by `applyStyle`; no fixture). A `ChainingScrollView`, once
made, is never torn down if the rows later change; a `content` op for a
`hidden`-only node is emitted and ignored (no document view).

**Not in v1:** per-axis splitting of one wheel event (an `NSEvent` cannot be split; the
dominant axis routes it whole, so the minor axis of a diagonal gesture at
an edge is lost or swallowed — the web splits); presentation transforms
in scrollable overflow (CSS includes them; the kernel's `content` is
layout only); a scroll offset or event reaching the runner (needed before
`List` can window — its `scrollCommand`/`virtualized` props exist in the
schema and are inert); rubber-banding on inner scroll nodes on macOS;
scroll position across a reload (the tree is rebuilt; LLP 1007 §9);
`overflowX`/`overflowY` in Contract (rows exist, attributes do not);
`overscroll-behavior: contain`/`none` (a row CSS has and the schema does
not); scroll snapping; `scrollIntoView` and programmatic scroll (an agent
operation, later).

**Current, 2026-09-17:** explicit fixed-height lists now have runner-owned row
lifetimes, logical extent and browser geometry feedback (§6.2). The inert
`virtualized` prop is deleted; its ordinal is now `itemHeight`. Ordinary
`scroll`/`each` remains eager. Apple/Linux still need the list geometry
path and interaction pins; do not migrate shared consumers before that
work lands. The old 60-fps-only trigger is replaced by §6's memory and
responsiveness requirements.

## 6. Bounded list memory — implementation plan, 2026-09-14

**Owner:** Codex. **Start:** 2026-09-14. **Consumer:** Messages inbox and
transcript, then the other apps' long lists. **Order:** baseline → runner
and web window → Apple and Linux → bounded raster loading → consumer sweep.
Baseline and the fixed-height runner/web slice are implemented; later slices remain open. Router/viewport work already assigned in LLPs 1038/1039 keeps
its assignment. ExactViewport is a viewport fact, not the missing size of
a nested scroll container; windowing must observe the actual scrollport.
This LLP returns to `current/`; 1036's research link leaves that overlay
to keep its 15-document budget. The research document itself stays.

**Scope trade:** list memory and construction cost trigger the work without
waiting for a dropped frame. Further Messages decorative Tapback/emoji
artwork, material matching and animation-timing polish move behind this
consumer. Navigation, editing and data-loss fixes remain functional work.
No virtualList v2, new blocking gate, agent verb, core Cargo feature, or
parallel layout engine. LLP 1037 F3 supplies research, not authority.

### 6.1 The promise and the five measurements

Let N be the supplied records and W the rows intersecting the scrollport
plus one viewport of overscan on either side. Input records and compact
key/height metadata may be O(N); **instantiated row state, kernel nodes
and host views must be O(W)**. The serialized plan's row template must
not expand with N. An app that loads all records therefore still has an
O(N) data cost. We do not call its process memory flat.

Use the existing `scripts/metrics.mjs`, with diagnostic cases in its
existing metrics binary and the existing agent carriers. Every result
names source/binary identity, host, viewport, record count and sample
phase. Unavailable measurements are `null` with a reason, never zero.

| Quantity | What is counted; what it must distinguish |
|---|---|
| Construction to first frame | Data acquisition/build, plan decode, runner construction, layout, host application and actual first pixel separately. A runner boot timer is not first pixel. Include any bake-time expansion separately. |
| Retained state | Encoded and decoded plan, input records, row instances/local slots and kernel allocations. Count shared storage once; label aggregate allocator deltas, estimates and exact counts as such. Report peak during replacement as well as settled live bytes. |
| Host objects | Actual mounted/live DOM elements or Apple views, including swipe tables, labels and image subviews. Kernel node count is a separate number. Linux reports no per-row native-view model, not a made-up view count. |
| Decoded images | Unique resident raster bytes, pinned/cache/in-flight reservations separately; encoded responses and platform symbol caches are different quantities. Browser-owned decoded bytes may be unavailable. |
| Process memory | Same OS metric at the same phases, with baseline, settled and peak distinguished. RSS, Apple physical footprint and browser-process memory are named separately; allocator bytes are not substitutes. |

Run N = 25 / 1,000 / 25,000 in fresh processes at the same viewport, with
text-only and distinct raster-image cases. Sample first frame, settled
top, middle, bottom, return to top, after 20 full traversals, after
replace/reorder, and after leaving the list. Repeated data and one shared
image cannot stand in for distinct records and bitmaps. Keep raw samples
and phase names beside any medians. Physical iPhone frame/memory results
are required before calling the phone budget satisfied; simulator results
are labeled separately.

**First delivery:** `bun scripts/metrics.mjs --list-memory [--json]` is
the eager text-row baseline only, one fresh process per N. It measures
requested live/peak heap deltas for data, decoded plan, runner and kernel,
live kernel nodes, decode/boot/layout time, and process RSS while the
runner is alive. The compiler and encoded input precede the heap baseline;
allocator slack and internal realloc transients are not tracked. Native
views, decoded image bytes and first pixel are explicitly unmeasured.
The counting allocator exists only in this diagnostic executable, never
in an app host.

**Baseline measured 2026-09-14:** Apple M5 Max, macOS 25.6.0 arm64,
Rust 1.97.0, optimized diagnostic, one fresh process per row count.
`--list-memory --json` passes the fixture's all-rows node-count assertion;
the decoded plan stays 894 bytes (730 encoded) at every N.

| Rows | Live kernel nodes | Input data, MiB | Retained heap delta, MiB | Peak heap delta, MiB | Process RSS, MiB | Runner boot / layout, ms |
|---|---:|---:|---:|---:|---:|---:|
| 25 | 27 | 0.002 | 0.097 | 0.126 | 3.28 | 0.294 / 0.068 |
| 1,000 | 1,002 | 0.084 | 3.275 | 4.298 | 9.19 | 2.558 / 0.760 |
| 25,000 | 25,002 | 2.098 | 95.945 | 124.555 | 109.94 | 81.628 / 29.990 |

The 25,000-record input is about 2.1 MiB; the retained data/plan/runner/kernel
aggregate is about 96 MiB before a native view or image is created. This
already earns the work. No first-pixel or phone claim follows from these
instrumented desktop timings. RSS includes the process, not just that
aggregate; different allocator/OS accounting need not order RSS and peak
requested heap the same way.

Raw sample: `/tmp/exact-list-memory-baseline.json`; source base
`818930f539551ef18da846ddf9518edb107c16e9` with working edits, recorded source
diff SHA-256 `3767311e51898723199d0ab410327e823b3ff6e540b922b95a3efa5e2142a5f5`,
binary SHA-256 `3f54c258cc3a1d9da826b959e27368be7c6acebf8f9298d7d07cc4ea60c9ac83`.
The command records the current identities when rerun; this sample is not
attributed to an unmodified HEAD.

### 6.2 Materialize the window before creating rows

Implement the first window in the runner, shared by every host, using
the existing keyed-region and create/destroy operations. Ordinary `each`
keeps its eager semantics; authoring the windowed case is explicit. The
compiler's row-template representation stays compact; define its small
opt-in surface with the first working fixture, not a second list DSL.
Do not turn on the inert prop while still building every row.

The list owns its total logical extent, visible range and estimated/measured
row heights. Hosts supply the actual scrollport/offset and measured row
boxes (browser layout on web; kernel layout on native). The engine
publishes the spacer/offset geometry needed to keep scrolling that logical
document. Offset-only updates visit the window; scanning/keying every
record on every drag frame is not acceptable. Membership/order changes
may rebuild the compact index; ordinary row updates retain keyed identity.

Start with fixed-height inbox rows to establish lifetime and geometry,
then variable-height transcript rows before acceptance. Preserve an
anchor key and its offset within the scrollport when heights change,
older messages prepend, images arrive, width changes, or records reorder.
If the anchor is deleted, use the next surviving neighbor, then the
previous one. Follow the end only when the user was already following it.
Apply extent/window/anchor changes coherently; never reset native momentum
or take over scroll recognition.

Rows leaving the window lose their view instances and component-local
slots. Drafts, selection and durable per-message state belong in the
existing parent/keyed data model. A focused editor or active native
interaction can pin its row until completion; pins are explicit, counted
and bounded (at most one focused row and one other interacting row).
Switching interactions releases the earlier pin. A keep-alive map that
grows with every visited key fails the memory requirement.

**First slice, 2026-09-17 — fixed-height runner and web:**

```contract
list item-height=24 height=240 width=390
  each item in rows key=item.id
    Cell(item=item)
```

`list` requires one direct keyed `each`, a positive literal `item-height`
in CSS pixels, and a constrained scrollport (`height`, `max-height` or
`flex`). This is explicit host policy, not a CSS property. Each item gets
an absolute, clipped fixed-height wrapper inside a full-height content
box. The wrapper has `role=listitem`, `aria-posinset` and `aria-setsize`;
the list has `role=list`. Normal CSS governs authored descendants. Row
height does not change while mounted. A changing width may wrap content
inside that fixed box; variable-height measurement is still owed.

`Runner::list_viewport` takes the actual offset, viewport height, content
origin and at most two descendant pins. The web ABI's `exact_list` reports
these on scroll, resize and focus/pointer changes, after batches and
before paint. Browser scroll anchoring is disabled on this list because
the runner preserves the first visible key and its intra-row offset.
An authored `scrollTop` change overrides that anchor. Pointer release
keeps its pin through the following click; focus retains its row until
blur. Viewport creation begins with two rows, then host measurement fills
the actual viewport plus one viewport on either side. Removed rows lose
instances and local slots; returning creates fresh instances.

Scroll-only updates neither settle resources nor evaluate every key.
App/data updates validate all keys (including offscreen duplicates),
rebuild the O(N) key index, and update retained rows. Reordering preserves
keyed identity; deleting the anchor selects the next surviving neighbor,
then the previous one. Input records and key metadata remain O(N). This
slice bounds instantiated UI, not total memory independently of N.

The existing memory diagnostic now compares eager and windowed rows in
fresh processes and reports initial versus post-traversal retained heap.
Windowed cases traverse every row down and up twenty times. Native views,
decoded image bytes and first pixel are still explicitly unmeasured by
that runner diagnostic. Native geometry/pins, accessibility navigation
to uninstantiated rows, variable heights, end-follow and Messages adoption
are not completed by this slice.

**Measured runner comparison, 2026-09-17:** M5 Max, Darwin 25.6.0,
Rust 1.97.0, release binary; 844px viewport and 24px text rows. Both modes
use the same binary, with tracking enabled. Heap includes input records,
decoded plan, key metadata, row instances, kernel and retained receipts;
RSS is the whole runner process, not native app footprint.

| Records | Mode | Live kernel nodes | Retained heap, MiB | Peak heap, MiB | RSS, MiB |
|---:|---|---:|---:|---:|---:|
| 25 | eager | 27 | 0.115 | 0.144 | 3.438 |
| 25 | windowed | 52 | 0.180 | 0.235 | 3.703 |
| 1,000 | eager | 1,002 | 3.926 | 4.948 | 10.984 |
| 1,000 | windowed | 214 | 1.435 | 1.710 | 6.781 |
| 25,000 | eager | 25,002 | 112.159 | 140.769 | 130.719 |
| 25,000 | windowed | 214 | 5.533 | 5.808 | 11.625 |

Windowed rows pay for an extra wrapper, so a short list can cost more.
The 25,000-record input alone is 2.098 MiB; input and key/index storage
explain the remaining dependence on record count. At 25,000, retained
requested heap is exactly 5,801,313 bytes after traversal 1 and traversal
20 at the same middle position. Kernel storage peaks at 356 slots for
both long sizes, reusing retired slots. The smaller runs are still warming
the kernel's bounded 64-receipt ring; the initial-to-traversed high-water
increase must not be confused with retaining visited rows.

The final loaded-machine sample has eager boot/layout 143.93/64.73ms and
windowed boot/window-fill/layout 11.19/4.52/0.074ms. These are single
allocation-instrumented samples, not p50s or first-frame measurements.
Raw results: `/tmp/exact-list-memory-windowed-final.json`, source base
`d0f60f5` plus this slice; the JSON records source and binary hashes.

**Validation status:** six focused tests in `host/web/tests/lists.rs` pass:
bounded live/retained nodes and no data calls during twenty traversals;
pinned state/identity then release; reorder/deleted-anchor preservation;
resize/programmatic offsets/invalid geometry; web batch retirement and
accessibility positions; invalid authoring and offscreen duplicate keys.
The web artifact builds; JavaScript syntax, caps and boot checks pass.

**Browser acceptance, 2026-09-17:** a temporary Contract fixture using
Caltrain's real station data passed in headless Chrome through
`scripts/agent.mjs`, against a fresh isolated web build. It exercised actual
wheel scrolling, retention of a focused offscreen row, retirement on blur,
fresh identity on return, bounded mounted rows, and an authored scroll
jump with 12px scrollport padding. The 492px jump aligned row 7 (80px
rows) with the scrollport's top, accounting for the content origin.
Fixture and raw output: `/tmp/exact-list-acceptance.contract`,
`/tmp/exact-list-acceptance.mjs`, `/tmp/exact-list-acceptance.log`.

The prior disk-space block is cleared. Workspace validation remains blocked:
`cargo build --workspace` failed when the Caltrain Apple asset gate's
`exact-filesystem` subprocess exited before replying. A direct helper call
then passed, but the build retry and full test run were interrupted after
persistent native executable startup stalls. The test build completed in
17m28s and the initial Caltrain suites passed; a sampled test process stayed
at macOS `_dyld_start` before executing test code. Rerunning, concurrent
`--list` startup preparation, and copying a binary outside the build directory
did not clear the stall; validation stopped at the three-round limit.
Clippy was not reached. Formatting, caps, boot and the six focused list tests
pass. Logs: `/tmp/exact-dirty-build.log`, `/tmp/exact-dirty-build-retry.log`,
`/tmp/exact-dirty-test.log`, `/tmp/exact-test-startup-sample.txt`.
Finish workspace validation before committing this slice. Native geometry
and consumer acceptance remain open.

**Measured rows and the Markdown consumer, 2026-09-18:**
`estimated-item-height` selects variable-height rows instead of fixed
`item-height`. A prefix-sum index accepts actual heights, preserves the reading
key through changes, and invalidates measurements when width changes. The
browser sends DOM sizes; Apple sends scrollport geometry and reads row sizes
from kernel layout. AppKit and UIKit settle windows before paint. The regular
Markdown reader now uses this path; `apps/markdown/README.md` records measured
construction costs and verification limits.

Selection can outlive a row instance. Its logical endpoint is the opaque row
key, outer text-paragraph ordinal and UTF-16 offset. The runner projects text
from the same row template, one unloaded row at a time, without committing
kernel operations; mounted rows retain their current local state. AppKit and
browser selection use this path for partial and full-document copy. Copy does
not mount the selected range. This does not claim virtual accessibility
navigation, Linux geometry, or the image budget below is complete.

**Rationed reports on Apple, 2026-09-19** (Claude, for the Markdown reader;
measurements in `apps/markdown/README.md`). AppKit scrolls a contained list on
the thread that lays out, so what a geometry report costs is what a scrolling
frame costs. Three changes to the measured-row path, none to the web's:

- *One report settles.* `Host::list_viewport` reports, lays out, reads the row
  heights back from the kernel and reports again until nothing changes, then
  emits one batch. A created row used to take a second report from AppKit's
  frames — a second batch and a second `Presenter.apply`.
- *A report has a budget.* `Runner::list_viewport_within(view, geometry,
  create_limit)` creates every row the scrollport shows and at most
  `create_limit` more, nearest first; `Runner::list_status` says whether rows
  remain. Rows that left the window are retired by a report that also
  creates, a few at a time, or once nothing is left to create — never by a
  report of their own. `list_viewport` is the same call with no limit, and is
  what the web and an agent use. `exact_list` takes the limit as its last
  argument (0: the whole window); `exact_list_pending` asks.
- *The Mac presenter fills between frames.* A scroll that still shows mounted
  rows a quarter of a scrollport past both ends reports nothing; the window is
  filled from a `BeforeWaiting` observer ordered after Core Animation's commit,
  one row or one paragraph's painting at a time, for a quarter of a frame. A
  scroll that outruns it reports at once, and the rows it shows are never
  rationed: no refresh showed uncovered space at 48,000 points a second. UIKit
  does not do this yet.

The mounted set is therefore a subset of the window while a fill is pending and,
briefly, a superset while retirement is; both are bounded by the window.
`host/apple/tests/lists.rs` holds the three behaviours.

### 6.3 Raster memory is a separate budget

After view lifetime works, bound loading independently on Apple/Linux:
start with a **32 MiB per-session budget** for unique Exact-owned decoded
raster storage and decode reservations, including images pinned by live
views. Deduplicate identical source/size/generation requests; use at most
two simultaneous decodes, reserve from metadata before decode, and
downsample to the required display size when the source would exceed the
budget. Avoid a full-resolution intermediate. A bitmap cannot be declared
evicted while a view still owns its bytes. Queueing, cancellation and
stale-generation completion must not release another request's reservation.

Visible demand goes first; off-window requests are cancelled or dropped,
and unpinned cold entries evict first. If visible demand exceeds the
budget, lower the requested decode resolution or defer a load with an
observable reason; never silently exceed it. Handle memory pressure by
dropping reclaimable entries. This changes loading/cache policy, not
intrinsic CSS dimensions or `object-fit`. A future measured change to
32 MiB is an explicit budget change, not a hidden exception.

On web the browser owns decoding and eviction. Bound DOM/image demand
and measure the browser where supported; promise no Exact-controlled
decoded-byte ceiling there. Symbol/framework caches are likewise outside
the Exact raster budget and remain visible in process measurements.

**Native implementation starts 2026-09-17:** Newton owns the shared std-only
`exact-raster` admission/accounting core; Darwin and Epicurus wire Apple and
Linux image loaders. Tuft integrates and records before/after evidence. The
session budget outlives replaceable runtime generations; active, candidate and
retiring backings share it. A payload-free allocation charge follows each
physical backing until its final owner drops. Sharing one backing shares one
charge; making a pixel copy requires another reservation. Charge ownership has
no strong path back to a session, cache or payload.

The process admits at most two running decodes. Each session additionally has
two reserved delivery cells, counting running and completed-unconsumed results;
admission reserves a cell before allocating. A session whose UI does not consume
its two results cannot stop other sessions decoding. Completed bytes remain
charged, but a finished worker returns its process slot after scratch is gone.
Pause/shutdown discard unclaimed results without requiring a UI callback; active
cancelled work retains its reservation until its actual allocations drop.
Native payload destruction occurs outside accounting locks.

Pending job metadata is capped at 64 unique jobs per session, live subscriptions
plus detached pinned-cache identities at 1,024, and unpinned cold cache entries
at 64. Overflow/defer reasons are observable; no per-visited-source or
failed-request history is retained. Temporary byte
pressure waits for capacity; the host can replace demand with a smaller decode.
Natural image size remains independent of decoded pixel dimensions in layout
and `object-fit: none`/`scale-down`, as well as aspect-ratio-preserving fits.
The adapter must establish its allocation bounds before claiming acceptance:
metadata limits and thumbnail output dimensions alone do not prove a bound on
opaque codec internals. Record known owned raster/copy/scratch charges, encoded
storage and process memory separately. This paragraph is the implementation
assignment and selected policy, not evidence that either native loader passes.

Adapter source identity must survive as long as the same immutable backing can
be reused. A weak resolver index points to an immutable source owner retained
by current interests, workers and the native backing/provider. A bounded cold
metadata cache may also retain it. Removing a view cannot assign a different
source ID while the old pixels remain cached or pinned; a new asset generation
cannot alias those pixels. Dead weak records are pruned and the index has a
named admission limit, including providers that outlive core view leases.

The two native workers also inspect metadata. Decode traffic must grant bounded
metadata turns so a new session can become eligible for admission. Cancelling
the last interest in a remote metadata load must cancel its download; a retired
session cannot occupy both workers until network timeouts. These are image-loader
lifetime rules, not additions to the application effect scheduler.

The initial adapters distinguish owned output from opaque library allocation.
Apple uses ImageIO thumbnails and reserves a conservative reduced-thumbnail
allowance; ImageIO's internal allocation peak is not contractually bounded by
that allowance. Linux's row decoder must account its observed scratch capacities
and avoid a second full-frame conversion allocation. Selected asset resolvers
can materialize encoded bytes before the raster loader sees their length, so
the encoded-size admission limit is not a pre-allocation bound on those reads.
Encoded caches, GPU/framework storage and whole-process footprint are reported
separately from the 32 MiB Exact-owned raster account.

**Shared core verified 2026-09-17:** `exact-raster` passes 18 ownership tests
and strict all-targets Clippy. Tests retain 20 MiB of displayed backing while
two cancelled 6 MiB decodes still own their allocations; cancellation and reset
cannot refund them early. Two unconsumed results in one session do not block
another session's worker admission. Shared native aliases remain charged after
view/cache/session release; destructors reenter the gate outside its locks.
Pinned dedup survives pressure, metadata caps reject replacement atomically,
and blocked budget waiters resume when the last allocation owner drops.
The priority regression also fails before its fix: reclaimable 28 MiB cold
storage must be destroyed before choosing an overscan 4 MiB request ahead of a
visible 8 MiB request. An external owner that prevents actual reclamation stays
charged without making the gate spin. This verifies core policy and ownership;
native codec, provider, GPU and whole-document traversal acceptance remains open.

**Linux adapter verified 2026-09-17:** PNG 0.18.1 decodes rows, including Adam7,
directly into the admitted reduced raster. Original dimensions remain the layout
and object-fit input. The two persistent workers alternate metadata and decode
turns; source identity follows retained native backings through replacement.
Pixel demand uses 128-pixel buckets and a 2,048-pixel maximum axis, with observable
budget waiting and reduced-resolution retry. A selected resolver callback or
blocking filesystem read is not forcibly preemptible. Candidate activation waits
at most one second for metadata preparation; it does not wait for every pixel.

Four instrumented decoder cases record allocation capacity:
4,000×2,000 reduced to 100×50, a wide row, a tall column and Adam7. Observed
allocation-capacity peaks are 456,984 / 1,134,600 / 126,476 / 37,200 bytes against
admitted totals of 1,580,608 / 11,289,008 / 1,049,136 / 1,051,180 bytes. These are
specific decoder allocation measurements, not RSS or a proof for every PNG.
The GPU adapter uploads borrowed charged pixels and registers an empty-blob
texture override. This removes the extra CPU pixel copy and Vello's retained
CPU backing without rebuilding the renderer. A real Metal test releases two
8 MiB cache-only rasters at the next frame and checks odd-stride premultiplied
pixels; GPU textures, atlas and driver staging remain outside the CPU ledger.

The frozen Ubuntu ARM64 snapshot `8d498d5546ce28a562dedfa46e83280a536a332d863bd76e6535bf469ae4f4d7`
passes 111 native tests and strict all-targets Clippy. The GPU-dependent test is
explicitly skipped there and separately passes on Metal. Its gallery executable
is SHA-256 `078fe5dd25f96ba6f67857e524c5dc1a6881604f4d9914c0a7b1a5588f651dbe`.
The 480-source replacement test is an ownership test, not twenty full collection
traversals. Whole-document native traversal/RSS acceptance is still pending.

**Apple adapter verified 2026-09-17:** three Rust ownership/FFI tests and strict
Clippy pass. The real ExactKit/AppKit/CoreGraphics implementation passes 14
XCTest-style methods and 2,347 assertions through a standalone assertion harness;
this machine's Command Line Tools lack XCTest. The debug library and optimized
gallery compile. UIKit source has not been compiled or driven here.

Tests cover original natural dimensions after downsampling, first-frame decoding
and orientation, independently retained CGImage providers, coherent replacement,
240 distinct source replacements, pinned source reuse, reduced-resolution retry
and metadata/decode fairness. Two deliberately stalled loopback HTTP inspections
release the process workers on cancel/reset/pause/shutdown, allowing another
session's local metadata inspection without main-runloop progress. Resolver
replacement cannot reuse old pixels just because its address matches an old key.

An actual AppKit gallery drive displays all six fixture photos and visits top,
interior and end of 25,000-row Arrange and Read collections. It observes 8–13
mounted rows, exact input echo, zero raster refusals and a 24.31 MiB peak managed
ledger. The drive exposed two additional regressions: creation can request an
image before its view is registered, and floating-point rounding could change
an already admitted decode size. Both have failing-before/passing-after tests.
Preserved executable SHA-256 is
`9116f766e9feadb2a8617b5347e1988b708c463ed60900afa5cdf374b2c710b2`;
source identities, logs and screenshots are in `target/apple-raster/`.
Endpoint visits do not prove twenty whole-document traversals or a frame rate.

A subsequent AppKit 1,000-row run completes twenty full sequential traversals
(ten round trips), 9,486 wheels, exact Unicode input and interleaved resizing.
Every traversal independently observes all 1,000 logical cards intersecting the
viewport. Mounted rows stay between 5 and 12 and mapped/kernel nodes between
108 and 200. Sampled RSS ranges from 145.36 to 173.00 MiB, with a 0.90 MiB
managed-raster lifetime peak. The six source images are reused; distinct-source
replacement remains a separate loader test. The earlier attempt is retained:
AppKit corrected a natural-language test token, so the harness now uses a nonce
while preserving exact composed/decomposed Unicode assertions. Full evidence
is `target/native-raster-20260917/macos-accept1k-20260917-015430/`.
Actual Ubuntu ARM64 subsequently completes the same twenty-traversal acceptance
at both 1,000 and 25,000 rows. The larger run issues 232,603 wheel commands,
3,634 exact Unicode nonce input ACKs and 3,635 resize ACKs, including startup.
Each traversal exposes all 25,000 logical rows; all twenty coverage bitmaps were
independently reread and hash-checked. Mounted rows/images remain 6–12 and
live kernel/painted boxes 121–200 across 232,623 geometry observations. These
counts do not cover every native object or retained arena slot.

The 25k run records a 6.01 MiB managed-raster lifetime peak and 104.12 MiB
maximum sampled RSS, also reported as Linux VmHWM. Its 934 sparse raster
probes do not exclude short jobs between samples. Six repeated source images
do not test unique-image churn. Source capture
`8d498d5546ce28a562dedfa46e83280a536a332d863bd76e6535bf469ae4f4d7`
and executable
`078fe5dd25f96ba6f67857e524c5dc1a6881604f4d9914c0a7b1a5588f651dbe`
remain unchanged before/after; evidence is under
`target/native-raster-20260917/linux-accept25k-20/`. Driver assertions check
interaction replies during execution; the complete 25 GB raw transcript was
not reparsed as an independent second validation. Targeted input works while
the existing focus command is unsupported; keyboard-navigation parity remains
open. This CPU Presenter drive proves geometric coverage and observed lifetime
bounds, not physical frame cadence. AppKit's full 25k traversal sweep remains open.

### 6.4 Acceptance and landing

At a fixed viewport and row shape, 1,000 and 25,000 records have the same
window-sized row-instance/kernel/view bound, plus the two declared pins.
Twenty traversals do not accumulate retired rows, row-local slots or
decoded pictures. Heap growth attributable to O(N) data/metadata is
reported separately; remaining growth needs an explanation before landing.
Image counters stay within their byte budget, including in-flight work.

Use the existing tests and agent operations to cover first/last row,
fast scroll in both directions, reorder/delete/prepend, resize, delayed
image sizes, keyboard/end-follow, swipe actions, row focus, selection,
accessibility position/count, and navigation away/back. A focused or
accessibility-targeted row must be brought into the window before being
addressed. Ordinary eager lists remain a parity comparison.

Land each slice with its measured before/after and code/docs together.
The five checks remain five; the host/memory sweep is diagnostic and runs
asynchronously. The work is complete only after Messages passes on web
and physical iOS and the shared fixtures pass on macOS/Linux. A runner-only
number, native cell recycling, or a smooth 25-record demo closes none of
those later slices.

### 6.5 Shared viewport collection implementation, 2026-09-16

`list virtualized=true` opts into the shared runner collection. An ordinary
`each` and `virtualized=false` retain eager behavior unless the list explicitly
selects the fixed/measured row policy of §6.2 with `item-height` or
`estimated-item-height`. Shared collections accept only the estimated hint;
exactly one policy owns the list. The initial supported
shape is a bounded vertical list with one direct keyed `each`, whose body
has one normal-flow element root. Wrap multiple or conditional roots in a
column. Absolute/overlapping rows and alternate container layouts are rejected.
Vertical scroll-container padding is also rejected until the feedback protocol
models content insets. Put top/end spacing inside measured first/last rows;
horizontal padding is represented by the host's actual offered row width.
The compiler keeps one row template; records and compact key/height metadata
remain O(N). Only selected rows own instances, local slots and kernel views.

Before host geometry arrives, at most sixteen rows mount with a provisional
32-point estimate. Actual border-box wrapper heights replace estimates, with
one viewport of overscan on each side. Offset-only feedback performs no key
evaluation or data-source query. Changed membership/order validates all keys;
this is still synchronous O(N) work. Row-local state dies on eviction; durable
drafts and selection belong in parent/keyed data. Visited rows are not cached.

Hosts report the actual nested scrollport, content-relative offset, offered
row width and measured wrappers. Width/content changes issue fresh measurement
epochs. Stale revisions, sequences and epochs cannot overwrite newer geometry.
Anchor corrections are tied to the accepted scroll sequence, preserving a key
and its offset or falling back to surviving neighbors. End-follow is explicit
and only applies when the user was already at the end; native eager end-follow
must not run independently on a virtualized list.

`Runner::collections`, `collections_json`, `collection_feedback` and
`collection_feedback_bytes` are the common host seam. Feedback is versioned
little-endian data; snapshot JSON encodes all u64 generations/sequences as
decimal strings, including values above JavaScript's exact integer range.
Snapshots contain only mounted wrapper metadata, not record text or offscreen
keys. Host adapters bound their feedback work per callback and coalesce pending
work; that count bound is not a time bound or proof of an OS presentation yield.

Messages stress has an opt-in full-history transcript beside its eager and
manual controls. Markdown stress similarly windows complete parsed block lists
with the existing rich-text renderer. Single giant paragraphs/code blocks are
still indivisible. Raster admission, selection spanning unmounted blocks,
physical iOS acceptance and presentation timing remain unfinished. The four
continuous-interaction demonstrations in LLP 1041 §8.5 extend this foundation;
the functional collection implementation alone does not satisfy their motion bar.

Feedback is preflighted before any mutation, including aggregate extent overflow
and foreign row pins. An accepted focus or interaction pin transfers that category
globally; clearing a pin affects only the addressed collection. Earlier owners
are re-realized and revision-bumped before the new owner. Stale feedback cannot
steal or revoke another collection's pin. Virtualized descendants inside a virtual
row template are rejected, including currently empty/inactive branches; an
explicit eager descendant remains allowed. A sheet containing one collection
does not introduce a nested virtual row and remains supported.

Measured zero-height runs are skipped by the height tree rather than flattened
from a potentially huge endpoint hull. Typography invalidation restores estimates,
the reading anchor and actual emitted rows/spacers. Individually valid heights
whose sum would overflow native geometry are rejected without poisoning the
runner. End-follow tolerates at most 0.5 logical units of host geometry rounding,
including the browser's integer scroll range for fractional CSS row extents.

**2026-09-18 (LLP 1027.004 S1, revised after review):** `reachstart` and
`reachend` are argument-free list handlers dispatched after accepted host geometry
commits. Only nonempty virtualized collections qualify: the geometric window
(scrollport plus one viewport of overscan) must contain the first/last supplied
row; bootstrap rows and pins alone never qualify. Edges start armed, disarm on
successful dispatch, and re-arm when the current edge row is outside a nonempty
geometric window whose entire overscan band has current measurements. The existing
height index checks that band in O(log N), without measuring offscreen rows.
Replacement estimates, width/content invalidation and a hidden scrollport cannot
manufacture an exit. Changing endpoint keys does not re-arm them. An empty data
set resets arming for later nonempty data but dispatches no edge itself.
Each edge fires at most once per feedback call. When both qualify, start precedes
end; end runs in the same call only after a pure no-op start (no state change or
request started). Otherwise end remains armed, waits for the first action's async
targets to settle, and gets one bounded follow-up report even if rows and geometry
are unchanged. Continuation tickets keep that wait; settled geometry decides
whether end still qualifies. An all-fitting window therefore becomes idle.
Refused actions remain armed and stop the sequence; hosts re-evaluate once after
deferred data activation. Ordinary action transactions preserve committed
feedback receipts and surface action errors separately from geometry acceptance,
including pin releases. Hosts attach no edge listeners. Offset-only feedback
without a qualifying handler performs no source query or key evaluation. Messages
stress also exercises bounded 200-record answers; its scrollbar spans the resident
window, not the complete history.

### 6.6 Paired runner evidence, 2026-09-16

`bun scripts/metrics.mjs --list-memory --collections --repeats 3 --json`
preserves the earlier eager diagnostic and adds paired fresh processes using one
release executable. This run used Apple M4 (Mac16,10), 10 cores, 16 GiB, macOS
26.2 / Darwin 25.2 arm64, Rust 1.97.0. It is not compared against the older M5
baseline. All eighteen cells passed with source content unchanged before/after
build and measurements; all 234 phase-boundary compiler observations were empty.

Both modes supply distinct complete records and the same fixed 24px text rows,
with one owned slot per row, in an actual 390×800 nested kernel scrollport.
Twenty traversals cover top→bottom→top with viewport-sized steps. Actual wrapper
measurements must converge to the complete N×24 extent. The 25-row case fits
without scrolling. Each cell also exercises typing, row-body updates, replacement,
reorder, leaving and destruction. Values below are medians across three processes;
heap and RSS are sampled after twenty traversals.

| Records / mode | Live rows | Requested heap MiB | Process RSS MiB | Input runner p50 / p95 ms | Body runner p50 / p95 ms |
|---|---:|---:|---:|---:|---:|
| 25 eager | 25 | 0.112 | 3.72 | 0.0306 / 0.0333 | 0.0326 / 0.0422 |
| 25 virtualized | 25 | 0.180 | 4.08 | 0.0467 / 0.0622 | 0.0497 / 0.0539 |
| 1,000 eager | 1,000 | 3.737 | 10.56 | 1.0962 / 1.1572 | 1.2122 / 1.2289 |
| 1,000 virtualized | 67 | 1.416 | 7.06 | 0.1217 / 0.1322 | 0.1336 / 0.1433 |
| 25,000 eager | 25,000 | 105.944 | 128.17 | 29.1057 / 30.9080 | 33.4697 / 33.9556 |
| 25,000 virtualized | 67 | 7.429 | 18.17 | 0.2462 / 0.2543 | 0.2505 / 0.2614 |

At both 1,000 and 25,000, the maximum sampled live kernel count is 207 and arena
capacity 307; settled top has 67 rows / 138 nodes. At 25,000, scroll-feedback
runner p50/p95 is 0.3121/0.3356 ms and layout 0.0368/0.0403 ms. Scroll and
input/body updates evaluate zero keys or source queries. Replacing/reversing the
data keys all N once. Cumulative requested-heap peaks through replacement/reorder
are 217.593 MiB eager and 15.829 MiB virtualized. Leaving releases every row;
source records and reusable kernel capacity remain until runner destruction.

Timings include synchronous action/feedback settlement with diagnostic allocator
accounting. They are elapsed CPU-path measurements, not OS CPU counters or input
to display latency. Requested heap excludes allocator slack and internal realloc
transients; RSS includes diagnostic buffers and is sampled, not continuous peak
or Apple physical footprint. O(N) input and compact metadata remain. No host
presenter, decoded images or first pixel participates, so this establishes no
macOS/Linux/web/phone frame rate or raster budget.

Raw samples, readable report and exact executable are preserved locally in
`target/collection-metrics-postreview/`; the pre-review run remains separately in
`target/collection-metrics/`. Source identity is `bf21e94` plus working edits,
full source SHA-256 `a5daf03adc0797a8dd4afb10f63010ab73b1fe6a6b7a44d26a70cacc2a78c206`;
binary `c14a55bf5407cde3aefb9f76affc085e9cf178744452195393d293677b48e2bb`;
raw JSON `2e2fbc9ff23e1cbe2bfaf11410e7f0fe568e2d17047455a9bcdab1826f577e16`.

Scoped runner/Contract/web/Apple/Linux/stress-data validation passed 427 tests
(four ignored), strict Clippy, formatting, caps and boot. This includes 39 runner
collection tests, 25k zero-height realization, cross-collection pin ownership,
overflow nonmutation, typography anchors and host adapter regressions. A stale
compiled Contract test artifact initially rejected the new list syntax; refreshing
changed Rust source mtimes without changing bytes rebuilt it and the same suite
passed. The cause of that cache mismatch remains unproven. Whole-workspace checks
remain blocked by the unrelated missing lean Hermes producer; workspace formatting
reports sibling Snapback differences, with no Exact2 formatting differences.

Final Ubuntu adapter validation additionally passed all 69 Linux tests and strict
Clippy. It caught two host geometry errors: subtracting a large f64 row top from
an f32 frame invented padding and invalidated corrections; border-box scroll
limits stopped short of the inner viewport's logical end. Linux now reads the
authored origin and uses one virtual-list range for wheel, clamp and correction,
preserving the eager fallback. A 25k fractional-height, bordered regression
requires one wheel, stable sequence and eventual idle refinement.

The final uninstrumented Ubuntu 10k Messages drive reaches every tested endpoint,
including a local echo, with one wheel and two no-wheel observation requests.
The 1 MiB Markdown smoke also passes. The headless carrier pumps during requests;
this does not prove autonomous OS scheduling or presentation. A shared guest Cargo
target had reused a stale runner with frozen-source mtimes: package-scoped clean
forced its recompilation and resolved a later discrepancy without source changes.
Failed and final binaries remain separate. Final source manifest:
`6840b5e9c16c7ed8fd1530cfe67481c88e12bc0f5578d526dc4de6ba08912827`;
evidence: `/tmp/exact2-linux-endfollow-final-6840b5e9/`. This adapter-only follow-up
does not alter the measured runner or replace the paired diagnostic above.

**2026-09-19 Shop consumer correction:** the Apple presenters now exclude shared-collection-owned lists from the earlier item-height `syncLists` callback. Reporting both protocols for the same node sent a collection row tree to `Tree::update_list`, raised `not a windowed list`, and poisoned the Shop iOS runner at startup. The focused `CollectionMacTests.testSharedCollectionResizeUsesOnlyRevisionedFeedback` passes; Shop's web and iOS simulator probes traverse eleven captured feed sections, evict offscreen rows, return to usable top-row links, and play/pause the visible native video. Both probes report three mounted cards at the settled top. This is bounded-view evidence, not a physical-iPhone frame-pacing or startup comparison.

**2026-09-19 tall-card bootstrap estimate:** shared `virtualized=true` lists accept the existing `estimated-item-height` as one positive literal. The runner seeds its height index from that hint and bounds bootstrap by the previous 16 × 32-point provisional budget (at most sixteen rows); measured heights replace the estimate. Invalidated heights revert to the authored estimate. Compiler cases and a 25,000-row integration exercise verify bounded bootstrap, replacement by actual measurements, distant scroll and return without rebuilding the source. Six compiler collection tests, 64 runner collection tests, targeted package tests and clippy pass. Shop's alternating simulator comparison reports median exec-to-first-draw 566.2 → 426.4 ms over five launches per build. Web must also exclude collection snapshot views from legacy list registration: the authored estimate otherwise activates `exact_list` on a shared collection and poisons the runner. The reproduced failure is fixed and Shop's full-feed browser/iOS probes pass. These are consumer-specific measurements, not whole-runtime or physical-phone parity.
