# Code review: a photo's pan yields the axes its touch-action names (iOS), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/bsky-tt-rv`.
- **Method:** one brief (sha256 `5bf7248fa1bd3ea13e2b309c277faf9e0890234c7d8b0f758922ebf51b35ec75`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 5fc8c2b16 in a detached worktree. The author (Claude, for the Bluesky clone) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r2, 48df72cf4):** Both taken. 1: the pan yields only when a scroller that can move on that axis (overflow scroll or auto, content past its bounds) would take it, with touch-action intersected from the node hit at the touch's start up through that scroller and its owner, as `ScrollView.gestureRecognizerShouldBegin` does; a refusing ancestor, a refusing node under the finger, a one-page pager and no pager keep the drag the photo's (test: each case). 2: a pan whose binding's pinch has begun or is changing is admitted whatever its direction (test: alone refused, with the pinch begun and changing admitted).

# Round 1

1. **P2 — Yielding ignores whether scrolling is actually permitted.** [TransformDragIOS.swift:74](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:74)

   A `pan-x` handle beneath a `touch-action="none"` or `pan-y` ancestor now rejects a horizontal drag. `ScrollView.gestureRecognizerShouldBegin` rejects it too, because that code intersects the ancestor values. Neither gesture begins. A restrictive descendant beneath the touch has the same problem. Matching `layoutPan` copies its limitation; it does not satisfy rule 2 or the [CSS intersection requirement](https://www.w3.org/TR/pointerevents3/#determining-supported-direct-manipulation-behavior).

   The rejection also occurs without a scroll ancestor to receive the drag. **Fix:** yield only when an eligible enclosing scroller can claim the direction, using the hit-to-container intersection, including the container’s owning node. Add restrictive-ancestor, restrictive-descendant, and no-scroll-recipient cases.

2. **P2 — An active pinch can lose its companion pan.** [TransformDragIOS.swift:71](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:71)

   On a `pan-x` handle, begin an app pinch, then move its centroid sideways far enough for the pan to attempt recognition. This check rejects the pan even though the binding’s pinch already owns the contact. Simultaneous-recognition permission cannot rescue a recognizer refused by `shouldBegin`.

   The pinch still follows its centroid while both fingers move, but the failed pan cannot provide §4’s continuation after one finger lifts. Changing touch-action to `none` cannot revive that failed recognizer for the existing contact. **Fix:** admit the companion pan when its binding’s pinch is already `.began` or `.changed`, preserving the existing pair and single release. Test pinch-first recognition, sideways movement, lifting one finger, and continued dragging.

The new test **would fail against the parent implementation**: four velocity cases and the horizontal translation fallback expect `false`, whereas the old predicate accepts every eligible transform pan. It covers the basic direction decision well, but calls the helper directly with a replacement recognizer; it proves neither pager takeover nor the pinch lifecycle. Add a scrolling fixture and the cases above.

The zero-direction fallback and `auto`/`manipulation` preservation match `layoutPan`. Setting the handle to `none` before a new contact admits every direction. Reorder, swipe, height, and descendant failure relationships remain unchanged; rule 4’s existing UIKit tie-order limitation is not introduced here. macOS needs no equivalent touch-action gate; browser touch tests would provide useful parity evidence.

XCTest was not run: its runner requires writes unavailable in this read-only checkout. No files were edited.

**Verdict: NOT READY.**
# Round 2

- **Method:** one brief (sha256 `fff5b167cad43749f6986affb97fb485557b78829d041fec9176f45d2bf1cf47`), blind to the other review, on e8eed0a5f with round 1's artifacts.
- **Verdict:** NOT READY.
- **Disposition:** All three taken in r3 (90c3a1a2a). 1: a scroller at its edge that chains (`ScrollView.chains`, `handsOff` without its `bounces` write) passes the intersection on to the scroller it chains to, owner included (test: nested pagers, outer auto/none/pan-y, at the edge and mid-pager). 2: the walk starts at `hitTest(start)` with no bounds guard, so an overflowing child counts (test: a pointer-observing child below the handle). 3: range counts `adjustedContentInset` on both sides (test: a one-page pager with a leading inset). The press fixture is now a `pointerdown` observer and the test pan has a location.

1. **P2 — A scroller that will hand off the drag is counted as its recipient.** [TransformDragIOS.swift:91](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:91)

   With an inner horizontal scroller at its trailing edge, an outer horizontal scroller with remaining travel, and `touch-action="none"` on the outer owner, a leftward drag has no winner. `platformPans` stops at the inner scroller and rejects the photo pan. The inner scroller rejects through [`handsOff`](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/ScrollViewIOS.swift:52); the outer scroller rejects its touch-action intersection.

   **Fix:** account for the existing handoff decision and continue the intersection toward the actual recipient when the inner scroller yields. Add this nested-edge case, alongside an accepting outer scroller.

2. **P2 — The bounds check skips valid hits on overflowing descendants.** [TransformDragIOS.swift:87](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:87)

   `NodeView.hitTest` deliberately supports descendants outside their parent’s bounds under visible overflow. This guard instead starts the walk at the handle. A touch on an overflowing descendant with `touch-action="none"` beneath a `pan-x` handle therefore rejects the photo pan, while the enclosing scroller’s hit test finds that descendant and rejects scrolling. This loses the [hit-to-container intersection](https://www.w3.org/TR/pointerevents3/#determining-supported-direct-manipulation-behavior).

   **Fix:** let `hitTest` handle overflow without the preliminary bounds check, or retain the original touch target. Test an overflowing, hit-testable descendant without a press handler.

3. **P2 — “Room” excludes inset-created scroll range.** [TransformDragIOS.swift:90](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:90)

   A scroller with `contentSize.height == bounds.height` and a positive bottom inset can scroll, but this calculation treats it as immovable. Exact creates such insets for the keyboard and automatic navigation-bar adjustment. A `pan-y` photo consequently remains eligible to take scrolling that should belong to that scroller.

   **Fix:** calculate the range using `adjustedContentInset`, consistent with [`ScrollView.handsOff`](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/ScrollViewIOS.swift:73): horizontal extent includes left/right insets; vertical extent includes top/bottom. Add inset-only range tests on both axes.

Round 1’s ordinary restrictive-ancestor, no-scroller, and active-pinch cases are addressed. The first finding remains partially unresolved through the cases above. The walk correctly continues past a scroller without range on the requested axis; I found no separate RTL-specific defect in the extent calculation.

The new tests **would fail against round 1’s implementation**, by inspection: the no-recipient assertions and active-pinch assertions reverse its results. Coverage still has limitations:

- The [restrictive-descendant fixture](/tmp/bsky-tt-rv/host/apple/tests/ExactKitTests/GesturePrecedenceIOSTests.swift:205) uses `press`, which makes production `shouldReceive` refuse the transform before this predicate runs. Use a pointer observer instead.
- Replacement pans provide no controlled location, so the tests do not verify start-position hit testing.
- Pinch admission is tested; finger-lift continuation and a single release are not.

XCTest was not run because its build requires writes unavailable in this read-only session. No files were changed.

**Verdict: NOT READY.**
# Round 3

- **Method:** one brief (sha256 `9b2a309872067f7a3ed314b836db8f210f61ff0de8bfd2b3329494206da9398e`), blind to the other review, on 6ae05e432 with rounds 1 and 2's artifacts. The last round.
- **Verdict:** NOT READY.
- **Disposition:** Both taken (ccb38d8df). 1: a scroller with `isScrollEnabled` off is passed over (test: a disabled pager alone keeps the drag the photo's; with an enabled outer pager the photo yields to it). 2: the photo is 150 points tall inside the 200-point pager, the child under the finger hangs inside the pager and a window-level hit test is asserted to reach it; the nested test's photo is the inner pager's second page, the one in view.

1. **P2 — A disabled scroller is still counted as a recipient.** [TransformDragIOS.swift:95](/tmp/bsky-tt-rv/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:95)

   A `pan-x` photo inside a pager with horizontal range still rejects its pan when `pager.isScrollEnabled == false`. With no other horizontal scroller, neither gesture takes the drag. App hooks can set this property; disabling it prevents UIKit scrolling. [Apple documentation](https://developer.apple.com/documentation/uikit/uiscrollview/isscrollenabled?changes=_8%2C_8)

   **Fix:** require `scroll.isScrollEnabled` before counting the scroller as a recipient, otherwise continue the intersection toward an enabled ancestor. Add disabled-pager cases both with and without an enabled outer scroller.

2. **P3 — Regression fixtures use touch positions that cannot reach the photo.** [GesturePrecedenceIOSTests.swift:210](/tmp/bsky-tt-rv/host/apple/tests/ExactKitTests/GesturePrecedenceIOSTests.swift:210)

   The overflowing child and touch at `y = 220` lie outside the 200-point-high pager, whose vertical overflow is hidden. Calling the handle’s predicate directly bypasses that clipping. Similarly, the nested-edge test at line 307 scrolls the sole photo, positioned at `x = 0`, entirely outside the viewport with `contentOffset.x = 300`.

   **Fix:** make the handle shorter than its enclosing viewport so the overflowing child remains visible; position the nested test’s photo on the visible page. Assert that window-level hit testing reaches the intended target before checking admission.

The earlier implementation findings are resolved: overflowing descendants participate, adjusted insets contribute to range, chaining extends the intersection through the receiving scroller’s owner, and an active pinch admits its companion pan. The walk correctly continues past scrollers without range on the requested axis. I found no additional RTL-specific defect; the extent calculation uses physical dimensions and both physical insets. Extracting `chains` preserves `handsOff`’s bounce update.

**The round-3 tests would fail against round 2 by inspection:** the restrictive overflowing descendant, inset-only range, and restrictive outer-owner edge cases reverse the old results. They remain predicate tests; they do not establish actual pager takeover or finger-lift continuation.

XCTest was not run because its build and simulator require writes unavailable in this read-only session. No files were changed.

**Verdict: NOT READY.**