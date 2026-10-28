# Code review: the clone's live reads on a local server, round 2, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `653c5c66c2`.
- **Method:** one brief (sha256 `2e7c52bdb25d350edc5a491bc00c66c0b4b0e8620511252c0e7bc398347e5170`), blind to the other reviewer; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** The brief also covered the clone's split (live-split 32377a3).

No BLOCKER found. Two SHOULDs and one NIT remain:

- **SHOULD — [scripts/live-reads-fixture.mjs:257](scripts/live-reads-fixture.mjs:257): re-arming still permits false passes.** After the first `arm()`, requests automatically release after 300 ms. A slow input or reload can consume that interval before `atStep`; the response has then left `withheld`, so re-arming cannot recover it. Delay between `arm()` and clock execution can also consume the interval. An in-memory timer probe confirmed both cases. Release should depend on the clock actually starting, with non-completion checked while the answer remains withheld.

- **SHOULD — [clone/app.test.contract:40](split/app.test.contract:40): the butterfly assertion still depends on the live answer.** A successful `{feed: []}` produces a `loading` row, unlike the non-loading error row. Consequently `homeShownHasPosts` and `latestOn` remain false, the local poll never arms, and `newPages.0` cannot equal `"discover"`. Confirmed using the clone’s stub network and actual feed functions. This is a remaining dependency for theme2’s ruling; stub the initial feed or include this case in the live split.

- **NIT — [scripts/live-reads-fixture.mjs:266](scripts/live-reads-fixture.mjs:266): partial app creation still leaks its directory.** If a subsequent `writeFileSync` throws, `liveReadsApp()` never returns its directory, leaving the caller’s `dir` undefined. Its `finally` therefore skips removal. The helper needs cleanup around its writes.

No additional deadlock found in the normal gate/arm sequence, failure run, or stale-request handling. `before data` is used correctly. The wasm recipe is consistent when build and test ports match.

Read-only probes and whitespace checks passed. Browser suites were not rerun; this checkout lacks dependencies and build artifacts.
