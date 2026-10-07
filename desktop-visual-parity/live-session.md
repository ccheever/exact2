# desktop-visual-parity: live session record (2026-10-08)

Machine: macOS 26 (Darwin 25.6.0), Xcode 27, Bun 1.4.2. Viewport 1280x840 (agent driver, `--size 1280x840`), window screenshots at 2x.

## Lane

- Ports 16510 (standalone reference server) and 16511 (the app's embedded server), both in this task's range 16500-16599.
- Isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_* and T3_LOCAL_HOME under the worktree's ignored `target/lane/`;
  `T3CODE_TELEMETRY_ENABLED=false`. Never port 3773, `~/.t3` or the `t3code` scheme.
- Server: the pinned official runtime `t3 v0.0.46-nightly.20261005.2667` (stage-runtime.mjs, offline cache).
- Fixture project `target/lane/project` (git repo, one commit) added offline with `t3 project add --base-dir`.
- Fixture provider: Grok enabled in the lane `settings.json` with `binaryPath` pointing at a lane-only stand-in
  (uncommitted): `--version`, `models`, and an ACP peer modelled on the reference's
  `apps/server/src/provider/testFixtures/grok-text-mock-agent.mjs` that answers every prompt with the fixture Markdown
  (the audit's V03 table, a heading, and a second table with inline code and a long value). No real provider call.
- Reference: the same server's own web client (the release's T3 Code UI) in Chrome, paired with the server's
  printed pairing link, 1280-wide window. Thread created there: "Show the surface check tables." -> the mock's reply.

## Sessions

1. Reference (Chrome, server on 16510): captures `ref-typography-light.png`, `ref-keybindings-new.png`,
   `ref-scheduled-new.png`, `ref-thread-tables-light.png`. Server stopped (its PID only) before the app sessions.
2. After, attempt 1: the agent driver launched the development standalone binary, which carries no
   `Contents/Resources/t3-runtime`; the embedded server reported `runtime-missing` and the window stayed empty.
   Closed without any flow run.
3. After, attempt 2 (the one retry): `EXACT_MAC_BIN` = the bundle's binary
   (`T3 Code (Exact).app/Contents/MacOS/T3 Code (Exact)`), `T3_LOCAL_HOME`/`T3_LOCAL_PORT=16511`. The embedded server
   unpacked the runtime, started (`state: ready`) and the app opened the shell with the lane project and thread.
   Flow: Settings > Appearance > Typography (light, dark, Advanced on, prompt 18 px + code 16 px, restored), Keybindings
   (+, Command menu, long command selected, When editor's Condition menu, cancelled unsaved), Scheduled Tasks > New task
   (Workspace menu, Cancel), the fixture thread (expanded, collapsed, dark, interface font 18 px, restored to 16 px).
   Transcript: `ops-after.txt` (2 refused ops, retried by view id: "Back" named two views; a thread row tapped while
   Settings covered it).
4. Before: the never-edited base worktree at `da4f4512f`, rebuilt under its build lock, same lane home and port
   (runtime already unpacked), same flow. Transcript: `ops-before.txt`.

Nothing was saved: no keybinding, scheduled task or preference change survives (sizes restored, drafts cancelled).
The plain `screenshot <png>` op returned a uniform white image for this app on both builds; every capture uses
`screenshot <png> window`, which shows the window's content.
