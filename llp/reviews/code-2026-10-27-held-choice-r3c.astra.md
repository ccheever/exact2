# Code review: a held proposal's choice, round 3 re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f0ae7eacae`.
- **Method:** one brief (sha256 `51ab30a08751406f329c964b43feb479a3c39811ea23e8419f4e75da8fe27adc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [apps/design-tool/choice.ts:82](apps/design-tool/choice.ts:82) — `shown` advances before the page receives the rebuilt card. `fold` calls `takeShown`, but `readChanges` can then await the resync `hello`. During that wait, the displayed card remains old and `inStep` passes with empty `picked`. Reproduced: card shows `[0]`; accept sends `[0,1]` with the new `choiceAt`, and edit records `[0,1]`. **Fix:** pass the displayed card’s generation into accept/edit and compare against that, rather than treating a module-side rebuild as displayed.

2. [apps/design-tool/app.ts:1365](apps/design-tool/app.ts:1365) — Edit’s guard does not cover the awaited `send` or the subsequent `edit-held` request. Reproduced both a successful `send` returning after its generation changed, and another client choosing `[0,1]` between this page’s `choose([0])` and `edit-held`; the edit succeeds carrying `[0,1]` although the card showed `[0]`. **Fix:** recheck the captured generation after `send`, and make `edit-held` atomically validate the expected `choiceAt` before carrying the selection.

**Non-blocking**

none.

B and C check out: the superseded invocation preserved the rebase marker, and checkbox/selection parity passed 128 combinations. All five existing unit tests pass.
