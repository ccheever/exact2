# Code review: native abort, landing: kept module work, round landing re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a90b53d9b8`.
- **Method:** one brief (sha256 `a31ff200aa2e499268d8e079be1e293c3599c5d28297ebd4ab3d879e31ca0d4f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

Both reported issues are resolved:

- Aborted kept tickets reach `fulfill` → `module_work_landed`, ending the runner’s wait.
- The fetch map survives retired-list eviction and is removed on landing, including admission refusal.
- Ticket removal precedes the module commit’s checkpoint, so refusal cannot restore it. Aborts heard before `keep_sent` retain their runner ticket; requests ultimately not kept remain cancellation-only.
- Repeated source tokens do not collide with fetch identities. The JavaScript adopter prevents adopting the same fetch twice.

**Non-blocking**

1. **Pre-existing continuation caveat:** [js/src/prelude.js:696](js/src/prelude.js:696) already removes the fetch and rejects its promise immediately. Consequently, [__exact_fulfill:1452](js/src/prelude.js:1452) finds nothing when the synthetic module reply arrives. A kept continuation can therefore run during the aborting answer’s turn, and its store writes can roll back if that transaction subsequently refuses. This behavior exists at `3217b852ee`; HEAD fixes runner settlement but does not establish the stronger claim that this rejection’s continuation runs outside a checkpoint. To guarantee that, deliver adopted-fetch rejection in the module turn and cover the refused-transaction case with a real JavaScript module.

Static review only; no files edited, network used, tests or builds run.
