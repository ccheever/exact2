# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 2 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `301a831244`.
- **Method:** one brief (sha256 `b05961431317a59a7803520e35dcb3a10f06badbee961f1f9fc602b47a83bdc0`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 2 SHOULD-FIX, 1 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

**Round 2: changes needed.** Round 1’s lost-job retry, A → B → A handling, repeated-failure loop, idle queue progress, compiler-unwind handling, and send-before-log ordering are fixed. Scrub frames now observe failures, and the protocol names background failures. Input completeness and test synchronization remain incomplete.

- **BLOCKING — Scrub recovery can clear the wrong retention key.** [tamber_scrub.rs:78](contract/cli/src/tamber_scrub.rs:78), [tamber_serve.rs:463](contract/cli/src/tamber_serve.rs:463). A frame supplies its opening key, but the job fingerprints and builds the **current disk files**. Open a scrub on broken design A, consume its failure, externally change to passing design B, and continue frames with A’s opening digests. B’s passing build clears A’s failure. Reverting to A then lets ordinary proposes hit A’s retained cache and answer `live` indefinitely: `Passed` bypasses fingerprint checking. Recovery must establish that the compiled inputs actually belong to the supplied retention key.

- **BLOCKING — The fingerprint still cannot prove “exactly current inputs.”** [tamber_check.rs:113](contract/cli/src/tamber_check.rs:113), [tamber_walk.rs:91](contract/cli/src/tamber_walk.rs:91). The extra scan ignores directory entries themselves, follows no symlink targets, and records only length/mtime. Creating an empty `strings/` changes compilation from success to `strings-base-missing` without changing this fingerprint; editing an externally linked strings table is similarly invisible. A stale pass can therefore clear unknown, or removing an empty strings directory can leave its cached failure permanent. `Walk::inputs` also omits its separately tracked manifest/resolution facts. Ordinary strings, assets and `.shells` files are now covered, but the round-1 finding remains open.

- **SHOULD-FIX — Tests still depend on scheduling.** [tamber_retention_reuse.rs:538](contract/cli/tests/it/tamber_retention_reuse.rs:538), [tamber_check.rs:385](contract/cli/src/tamber_check.rs:385). `reported(2)` assumes both initial jobs execute, but the new worker may legitimately coalesce them into one, causing a timeout. Immediate `known()` assertions assume the worker cannot finish before the second poll. The sleep-based race test still establishes no ordering; close still tests process exit rather than result delivery after dropping the checker. Use synchronization around identified jobs and assert the resulting state.

- **SHOULD-FIX — Thread-spawn failure blocks answers and close.** [tamber_check.rs:302](contract/cli/src/tamber_check.rs:302). The fallback runs the whole build synchronously in the request loop. That delays both the response and processing EOF, contrary to condition (a). Conservatively answer `rebuild` when the worker cannot start.

- **NIT — Scrub documentation now overstates isolation from disk.** [tamber_scrub.rs:8](contract/cli/src/tamber_scrub.rs:8), [tamber_serve.rs:460](contract/cli/src/tamber_serve.rs:460). Every eligible frame refreshes `walk()` before the checker’s lazy fingerprint closure—even for `Passed`. “Reads no file and walks no Contract source” is no longer true; the reported benchmark does not measure this path.

What checks out: one worker executes builds serially and drains pending jobs to the newest; normal requests never join it. Dropped receivers discard later results. Compiler state is thread-local; shared kernel interners are synchronized, though they retain shared contention and capacity. Compiler panics are caught when unwinding; release-profile panics still abort the process.

The recorded medians match the samples: alias replies **2747/2754 → 682/672 ms**. These measure reply latency, not completed validation.

Read-only review; no files changed. Diff whitespace checks passed. Repository tests and benchmarks were not run because they write files.
