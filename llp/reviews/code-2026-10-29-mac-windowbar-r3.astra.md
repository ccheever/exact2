# Code review: the Mac window bar's read set, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `8915e61041`.
- **Method:** one brief (sha256 `508423039276796f4a1d9d9dd9caa2f1e4b1d3d7377425030a06bf0abe9d9a8f`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Static review only: the read-only sandbox prevented executing the Swift tests.

VERDICT: CHANGES

1. **MAJOR — Secondary navigation can still invalidate an external alert target without presenting the bar.** [ToolbarReadsMac.swift:149](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:149)

   Give a projected header button an external `alertdialog` popover under secondary root 80, whose children are 81 (`navigationKey=x`) and 82 (`navigationKey=y`), initially selecting `y`. Put the popover under 82 with explanatory text, one enabled action, one cancel, no field, and `position_area=top`. Initially, `alerts.plan` succeeds, so `opensFromTheBar` permits projection despite that placement.

   Now change only 81’s key to `y`. Navigation hides and inerts 82, making `alerts.plan` fail; the authored placement then requires keeping the header in the page. But 81 was never recorded, and `BarInputs` tracks neither this target’s gates nor its secondary root’s selection. Both predicates remain false and the header stays incorrectly projected. The added carrier-visibility test does not cover this dependency.

2. **MAJOR — The shadow still silently repairs projection visibility.** [ToolbarReadsMac.swift:249](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:249)

   Adapt the secondary-root fixture so route 82 itself carries `toolbarPlacement=window` and `accessibilityRole=toolbar`, with its command button directly beneath it. After attachment, projection hides 82. Apply an unrelated body-text `.props` update: `NavigationHost.gate` unconditionally unhides the selected route. `concerns` misses the unrelated update, while the carrier input remains unchanged because `visible` deliberately ignores projection hiding.

   With the shadow off, 82 remains unhidden. With it on, `sync()` hides 82 again, but `shadowState` compares only the `projected` flag and item/header metadata—not actual projection-node visibility. It therefore records **zero mismatches**, and only `reads`/`inputs` are restored. Guarded tests can pass while release presentation differs. The existing restoration test checks the record, not the visible state.
