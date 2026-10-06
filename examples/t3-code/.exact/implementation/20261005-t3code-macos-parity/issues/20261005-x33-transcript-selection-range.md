---
name: 20261005-x33-transcript-selection-range
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-diff-review-engine]
upstream_url: null
reproduced_on: null
---

# X33: Selected text, its source message and UTF-16 offsets, and its end rectangle from the rendered transcript

## Summary
In T3 Code the user can select text in an assistant answer and press a floating "Cite" button; the
quote enters the composer as a chip that stores the message id, the exact text, its UTF-16
start/end in the message's rendered text and 32 characters of context on each side, and can carry
a comment. exact2 offers no way for an app to read a selection inside rendered content. The clone
can insert a citation chip and jump to a cited answer, but cannot create one from a selection.
exact2 needs a selection fact for rendered text (Selection API analogue): the selected string,
where it starts and ends inside identifiable nodes, its end rectangle, and a way to clear it.

## Why this issue arose

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
  `createAssistantTextSelector` (:52-84): the exact quote keeps its whitespace and line breaks,
  while positions and context are UTF-16 units in the text with every whitespace run collapsed to
  one space; context is up to 32 units (`ASSISTANT_CITATION_CONTEXT_LENGTH`), never splitting a
  surrogate pair. `resolveAssistantCitationRange` (:263-282) finds the quote again later
  (`findAssistantCitationText`, repeated quotes need context to disambiguate).
- **Button.** `AssistantSelectionToolbar.tsx`: floating `glass` button "Cite" with a quote icon,
  placed at the pointer release or 4 px below the selection's last rectangle, clamped 8 px inside the
  window and inside the timeline (`lib/selectionActions.ts:5-30`); `aria-label` "Cite selection in
  composer"; over 8,000 UTF-16 units (`ASSISTANT_CITATION_MAX_TEXT_LENGTH`) it is disabled, reads
  "Shorten selection" with `aria-label` "Selection is too long to cite"; Tab moves focus to it,
  Escape dismisses, pressing it does not clear the selection until it succeeds.
- **Result.** The chip is inserted at the composer caret (`ChatView.tsx:1785-1799`); if the
  composer cannot take it, a warning toast "The composer is not ready" ("Try citing the selection
  after the connection or pending input is resolved."). The chip's popover "Comment on selected
  text" (`AssistantCitationCommentEditor.tsx`) saves with Enter, saves and sends with ⌘↵, limit
  8,000 characters.
- **Tests (reference).** `lib/assistantTextSelection.test.ts` (31 cases, e.g. "keeps exact
  multiline Unicode text and omits interior controls and hidden text", "uses UTF-16 offsets for emoji
  and combining characters"), `lib/selectionActions.test.ts` (22),
  `chat/assistantCitationCommentDismissal.test.ts` (6),
  `packages/shared/src/assistantCitations.test.ts`.

### What exact2 does today
- Not in `EXACT2-GAPS.md` as its own item. Related text in its X20 detail (written from
  framework source at `c1522fdac`, checked against `main` `d2cb661eb`): "A textarea has no
  caret/selection events (`selectionchange` is on `text` only)". Whether that event gives a range
  with offsets, and whether it spans several `text` nodes, is **to confirm at `issue-open`**.
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**. Not reproduced.
- Observed in the clone (code reading, mc-orch tree 2026-10-05): the transcript renders each
  Markdown block as `text` runs and boxes (`markdown.contract:311-345` `ChatRuns`/`FlowRuns`), so
  one answer is many nodes; each assistant row is registered as a `cite:` jump target
  (`timeline.contract:307`, `modules/apple/T3TimelineTurns.swift`). Whether the pointer can select
  and copy transcript text at all was not checked by the planner.

### Where the clone hits it
Clone pieces that exist: `composer-editor.ts:345-369` `insertContext('citation')` (inserts
`[Assistant quote](t3-citation://v1/…)`), `r4-timeline-chips.ts:67-85` (renders the chip from the
href, "View source"), `r5-composer-citation.ts` (`citedRow`, `revealCitation`, the "quote no longer
reads" warning). Missing: the selection fact, the Cite button, the chip's comment popover, the href
encoder (`formatAssistantCitationHref`, in `packages/shared/src/assistantCitations.ts`), and the
quote re-resolution. There is no workaround; `20261005-diff-review-engine` holds the Cite row.

## Why it must be resolved
Cite is a headline desktop feature of the answer view (G3 in the audit), and the clone's partial
citation support is useless without a way to create one. The ticket `20261005-diff-review-engine`
splits Cite into its own ticket if this cannot be solved; its acceptance rows (Cite button,
chip comment, too-long and no-composer cases) are blocked until a selection fact exists. No
workaround matches: copying the selection and pasting is not the same feature, and a context
menu "Cite" item would not match the floating button or keyboard path.

## Requested support
The web way (`Selection`/`Range` API, `selectionchange`, `Range.getClientRects()`), macOS host first:
- **A (preferred).** A `selectionchange` event on a container node whose payload, for a selection
  inside that subtree, gives: the selected string (with line breaks), start and end as
  `(nodeId, utf16Offset)` pairs in node-local text, an ordered list of the container's text nodes
  in document order (or their concatenated length so the app can compute global offsets), the
  rectangle of the last line of the selection, and `collapsed`; plus `clearSelection()` and
  `select(startNode, startOffset, endNode, endOffset)` for the "View source" highlight; works across
  `text`, inline spans and code blocks, and skips `aria-hidden`/`inert` content.
- **B.** Expose the same facts through a native hook on the transcript scroll view
  (`ExactElement` text storage) so the app's Swift module reads the host's own selection; smaller
  for the framework, limited to the macOS app.
Trade-off: A is portable and testable against Chrome; B may suffice if transcript text is native
(to confirm) but repeats for every surface that needs selection.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: a column containing two
paragraphs, each made of three `text` runs (one bold, one inline-code box), with a
`selectionchange=` handler that records the payload. Steps: drag from the middle of paragraph 1 to
the middle of paragraph 2; double-click a word; Shift+→ ten times. Expected (Chrome's
`getSelection()` on the same structure): the string and start/end positions. Actual: to confirm.
Clone scenario: any thread with a long answer; select a sentence; expected (reference) a "Cite"
button at the release point; actual none. The agent cannot drag or double-click native text (X8),
so the part that needs a real pointer is an attended session.

## Acceptance for the fix
- Minimal app: payload matches Chrome for a same-block selection, a cross-block selection, a
  selection starting in an inline-code run, emoji and combining characters (UTF-16 units), and a
  backward drag; `clearSelection()` leaves no highlight.
- AppKit test with a constructed selection on a rendered answer.
- Clone: the Cite row of `20261005-diff-review-engine` passes on a real pointer (attended
  session) and the ported tests of `assistantTextSelection` pass on the app's offsets.

## App adoption after resolution
Implement G3 in `20261005-diff-review-engine`: port `createAssistantTextSelector`,
`findAssistantCitationText`, `resolveAssistantCitationRange`, `resolveSelectionActionPosition` and
`packages/shared` `assistantCitations`; replace the regex in `r4-timeline-chips.ts` with
`parseAssistantCitationHref`; add the Cite button and the chip popover. Nothing to remove.
`issue-close` verifies the Cite rows.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's
approval; publication only after approval).
