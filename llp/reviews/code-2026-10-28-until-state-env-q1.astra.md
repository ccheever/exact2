# Code review: until-state.test restores EXACT_APP_DIR (main red), quick review, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4d84986e21`.
- **Method:** a narrow brief naming the earlier findings and their fixes; a quick review (a mechanical main-red fix, test isolation). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Its two NITs (setup outside the cleanup, arm's timers) were taken.

No BLOCKER/SHOULD. The environment restore is correct. Two pre-existing cleanup NITs:

- **NIT — [until-state.test.mjs:29](host/web-js/until-state.test.mjs:29):** Fixture writes occur before `try`. A write failure leaves the peer server and temporary directory uncleaned. Move setup inside the cleanup scope.
- **NIT — [until-state.test.mjs:26](host/web-js/until-state.test.mjs:26):** `arm()` discards its timeout handle. An early failure after tapping can leave the callback pending beyond teardown. Retain and clear pending timers in `finally`.

No other matching `runTests({ app })` leak found: JS live-reads already restores the variable; wasm live-reads passes no app; `host/web/agent.test.mjs` invokes it in a subprocess.

Read-only stub execution confirmed restoration for unset, empty, and populated values, including thrown runs, and reproduced both cleanup gaps. Browser tests were not rerun.
