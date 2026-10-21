# Code review: LLP 1121 stage 12 B1 follow-up, the retention build: stop at retention, reuse types and expansion, round 2 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1cd4f1fbd8`.
- **Method:** one brief (sha256 `e6108e9f9a7c120b998c1997b4ec7ace2ee767a0795471f7a51d594d74f2c208`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 SHOULD-FIX, 1 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

No new **BLOCKING** cache divergence found. Two verification gaps remain:

- **SHOULD-FIX — Retention equality still isn’t tested.** [tamber_retention_reuse.rs:25](contract/cli/tests/it/tamber_retention_reuse.rs:25) probes hex-to-hex colour changes, which generally preserve plan shape whether those names are retained. The helper compares responses, never `Retention` or raw candidates. Incorrect retention can therefore pass; both servers also use the same stopped pipeline. Compare retention and candidate pairs directly against an ordinary full compile. This part of round 1 remains unresolved.

- **SHOULD-FIX — VERIFY misses fresh-check rejection.** [lib.rs:992](contract/cli/src/lib.rs:992) returns through `?` before verification if fresh typing/expansion fails. Reuse has already been marked, so the log says `built reused`, without `stale`. A future design-dependent refusal should fail condition (b)’s invariant too. Mark this outcome stale or explicitly fail verification.

- **NIT — Q2 documentation overclaims.** [QUEUE.md:16](QUEUE.md:16) describes the background check as already running; [tamber_retention_reuse.rs:385](contract/cli/tests/it/tamber_retention_reuse.rs:385) broadly says the failure is never hidden. Describe the worker as forthcoming and limit the test’s claim to its kind-changing edit. The commit message correctly acknowledges the deferred regression; I’m not re-reporting it.

What checks out:

- Round-1 guard and parse-fallback fixes are present. The guard restores `PROGRAM` and the previous flag during unwinding; no reachable reentrant compilation path was found.
- Production reuse and VERIFY now both run in the tests, with nonempty candidates, package-header repair, parse recovery, strings, motion arguments, and an early type failure.
- Retention and candidates are fixed before the stop. `params::reached` only contributes diagnostics; earlier passes do not observe the flag. `retention().is_some()` correctly means retention was computed, independently of compilation success.
- The gate covers the merged app/package program, strings, mapping mode, and substituted motion literals. I found no omitted current typing/expansion input. Changed source keys discard `kept`; successful VERIFY comparisons detect differences.

Recorded benchmark medians match the claim: approximately **2.57 s → 0.56 s, 4.6×**, with value edits unchanged.

No files changed. Diff whitespace check passed; tests and benchmarks were not rerun because they write files.
