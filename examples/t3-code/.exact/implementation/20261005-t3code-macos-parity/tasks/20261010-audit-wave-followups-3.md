---
name: 20261010-audit-wave-followups-3
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-audit-wave-followups-3
pr_url: https://github.com/ccheever/exact2/pull/378
verified_commit: null
---

# Differences the audit fix agents found outside their tasks (third set)

## Outcome

PRs #371 and #375 reported four more differences from the reference, outside their findings. This task fixes them as
the reference does. ([First set](closed/20261010-audit-wave-followups.md), [second set](20261010-audit-wave-followups-2.md).)

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FW-1 | Expand diagram shows the Mermaid diagram on the app's card colour: white in the app's Light mode, also when macOS is Dark. | On macOS Dark with the app Light, the expanded dialog shows the diagram on a dark card (the shared `DiagramPreviewDialog`, or the offscreen Mermaid render's theme). TH-7 (#369) fixed the dialog's chrome; this is what remains. | markdown-links-and-files-preview (#371) | Agent mode: `prefer` dark, app mode Light; a thread or a Files `.md` with a mermaid fence; Expand diagram. |
| FW-2 | A sent attachment's Markdown preview renders with the chat renderer (`ChatMarkdown`), as Files' rendered Markdown now does (#371). Check the reference first. | It uses the reduced parser (`r4-surfaces-render.ts`). | markdown-links-and-files-preview (#371) | Send (or seed) a `.md` attachment; open its preview in both apps. |
| FW-3 | Escape in a Pull Requests Filters submenu (Author, Labels, …) closes only that submenu; the Filters menu stays. | Escape closes the whole Filters menu (since fix-keyboard-focus). | usage-and-pr-pages (#375) | Pull Requests › Filters › → into Author › Escape. |
| FW-4 | With no author chosen, the Author submenu shows no check on "Anyone". | "Anyone" is ticked. | usage-and-pr-pages (#375) | Pull Requests › Filters › Author. |

## Scope and exclusions

Included: the four rows. Excluded: framework changes. FW-2 changes nothing if the reference's attachment preview also
uses a reduced renderer: then record the reference lines.

## Context and guidance

- FW-1: `timeline-mermaid.contract`, `timeline-mermaid.ts` (the render theme sent to the offscreen renderer),
  `app-overlays.contract` (`DiagramPreviewDialog`), #369's `scheme`.
- FW-2: reference attachment preview component (`apps/web/src/components/media` or the attachment panel); clone
  `r4-surfaces-render.ts`, the chat renderer path #371 uses for Files.
- FW-3, FW-4: reference `PullRequestFilters` (Base UI menu nesting); clone `pages-prs.contract`, `pages-prs.ts`.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FW-1 | agent drive with `prefer` dark and app Light | before / after / reference image |
| FW-2 | agent drive on an attachment preview (or the reference lines if unchanged) | before / after / reference image |
| FW-3 | agent keys; real Escape in the batch | before / after / reference image |
| FW-4 | agent drive | before / after / reference image |

## Cause and fix

- **FW-1.** The record's own steps (app Light, `prefer prefers-color-scheme dark`) already showed a white card on the
  tip: #369 (app-color-scheme) gave the dialog the resolved `scheme`. The black card #371's drive saw was mode System in
  agent mode, the state the 2026-10-09 audit left unexplained ("Observations not filed", agent-mode appearance): the
  window draws Light (the agent's own system appearance) while `viewport.prefersColorScheme`, so `scheme`, reads the
  Mac's Dark. The card chose its colour by `scheme` and the drawing's `light-dark()` paints resolved by the window, so a
  light drawing sat on a black card (reproduced on the tip, before image). `DiagramPreviewDialog`
  (`timeline-mermaid.contract`) now paints the card `light-dark(#fcfcfc, #0a0a0a)` with a `light-dark()` ring: the
  page's colour as the window draws it, which is ZoomableImage's `bg-background`, the colour the drawing resolves by.
  The drawing is still rebuilt per `scheme`. In normal use (the window's appearance set from the mode by
  `T3WindowChrome.setAppearance`) nothing changes. The agent-mode state itself is host code and stays out of scope.
- **FW-2.** Checked the reference first: `AttachmentFilePreview.tsx` renders rendered Markdown as `<ChatMarkdown
  text={content.text} cwd={undefined} className="mx-auto max-w-4xl px-6 py-5" />` in a ScrollArea, so it is the chat
  renderer with no folder. `r5-panels-attach.ts renderedAttachment` now hands it to the chat parser as Files does: one
  `markdownSource` message of kind `attachment` (app.contract's `filesMarkdown` resource asks
  `renderFileMarkdown(concat(files, attachment))`; the panel shows one of the two at a time), the code colours
  (`messageCodeBlocks`), the Mermaid fences (`filesMermaid`, so Expand diagram opens the preview), host-path media
  with no root and Copy code's check. `R5AttachmentBody` draws it with `DiagramBlocks`, rebuilt per scheme
  (`R5AttachmentMarkdown`, `r5-panels.contract`). In Rust (`macos/src/markdown.rs`, `markdown_links.rs`), kind
  `attachment` parses with `link_href_without_cwd`: resolveMarkdownFileLinkTarget finds no folder for a relative
  destination, so `[the guide](docs/guide.md)` is link-coloured text with no target (the desktop window opens nothing
  for it), while an absolute path or a `file:` URL stays a chip; task checkboxes are disabled. `R4MarkdownSource`
  moved to `markdown.contract` as `MarkdownSource` (both surfaces use it). The reduced parser (`markdownDocument`,
  `inlineRuns` in `r4-surfaces-render.ts`) had no other user and is deleted.
- **FW-3.** Base UI's submenu closes alone on Escape (`closeParentOnEsc` false; a menu with an open child leaves Escape
  to it), checked on the reference over CDP: from a submenu row the focus returns to its row, from "Search authors" the
  focus falls to the page (the field unmounts first) and no row is lit; a second Escape closes Filters.
  `PrFiltersMenu` (`pages-prs.contract`) now prevents Escape while a submenu is open: `subEscape` on the row that holds
  the submenu and the Filters rows closes the submenu and focuses its row; the author field's own `searchKey` closes it
  through `searchEscape` and gives the focus to the Filters popup (`pr-filters-keys`), so no row is lit and the menu's
  keys still reach it. A prevented Escape never reaches the popover's own (`Presenter.routeKey`).
- **FW-4.** `PullRequestAuthorFilter`'s radio items carry no `MenuRadioItemIndicator`, so the chosen row is only
  tinted (`data-checked:bg-foreground/[0.08]`). Anyone is `PrMenuItem(tick=false)` (tinted while no author is
  chosen) and `PrAuthorItem` lost its check and tints its chosen author. The other radio submenus keep their tick.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| FW-1 | pass (agent): mode System in agent mode, Expand diagram on `docs/flow.md` shows the drawing on a white card (before: black) as the reference's; with the record's steps (app Light, macOS Dark) white before and after, as the reference's | [fw1-system.png](https://raw.githubusercontent.com/ccheever/exact2/9d13a05b3c3e4703b8f67e3367169f486e60061f/audit-wave-followups-3/fw1-system.png), [fw1-light-on-dark.png](https://raw.githubusercontent.com/ccheever/exact2/e991e8ef72d70d5f2f15f95ab01c001ad0aad8c1/audit-wave-followups-3/fw1-light-on-dark.png) |
| FW-2 | pass (agent): the sent `notes.md` renders as the reference's ChatMarkdown: "the guide" a targetless link, T3 with its favicon, the table with Collapse and Copy, disabled checkboxes, the TS block coloured with its icon, wrap and copy, and the Mermaid diagram with Expand (tree: `mermaid-expand-8`). Before: plain text links, no table menu, a "ts" label, uncoloured code and the fence as code | [fw2-attachment.png](https://raw.githubusercontent.com/ccheever/exact2/1f3228ca638ea2fcbaa1b71ceb597710f17637c4/audit-wave-followups-3/fw2-attachment.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/0c613d0091cd32e78ff390e687fb29e6564e2614/audit-wave-followups-3/drive-evidence.txt) |
| FW-3 | pass (agent keys): Filters › → into State › Escape closes State only, the focus on the State row and Filters open (tree: focus `pr-filter-state`, `pr-filters-layer` open); Escape in Search authors closes Author only, Filters open, the focus on the popup (no row lit), as the reference. Before: both Escapes closed the whole menu. Real Escape: open row (batch steps below) | [fw3-row-escape.png](https://raw.githubusercontent.com/ccheever/exact2/1b49bbf105044db4132429cdb48b6473ffd495c4/audit-wave-followups-3/fw3-row-escape.png), [fw3-search-escape.png](https://raw.githubusercontent.com/ccheever/exact2/0345354c93df37d65a02d252f13eda900d7e0dbe/audit-wave-followups-3/fw3-search-escape.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/0c613d0091cd32e78ff390e687fb29e6564e2614/audit-wave-followups-3/drive-evidence.txt) |
| FW-4 | pass (agent): Anyone is tinted with no check while no author is chosen (still `aria-checked`, as the reference's `menuitemradio`); before: ticked | [fw4-author.png](https://raw.githubusercontent.com/ccheever/exact2/1c9cd1cd8ef9f1db2fda6285effdd129f6e2711a/audit-wave-followups-3/fw4-author.png) |

## Tests

- `menu-keys.test.ts` "the Pull Requests Filters submenus": FW-3's `subEscape` and `searchEscape` wiring, FW-4's
  untinted-tick rule (Anyone `tick=false`, no check in `PrAuthorItem`, the radio submenus keep `tick=true`).
- `app-color-scheme.test.ts`: FW-1's card is `light-dark()` with the drawing rebuilt per scheme.
- `r5-panels.test.ts`: the attachment's `markdownSource` (kind `attachment`), and a new test: code colours, no chips,
  a Mermaid fence loading then rendered, and its expand opening the preview.
- `r4-surfaces.test.ts`: the reduced parser's expectations removed with it (CSV stays).
- `macos/src/markdown_links.rs` `an_attachment_has_no_folder_for_relative_links`: relative links targetless,
  absolute, `file:` and drive paths chips, web links kept, tasks disabled (`cargo test -p t3-code-macos --lib`: 17 pass).

## Real-input batch steps

Run on this branch's bundle (lane `audit-wave-followups-3`, base port 16420: `T3_LOCAL_HOME=<lane>/clone-t3-home
T3_LOCAL_PORT=16422`), app launched normally and active, window 1280×840:

1. FW-3, keyboard: click Pull Requests in the sidebar, click Filters, press ↓ (State has the ring), then →: the State
   submenu opens with the ring on All. Press Escape: the State submenu closes, Filters stays open with the ring on
   State. Press Escape again: Filters closes and the Filters button has the ring.
2. FW-3, search: click Filters, click Author: "Search authors" has the caret. Press Escape: the Author submenu closes,
   Filters stays open with no row highlighted. Press ↓: the ring goes to State. Press Escape: Filters closes.
3. FW-4 (pointer, quick look in step 2): with no author chosen, Anyone is shaded and has no check mark.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975` Electron over CDP, app Light, `prefers-color-scheme` emulated dark | Complete | the third column of each image |
| Before drive | the feature tip `b8e36d476`, built in its own worktree `t3-code-awf3-before` (the shared `t3-code-evidence-base` sits at `950e8e2e5`, 30 commits behind, without #369, #371 and #375, which these rows start from) | Complete on the second run (the first run's attachment tap landed on the Codex error banner over the chip; the drive now dismisses it) | [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/0c613d0091cd32e78ff390e687fb29e6564e2614/audit-wave-followups-3/drive-evidence.txt) |
| After drive (the live drive) | this branch's bundle `a66131ef1` | The first run stopped at op 48, `clock: the clock cannot go backwards (60700.0 → 60699.99999999999)` (a driver float, before any FW-3 step); the one retry completed every step | the links above |

## Found, not changed

- A disabled, checked task checkbox (an attachment, the transcript) draws the blue box at half opacity
  (`markdown.contract` TaskBox); the reference draws Chrome's grey disabled checkbox. Visual only, shared with the
  transcript.
- The reference's radio submenus (State, Involvement, Draft, Review, Checks, Project) tint the checked row as well as
  ticking it (MenuRadioItem's `data-checked:bg-foreground/[0.08]`); the clone ticks only (`PrMenuItem` with
  `tick=true`). Visual only.
- The agent-mode appearance mismatch (mode System: the window drawn Light, `viewport.prefersColorScheme` Dark) is host
  code; the views that pick a palette by `scheme` (the Usage colours, the terminal) take the Dark one in that state, in
  agent mode only (in normal use the window and the fact agree).

## Next action

The coordinator reviews the draft PR [#378](https://github.com/ccheever/exact2/pull/378), runs the real-input steps in
the next batch, and merges it.
