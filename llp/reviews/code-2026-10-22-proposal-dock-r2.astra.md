# Code review: the design tool's proposal dock, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `4fb2fcb3bb`. The author (Claude) is not a reviewer.
- **Verdict:** one BLOCKER (docking kept after a write) and four SHOULD-FIXes, all taken post-round in 2a65e23709.

1. **BLOCKER — [app.contract:1135](/tmp/rv-dt-propanel/apps/design-tool/app.contract:1135):** Successful Apply/Keep clears `editKind` but leaves `dockedFor`. Editing the same field again skips scrolling. If it is now near the window’s bottom, the reopened dock covers it. Clear docking when the completed draft closes.

2. **SHOULD-FIX — [app.contract:1394](/tmp/rv-dt-propanel/apps/design-tool/app.contract:1394):** “Back to the package” never scrolls its originating row. A row near the bottom remains underneath the dock. Pass its row identity and scroll it as reset does.

3. **SHOULD-FIX — [app.contract:1384](/tmp/rv-dt-propanel/apps/design-tool/app.contract:1384):** With the app’s design explicitly selected by filename, assigning `file = ""` temporarily removes the proposal subtree (`design.file != file`). The immediate focus command finds no dock, and focus is never restored when it appears.

4. **SHOULD-FIX — [app.contract:1279](/tmp/rv-dt-propanel/apps/design-tool/app.contract:1279):** `cellText`, `partEdit`, and `onlyThisMode` still retain `dockedFor`. Open a param’s branch, edit that same param through its panel, then return directly to the open branch editor: its proposal incorrectly remains docked.

5. **SHOULD-FIX — [app.contract:982](/tmp/rv-dt-propanel/apps/design-tool/app.contract:982):** Once latched, readiness survives a failed re-proposal: “Proposal ready” remains beside an error and disabled Apply. Cancellation also retains the latch, so reopening an identical draft claims readiness before validation. Reset it across draft lifetimes and suppress it on definitive failure.

One blocker remains. No builds or tests run.
