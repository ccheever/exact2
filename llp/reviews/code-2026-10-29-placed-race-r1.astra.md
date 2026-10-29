# Code review: the Placed retirement race, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f265465bc3`.
- **Method:** one brief (sha256 `6c2828c543b441e4435e38a8c672f5c38dd08bfbf28c97fa46a3d99f0b44cd88`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **[P1] A retired call can still arrive after its retirement marker** — [data/src/placed.rs:794](data/src/placed.rs:794). This is a pre-existing gap that remains after the fix: `dispatch(A)` removes A’s record and returns a deferred closure; retirement excludes A; then A’s closure queues its turn. The owner processes `Forgotten` before A exists, then runs A and parks it. Its discarded reply causes no further cleanup, so A can miss `__exact_forget` indefinitely.

   This ordering is reachable: the host [retains forgotten module jobs](host/apple/src/executor_core.rs:685), executes their handoffs, and [discards their completions](host/apple/src/executor_core.rs:1046).

   **Fix:** retain per-call retirement state through the dispatch-to-enqueue interval and check it under the queue lock, rejecting obsolete handoffs or ensuring cleanup after they execute. Add a deterministic unit test that holds A’s dispatched closure across retirement, releases it, and checks that A cannot remain parked. The current property test cannot catch this: both queues permit the same leak.

**Non-blocking**

none.

The reported overwrite diagnosis is correct. Intersection gives the same surviving parked-call set as sequential retirements: owner tokens are all `None`, duplicates do not affect membership, and `turn()` clears `pending` before enqueueing. A turn already ahead of both markers is harmless; an intervening queued turn prevents merging. Taking a slot forces a subsequent retirement into a new marker; failed sends imply the receiver is gone.

The newest snapshot is retained as intended. Equivalence concerns call membership; callback counts and snapshot histories still differ. The tests are sound for that narrower property, and a TypeScript integration test is unnecessary to establish it.

Static review only; no files changed, tests/builds run, or network used.
