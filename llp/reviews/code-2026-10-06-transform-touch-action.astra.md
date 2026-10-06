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