# Code review: the Mac window bar's read set, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `1a975c6c5d`.
- **Method:** one brief (sha256 `4d2357ffa155112d260c7bc2fdb1e4fd472a6124425164ecce81334a44589cdf`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Static review only; Mac tests were not run because this session is read-only and the checkout has no build artifacts. No files changed.

VERDICT: CHANGES

1. **MAJOR — Failed header reads lose their dependencies.** [ToolbarReadsMac.swift:166](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:166)  
   `HeaderRule` walks the header subtree even when it returns `nil`, but the record retains that subtree only on success. For example, remove the heading’s role, then restore it in a subsequent props batch: the first present drops the heading from `reads`, so the restoration is skipped and the window title/items remain absent. Adding a heading inside an existing nested header container has the same problem. Record the candidate header subtree regardless of the rule’s result.

2. **MAJOR — Unsuccessful invocations omit the resolved target.** [ToolbarReadsMac.swift:148](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:148)  
   A header button can name an existing ordinary view through `popovertarget`. `Invocation` reads that view’s props but returns `nil` until it carries `popover`, so the view never enters `reads.nodes`. Adding `popover` without changing its ID is then skipped. If it already has authored `position_area`, the header incorrectly remains projected despite now needing the button’s page box. Record named elements consulted during resolution, including rejected targets.

3. **MAJOR — External accessibility labels are untracked.** [ToolbarReadsMac.swift:143](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:143)  
   The traversal follows invocation references but ignores `accessibilityLabelledBy`. `HeaderSearch.update`, `HeaderSegments.update`, and readout presentation consume `authoredLabel`, which resolves external nodes through `chrome.named`. Changing a referenced label nested in a page row therefore leaves the toolbar’s label stale; navigation inputs remain unchanged. Record these names and the referenced content, including unresolved references that may appear later.

4. **MAJOR — Shadow presentation changes future invalidation decisions.** [ToolbarReadsMac.swift:195](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:195)  
   `sync()` overwrites the live `reads` and `inputs`; the guard restores only `presents`. This can conceal finding 2: first add `popover` to an existing named form with default placement. Its toolbar appearance stays identical, so the shadow passes but records the newly valid target. A later `position_area` change now correctly triggers presentation in tests. In release, neither batch triggers presentation and the header stays stale. Shadow execution must preserve the optimized path’s dependency state.

5. **MAJOR — The shadow comparison omits displayed and accessibility state.** [ToolbarReadsMac.swift:218](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:218)  
   Search contributes only its node ID; segments omit their label and individual enabled states. Custom views also omit their displayed images, and toolbar background tint and accessibility state are absent. Consequently, changing the external search/group label from finding 3 is silently repaired by shadow presentation with **zero mismatches**. Compare these actual outputs; the current comparison does not meet the required hard guard. The comparison code itself is correctly enclosed in `#if DEBUG`.

6. **MAJOR — Skipped batches still perform a whole-page Back lookup.** [ToolbarReadsMac.swift:113](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:113)  
   Every skipped batch calls `currentInputs()` → `canGoBack` → `back` → `backControls`. On a pushed route, `backControls` calls `presenter.carrying("id")`, which sorts and materializes every named view before filtering for the Back name. This adds O(N log N) work for N named page nodes even to geometry-only batches that previously skipped the bar entirely. Resolve the particular name through `chrome.named`, or maintain the navigation output without rescanning the page.
