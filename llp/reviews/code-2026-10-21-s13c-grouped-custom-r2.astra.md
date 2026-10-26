# Code review: iOS grouped lists: a custom row's leading symbol and line as a standard cell's (Harmony), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e7671be379`.
- **Method:** one brief (sha256 `575ebc306a3c59d492d790cc9ad29942a40ed32732b1678d651b7fdf703e7510`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 72465c42f6 (dropped rows, live line, relayout as a cell shows, RTL)

VERDICT: WRONG

1. **should-fix — [GroupedCustomRowsIOS.swift:34](/tmp/iosfix-rv-gcr2/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:34): Marks still outlive removed rows.** `customLooks` is never reconciled when `update` replaces the row model. Cleanup requires another carry, standard-cell configuration, or removal of the entire list. Removing a custom row therefore retains its symbol’s `NodeView`; a surviving image moved elsewhere can keep `listSymbol`. Offscreen transitions also escape cleanup. Reconcile ownership independently of visible-cell configuration.

2. **should-fix — [GroupedCustomRowsIOS.swift:48](/tmp/iosfix-rv-gcr2/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:48): Matching default values does not establish that they were unauthored.** An explicitly authored `object-fit: contain; width: 24px; height: 20px` passes this predicate, then renders enlarged, centred and unclipped instead of contained. The new tests cover `cover` and 32×32, but miss authored values equal to the defaults. Check declaration provenance, including authored dimensions, before granting the list look.

3. **should-fix — [GroupedCustomRowsIOS.swift:33](/tmp/iosfix-rv-gcr2/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:33): Separator changes discovered during scrolling lack a scheduled relayout.** Consider a previously displayed custom row without a leading symbol: scroll it offscreen, change its inset without changing its height, then scroll back. `carry` updates `line` and sets `resized`, but only `mount` consumes that flag. [willDisplay:1331](/tmp/iosfix-rv-gcr2/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1331) requests synchronization only when height changes. The old separator can remain until another batch. Schedule deferred projection synchronization for these line changes.

4. **should-fix — [GroupedCustomRowsIOS.swift:65](/tmp/iosfix-rv-gcr2/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:65): RTL still uses the row’s physical left offset as its right offset.** `carry` places the row at physical `x`; it does not mirror that coordinate. In a 300-point cell, a row at `x=20`, width 280, with right padding 16 produces an inset of 36 here, although its content ends 16 points from the cell’s right edge. Measure the converted content edge from the cell’s right edge; the current formula requires symmetric outer spacing.

No files edited or tests run. Initial Git startup unexpectedly invoked `xcodebuild` for discovery; subsequent reads used the direct Git executable. No build or simulator command was requested.
