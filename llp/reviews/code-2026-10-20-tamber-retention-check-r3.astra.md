# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 3 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `5dcbc4b358`.
- **Method:** one brief (sha256 `ae2544292f7a8b11da7c2a1682d652269029f74927aad2d5a2ea58f296a0cb13`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 1 SHOULD-FIX, 1 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

**Round 3: changes needed.** Prior fixes close the lost-job retry, A → B → A replacement, idle progress, repeated unchanged failures, compiler-unwind handling, synchronous spawn fallback, and original wrong-key scrub recovery. The concrete directory, symlink-target, and manifest omissions are fixed. Input completeness and test synchronization remain incomplete.

- **BLOCKING — Scrub frames can leave a reported failure unread.** [tamber_serve.rs:489](contract/cli/src/tamber_serve.rs:489). Open a scrub on broken A while its check runs; let it report failure; externally move to retention key B before another request consumes the result. Frames retaining A’s opening digests take this branch, which calls `failed(A)` without polling. They keep answering `live` indefinitely. Poll before either branch. The regression test masks this by calling `until(rebuild)` before changing keys.

- **BLOCKING — The fingerprint still omits compiler inputs.** [tamber_walk.rs:113](contract/cli/src/tamber_walk.rs:113), [tamber_check.rs:136](contract/cli/src/tamber_check.rs:136). [declaration.rs:113](contract/cli/src/declaration.rs:113) reads every endorsed implementation’s `exact.json`, but implementations without shapes never become graph inputs. Geolocation’s `web/exact.json` is a real example; the read-only `sources` probe confirms its omission. Package files outside the app—or under excluded `node_modules`—escape the additional scan. Editing that declaration during recovery can accept a stale pass; fixing it after a recorded failure can leave `rebuild` permanent. Record these reads as consulted inputs.

- **SHOULD-FIX — Tests still race scheduling and logging.** [tamber_retention_reuse.rs:689](contract/cli/tests/it/tamber_retention_reuse.rs:689). `until(rebuild)` can consume the failure before the worker logs it; `seen("lost")` then returns an older line, and the immediate `"failed"` assertion fails. Separately, observing `began` at [line 636](contract/cli/tests/it/tamber_retention_reuse.rs:636) does not prevent the worker’s delay expiring before the edit. Use explicit synchronization for the race and wait for the particular completion being asserted.

- **NIT — Claims need qualification.** “Never built twice” at [tamber_check.rs:20](contract/cli/src/tamber_check.rs:20) exceeds the 64-completion cache guarantee. The [benchmark samples](/tmp/rl/ret-bench5.txt:2) correctly yield **2747/2754 → 682/672 ms**, but measure `contract-ret6`, which predates `known_under`, rather than final HEAD. They measure reply latency, not completed validation.

What checks out: failures are keyed by retention; ordinary recovery requires a matching completed fingerprint; stable inputs do not loop. One worker processes jobs serially and advances while idle. Compiler state is thread-local; shared interners are synchronized. Answers and close contain no worker join or whole-build lock, and late results are discarded. Unwinding compiler panics become failures; release panics still abort. Close coverage establishes process exit, not late delivery after dropping a checker. Protocol and scrub documentation corrections are otherwise accurate.

Read-only throughout. Diff whitespace checks passed; tests and benchmarks were not rerun.
