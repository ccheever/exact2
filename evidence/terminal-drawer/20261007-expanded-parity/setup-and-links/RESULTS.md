# Worktree setup and editor links, 2026-10-07

Actual normal macOS app, physical CG mouse/key input, disposable backend 16331, harmless native-fixture/default provider. No login, installation, or real provider request.

## D04: final real-app consumer passes

A fresh normal app copied from the completed worktree-fix build created a new worktree through the GUI and sent one harmless fixture prompt. The real server setup script was `printf D04_SETUP_STARTED; sleep 45; exit 1`, in terminal `setup-native-setup-proof`. After the 45-second script completed, “Worktree ready, setup script failed” and **Open terminal** appeared automatically, with no navigation away/back. Physical activation revealed the existing setup transcript. Hide/reopen retained the same transcript, and the thread still had only its one setup PTY log. [Automatic card](d04-final-card.png), [opened terminal](d04-final-open.png), [reopened terminal](d04-final-reopened.png), [sanitized proof](d04-final-proof.json).

During the first 15 seconds, the fast fake agent had already completed and the running setup card was absent. This matches the pinned original's `setupHandedOff` placement rule (`MessagesTimeline.logic.ts:1683–1693`): a running asynchronous setup is hidden after agent handoff, while a finished failed setup remains visible. It is not a remaining live-card gap for this fixture.

Historical evidence [card](d04-card.png) and [drawer](d04-open.png) predates the subscription fix: that run needed navigation away/back after completion. The fresh run supersedes that final-delivery failure. The fixture's project script override was cleared through `server.updateSettings` after verification; provider settings were unchanged.

## C06: physical activation and routing pass; external editor caret unverified

The setup shell printed `./link-fixture.txt:7:4` for a committed 14-line fixture. Command-click underlined and activated the path. The real server resolved the preferred Cursor target to the actual worktree file including `:7:4`, then its spawn/unref and RPC spans returned Success. No Cursor window appeared. One bounded fallback selected VS Code through the app editor menu and repeated Command-click: the server resolved vscode and the same positioned path, its spawn/unref and RPC spans returned Success, but the existing VS Code window did not change to the fixture. The original Cursor choice was restored afterward.

[Visible link](c06-link-output.png), [activated link](c06-cg-after.png), [sanitized server routing](editor-route.json). These observations prove actual GUI link activation plus cwd/line/column/editor routing, but **do not prove editor caret placement or successful external application display**. Spawn/unref success alone is insufficient: the pinned server ignores child stdout/stderr and does not wait for the CLI exit or window creation. The external editor outcome remains unresolved.

### Headless launch-path investigation

The pinned original uses the same API shape: `ThreadTerminalDrawer.tsx` resolves the positioned target and `editorPreferences.ts` sends `{cwd: targetPath, editor}`. On the server, `process/externalLauncher.ts:515` treats that field as the target; `resolveCommandEditorArgs` produces `--goto <path>:7:4` for these editors. `launchEditorProcess` does **not** set the spawn process cwd to that positioned path, so a file-as-cwd ENOTDIR explanation is ruled out for this path.

The linked spans in `editor-route.json` include the actual `externalLauncher.launchAndUnref`, `externalLauncher.launchEditorProcess`, and `ws.rpc.shell.openInEditor` Success exits, not just command resolution. `launchAndUnref` waits for spawn/unref but not process completion; child stdout/stderr are ignored. Both installed CLI wrappers returned exit 0 and their versions under the disposable HOME when invoked with `--version`. No matching launch failure was found in the server log. These checks do not identify why a visible editor failed to appear, and no GUI or editor launch was attempted during this headless investigation.
