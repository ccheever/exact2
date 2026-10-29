# Code review: the Placed retirement race, round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3770de6d9f`.
- **Method:** one brief (sha256 `8409f5fbaa8809b92946cdc86322c763baa0a8486353459fbc1f224e577967d4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `data/src/placed.rs:1364` — the abandoned-entry assert also passes without `Unqueued`. `gone` is `Resource(1)`, and the retirement view is only `Resource(0)` / `read`, so `unqueued.retain` removes that entry on its own. Drop a closure whose key is still in the view, and assert the map is empty before `forgotten`.
2. `data/src/placed.rs:104` — `turn` drops the job if `send` fails, and the success path calls it while holding the queue lock (`854`). The let-go path releases that lock first. This only happens after the owner thread has already dropped the receiver. On `Err`, return the job and drop it after the guard is released, as in the let-go branch.
