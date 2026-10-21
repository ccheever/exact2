# Code review r2: Apple kept answers written on a thread of their own (8fca848db..aae425aab), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `aae425aab`.
- **Method:** one brief (sha256 `f107c19dff1f8ea409c7f2ae0f5f3e131537539161ff1a4632977999de4762f8`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken: the queue is gone; what waits is a map of the last value per platform and key, in first-kept order, so memory is bounded by the keys. 2 taken: a dropped platform's untold failures pass to the next platform that asks (`Failures`'s `Drop`), whichever path replaced its host; the `commit_plan` hand-off is removed. A test of it is DEFERRED (it would race other tests' hosts for the shared list). 3 taken: flushes no longer split what waits, so the bound (one write per key after the first) holds whatever another test flushes; the reader check stays a check that it has not finished. 4 taken: `abi.rs` is unchanged.

---

LAND WITH CHANGES

1. **MATERIAL — Coalescing still leaves unbounded pending memory.** [store.rs:153](host/apple/src/store.rs:153) queues every cloned answer; coalescing happens only after dequeue. While one disk write stalls, repeated updates to one key retain every obsolete value and can exhaust memory. Round 1’s backlog disposition is incomplete. Fix: coalesce at enqueue and bound pending bytes, preserving flush boundaries.

2. **MINOR — Fresh reload still loses failure reports.** The dev menu can use `boot_fresh`: its snapshot waits at [abi.rs:466](host/apple/src/abi.rs:466), but replacement at [abi.rs:502](host/apple/src/abi.rs:502) never takes the old host’s failures. A write failing during that wait disappears without being journaled. Only `commit_plan` received the fix. Transfer failures on both replacement paths and test both.

3. **MINOR — The gated test can fail under parallel execution.** [store.rs:383](host/apple/src/store.rs:383) assumes at most two sets despite sharing the process-wide queue. Valid schedule: write `1` blocks; enqueue `2`; another test flushes; enqueue the remaining writes. The flush boundary preserves `2`, producing three sets: `1`, `2`, `4`. Fix: test coalescing with an isolated queue or explicit batches; synchronize reader entry instead of relying on the 50 ms sleep.

4. **MINOR — Required cap check fails.** The addition at [abi.rs:832](host/apple/src/abi.rs:832) pushes the file to **1,507 lines by `caps`**, exceeding 1,500. `bun scripts/caps.mjs` reports this sole violation. Shorten or extract code before landing.

`git diff --check` passed. Thread-safe KV ownership and synchronous secrets are preserved. Exit persistence and host-journal coverage remain deferred; runtime tests were not run in this read-only checkout.