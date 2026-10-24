# Code review: the painted popover's default placement and AppKit live margins, round 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `afd265f7b2`.
- **Method:** one brief (sha256 `5d00c62318d2409fd3ed0744072b4fe66a6f026050f471e720cdc53b8c2aed2c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**Position-try for the default — RIGHT.** `painted` (`PositionArea.swift:24-26`) feeds `bottom span-right` (right-to-left: `bottom span-left`) and `flip-block, flip-inline, flip-block flip-inline`. `origin` tries that base, then each fallback in that order (`PositionArea.swift:106-132`). `flip-block` swaps top and bottom margins; `flip-inline` swaps left and right, in the order written (`115-116`). A bottom-right anchor `(380,570,20×30)`, box `120×60`, margins right 5 and bottom 5, layer `400×600`: the base and each single flip miss; both flips land the border box at `(280,510)`. Without the swap that point is `(275,505)`. History (`80-82`, `125-132`) keeps the last fitting index while the area, fallbacks, size, margins, overflow mode, and direction are unchanged, and if a later layout fits nothing it shifts that same option. That is the same helper Chrome already matches in `testSuccessfulChoiceSurvivesResizeAndResetsWithTheStyle`.

**Right-to-left `span-inline-end` — RIGHT.** Inline-end is physical left, so the box’s right edge stays on the anchor’s right edge. Anchor `(200,100,30×30)`, box `120×60`: `(110,130)`. `flip-inline` then negates x from `-1` to `1` (`span-right`).

**No-fit clamp — RIGHT.** An unauthored popover has no `position_type`, so `overflow(of:)` is `.fixed` and the shift pulls every edge (`136-137`, `157-163`). A `300×700` box on the emoji anchor ends at y `0`. `.absolute` still holds only the start edges (the main tests at `PositionAreaTests.swift:148-160`). The chooser does not take this path.

**`.platform` menu point — RIGHT, not a regression.** `popUpPoint` (`MenusMac.swift:443-444`) still calls `of` with the default fallbacks `"none"` and `.platform`. An unauthored menu is area `"none"`, which clamps inside `attempt` (`PositionArea.swift:100-103`) and returns at `126`, before the unshifted `.platform` return at `136`. That unshifted return is only an authored non-`none` area, which is main’s behavior (`PositionAreaTests.swift:161-162`).

**Authored fallbacks and no area — RIGHT.** A missing `position_area` row is nil (only a set row is sent), so `painted` keeps the user-agent area and uses the authored list, including the string `"none"`. `axes("none")` is `(1, 2)` plus the immediate clamp and the skip-fallbacks return (`176`, `100-103`, `126`): D2’s placement, not CSS `position-area: none`, and not flipped in right-to-left. The emoji anchor with fallbacks `"none"` therefore does not flip; it shifts to `(12,400)`. An authored area `"none"` stays on that D2 path (`67`). An authored area with no fallbacks row stays `"none"`, as on main.

**iOS keyboard — RIGHT.** `lift` still shrinks `placement` to `keyboardTop` and the `defer` still clamps the center (`MenusIOS.swift:743-752`). That rect is what `origin` fits and shifts in.

**Tests — RIGHT.** Each new flip expectation fails against the old clamp: unit bottom-left `(12,360)` versus clamped y `400`; right-to-left x `110` versus `200`; Mac bottom-left `(10,210)` versus `(10,250)`; Mac bottom-right `(240,190)` versus `(260,250)`; iOS `maxY <= 740` versus the old clamp’s `maxY == 800`. The numbers match the flips above. `fallbacks: "none"` and `area: "none"` would flip to `(12,360)` if those were conflated with “absent.”

**QUEUE — RIGHT.** Painted placement, stack, light dismiss, and `openTopLayer` are host geometry (`PressFeedback.swift:323`, `OwnersIOS.swift:395`, `SheetsMac.swift:204`), on both Apple hosts. A UIKit popover would take placement and dismissal from that.
