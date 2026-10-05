# Code review r2: Apple kept answers written on a thread of their own (8fca848db..aae425aab), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `aae425aab`, plain output.
- **Method:** one brief (sha256 `f107c19dff1f8ea409c7f2ae0f5f3e131537539161ff1a4632977999de4762f8`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken (astra's 4). 2 taken: the exit drain sets a closing flag first, so a write kept after it is made in place, and the flush waits until nothing waits and the writer is idle, which covers a write that arrives while it waits. 3 taken: a store that panics is caught and told as a failure; the writer never dies, so there is no dead-writer path. 4 taken in part: two keys in one wait are tested; host-level persist, reload and exit tests are DEFERRED. 5 taken.

---

LAND WITH CHANGES

Round-1 dispositions that match the code: per-key coalesce before the next flush (`store.rs:190-221`), the gated-store test (`store.rs:346-385`), `commit_plan` flushing and journaling the replaced host (`abi.rs:832-839`), `SendError` recorded on a dead writer (`store.rs:178-181`), and the `boot_stored` comment (`host.rs:247-251`). Secrets stay on the caller (`store.rs:112-113`). `KvStore: Send + Sync`; `MemoryStore` is mutex-backed; `FileStore` renames a temp file into place. One owner thread (LLP 1072 T1) queues every kept write, so a launch or reload `snapshot_of` cannot race the writer.

1. **BLOCKING** — `host/apple/src/abi.rs` is over the 1,500-line cap. `scripts/caps.mjs:154` counts `split('\n').length`. That is 1507 here, and this is the only non-vendored source over the cap (`rules/RULES.md`, baseline 0). The reload block at `abi.rs:832-839` is the 8 lines that push it from 1499. `caps` fails. Move that handoff into `store.rs` (flush, take `kept_failures`, return the lines) so `abi.rs` only calls it.

2. **MATERIAL** — An orderly quit can drop kept writes the `atexit` drain says it saves. `drain` (`store.rs:163-165`) calls `flush_kept` once. `flush_kept` (`store.rs:225-232`) posts `Flush` and waits. The owner thread can still be in `persist` (`host.rs:1318-1319`) and `send` (`store.rs:178`). The batch is `recv` plus `try_iter` (`store.rs:157-159`), so a write that lands after `Flush` shares the batch, and the ack is sent when `Flush` is reached, before later jobs (`store.rs:218-220`). `exit(0)` then runs (`Agent.swift:121`, and Cmd-Q's `.terminateNow` at `main.swift:412`). `_exit` kills the writer with those writes still queued or not yet synced. Idle quit is fine; quit overlapping a commit is not. Under the send mutex, set a drain flag, enqueue `Flush` as the last job, wait, and run any `send` that arrives after the flag inline once that wait returns.

3. **MINOR** — A panicked writer still looks like a finished queue. Round 1's dead-writer note only covered later `send`s. If `exact.kept` panics inside `write_kept`, the rest of the batch (including `Flush`) is dropped with no `failure` line. The next `send` records "writer thread is gone" (`store.rs:178-181`), but `Flush`'s `done` is dropped and `wait.recv()` ignores the disconnect (`store.rs:182-183`, `store.rs:232`). `snapshot_of` (`store.rs:242-249`) then reads a stale store and boots it. On `Flush` failure, leave a process-wide failure (or respawn) and make `snapshot_of` refuse to treat that as drained.

4. **MINOR** — Tests pin the store, not the host. `kept_answers_are_written_in_order_after_the_commit_and_a_snapshot_waits` fails a synchronous `write`, a snapshot that does not wait, or a coalesce that writes all four sets. Nothing fails if `persist` stops logging `kept_failures`, if `commit_plan` stops the handoff, or if `atexit` is removed. `kept_answers_reach_the_platform_store_without_an_app_grant` (`abi_tests.rs:1001-1008`) only reads the runner's synchronous `store exact.kept…` line. Same-key order is covered; two keys in one batch are not. The reload and exit tests were deferred and are still the hole.

5. **NIT** — `host/apple/src/lib.rs:25-26` still says the store is written after each commit, with no writer thread. Point it at `flush_kept`, as `boot_stored` now does.
