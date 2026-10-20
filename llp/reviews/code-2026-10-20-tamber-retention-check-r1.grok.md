# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 1 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `a3832cc4c2`.
- **Method:** one brief (sha256 `1bdfa912f0b74744d5bb30f98388dcac91ba9ac5405e803fd1a7f66ba72208ca`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, 3 SHOULD-FIX, 2 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

## Findings

**BLOCKING — A reported failure is thrown away when the bytes have moved, and nothing retries it — `contract/cli/src/tamber_check.rs:136`**

`poll` records `Built(false)` only when `next` is empty and `job.inputs == current`. A value edit keeps the retention key (a hex is outside `retention_key`) but changes the design digest, so that equality fails and the failure is discarded. `synthetic` then cache-hits the same key and returns without `start` (`contract/cli/src/tamber_serve.rs:418`).

That is a normal pair of transactions. The breaking write still sees the pre-write snapshot, cache-hits, and does not start a check. The next write (an `ink` change) is the first call that sees the new key: it starts the check, stores the exprs, returns `live`, then writes. The check's `failed` or `raced` is for the pre-write digests. The following propose polls the post-write digests, drops the result, cache-hits, and never starts a job. Every later propose of that key stays `live` until a name-moving edit. On main those proposes are `rebuild`. The 12:19 rule keys the failure by the retention key so this edit stays `rebuild`.

The tests miss it. `a_value_edit_while_broken_is_rebuild_and_a_fixing_one_returns_to_live` waits until `failed` is logged, then edits; that write's `synthetic` still sees the pre-write bytes, so the latch sticks. The unit test `a_newer_job_drops_an_older_ones_result` asserts `!unknown` at the end, which is also true if the stale failure was applied and the later pass cleared it.

**SHOULD-FIX — Restarting the running job leaves a stale `next`, which discards the current result — `contract/cli/src/tamber_check.rs:63`**

`start` returns immediately when the running job equals the new one, and does not clear `next`. `poll` then treats the running result as stale solely because `next` is `Some`, even when those inputs are still current. An edit reverted before the check finishes drops the failure, runs the obsolete job, and races. If the key was never latched, the cache hit above never starts a replacement.

**SHOULD-FIX — A panicked worker is ignored and not retried — `contract/cli/src/tamber_check.rs:132`**

`TryRecvError::Disconnected` becomes a `None` outcome and falls through the `_` arm: the key is not marked unknown, and `running` is cleared. The next `synthetic` cache-hits and does not call `start`. One panic in `compile_path_all` leaves that key `live` with no further whole build. A disconnect is a check that did not succeed; it should latch the same way a `failed` does, and the server should keep checking.

**SHOULD-FIX — The digest gate is not the whole build's inputs — `contract/cli/src/tamber_walk.rs:90`**

`Walk::inputs` is the walk's file list: the root, `app.json`, Contract sources, design files, consulted `package.json`s, and icon SVGs (present or missing). That covers the ruling's Contract and design files, package sources, and `app.json`. It does not cover bytes `compile_path_all` still reads, and whose contents can fail the build:

- Font bytes (`contract/lower/src/fonts.rs:159`) and sound bytes (`contract/lower/src/sounds.rs:123`).
- `app.ts` and `data/Cargo.toml`, which `code_beside` only checks for existence (`contract/cli/src/sources.rs:461`) and which change both the load and `sources_key`.
- Shipped-target markers the icon pass reads (`.shells/app.json`, `android|linux|web/Cargo.toml`).

There is no separate strings file; string data is the source text already in the walk. A font or sound that changes during the build is not a race, so a pass can clear the key, or a failure can latch it, for a snapshot the compile did not actually see. `digest_of` matches the walk's read (empty when missing or unreadable).

**NIT — Identical failures are whole-built again on every later answer — `contract/cli/src/tamber_serve.rs:414`**

The call that records a failure has just cleared `running`, and the `unknown` branch `start`s that same job. Tracked inputs that just failed cannot pass, so this is not the 12:19 clear. While a client keeps proposing, the same broken bytes are compiled once per answer until the key changes. It is one worker, not a spin, and it stops when answers stop.

**NIT — The protocol doc still describes the old closure — `docs/tamber-protocol.md:683`**

`class` is documented as `rebuild` when the build stops before the design. It does not say that a whole build that fails after the design becomes `rebuild` only once the background check is applied, and that the breaking write itself stays `live`. The bench numbers exist only in the commit message. The CPU-sharing explanation matches a concurrent `compile_path_all`; this review did not re-measure them.

## What checks out

- At most one worker. A different job replaces `next`; it does not start a second thread.
- `poll` uses `try_recv`. Close returns from the stdin loop and drops the receiver; the worker's `send` after close is ignored, and the process exit does not join the thread. The 30s-delay test covers that sleep. The worker does not hold a lock `close` needs.
- Lowering state is thread-local (`retention_only`, `PROGRAM`, candidates, the server's contract/design text overrides), and the worker is a fresh thread, so it does not see the server's retention stop or its planned texts. The kernel tables a compile shares (calc intern, calc handles, platform-color intern) are mutexes held for the intern only. An answer does not wait for the worker; two compiles do not swap thread-locals.
- Unknown is stored by the retention key, not the input digest. A pass clears only `job.key`, and only when `next` is empty and the job's digests equal the walk at `poll`. A `Raced` result and a superseded result do not clear it and do not latch it. That predicate matches the 12:19 amendment when a result is actually applied.
- The answer that polls a latched key returns `rebuild` before the expr cache. The breaking write stays on the pre-write snapshot, so it is `live`, as ruled.
- `a_check_raced_by_an_edit_mid_build_is_dropped` matches the hook: the delay sits between the two digest reads. The 500 ms write against a 1500 ms sleep is inside the window if the thread has reached the sleep; a slow start still logs `raced` (the first digest already disagrees). It never asserts the class afterward.
