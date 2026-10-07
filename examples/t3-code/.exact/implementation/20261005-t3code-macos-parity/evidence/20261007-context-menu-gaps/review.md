# Independent review, 2026-10-07

Reviewer: a separate agent given the staged diff (base `4f523ef5c`), the ticket, the parent spec and
the reference sources (T3 Code `1e2ecbd975`: fileContextMenu.ts, FileBrowserPanel.tsx,
pullRequestLinkContextMenu.ts, PullRequestRow.tsx, PullRequestDetailPanel.tsx, ChatMarkdown.tsx,
fileExplorerLabel.ts, editorLabels.ts, editorPreferences.ts, ElectronMenu buildTemplate); not told an
expected verdict. It ran `bun test examples/t3-code` (2288 pass, 1 skip, 0 fail).

Verdict: PASS, no blocking findings. Items, order, submenu and disabled rules of the three menus match
the reference; no framework file changed. A right-click on the PR row's number reaches the inner box's
`contextmenu` and never fires the row's `press` (the macOS host sends `pointerdown` only).

| # | Finding (non-blocking) | Resolution |
| --- | --- | --- |
| 1 | "Preview media" opens the Files surface's media preview, not the reference's expanded media dialog; media outside the workspace may differ | Declared: the clone has no expanded dialog for a reply's file links (a click on the link already opens the Files surface); follow-up |
| 2 | The detail header's number went through `prAct`, skipped while `commandPending` | Fixed: it goes through the window's `chatLocal` (`pr-link-menu`), always available |
| 3 | A missing provider read "Open on GitHub"; the reference says "Open on host" | Fixed: no default provider |
| 4 | "Preview media" is gated on a thread being open, not on the link's own thread | Kept: reply links only exist in a thread |
| 5 | The contract wiring (`contextmenu=`) has no automated test | Covered by the live drive for the Files tree; the PR rows need a GitHub fixture (task `20261005-fake-github-fixture`) |
| 6 | `remote-open.test.ts` fixture now carries the server's reveal kind | Correct: without a kind or a platform OS the reference reads "Reveal in Files" |
| 7 | "<provider> <url>" as one value; `terminalOpenExternal` opens http(s) only | Fine as is |

Runner: attempt 1 passed before findings 2 and 3; attempt 2 passed with `source_unchanged: true`
(`attempt2-report.json`), and the staged tree matches it (`--compare-report`).
