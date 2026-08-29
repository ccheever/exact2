# LLP 1010: Scrolling v1 — what scrolls, who scrolls it, as built

**Type:** Spec
**Status:** Review (llp-review, one round on r1, 2026-08-29 at Charlie's request: codex NOT READY (7 MATERIAL, 2 MINOR), grok NOT READY (3 MATERIAL, 5 MINOR) — every finding folded into the code or declared in r2, dispositions in `llp/reviews/1010-scrolling-v1.{codex,grok}.md`; **r2 is unreviewed**. The code was reviewed the same day (`llp/reviews/code-2026-08-29-scrolling.{codex,grok}.md`), likewise folded.)
**Revised:** 2026-08-29 (r2 — the spec-review fold: the overflow computation made symmetric and its `auto` stand-in named; sizing vs contribution stated correctly; the root rule's border-box/min-max deviation declared; `content` floored with end padding; the page extent declared; §4 split into asserted and observed, with the smoke now asserting limits and the root's width; §5 split into built-untested and not built, with the `List` windowing seam stated as insufficient; LLP 1008 §5 pointed here.)
**Systems:** Kernel (node types, overflow rows, layout), Contract (`scroll` tag), Web host, Apple host
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Implementer:** Claude (Fable 5); the macOS behavior landed 2026-08-29 (this document transcribes it)
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
the document as for any page. The host does nothing per frame and knows
nothing about scroll positions.

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

## 5. Not in v1, and built but untested (each declared here)

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
operation, later); windowing for `List` (a plain windowed
list when it misses 60 fps — `rules/NOT-DOING.md` §Components). **The
current seam does not support windowing without extension**: the kernel's
`content` is the extent of the *materialized* children only, the runner
admits only press and change events, and `scrollCommand`/`virtualized`
are inert props. A windowed `List` needs three things this spec does not
have: a logical total extent the host sizes the document to (a spacer the
kernel or the plan declares), window-origin compensation so materialized
rows land at their logical offsets, and a scroll-offset event from the
host to the runner so the window follows the scroll. `List` and
`ScrollView` are today the same thing on every host.
