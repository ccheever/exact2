# LLP 1063: Presence and layout motion — `exit-animation` and `layout-transition`

**Type:** RFC
**Status:** Implemented 2026-09-26 (web, iOS, macOS; Linux layout only, exit refused)
**Systems:** Kernel (style rows, receipt), Motion (`exact-motion`), Contract, Web, Apple, Linux
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26 (gaps closed 2026-09-27: size, windowed rows, the web's
measure, load, ghost and springs, first moves, XCTests)
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
place over 320 ms. A box that grows or shrinks (an accordion opening, a card
whose row left) changes size under the same row. Pressing again creates a new "First" (no exit, no slide:
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
outermost). A `when` arm turning false and an `each` row leaving both arrive
this way, inside a `list` row as anywhere. A windowed list destroys rows that
scroll away as well as rows whose item left the data; the runner's window
(and a virtualized collection) empties the row wrapper's `listItemKey` for
the latter alone, before it detaches it. A wrapper with a key simply goes; an
emptied one leaves as the row, where the window placed it, playing its one
root's `exit-animation` (a row of several roots goes at once).

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
animations end, or at once when its parent is destroyed. On the web it also takes `transition: none` first, so an authored
`transition: all` cannot animate its switch to an absolute box (a transition
still running on it ends where it was going).

**D5 — Native exits run on the engine.** The Apple host withholds the
ghost's and its descendants' destroys, spares its motion node from the
commit's removals, and calls `Engine::play_exit` at the commit's clock. As a browser appends a
list's entry, the exit follows the animations the node already plays, which
keep running (a spinner fades while it spins), and composites over them; an
exit naming keyframes the node already plays is a second entry and starts
now. Only the exit's end is waited for. The web does the same: the exit's
CSS `animation` list is appended to the element's own, and the ghost is
removed when the appended animations end.
`settle_time` includes its end, so `clock settle` waits for it; when the
engine passes the end, one `destroy` of the ghost lets the presenter drop the
whole subtree. The presenter needs one new op (`exit`) and one new `present`
property (`layout`).

**D6 — Layout is FLIP on a presentation offset and scale.** Natively a node
with the row has its laid-out box observed as `Property::Layout` (four
components: origin in its parent, width, height; `Kernel::layout_box`) after
each layout that moved or resized it; the engine's transition rules give
first seen, interruption from the current presentation and springs (settling
per axis in points) for free. The presenter gets `layout` as the shown box's
offset from the laid-out origin and scale of the laid-out size, applied
outermost from the box's top-left corner, so it composes with authored
`translate`/`scale`/`rotate`, a moving parent carries its children (only
relative motion animates), and nothing is laid out per frame. A growing box
therefore grows as a web FLIP of size does: by a scale, its content scaled
with it until it lands (no counter-scale of children). On the web
`presence-glue.js` measures every declaring element before and after each
batch, sub-pixel and transform-free: its bounding rect, its own transforms
taken off through its computed style, its ancestors' scale divided out
(ancestors are assumed unrotated), and plays the difference as additive
(`composite: "add"`) `translate` and `scale` from its origin, residual and
velocity included on interrupt. A spring is lowered by the module per move
from its displacement and velocity in points, on the engine's 240 Hz grid and
rest threshold, so both settle at the same time. A windowed list's row root
is placed in the list content, not its wrapper (which only positions it), so
rows slide when one leaves. A resize takes new boxes without animating
(native and web alike: the web never measures a resize).

**D7 — The web loads it after paint.** Rows travel as custom properties in
the node's `cssText` (`--exact-exit-animation`, `--exact-layout-transition`);
the exit's `@keyframes` rule is sent while its node lives. `glue.js` loads
`presence-glue.js` the first time a batch carries either, so boot is
unchanged (`boot.mjs` counts the same graph). A batch with an exit that
arrives while it loads waits for it, with every batch after it in order
(`presenceLoader`, navigation.js), so no exit is lost. An `exit` op carries
the CSS list it plays: a windowed row's wrapper declares none.

**D8 — Linux is honest.** Layout transitions run (the offset and scale are
painted outermost, as on Apple). Exit animation is refused: the painter draws the
live kernel tree every frame, and keeping a destroyed subtree would need a
retained paint list that does not exist. The node leaves at once and the
journal records `exit-animation: refused on Linux (LLP 1063)` the first time.

**D9 — A row gained is a transition gained.** A node that gains
`layout-transition` in a commit starts from the box it had before that
commit's layout, so its first move animates, as a CSS transition declared in
the same style change runs (Apple and Linux seed the engine before layout; the
web measures a node a batch's `style` op gives the row).

**Not a gap — route and screen pops.** A popped screen leaves under its
platform's transition: UIKit's pop slides the controller away (a button's pop
over a frozen snapshot of its outgoing pixels, an interactive one over live
views), and the web swaps route content as a history navigation. The route
node is the destroyed subtree's root, so its content's exits are nested exits
and do not play; playing them would be invisible under the snapshot or fight
the slide. A route node's own `exit-animation` is the author's way to animate
a leaving screen where the platform does not.

## Verified

- `motion/tests/it/presence.rs`, `kernel/tests/it/presence.rs`,
  `contract/cli/tests/it/presence.rs`, `host/{apple,linux,web}/tests/it/presence.rs`,
  `host/linux/src/paint/presented.rs` (the painted box), and the XCTests
  `PresenceIOSTests`/`PresenceMacTests` (the presenters' `exit`, `destroy`,
  and `layout` ops).
- iOS simulator (UIKit presenter, `scripts/agent.mjs ios`, the same cases):
  at 150 ms the exit plays over the node's still-running spin while the
  accordion card grows from its old size and the node that gained the row
  slides; `clock settle` lands at 720.83 ms, the spring's settle for its 18
  point move, the same as the web's.
- Headless Chrome (an outside app with every case, driven over CDP): an exit
  appended after a spinning node's infinite `animation`, which keeps its time,
  with `transition: all` and no transition started; a spring slide lowered to
  721 ms for 18 points, as `SpringConfig::settle_time` gives natively; an
  accordion card 34 → 154 points shown at 34, 94 at mid-time, 154 at the end; a
  node gaining the row moving 37.39 points (sub-pixel) from its old place; a
  windowed row leaving as its wrapper while the row below slides 40 points;
  the presence module's fetch held while an exit arrives: the batch waits and
  the exit plays when it lands.
- macOS: `PresenceMacTests` only; the app was not driven after these changes.

## Known gaps

- A size change scales its content until it lands (a web FLIP's look); no
  child is counter-scaled.
- Web: the measure assumes unrotated ancestors, and a size change's origin
  ignores an authored `translate` on the same element (the error is that
  translate times the change of scale).
