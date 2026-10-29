# Code review: the clone's live reads on a local server, narrow re-check 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `05dec78dc8`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD — [scripts/live-reads-fixture.mjs:263](scripts/live-reads-fixture.mjs:263): a slow driver can still hide a skipped wait.** `s.clock('data')` performs `nextCommitTasks → s.state()` after the underlying clock operation returns ([agent.mjs:1099](scripts/agent.mjs:1099)). The wrapper keeps releases enabled during that read. An in-memory probe using the actual driver method reproduced this: a nonwaiting operation returned at 1 ms, the answer arrived at 301 ms, and a delayed state read let `s.clock` return at 401 ms with the expected config available.

Additionally, [release timers](scripts/live-reads-fixture.mjs:228) survive `done()`, allowing answers between steps. Verify that the underlying clock operation remains pending until release, and re-gate outstanding timers when it ends.
