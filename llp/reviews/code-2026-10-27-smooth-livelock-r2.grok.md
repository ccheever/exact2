# Code review: the iOS smooth-correction read-back livelock (SmoothCollectionIOSTests' flake), round 2 of 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `0b54dfb664`.
- **Method:** one brief (sha256 `802311195f62c83e84c3f436e701318a6a62b6620eb664b62d4ea110e3896e3e`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

CLEAN

The reorder predicate keeps the old edge test and adds `PixelGrid.same`. A read-back one ULP under the snapped last pixel — the 5/3 case, `1.6666666666666665` — still has `now < high`, so `canScroll(toward: 1)` becomes false only because of the slop. `testAScrollerAtItsLastPixelReadBackOffTheGridCannotScrollOn` fails on the old predicate. The unwrap fires only after UIKit returns something other than the grid offset that was assigned, and that assigned value is `range.high` (`contentSize` is `bounds + y`, insets are zero). `toward: 1` is the bottom band (`ReorderEdge.direction` returns `+1` in the last 32 points), and `toward: -1` stays true a hundred points above the low edge.

`testAutoscrollPassesAFractionalInnerLimitToItsAncestor` sets the offset to 100 with a 0.3 bottom inset. At scales 2 and 3 the snapped high is 100, so `now < high` is already false. `scroll(by:)` is unchanged. The slop only removes positions the old predicate treated as scrollable, and that test expects false.

On the smooth-correction commit, a one-ULP read-back still rounds to the same device pixel, and 0.01 device pixels is well under one pixel, so `OffsetDriver.frame` no longer restarts the ease. `here == reached` compares two `reachable` results. The reading-anchor checks use that same slop; a real reader move is a pixel or more.
