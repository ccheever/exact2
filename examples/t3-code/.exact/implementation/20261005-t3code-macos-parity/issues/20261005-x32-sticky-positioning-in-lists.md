---
name: 20261005-x32-sticky-positioning-in-lists
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-diff-review-engine, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh]
upstream_url: https://github.com/ccheever/exact2/issues/131
reproduced_on: 4c893fef6
---

# X32: `position: sticky` inside a scroll container and a virtualized list

**Status (reclassified 2026-10-08):** Bucket 5, design pending: #131 waits for a list-lifetime design. Pinned diff file headers are a declared difference until it lands.

## Summary
T3 Code pins headers while their content scrolls: each file header in a diff stays at the top
until the next file pushes it away; the pull request Summary section headings stay while the
section scrolls; the "Changed files" card header sticks inside the transcript. exact2's Contract
has no verified sticky positioning, and a virtualized list admits only keyed rows with one flow
root each, so a header cannot be a sibling that pins to the scrollport. The clone draws headers
as ordinary rows that scroll away. exact2 needs `position: sticky` (with `top`/`bottom`, and
containing-block push-out) for children of a `scroll` and for rows or row subtrees of a virtualized
`list`.

## Why this issue arose

### The T3 Code behavior
- **Diff file headers.** The diff viewer's header is sticky: `[data-diffs-header] { position:
  sticky !important; top: 0; z-index: 4; min-height: 32px … }`
  (`apps/web/src/components/diffs/StyledDiffCodeView.tsx:79-90`) and the viewer option
  `stickyHeaders: true` (`DiffPanel.tsx:1152`, `pullRequest/PullRequestCodeTab.tsx:876`). In a
  long file the reader always sees the file name, status, stat and collapse button; the header
  hovers with an inset left bar (`:hover`), and the next file's header replaces it as it reaches
  the top. Collapse/expand and the "Viewed" checkbox live in that header.
- **PR Summary sections.** `pullRequest/PullRequestSummaryTab.tsx:331` (`sticky top-0 z-10 bg-background`)
  and the ghost at `PullRequestGhosts.tsx:380`. Collapsing a pinned section keeps the pressed
  heading under the pointer by adjusting the scroll offset (`pullRequestSummaryScroll.logic.ts`
  `sectionCollapseAnchorScrollTop`; tests: "returns the section's natural offset when its heading is
  pinned", "allows a fractional sticky-edge difference", "leaves the scroll position alone before
  the heading becomes sticky", "does not produce a negative scroll offset").
- **Timeline and other surfaces.** The "N changed files" card header in the transcript
  (`chat/ChangedFilesTree.tsx:52`, `sticky top-2 z-10`); the CSV preview header row
  (`files/DelimitedTablePreview.tsx:31`); Diagnostics tables (`settings/DiagnosticsSettings.tsx:359,585`,
  `settings/ResourceTelemetryDiagnostics.tsx:588,691`); the project content search's group headers
  (`search/ProjectContentSearchDialog.tsx:255`).

### What exact2 does today
- Not in `EXACT2-GAPS.md` (found while writing the plan's tickets, 2026-10-05).
- Bundled library (`20261005-platforms-v3`): `capabilities.md` lists "Vertical/horizontal
  virtualization | Supported with compiler-enforced shape restrictions" and "Arbitrary CSS/browser
  APIs | Not generally supported by implication — use admitted vocabulary"; `layout-and-interaction.md`:
  "For virtualization use `list virtualized=true`, one direct keyed `each`, and one flow root per
  row." Sticky positioning is **not covered: unknown**. Whether `position="sticky"` is admitted
  by the compiler on the pinned `main` is to confirm at `issue-open`
  (`bun exact.mjs contract vocab --json position`).
- Observed in the clone: no Contract file in the mc-orch tree sets sticky positioning (grep of
  `*.contract`, 2026-10-05); every header listed above is an ordinary row.

### Where the clone hits it
`diff.contract` `DiffRow`/`DiffFileHeader` (the flat virtualized `diffItems` list: file header,
code rows, gaps, padding as sibling items, `diff.contract:126-140,289`); `pages-pr-detail.contract`
`PrdSectionHeader` (Summary headings, plain buttons); the transcript's changed-files card
(`timeline.contract:198-200` `ChangedFiles`). Current behavior: headers scroll away with their content. A pinned
overlay computed from the scroll offset and row geometry is conceivable but needs a live offset
(X23d), row frames for wrapped lines (X22) and a push-out calculation; it is not claimed to match
the reference and no such code exists.

## Why it must be resolved
The parity goal covers the diff and pull request surfaces, where file context while scrolling is
the main reading aid. Without it a long diff loses the file name after the first screen, the
pinned-heading scroll compensation of the Summary tab has nothing to compensate, and every
pixel pair mid-scroll differs. Plan rows that wait: the pinned-header rows of
`20261005-diff-review-engine`, `20261005-pr-code-tab` and `20261005-pr-conversation-and-refresh`
(Summary headings); the timeline card header and the CSV/diagnostics headers are not yet assigned
to a ticket and are checked at those tickets' `prepare`. There is no matching workaround, so the
difference stays visible until the capability exists.

## Requested support
The web way: `position: sticky` with `top`/`bottom`/`inset-*`, `z-index`, and the usual
containing-block rule (a sticky element sticks within its parent's box and is pushed out by the
next sibling's section), working for:
1. a child of a `scroll` container;
2. a node inside a virtualized `list` row, where the "parent box" is a section that spans
   several rows (a file's rows) — A: let a keyed group of rows share a sticky header
   (`<each>` item with a header node and a body range), or B: a list-level
   `sticky-header` role for an item that pins until the next item with the same role arrives
   (the iOS/UIKit "section header" model).
macOS host first; web already has it; iOS follows the same model.
Trade-off: (1) is small and mirrors CSS; (2) is the real need for diffs and needs a decision
about how virtualization measures a pinned item.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: a `scroll` containing three
sections, each with a header box `position="sticky" top=0` and 40 text rows. Scroll down 300 pt.
Expected (Chrome): the current section's header is pinned and the next header pushes it out.
Actual: unknown — compile result of `contract build` and the layout after scrolling. Repeat with
a `list virtualized=true` whose items are `header`/`row` kinds. Clone scenario: open the Changes
panel on a thread with a diff of at least three files and 200 lines in the first, scroll with
`scroll` (agent) and `layout diff-file-<path>`; compare with the oracle.

## Acceptance for the fix
- Conformance case against Chrome: pinned and pushed-out header frames at three scroll offsets
  for (1) and (2).
- Agent: `layout` of the header at scroll offsets 0, 300 and 900 shows it at the viewport's top
  while its section is in view; screenshots match the oracle.
- Clone pairs: Changes, Code tab and PR Summary mid-scroll at 1280×840 and 840×620.

## App adoption after resolution
Mark the file header, Summary section headings (and, when assigned, the changed-files card and
table headers) sticky in `diff.contract`, `pages-pr-detail.contract`, `timeline.contract`;
port `sectionCollapseAnchorScrollTop` with its four tests; unblock the pinned-header rows of the
three tickets above. `issue-close` verifies the rows and the pairs.

## Status and next action
Published 2026-10-06 as [#131](https://github.com/ccheever/exact2/issues/131) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Decided upstream (2026-10-08): declared difference until #131 lands on main

[Charlie on #131](https://github.com/ccheever/exact2/issues/131#issuecomment-6055588484): "Defer virtualized section headers until a list-lifetime design is selected. … Choose grouped
containment with its CSS behavior, or explicitly declare a section-header extension."
- **Correction to "What exact2 does today":** sticky positioning is not unknown. Since LLP 1083, `position="sticky"`
  works inside a `scroll` and inside one row; `pages-pr-summary.contract:282` uses it. The gap is only a header
  pinned over several rows of a virtualized list.
- **Declared difference until #131 lands on main** (user decision, 2026-10-08): the pinned diff file headers of
  diff-review-engine and pr-code-tab scroll away with their file. Waits for main fix of [#131](https://github.com/ccheever/exact2/issues/131).
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): explicitly deferred pending a list-lifetime design; kept open.
