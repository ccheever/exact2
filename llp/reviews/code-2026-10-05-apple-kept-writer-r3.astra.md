# Code review r3: Apple kept answers written on a thread of their own (8fca848db..79f2cef52), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `79f2cef52`.
- **Method:** one brief (sha256 `5025ea65ae37771579c9630a3b7a6a563bff8791ee676d2b1d2b1daa37c10419`), shared with grok. Round 3, the last, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (landed):** 1 taken: there is one write path; the exit's drain waits until nothing waits and the writer is idle, then keeps the writer's lock for the rest of the exit, so no write starts after it and none is made beside the writer. A subprocess exit test is DEFERRED. 2 taken: a key kept again moves behind everything kept before it, so across platforms on one storage the last kept is the last written; the gated test keeps the key from a second platform on the same store between two of the first's. 3 DEFERRED (as round 2).

---

LAND WITH CHANGES

1. **MATERIAL — Shutdown can persist an older value last.** [store.rs:178](host/apple/src/store.rs:178) writes inline once `CLOSING` is set, while [store.rs:188](host/apple/src/store.rs:188) is still draining queued writes. Sequence: old `k` waits behind another write; shutdown starts; the owner writes new `k` inline; the queue subsequently overwrites it with old `k`. Inline writes also escape `flush_kept`’s accounting. Round 2’s exit disposition is incomplete. **Fix:** quiesce the owner and synchronize closing with queue admission; retain one ordered write path through the final drain. Add a gated subprocess exit test.

2. **MATERIAL — Coalescing reverses writes between sessions sharing storage.** Keys include platform identity, but replacement retains the original queue position ([store.rs:111](host/apple/src/store.rs:111), [store.rs:203](host/apple/src/store.rs:203)). With the writer blocked, enqueue `A:k=1`, `B:k=2`, `A:k=3`: draining writes `3`, then `2`. Apple sessions use the same app storage directory ([kv.rs:537](vendor/ibex/crates/ibex2/src/kv.rs:537)); relaunch therefore reads `2`, whereas synchronous commit order left `3`. **Fix:** order retained entries by their latest enqueue, or coalesce by actual storage identity and key. Test two platforms sharing one store.

3. **MINOR — Host failure reporting remains untested.** [store.rs:324](host/apple/src/store.rs:324) drains failures directly; [abi_tests.rs:1001](host/apple/src/abi_tests.rs:1001) only inspects the immediate runner journal. Neither fails if `persist` stops reporting asynchronous failures or reload loses them. **Fix:** add an isolated subprocess test asserting that a later host commit journals a failed write exactly once, including after replacement.

Queue-size and file-cap fixes hold. Caps, formatting, boot and diff-whitespace checks passed. Runtime tests were not run: this read-only checkout has no build artifacts.