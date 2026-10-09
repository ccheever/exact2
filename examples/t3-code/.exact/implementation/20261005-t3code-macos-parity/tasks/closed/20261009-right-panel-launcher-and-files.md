---
name: 20261009-right-panel-launcher-and-files
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-right-panel-launcher-and-files
pr_url: https://github.com/ccheever/exact2/pull/355
verified_commit: 21019bed3936baedf5b9169324a637b894d2e7a9
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

## Cause and fix

- PA-1: `availability().device` was `!!client.threadId`; the reference's is `activeThreadRef !== null`, and a draft has
  its thread ref. Device is now available for a thread or a project's draft. Opening it on a draft first allocates the
  draft's thread id (`ensureDraftThreadId`, as the Browser surface does), so the setup wizard, `device.open`, the device
  workspace and the floating player all use that id (`deviceThreadId`, `r6-media-device.ts`; `r12-threads-device.ts`,
  `chat-canvas-view.ts`). The window's gate in `panelUi` (`app.contract`) lets `device`/`m`/`M` through on a draft.
- PA-2: the launcher is its own component, `SurfaceLauncher` (`shell-panels.contract`), mounted afresh whenever the panel
  has no surface. One highlight (`-1` = none) runs over the available rows, as `handleKeyDown`: ↓/→ move it on and ↑/←
  back, wrapping (↑ with none goes to the last); a pointer over an available row sets it and leaving that row clears it;
  Enter on the launcher opens the highlighted surface. A ⌘/⌃/⌥ chord and every other key still go to `panelUi("key")`
  (letters, Escape). Enter on a focused row is the row's own press (`stopPropagation`, the reference's
  `event.target !== event.currentTarget`). `R8KeysLauncher.swift` is unchanged.
- PA-10: the Terminal tab's icon name was `terminal`, which `ShellIcon` does not draw; it is `square-terminal` now.
- PA-4: `R4Files.openInBrowser` (`canOpenInBrowser`: a `.html`/`.htm`/`.pdf` preview, not a video, while the module's web
  views exist) shows a globe toggle, "Open file in preview browser", after word wrap and before the explorer toggle. It
  runs `openFileInPreview` (`browser-links.ts`, the reference's `browser/openFileInPreview.ts`): `assets.createUrl` for a
  `workspace-file` (inside the workspace) or `media-file`, under the thread's id, then `openUrlInPreview` (Browser
  part 1). A failure is the stacked toast "Unable to open file in browser". A PDF had no body in Files (its bytes showed
  as numbered text); it now renders as its document, as `renderBrowserFile = isPdf` does, through the module's `t3-media`
  PDFView (the attachment's; declared X29, its scope in `EXACT2-GAPS.md` extended to the Files PDF).
- PA-4, review fix (2026-10-10): a PDF shorter than the panel sat about a third of the way down under a #282828 band,
  where Chromium's viewer (`#toolbar=0&view=FitH`) starts the page at the top with the surface below it. PDFKit's
  `PDFClipView` centers a document shorter than the view, so scrolling to the top cannot move it. The PDF body is now a
  box, `R6PDFBody` (`R6MediaPreview.swift`), on the viewer surface: it holds the PDFView at the document's height at the
  box's width, top-aligned, and at the box's full height (scrolling inside, from the first page's top) once the document
  is taller. The attachment preview uses the same body, so it is fixed there too.
- PA-5: `R4Files.rawText` (`showsRawText`) is true only for source text (`preview == "code"`), so an image, a video, a
  PDF, a rendered page, Markdown or table has no word-wrap toggle.

## Acceptance results

| Id | Result | Proof |
| --- | --- | --- |
| PA-1 | pass: on the work draft Device is available (no dimming, no "Available from a thread."), the pointer highlights it, and M opens Device: the "Set up devices" wizard, as the reference (onboarding not done on both) | [pa1-device-on-draft.png](https://raw.githubusercontent.com/ccheever/exact2/34f2cdaa0782cf9f836a783c4a39a1b5a7e97912/right-panel-launcher-and-files/pa1-device-on-draft.png), [pa1-m-opens-device.png](https://raw.githubusercontent.com/ccheever/exact2/b93364d7e2c75e86fbe54c5208dd442eaba44523/right-panel-launcher-and-files/pa1-m-opens-device.png) |
| PA-2 | pass by agent input: on Audit work thread, ArrowDown ×3 highlights Files (third available row; the reference's `(highlight + 1) % n` from none), ArrowUp ×3 wraps to Device, ArrowDown ×3 then Enter opens Files; same as the reference at each step. Right after Toggle right panel, before any key op, the launcher is the one focused node (`surface-chooser`, `focused: true`, 1 of 93). Not verified: real arrow keys reaching it without an op that focuses it first (`type surface-chooser key …` focuses its target); real-input batch step 1 | [pa2-launcher-arrows.png](https://raw.githubusercontent.com/ccheever/exact2/86b718aa276bb9baffc955c284f38718222ae542/right-panel-launcher-and-files/pa2-launcher-arrows.png), focus: [drive-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/3de7d5a6e37cc5145b699d04a243b5de2a484b36/right-panel-launcher-and-files/drive-ops.txt) ("Review fixes") |
| PA-10 | pass: the Terminal tab shows the terminal-square glyph before "Terminal 1" | [pa10-terminal-tab-icon.png](https://raw.githubusercontent.com/ccheever/exact2/14cf63af4865f954903df492e53603b360678bc0/right-panel-launcher-and-files/pa10-terminal-tab-icon.png) |
| PA-4 | pass: `docs/page.html` (Open in editor, Show HTML source, globe, Hide file explorer) and `docs/guide.pdf` (Open in editor, globe, Hide file explorer) match the reference's ARIA order; the PDF body starts with the page at the top, the surface below (as the reference); pressing the globe opens a new Browser tab at the file's signed `workspace-file` URL, driven live for both `page.html` and `guide.pdf` (the reference opens both the same way) | [pa4-open-in-preview.png](https://raw.githubusercontent.com/ccheever/exact2/3c328d08bea48f6ac07a380d7bedbf153b841942/right-panel-launcher-and-files/pa4-open-in-preview.png), [pa4-browser-tab.png](https://raw.githubusercontent.com/ccheever/exact2/3d8ec3f1420ad7c7d0c61cd2348dfb4a1deb41ab/right-panel-launcher-and-files/pa4-browser-tab.png) |
| PA-5 | pass: `docs/logo.png` shows Open in editor and Hide file explorer only | [pa5-image-subheader.png](https://raw.githubusercontent.com/ccheever/exact2/95488f5ed0ac8a5e17abddf80270f1843d78d919/right-panel-launcher-and-files/pa5-image-subheader.png) |

The drives (before: base build `950e8e2e5`; after: this branch, one live drive, plus one after the review fixes on a
fresh app storage; reference: Electron over CDP, same lane fixture):
[drive-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/3de7d5a6e37cc5145b699d04a243b5de2a484b36/right-panel-launcher-and-files/drive-ops.txt).
All five rows were driven by agent input (platform-delivered keys and taps). One part of PA-2 needs real keys: the
real-input batch step below.

## Real-input batch steps

Lane `right-panel-launcher-and-files` (base port 16860), this branch's bundle (`target/clients/…/T3 Code (Exact).app`
in the worktree), opened by hand on the lane (the `clone-drive.sh` environment, without `agent.mjs`).
1. **Launcher keys with real focus (PA-2).** Select "Audit work thread". If its right panel shows tabs, close each tab
   (the panel closes with the last one). Press ⌘⌥B (or click Toggle right panel): the panel opens on "Open a surface".
   Without clicking anything else, press ↓ three times: Files is highlighted (Browser, Terminal, Files). Press ↑ three
   times: the highlight wraps to Device. Press ↓ three times, then Return: Files opens. Close the Files tab (the panel
   closes), press ⌘⌥B again and then F: Files opens (the letter keys still work).

## Tests

- `r4-surfaces.test.ts`: Device available on a draft with a project, not without one; M on a draft allocates the draft's
  thread id and opens the wizard under it; a Terminal tab's icon is one `ShellIcon` draws; the subheader flags for
  source, page, PDF, image and page source, and without the module; the globe signs a `workspace-file` for the thread
  and opens a Browser tab, for `docs/page.html` and for `docs/guide.pdf` (the module is told to sync the new tab, and no
  toast); a refused signature is the toast and no tab; the launcher keyboard's wiring.
- AppKit `macos/tests/r6-media` (`testShortPdfStartsAtTheTop`, and `testPdfFitsTheWidthOnTheViewerSurface` extended):
  a one-page PDF in a 300×700 panel starts at the top (top gap under 12 pt; before the fix 158 pt, centered) with the
  surface below, stays there when the panel grows, and a document taller than the panel fills it and opens on the
  first page's top. `media-actions` also passes (7/7).
- `r6-media-device.test.ts`: on a draft, `device.open` and the workspace use the draft's thread id.

Checks: see the PR ("Checks").

## Found, not changed

- On a fresh draft the reference's right panel is closed (its open state is per thread); the clone shows the launcher
  there when the panel was open on the previous thread (both the base and this branch). Seen while taking PA-1; not in
  this task's findings.
- A PDF opened in a Browser tab shows WebKit's PDF view (the page on white, no toolbar); the reference shows
  Chromium's PDF viewer (toolbar, thumbnails, #282828 surface). The Browser surface is a `WKWebView`: X1 path B,
  declared in `EXACT2-GAPS.md`.

## Next action

Run real-input batch step 1; then the coordinator reviews and merges #355.

## Delivery

Merged by the coordinator on 2026-10-10 as `21019bed3` (#355, squash) after an independent review and its repair round. Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
