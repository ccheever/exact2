---
name: 20261005-diff-review-engine
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Diff panel engine: folder tree, large diffs, line comments and text citations

## Outcome

The thread Diff panel and the Files preview behave like the reference's. The diff has a folder
tree and pinned file headers; a diff larger than the server's preview limit loads four files at
a time with ghosts, a per-file Retry and a "partial" mark; "N unmodified lines" separators
expand by loading the file's contents; a line or a line range in a diff or a file can be
commented, and the comment becomes a review-comment chip in the thread's composer. Selecting
text in an assistant message offers "Cite", which puts a citation chip in the composer; the
chip carries an optional comment.

## Scope and exclusions

Included: **G13** (folder tree with "Files" header, count, Expand/Collapse all folders, status
tint, reveal on select; pinned file headers; no word-level marks if the oracle confirms
`lineDiffType: "none"`), **G4** (lazy per-file patches, retry, partial mark, size banner,
clickable unmodified-line separators), **G2** for the thread Diff and the Files preview,
**G3** (Cite button, citation chip comment editor).
Excluded: the pull request Code tab and host line comments (`20261005-pr-code-tab`); Browser
annotations; the citation "View source" jump (done: `r5-composer-citation.ts`).
Reuse (done): `parsePatch`, `diffSnapshot`, scope menu, wrap/layout/whitespace toggles
(`diff.ts`, `diff.contract`), Files preview rows (`r4-surfaces-files.ts`, `R4CodeRow`), review-comment chip
rendering (`r4-timeline-chips.ts:113`, `composer-editor.ts:105-114,282-293`), `insertContext('citation')`
(`composer-editor.ts:345-369`), citation chips and "View source" (`r4-timeline-chips.ts:67-85`).

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `D/` = `apps/web/src/components/diffs/`):
`D/DiffFileTree.tsx:80-205`, `diffFileTree.logic.ts`, `StyledDiffCodeView.tsx:79-90`, `AnnotatableCodeView.tsx:104-300`, `DiffCommentAnnotation.tsx`, `commentSubmitShortcut.ts`,
`useReviewFilePatches.ts`, `DiffFileStatus.tsx`, `DiffFileLoadingBoundary.tsx`; `apps/web/src/components/DiffPanel.tsx:394-495,987-1153`; `apps/web/src/lib/diffFileContents.ts`;
`apps/web/src/components/files/FilePreviewPanel.tsx:579-830`, `fileCommentAnnotations.ts`; `apps/web/src/reviewCommentContext.ts`; `apps/web/src/lib/composerContextRecords.ts:69-215`;
`apps/web/src/components/chat/AssistantSelectionToolbar.tsx`, `AssistantCitationCommentEditor.tsx`, `AssistantCitationChip.tsx:148-165`, `apps/web/src/lib/assistantTextSelection.ts`, `selectionActions.ts`,
`packages/shared/src/assistantCitations.ts`; server `GitVcsDriverCore.ts:58-60` (preview cap 120,000 bytes, per-file cap 1 MiB), RPC `review.getDiffPreview` (optional `file`), `review.getDiffFileContents`.
Clone paths `examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (virtualized list restrictions: one keyed `each`, one flow root per row), state-and-data (late old replies; single-flight reads), components (child state lifetime), design (all states), motion (collapse chevron), accessibility, testing-and-debugging. Unknown in the library: native selection read, text-view overlays, Swift timeline hooks (`T3TimelineTurns.swift`); the clone's runtime evidence on the pinned main is the basis.
Observed today: flat tree of basenames (`diff.ts:248`, `diff.contract:146-152,343-352`); header is an ordinary row (`diff.contract:289`, not pinned); word marks drawn (`diff.ts:151-165`, `diff.contract:341`) while the reference passes `lineDiffType: "none"` (`DiffPanel.tsx:1147`); `adoptDiff` keeps text and `truncated` only (`diff.ts:95-100`), the banner shows for every truncated source (`diff.contract:127`); separators are static labels (`diff.ts:224`, `diff.contract:277`); no gutter interaction in diffs or Files; no selection toolbar. The `truncated` banner in the reference shows only without per-file stats (older servers, `DiffPanel.tsx:987`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle can select text and hover) | pending |
| scheduling preference | [20261005-main-fix-adoption](20261005-main-fix-adoption.md) | pending | Merged first (popover/tooltip/sidebar Contract) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| recorded decision | Parser basis: adapt the clone's `parsePatch` model (recommended) or vendor `@pierre/diffs`'s parser (needs a license and size check) | none | User decides at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X33](../issues/20261005-x33-transcript-selection-range.md) | Selected text with its message id and UTF-16 start/end from the rendered transcript (Cite) | Not covered by the library; the transcript is `T3TimelineTurns.swift` | unknown | Spike at `prepare`; if the native view cannot supply it, Cite is held for a user decision (no matching workaround) and G3 becomes its own ticket |
| [X20](../issues/20261005-x20-rich-text-editing.md) | Inserting a citation chip at the composer caret | `insertContext('citation')` already inserts (`composer-editor.ts`) | nonblocking (workaround exists) | Reuse |
| [X23](../issues/20261005-x23-scroll-restore-offsets.md) | Reveal a file from the tree (scroll to item, smooth landing) | Main has `scrollIntoView(id, block, behavior)` (EXACT2-GAPS) | nonblocking (workaround: `R9Input`/jump ops) | Reuse |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) / [X24](../issues/20261005-x24-still-pointer-rehover.md) | Gutter "+" appears on hover; after layout changes under a still pointer | `t3-rehover` hook exists | nonblocking (workaround exists) | Reuse |
| [X22](../issues/20261005-x22-reactive-layout-facts.md) | Place the Cite button at the selection; the chip comment popover at its chip | `t3-anchor`/`t3-frame` hooks | nonblocking (workaround exists) | Reuse |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md), [X16](../issues/20261005-x16-smart-substitutions-off.md) | ⌘↵/Escape/composition and exact bytes in the annotation textarea | `t3-plain-text` hook | nonblocking | Reuse |
| [X8](../issues/20261005-x08-agent-pointer-native-views.md) | Agent cannot drag-select in native views | — | nonblocking (workaround: attended rows) | Mark rows |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327/1,500 | nonblocking | New files, no new root resource |
| [X32](../issues/20261005-x32-sticky-positioning-in-lists.md) | Sticky file headers in a virtualized list | Not established; `bun exact.mjs contract vocab --json position` | unknown | If unsupported, the pinned-header row is held for a user decision (no matching workaround) |
| [X16](../issues/20261005-x16-smart-substitutions-off.md) | Exact typed bytes in the line-comment and citation-comment textareas | X16 | nonblocking (workaround: the `t3-plain-text` hook on these textareas) | remove the hook when X16 is adopted |

## Implementation notes

- **Port** (names, tests, header records changes): `diffFileTree.logic.ts` (`diffFileTreeEntries`, `collectDirectoryPaths`, `diffFileTreePositions`, `compareDiffFileTreeEntries`; `buildDiffFileTreeUpdates` only if the tree keeps open folders across slices), `lib/diffFileContents.ts` (`createGitDiffFileContentsLoader`),
  the `useReviewFilePatches` rules (first 4 files, then 4 per boundary hit, `settledFileCount`, per-file `error`/`truncated`, retry by path, scope key), `reviewCommentContext.ts` (`formatReviewCommentFence`, `buildFileReviewComment`, `inferReviewCommentFenceLanguage`, `buildDiffReviewComment`, `restoreDiffReviewCommentRange`), `composerContextRecords.ts` (`reviewCommentContextLabel`, `reviewCommentContextRecord`), `fileCommentAnnotations.ts`, `commentSubmitShortcut.ts`,
  `packages/shared/src/assistantCitations.ts` (`formatAssistantCitationHref`, `parseAssistantCitationHref`, `withAssistantCitationComment`, `serializeAssistantCitation`, `collectAssistantCitations`, `assistantCitationsToPlainText`, `expandAssistantCitationsForProvider`, `renderAssistantCitationsAsText`; replaces the regex in `r4-timeline-chips.ts:76`), `assistantTextSelection.ts` pure parts (`createAssistantTextSelector`, `findAssistantCitationText`, `resolveAssistantCitationRange`), `assistantCitationCommentDismissal.ts`, `selectionActions.ts` (`resolveSelectionActionPosition`). The DOM-bound parts (`captureAssistantTextSelection`, `observeSelectionActions`) are replaced by a native selection fact.
- **Diff rows.** Extend the flat virtualized item list (`diffItems`) with kinds for annotation rows (draft, saved) inserted after the range's end line, ghost rows, and clickable gap rows; the model keeps the reference's anchor rule (side of the end line; one annotation per line, entries stacked). A draft disables the gutter and line selection until closed or sent.
- **Annotation card.** Draft: textarea (`aria-label` "Comment on lines <range>", placeholder "Add a comment…", hint "⌘/Ctrl Enter to send", Cancel, "Comment", Escape cancels); saved: left border card with message icon and a trash button (`aria-label` "Delete comment", visible on hover or focus). Saving calls the ported builder and registers the record the way `prRecords` does (`composer-editor.ts:105`), inserting the chip at the caret; deleting removes record and chip (confirm in the oracle).
- **Files preview.** Same card under the end line of the selected range; comment stays tied to its text through edits (the reference remaps annotations on edit); if the native editor cannot supply a line remap, record a new gap rather than freezing comments.
- **Lazy loading.** Initial `review.getDiffPreview` unchanged; when `truncated` and `files` exist, request `file:{path,previousPath,sourceKind}` for the first four files, then four more when the boundary row nears view or a tree click reveals a later file; show per-file stats from `files`; icon buttons `aria-label` "Retry loading diff" / "Partial diff preview". Separator click calls `review.getDiffFileContents` (single-flight per file) for git sources only (not turn diffs); expansion step and failure display are taken from the oracle.
- **Tree.** Right-hand column as today (width 216): header "Files" + count, `aria-label` "Collapse all folders"/"Expand all folders", folders flattened when empty, initially open, status tint, selection reveals the file (expands, loads, scrolls); clicking the selected row again reveals again.
- **Cite.** Native selection fact → floating button `aria-label` "Cite selection in composer" ("Selection is too long to cite", label "Shorten selection" over 8,000 UTF-16 units); Tab moves focus to it, Escape dismisses; chip popover `aria-label` "Add comment to citation"/"Edit citation comment", textarea "Comment on selected text" (Enter saves, ⌘↵ saves and sends, Shift+Enter newline, 8,000 limit message).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Folder tree | Lane repo `diff-demo`: 12 changed files under `src/ui/…`, `docs/`, one added, deleted, renamed | `bun scripts/agent.mjs macos --json "tap diff-tree-toggle" tree` then tap a file | "Files 12", folders flattened, collapse/expand all, status tint, tap reveals and expands the file | macOS 1280×840 | tree JSON, shots |
| Pinned headers | Long file | Scroll with `scroll`; `layout diff-file-<path>`; `(attended session)` for the real wheel | Header stays at the top of the viewport | macOS; attended row on a lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | shots, recorded steps |
| Lazy loading | `diff-demo` patch >120,000 B (git source `truncated`) | Open Changes; scroll; delete one file between list and read; press Retry | Four files then more; skeleton headers; stats from `files`; Retry icon; trace shows `review.getDiffPreview` with `file` ×4 then ×4 | macOS | trace, shots |
| Expand unmodified lines | File with 3 hunks | Tap a separator | One `review.getDiffFileContents`; rows appear as in the oracle; error path matches | macOS | trace, pair |
| Diff line comment | Thread on `diff-demo` | Gutter "+" or drag line numbers `(attended session)`; type; ⌘↵ | Chip "<basename> L12 to L14" in the composer; sending carries the record (`fenceLanguage: diff`, range, text) equal to the reference's; delete removes it | macOS; attended part on a lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | `target/t3-ui-parity/trace-diff.mjs diff-comments`, shots |
| Files comment | Preview of `README.md` | Comment lines 3-5 | Chip "README.md L3 to L5"; behavior while editing as the oracle | macOS | trace, shots |
| Cite | Thread with a long answer | Select text `(attended session)`; press "Cite"; edit the chip comment | Chip href round-trips `parseAssistantCitationHref`; popover keys as listed; too-long and no-composer cases | macOS; same lane build and environment | recorded steps, shots |
| Visual parity | Oracle on the same repo | Pairs: tree open/closed, sticky mid-scroll, ghosts, separators, draft and saved cards, Cite button | 1280×840 and 840×620, light and dark; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | pair table |
| Ported tests | `bun test` | Original names: "diffFileTreeEntries", "collectDirectoryPaths", "diff tree reading order", "createGitDiffFileContentsLoader", "review comment context parsing", "createAssistantTextSelector", "findAssistantCitationText", assistant citation round-trip cases, "resolveAssistantCitationCommentDismissal" cases, `commentSubmitShortcut` | Pass; DOM-bound cases classified n/a-ui | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab through tree, headers, annotation card and Cite button (Tab reaches it from the selection); Escape cancels a draft, dismisses Cite, closes the chip popover | Focus visible and returned to the trigger; nothing is sent on Escape; chevron changes without rotation | macOS | `tree --ax`, state |
| Gates | `git add -A` | Clone checks (incl. AppKit binary for the selection fact); `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/diff.ts`, `diff.contract`, new `diff-tree.*`, `diff-lazy.ts`, `diff-comments.*`, `r4-surfaces-files.*`, `composer-editor.ts`, `r4-timeline-chips.ts`, `modules/apple/T3TimelineTurns.swift` (selection fact), `apple/tests/`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, git, reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-clone-on-exact2-main`, `20261005-desktop-oracle-and-trace` and `20261005-hot-file-split` merge: run the selection spike and the sticky-vocabulary check; if the spike fails, split G3 into its own ticket.
