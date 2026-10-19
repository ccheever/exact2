# Code review: iOS scrollIntoView past the range's end under a collapsing large title (the reveal test's flake), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `9f818d1e9`.
- **Method:** one brief (sha256 `51884fd07c24ca4ccfa7dda513e3425e940dae929693bf6ea53580025507ec79`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; the round-1 fix is right, no regressions on the paths named.
- **Disposition:** nothing to take.

Nothing is blocking. The round-1 fix is right, and the extra pass does not regress the paths you named.

`destination()` in `IntoView.swift` reads the target rect on every call, after `layoutIfNeeded`, so the second alignment uses the row's post-layout frame (including a percentage `scroll-margin` on that size). `projectedRect` and `convert` only read layout attributes. They run a second time only when that scroller's top inset actually changed.

1. **Outer and non-grouped scrollers.** Innermost scroller first, then ancestors. An outer port converts the inner collection's projected rect, so it sees the inner offset already applied. A second convert runs only if that outer view's own top inset changed (`contentInsetAdjustmentBehavior` is `.automatic` only for the bar's collapse scroller and the bottom-edge scroller; other scroll views are `.never`). The second `setContentOffset` happens only when the recomputed point differs.

2. **Held `ElementScrolls` replays.** `flush` calls the same `scrollIntoView`. Once the inset has settled, the next replay's `destination()` matches `contentOffset` and the `guard` skips the write. A replay while a transition is still changing the inset aligns to the inset at that moment. The extra write does not drop the hold (`userScrolled` runs from `scrollViewWillBeginDragging` only).

3. **Keyboard insets.** The keyboard changes the bottom inset. The range clamp already includes `adjustedContentInset.bottom` on the first pass, and again on the second if the top inset also moved. A bottom-only change leaves the offset where it was; keyboard avoidance still does its own one-shot reclamp in `KeyboardAvoidanceIOS.swift`.

4. **A jump that would expand the title.** The clamp never writes above `-adjustedContentInset.top`. `restAtTop` opens a collapsed title by pulling past that edge, in steps. This path stays at the current top, which leaves a collapsed title collapsed. When the inset grows inside `setContentOffset`, UIKit keeps offset+inset fixed and the second `destination()` lands on that same point, so there is no second write and no further growth.

5. **Observers on the second `setContentOffset`.** `scrollViewDidScroll` runs again. `onScrolled` only notes the position (`scrolled` is documented as never dispatched from that call). A `scroll` handler is queued once (`scrollEventQueued`) and sent after this function returns, at the settled offset. `scrollend` resets its 0.1s quiet timer. `ScrollPump.scrolled` refuses re-entry with `inScroll`. There is no third settle, so a later inset change cannot loop back into this function.

6. **Tests.** `reveal(20)` is the unclamped case: UIKit's own offset+inset adjustment already leaves the row on the port top, and the following batch must leave it there. `reveal(30)` is the clamp: `short == 0` is the range end, and it is the assertion that fails if the +31 pt overshoot remains. Dropping `spin(0.2)` removes the fixture timer race; `layoutIfNeeded` does not run the run loop.

Skipping a varying-height test is the right call. The stale-rect window is the synchronous gap inside `destination()`, and this fixture's rows are uniform, so a second read of the same estimated height cannot fail. `GroupedListView.reveal` loops three times because a large estimate error reveals a new unmeasured region; the title correction is about 31 pt and stays on the same cells, which one `layoutIfNeeded` sizes. A test cannot inject a resize strictly between those two reads without a hook, and an end-state test on these rows would pass even if the rect were still captured once.
