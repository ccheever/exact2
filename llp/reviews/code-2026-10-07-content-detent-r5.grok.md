I'll review only the tip commit, starting with the project rules, the prior review, and the fit-content change itself.The rules and pitfalls are in. Next I’ll read the prior review and the tip commit so the two judgment calls can be checked against the actual diff.The commit message names the two judgment calls. I’ll read the full diff of the trial layout and the host walk next.The taller-viewport probe is unsound, and it leaves a sheet looping. The host walk does not drop a refit for the cases you named.

## Blocker

**A height that is flat above the current sheet is kept as a length, then flipped back to `auto`, so the detent never settles.** `kernel/src/kernel/geometry.rs:124-136`

`sheet` is one sample, `viewport_height * 2 + 1`. `min`, `max`, `clamp`, `vmin`, and `vmax` are constant on one side of a breakpoint, so that sample often resolves to the same points and the value stays a length. `auto` is the right stand-in for a bare `vh`/`svh`/`lvh`/`dvh` (a resolved length is the sheet; the new test is right). It is the wrong stand-in for a comparison that also has a length: the whole value is replaced, so the length is discarded.

`min-height: max(200px, 80vh)` on the route, one `row` of 44, width 390. At 844, `80vh` is 675, the taller sample still moves, the trial uses `auto`, and the detent goes to 44. At 44, `max(200, 35)` and `max(200, 71)` are both 200, so the trial keeps 200 and the detent goes to 200. At 200, the taller sample is 401 and `80vh` is 321, so it is `auto` again. The stretched route’s frame changes on every resize, so the cache does not stop the loop: 44, then 200, forever. The same loop is `height: 100vmax` (44 against `44 + 390`) and `clamp(100px, 50vh, 400px)` (44 against 100). `min(100vh, 800px)` happens to be stable, because the cap does not push the viewport off the plateau. `viewport_heights_do_not_read_the_sheet` only uses bare `vh`.

Fix: classify the expression, not one taller sample. A bare height-axis unit (`vh`/`svh`/`lvh`/`dvh`, and `vmin`/`vmax`) stays `auto`. For `min`/`max`/`clamp`, re-resolve with those bases at 0 and keep that length (`max(200px, 80vh)` is 200, `clamp(100px, 50vh, 400px)` is 100).

## Should-fix

**`flex-basis`, block margins, and block padding still resolve against the sheet.** `kernel/src/kernel/geometry.rs:127-137`

`derive` only rewrites `height`, `min-height`, and `max-height`. `taffy_style` still bakes every other dimension against `env.viewport_height`, and the route’s own padding is the resolved padding from the last layout. A column child with `flex-basis: 100vh`, or `padding-top: 100vh`, measures at least as tall as the sheet, so the detent stays where it is. The same classification as above belongs on `flex-basis` and on the block-axis margin and padding before they are lowered.

## The other two checks

The host stop does not miss a refit. It breaks only for an ancestor whose specified `height` is `Dimension::Points` and whose border box did not change (`host/apple/src/layout.rs:325-330`). A `100vh` height is `Dimension::Viewport`, so the walk continues, and the trial then treats that height as `auto`. Sticky is a post-layout offset (`kernel/src/kernel/sticky.rs`); it is not in the border box the trial returns. A scroll container’s automatic minimum is 0, and a definite points height is its contribution, so rows inside `scroll height=100` do not change an ancestor’s fit height. Any node outside that scroller is still walked on its own, including a sibling whose frame moves because the scroller’s baseline moved.

The reuse key is the measure’s inputs that can change with no descendant visit: border-box width, the four resolved paddings, the four resolved borders, and the bottom cover (`host/apple/src/layout.rs:347-349`). A stretched route republishes when the viewport height changes, so the key is not what feeds the loop above. A `min-height` or `max-height` edit that does not move the stretched box still keeps the old number; that was already true before this cache.

Yes. One follow-up should replace the probe and extend the same classification past height, min-height, and max-height.
