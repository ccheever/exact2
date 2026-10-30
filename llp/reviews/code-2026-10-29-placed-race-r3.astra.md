# Code review: the Placed retirement race, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3770de6d9f`.
- **Method:** one brief (sha256 `8409f5fbaa8809b92946cdc86322c763baa0a8486353459fbc1f224e577967d4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [data/src/placed.rs:855](data/src/placed.rs:855) — Failed sends still deliver `Aborted` under the queue mutex: `OwnerQueue::turn` drops `SendError<Job>` at line 107. If the owner has died and that reply callback drops another outstanding turn from the same `Placed`, its `Unqueued::drop` reacquires the same mutex and deadlocks. Return the failed job from `turn`, release the queue lock, then drop it. The let-go branch already handles this correctly.

**Non-blocking**

1. [data/src/placed.rs:1364](data/src/placed.rs:1364) — The abandonment test can pass without the guard: the subsequent `forgotten()` removes the abandoned `"gone"` entry before the final assertion. Assert its removal immediately after dropping the closure, before retirement, or abandon an untargeted turn.

Round 2’s reported scenarios are resolved by inspection; the blocker above concerns the separate failed-send path. No tests or builds run.
