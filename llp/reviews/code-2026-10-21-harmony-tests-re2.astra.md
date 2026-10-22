# Code review: Harmony's web tests against a Snapback server of their own (scripts/memory-backend-test.mjs, the async lane), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `76e17c8fb9`.
- **Method:** one brief (sha256 `807076c19a2a550284d0c940c9f97be288fcab068d8b93aa30e5473a5b1b3e68`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

One blocker remains: **[P1] Cleanup can signal unrelated processes after PID reuse** in [scripts/memory-backend-test.mjs](scripts/memory-backend-test.mjs:80).

The grace timer and post-exit sweep reuse recorded numeric PIDs without verifying process identity. If a descendant exits and its PID is reassigned, `kill(pid, 0)` accepts the replacement, and `tree.flatMap(descendants)` can also collect its children. The delayed `SIGKILL` can therefore terminate processes outside this run. Cleanup needs ownership that survives PID reuse.

The walk correctly orders the captured descendants deepest first. Immediate promise observation and the null-stdout guard address the reported spawn-failure issues.

Static review only; no edits, tests, servers, installs, or network commands run.
