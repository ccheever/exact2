---
name: 20261005-ssh-password-and-remote-open
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# SSH password prompt and Open in a local editor for remote environments

## Outcome

Adding or reconnecting an SSH environment whose host needs a password asks for it in the reference's "SSH Password Required" dialog and passes it to the local `ssh`
for that attempt only. For an environment that lives on another machine, "Open" in the details card hands the OS a deep link that opens the project in the user's
local editor (VS Code, Cursor, Zed, …) over SSH, offers only the editors that are installed, and says so when there is no SSH route.

## Scope and exclusions

Included (clone rows missing; evidence below; line numbers are from the mc-orch tree on 2026-10-05):

1. **Auth-attempt wrapper (Swift).** Clone: every `ssh` run in `modules/apple/T3Ssh.swift` uses `BatchMode=yes` (`baseArgs`, line 198) and a failure is final (`connect(_:pair:)`, line 207; `run`, line 310).
   Reference (`packages/ssh/src/tunnel.ts:1375-1470`, `runWithSshAuthAttempt`): first run with `BatchMode=yes`; on an auth failure (`isSshAuthFailure`, `packages/ssh/src/auth.ts:205-215`) ask for a password,
   at most two prompts, then run again with `SSH_ASKPASS=<script>`, `SSH_ASKPASS_REQUIRE=force`, `T3_SSH_AUTH_SECRET=<password>`, `DISPLAY=t3code` when unset (`buildSshChildEnvironment`, `auth.ts:173-203`); keep the secret in
   memory for later runs of the same target; drop it on failure. The helper is `ASKPASS_POSIX_SCRIPT` (`auth.ts:74-84`, mode 0700, in a fresh `t3code-ssh-runtime-*` temp folder, `auth.ts:102-109`). The secret goes in the child's
   environment, never in argv, disk, Keychain, `defaults` or logs. `T3Ssh` runs on a background queue, so waiting for the answer blocks that worker, not the UI; expiry 3 minutes
   (`apps/desktop/src/ssh/DesktopSshPasswordPrompts.ts:18`). Messages: "SSH authentication cancelled for <host>.", "SSH authentication failed for <host>." when no prompt service exists (agent runs), "SSH authentication was
   cancelled because the app window closed." Tests: `packages/ssh/src/auth.test.ts` ("detects ssh auth failures from common permission denied messages", "creates askpass env for cached password prompts") as bun tests on the ported pure
   functions, and the attempt loop against the clone's `T3_SSH_COMMAND` double.
2. **Prompt queue and dialog (`apps/web/src/components/desktop/SshPasswordPromptDialog.tsx`).** FIFO queue, one dialog at a time. Title "SSH Password Required"; body "T3 needs your SSH password to connect to `<user@host>`. The
   password is passed to the local SSH process for this connection attempt and is not saved by T3 Code."; the request's prompt line ("Enter the SSH password for <host>."); a `m:ss` countdown that turns into "Expired"; a password
   field (focused and selected when the dialog opens, `autocomplete=current-password`); hint "Use SSH keys to avoid repeated password prompts on new SSH sessions." which becomes the error text when a response fails or expires
   ("This SSH password prompt expired. Try connecting again."); buttons Cancel ("Dismiss" when expired) and Continue (disabled while responding or expired); Enter submits; Escape or the outside click cancels (answers none). No close button.
   The field is a native secure field until X35 confirms what the macOS host does with Contract `type="password"` ([X35](../issues/20261005-x35-secure-text-entry.md); native view in the style of `t3-key-recorder`, `T3Module.swift` `views`); it must clear its
   memory on dismiss and refuse copy. The field's accessible name is the request's prompt text.
3. **Remote Open (TS).** Port `apps/web/src/remoteOpen.ts`: `resolveRemoteOpenState` (modes `local-exec`, `remote-links`, `remote-unavailable`; a primary or desktop-local environment stays `local-exec`; otherwise the SSH alias of the
   environment's SSH route, else the server's first advertised `remoteOpenTargets` entry, else unavailable), the cached `useRemoteCapableEditors` logic with the `["vscode"]` fallback, and the one-time hint state. Port
   `EDITORS` with `remoteScheme`, `REMOTE_CAPABLE_EDITOR_IDS`, `remoteSchemeForEditor`, `buildRemoteOpenUrl` (`packages/contracts/src/editor.ts`: `<scheme>://vscode-remote/ssh-remote+<host><path>`, Zed `zed://ssh/<host><path>`).
   With the routes model (`20261005-environment-routes`) read the alias from the SSH route, not from the entry's first route. The clone always runs `shell.openInEditor` on the server host today (`shell-details.ts` `EDITORS`, `preferredEditor`;
   `shell-details.contract` editor row); change that row to the reference's `OpenInPicker.tsx` behaviour: primary label "Open" (panel form "Open in <editor>"), menu items for the effective editors, "No installed editors found",
   disabled "No SSH route to <label>", disabled hint "Opens over SSH. Needs your key on <label>" until the first remote open fires, and no open when unavailable. The reference reads the state in three places, and each must follow it: `shouldShowOpenInPicker` (`components/chat/OpenInPicker.logic.ts`: the picker shows for the primary, and for other
   environments only outside `local-exec`), the open-favorite shortcut, which is enabled only when the picker shows (`ChatView.tsx` `useOpenFavoriteEditorShortcut`), and Markdown file links, which get editor actions only in `local-exec`
   (`canUseMarkdownFileShellActions`, `ChatMarkdown.tsx`). Other clone callers of `shell.openInEditor` (palette, Files, diagnostics) are compared with the oracle and keep the reference's behaviour; find them by symbol.
4. **Probe and safe open (Swift).** `probeRemoteEditors`: for each remote-capable editor, true when one of its commands resolves on the login-shell `PATH` or at the macOS app locations of `resolveEditorCommand`
   (`packages/shared/src/editor.ts:22-100`, macOS branch: `~/Applications` and `/Applications`, `Contents/Resources/app/bin/<command>` or the app's `Contents/MacOS/cli` for Zed). Port `parseSafeExternalUrl`
   (`apps/desktop/src/electron/ElectronShell.ts:11-52`): only `http(s)` and the remote editor deep links with no user info, host `vscode-remote` and path `/ssh-remote+…` (Zed: host `ssh`, path `/<host>/…`) reach
   `NSWorkspace.open`; everything else returns false. Tests: `ElectronShell.test.ts` ("opens safe external URLs", "opens remote SSH editor URLs", "opens Zed's ssh deep link", "does not open editor URLs that mix up link shapes",
   "does not open remote editor URLs with userinfo", "does not open unsafe external URLs", "does not open non-remote editor URLs", "returns false when Electron rejects openExternal") and `remoteOpen.test.ts` (12 cases, e.g.
   "prefers the desktop SSH alias over server-advertised hosts", "reports unavailable when a remote environment advertises no hosts"). The reference has no test for the probe itself: add "lists only remote-capable editors that resolve"
   and "falls back to VS Code when none resolve".

Excluded: the Browser surface, Full Disk Access (Browser only), the SSH discovery and tunnel code that works today, T3 Connect, WSL.

## Context and guidance

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`);
agent runs never reach a real host: discovery reads only `T3_SSH_HOME` and connecting needs the `T3_SSH_COMMAND` double (`T3Ssh.swift:11-12`).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (dialog structure), accessibility (names, keyboard path, focus), design (all states), state-and-data, testing-and-debugging. Child processes, native views,
`NSWorkspace` and the login-shell `PATH` are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis. Consumer framework revision: the pin from `20261005-clone-on-exact2-main`.
Port changes for headers: `Effect` services become plain functions; the prompt service becomes a Swift queue with an expiry clock passed in (`now`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-environment-routes` | none | Shared `T3Ssh.swift` and `environmentKey` call sites; remote Open reads the SSH alias from the route list | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X35](../issues/20261005-x35-secure-text-entry.md) | Secure text entry in Contract | not in the library | unknown (workaround: native secure field in the module; result equals the reference) | check on the pin at `prepare` |
| [X26](../issues/20261005-x26-app-menu-control.md) | The editor menu is a native menu | existing menus | nonblocking | none |

## Implementation notes

- New native files `T3SshAuth.swift` (askpass, queue, expiry) and `T3RemoteEditors.swift` (probe, safe open); edit `T3Ssh.swift` in place; keep `T3Module.swift` to op prefixes.
- The prompt is a root-level overlay so a queued request works from any page; it must not steal focus from an open dialog before it appears and returns focus afterwards.
- Remote Open logs no host names and copies nothing.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test` (ssh auth, remote open, safe URL, probe); Swift `ssh` binary for the attempt loop | Original names pass | macOS | logs |
| Password flow | `T3_SSH_COMMAND` double that refuses until `T3_SSH_AUTH_SECRET` is right | Add an SSH environment: wrong password, then right; cancel; let it expire (agent-only expiry seam); close the window mid-prompt | Dialog text and countdown; two prompts at most; secret reused for the next run of that target and dropped after a failure; the three messages; no secret in logs, defaults, Keychain or files (`grep -r` of the data folder and `log show`) | macOS 1280×840 and 840×620, light and dark | transcript, png pairs vs `target/t3-ui-parity/electron-oracle.mjs` |
| Dialog states | Same | Open; type; Enter; Tab; Escape; second request while the first is open; expired | Loading (Continue disabled while responding), error text replaces the hint, "Dismiss" when expired, queue shows the next dialog after the first ends; field focused and selected on open; Tab order field, Cancel, Continue; Escape cancels and answers none; focus returns to the control that started the connect; no motion in the dialog, so reduced motion changes nothing (confirm on the oracle) | macOS | `tree --ax`, screenshots |
| Secure field | Same | Type; try copy and the visible-text accessibility value | Characters masked; copy refused; accessibility value empty or masked | macOS | `tree --ax`, `pbpaste` |
| Remote Open states | A saved remote environment with an SSH route; one with only a bearer route and `remoteOpenTargets`; one with neither; editors on `PATH` or in `/Applications` | Open the details card menu | `vscode://vscode-remote/ssh-remote+<alias>/<path>` handed to `NSWorkspace` (recorded by the test double); "No SSH route to <label>" disabled; the hint until the first open; "No installed editors found" | macOS | `NSWorkspace` log, png |
| Picker visibility | A thread in the primary, in an SSH environment, in a bearer environment without hosts | Open each thread | Picker shown for the primary and for the remote ones; the open-favorite key works only where the picker shows; a Markdown file link offers editor actions only for local-exec environments | macOS | `tree`, state |
| Probe and safe open | Fake application folders and a `PATH` with `code`, `cursor`, `zed` | `probeRemoteEditors` | Only installed remote-capable editors, VS Code fallback when none | macOS | test log |
| Keyboard, focus, Escape | Same menu | Open with the keyboard, arrow through items, Escape | Items reachable and activatable; Escape closes and returns focus; disabled items skipped | macOS | `tree --ax` |
| Real prompt (attended session) | Lane build, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>`; the user's test SSH host that accepts passwords | Add the host | Native `ssh` accepts the password from the dialog. Input-source behavior in the secure field (whether Korean IME composition is available or macOS disables it, as it normally does for secure fields) matches the oracle's behavior, recorded at `prepare` from the reference app before any expectation is written | macOS | recording, oracle recording |

Task-owned source paths: `modules/apple/{T3Ssh,T3SshAuth,T3RemoteEditors}.swift`, `ssh-auth.ts`, `remote-open.ts`, `editors.ts`, `shell-details.ts`, `shell-details.contract` (+ tests), `macos/tests/ssh`.
Required environment: Xcode 27.0, pinned Bun, the oracle build; a Korean input source for the input-source row.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `ssh` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
