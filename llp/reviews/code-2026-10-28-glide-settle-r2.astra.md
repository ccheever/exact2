# Code review: web clock settle waits for a glide, round 2, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fa9691b7cf`.
- **Method:** one brief (sha256 `ea477371267f65efe72c0568331b17320ea4f9d391be868cfc8b2f219c643d62`), blind to the other reviewer; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [host/web-js/agent.js:459](host/web-js/agent.js:459), [host/web/glue.js:1228](host/web/glue.js:1228): unrelated work exhausts the scroll deadline.** A request taking over 4 seconds, followed by animation or another settle round, reaches `glidesSettled` with an expired deadline. Its timer then returns false even with **zero scroll events**. Reproduced on both settle implementations with a 4.2-second request: `{ settled: false, reason: "scroll" }`, despite remaining request budget. The cumulative scroll-wait budget must not charge unrelated data waits.

- **BLOCKER — [host/web-js/agent.js:462](host/web-js/agent.js:462), [host/web/glue.js:1232](host/web/glue.js:1232): scrolling started during a round escapes the check.** Glide quiescence is checked before requests land and the round advances. A request completion that starts a non-virtualized scroller can therefore reach `settled: true` without another glide check. Reproduced on both implementations; the JS case returned true with one scroll event observed, increasing to five 80 ms later. Revalidate quiescence after the round’s work before accepting success.

Verified using the imported helper and checked-in settle bodies with controlled scheduling. Browser suites were not rerun: this read-only checkout lacks dependencies and build outputs. No files changed.
