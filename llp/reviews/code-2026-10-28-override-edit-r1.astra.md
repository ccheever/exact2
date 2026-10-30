# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3db023ed51`.
- **Method:** one brief (sha256 `7b541e89a01ca2b1cef59706bddd6c484baf0852559c6cf71ebde0b526258695`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [apps/design-tool/app.contract:753](apps/design-tool/app.contract:753): classify by current ownership.** `setForAppName` survives Apply, Cancel, and file changes. After Set → Cancel, removing the package dependency can leave that name wholly undefined; a later genuine Add still sends `set-token`, which fails with `no token`. If an app-only definition remains, it overwrites it instead of rejecting a duplicate Add. Use current source/override metadata, accounting for the selected file. Removing only an override while its package/base remains is harmless: `set-token` correctly recreates that override.

- **SHOULD — [apps/design-tool/app.contract:753](apps/design-tool/app.contract:753): clear branch-draft state when submitting this form.** Extend the new test after its second Apply: enter `740px` in `new-value` and press Enter. The cell edit left `draftFor` describing `700px`; `afterWrite` retained it while the branch became `720px`. Switching back to `set-token` activates `partsStale`, disabling Apply despite a valid proposal. This form path needs the branch-state reset performed by `setForApp`.

The [new test](scripts/design-tool-scale.test.mjs:175) should fail on the parent: its first submit produces `add-token`. It also checks the cell edit-again path with two successful transactions and exact file contents. In-memory expression checks confirmed the dispatch difference and stale guard; the full server test was not run because this read-only sandbox forbids its fixtures and build output.

VERDICT: findings
