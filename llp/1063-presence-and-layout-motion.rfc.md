# LLP 1063: Presence and layout motion — `exit-animation` and `layout-transition`

**Type:** RFC
**Status:** Implemented 2026-09-26 (web, iOS, macOS; Linux layout only, exit refused)
**Systems:** Kernel (style rows, receipt), Motion (`exact-motion`), Contract, Web, Apple, Linux
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002/1003 (motion v1), LLP 1057 (keyframe animation — the grammar reused here), LLP 1041 §8.12 (the numeric-height trial, unchanged), `rules/NOT-DOING.md` §Motion

## Summary

Two things a motion designer reaches for first were missing: a node that
disappears could not animate out (a destroyed node was forgotten at once),
and siblings jumped when content around them changed. This adds both as
style rows, in the LLP 1002 shape: the kernel carries the declaration, the
browser executes on the web, `exact-motion` executes natively under the same
seekable clock.

```
keyframes leave
  to
    opacity=0
    scale=0.96

style Row
  layout-transition="320ms cubic-bezier(.32,.72,0,1)"
  exit-animation="leave 200ms cubic-bezier(.32,.72,0,1) both"

component List
  state shown = true
  action toggle writes shown
    shown = not shown
  view
    column
      button press=toggle
        text "Toggle"
      when shown
        text "First" class=Row
      text "Second" class=Row
```

Pressing Toggle removes "First" from the tree at once; it stays on screen,
fading and shrinking in place for 200 ms, while "Second" slides up into its
place over 320 ms. Pressing again creates a new "First" (no exit, no slide:
first seen) and "Second" slides back down.

## Decisions

**D1 — Two rows, CSS grammar.** `exit-animation` (bit 103, codec
`animations`) is the `animation` shorthand, its `keyframes` resolved at
compile time exactly as LLP 1057 D3 resolves `animation` (same parser, same
lowering, same errors named for the attribute). `layout-transition` (bit 104,
codec `transitions`) is a `transition` shorthand; the last declaration that
names no property (or `all`) governs, so `layout-transition="320ms
cubic-bezier(.32,.72,0,1)"` and `"spring(300, 30, 1)"` both work, and one
naming `opacity` moves nothing. `transition: all` never moves a box: layout
is not a CSS property and is outside `Property::ALL`. *Rejected:* a
`layout` transition-property inside `transition` — `all` would silently start
covering it, changing every app that writes `transition: all`.

**D2 — An exit must end.** The schema marks the row `"ends": true`; the
generator validates it with `Animations::validate_ending`, refusing
`infinite` or `paused` (`AnimationError::Endless`) on both ingress paths, and
the compiler refuses the literal first (`lower-exit-endless`).

**D3 — The kernel chooses who leaves.** `CommitReceipt::exits` lists each
destroyed subtree root that declares the row, with the parent it left from
(a producer detaches with `SetChildren` before it destroys; the kernel
remembers the parent a detached exit-declaring child had) and its row as it
was. Not listed: a node created in the same batch (never presented), a root
or inline run (nowhere to stay), a node whose parent the same batch creates
or destroys (it goes inside that parent; nested exits play only on the
outermost), and any row under a `List` — the window destroys rows that scroll
away and the kernel cannot tell that from a removal. A `when` arm turning
false and an `each` row leaving both arrive this way.

**D4 — The leaving view is a ghost, out of layout.** Siblings take its place
immediately; it keeps its last laid-out box and paints above its old siblings
(web: `position:absolute` at its last offset box, which a positioned element
does by painting order; UIKit/AppKit: brought to front). It receives no input
(`inert` / `isUserInteractionEnabled=false` / `routeInert`), is hidden from
accessibility (`aria-hidden` / `accessibilityElementsHidden` /
`setAccessibilityHidden`), gives up focus, and on the web loses every `id`
attribute in its subtree. On Apple it and its descendants leave the
presenter's maps, so focus-by-id and every other lookup find only the live
node; a node re-created with the same key while the ghost leaves is a new
view with a new id (runner ids never repeat). The ghost is removed when its
animations end, or at once when its parent is destroyed.

**D5 — Native exits run on the engine.** The Apple host withholds the
ghost's and its descendants' destroys, spares its motion node from the
commit's removals, and calls `Engine::restart_animations` (which restarts
even when the exit names the keyframes an entry animation was playing) at the
commit's clock. The exit composites over the node's last transition values;
`settle_time` includes its end, so `clock settle` waits for it; when the
engine passes the end, one `destroy` of the ghost lets the presenter drop the
whole subtree. The presenter needs one new op (`exit`) and one new `present`
property (`layout`).

**D6 — Layout is FLIP on a presentation offset.** Natively a node with the
row has its laid-out origin *in its parent* observed as `Property::Layout`
after each layout that moved it; the engine's transition rules give first
seen, interruption from the current presentation and springs for free. The
presenter gets `shown − laid-out` as an offset added to the view's
translation, outermost, so it composes with authored `translate`/`scale`/
`rotate`, a moving parent carries its children (only relative motion
animates), and nothing is laid out per frame. On the web
`presence-glue.js` measures transform-free offset positions of every
declaring element before and after each batch and plays the difference as an
additive (`composite: "add"`) `translate` animation, residual included on
interrupt; a spring is its unit curve lowered to `linear()` over its settle
time. A resize takes new positions without animating (native and web alike:
the web never measures a resize).

**D7 — The web loads it after paint.** Rows travel as custom properties in
the node's `cssText` (`--exact-exit-animation`, `--exact-layout-transition`);
the exit's `@keyframes` rule is sent while its node lives. `glue.js` loads
`presence-glue.js` the first time a batch carries either, so boot is
unchanged (`boot.mjs` counts the same graph).

**D8 — Linux is honest.** Layout transitions run (the offset is folded into
the painted translation). Exit animation is refused: the painter draws the
live kernel tree every frame, and keeping a destroyed subtree would need a
retained paint list that does not exist. The node leaves at once and the
journal records `exit-animation: refused on Linux (LLP 1063)` the first time.

## Verified

- `motion/tests/it/presence.rs`, `kernel/tests/it/presence.rs`,
  `contract/cli/tests/it/presence.rs`, `host/{apple,linux,web}/tests/it/presence.rs`.
- iOS simulator (UIKit presenter, `scripts/agent.mjs ios` against an outside
  app with the rows): mid-exit screenshots show the ghost fading above the
  sibling sliding into its place; re-adding mid-exit shows the ghost and the
  new node together with one node in the tree; a text wrap slides the node
  below it.
- Headless Chrome (`scripts/agent.mjs web`, same app): the same sequence; the
  ghost is `data-exiting`, `inert`, `aria-hidden`, above its siblings, and the
  siblings carry additive `translate` animations that end at their laid-out
  places.
- macOS (`scripts/agent.mjs macos`, same app): the exit and slide mid-flight.

## Known gaps

- Size does not animate (position only); a growing box snaps to its new size
  while following siblings slide.
- Route/screen pops do not play exits (navigation owns that transition).
- Windowed `list` rows never exit (D3).
- Web: positions are CSSOM offsets (whole pixels) — a sub-pixel move does not
  animate; the first exit can be lost if it happens before the lazy module
  arrives; a ghost keeps authored `transition`s, so `transition: all` can
  animate its switch to an absolute box; the exit is a CSS animation on the
  element, so it replaces the element's own `animation` list.
- Web springs settle at a unit displacement's rest threshold; native settles
  in pixels, so native's tail can be a little longer.
- A node that gains the row by a later style change animates only from its
  second move.
- No UIKit/AppKit XCTest covers the presenter ops; they were verified by
  driving the app, not by a Swift test.
