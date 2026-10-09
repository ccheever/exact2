---
name: 20261009-right-panel-launcher-and-files
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Right panel: launcher keys and Device on a draft, the Terminal tab icon, and the Files subheader actions

## Outcome

- The "Open a surface" launcher offers Device on a draft.
- The launcher moves a highlight with the arrow keys and opens the highlighted surface with Enter.
- A Terminal tab shows its icon.
- An HTML or PDF file in Files has "Open file in preview browser".
- An image file has no word-wrap toggle.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PA-1 | On a draft (work › New thread), the launcher lists Device (M) as available, and M opens it (`deviceAvailable={activeThreadRef !== null}`; a draft has a thread ref). | Device is dimmed with the tooltip "Available from a thread.". M is not among the launcher keys (`r4-surfaces-panel.ts:117`: `device: !!client.threadId`). | Select the work project's draft, toggle the right panel, look at the Device row and hover it. | `target/t3-audit/evidence/panel/PA-1-ref.png`, `PA-1-clone.png`, `panel/dumps/r-aria-launcher-draft.txt`, `panel/dumps/clone-drive-02.jsonl` |
| PA-2 | The focused launcher moves a highlight with ArrowDown/ArrowRight and ArrowUp/ArrowLeft over the available rows. Enter opens the highlighted surface. ArrowDown ×3 then Enter opens Files. | Arrow keys show no highlight and Enter does nothing. Only hover highlights a row (`shell-panels.contract:94`: `highlighted=(hovered == surface.id)`), and only letters open surfaces (`R8KeysLauncher.swift`). | On work › Audit work thread, open the right panel (the launcher has the focus). Press ArrowDown three times, then Enter. | `target/t3-audit/evidence/panel/PA-2-ref.png`, `PA-2-clone.png`, `panel/dumps/clone-drive-13.jsonl` |
| PA-10 | A Terminal tab shows the TerminalSquare icon before "Terminal 2". | The tab shows an empty icon slot. The tab icon name `terminal` (`r4-surfaces-panel.ts:280`) does not exist in `ShellIcon`, which has `square-terminal` (`shell-icons.contract:11`). | Right panel › T (Terminal). Look at the tab. | `target/t3-audit/evidence/panel/PA-10-ref.png`, `PA-10-clone.png` |
| PA-4 | For `.html` and `.pdf` files the subheader shows a globe button, "Open file in preview browser", that opens the file in a Browser tab. | The subheader has Open in editor, Show HTML source and Hide file explorer only (`r4-surfaces-files.contract:129-134`). Browser part 1 is merged. | On work › Audit work thread, open Files › `docs/page.html`. Read the subheader buttons. | `target/t3-audit/evidence/panel/PA-4-ref.png`, `PA-4-clone.png`, `panel/dumps/r-aria-html-server.txt` |
| PA-5 | An image file shows the image with Open in editor and Hide file explorer only (`showsRawText`). | The subheader also shows "Disable word wrap", which does nothing for an image (`r4-surfaces-files.contract:131`: the toggle shows for preview `media`). | Files › `docs/logo.png`. | `target/t3-audit/evidence/panel/PA-5-ref.png`, `PA-5-clone.png`, `panel/dumps/r-aria-png-server.txt` |

## Scope and exclusions

Included: the five findings above.

Excluded:
- The launcher's Browser profile chevron (PA-13): [browser-surface-profiles](20261005-browser-surface-profiles.md) (#354).
- Browser parts 2–4 rows (PA-14): their own tasks.
- The Files rendered Markdown (PA-3): [markdown-links-and-files-preview](20261009-markdown-links-and-files-preview.md).
- The terminal's palette (PA-11): [app-color-scheme](20261009-app-color-scheme.md).
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`):
- PA-1: `components/ChatView.tsx:11502`.
- PA-2: `components/RightPanelTabs.tsx:417-445` (`handleKeyDown`).
- PA-10: `components/RightPanelTabs.tsx` (`SurfaceIcon`: `TerminalSquare`).
- PA-4: `components/files/FilePreviewPanel.tsx:1031-1036, 1160-1164`; `browser/openFileInPreview.ts:32`.
- PA-5: `components/files/FilePreviewPanel.tsx:1019-1024, 1150-1158` (`showsRawText`).

Clone (`examples/t3-code`): `r4-surfaces-panel.ts:117, 280`; `shell-panels.contract:94` (`SurfaceRow`);
`modules/apple/R8KeysLauncher.swift`; `shell-icons.contract:11`; `r4-surfaces-files.contract:129-134`; Browser part 1
(`browser-surface.contract`, the module's Browser sessions).

Notes:
- PA-2: follow the clone's menu-key pattern (`menu-keys.contract`, from fix-keyboard-focus) or extend
  `R8KeysLauncher.swift`. Keep the letter keys.
- PA-4: open the file through Browser part 1's open path. If part 1 cannot show a local file, say so in the PR and keep
  the row open with its blocker.
- Shared file: `r4-surfaces-files.contract` with [markdown-links-and-files-preview](20261009-markdown-links-and-files-preview.md).

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PA-1 | On a draft, Device is available and M opens it. | `pa1-device-on-draft.png` | agent |
| PA-2 | ArrowDown ×3 highlights the fourth available row; Enter opens Files. ArrowUp wraps as the reference. | `pa2-launcher-arrows.png` | agent |
| PA-10 | A Terminal tab shows the terminal icon. | `pa10-terminal-tab-icon.png` | agent |
| PA-4 | `docs/page.html` and a PDF show "Open file in preview browser"; pressing it opens the file in a Browser tab. | `pa4-open-in-preview.png`, `pa4-browser-tab.png` | agent |
| PA-5 | `docs/logo.png` shows no word-wrap toggle. | `pa5-image-subheader.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build and unit-test. Then do one batched live drive at the end for every
row's before/after pair. Close every row in this PR, or record the blocker of a row that cannot pass.
