# settings-model-picker: live macOS sessions (2026-10-08)

Branch build: `e9fbc8ef4` sources (dev build `bun host/apple/build.mjs t3-code-macos`, agent mode, 1280x840).
Lane: `target/smp/lane` in the task worktree; isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*,
T3_LOCAL_HOME / T3CODE_HOME, T3_LOCAL_PORT 16810, T3CODE_TELEMETRY_ENABLED=false. No real accounts.

Provider catalog (lane fixture, read from the lane server's provider status cache on port 16811):
- Codex instance: `binaryPath` = lane symlink to the system codex (0.151.0), `homePath` = lane CODEX_HOME
  holding an API-key auth file with a placeholder key -> status ready, auth authenticated, 5 models
  (gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna, gpt-5.5, gpt-5.2; all `isLegacy`).
- Claude instance: `binaryPath` = lane symlink to the Claude CLI (2.1.293), instance environment
  ANTHROPIC_API_KEY = placeholder, CLAUDE_CONFIG_DIR and HOME = lane dirs -> status ready, auth
  authenticated, 12 models (claude-opus-5-5, claude-sonnet-5-5, claude-fable-5-1, claude-opus-5,
  claude-sonnet-5 current; 7 legacy). The probe uses SDK initialization only; no prompt is sent.
- Every other driver keeps its default (disabled).

| Session | Started (UTC) | App pid | Result |
| --- | --- | --- | --- |
| 1 | 18:20:05 | 32470 | Stopped by me after 90 s: the development build has no bundled `t3-runtime`, so "This machine" never started (`T3_LOCAL_RUNTIME_DIR` was not set). No flow ran. |
| 2 (retry) | 18:22:22 | 44724 | Server started in 2 s (pid 44882, port 16810; providers Codex and Claude available after 3 s). The drive script took the still-inert main window for the decided first-run state and every tap was refused ("view is hidden or inert"); the session closed at 18:22:29. No flow ran. Embedded server stopped with the app (pid 44882 gone). |

Both sessions failed in the drive's setup, before any task flow. The task's live rows need one more
session; the drive script (`target/smp/drive.mjs`, uncommitted) now sets `T3_LOCAL_RUNTIME_DIR` and
waits for the first-run decision before tapping.
