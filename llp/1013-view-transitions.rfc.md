# LLP 1013: View transitions — shared elements the web's way

**Type:** RFC
**Status:** Draft
**Systems:** Kernel (one style row), Contract (an attribute, an action keyword), Plan (a flag on actions), Runner (the flag on a commit), Web host, Apple host, Motion (the ghost's properties)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Related:** LLP 1002 D1/D2 (one representation, two executors, the browser as oracle) and §4 (layout-affecting transitions not decided), LLP 1003 §9, LLP 1001 §3 (`SetChildren` reparents; roots) and §9 (portals not in v1), LLP 1005 §6 (`when`/`match` arms destroy and create; `each` keeps rows by key), LLP 1007 §3–§4 (springs lowered, the glue, the agent clock seeking `getAnimations()`), LLP 1008 §1 (one `NSView` per node; transforms from presentation values), LLP 1010 (clipping at scroll views), CSS View Transitions Module Level 1, `rules/DEFERRED.md` §Motion

## Summary

The effects people mean by "shared element transition" — a thumbnail that
becomes the hero, a card that expands into a screen, a list that magic-moves
on reorder — are, on the web, **View Transitions**: `view-transition-name` on
an element and `document.startViewTransition()` around the DOM update. The
browser snapshots the named elements before and after, and animates a group
from the old box to the new one with a crossfade, above everything, across
destroy/create. This RFC proposes exactly that model for Exact: a name row the
host matches at a commit boundary, the browser doing it natively on the web,
and the Apple host doing what the browser does — snapshot, ghost, animate —
under `exact-motion` and the seekable clock, held to the browser by fixture.
It is the LLP 1002 D2 shape again, and it needs no reparenting in the plan
model at all.

## Motivation

The kernel already has what a React-Native-style shared element library would
need: `SetChildren` under a new parent reparents in one batch with identity
kept (`kernel/src/txn.rs`, LLP 1001 §3); several roots (`AttachRoot`), so an
overlay is a root; `translate`/`scale`/`rotate`/`opacity` are paint rows, so a
FLIP never re-runs layout; absolute frames published after every pass
(`kernel/src/layout.rs`); presenters that keep element identity across a
move (`glue.js` `children`, `Presenter.swift` `children`). Nothing structural
is in the way.

What is in the way is above the kernel, and reparenting would not fix it:

1. **Arms destroy and create.** A `when`/`match` flip tears down the old arm
   and realizes the new one (`runner/src/instance.rs`, `RegionInst::switch`).
   The list's thumbnail and the detail's hero are two nodes with no relation.
   Identity across arms has to be *named*; it is not a node.
2. **The web has no kernel layout.** The wasm carries no Taffy; geometry is the
   browser's (`glue.js` `layout` reads `getBoundingClientRect`). A runner that
   computed "first minus last" from kernel frames would work on macOS and be
   blind on the web. So the transition is the host's job — and on the web the
   host is the browser.
3. **Clipping.** `overflow: hidden` clips (`clipsToBounds`, LLP 1010) and scroll
   views clip; a node in flight across a scroll view is cut off unless something
   lives above the content for the duration.
4. **Size.** A hero changes width and height, and layout-affecting transitions
   are out (LLP 1002 §4, gated on incremental relayout). Scale-as-size, the RN
   habit, stretches text and corners.
5. On the web, hold-then-transition needs a style flush between two writes; the
   glue sets whole `cssText`, and two commits in one frame collapse into no
   transition.

View Transitions answer all five: identity by name (1); the host measures, and
on the web the host is the browser (2); the animation runs in a layer above the
page (3); the animated thing is a snapshot with no children, so its size can
animate without touching the kernel's layout (4); the browser owns the flush (5).

## Design

**D1 — The representation is CSS's.** One new style row, `view_transition_name`
(`kernel/tables/schema.json`): codec `u16`, the `font_family` precedent — the
producer interns the author's string, the kernel and hosts carry a number, and
`0` is CSS's `none`. The web host lowers it to `view-transition-name:v<n>`; the
name only has to be unique and stable within the document, and nobody reads it.
In Contract it is an attribute on any node, `viewTransitionName=\`station-${s.id}\``.
Not a prop: it is a CSS property, so it is a row (`rules/RULES.md` §Scope).

**D2 — The action is the boundary.** On the web a view transition is started
by script, not by the browser noticing a change; the author decides which update
is one. The Contract analog is a keyword on an action — `action selectStation(id)
transition writes …` — that the compiler carries as a flag on the plan's action
row and the runner puts on the commit it produces. A commit without the flag
cuts, as it does today; the one-second `tick` never snapshots the page. (The
alternative, "any commit that creates or destroys a named node", is the
browser's cross-document `@view-transition { navigation: auto }` and is the
open question below.)

**D3 — Two executors; the browser is the oracle.** The web host applies a
flagged batch inside `document.startViewTransition(() => apply(batch))` and
does nothing else: the browser snapshots, pairs by name, animates
`::view-transition-group` (box: `0.25s cubic-bezier(0.4, 0, 0.2, 1)`, the UA
stylesheet) and crossfades `::view-transition-old/new`; a name present only
before or only after fades out or in. The page stylesheet sets
`:root { view-transition-name: none }`: a stylesheet choice, as a reset is, so
only named nodes animate and the whole-page crossfade is opt-in by naming the
root (its native cost is a whole-document capture, ~5 ms on macOS by the canvas
probe in `QUEUE.md`; measure before turning it on).

Every other host does what the browser does, with what it already has — kernel
frames and `exact-motion`: on a flagged commit, before apply, record every
named node's absolute frame and a pixel snapshot (`cacheDisplay` on AppKit);
apply and lay out; pair old and new by name; for each pair put a **ghost** — a
host-owned layer above the content, never a kernel node, like scroll offset is
host state — at the old box and transition it to the new box with the old
snapshot fading out and the new fading in, on the browser's timing. The ghost's
`translate` and `opacity` are engine properties today; its size is the one new
thing (D4). Every ghost runs under the engine, so `settle_time()` covers it, the
agent's `clock` seeks it (LLP 1012), and the presenter's motion frame paints it
like any presentation value.

**D4 — The ghost's size is a motion property, not a layout transition.** The
ghost has no children and no place in the kernel's tree, so animating its width
and height is two more closed-form properties on the engine — the `Custom(u16)`
extension LLP 1009 §4 deferred, or two named ones — and never a relayout. LLP
1002 §4's gate on layout-affecting transitions stands untouched: no kernel row
animates. Scale would work for a constant aspect ratio and is wrong otherwise;
the browser animates the box, so do we.

**D5 — Held by fixture.** The parity harness (LLP 1007 §5) gains a view-transition
fixture: the browser records the group's box at sampled times through
`document.getAnimations()` on the pseudo-elements; the native ghost matches
within the same 1e-3 band. `glue.js` `seek` already walks `getAnimations()`, so
the agent clock should seek view-transition animations with no change; the
fixture is the check.

**What this is not.** Reparenting a *live* node — a focused input, a playing
surface, a GPU canvas keeping its device — is a different, rarer need: the
kernel does it now; the missing piece is a plan-level portal (LLP 1001 §9,
awaiting a consumer) and `Element.moveBefore()` on the web, which preserves
state where `insertBefore` resets it. Not this RFC. Interactive (scrubbed,
drag-to-dismiss) transitions are also later: the seek is the mechanism on both
executors, but the gesture side is LLP 1002 D4's.

## Costs

One row in `schema.json`; an attribute and an action keyword in the compiler
(LLP 1006's tag table); a flag through the plan and the runner's commit; in the
web host, one `startViewTransition` wrapper and one `:root` rule; in the Apple
host, snapshot + ghost + pairing, a few hundred lines; two engine properties;
one fixture. A lane the size of scrolling (LLP 1010). Support: Chrome 111+,
Safari 18+, Firefox 144+; where absent, `startViewTransition` is a plain apply
and the commit cuts — the same page, no motion bytes either way.

`rules/DEFERRED.md` names nothing this crosses: layout transitions stay out
(D4), `@keyframes` stays out (the browser's keyframes are the browser's), and
the ghost is host-owned. Nothing has to come off the doing-list; if one does,
the candidate is the RN-style reparent-to-overlay this RFC declines to build.

## Open questions

- **Boundary: action keyword (D2) or region flip?** The keyword is the web's
  same-document model and keeps ticks out. Flagging every `when`/`match` flip
  instead is the cross-document model and needs no authoring, but a flip caused
  by a timer would snapshot the page. Recommendation: the keyword.
- **Root crossfade default.** The browser's default is on; D3 turns it off in
  the page stylesheet until the native whole-window capture is measured.
- **Engine extension shape.** `Custom(u16)` (LLP 1009 §4) or two named
  properties `width`/`height` restricted to ghosts. Decide in the spec.
- **Clipping the ghost.** The browser clips `::view-transition-group` to nothing
  and `::view-transition-image-pair` to `overflow: clip` by default; the native
  ghost follows, and a node whose old box was partly scrolled out of view
  snapshots only what was visible. Confirm against the browser in the fixture.
