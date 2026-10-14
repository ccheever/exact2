---
name: 20261014-terminal-links-and-close
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: agent-verified (real-input steps pending)
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-links-and-close
pr_url: https://github.com/ccheever/exact2/pull/531
verified_commit:
---

# Terminal links in the wizard and the drawer, the close confirm, the paste refusal and the sign-in terminal's focus

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences (STATUS "Known
differences", the `realinput-1011c` tables); the coordinator's triage (fix wave, round 13 base `609f392d5`) put these
five rows in this task. The user decided on 2026-10-11 that each is built now, matching the reference T3 Code.

| Id | Clone (`realinput-1011c` B1) | Reference (check it first) |
| --- | --- | --- |
| G1-10 | In the welcome wizard's setup terminal (Welcome › Agents › Sign in / Install) a click or ⌘-click on an http link opens nothing, and a path link calls no editor; the link message the terminal emits reaches `pages-welcome.ts` and is dropped ([pair](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-10c-out-pair.png)) | The wizard's `AgentInstallTerminal` is the drawer's `TerminalViewport`, so its links go through the drawer's `handleLinkActivate`: the system browser once per click (plain or ⌘), a path to the preferred editor once (`cursor --goto <cwd>/./README.md:1`) |
| G1-5 | With Settings › Integrations › Open links in = T3 Code, a drawer terminal link still opens the system browser ([hits](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/G1-5-link-hits.txt)) | `openTerminalLinkInPreview`: a plain click opens a Browser tab beside the thread at the link ("Lane link"); ⌘-click the system browser |
| G1-4 | A second ⌘W while a terminal close confirm is pending opens the next close dialog ("Close 2 terminals?", the right panel's Terminal surface) once the first is dismissed; after Escape on the confirm the toolbar's Close opened, the focus goes to the terminal ([pair](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-4d-pair.png)) | While any terminal close confirm is pending, ChatView's window key handler refuses the terminal close chord (`isTerminalCloseConfirmPending() && preventTerminalCloseShortcut`): one dialog, nothing after it; Escape gives the focus back to the button that opened it (Base UI's finalFocus: the toolbar's "Close Terminal (⌘W)") |
| G1-7 | A 70,000-character paste is refused with nothing sent (as the reference) but reads `[terminal] Expected a value with a length of at most 65536` and a stair-stepped `at ["data"]` line (a bare line feed) ([pair](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-7b-pair.png)) | `[terminal] Schema validation failed` on its own line: the client encodes `TerminalWriteInput` (`data` at most 65,536) before sending, and effect's sync encode throws that message |
| G1-11 | The provider sign-in terminal (Settings › Providers › Account › Sign in) takes the focus as it opens ([pair](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-11-pair.png)) | `ProviderAuthTerminal` creates its surface without focusing it (hollow cursor; the focus stays outside) |

## Steps

1. Reference first: the source below, and the `realinput-1011c` B1 notes
   (`target/t3-ui-parity/realinput-1011c/b1-2set/00-b1-notes.txt`, G1-4, G1-5, G1-7, G1-10, G1-11).
2. Build this branch's tip before any change and drive it once (agent mode, the lane's fixture): the "before" half.
3. G1-10: the wizard's terminal messages (`WelcomeSetup`'s `message`, `welcomeLocal` `terminal-exited`) send a `link`
   to the drawer's `terminalLinkAction` with the setup session's cwd and its environment's editors and server.
4. G1-5: trace `linkTargetPreference(client)` and `terminalLinkAction`'s target after an "Open links in" change; fix
   what keeps the link from `openTerminalLinkInPreview`.
5. G1-4: the keyboard dispatch refuses the terminal close chord app-wide while a terminal close confirm is pending;
   the toolbar's Close gives the focus back to itself on Cancel / Escape.
6. G1-7: validate `terminal.write`'s `data` before sending (`Schema validation failed`); system messages written with
   CRLF line ends.
7. G1-11: the sign-in terminal is not focused at mount.
8. Tests; one agent drive at the end (before | after | reference per scenario); real-input steps for the final session.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G1-10 | Bun: an onboarding link message produces the drawer's action (http click and ⌘-click: `terminalOpenExternal` once each; a path: `shell.openInEditor` once with the session's cwd); agent drive (the wizard's Sign in terminal, a printed link tapped) | `T3_REMOTE_OPEN_LOG` / module log before and after |
| G1-5 | Bun: preference "app" set at runtime through `rest:browser-link-target`, then a drawer http link goes to `openTerminalLinkInPreview` (a `preview.open`, no system browser); agent drive | before / after (Browser tab) images |
| G1-4 | Bun: with a terminal close confirm pending the dispatch's ⌘W is the refusal row (no `rightPanel.close`, no `terminal.close`); agent drive: two terminal surfaces, ⌘W, ⌘W, Escape: one confirm, none after; the toolbar's Close, Escape: the focus on that Close (`state.focus`) | before / after images and `state.focus` |
| G1-7 | Bun / AppKit: the refusal string and CRLF in system lines; agent drive: a 70,000-character paste | before / after images |
| G1-11 | contract and agent drive: the sign-in terminal opens without the focus (`state.focus`) | before / after `state.focus` |
| All five under real input | real-input steps for the final session (below) | — |

## Reference

- G1-10: `apps/web/src/components/onboarding/WelcomeWizard.tsx` `AgentInstallTerminal` (:985-1130: `TerminalViewport`
  with the onboarding `threadRef`, `cwd`, `autoFocus`); `apps/web/src/components/ThreadTerminalDrawer.tsx`
  `handleLinkActivate` (:783-826), `useOpenInPreferredEditor(environmentId, …)` (:359).
- G1-5: `apps/web/src/components/preview/openTerminalLinkInPreview.ts`; `apps/web/src/browser/browserLinkTarget.ts`.
- G1-4: `apps/web/src/lib/terminalCloseConfirm.ts`, `apps/web/src/lib/terminalCloseShortcut.ts`,
  `apps/web/src/components/ChatView.tsx` (:7584-7593, the window key handler); `packages/shared/src/keybindings.ts`
  (`mod+w` → `terminal.close` when `terminalFocus`, `rightPanel.close` when `!terminalFocus`).
- G1-7: `packages/contracts/src/terminal.ts` `TerminalWriteInput` (`data`: non-empty, `isMaxLength(65_536)`);
  effect 4.0.0-rc.115 `SchemaParser.ts` (:122, "Schema validation failed"); `ThreadTerminalDrawer.tsx`
  `writeSystemMessage` (:107) and `handleData`.
- G1-11: `apps/web/src/components/settings/ProviderAuthTerminal.tsx` (no `focus()` on the surface).

## Cause and fix

**G1-10.** The wizard's `WelcomeSetup` passes every message of its `t3-terminal` to `welcomeLocal`'s `terminal-exited`, which
handled only `exited` / `closed`; the link message the view emits was dropped. The reference's `AgentInstallTerminal` is
the drawer's `TerminalViewport`, so its links take the drawer's `handleLinkActivate`. Fix (`pages-welcome.ts`): a `link`
of the current setup session goes to the drawer's `terminalLinkAction` with the onboarding thread, the setup's cwd (a
path resolves against it, `<cwd>/./README.md:1` as the reference's `cursor --goto`) and, for a setup on a background
environment, that environment's editors and server (`TerminalLinkEnvironment`; the request helper is the setup terminal's
own, now `environmentRequest`). An http(s) click or ⌘-click is one `terminalOpenExternal`.

**G1-5.** Traced, not reproduced: Settings › Integrations' `rest:browser-link-target` writes `client.local.clientSettings`,
which the drawer's next link reads (`resolveLinkTargetPreference`); a Bun test drives the runtime change through
`restCommand` and the link through `terminalOps` (`preview.open`, no system browser), and the agent drive on the base
(`609f392d5`) and on this branch both open the link in a "Lane link" Browser tab (one hit with WebKit's UA on the lane
link server, no Chrome). The only path from "T3 Code" to the system browser left is `openTerminalLinkInPreview`'s
fallback after a failed in-app open, which the clone dropped silently; the reference logs it (`console.error(new
TerminalLinkPreviewOpenError(…))`). Change: the drawer keeps that error with its cause as its `failure` (the clone's
place for what the reference logs; the agent's `state` shows it), so a repeat names its cause. Real-input step 2 checks
it again.

**G1-4.** (d) The clone's close-pending guard covered a focused terminal (`T3TerminalView` `close-pending`, the page's
key policy); a ⌘W while the confirm holds the focus goes to the window's dispatch buttons, where ⌘W was
`rightPanel.close` (a Terminal surface: its close confirm queued behind the first, `realinput-1011c`) or the same
terminal close again. Fix: `keyboardDispatch` gets the window's `confirmOp`; while it is a terminal close (the drawer's,
a panel terminal's, a Terminal surface's), every chord `terminal.close` wins with a terminal focused is one refusal row
(kind `refuse`, its press a no-op window op), so it neither runs nor falls through to the window's Close, as ChatView's
`isTerminalCloseConfirmPending() && preventTerminalCloseShortcut`; and `confirmUi` marks a terminal close asked while
the confirm was open as handled when it ends. (a) The toolbar's Close (and a tab row's close) asks with `ui:confirm:<its
element id>`; Cancel / Escape gives the focus back to that element (Base UI's finalFocus), a ⌘W-opened confirm still to
the terminal.

**G1-7.** The clone sent every write and showed the server's schema message; the reference's client encodes
`TerminalWriteInput` (`data` at most 65,536) first and its sync encode throws "Schema validation failed", nothing sent.
Fix (`T3TerminalSessions.write`): over 65,536 UTF-16 units it writes `[terminal] Schema validation failed` and sends
nothing. `systemMessage` writes the message's line ends as CRLF, so no line stair-steps.
Normalizing embedded newlines is the task's explicit CRLF cleanup. The reference wraps system messages in CRLF
but preserves their embedded line endings; full multiline parity is not claimed for that normalization.

**G1-11.** `providers-setup.contract` gave the sign-in terminal `focus-request=1`; `ProviderAuthTerminal` never focuses
its surface. Now `0`. The wizard's setup terminal keeps `1` (`TerminalViewport autoFocus focusRequestId={1}`).

**Supporting change.** An agent run records a terminal's system-browser open (`terminalOpenExternal`) in
`T3_REMOTE_OPEN_LOG` instead of opening it, as `remoteEditorsOpen` already does (`T3RemoteEditors.openOrRecord`), so a
drive reads what a link did and opens no browser.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| G1-10 | pass (Bun, agent drive): in the wizard's Claude Install terminal (the curl line typed, never run) a click and a ⌘-click on its https link: before nothing, after one `terminalOpenExternal` each (`opened-urls.txt`); a path (Bun) one `shell.openInEditor` at `<cwd>/./README.md:1`, the drawer's action for the same messages | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f3cfb93e69996292ae8b0f238999dab6ddd420b3/terminal-links-and-close/drive-record.txt) |
| G1-5 | not reproduced: the runtime preference reaches the drawer link (Bun) and the base and this branch open a Browser tab under the agent; a failed in-app open is now kept as the drawer's failure. Under real input: open, step 2 | [image](https://raw.githubusercontent.com/ccheever/exact2/181782d0e627f4b7e64bde9f071b2ea9c25ee1db/terminal-links-and-close/g15-link-in-app.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f3cfb93e69996292ae8b0f238999dab6ddd420b3/terminal-links-and-close/drive-record.txt) |
| G1-4 | pass (Bun, agent drive): while the confirm is open the ⌘W row is the refusal (before: the close again; `realinput-1011c`: `rightPanel.close`), none after Escape; the toolbar's Close then Escape: focus on "Close Terminal (⌘W)" (`terminal-close-drawer`), before the terminal | [image](https://raw.githubusercontent.com/ccheever/exact2/23ca331f1edfab464231130ee7f1875456c47dfc/terminal-links-and-close/g14a-focus-return.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f3cfb93e69996292ae8b0f238999dab6ddd420b3/terminal-links-and-close/drive-record.txt) |
| G1-7 | pass (AppKit, agent drive): a 70,000-character ⌘V reads `[terminal] Schema validation failed`, nothing sent; a two-line system message starts each line at the left edge | [image](https://raw.githubusercontent.com/ccheever/exact2/1f42465cea91ab7a21fb800f47cc3f8e7d3db67b/terminal-links-and-close/g17-paste-refusal.png) |
| G1-11 | pass (agent drive): at open `state.focus` before the sign-in terminal, after none; the hollow cursor | [image](https://raw.githubusercontent.com/ccheever/exact2/4084fa630b1eaa16f91ee19df4c7e2132b1f9bd9/terminal-links-and-close/g111-signin-focus.png) |
| All five under real input | open: the real-input batch steps below | — |

Drives (agent mode, 1280×840, [scripts](https://raw.githubusercontent.com/ccheever/exact2/22867a1409391ddbc7fa4abe31f4dc43df71c3af/terminal-links-and-close/drives.sh.txt)): before on this worktree's build of `609f392d5`, after on
this branch's bundle (`1e4513680`'s app code); the lane fixture homes are `target/t3-audit/lanes/terminal-links-and-close[-wiz][-before]`
(absent Claude and Codex binaries, a lane ACP stand-in for the sign-in), the clone server on 16402, a lane link server
on 16405. The reference column is `realinput-1011c`'s real reference windows.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout), terminal shells
with `PROMPT='$ '` and the editor and `codex` stand-ins first on PATH, and a lane link server, as `realinput-1011c`'s
g1 lanes (`prep/g1/f3-standins.sh`). Never send a message; never run an installer.

1. **G1-10.** A fresh lane (no projects, no completed onboarding): Welcome › Continue › Agents › Codex "Use existing
   CLI" › Sign in; check the pre-typed line is the stand-in's `codex login`; Return. Click its `http://127.0.0.1:<lane>/lane-signin?n=welcome`
   (expected: the system browser, one hit), ⌘-click it (a second hit), click `./README.md:1` (one editor stand-in call
   `cursor --classic --goto <cwd>/./README.md:1`). Return, paste `exit` + Return.
2. **G1-5.** Settings › Integrations › Open links in = T3 Code; the drawer terminal `printf 'web http://127.0.0.1:<lane>/lane-link?n=app\n'`;
   click the link with no key held. Expected: a "Lane link" Browser tab in the right panel at that URL, a hit with
   WebKit's UA, no Chrome. ⌘-click: the system browser. If the system browser opens on the plain click, note whether a
   Browser tab appeared and which keys the input tool held. Set the row back to "Your default browser".
3. **G1-4.** The drawer's Terminal 1 and a right-panel Terminal surface open; click in the drawer terminal; ⌘W (one
   confirm, Cancel focused); ⌘W again 0.8 s later (nothing); Escape: no second dialog, the window open. Then the
   drawer toolbar's trash ("Close Terminal (⌘W)"), Escape: the focus ring on that button (AX focus `Close Terminal (⌘W)`).
4. **G1-7.** 70,000 characters on the pasteboard, ⌘V at an empty drawer prompt: `[terminal] Schema validation failed`
   on its own line, nothing sent, no stair-stepped line; restore the pasteboard.
5. **G1-11.** Settings › Providers › ACP Registry (the lane stand-in) › Account › Sign in: the terminal's cursor is
   hollow and the AX focus is outside it; a click in it focuses it.

## Tests

- `terminal-links-and-close.test.ts` (new, 10): G1-10, the wizard's link messages through `welcomeLocal` (http click
  and ⌘-click: `terminalOpenExternal` once each; a path: `shell.openInEditor` once at the setup cwd; another session's
  link ignored; the exit still closes), the drawer's action for the same messages, a background environment's editors and
  server; G1-5, `rest:browser-link-target` then a drawer link through `terminalOps` (`preview.open`, ⌘-click the system
  browser), a failed in-app open kept as the drawer's failure and one system-browser open; G1-4, the ⌘W row with the
  focus off and on a terminal, with and without a pending confirm, which confirms count, and the contracts' focus
  return; G1-11, the sign-in terminal's `focus-request=0` and the wizard's `1`.
- `dialog-focus.test.ts`: the terminal's askClose assertions follow `ui:confirm:<element>`.
- `macos/tests/terminal` (AppKit) `testAnOversizedWriteIsRefusedBeforeSendingAndSystemLinesStartAtTheLeftEdge`: 65,537
  refused with the reference's text, 65,536 sent on, a two-line system message, `crlf`.

## Checks

Earlier verification on application code `1e4513680` (before the review test repair below):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4866 pass, 1 skip, 0 fail (329 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6471 slots, 48 resources; `app.contract` 1,460 lines |
| AppKit `macos/tests/terminal` (README recipe) | 0 | 39 run, 1 skipped (`T3_TERMINAL_SCALE`), 0 failed; the new G1-7 case passes |
| `cargo test -p t3-code-macos --lib` | not run | no Rust changed (the Swift module is the AppKit test's) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed; the first run (beside the Bun suite) failed `exact-apple`'s wall-clock `a_parked_upload_starts_when_its_bytes_drain_and_is_not_crowded_out` ("not within five seconds"), which passed alone and in the rerun; no framework file changed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once before the after drive (`1e4513680`) |

## Review repair, 2026-10-14

The earlier background-environment test called `terminalLinkAction` directly with a supplied environment. It did
not cover how the welcome wizard selects the setup session's configuration and routes its request. Replaced that
test with the complete `welcomeView` / `welcomeLocal` flow: enter the wizard, pair a second environment, open its
setup terminal, then deliver its path-link message and close report. The focused environment offers Cursor at
`/Users/lane/work`; the setup environment offers Zed at `/remote/work`. The test requires `shell.openInEditor` to
reach the second environment's fleet transport and generation with `/remote/work/src/app.ts:3` and `zed`, and
requires no RPC on the focused connection. Fleet and primary fixture state are restored after the test.

Focused verification with Bun 1.4.2:

```sh
bun test examples/t3-code/terminal-links-and-close.test.ts \
  examples/t3-code/pages-welcome.test.ts --timeout 60000
```

Result: 17 passed, 0 failed, 78 assertions. This repair changes one test and this record; the application code and
the earlier native-drive evidence are unchanged. G1-5 remains unreproduced under real input, and all real-input
acceptance steps remain open. The source assertions for confirm focus and sign-in focus are supported by the
earlier agent drive; those assertions alone do not establish runtime behavior.

## Not done / not verified

- Real input for all five rows: the screen is locked for this task (agent mode only); the real-input batch steps
  above are for the coordinator's final session.
- G1-5's `realinput-1011c` failure (the system browser with "T3 Code" set) did not reproduce under the agent on the
  base or this branch, and no stale or unread setting was found (the trace above); step 2 re-checks it under real
  input, and a failed in-app open now names its cause in the drawer's failure.

## Next action

The coordinator reviews and merges draft PR #531, then runs the real-input batch steps.

## Coordinator publication, 2026-10-14

Draft PR [#531](https://github.com/ccheever/exact2/pull/531) was read back at `d3f075a20892c469a15549fb5f78cbdbf0dfebf3`, based on `feat(example)/t3-code`. Actual background onboarding environment test; original system-browser discrepancy remains unreproduced. The combined repository gates and app drive are pending. This local integration bookkeeping does not close the task or replace the earlier verification revision.
