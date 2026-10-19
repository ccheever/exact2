# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 3: high), sandbox read-only, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `b3f403aa24c71fe70d1c89bccab32eb4928d76d7296e6a2857c78fb50dc29fc7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (a parked descendant keeping its hold; a stale look after removal; a re-hold after a cancel).
- **Disposition:** fixed in round 2's commit.

---

Nothing is blocking, but three should-fix issues remain.

1. **Should-fix — a parked descendant retains its hover hold.** [NodePoolIOS.swift:348](/tmp/rv-g3-2/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:348)  
   Hold hover on a child of a recyclable row while its recognizer is `.possible`, then destroy the row without immediately reusing it. The new allowance permits parking. Only the root’s destroy calls `holdGone`; parking removes descendants from `views` through `recycle()`, which never releases their holds. Their later destroy operations find no view. The hold and its temporary recognizer survive until reuse or eviction, and `gone` is delayed. **Fix:** call `holdGone()` for every parked member before releasing its identity. Test parking a held child without an immediate replacement.

2. **Should-fix — an entry callback restores stale hover after removing its recognizer.** [LookHoverIOS.swift:43](/tmp/rv-g3-2/host/apple/Sources/ExactKit/IOS/LookHoverIOS.swift:43)  
   Hold hover on a styled node with the pointer outside, then remove its hover look while held. On pointer entry, `lookHovered` releases the hold; `letGo` removes the now-unneeded recognizer and clears `hovered`. The callback then executes `lookState(hovered: true)`, undoing that cleanup. Pointer exit is no longer observed. Reintroducing the hover look subsequently paints it even with the pointer outside. **Fix:** update the real hover state before releasing the hold, or prevent the callback from repainting after its recognizer was removed. Test this full style-change sequence.

3. **Should-fix — replacing a hold loses the cancellation baseline.** [HeldState.swift:184](/tmp/rv-g3-2/host/apple/Sources/ExactKit/HeldState.swift:184)  
   After a styled node’s recognizer is cancelled, `overKnown` is false and `hovered` is cleared. Replay or replace its hover hold before the next begin. Because the style retains the recognizer, `lookHoverInstalled` makes the baseline “known outside” again. The next begin releases the hold instead of re-establishing the baseline—even if the pointer remained inside throughout the cancellation. Inspect’s disable/enable path can cause this [cancellation](https://developer.apple.com/documentation/uikit/uigesturerecognizer/isenabled). **Fix:** retain observation validity with the node/recognizer and initialize holds from that validity, not recognizer existence. Add cancellation → re-hold → begin → end coverage.

Direct `retire`/`swap` cleanup and target detachment look correct; Mac behavior remains unchanged and the dead tvOS test is removed. `SaidHover` now exercises callback logic, but its detached instance does not validate installed-recognizer state, removal callbacks, or recycling.

Static review only; no files changed, builds, or tests run.
