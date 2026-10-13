---
name: 20261014-terminal-links-and-close
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: not-started
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-links-and-close
pr_url:
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
