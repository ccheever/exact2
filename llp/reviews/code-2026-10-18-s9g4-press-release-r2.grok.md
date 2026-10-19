# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `2ece4991b75b560d626e98edb6a286cd4e29d2ccf87d96ecc9919a33efbb7e3b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the same three; the agent's frozen clock in the tests).
- **Disposition:** fixed in round 2's commit.

---

mits are the original ownership mark and the round-1 lender fix. I'll read both diffs and the press-paint call sites they touch.The round-1 diff drops the self-ownership mark and narrows release. Next I’ll read the paint state machine and the tests that cover lenders, holds, and the frozen clock.The round-1 lender model is right for a background-only press, and in that case the lender’s later release still reaches the descendant. Three holes remain where `inkedBy` stays the lender but a descendant’s own press colour is treated as lent ink. Nothing in `heldHighlight` or the frozen clock adds a further blocking failure: under `agentFreezes`, `paintPress` never sets `releaseHeld` and the release runs in the same turn.

1. **Blocking** — `host/apple/Sources/ExactKit/PressPaint.swift:247` (labels at `:254`). Releasing a descendant that has `press_color` while its lender is still down or `releaseHeld` does not keep the lent colour, and the lender’s later release cannot put it back.

   Ancestor A is down with `press_color` yellow, so B and B’s label show yellow (`inkedBy` A, `rest` white). B is pressed with `press_color` navy and takes the label (`inkBelow`’s new clause). B is released while A is still down or waiting out `PressFeedback.unseen`. The self branch requires `pressOverlay` to have no `text_color`, so it full-clears B to the engine white and clears `inkedBy`. The label is not down and `inkedBy` is B, so it is full-cleared to white too. A is still showing yellow. A’s later `endInked` skips both because `inkedBy` is no longer A, and a colour change can drop them from `inked` entirely when it stops at B.

   The background-only path is fine: `endPressStandIn(only:keepLender:)` leaves the text overlay and `inkedBy`, and A’s release still clears it. `testADescendantsReleaseKeepsAHeldAncestorsColour` only covers that path.

   **Fix:** If the lender is still `down` or `releaseHeld`, strip only this press’s own keys and, when those keys include `text_color`, stand the lender’s current `overlay["text_color"]` back in and keep `inkedBy`. For a label this press took, hand it back the same way (set `inkedBy` to that lender and append it to the lender’s `inked` if it is no longer listed) instead of `endPressStandIn()`.

2. **Blocking** — `host/apple/Sources/ExactKit/PressPaint.swift:216`. `inkBelow` overwrites a descendant that is down with its own `press_color`. The lender’s release and `dropPressPaint` then leave that colour in place.

   `showsOwnTextColor` (`:75`) is “wrote `text_color`, or down with `press_color`”. `inkBelow` only checks `wrote`. After round 1, `inkedBy` stays the lender, so either of these matches a pressed descendant whose saved base equals the inherited colour:

   - `inkedBy.view === self` on the lender’s next press or `press_color` change
   - the new clause `inkedBy != nil && rest["text_color"] == inherited` when a middle press inks

   `standInForPress` then replaces navy with the ancestor’s yellow. `endInked` (`:255`) and `dropPressPaint` (`:429`) skip a down view whose style still has `press_color`, assuming that text is its own. The yellow stays after the lender releases or the view is recycled, and the stale `inkedBy` lets the next press take it again.

   Labels with no press of their own still match the new clause, which is what that clause is for.

   **Fix:** Guard `inkBelow` with `!node.showsOwnTextColor` before the three match arms, and do not recurse into that node (its subtree keeps its press). When `endInked` skips a down `press_color` descendant, clear `inkedBy` if this press is ending so a recycled lender is not still the lender.

3. **Blocking** — `host/apple/Sources/ExactKit/PressPaint.swift:397` (`endInk` at `:287`). A middle node that starts writing `text_color` full-clears a grandchild whose own press is still down.

   A lends yellow to B and to D under B. D is pressed (`press_background` and maybe `press_color`); round 1 does not retarget D, so `inkedBy` stays A. A style batch sets B’s `written` to include `text_color`. B is not down, the overlay still holds A’s text colour, and the `writes` branch calls `endInk(below:by:)` with A. That calls `endPressStandIn()` on every descendant with `inkedBy === A`, with no `down` check. D loses its press background and text while D is still down. HEAD~1’s self-mark hid this, because `endInk` then did not see D as A’s.

   **Fix:** Use the same rule as `endInked`: a down descendant loses only `text_color`, and only when its own press writes none. Leave a down `press_color` node alone. When this branch drops the lent `text_color`, clear this view’s `inkedBy` too.

4. **Should-fix** — `host/apple/tests/ExactKitTests/PressPaintTests.swift:140`. The new tests miss the three cases above, and they assert a wait the frozen clock never starts.

   `pressedInsideTheWait` requires `releaseHeld`. `paintPress` (`PressPaint.swift:128`) sets that only when `!ExactEnv.agentFreezes`. With `EXACT_AGENT=1` and timing other than `platform`, `build.mjs` forwards the environment and the assertion fails; the ancestor has already cleared before the descendant is pressed. The tests also never release a `press_color` descendant under a still-down ancestor, never change the ancestor’s `press_color` or call `dropPressPaint` while that descendant is down, and never write `text_color` on a middle node while a grandchild’s press is down.

   **Fix:** Drive “lender still showing” with `down` (as `testADescendantsReleaseKeepsAHeldAncestorsColour` does) or skip the `releaseHeld` wait when `agentFreezes`. Add the three sequences above.

5. **Nit** — `host/apple/Sources/ExactKit/PressPaint.swift:394` and `:61`. After the `writes` branch removes the lent `text_color`, `inkedBy` stays set until the lender releases. On a view that was never itself pressed, `at` is still 0 and the overlay is empty, so `atRest` is false only because of that pointer. Clearing `inkedBy` in that branch (finding 3) makes `atRest` match a view that shows nobody’s ink.
