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
  to it); a second Escape closes Filters. Where the focus goes is the submenu's `FloatingFocusManager` (Base UI 1.5.0):
  back to its trigger row (`MenuPopup` `returnFocus` true with a trigger), unless the trigger saw a mouseleave after the
  submenu opened (`preventReturnFocusRef`, reason `triggerHover`). Checked on the reference over CDP (review round,
  [drive-evidence-2.txt](https://raw.githubusercontent.com/ccheever/exact2/d62b19943b85a04b43f088612e69f9ee33c8c5bc/audit-wave-followups-3/drive-evidence-2.txt)):
  click Author, Escape in "Search authors" focuses the Author row (lit) and ↓ goes to Labels; with the pointer moved
  into the field first (or State clicked, then the pointer onto Closed), Escape leaves the focus on BODY, no row lit,
  and ↓ moves nothing. (The first round's CDP check saw BODY because its pointer had left the row; the clone then gave
  the focus to the Filters popup, which no reference state does.) `PrFiltersMenu` (`pages-prs.contract`) now prevents
  Escape while a submenu is open (`subEscape` on the row that holds the submenu and the Filters rows; the author field's
  `searchKey` calls the same `closeSub`): the focus goes back to the submenu's row, or, when the pointer left that row
  since it opened (`PrSubTrigger`'s new `leave`, `rowLeft`, `subLeft`, reset by each opening), `blur()` drops it; a
  second Escape still closes Filters (`Presenter.routeKey`'s popover Escape needs no focus). A prevented Escape never
  reaches the popover's own. The Filters rows now move the focus from themselves (`subKey` runs `kmTarget` over
  `filterItems` from its own row): the popup's `KeyMenu` knew only the rows its own keys reached, so ↓ from the
  returned Author row went to State.
- **FW-4.** `PullRequestAuthorFilter`'s radio items carry no `MenuRadioItemIndicator`, so the chosen row is only
  tinted (`data-checked:bg-foreground/[0.08]`). Anyone is `PrMenuItem(tick=false)` (tinted while no author is
  chosen) and `PrAuthorItem` lost its check and tints its chosen author. The other radio submenus keep their tick.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| FW-1 | pass (agent): mode System in agent mode, Expand diagram on `docs/flow.md` shows the drawing on a white card (before: black) as the reference's; with the record's steps (app Light, macOS Dark) white before and after, as the reference's | [fw1-system.png](https://raw.githubusercontent.com/ccheever/exact2/9d13a05b3c3e4703b8f67e3367169f486e60061f/audit-wave-followups-3/fw1-system.png), [fw1-light-on-dark.png](https://raw.githubusercontent.com/ccheever/exact2/e991e8ef72d70d5f2f15f95ab01c001ad0aad8c1/audit-wave-followups-3/fw1-light-on-dark.png) |
| FW-2 | pass (agent): the sent `notes.md` renders as the reference's ChatMarkdown: "the guide" a targetless link, T3 with its favicon, the table with Collapse and Copy, disabled checkboxes, the TS block coloured with its icon, wrap and copy, and the Mermaid diagram with Expand (tree: `mermaid-expand-8`). Before: plain text links, no table menu, a "ts" label, uncoloured code and the fence as code | [fw2-attachment.png](https://raw.githubusercontent.com/ccheever/exact2/1f3228ca638ea2fcbaa1b71ceb597710f17637c4/audit-wave-followups-3/fw2-attachment.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/0c613d0091cd32e78ff390e687fb29e6564e2614/audit-wave-followups-3/drive-evidence.txt) |
| FW-3 | pass (agent keys and pointer moves): Filters › → into State › Escape closes State only, the focus on the State row and Filters open (tree: focus `pr-filter-state`, `pr-filters-layer` open). Review round, as the reference: click Author with the pointer on it, Escape in Search authors closes Author only and focuses the Author row (lit), ↓ then focuses Labels; with the pointer moved into the field, Escape closes Author, Filters stays and nothing has the focus (no row lit); State with the pointer moved onto Closed, the same. Before: every Escape closed the whole menu. Real Escape: open row (batch steps below) | [fw3-row-escape.png](https://raw.githubusercontent.com/ccheever/exact2/1b49bbf105044db4132429cdb48b6473ffd495c4/audit-wave-followups-3/fw3-row-escape.png), [fw3-search-escape-2.png](https://raw.githubusercontent.com/ccheever/exact2/c670ff274690c2878d9d92a0ad1c4121d86c7e3f/audit-wave-followups-3/fw3-search-escape-2.png), [fw3-search-down.png](https://raw.githubusercontent.com/ccheever/exact2/eba23cd5542f95e780066b6820b7385d51ce213d/audit-wave-followups-3/fw3-search-down.png), [fw3-search-left.png](https://raw.githubusercontent.com/ccheever/exact2/bba2d24b2536967bfa531764e8378062ee62bc48/audit-wave-followups-3/fw3-search-left.png), [drive-evidence-2.txt](https://raw.githubusercontent.com/ccheever/exact2/d62b19943b85a04b43f088612e69f9ee33c8c5bc/audit-wave-followups-3/drive-evidence-2.txt) |
| FW-4 | pass (agent): Anyone is tinted with no check while no author is chosen (still `aria-checked`, as the reference's `menuitemradio`); before: ticked | [fw4-author.png](https://raw.githubusercontent.com/ccheever/exact2/1c9cd1cd8ef9f1db2fda6285effdd129f6e2711a/audit-wave-followups-3/fw4-author.png) |

## Tests

- `menu-keys.test.ts` "the Pull Requests Filters submenus": FW-3's `subEscape`, `closeSub` (focus the row or `blur()`
  by `subLeft`), `rowLeft` on every row, `PrSubTrigger`'s `leave`, the author field's Escape through `closeSub`, the rows'
  own `kmTarget` keys over `filterItems`; FW-4's untinted-tick rule (Anyone `tick=false`, no check in `PrAuthorItem`, the
  radio submenus keep `tick=true`). `usage-pr-pages.test.ts` PG-6 pins: each opening resets `subLeft`.
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
2. FW-3, search, the pointer resting on Author: click Filters, click Author and leave the pointer there: "Search
   authors" has the caret. Press Escape: the Author submenu closes, Filters stays open with Author highlighted (the
   focus on it). Press ↓: the focus goes to Labels (Author keeps its hover shade while the pointer rests on it, see
   "Found, not changed"). Press Escape: Filters closes. (Reference over CDP, same steps: the focus on the Author row,
   then on Labels.)
3. FW-3, search, the pointer moved into the submenu: click Filters, click Author, click inside "Search authors". Press
   Escape: the Author submenu closes, Filters stays open, no row is highlighted and no row has the ring. Press ↓:
   nothing moves. Press Escape: Filters closes. (Reference: the focus on BODY, ↓ moves nothing.)
4. FW-4 (pointer, quick look in step 2): with no author chosen, Anyone is shaded and has no check mark.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975` Electron over CDP, app Light, `prefers-color-scheme` emulated dark | Complete | the third column of each image |
| Before drive | the feature tip `b8e36d476`, built in its own worktree `t3-code-awf3-before` (the shared `t3-code-evidence-base` sits at `950e8e2e5`, 30 commits behind, without #369, #371 and #375, which these rows start from) | Complete on the second run (the first run's attachment tap landed on the Codex error banner over the chip; the drive now dismisses it) | [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/0c613d0091cd32e78ff390e687fb29e6564e2614/audit-wave-followups-3/drive-evidence.txt) |
| After drive (the live drive) | this branch's bundle `a66131ef1` | The first run stopped at op 48, `clock: the clock cannot go backwards (60700.0 → 60699.99999999999)` (a driver float, before any FW-3 step); the one retry completed every step | the links above |
| Review round: FW-3 focus after Escape | reference over CDP; before `b8e36d476`; after `44eaec895`, then `f7bc361b9` | The review found the popup focus unbacked by the reference. The reference returns the focus to the row unless the pointer left it. First after run: the focus right in every case, but ↓ from the returned Author row went to State (the popup KeyMenu's own index); fixed by the rows' own keys; the one retry passed every step | [drive-evidence-2.txt](https://raw.githubusercontent.com/ccheever/exact2/d62b19943b85a04b43f088612e69f9ee33c8c5bc/audit-wave-followups-3/drive-evidence-2.txt) |

## Found, not changed

- A disabled, checked task checkbox (an attachment, the transcript) draws the blue box at half opacity
  (`markdown.contract` TaskBox); the reference draws Chrome's grey disabled checkbox. Visual only, shared with the
  transcript.
- The reference's radio submenus (State, Involvement, Draft, Review, Checks, Project) tint the checked row as well as
  ticking it (MenuRadioItem's `data-checked:bg-foreground/[0.08]`); the clone ticks only (`PrMenuItem` with
  `tick=true`). Visual only.
- Filters rows: the clone shades a row while the pointer is over it (`PrSubTrigger` `over`) and while it has the focus,
  so after ↓ from a hovered row two rows are shaded until the pointer moves off; Base UI keeps one highlight, which the
  keys move off the hovered row (a mouse move within it takes it back). The clone's painted menus share this hover
  model. Visual only.
- The agent-mode appearance mismatch (mode System: the window drawn Light, `viewport.prefersColorScheme` Dark) is host
  code; the views that pick a palette by `scheme` (the Usage colours, the terminal) take the Dark one in that state, in
  agent mode only (in normal use the window and the fact agree).

## Next action

The coordinator reviews the draft PR [#378](https://github.com/ccheever/exact2/pull/378), runs the real-input steps in
the next batch, and merges it.
