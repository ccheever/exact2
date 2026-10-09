---
name: 20261009-markdown-links-and-files-preview
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

## Next action

TH-9 and PA-3 can start now. Prepare a branch from `feat(example)/t3-code`. Build
and unit-test. Then do one batched live drive at the end for every row's before/after pair. Close every row in this PR,
or record the blocker of a row that cannot pass.
