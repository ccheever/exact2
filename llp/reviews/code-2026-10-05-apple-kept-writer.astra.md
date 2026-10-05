# Code review: Apple kept answers written on a thread of their own (8fca848db..fedc8b658), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `fedc8b658`.
- **Method:** one brief (sha256 `334e23df606c924e82feeebecd944753d1e1e3b28fc62783c11bffcc3507773e`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken in part: the first kept write registers an `atexit` drain, so an orderly exit (an app quitting through `exit`, the agent's quit) writes what is queued first. macOS's sudden termination (a SIGKILL) still drops a write in flight, as it could drop the synchronous one mid-write; a subprocess test is DEFERRED. 2 taken: the writer takes everything queued at once and skips a write the same platform's later write to the same key replaces before the next flush, so a fast-changing store costs one write per key per drain. 3 taken: a gated store proves a write returns before the disk is touched, a snapshot waits for the queue, five ordered writes (one a delete) land as the last, and the superseded ones are skipped.

---

LAND WITH CHANGES

1. **MATERIAL — Graceful exit loses committed writes.** The writer is detached ([store.rs:149](host/apple/src/store.rs:149)); only snapshots drain it. macOS permits immediate termination ([main.swift:412](host/apple/Sources/ExactMac/main.swift:412)), and agent quit calls `exit(0)`. Commit an answer, then quit while its write is pending: the next process reads the old answer, and its empty queue cannot recover the update. Fix: drain before orderly termination, including agent quit, and add a subprocess persistence test.

2. **MATERIAL — Pending cache writes are unbounded.** Every update clones its value into an unbounded channel ([store.rs:104](host/apple/src/store.rs:104), [store.rs:148](host/apple/src/store.rs:148)). Changing store-reading resources faster than disk can persist them retains every obsolete answer; memory and subsequent snapshot latency grow continuously. Previously synchronous persistence imposed backpressure. Fix: bound pending bytes and coalesce superseded writes per store/key while preserving delete ordering and flush barriers.

3. **MINOR — Tests do not pin the concurrency contract.** The new test asserts `write().is_ok()` ([store.rs:258](host/apple/src/store.rs:258)); synchronous writing with deferred error reporting would pass. The existing integration test checks the runner’s journal immediately ([abi_tests.rs:1001](host/apple/src/abi_tests.rs:1001)), before asynchronous failures are drained. Fix: use a controllably blocked store to prove write returns early, snapshots wait, successive writes/deletes remain ordered, and a later commit journals failure exactly once.

`Kv` supports cross-thread use through `Arc<dyn KvStore>` and `Send + Sync`. Both changed source files meet the line cap; `git diff --check` passed. No earlier review files found. Runtime tests were not run in this read-only checkout.