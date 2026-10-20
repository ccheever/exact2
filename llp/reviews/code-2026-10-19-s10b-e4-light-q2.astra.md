# Code review: LLP 1121 stage 10b E4, the one light mode and theme2's 18:54 amendments, quick re-check 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f3f5c88430`.
- **Method:** one brief (sha256 `f9d92f7d25712d5f0f319d11d74224526e5b4d8cb6a434944fe1acb34d9a5081`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **Note — amendments (1), (3), and (4) are met.** The old modes merge under the kept mode’s stored ID, with dropped-mode edits reported first; collapsing uses header declarations; tests cover the merge and split choice. Amendment (2) works for the fixture but has the blocker below. **54 tests pass**, including both entry-point builds.

2. **Note — earlier blockers are closed.** Conflicts now check every twin. Removing a mode containing personal variables’ values is refused with names, and failed changes are undone. The UI’s duplicate declaration is fixed.

3. **Blocker — split choices overwrite each other when one token clashes in multiple light groups.** With ordinary `density=compact|comfortable` plus under-dark `dark-style=black|dim`, the same token can clash separately in both density groups. The UI asks twice but stores both answers in [`prefer[c.token]`](/tmp/rv-e4-light2/tools/figma-tamber/src/ui.ts:96), so the second replaces the first. [`collect`](/tmp/rv-e4-light2/tools/figma-tamber/src/code.ts:334) treats both clashes as resolved; [`push`](/tmp/rv-e4-light2/tools/figma-tamber/src/mapping.ts:258) silently skips the first group because the selected collection belongs to the second. The outgoing export therefore ignores the first choice and retains conflicting twins. Choices need a token-and-group key, with selection membership validated before sending.

**One correctness blocker remains.** No additional security or data-loss defect found in this re-check.
