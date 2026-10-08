# visual-parity-followup: live session record (2026-10-08)

Machine: macOS 26 (Darwin 25.6.0), Bun 1.4.2. Viewport 1280x840 (agent driver `--size 1280x840`), window
screenshots at 2x (`screenshot <png> window`). No real input was used: every row was driven in agent mode,
so the shared real-input lock (held by the real-input batch) was not needed or taken.

## Lane

- Ports 16520 (standalone reference server) and 16521 (the app's embedded server), in this task's range
  16520-16549. The two never ran at the same time; they share one lane T3 home. (`stage-runtime.mjs --offline`
  runs its own smoke start on 16900, as for every task.)
- Isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_* and `T3_LOCAL_HOME` under the worktree's ignored
  `target/lane/`; `T3CODE_TELEMETRY_ENABLED=false`. Never port 3773, `~/.t3` or the `t3code` scheme.
- Server: the pinned runtime `t3 v0.0.46-nightly.20261005.2667` (stage-runtime.mjs, offline cache).
- Fixture project `target/lane/project` (git repo, one commit), added with `t3 project add --base-dir`.
- Saved device host (row 2): `deviceHosts: [{ id: "lane-mini", label: "Lane Mac mini", target: "lane@lane-mini.invalid" }]`
  in the lane server's `userdata/settings.json` (the setting the Add host dialog writes). The `.invalid` target
  never resolves, so no SSH connection is made.
- A Custom keybinding for the row menu: `appearance.cycle` with `when: terminalFocus` in the lane
  `keybindings.json` (Reset to default and Remove both apply to it). Not reset afterwards: the lane is disposable.
- Fixture provider: Grok enabled with `binaryPath` = a lane-only stand-in (uncommitted, `target/lane/bin/grok`)
  modelled on the reference's `apps/server/src/provider/testFixtures/grok-text-mock-agent.mjs`: `--version`
  (1.0.20, inside the supported range), `models`, `inspect --json`, and an ACP peer whose every prompt is
  answered with the fixture Markdown (the audit's V03 table, a heading, and a second table with inline code
  and a long value). No real provider call.
- Reference: the same server's own web client (the release's T3 Code UI) in a headless Google Chrome driven by
  the repository's `playwright-core`, with a lane-only profile (not the user's Chrome), 1280x840 at 2x, paired
  once through the server's printed pairing link (not recorded). The fixture thread was created there
  (Grok mock, "Show the surface check tables."). This is #250's reference method; it does not use the
  desktop oracle tool.

## Sessions

1. Reference (Chrome, server on 16520, 03:43-04:22Z, in passes while no app ran): Storage, Source Control
   (merge method, writing style), Project (Workspace, model picker), Keybindings row Actions menu and Command
   menu, Integrations > Device hosts row options menu, Scheduled Tasks > New task > Workspace, Typography
   light/dark with Advanced off/on, fixture tables light/dark expanded/collapsed. Server stopped by its PID.
2. Before (03:52-03:58Z, app PID 38872): the never-edited evidence worktree checked out at `07dcef1ab` under its
   build lock, built, copied to `target/vpf/apps/T3 Code (Lane VPF before).app`, lock released. Transcript:
   `ops-before.txt`.
3. After, attempt 1 (04:12-04:17Z, app PID 60431): this branch before its last two edits (TraitsPicker label,
   SettingsSelect `value`). Every flow ran; the Project page model picker was also used to write and reset a
   project override (`projectSettingsOverrides` read back from the lane settings file). Transcript:
   `ops-after-attempt1.txt` (one read refused: `state modelTarget` is not a view).
4. After, the one retry (04:23-04:27Z, app PID 51583): the final tree of this branch, the whole flow again;
   these are the "after" panels. Transcript: `ops-after.txt`.

The agent's REPL is a copy of `scripts/agent.mjs` under `target/vpf/drive/` (uncommitted) that reads further
ops from files, so one launch serves a whole session. Each app quit at the end of its session; no lane server
was left listening.
