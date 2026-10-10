---
name: 20261005-x33-transcript-selection-range
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap (unconfirmed)
blocks: [20261005-diff-review-engine]
upstream_url: https://github.com/ccheever/exact2/issues/132
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/274
---

# X33: Selected text, its source message and UTF-16 offsets, and its end rectangle from the rendered transcript

Moved to main `issues/20261009-text-selection-rectangles-commands.md` (2026-10-09); tracked there.

## Summary
In T3 Code the user can select text in an assistant answer and press a floating "Cite" button; the
quote enters the composer as a chip that stores the message id, the exact text, its UTF-16
start/end in the message's rendered text and 32 characters of context on each side, and can carry
a comment. When the clone was planned, exact2 offered no way for an app to read a selection inside
rendered content, so the clone could insert a citation chip and jump to a cited answer but not create one.

## Why it arose

### The T3 Code behavior
- **Trigger.** A non-collapsed selection that lies inside one assistant message of an open
  thread (not a draft): mouse drag, double/triple click (the button waits 500 ms for a multi-click
  to finish), or keyboard selection. Source: elements marked `data-assistant-citation-source=<message id>`
  inside the timeline viewport (`apps/web/src/components/chat/AssistantCitationSource.tsx:371`,
  `MessagesTimeline.tsx:1329-1342`). A selection spanning two messages, selecting only controls or
  hidden text, or scrolled out of view shows nothing (`AssistantSelectionToolbar.tsx:46-79`).
- **Selector.** `apps/web/src/lib/assistantTextSelection.ts`: `captureAssistantTextSelection`
  (:212-244) reads the ordered range, ignores controls/hidden/`aria-hidden`/svg text
  (`EXCLUDED_SELECTOR`, :36-37), and builds `{ text, start, end, prefix, suffix }` with
  `createAssistantTextSelector` (:52-84): positions and context are UTF-16 units in the text with every
  whitespace run collapsed to one space; context is up to 32 units (`ASSISTANT_CITATION_CONTEXT_LENGTH`).
- **Button.** `AssistantSelectionToolbar.tsx`: floating `glass` button "Cite", placed at the pointer
  release or 4 px below the selection's last rectangle, clamped 8 px inside the window and the timeline
  (`lib/selectionActions.ts:5-30`); over 8,000 UTF-16 units it reads "Shorten selection"; Tab moves focus
  to it, Escape dismisses; the selection is cleared after a successful cite.
- **Result.** The chip is inserted at the composer caret (`ChatView.tsx:1785-1799`); its popover
  "Comment on selected text" (`AssistantCitationCommentEditor.tsx`) saves with Enter, saves and sends with ⌘↵.
- **Tests (reference).** `lib/assistantTextSelection.test.ts` (31 cases), `lib/selectionActions.test.ts` (22),
  `chat/assistantCitationCommentDismissal.test.ts` (6), `packages/shared/src/assistantCitations.test.ts`.

### Where the clone hit it
The transcript renders each Markdown block as `text` runs and boxes (`markdown.contract` `ChatRuns`/`FlowRuns`),
so one answer is many nodes. Clone pieces that existed: `composer-editor.ts` `insertContext('citation')`,
`r4-timeline-chips.ts` (renders the chip from the href, "View source"), `r5-composer-citation.ts` (`citedRow`,
`revealCitation`). `20261005-diff-review-engine` held the Cite rows (G3).

## Clone workaround
[diff-review-engine](../../tasks/closed/20261005-diff-review-engine.md) built Cite (G3: `diff-citations.ts`,
`diff-cite.contract`, `markdown.contract`) on `text`'s `selectionchange`: "Cite" shows under the selection's
block, not under the selection's end (no selection rectangle on main), and `retainFocus` keeps the selection;
the press inserts an `[Assistant quote](t3-citation://…)` chip with the reference selector. Declared
differences: the selection is not cleared after citing (no `removeAllRanges` on main); the button sits under
the block. Once main gives selection rectangles and the web's selection commands, Cite moves to the selection's
end and clears the selection.

## Evidence and history
- Filed as [#132](https://github.com/ccheever/exact2/issues/132); closed after main #171 fixed part 1 only (a
  press on a `button` keeps the selection). Nothing to adopt in `20261007-adopt-main-fixes-input`: Cite's
  `retainFocus=true` is the reference's `onPointerDown` `preventDefault()`, not a workaround.
- Re-checked 2026-10-07 on main `cff90b364` ([adopt-main-fixes-r3](../../tasks/closed/20261007-adopt-main-fixes-r3.md)):
  parts 2 (the end rectangle) and 3 (clearing the selection) still missing.
- The rest filed as [#274](https://github.com/ccheever/exact2/issues/274) on 2026-10-08, reproduced on main
  `0365ad1a4`: part 1 still passes; `setSelectionRange` on a `text` logs `refused: not a text field` on the web
  and macOS, `clearSelection()` is `type-unknown-command`, `Selection.rect`/`.rects` are `type-unknown-field`.
- Decision on #274 (2026-10-08): the web's names (`removeAllRanges`, `setBaseAndExtent`), not `clearSelection()`.
