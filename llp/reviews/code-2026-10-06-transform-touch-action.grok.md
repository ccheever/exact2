# Code review: a photo's pan yields the axes its touch-action names (iOS), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--cwd /tmp/bsky-tt-rv`.
- **Method:** one brief (sha256 `5bf7248fa1bd3ea13e2b309c277faf9e0890234c7d8b0f758922ebf51b35ec75`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 5fc8c2b16 in a detached worktree. The author (Claude, for the Bluesky clone) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r2, 48df72cf4):** No findings to take. Its two noted holes (the drag dying under a refusing descendant or scroller, and stricter-than-web with no scroller) are closed by the r2 change made for the other review.

# Round 1

I'll review this commit against the gesture-precedence spec and the existing iOS pan checks, starting with the review method and the diff.The diff is small. Next I’ll compare it with the spec and the existing pan and scroll checks so the yield logic is judged against the same rules.The pan yield matches the existing layout-pan predicate and the scroll view's axis check. The pinch path is untouched, and the new test fails on the old code.

No findings.

## Checked against the spec

Rule 2 on iOS is already split in two. `ScrollView.gestureRecognizerShouldBegin` (`ScrollViewIOS.swift:44-51`) intersects `touch-action` from the hit view through the scroll container. A recognizer only has to step aside, or that intersection never runs, because a pan that begins cancels the scroll. `transformShouldBegin` (`TransformDragIOS.swift:70-74`) does the same step-aside as `layoutPanRecognizer` (`NodeViewIOS.swift:124-130`): `auto` and `manipulation` keep every drag, an explicit axis that `allowsTouchPan` allows is refused, and a zero velocity falls back to translation and is not refused. Direction signs match the existing mapping (a leftward finger is `pan-right`, LLP 1001). `pan-left` / `pan-right` in the new test agree with `PanReleaseIOSTests`.

Checking only the handle is acceptable. Swipe and reorder also consult their own node; the scroll view is what intersects. The pinch walks ancestors because §2 and §4 are a different question (any node from the handle up excluding `pinch-zoom`). For a pager, the handle's `pan-x` is the restrictive value and ancestors are `auto`, so the two sides agree. The hole that remains is the one layout pan already has: the handle says `pan-x`, but a descendant `none` or the scroller's own `pan-y` makes the scroll view refuse too, and the drag dies.

A handle with no scroll ancestor is stranded on a named axis. `photo()` builds no scroller, and the test expects `pan-x` plus a sideways velocity to return false. On the web the browser does not cancel that touch unless some ancestor can actually scroll that axis, so this is stricter than the web. It is the same contract as an iOS `pan`, which is what the change claims. A photo that should drag on both axes still wants `none` (covered) or `auto` (still kept).

Rule 6 and §4: the pinch still returns `!allowsPinchZoom` before the new check (`TransformDragIOS.swift:69`). A sideways pinch still begins. Its location is the centroid, so the focal-point math still tracks a pinch whose pan partner never begins. A `.failed` pan does not drop a live pinch: `transformDragging` only finishes the hold when neither half is still active (`TransformDragIOS.swift:128-134`). A two-finger drag with no scale change is yielded, so the pager can take it. What remains is a race: if the scroll view's pan recognizes before the pinch, it is not in the simultaneous set and can cancel the pinch. One-finger paging does not have that race.

A zoomed photo set to `none` on the handle keeps both axes; the style is read at `shouldBegin`. Switching it during a gesture that already failed does not revive that gesture (UIKit asks once; LLP 1001 says the same).

Reorder and height only accept a vertical axis, so a sideways yield does not hand them the page. An ancestor `pan` or `swiperight` is different: rule 3 makes it wait for the descendant transform pan to fail (`GesturePrecedenceIOS.swift:40-42`), and this change is the first time that pan fails for a direction. An ancestor whose own `touch-action` still accepts horizontal (a `pan` at `auto`, a `swiperight` at `pan-y`) can then take the drag the scroller was supposed to get. A full-screen pager has no such ancestor. The same hole already exists for a child `pan`.

## Tests

`testThePhotoPanLeavesTheAxesItsTouchActionNamesToThePlatform` fails without the fix. The old line returned true for every eligible pan; the new cases expect false for `pan-x` sideways, `pan-y` down, `pan-x pinch-zoom` sideways, `pan-right` sideways, and the translation fallback.

The matrix covers both axes, `pan-left` / `pan-right`, a `pinch-zoom` token, `none` / `auto` / `manipulation`, a zero vector, and translation when velocity is still zero. It does not build a scroll view, so it shows the pan stepping aside, not the pager beginning. That is the same shape as the layout-pan tests. `pan-up` / `pan-down` are missing; they go through `allowsTouchPan`, already covered for layout pan. No two-touch case and no ancestor `touch-action` case.

Web does not need this predicate: the browser cancels the pointer. macOS trackpad scrolling is zoom-gated (`scale > 1` pans the photo, otherwise the scroll view keeps it, §4), not `touch-action`. Linux still accepts a transform on every axis (`contact.rs` `Candidate.accepts`); that is outside this commit.

## Verdict

READY.

# Round 2

- **Method:** one brief (sha256 `fff5b167cad43749f6986affb97fb485557b78829d041fec9176f45d2bf1cf47`), blind to the other review, on e8eed0a5f with round 1's artifacts.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Both taken in r3 (90c3a1a2a): range counts `adjustedContentInset` (test: inset-only range); the bounds guard is gone, so a child drawn outside the handle is the hit (test). Nested scrollers are now covered too.

I'll review the iOS pan-yield change read-only, starting with the review skill and the round-1 findings for this family.Round 1 of this review had no findings. I’ll read the round-2 pan-yield change and the tests that are supposed to cover it.Round 1 for this family had no findings. The two holes it described are closed for the cases the new tests build: a refusing hit, ancestor, or scroller owner keeps the drag, and a drag with no movable scroller stays the photo's. A pan whose pinch is already `.began` or `.changed` is admitted.

The pager case is right. `platformPans` walks from the hit view through ancestors, stops at the first scroller that can move on the dominant axis, and includes that scroller's owner. An inner scroller that cannot move on the axis is skipped, so an ancestor that can still gets it. A `pan-x` owner on that inner scroller still blocks the ancestor, which matches CSS and `ScrollView.gestureRecognizerShouldBegin`. `pan-left` and `pan-right` stay physical (a leftward finger is `pan-right`), including under RTL. RTL does not change the overflow test; left and right insets add.

[P2] Count insets in the "can move" test — `host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:90`

`room` is `contentSize` minus `bounds`. Everywhere else, scrollable range is `contentSize + adjustedContentInset` on both sides minus `bounds` (`ScrollView.handsOff`, `Presenter.setKeyboardInset`, the keyboard toolbar's `contentInset.bottom`, and a navigation scroller with `contentInsetAdjustmentBehavior = .always`). A page that fits the viewport, with the keyboard open, still scrolls by that bottom inset. A handle whose `touch-action` allows the axis (`pan-y`, `pan-down`, `pan-x pan-y`) takes the drag, and the scroll view's pan begins too. That is the steal this change is meant to stop, limited to inset-only range. A multi-page pager whose content is already wider than its bounds is unaffected.

Use the same span as `handsOff`: `adjustedContentInset` on both sides, threshold `0.5`. Keep it axis-wide, not "room from the current offset": at the edge the scroll view still takes the pan (rubber-band, or `handsOff` to an ancestor). The one-page test sets no inset, so it stays green. Add a pager whose content size equals its bounds and whose `contentInset` (or `adjustedContentInset`) is nonzero, and expect the pan to yield.

[P2] Hit-test a start point that lies outside the handle — `host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:87`

The walk starts at `hitTest` only when `bounds.contains(start)`. A touch on a child drawn outside the handle (`overflow` stays `visible`, and `NodeView.hitTest` is written to hit those children) never reaches that child. The handle's `pan-x` then yields to the pager, while `ScrollView.gestureRecognizerShouldBegin` hit-tests from the scroller, sees the child's `none`, and refuses. Neither gesture begins. That is the descendant-`none` hole from round 1, for a child the new hit test skips.

Drop the `bounds.contains` guard and use `hitTest(start, with: nil) ?? self`. A miss still starts at the handle. `NodeView.hitTest` already returns the overflowing child when there is one.

## Tests

These assertions fail on the round-1 predicate, which stepped aside whenever the handle's own `touch-action` named the axis: `pan-y` downward on a horizontal pager, every case in `testThePhotoKeepsADragNoScrollerWouldTake`, and the `.began` / `.changed` pinch cases. The rest of `testThePhotoPanLeavesThePagerTheAxesItsTouchActionNames` also passed before this fix.

Nothing covers a nonzero inset, a nested scroller (inner axis immovable, outer able to move), or a child outside the handle. `Pan` does not override `location(in:)`, so the translation fallback reconstructs a start point outside the handle and never hit-tests. I did not execute the XCTest bundle.

Residual: `fitScroll` turns on `alwaysBounceVertical` for a non-carousel that scrolls vertically even when the content fits. UIKit will still begin that pan. The predicate treats it as immovable, matching the web and the one-page test, so the photo pan and the rubber-band can both try to begin. The unit test only calls `transformShouldBegin`.

## Verdict

READY WITH CHANGES.
