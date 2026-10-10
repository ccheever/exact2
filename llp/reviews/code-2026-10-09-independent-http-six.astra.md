# Code review: six independent HTTP workers (6677007d6), 2026-10-09 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4f07671a6f929f9339b69822b7e6ac30110daa96957b69f6463de715c99fd794`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra 1 taken: the ordered-releaser and stream saturation tests hold every independent worker (`INDEPENDENT`). Astra 2 / Grok MINOR and NIT taken: LLP 1041's worker cap, Completion Storm's README, LLP 1054.000's refusal note and LLP 1110's long-poll cost say six workers, 64 MiB and 112. Astra 3 taken: the amendment says six bounds an executor's concurrent independent requests across origins, not connections per origin. `cargo fmt` applied.

---

No BLOCKER or MAJOR findings.

- **MINOR — Saturation tests still assume two workers.** [executor_stream_tests.rs:311](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_stream_tests.rs:311) holds only two requests before checking that a stream opens, leaving four workers free. [executor_tests.rs:138](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_tests.rs:138) similarly weakens the ordered-control isolation test. Both should hold `INDEPENDENT` requests and observe all six entering transport.

- **MINOR — Old limits remain documented.** [LLP 1041:495](/Users/admin/x2p2/apps/exact2-http/llp/1041-graceful-overload.rfc.md:495) still specifies 48 workers, contradicting 112. [Completion Storm README:109](/Users/admin/x2p2/apps/exact2-http/apps/completion-storm/README.md:109) retains 32 MiB/two transports, and its line 121 retains 48 workers. [LLP 1110:379](/Users/admin/x2p2/apps/exact2-http/llp/1110-live-snapback-reads.rfc.md:379) still says two long polls exhaust the independent workers.

- **MINOR — The new connection claim needs qualification.** [LLP 1041:451](/Users/admin/x2p2/apps/exact2-http/llp/1041-graceful-overload.rfc.md:451) says a cold launch opens “up to six connections.” Six bounds simultaneous independent worker requests **per executor**, across origins. Ordered requests, streams, and retained connections are additional; this is not a per-origin connection limit.

The implementation otherwise looks sound. `owners.len()` matches guard releases on successful spawning, partial failure, and retirement; unsuccessful reservation creates no guards. Linux and render include the same core. Ordered execution, settlement ordering, lane alternation, the 128-request limit, stream accounting, and `forget` remain intact.

Threads start eagerly: seven per executor, fourteen for two sessions. Darwin sessions are lazy. The 64 MiB budget is accounting capacity, not preallocated memory; transport, stack, stream, and deadline costs sit outside it.

Read-only validation: 54 focused tests passed using an existing Apple binary; nine were ignored and file-writing tests excluded. No fresh build or performance measurements were performed. Findings above are nonblocking.

READY
