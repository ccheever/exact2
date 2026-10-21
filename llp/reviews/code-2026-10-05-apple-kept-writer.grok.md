# Code review: Apple kept answers written on a thread of their own (8fca848db..fedc8b658), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `fedc8b658`, plain output.
- **Method:** one brief (sha256 `334e23df606c924e82feeebecd944753d1e1e3b28fc62783c11bffcc3507773e`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken: before a reload drops the replaced host, it waits for the queue (`flush_kept`, a development path) and journals the old host's failures on its successor. 2 taken: a write sent to a writer that is gone is told as a failure. 3 taken in part: the gated-store test (astra's 3); a persist-to-journal and a reload test through `Host` are DEFERRED. 4 taken.

---

LAND WITH CHANGES

1. MATERIAL — A failed kept write is dropped with the host that queued it, so the later commit that is supposed to journal it never sees the line. `Platform::write` returns `Ok` immediately and the writer records the error on that platform's `failed` arc (`host/apple/src/store.rs:104-110`, `store.rs:164-167`). `persist` drains only the live host (`host/apple/src/host.rs:702-705`). A dev reload builds a new `Platform` (`host/apple/src/abi.rs:735-740`) and `commit_plan` drops the old host (`abi.rs:824-833`) without flushing. In-flight jobs then push into an arc whose last owner is the job, and the vec is freed with the message. The runner has already journaled `store exact.kept.<name>` (`runner/src/runner/lines.rs:66-68`) for that commit. Sequence: commit queues `exact.kept.feed`, the kv write fails, reload, the new host's `commit_boot` journals nothing. Fix: flush the writer while the old `Platform` is still alive, take `kept_failures`, and log those lines on the successor (or push failures into a process-wide list that every `persist` drains). Do not flush inside `persist` or the next commit waits on disk again.

2. MINOR — A dead writer silently drops later writes, and `flush` treats that as a drained queue. `send` ignores a disconnected channel (`store.rs:178`); `flush` ignores `recv` failure (`store.rs:182-188`). After the `exact.kept` thread panics, every `write` still returns `Ok`, nothing is pushed to `failed`, and `snapshot_of` reads whatever is already on disk. Fix: on `SendError`, push `store exact.kept.<key> failed: …` from the job in the error; if `Flush` fails, respawn the worker or surface the failure instead of returning.

3. MINOR — Tests do not pin the host wiring or a wrong-but-fast implementation. `a_kept_answer_is_written_off_the_commit` (`store.rs:249-268`) passes for a synchronous `write` that returns `Ok` and records the error. `kept_answers_reach_the_platform_store_without_an_app_grant` (`host/apple/src/abi_tests.rs:999-1008`) reads the runner journal in the same turn as the queue and never reads the kv, so a denied or dropped platform write still passes. Nothing fails if `persist` stops calling `kept_failures`, or if a reload loses finding 1. `kept_answers_outlive_the_launch` does pin same-key order through `snapshot_of`. Fix: drive a failure through `Host::persist` and assert the next commit's journal; add a reload that drops the first `Platform` and still journals; assert the secret in that test stays visible without a flush.

4. NIT — `Host::boot_stored` still says store writes happen "on this thread" (`host/apple/src/host.rs:247-250`). That is now true only for secrets. Fix: point kept answers at the writer thread.

No earlier `llp/reviews/code-2026-10-05-apple-kept-writer*.md`. Checked, not filed: one owner thread plus the FIFO worker keeps commit order; `snapshot_of`'s flush is enough for a later launch or fresh boot on that thread; `KvStore` is `Send + Sync`, `MemoryStore` is mutex-backed, and agent mode still boots an empty store and reads the runner, not the kv. Crash loss of an in-flight queue is the change's stated tradeoff. `store.rs` is 284 lines, `host.rs` 1475, under the 1,500-line cap.
