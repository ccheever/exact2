# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** round 2's commit.
- **Method:** one brief (sha256 `b1cf56c436f000b31f1e08c362657611f3effb3f231ef0c2784f41b399099504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (a cancel should take the real hover off).
- **Disposition:** fixed in round 3's commit.

---

Nothing blocking. The round-2 fixes hold: a hold samples `pressPaint.hovered` and drops `overKnown`, so the first `.began` on a recognizer added for the hold is a crossing; a cancel does not change `HeldState`; the look is applied before `lookHovered`, so `letGo` can remove that recognizer and clear `hovered`; `recycle` calls `holdGone`, and `lookAtRest` rejects a view that itself holds hover (every view in the row is checked, so a descendant is included). The callback tests match those hold cases.

1. **should-fix** — `host/apple/Sources/ExactKit/IOS/LookHoverIOS.swift:50`. Inspect turns every recognizer off and straight back on (`InspectIOS.swift:69`). A `.began`/`.changed` hover recognizer is delivered `.cancelled` and then reset to `.possible`, and this branch ignores that. `pressPaint.hovered` stays true. Leaving does not deliver `.ended` from `.possible`, so the `:hover` look stays on. The next `hold()` trusts that flag (`HeldState.swift:175`), takes the pointer to be over, and swallows the next real entry — the miss this commit just removed. On `.cancelled`/`.failed`, call `lookState(hovered: false)` and do not touch `HeldState`. A held look still shows through `holds("hover")`, and `over` is unchanged, so a cancel is still not a crossing.

2. **nit** — `host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:350` and `:498`. Nothing in `HeldStateTests` parks a row, holds hover on a descendant, or asserts the row stays out of the pool and that `recycle` ends the hold. The new callback assertions do not cover that half of the fix.
