# settings-model-picker: approved live session (2026-10-08, screen locked)

The coordinator approved one more session (base build for the before images, then the branch build), with no retry.
The Mac's screen was locked for the whole session (`CGSSessionScreenIsLocked = 1`, user away). Both launches ran in
agent mode (`scripts/agent.mjs` `open()`, macOS, 1280x840) from `target/smp/drive.mjs`.

Lane: `target/smp/lane`. Isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3_LOCAL_HOME/T3CODE_HOME, and
T3_LOCAL_RUNTIME_DIR (the staged t3 0.0.46-nightly runtime, unpacked). T3_LOCAL_PORT is 16812 for the base and 16810
for the branch. T3CODE_TELEMETRY_ENABLED=false.

Provider catalog: placeholder API keys, no real account, no prompt sent.
- Codex: the auth file holds a placeholder API key. Status `ready`, 5 models (all legacy).
- Claude: a placeholder `ANTHROPIC_API_KEY` in the instance environment. Status `ready`, 12 models.
- Antigravity is enabled but not installed. It reports `supportsTextGeneration: false`, which makes it the Text generation row's
  unsupported-provider fixture.

| Launch | UTC | App pid | Server pid | Read back | Result |
| --- | --- | --- | --- | --- | --- |
| Base `da4f4512f` (evidence-base, build lock held) | 01:07:48-01:08:13 | 48607 | 48858 (port 16812) | Codex and Claude available after 1.6 s | Every tap was refused for 20 s (`view 97 is hidden or inert`). Every capture is one uniform colour (blank). No before image |
| Branch `e9fbc8ef4` | 01:09:25-01:10:13 | 54699 | 54736 (port 16810) | Settings opened at 0.56 s (`close-settings` present) and closed. Codex and Claude available after 1.1 s. Composer model `codex\|gpt-5.6-sol` | Afterwards every tap was refused for 40 s (`hidden or inert`). Every capture is blank. No row flow ran |

Both apps and both embedded servers exited with the session; ports 16810 and 16812 are free. A blank capture comes from
the view's own picture (`Capture.picture`), not the window server. With the screen locked, the logs cannot show
whether the cause is the lock or a window-wide modal, because no wizard node showed.

Result: every live row and every UI image is deferred to the real-input batch (screen locked, user away). The steps are in the
task record under "Real-input batch steps".
