---
name: 20261011-reply-links-reference-behavior
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-reply-links-reference-behavior
pr_url:
verified_commit:
---

# Reply links as the reference has them: a fragment leaves the thread, a table cell's mailto opens nothing, the ContextMenu key's shell menu

## Outcome

The user decided on 2026-10-11 to match the reference on the three reply-link rows
[reply-links](closed/20261011-reply-links.md) kept (its "Decision", "Not done" and "Seen while building"; `STATUS.md`
"Known differences" rows "A reply's `#fragment` link", "A `mailto:` link in a reply's Markdown table cell" and "The
ContextMenu key (or VoiceOver's show menu) on a focused …").

| Id | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- |
| RL-4 | A click on a reply's `#fragment` link opens nothing and keeps the thread (reply-links RL-2, "kept by choice") | `handleMarkdownFragmentClick` finds no element with that id, the click sets the page's hash, and Electron's hash router (`createHashHistory`) reads it as a route: the app leaves the thread for the new-thread draft |
| RL-5 | A `mailto:` link in a reply's Markdown table cell (`TableCell` → `ChatRuns`, an inline run) has no press, so a click goes to the host's own link following, which hands the URL to the mail app; a press there was measured at +308,280 bytes of plan (accepted by the user: measure it) | Opens nothing (the anchor's `_blank` reaches `setWindowOpenHandler`, which opens only http(s) and remote editor links) |
| RL-6 | The ContextMenu key (or VoiceOver's show menu) on a focused `mailto:`, `irc:`, `xmpp:` or `#fragment` link opens no menu: the link node's `contextmenu` asks for the shell's menu, which answers only a fresh page click (reply-links review R1-4) | Chrome raises `context-menu` at the focused anchor and the shell's menu opens; build it if the host allows it, else record the blocker |

## Steps

1. Reference first (the reference app on this lane, CDP and its main process's inspector; source under
   `target/t3-ref/src-1e2ecbd975`): RL-4's navigation and focus, from prose and from a table cell; RL-5's click on a
   cell's mailto link; RL-6's ContextMenu key on each kind of focused link, the menu's items and where it pops up.
2. RL-4: `browser-links.ts` `openLinkFromUi` opens the most recently active project's draft for a fragment (the index
   route's `IndexDraftLanding`), with the window's new-thread handling (the composer takes the focus); every word of
   the link presses it (`markdown.contract` FlowRuns), and a table cell's fragment link too.
3. RL-5: the table cell's link run gets the link's press without a new handler site (merge `ChatRuns`' web and other
   link branches); measure the plan.
4. RL-6: `T3TextContextMenu` `shellMenu` answers a `contextmenu` no click raised with the shell's menu for the focused
   link; find whether VoiceOver's show menu reaches `contextmenu` at all on the host.
5. Tests (Bun; AppKit `contextmenu`); one agent drive per build with the lane's fixture reply; before / after /
   reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RL-4 | reference comparison first; tests; agent drive (`tap "the notes"`, `tap "the summary"`) | before / after / reference images |
| RL-5 | reference; tests; plan size; agent drive (the cell's irc link before and after, its mailto link after only) | text before / after (logs, tree) and images |
| RL-6 | reference; AppKit test; agent drive (`type "the team" key ContextMenu`) | text before / after (the menu's log line) |
| RL-4..RL-6 under real keys and pointer | real-input steps for the coordinator's session | — |

## Reference

- RL-4: `apps/web/src/components/ChatMarkdown.tsx` (`handleMarkdownFragmentClick`, the anchor's `onClick`),
  `apps/web/src/main.tsx` (`createHashHistory` in Electron), `apps/web/src/routes/_chat.index.tsx` (`IndexDraftLanding`:
  `sortScopedProjectsForSidebar(projects, threads, "updated_at")[0]`, `handleNewThread(…, { replace: true })`).
- RL-5: the same anchor in a table cell; `apps/desktop/src/window/DesktopWindow.ts` (`setWindowOpenHandler`,
  `parseSafeExternalUrl`).
- RL-6: `apps/desktop/src/window/DesktopWindow.ts` `installContextMenu`, `apps/desktop/src/electron/ElectronMenu.ts`
  `popupTemplate`.
