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

# Round 3

- **Method:** one brief (sha256 `9b2a309872067f7a3ed314b836db8f210f61ff0de8bfd2b3329494206da9398e`), blind to the other review, on 6ae05e432 with rounds 1 and 2's artifacts. The last round.
- **Verdict:** NOT READY.
- **Disposition:** Taken (ccb38d8df): `ScrollView.gestureRecognizerShouldBegin` now resolves one direction, the velocity or the slop's movement while it is zero, for its touch-action walk and `handsOff` (`ScrollView.admitsPan`), as the photo does; a test asserts the pager admits the very drag the photo yields before any velocity, and refuses the vertical one. The `alwaysBounceVertical` residual is left as noted: a fitting vertical scroller is treated as immovable, as on the web.

I'll review the round-3 touch-action pan change read-only, starting from the earlier findings in this review family and the host diff.Round 2’s two findings are recorded as taken. I’ll read the pan walk, scroll chaining, and the tests against the gesture spec.Round 2’s inset and hit-test findings are in the current walk. The yield still hands the drag to a scroll view that refuses it when UIKit has not produced a velocity yet.[P1] Use the pan's translation when velocity is still zero, in the scroll view that must accept the yield — `host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:75`

A sideways drag on a `pan-x` photo is the case this change exists for, and `transformShouldBegin` already resolves it the way back-swipe does: velocity, or translation when velocity is still `.zero` (`NavigationIOS.swift:641`). It then yields. `ScrollView.gestureRecognizerShouldBegin` still feeds the raw velocity to `allowsTouchPan` and `handsOff` (`ScrollViewIOS.swift:39-52`). For `pan-x`, a zero vector fails the horizontal test and is read as `pan-up`, so the scroll view returns false before `handsOff`. UIKit asks each recognizer once. Both decline, and the pager never receives the drag the photo stepped aside from. The same split breaks chaining: the photo calls `chains` with the translation, while the scroll view never gets there. Resolve one direction with `NavigationRules.popDirection(velocity:travel:)` and pass that to `allowsTouchPan` and `handsOff`. The photo test that sets translation `(12, 2)` and leaves velocity zero only asserts the yield, so it stays green through the dead drag. Assert the pager's `gestureRecognizerShouldBegin` with that same direction as well.

Round 2's two findings are fixed. The range is `contentSize + adjustedContentInset` on both sides minus `bounds`, the same span as `handsOff` (`high - low`), with the `0.5` threshold. The walk is `hitTest(start, with: nil) ?? self`, so a child drawn outside the handle is the hit; `NodeView.hitTest` already walks visible overflow. `chains` is `handsOff`'s answer without the `bounces` write, and `handsOff`'s result is unchanged. An inner scroller that cannot move on the axis is skipped, so an ancestor that can still gets the drag; that inner owner's `touch-action` stays in the intersection. An inner scroller at its edge defers only when `chains` says an ancestor can take that direction, and the walk then stops on the scroller that actually begins, owner included. A refusing owner stops the walk, which matches the outer scroll view refusing on that same node. `pan-left` and `pan-right` stay physical. Left and right insets both add, so RTL does not change the span, and the edge test is the same `contentOffset` geometry `handsOff` already uses. A pan whose pinch is `.began` or `.changed` is still admitted before the walk.

The new tests do fail without this round where the predicate changed. The inset-only pager stays the photo's if range ignores insets. The child with `none` drawn outside the handle yields if the bounds guard is back. At the trailing edge, an outer `none` or `pan-y` still yields to the inner pager if the walk never chains. Outer `auto` at the edge, the mid-pager cases, and a child whose own value is `pan-x` already yielded in round 2, so those assertions pass on the old predicate. The child under the finger is placed at y=220 in a pager 200 points tall with `overflow_y: hidden`, so a real hit never reaches it; the test calls the predicate directly. A vertical scroller whose content fits is still treated as immovable while `alwaysBounceVertical` lets UIKit begin that pan. That residual is unchanged from round 2. The XCTest bundle was not run.

Verdict: NOT READY.
