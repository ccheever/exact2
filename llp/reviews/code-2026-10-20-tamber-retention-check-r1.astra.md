# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 1 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `a3832cc4c2`.
- **Method:** one brief (sha256 `1bdfa912f0b74744d5bb30f98388dcac91ba9ac5405e803fd1a7f66ba72208ca`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (4 BLOCKING, 3 SHOULD-FIX, 1 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

**Round 1: changes needed.** Four blocking correctness gaps:

- **BLOCKING — Lost checks can leave `live` indefinitely.** [tamber_check.rs:133](contract/cli/src/tamber_check.rs:133), [tamber_serve.rs:418](contract/cli/src/tamber_serve.rs:418). Start checking a broken press-scale design, then change only `ink`. The raced/stale result is discarded, while the unchanged retention key takes the cache-hit return without scheduling another check. Main would keep answering `rebuild`. An unwinding worker panic or failed thread spawn similarly leaves the cache trusted.

- **BLOCKING — A → B → A keeps the wrong next job.** [tamber_check.rs:62](contract/cli/src/tamber_check.rs:62). With A running and B queued, requesting A again returns without removing B. `poll` then discards A’s result because `next` exists and runs obsolete B. If A is broken, its failure can disappear permanently.

- **BLOCKING — The fingerprint cannot establish “exactly current inputs.”** [tamber_walk.rs:91](contract/cli/src/tamber_walk.rs:91). It omits strings tables and directory membership, font/sound assets, `.shells` inputs, and separately tracked manifest/resolution facts. For example, corrupt `strings/en.json` after a recovery build reads it: both digest checks still pass, and the result clears unknown although the current app fails compilation. Ordinary package sources/designs, consulted manifests, `app.json`, and discovered icon SVGs are covered.

- **BLOCKING — An open scrub retains obsolete known state.** [tamber_scrub.rs:58](contract/cli/src/tamber_scrub.rs:58), [tamber_scrub.rs:100](contract/cli/src/tamber_scrub.rs:100). Open a scrub while the check is pending; let a subsequent `propose` consume its failure. Further scrub frames—including verification—still classify using cached `base.exprs` and can answer `live`.

Other findings:

- **SHOULD-FIX — Unchanged failures rebuild repeatedly.** [tamber_serve.rs:414](contract/cli/src/tamber_serve.rs:414). Each request consuming a failure immediately starts the identical job again. Deduplication remembers only running/queued jobs. This is a request-driven build loop; remember completed fingerprints.

- **SHOULD-FIX — Queued checks stall while idle.** [tamber_check.rs:146](contract/cli/src/tamber_check.rs:146). Only another request’s `poll` starts the next job. Consequently, [the test’s `reported(2)`](contract/cli/tests/it/tamber_retention_reuse.rs:477) can time out if the initial worker had not finished at the preceding poll.

- **SHOULD-FIX — Tests do not synchronize or observe enough.** [tamber_check.rs:109](contract/cli/src/tamber_check.rs:109) logs before sending the result; seeing a log line does not establish readiness. [The race test’s sleep](contract/cli/tests/it/tamber_retention_reuse.rs:576) establishes no ordering and never checks the subsequent answer. Units miss A → B → A and can hide stale failure acceptance behind the later successful build. Close tests establish process exit, not delivery after dropping a checker.

- **NIT — Update the documentation.** [tamber_check.rs:9](contract/cli/src/tamber_check.rs:9) says “After an answer,” but spawning precedes response generation. [Protocol documentation:684](docs/tamber-protocol.md:684) still describes unknown only as failure before the design.

What checks out: recorded failures use the retention key correctly; ordinary stable recovery works. Background compilation is serialized, the pending slot is bounded, and close never joins the worker. Dropping the receiver safely discards later delivery. Compiler state is thread-local; shared kernel interners use mutexes, although their capacity and contention remain shared. Release-profile panics abort the entire process.

The recorded benchmark medians are arithmetically correct; they measure reply latency, without waiting for current-input validation. Diff whitespace checks passed and the tree is clean. No files changed; repository tests and benchmarks were not run.
