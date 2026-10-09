---
name: 20261009-markdown-links-and-files-preview
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-markdown-links-and-files-preview
pr_url: https://github.com/ccheever/exact2/pull/371
verified_commit: null
---

# Markdown: relative ":line" file links, and the chat renderer in the Files rendered preview

## Outcome

- Assistant Markdown links to relative `name:line` targets render as the reference renders them, and no chip that does
  nothing on a press remains (TH-9, see "Decision").
- Files › "Show rendered markdown" uses the chat Markdown renderer, as the reference does: links in the link colour with a
  favicon that open, tables with "Collapse table cells" and "Copy table", code blocks with the language icon, "Disable
  line wrap" and "Copy code", and task-list checkboxes that toggle and save the file (PA-3).

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| TH-9 | `Review [parser](fixture.txt:3), [fixture.txt](fixture.txt), [fixture.txt:3](fixture.txt:3).` renders "Review parser, [fixture.txt chip], fixture.txt:3.". react-markdown's `defaultUrlTransform` reads `fixture.txt:3` as a URL scheme and strips it, so "parser" and "fixture.txt:3" are anchors with no href. Only the bare path becomes a chip. Absolute paths behave alike in both apps. | "Review parser [fixture.txt · L3], [fixture.txt], [fixture.txt · L3].": two extra "· L3" chips, and those two chips do nothing on a press. | Open "Timeline verification". Read the assistant message's second paragraph. Press each chip. | `target/t3-audit/evidence/thread/TH-9-ref.png`, `TH-9-clone.png` |
| PA-3 | "Show rendered markdown" uses the chat renderer. A link is blue with a favicon and opens. A table has "Collapse table cells" and "Copy table". A code block shows the language icon with "Disable line wrap" and "Copy code". Task-list checkboxes toggle and save the file. | The link text is plain body text (no colour, no favicon, no press handler in the tree). The table has no controls. The code block shows the text "ts". Task markers are static ☐/☑ glyphs. | Files › `docs/notes.md` (heading, link, list, ts code block, table) › Show rendered markdown. | `target/t3-audit/evidence/panel/PA-3-ref.png`, `PA-3-clone.png`, `panel/dumps/r-aria-notes-r.txt`, `panel/dumps/clone-drive-04.jsonl` |

## Scope and exclusions

Included: the two findings above.

Excluded:
- The Mermaid preview's black box (TH-7): [app-color-scheme](20261009-app-color-scheme.md).
- The Files subheader's actions (PA-4, PA-5): [right-panel-launcher-and-files](20261009-right-panel-launcher-and-files.md).
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975`):
- TH-9: `apps/web/src/components/ChatMarkdown.tsx` (react-markdown with its default `urlTransform`),
  `packages/client-runtime/src/markdownLinks.ts:268-288` (`isMarkdownFileLinkLabel`).
- PA-3: `apps/web/src/components/files/FilePreviewPanel.tsx:842-890` (`RenderedMarkdownSurface`, `FileMarkdownPreview`,
  `onTaskListChange`).

Clone (`examples/t3-code`):
- TH-9: the A4 rule lives in the app's Rust parser (`macos/src/markdown.rs`, from
  [upstream-timeline-and-markdown](closed/20261005-upstream-timeline-and-markdown.md) A4, line 38 and the acceptance row
  at line 106), the chip list in `r4-timeline-chips.ts`, the view in `markdown.contract`.
- PA-3: `r4-surfaces-render.ts:15-90` (`markdownDocument`, `inlineRuns`: table rows as blocks, "☐ " glyphs),
  `r4-surfaces-files.contract:246-249`. Reuse the chat renderer (`markdown.contract` and its data) instead of a second
  renderer. A task-list toggle writes the file through the same save path as the Files editor.

Regression note: the closed A4 row expected "Prose + chip; chip only; chip only" for the TH-9 message. That expectation
came from `markdownLinks.ts`. The live reference at the pin strips relative `name:line` hrefs before that rule runs.

Shared file: `r4-surfaces-files.contract` is also changed by
[right-panel-launcher-and-files](20261009-right-panel-launcher-and-files.md). Merge one before the other starts, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| TH-9 | The message renders as the decision says. No chip remains that does nothing on a press. Ported `markdownLinks.test.ts` cases still pass; add cases for relative `name:line` hrefs. | `th9-file-links.png` | agent |
| PA-3 | In the rendered `docs/notes.md`: the link is coloured with a favicon and opens; the table has Collapse and Copy; the code block has its language icon, wrap and copy; a task checkbox toggles and the file on disk changes. | `pa3-rendered-markdown.png`, `pa3-task-toggled.png` | agent |

## Decision (settled by the user's rule)

TH-9 builds (a): match the live reference. The label of `[parser](fixture.txt:3)` renders as link text with no target
(react-markdown's `defaultUrlTransform` strips the href), and only `[fixture.txt](fixture.txt)` becomes a chip. The user
ruled on 2026-10-09 (browser "Recently used") that the clone matches the reference at the pinned revision even where the
original looks broken, and on 2026-10-08 that every provisional choice must match the original. This reverses the
closed A4 row's accepted output for these links (`tasks/closed/20261005-upstream-timeline-and-markdown.md` A4); note
that in the PR.

## Cause and fix

- TH-9: the clone's link resolver (`macos/src/markdown.rs`) sent every scheme-less destination to the file-chip path,
  so `fixture.txt:3` became a chip, and `r4-timeline-chips.ts` skipped it as a web URL (`isWebHref` read `fixture.txt:`
  as a scheme), so the chip had no ChipView and no press. The reference first runs ChatMarkdown's
  `markdownUrlTransform`: citation and context references, Windows drive paths and `file:` URLs pass, anything else
  goes through react-markdown's `defaultUrlTransform`, which empties a destination whose first colon comes before any
  `/`, `?` or `#` unless the scheme is http(s), irc(s), mailto or xmpp (the sanitize schema agrees). The anchor keeps
  its label in the link colour with no href, so it does nothing on a press. `macos/src/markdown_links.rs` ports that
  rule (`link_href`); an emptied destination is a `link` run with an empty href (blue, no press, no favicon), and a
  table cell copies its label alone (`serializeAnchor`). `media-views.ts` `markdownLinkHref` is the same rule for the
  chip list and an image's source, so every chip the parser draws has its ChipView. A side effect that also follows the
  reference: `tel:`, `javascript:`, `data:` and `ftp://` destinations are targetless link text, `xmpp:` is a link, not a
  chip, and a Windows drive path is a chip with its ChipView.
- Regression note: the closed A4 row ([upstream-timeline-and-markdown](closed/20261005-upstream-timeline-and-markdown.md)
  A4, line 38 and the acceptance row at line 106) expected "prose + chip; chip only; chip only" for
  `[parser](fixture.txt:3) [fixture.txt](fixture.txt) [fixture.txt:3](fixture.txt:3)`. Per the decision above, the
  clone now renders "parser" and "fixture.txt:3" as targetless link text. A destination with a slash before the colon
  (`src/a.ts:3`, `./a.ts:3`, `/repo/a.ts:3`) still survives the transform and keeps A4's prose-plus-chip rule
  (`prose_file_labels_survive_beside_one_chip`).
- PA-3: Files rendered Markdown with `r4-surfaces-render.ts`, a second, reduced parser (table rows as plain blocks,
  "☐"/"☑" glyphs, links as plain runs) into `MarkdownBlocks` with no chips or highlights. The reference's
  FileMarkdownPreview is ChatMarkdown. Files now sends the file as one message (`R4Files.markdownSource`, kind `file`
  when editable) to a new Rust source, `renderFileMarkdown` (`app.contract` `filesMarkdown`, the transcript's parser),
  and draws its document with `DiagramBlocks` (`r4-surfaces-files.contract`, plumbed `filesDocuments` through
  `app-window`, `app-main`, `shell-panels`, `r4-surfaces`). `r4-surfaces-files.ts renderedMarkdown` gives it what an
  answer carries: `markdownEnv` with chips from `messageChips` resolved against the file's own folder (`imageBaseDir`),
  `messageCodeBlocks` (the language icon, colours), host-path media URLs (`markdownMediaUrls` over the file's text) and
  Copy code's check.
- PA-3 task lists: `markdown_links.rs task_items` reads GFM task items (`- [ ] `, `1. [x] `, text after the marker): the
  marker leaves the first run and the block carries `task` ("open"/"done") and `taskOffset` (the `[`'s UTF-16 offset,
  `findTaskListMarkerOffset`; the item lines are matched to the parser's items in order, and an offset is given only
  when the counts agree). `markdown.contract TaskCheckbox` draws the checkbox in the marker's gutter, as Electron's
  macOS checkbox (13pt, white or the accent blue with a tick); in a file preview its press sends
  `surface-files-task` with the offset and the new state, and `toggleTask` applies the reference's
  `setMarkdownTaskChecked` to the latest contents and saves through `editFile` (the editor's save path). The transcript
  and a read-only or truncated file get offset -1: GFM's disabled checkbox. A sent attachment's Markdown
  (`r4-surfaces-render.ts`, still the attachment preview's parser) draws the same disabled checkbox instead of glyphs.

- Review fixes (2026-10-10, independent review of PR #371):
  - A task item whose text starts with emphasis, inline code or a link (`- [ ] **Write** the tests`, ``- [x] `code`
    first``, `- [ ] [link](https://a.com) after`) was not a task: the parser splits the marker into a first run of its
    own (`"[ ] "`), and `task_marker` wanted text after the marker in that run, so the item drew a literal "[ ] ". GFM
    and the reference treat all three as tasks (and the base's reduced parser drew ☐/☑ for them). `task_items` now
    accepts a marker-only first run when the block has more runs and drops that run.
  - A ```mermaid fence in a rendered Markdown file now draws its diagram, as FileMarkdownPreview's ChatMarkdown does
    (MarkdownMermaidCodeBlock, settled): `timeline-mermaid.ts filesMermaid` asks the app module for the file's fences
    while `renderedMarkdown` builds the view (the module announces each finished render on `t3.status`, so the next
    view carries it), Files passes them to `DiagramBlocks`, and "Expand diagram" opens the same preview dialog as the
    transcript's (`diagramPreviewAction` accepts the file's fences). A Retry is sent with the next request that asks
    for that fence, so the transcript's request no longer drops a file fence's retry.
  - Task offsets are computed on the file's own text, not on the text after `image_chip_links` (which drops the `!`
    of each `![name](t3-context://…)` and shifted every later marker by one, making its press a no-op).

## Acceptance results

| Id | Result | Proof |
| --- | --- | --- |
| TH-9 | pass: "Review parser, [fixture.txt chip], fixture.txt:3." as the reference: "parser" and "fixture.txt:3" are link-coloured text with no href and no handler (tree), the one chip opens fixture.txt in Files; no "· L3" chip remains. Before: two `file-chip` views with no press | [th9-file-links.png](https://raw.githubusercontent.com/ccheever/exact2/d654443840a5e3662ffb085af9646207d8ff17ec/markdown-links-and-files-preview/th9-file-links.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/4d88d8e5ce27275caffa5ab93d812331bbdafb1c/markdown-links-and-files-preview/drive-evidence.txt) |
| PA-3 | pass: in the rendered `docs/notes.md` the link has the globe favicon, the blue colour and a press (the drive's tap recorded `https://example.com`, agent runs record instead of opening); `[app.ts](../src/app.ts)` is a chip resolved against the file's folder; the table has Collapse table cells and Copy table; the code block shows the TS icon, Disable line wrap and Copy code with Shiki colours; the task checkboxes match the reference's | [pa3-rendered-markdown.png](https://raw.githubusercontent.com/ccheever/exact2/1536ac9259240e0688841c1b2ae49cc70c8d8ea0/markdown-links-and-files-preview/pa3-rendered-markdown.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/4d88d8e5ce27275caffa5ab93d812331bbdafb1c/markdown-links-and-files-preview/drive-evidence.txt) |
| PA-3 (task) | pass: pressing "Write the tests" checks it and `docs/notes.md` on disk reads `- [x] Write the tests`, byte-identical to the file the reference wrote for the same press | [pa3-task-toggled.png](https://raw.githubusercontent.com/ccheever/exact2/d62f77d6e0a8014f060926c9a5b507f7fa8e7eef/markdown-links-and-files-preview/pa3-task-toggled.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/4d88d8e5ce27275caffa5ab93d812331bbdafb1c/markdown-links-and-files-preview/drive-evidence.txt) |
| PA-3 (review) | pass: in the rendered `docs/flow.md`, `- [ ] **Write** the tests`, ``- [x] `code` first`` and `- [ ] [link](https://example.com) after` are checkboxes (open, done, open; the tree has `task-1`..`task-3` with presses), as the reference; the ```mermaid fence draws its diagram (`mermaid-4`), and Expand diagram opens the "Mermaid diagram" preview. Pressing the bold item checks it and `docs/flow.md` reads `- [x] **Write** the tests`, byte-identical to the reference's write. Before: ☐/☑ glyphs and a code block | [pa3-review-tasks-mermaid.png](https://raw.githubusercontent.com/ccheever/exact2/1a7180739d6fcc55efbd1a673d988fe09dffd173/markdown-links-and-files-preview/pa3-review-tasks-mermaid.png), [pa3-review-task-toggled.png](https://raw.githubusercontent.com/ccheever/exact2/922b7804a886ad5c7d70cbc4649e8c0f88eb4ceb/markdown-links-and-files-preview/pa3-review-task-toggled.png), [pa3-review-diagram-expanded.png](https://raw.githubusercontent.com/ccheever/exact2/e35345cc29401f1889795deae39dde30bb65629f/markdown-links-and-files-preview/pa3-review-diagram-expanded.png), [drive-evidence-r2.txt](https://raw.githubusercontent.com/ccheever/exact2/328ffc3cd3ab58da67338bcd7af1f208759799d1/markdown-links-and-files-preview/drive-evidence-r2.txt) |

The drives: before on the base build (`950e8e2e5`, worktree `t3-code-evidence-base`), after on this branch's bundle
(one live drive), reference on the Electron build over CDP; same lane fixture (`docs/notes.md` with a task list and a
relative file link added in both lanes; restored after each toggle). All rows were driven by agent input; none needs
real input. The review round (`docs/flow.md`) was a second live drive of a new bundle (after the review fixes), on the
same lanes with fresh storage.

## Tests

- `macos/src/markdown_links.rs` (`cargo test -p t3-code-macos --lib`): the transform's table (`fixture.txt:3`,
  `Makefile:12`, `a.ts:12:4`, `tel:`, `javascript:`, `data:`, `ftp://` emptied; bare, slashed, `./`, absolute, `#L`,
  Windows and `file:` paths chips; web, mailto and xmpp links kept; fragments and protocol-relative URLs not chips);
  the TH-9 message's runs (one chip, two targetless links) and its table-cell copy; task items' state, stripped text
  and UTF-16 offsets (a fence's task line and an empty `- [ ]` are not tasks; items that start with emphasis, code or a
  link are tasks; an image chip's `!` before them does not shift an offset), the transcript's -1 offsets and a
  `renderFileMarkdown` source. The ported `markdownLinks.test.ts` cases (`isMarkdownFileLinkLabel_classifies_labels`)
  and A4's slashed case still pass; two shape tests count the new fields.
- `r4-timeline-chips.test.ts`: the TH-9 message gives one chip; the same transform table as the Rust test
  (`markdownLinkHref`); an image's emptied source loads nothing.
- `r4-surfaces.test.ts`: a rendered Markdown file is the chat renderer's `file` source with its folder's chip and its
  fence's highlight; `files-task` writes the toggled text through `projects.writeFile`, the next render reads it, and
  a stale offset writes nothing; `setMarkdownTaskChecked`; an attachment's task items are disabled checkboxes; a
  rendered file's ```mermaid fence asks the module for both themes, is "loading" until it answers, then "rendered",
  and its expand opens the diagram preview.

Checks: see the PR ("Checks").

## Found, not changed

- On the review drive's lane (macOS Dark, the app Light), "Expand diagram" shows the diagram on a dark card where the
  reference shows a white one. The dialog is the transcript's shared `DiagramPreviewDialog` (`timeline-mermaid.contract`,
  `app-overlays.contract`), unchanged here; [app-color-scheme](closed/20261009-app-color-scheme.md) TH-7 owns its card
  colour. Not re-checked on the transcript (one live drive per task).
- A sent attachment's Markdown preview (`r5-panels-attach.ts`) still uses `r4-surfaces-render.ts`; the reference's
  AttachmentFilePreview is ChatMarkdown with no `cwd` (relative links stay plain links, not chips). Not in the
  audit's findings.

## Next action

The coordinator reviews and merges the PR. No real-input rows.
