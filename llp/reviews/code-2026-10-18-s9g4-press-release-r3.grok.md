# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `a2a935f226c37f0d276713b6a540c5d2bdfce7d2706b87673101e96268eb300a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the stale lender; the !down clause; a style write dropping a lent colour; duplicates).
- **Disposition:** fixed in round 3's commit.

---

wo release and press-order holes still leave the wrong text colour, and a style or hover refresh while a nested background press is down strips a colour the ancestor is still showing.

1. **Blocking.** `host/apple/Sources/ExactKit/PressPaint.swift:246` (clear at `:259`). A press that ends asks `inkedBy`, which stays the older lender, because a closer press never claims a node that is already showing its own press colour, and `endLent` then clears `inkedBy` when that closer press ends. `inkAgain` on the older lender stops at the closer press (`showsOwnTextColor`), so the node just released is left on its engine colour.

   Press A (yellow), C (blue), then B (green), with A > B > C, each with `press_color`. Release C while A and B are still down. C, and the label C coloured, turn white. B is still green, so both should be green. The same white shows up in the other order: press A, B, then C, release B while C is still down (that only clears C’s `inkedBy`), then release C while A is still down. C turns white instead of yellow. A quick release does not change this: during the wait `down` is still true, so the same lender is chosen. `testAMiddlePressLeavesAPressedDescendantsOwnColour` (`PressPaintTests.swift:215`) stops at B’s press and never releases C.

   **Fix:** after the stand-ins are cleared, walk superviews from the parent and `inkAgain` the nearest ancestor that is still `down` or `releaseHeld` and whose `pressOverlay` has `text_color`. Do not call `inkAgain` on `inkedBy`.

2. **Blocking.** `host/apple/Sources/ExactKit/PressPaint.swift:218`. The third clause of `inkBelow` requires `!down`. `showsOwnTextColor` already stops a node that is down with its own `press_color`. The extra check also skips a node that is down with only a background or opacity press, so a closer press never takes it.

   Press A (yellow). Press C with only `press_background_color` (text stays yellow, `inkedBy` stays A). Press B with `press_color` green. C’s text stays yellow. It should be green. Then release A, or change A’s `press_color`: C is still in A’s `inked` list and still `inkedBy` A, so `endLent` puts C’s text back to white while B is still green.

   **Fix:** drop `&& !node.pressPaint.down` from that clause. Add the press order above, including A’s release afterward.

3. **Blocking.** `host/apple/Sources/ExactKit/PressPaint.swift:394` and the wipe at `:408`. While a node is down, every `presented` style (and `relook`) rebuilds the overlay with `pressOverlay`, which only copies `press_color` and `press_background_color`. A lent text colour is not that. The inequality fires, `endInked([])` still runs the ancestor’s `inkAgain`, and lines 408–413 then delete `rest["text_color"]` and store an overlay with no text colour. The style returned to `applyStyle` is the engine text colour.

   A is down, yellow. B is down with only `press_background_color`, so B’s text is yellow and `overlay` holds both the background and the lent text. Any later style write to B, or a hover that calls `relook`, turns B’s text white (or whatever the engine colour is on that write) while A is still yellow. B’s label stays yellow. `rest` for the text colour is gone, so the engine colour from that write is not what a later release restores until something stands in again.

   **Fix:** if this press has no `press_color` and the overlay text colour is lent, leave that overlay entry and `rest` alone. If this press’s own `press_color` was removed, drop it and only then re-ink from the nearest lender in finding 1, and keep that colour in the style this function returns. `inkAgain` has to run after `showsOwnTextColor` is false for the new style, not before line 408 undoes it.

4. **Should-fix.** `host/apple/Sources/ExactKit/PressPaint.swift:250` and `:263`. `inkAgain` always appends, and `endInked` calls it even when `views` is empty. Releasing a nested press, recycling a label A has coloured (`dropPressPaint` at `:438` with an empty `inked`), or the empty `endInked` in finding 3 all append another copy of the whole subtree. A second `endLent` no-ops because `inkedBy` no longer matches, so the release colour itself does not flip twice. The list grows for the life of the press and keeps recycled views. `inkAgain` also runs before `rebind`’s new style is applied, so the recycled view still comes back in its engine colour while the lender is down (`presented` returns early: not down, overlay empty).

   **Fix:** rebuild `inked` as `[self]` plus one `inkBelow`, instead of appending. Call `inkAgain` only when this press actually ended or a real descendant was given back. Re-ink after the recycled view’s new style is applied.

`PressPaint.atRest` (`PressPaint.swift:61`) still names every field, including `inkedBy`, and a default `PressPaint` is at rest. Nothing in this diff leaves a dirty paint that `atRest` would treat as clear. After a normal press, `at` and `ease` stay non-zero, which is unchanged and does not affect these colours.
