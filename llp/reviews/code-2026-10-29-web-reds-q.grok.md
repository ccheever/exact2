# Code review: the six host/web tests that fell behind their code (main red), quick review (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `d027759ab6`.
- **Method:** one brief (sha256 `54491b3159a75fa92a4b1a0ef9c3e9f02223d673545983b7c941eaa111d8e50f`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

RIGHT

The six test updates follow the current code, and none of them drops or loosens an assertion to hide a product bug.

- `applyBatch` reads `orientationLock` and calls `holdOrientation` on every batch. Both glue fixtures now supply those bindings, the same way they already supply the wake-lock pair. Nothing in these tests sets `data-exact-orientationlock`, so the new lock is never taken.
- `js-runtime` copies real `resource-inspection.js` and `sheets.js`. With `DEVELOPMENT_STORE` null, inspection does not write resource fields or decide reuse. The module tests never open a payment sheet, so `has`, `letGo`, and `deadlines` run against an empty map. The `ts-fetch` stand-in only gained the `untraced` set and `dropTimers` export that `ts-data.js` now imports. Storage tests still arm no timers.
- The popover Swift stub's `length(basis:)` exists so `PositionArea.swift` type-checks. The driver calls `PositionArea.origin`, which does not use it, and the Chrome comparisons are unchanged.
- The agent fixture's compiler prints `{"tests":[],"properties":[]}`, and the empty run expects `skipped: 0`, matching `authored_json` and `runTests`. `hasAttribute: () => false` is the unpressed pressable `showPress` reads after the click these assertions count.
- Both Fieldnotes grant strings in `js/web/tests/it/browser.rs` match `apps/fieldnotes/app.ts`, including `fs.read app:/tmp/shared`. The probe still checks the narrow scope, traversal, agent mode, and disposal.
EXIT=0
