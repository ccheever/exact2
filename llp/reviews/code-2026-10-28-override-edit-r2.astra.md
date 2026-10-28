# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6c3fe3210d`.
- **Method:** one brief (sha256 `c5764e719da72e45f6a668d035a67da12e8cfbf2461a0fcd990758dea994956b`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [apps/design-tool/app.contract:755](apps/design-tool/app.contract:755): Operation choice ignores the destination file.** Open another writable design, such as `brand.tamber`, without `space.md`, then submit `space.md = 20px`. Every file receives base rows, so this selects `set-token` while retaining `file="brand.tamber"`. The server converts missing-token sets into additions only for the app design (`tamber_packages.rs:477`); this request fails with “no token.” Previously, `add-token` could create that line. Make classification respect the destination and cover another-file submission.

- **SHOULD — [apps/design-tool/app.contract:760](apps/design-tool/app.contract:760): The form retains the selected cell’s read-only guard.** Select an unoverridden package token’s cell, then submit its name/value directly through Add a token. The proposal becomes valid `set-token`, but `selToken` remains selected, so `applyAnswer` at line 310 keeps Apply disabled. Clear that selection or restrict the guard to cell drafts. The new test misses this because `setForApp` clears the selection first.

`of` correctly distinguishes all requested row kinds, and nested filters preserve `n`. Clearing the draft selectors introduces no additional loss: form submission already replaces the single pending edit.

In-memory probes confirmed classification, request targeting, and the guard behavior. The full integration test was not run under read-only constraints.

VERDICT: findings
