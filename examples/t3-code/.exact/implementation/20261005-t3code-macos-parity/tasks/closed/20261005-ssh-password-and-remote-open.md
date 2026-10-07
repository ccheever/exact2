---
name: 20261005-ssh-password-and-remote-open
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: open
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-ssh-password-and-remote-open
pr_url: https://github.com/ccheever/exact2/pull/157
verified_commit: 762f501cb82f933fa00dd72146934d4ae37fcc8a
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
   The field is a native secure field until X35 confirms what the macOS host does with Contract `type="password"` ([X35](../../issues/closed/20261005-x35-secure-text-entry.md); native view in the style of `t3-key-recorder`, `T3Module.swift` `views`); it must clear its
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

Parent specification: [spec](../../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`);
agent runs never reach a real host: discovery reads only `T3_SSH_HOME` and connecting needs the `T3_SSH_COMMAND` double (`T3Ssh.swift:11-12`).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (dialog structure), accessibility (names, keyboard path, focus), design (all states), state-and-data, testing-and-debugging. Child processes, native views,
`NSWorkspace` and the login-shell `PATH` are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis. Consumer framework revision: the pin from `20261005-clone-on-exact2-main`.
Port changes for headers: `Effect` services become plain functions; the prompt service becomes a Swift queue with an expiry clock passed in (`now`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | [#147](https://github.com/ccheever/exact2/pull/147) | Merged into feature integration | Merged 2026-10-06, `7f692c9a1`; split sources present in this checkout |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | none | Merged | Task remains planned/unverified; direct native builds and app checks are recorded below, without claiming this prerequisite merged |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | none | Merged | Still planned/unverified as reusable tooling; this task now has a direct pinned Electron oracle for SSH prompt/input evidence (linked below) |
| scheduling preference | After `20261005-environment-routes` | [#148](https://github.com/ccheever/exact2/pull/148) | Shared route call sites | Merged 2026-10-06, `01f4cbb0a`; route code present, both merges are included after the final feature-base merge |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X35](../../issues/closed/20261005-x35-secure-text-entry.md) | Secure text entry in Contract | not in the library | unknown (workaround: native secure field in the module; result equals the reference) | check on the pin at `prepare` 2026-10-07: #134 closed by main #167; the native field stays, because a Contract field would carry the password through Contract state and the data module, which #167 leaves outside its guarantee (adopt-main-fixes-input). |
| [X26](../../issues/20261005-x26-app-menu-control.md) | The editor menu is a native menu | existing menus | nonblocking | none |

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

Partial implementation on `feat(example)/t3-code-ssh-password-and-remote-open`; task remains in progress and verification unverified. Existing tests and fixture drives do not establish complete acceptance.

- **Auth-attempt wrapper** (`modules/apple/T3SshAuth.swift`, `T3Ssh.swift` `withAuth`): every launch, tunnel and pairing run starts with `BatchMode=yes`; an auth failure (`isAuthFailure`, the reference's three patterns) asks for a password, two prompts at most, then runs again with `BatchMode=no`, `SSH_ASKPASS=<t3code-ssh-runtime-*/t3code-ssh-askpass/ssh-askpass.sh>` (0700), `SSH_ASKPASS_REQUIRE=force`, `T3_SSH_AUTH_SECRET`, `DISPLAY=t3code` when unset. The secret is kept in memory per connection key for later runs and dropped on failure; it never enters argv, a file, Keychain, defaults or logs. Messages: "SSH authentication cancelled for <destination>.", "SSH authentication timed out for <destination>.", "SSH authentication was cancelled because the app window closed." (module `destroy`). Without a prompt service (the AppKit test binary) the refusal is final with ssh's own message, as in the reference (`handleSshAuthFailure` returns the error before `promptForPassword`, so the reference's "SSH authentication failed for <host>." is unreachable there too).
- **Queue and dialog** (`T3SshPrompts`, `ssh-auth.ts`, `ssh-prompt.contract`, root registration in `app.contract`): FIFO, one dialog at a time, 3-minute expiry (agent-only seam `T3_SSH_PROMPT_TIMEOUT_MS`); title, body with the target, the prompt line, `m:ss` countdown → "Expired", hint ↔ error text, Cancel/Dismiss, Continue disabled while responding or expired, Enter submits, Escape and an outside press cancel, no close button, no motion. The field is the module's native secure field `t3-ssh-password` (X35, upstream #134): masked, copy and cut refused, accessible name = the prompt, focused and selected on open, cleared on dismiss; Continue asks the module to read it, so the password never enters TypeScript or Contract state. Focus returns to the previous first responder when the field goes away (best effort).
- **Remote Open** (`editors.ts`, `remote-open.ts`, `shell-details.ts/.contract`): `resolveRemoteOpenState` (an SSH environment's alias from the saved SSH targets, else the server's `remoteOpenTargets`, else unavailable; a loopback environment stands in for the primary), `buildRemoteOpenUrl`, the probed remote-capable editors with the `["vscode"]` fallback, the one-time hint (kept by the module per device), `shouldShowOpenInPicker`, `canUseMarkdownFileShellActions`. The details card's Open row: "Open in <editor>" from the effective editors, "No SSH route to <label>", "No installed editors found", the hint until the first accepted open; remote opens go to the OS, never `shell.openInEditor` on the other machine. The open-favorite key is offered only where the picker shows (`keyboard-dispatch.ts`) and opens remote-aware (`palette-commands.ts`); the Files surface's open goes the same way (`r4-surfaces-files.ts`).
- **Probe and safe open** (`modules/apple/T3RemoteEditors.swift`): `probeRemoteEditors` over the login shell's PATH and `~/Applications` + `/Applications` bundle CLIs; `safeExternalUrl` (port of `parseSafeExternalUrl`) gates `NSWorkspace.open`; agent runs record the URL (`T3_REMOTE_OPEN_LOG`) and open nothing.

Current differences and remaining acceptance are recorded in the attended follow-up below.
The Markdown caller and Files editor list are now implemented. Earlier fixture-only and
not-run statements in the historical attempt table describe those attempts, not the
current evidence. Full acceptance remains unverified.

## Follow-up handoff: incomplete acceptance (2026-10-06)

The user explicitly requires this task to remain active until the missing implementation
and runtime acceptance are handled. Resume this ticket and PR #157; do not treat the
existing code, a green test suite, or a merge as completion. The plan and AGENT-HANDOFF.md
link here so a later agent can pick up the same unfinished work.

Evidence boundary: prior macOS drives used `fake-ssh.sh`. Remote Open in agent mode
records a URL and returns success without calling `NSWorkspace.open`. Neither proves
real SSH password authentication or an editor opening the requested remote project.
On `ea407aa162d883e5a1c6583f7d4edc3f9935bc4c`, the follow-up audit reran
`bun test examples/t3-code/remote-open.test.ts examples/t3-code/ssh-auth.test.ts examples/t3-code/settings-b-ssh.test.ts`:
36 passed, 0 failed. This is unit-test evidence only; the larger check counts below
are historical reports, not a fresh end-to-end verification.

Next agent checklist (historical composite rows; current follow-up below):

- [ ] Reconcile the current feature-branch base and the dependency table with merged PRs;
  record the exact app/reference commits and actual oracle availability before testing.
- [ ] Complete or resolve the missing Markdown editor-action caller and the Files editor
  list sourced from the server. Compare the relevant reference callers, implement the
  required behavior, and test local, SSH, advertised-host and unavailable routes.
  If a requirement is excluded, record the user's explicit scope decision here.
- [ ] Use an authorized password-accepting SSH test host in a normal, isolated lane launch
  with `T3_LOCAL_HOME` and `T3_LOCAL_PORT`; retain the agent mode's fake-host isolation.
  Verify wrong/right passwords, the two-prompt limit, connect/reconnect, cached-secret
  reuse and invalidation. Record observable server/tunnel outcomes without secrets.
- [ ] In that normal launch, use a real installed remote-capable editor and verify the
  requested host and project actually open. A recorded URL or successful OS dispatch
  alone is insufficient. Verify local Open and unavailable-route behavior too.
- [ ] Drive Cancel, Escape/outside dismiss, FIFO queue, live countdown/expiry and window
  close; confirm disabled/error states and focus restoration. AppKit unit coverage
  alone does not close the app-interaction rows.
- [ ] Check real Tab order, Enter/Escape, copy/cut refusal, masked accessibility values,
  field clearing and Korean input-source behavior against the reference. Audit logs
  and app storage for the test secret without publishing the secret or credentials.
- [ ] Complete bounded functional checks at 1280×840 and 840×620 in light/dark mode,
  plus the required oracle/trace comparison when its dependency is available. Respect
  the handoff's user stop on pixel-perfect fix loops; do not restart those loops.
- [ ] Re-run the key-only fixture on a free lane port, and investigate/verify tunnel
  cleanup after app exit. Record whether the previous root-ok proxy limitation still
  applies; a proxy-only result is not proof of an unmodified real server working.
- [ ] Run the applicable tests, strict TypeScript, Contract build and native build/drive
  on the final code. Record commands, revision, observed outcomes and evidence links
  per acceptance row, then update this ticket, the handoff, plan and PR together.

If a host, authorized credentials, interactive session, editor or oracle is unavailable,
record that specific dependency and the next action; leave its checkbox open. Do other
independent work first. Do not replace missing evidence with an inferred pass.
Only mark implementation complete and verification verified, with `verified_commit` set,
when every required acceptance row is evidenced or explicitly descoped by the user.
Until then keep PR #157 described as partial and not ready for final acceptance.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implementation) | `02157028d` (+ this record) on `feat(example)/t3-code-ssh-password-and-remote-open`, rebased on `da40e6590`, then on `eb259ef28` (PR #157) | `bun test examples/t3-code` 1352 pass / 0 fail (base 1321; after the second rebase 1390 / 0); strict `tsc` clean; `contract build` 2187 slots, 44 resources, 48396 nodes (2209 slots, 48733 nodes after the second rebase); `cargo test -p t3-code-macos --lib` 10/0; AppKit `ssh` 14/0 (live tunnel test through `fake-ssh.sh` included, 0 skipped); every other AppKit binary passes (activity 7, attach 3, composer 45, composer-files 4, contextmenu 11, fleet 8, intent 4, menus 10, notifications 4, r10-connect 5, r10-device 4, r11-device 3, r11-upstream 3, r12-sidebar 3, r5-composer 3, r5-panels 8, r6-device 3, r6-media 5, r7-device 13, …); `bun scripts/caps.mjs` within caps; the five checks: build, test (2926 pass / 0 fail), clippy, fmt, caps, boot all exit 0; macOS bundle build exit 0. | logs in `target/` and the session scratchpad (not committed) | — |
| 2 (live drives, ADDENDUM 3/7) | same | Drive 1 (before + after): fixture error, `bin.mjs serve` ignores the bootstrap flag so the wizard stopped at an empty import step; nothing of this task ran. Drive 2: before (base `da40e6590`) shows "devbox: Permission denied (publickey,password,keyboard-interactive)."; after: `01 dialog: ssh-password-title "SSH Password Required", ssh-password-prompt "Enter the SSH password for devbox.", ssh-password-countdown "3:00", ssh-password-hint "Use SSH keys to avoid repeated password prompts on new SSH sessions."`; double log `batch refused devbox`, then after a wrong and a right password `password refused devbox, password accepted devbox ×3` (launch, tunnel and pairing: the secret reused, no third prompt); toast "Environment connected · devbox is ready over an SSH-managed tunnel."; with the loopback environment switched off the details card's menu lists `details-editor-vscode, details-editor-zed, details-editor-remote-hint` ("Opens over SSH. Needs your key on <label>"); VS Code recorded `vscode://vscode-remote/ssh-remote+devbox/<lane>/repo-devbox`; the reopened menu has no hint; 4096 log lines, 0 mention the password. The second SSH host of the fixture (keybox) did not connect in either drive: its proxy port 16115 was taken by another program. | composed before/after images in the PR | — |

Not run: the attended rows (real copy, Tab order with a real keyboard, IME, a real password host), oracle and trace-diff pairs, 840×620 and dark mode, the expiry and window-close flows live (covered by the AppKit `ssh` tests).

## Next action

`prepare` after the three merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `ssh` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.

### Follow-up integration checks, 2026-10-06

Merged the current `origin/feat(example)/t3-code`; the sole conflict was the
`shell-details.ts` imports. Retained both remote Open and thread automations.
`~/.bun-1.4.2/bin/bun test examples/t3-code`: 1,452 pass, 1 skip, 0 fail.
The skipped row is grammar regeneration. The default PATH's Bun 1.3.14 produced
one HTML highlighting failure; use the repository-pinned Bun 1.4.2.
Focused SSH/remote Open/details/automation tests: 83 pass, 0 fail.
Strict TypeScript, Contract build (2,222 slots, 44 resources, 48,923 nodes),
staged caps, boot and `git diff --cached --check` passed.
No native app rebuild, attended session, real SSH host or actual editor launch
was performed in this follow-up. These checks resolve integration confidence,
not the open acceptance checklist above.


### Attended follow-up, 2026-10-06 (PR #157)

Base for this work: `6d41ae81194b707a66d01049f411351d900c2dbc`. Reference source:
`1e2ecbd975`. Final code remains **in progress / unverified** because the entire
acceptance matrix is not closed. No framework source change is included.

Implementation added:

- Files now uses installed local remote-capable editors for an SSH route and renders
  the hint and unavailable state. The local route retains its server editor list.
- Markdown file-link context menus offer editor/reveal actions only on resolved local
  routes; remote routes retain path copying. Actions recheck the route after the menu.
- The password overlay makes the background inert. Tab stays within field → Cancel →
  Continue; Escape is bound inside the dialog. Cancelled Add Environment attempts
  restore focus to the Add environment control after the prompt and busy state end.
- A normal app-termination observer in T3Ssh cleans up pending prompts, tunnel processes,
  cached secrets and askpass files when the host skips deferred session teardown.
  See the [reproduced framework issue draft](../../issues/20261006-native-module-termination.md).

The user authorized an isolated local SSH fixture. The normal native module ran with
`EXACT_AGENT=live` (not agent mode), a real wall clock, isolated app bundle identity and
named storage. A wrapper only supplied `ssh -F <fixture config>`; authentication used
`/usr/bin/ssh` against actual OpenSSH in Docker on loopback port 16257. The unmodified
pinned T3 server and its built web assets served HTTP 200; the old root-ok proxy was
removed. The prescribed T3_LOCAL_HOME/T3_LOCAL_PORT launch remains unavailable because
its embedded-server-runtime prerequisite is still planned. This fixture is evidence
for the tested flows, not a claim that the embedded runtime contract is implemented.

| Check | Observed result | Local evidence under `target/ssh-live/` |
| --- | --- | --- |
| Password authentication | Wrong/right sequence, two-prompt limit, Cancel, reconnect and empty retry field exercised; actual sshd accepted the password | `real-ssh-connected.png`, `real-ssh-retry.png`, matching AX text |
| Cache | One answer authenticated launch/tunnel/pairing; after server password rotation the cached secret failed, a new prompt appeared, and the replacement password connected | sshd Accepted/Failed password entries; no secret printed |
| Real editor | OS dispatch opened VS Code Remote SSH on the fixture project; README content visible | `real-editor-project.png`, `real-editor-project-ax.txt` |
| Secure input | Masking, copy/cut refusal, Enter, Tab and Escape exercised with native keyboard input; AppKit field-editor test also verifies clearing between requests | AppKit log, native AX captures |
| Expiry | Actual three-minute wait; Expired, error hint, disabled input/Continue, Dismiss; no shortened timeout | `real-expiry.png`, `real-expiry-ax.txt` |
| Size/theme | Dialog controls visible at 1280×840 and 840×620 in light and dark | `prompt-{light,dark}-{1280,840}.png` and AX text; no oracle comparison |
| Focus restoration | After native Escape cancellation, focus returned to Add environment | `focus-restored-ax.txt` |
| Window close | With prompt open and two live SSH children, ⌘W ended the app and both children; final app-only workaround, no host patch | `app-cleanup-before.json`, `app-cleanup-after.json` |
| Key-only SSH | Real generated key route connected without password; native SSH suite ran against that fixture | `appkit-ssh-real-key.log`, `final-appkit.log` |
| Secret audit | 80 product-storage/log/AX files read with zero matches for all three rotated secrets; recent ExactMac unified log also zero matches | `secret-audit.json`; no assertion of a forensic Keychain audit |
| Unit/build checks | Bun 1,457 pass / 1 skip / 0 fail; strict TS clean; native SSH 15 pass / 0 skip; Contract and native bundle build passed | `final-bun.log`, `final-tsc.log`, `final-appkit.log`, `build-final.log` |

The repository build/test/clippy/fmt/boot checks passed during this follow-up (use pinned
Bun 1.4.2; system Bun 1.3.14 causes an unrelated grammar test failure). Final Rust app tests passed (10 tests); staged caps and final diff checks are recorded at close-out. Screenshot/log paths above
are local ignored artifacts, not committed attachments or durable remote evidence.

At that checkpoint, the oracle/IME comparison, concurrent prompt transitions, local
and missing-route Open, Files/Markdown interactions, and dependency reconciliation
were still open. See the current follow-up below for rows now evidenced. Unit tests cover routing and
menu gating but do not close those runtime rows. Keep PR #157 draft. The earlier
unchecked checklist remains the full acceptance contract; this table records partial
progress without treating composite rows as passed.


Final integration: merged feature base `587798ca8c0f5c2c761a4219263edce8256d50d8`
after the attended fixes in `ef29fccda`. Resolved six shared-file conflicts while keeping
SSH password and terminal native views, the complete inert password backdrop, route
management, Files diff comments and both framework-gap records. Remote Open now selects
the SSH route from the environment's saved route list even when the active/preferred
route is a direct URL; a new test covers this precedence over advertised hosts.
Post-merge Bun: 1,848 pass / 1 skip / 0 fail; strict TypeScript clean; native SSH suite:
15 pass / 0 skip; terminal assets built; boot and caps passed. The final diff against
the feature base passes whitespace checks. The base itself contains whitespace in
committed terminal evidence logs; those unrelated evidence files were preserved.


Post-merge native drive on `87246167d`: rebuilt and launched the app, authenticated
with the real password host again, opened the Files surface and read the remote README.
Its editor menu listed local Cursor, VS Code and Zed (`merged-files-editors.png` and AX
text). Closing the window terminated both recorded SSH children (`merged-cleanup-*.json`).
The Files menu initially let Escape close the whole panel; a follow-up adds its own
Escape dismissal. Markdown native context actions and actual local/unavailable-route
UI outcomes remain unverified. Post-merge Rust app tests: 10 passed.


Files Escape close-out: the third bounded fix/drive passed. The panel's Escape shortcut
is disabled while its editor menu is open; the menu backdrop handles Escape. Native
Escape closed only the menu, kept the Files surface visible, and kept focus on Choose
editor (`files-escape-fixed.png`, `files-escape-fixed-ax.txt`). Earlier attempts that only
added a competing shortcut or guarded the launcher's key action did not work and are
not retained. The final bundle built successfully (`files-escape-third-build.log`).
Closing that app ended its three SSH children (`final-cleanup-before.json`,
`final-cleanup-after.json`). The verification container was stopped. The final source
suite remains 1,848 pass / 1 skip; staged caps and PR-diff whitespace pass. This closes
Files menu rendering/dismissal evidence, not the remaining local/remote editor-action,
Markdown context-menu, oracle/IME and full concurrent prompt acceptance rows.


## Current native follow-up, 2026-10-06

[Durable fixture evidence](../../evidence/20261005-ssh-password-and-remote-open/README.md)
records the later native drive. These items narrow the historical composite
checklist above; implementation and verification status remain unchanged.

- [x] Observe simultaneous password requests in FIFO order: first target with one
  queued request, then second target with none queued and an empty secure field.
- [x] Observe Continue disabled while responding: both visible loading samples
  were disabled. The sample summary records the polling limits.
- [x] Drive the remote Markdown context menu and copy relative path. The menu
  exposes only copy actions, and the clipboard assertion matches `README.md`.
- [x] Drive the local Markdown context menu and Reveal in Finder. The menu exposes
  editor/reveal actions, and Finder selects the fixture `README.md`.
- [x] Rebuild and drive the SSH-alias follow-up change on `762f501cb`: the saved
  SSH conversation now offers enabled Open and local remote-capable editors.
- [x] Verify actual remote project Open in VS Code and the Files remote OS handoff
  with the requested alias/path; no Files document-content outcome is inferred.
- [x] Verify local Open using the offered Finder entry and successful backend RPC;
  the fixture README is selected in Finder.
- [x] Verify missing-route UI through an authorized authenticated HTTPS fixture: details and Files Open are disabled; menus explain the missing SSH route.
- [x] Compare Korean input-source behavior with the pinned desktop oracle: both
  secure fields select ABC when focused. Preserve the first-to-second FIFO comparison.
- [x] Reconcile dependency status: #142 is merged and included; direct feature oracle
  evidence exists, while the generic oracle/trace and clone-on-main tasks remain
  planned/unverified. This does not waive their broader acceptance requirements.
- [x] Record final acceptance evidence and prepare PR #157 for review.

The recorded FIFO `afterCancel` sample still has the second prompt open; no claim
of a drained queue is made. A local editor menu item is not evidence of an editor
launch. Existing successful checks need rerunning only for later affected changes.


### Durable historical evidence and scope reconciliation

The [evidence record](../../evidence/20261005-ssh-password-and-remote-open/README.md#pinned-oracle-and-earlier-acceptance-evidence)
now retains the already performed pinned Electron IME/FIFO comparison, native
three-minute expiry and actual VS Code remote-project README, rather than rerunning
those checks. Oracle `remainingDialogs: 1` is retained without a drained-queue claim.
The alias fix has 40 passing focused tests, strict TypeScript and caps, plus its
rebuilt native drive. These supersede the earlier statements that IME and all oracle
work were unperformed; the old attempts remain historical records.

Merge `bd59d77d7` incorporates #142's `9670b0723` while preserving both the SSH
alias and standard-scope imports. #142's [acceptance record](../../evidence/20261005-remote-scopes-and-update-commands/20261006-repair-and-trace/README.md)
distinguishes feature acceptance from full T0/RPC equality. This task likewise
records only its observed SSH/remote-open behavior; no full parent trace or reusable
oracle-tooling completion is claimed. The dependency table retains that distinction.


## Final acceptance, 2026-10-06

Implementation `762f501cb82f933fa00dd72146934d4ae37fcc8a` passed the remaining
feature checks; subsequent commits contain evidence and tracking only. The native
HTTPS fixture closes the last missing-route row, including Details, Files and the
Open shortcut. See the durable evidence record for screenshots and bounded claims.
The earlier in-progress/Draft instructions describe superseded checkpoints.

The feature is implemented and verified; PR #157 is being marked Ready under the
user's explicit instruction. It is not merged. Generic parent tooling and the full
T0 matrix remain separately tracked and are not claimed complete by this task.
