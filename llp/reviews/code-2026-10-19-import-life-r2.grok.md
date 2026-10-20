# Code review: Tamber imports' lifecycle leftovers, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f5efc882e3`.
- **Method:** one brief (sha256 `c8d3ac23c6443155a647db735b901dc0a9f03b891a4107b4f16af58240fd28b4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this commit read-only: first the round-1 findings, then the diff, then the cancel, read, and `kept()` paths for double-release or cross-client bugs.Round 1 is in. I’ll read the fixed cancel, hold, and `kept()` paths in the current tree and check the tests still lock those cases.The round-1 fixes are in the tree. Next I’ll trace slot release, the hold-time drop, and `kept()` against the tests.## Blocking

None.

## Should-fix

**A cancel at the hold cap deletes some other proposal.** `scripts/tamber-server.mjs:1140`

`forget(held.ok.proposal)` only removes the id `hold()` just inserted for this import. That id is not reused, and nothing else can accept it before this check. The drop itself is this import's.

`hold()` does not see the signal. If the list is already at 64 when the import is cancelled during `hold()`'s propose, a write-scope import still evicts the oldest proposal (`scripts/tamber-server.mjs:980`, any client's) and a propose-scope import still evicts that client's own oldest (`:974`) before this line deletes the new one. The evicted proposal is not put back. Check the signal inside `hold()` after its awaits and before either eviction.

## Nits

- `scripts/tamber-reimport.test.mjs:190` — the HTTP cancel test still sleeps 300 ms instead of waiting until the planner has started. The isolate test now waits; this one can pass on the queue path.
- `scripts/tamber-isolate.mjs:120` — nothing forces a short read, a file that grows, or the 2 s deadline. Reverting the loop would not fail these tests. Nothing cancels during `hold()` either.
- `contract/cli/src/tamber_export.rs:189` — a failed `read_to_end` is still kept when the bytes read are UTF-8, including an empty buffer. One non-JSON baseline fails a headerless re-import. Invalid UTF-8 is charged and not kept. The test locks that, not the 64 MiB constant itself.

## Checked

Round 1 is fixed apart from the cap side effect above. The queued payload is dropped (`job = null`). Invalid UTF-8 is charged. `stopImports` no longer signals a reaped pid; `stop()` kills only while `exit` has not been seen (`scripts/tamber-isolate.mjs:155`).

The queue count and the per-client count are released once. `release()` is idempotent. Cancel while queued, `waitMs`, and `finally` all use it; `waitMs` is cleared before the turn awaits, so it cannot race the running planner. Whichever of cancel and `waitMs` runs first sets `expired`. A running cancel does not release; `finally` does, after `close`. One client's `AbortController` and `onEnd` are not another's. `res` `close` aborts only when `writableEnded` is still false, so a normal `end()` does not. `unend()` runs after `dispatch` returns, including on throw.

The read loop treats `bytesRead === 0` as EOF, continues after a short read, and stops at `max + 1` or the deadline. `kept()` refuses a symlink at `.exact` or the baselines directory (`symlink_metadata` + `is_dir()`), and directory entries use `file_type()` so a symlink is not a file. TOCTOU aside, that check is sound. The newest 16 files are read, and a file that would pass the cap is not kept in part.

`bun test scripts/tamber-isolate.test.mjs scripts/tamber-reimport.test.mjs`: 14 passed. `cargo test -p contract --bin contract kept_`: 2 passed. The cancel, queue-slot, symlink, and byte-charge assertions would fail on the previous code.
