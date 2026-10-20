# Code review: Tamber imports' lifecycle leftovers, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f5efc882e3`.
- **Method:** one brief (sha256 `c8d3ac23c6443155a647db735b901dc0a9f03b891a4107b4f16af58240fd28b4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking (correctness or security)**

- [scripts/tamber-server.mjs:1140](/tmp/rv-il-2/scripts/tamber-server.mjs:1140): Cancellation cleanup happens after `hold()`’s mutations. With 64 proposals held, cancellation during its await still lets it evict an older proposal—including another client’s for write scope—before dropping the new one. The returned proposal ID is safe; the preceding eviction is not. Check cancellation inside `hold()`, before eviction/insertion.

- [scripts/tamber-isolate.mjs:84](/tmp/rv-il-2/scripts/tamber-isolate.mjs:84): Round 1’s retention fix is incomplete. Cancelled entries remain in `pending` and the promise chain until their turn. Although `job` is cleared, `release()` still captures `who`. Local requests allow nearly 1 MiB `client` strings, so repeated enqueue/abort requests can retain large payloads while returning every admission slot. Remove cancelled entries or immediately clear their retained state.

**Should-fix**

- [scripts/tamber-server.mjs:1139](/tmp/rv-il-2/scripts/tamber-server.mjs:1139): Cancellation during `hold()` still waits for its answer, potentially 120 seconds; an error bypasses the cancellation response entirely. Also, `stopImports()` no longer tracks this stage, so server shutdown does not abort a local import’s signal. Carry cancellation through the whole import lifetime and return cancellation regardless of the hold’s eventual outcome.

- [scripts/tamber-reimport.test.mjs:190](/tmp/rv-il-2/scripts/tamber-reimport.test.mjs:190): Round 1’s HTTP-test finding remains: fixed sleeps neither establish planner startup nor assert exit. No test exercises cancellation during `hold()`. The [read loop](/tmp/rv-il-2/scripts/tamber-isolate.mjs:123) also still lacks short-read, growth and deadline coverage; reverting it would leave these tests green.

**Nits**

- [QUEUE.md:6](/tmp/rv-il-2/QUEUE.md:6): “held proposals … capped … at four” per client is inaccurate. `hold()` enforces global count/byte caps, without that per-client cap.

Round 1’s stale-PID kill and malformed-UTF-8 accounting findings are fixed. Release accounting, normal response-close handling, the read loop and directory symlink checks otherwise look sound.

Both permitted test commands were blocked during setup by sandbox `EPERM`; validation was static.
