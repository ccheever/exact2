# Code review: iOS scrollIntoView past the range's end under a collapsing large title (the reveal test's flake), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5ea27cf46`.
- **Method:** one brief (sha256 `afe870206a83c8521df0d311ee26cbdcbd8a0757712cb60cd03c8d999d5e271f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one should-fix: the target's rect is captured before `layoutIfNeeded`, which can size an estimated row; measure it again for the second alignment.
- **Disposition:** taken in `9f818d1e9` (`destination()` measures the rect each call). A varying-height test was not added: the fixture's rows are uniform and a resize exactly between the jump and the settle cannot be staged without a hook (round 2 agreed).

- **SHOULD-FIX — [IntoView.swift:249](/tmp/rv-ios-1/host/apple/Sources/ExactKit/IntoView.swift:249): Recompute the target geometry after layout.** Captured `area` remains valid across an offset/inset change alone, but grouped rows can change their content coordinates when estimated heights become measured heights. The new `layoutIfNeeded()` can perform that sizing; `destination()` then uses the old rectangle and can overwrite UIKit’s corresponding offset adjustment, misaligning the target. The existing [grouped reveal implementation](/tmp/rv-ios-1/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:811) explicitly remeasures for this reason. Refresh the projected rectangle after layout and cover a reveal through rows with differing heights; the new uniform-row tests miss this case.

The clamping policy itself is correct: alignment remains constrained by the scrollable range. [CSSOM View](https://drafts.csswg.org/cssom-view/#scroll-an-element) supports that behavior. Outer scrollers calculate their frames after inner scrolling finishes; ordinary scrollers, page margins, and keyboard insets retain their existing calculations. Held requests obtain fresh geometry on each replay. macOS is unchanged.

The new tests meaningfully distinguish reachable alignment from end clamping. Their explicit batch exercises restoration without yielding to the timer between measurements, removing the identified race. XCTest was not rerun in this read-only worktree; the finding above is from static review.

**Verdict: SHOULD-FIX — keep the clamping correction, but refresh grouped-row geometry after layout.**
