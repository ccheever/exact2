# Code review: a held proposal's choice (tamber/1.18), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `32fa11a433`.
- **Method:** one brief (sha256 `a42f2862fa74fe77d3404e02473bcfb28f3e052c00206dcd45aeadd6503c2338`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Accept can still write a selection the card does not show.** [apps/design-tool/app.ts:1340](apps/design-tool/app.ts:1340). After a foreign rebase, `chosenOf` immediately ignores obsolete flips, while the checkboxes still apply them until `takeChoices` runs. For example, `p:1` leaves operation 1 visibly unchecked, but a foreign base `[0,1]` makes Accept send both operations. The generation check passes because the rebase happened before the press. Another card’s pending choose can prolong this interval. **Fix:** reconcile the displayed base and flips atomically, or refuse accept/edit until both reflect the same generation.

2. **An older resource invocation can reactivate obsolete flips.** [apps/design-tool/choice.ts:84](apps/design-tool/choice.ts:84), called from [app.ts:1379](apps/design-tool/app.ts:1379). `drop` treats its argument as the current selection, but `sendChoices` captured it before awaiting requests. An older invocation waiting on card Q can contain no flips for P; meanwhile P is flipped and then rebased. When that older invocation finishes, it clears P’s `rebased` marker—even if its resource answer is discarded. P’s obsolete flips now apply over the foreign base and can be saved or accepted as an unintended selection. **Fix:** acknowledge flip removal only from the latest committed selection, with a generation check; superseded invocations must not clear rebase markers.

**Non-blocking**

1. **The unit tests miss the claimed in-flight ordering.** [scripts/design-tool-choice.test.mjs:31](scripts/design-tool-choice.test.mjs:31) and [line 96](scripts/design-tool-choice.test.mjs:96) inject the foreign record before `send`’s `.then` executes. Its initial `foreign` check therefore refuses without calling the mocked request. Await an explicit “request started” barrier before injecting the record, and cover rendered cards and superseded resource completions.

No scope, pinned-name, or paired-door bypass found. Both permitted test commands stopped before assertions with sandbox `EPERM`: temporary-directory creation and TLS listener binding.
