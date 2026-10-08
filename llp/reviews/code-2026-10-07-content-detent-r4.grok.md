I'll review only the tip commit, starting with the project rules, the pitfalls note, and the review this change is meant to fix.The prior review’s three findings are the bar. I’ll inspect only HEAD and check whether the trial layout, invalidation, and tests actually close them.The diff is the kernel trial layout plus the Apple host’s remeasure. Next I’ll read those paths against the real layout, then the tests.The review is ready to write. I'll confirm the line numbers on the three defects before ranking them.Nothing blocks landing. The menu path this commit fixes — a flex column of length-sized rows, remeasured alone at its width — holds, and the new Rust tests lock it. Three should-fixes sit outside that path.

## Should-fix

**1. Viewport units bake the sheet into the trial.** `kernel/src/kernel/geometry.rs:116-127`, `kernel/src/style.rs:170` and `:1268-1274`, `vendor/taffy/src/compute/mod.rs:175-188`.

`fit_content_height` sets the route's height to `auto` and clears min/max width, and it keeps min/max height. Those heights are already points: `taffy_style` resolves `vh`/`svh`/`lvh`/`dvh`/`vmin`/`vmax` through `Dimension::resolve` against `env.viewport_height`, and on iOS that viewport is the sheet (`ExactViewIOS.fit` → `session.resize`). The trial never resets the env. The comment at `geometry.rs:98-99` ("nothing it is placed in constrains it") is false for these units.

A definite min-height at least as large as the content makes Taffy lay the children out a second time at that floor (`compute/mod.rs:178-188`), so `flex-shrink` applies again and the returned height pins to the sheet — the bug this commit exists to fix, back through `min-height: 100vh`. `max-height: 100vh` clamps growth, so a new row does not raise the detent. A child `height: 50vh` makes the next detent half the current one, and the sheet walks down on each resize. With a bottom cover, `emit_fitted` (`host/apple/src/layout.rs:283`) floors the extent at 0, and the resolver treats `height > 0` as the only measured case (`ModalIOS.swift:284`) and jumps to the maximum, then walks down again.

Percentage `min-height: 100%` does not pin: the trial's `MaxContent` offer leaves the parent height indefinite. The fixture menu uses insets (the trial clears them) and no viewport units, so that path is unaffected.

Fix: in the trial, treat height-axis viewport units on the route's min/max and on descendants' block sizes as `auto`, or resolve them against the unclipped screen viewport. Leave them as points of the current detent and the sheet feeds itself.

**2. A padding-only change on a non-scrolling block route never refits.** `host/apple/src/layout.rs:201-207`, `kernel/src/layout/publication.rs:147-164`, `vendor/taffy/src/compute/block.rs:797-804`.

`emit_fitted` runs only for fit-content ancestors of a node in this pass's pending set. Publication puts a node in `updated` when its border box moved, its parent moved, or `arena.content` changed (`publication.rs:163-164`). Content is the scrollable-overflow rect (`publication.rs:118`). A block that is not a scroll container does not add its own end padding to that rect (`block.rs:797-804`).

Scenario: a fit-content route that is a `view` or `box` (`overflow: visible`), stretched with `top/right/bottom/left: 0`, so its border box is the viewport and does not move. `padding-bottom: 10%` and the containing block goes from 400 to 800. Taffy records the node because the layout's padding changed, the border box is unchanged, the overflow bits are unchanged, and padding-bottom does not move the children. Nobody is pending. The sheet stays at 44+40 instead of 44+80. The first frame is fine, because the route itself is pending. A `column` is flex (`contract/lower/src/tags.rs`), and flex content size includes end padding, so the fixture and `percentage_padding_is_of_the_containing_block` (both `display: flex`) do not catch this. Rotation changes the route's width and does refit. `padding-top` moves children and does refit.

Fix: treat a resolved padding or border change as pending, or refit every live fit route the real layout visited. The same gap already leaves a block scroller's `content_size` floor stale; this commit makes it the detent.

**3. Every descendant layout rebuilds the route, and the trial ignores a height transition.** `host/apple/src/layout.rs:201-207` and `:270-282`, `kernel/src/layout.rs:1240`, `kernel/src/layout.rs:529-531`.

Any pending descendant walks up and, once per fit route per pass, `of_subtree` builds a fresh Taffy tree with no reuse. The mirror drops a content op when the height matches, so a stable sheet does not storm `invalidateDetents`. A fit-content sheet that contains a virtualized list with a definite box still lays the whole subtree out again on the main thread every scroll frame, because the windowed rows land in `updated`. A menu of tens of rows is cheap.

A row with `transition: height` from 44 to 200 is worse than cost. The real layout uses the presented sample (`present_heights` writes `style.size.height` on the main tree only). The trial calls `taffy_style`, which is the authored 200. The content op sends 200 on the first frame: the sheet jumps, and the still-short row leaves a gap (a shrinking row is clipped). Later frames match, so the op does not repeat, and the trial still runs every motion frame.

Fix: skip the trial when the changed node sits inside a nested scrollport whose border box did not change, and reuse the last height when width, resolved padding, border, and content signature match. Copy the main tree's presented heights onto the trial nodes.

## Nit

**Same-pass membership and refit disagree.** `host/apple/src/layout.rs:194-207` against `:270`.

Pending keys are sorted by `NodeKey`. A descendant processed before its route can push the route into `refit` while it is still in `fit_routes`; later in the same loop the route is removed because it started scrolling or lost `fit-content`. `emit_fitted` still measures it and overwrites the `content_size` just recorded (cover included, then cover subtracted). One frame. It needs a child key lower than the route's, which happens when an existing node is reparented under a newer route.

Fix: after the loop, drop any `refit` id that is no longer in `fit_routes`.

## What holds

The trial matches the real route for the cases the spec names. Width honors box-sizing; the offer stays the border box. Padding and border are the last layout's resolved points, so a percentage is of the containing block and a host cover is included; `emit_fitted` then subtracts only the bottom edge. Descendant percentage padding is resolved again inside the trial. Length min/max heights are kept. Images go through the measure callback (an unloaded image is 0). A nested scroller keeps its overflow style; under an indefinite height `flex-grow` and percentage heights contribute nothing, which `grows` locks. Exclusions and multi-column fragments stay unsettled, as the comment says.

It does not write the live tree. `compute_mapped` dirties the trial, the trial is dropped, and frames, epoch, receipts, and the main Taffy caches stay put. The geometry test checks that the next real layout is clean.

`order::laid_out`'s new handle closure is behavior-preserving on the main tree (`|c| arena.taffy(c)`). `of_subtree` passes its own map and skips `Text`/`Control` children the same way `rebuild` does. A missed call site would not compile.

A route that scrolls is removed from `fit_routes` and keeps `content_size`, cover included; the resolver subtracts the cover only when `route.scroll != nil`. The second extent for shrinkable rows inside a scrolling route is still deferred, and the tests use `flex-shrink: 0`. That matches the spec.

`rows_that_may_shrink_still_grow_the_extent` (resize to 164, expect 208) and the kernel squeeze assertion fail if the measure reads in-sheet frames. `percentage_padding_is_of_the_containing_block` expects 84 and gets 64 from the old route-width math. `testAFitContentSheetFollowsItsRows` fails if `contentChanged` stops calling `invalidateDetents`. It does not fail if the trial is swapped back for the old child-frame measure: the XCTest window never finishes the presentation transition, the sheet stays zero-sized, `fit()` does not resize the kernel, and the boot viewport stays 402×874, so the rows are never shrunk.

The swizzle is confined to the test. `countingInvalidations` (`ModalDetentIOSTests.swift:112-120`) exchanges `invalidateDetents` with `exactTestInvalidateDetents`, and `defer` swaps it back, including on throw. The method exists only on the test target. While the body runs the exchange is process-global, so it counts every sheet, and a crash or an overlapping call can leave it installed. The success path restores it.
