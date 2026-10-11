---
name: 20261011-diff-cite-popover
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-diff-cite-popover
pr_url: null
verified_commit: null
---

# Cite: the citation chip's comment popover, and the rest of Cite

## Outcome

The user decided on 2026-10-11 that both items are built now, matching the reference T3 Code. Source record:
[diff-review-engine](closed/20261005-diff-review-engine.md) (G3 Cite: "the chip's comment popover is not built (native
composer chip; needs a popover anchored to a chip, X22 #127)"; "blocks with inline code (word-by-word texts) and code
blocks are not citable; the selection is not cleared after citing (#132)").

| Id | Clone (feature tip `fe0e39c91`) | Reference (check it first) |
| --- | --- | --- |
| DC-1 | Cite puts an `[Assistant quote](t3-citation://…)` chip at the composer caret and nothing else: no comment editor opens, the chip has no pencil, its label is the quote cut at 40 characters, and a chip's comment can neither be added nor edited | Cite inserts the chip and opens the comment popover "Edit citation comment" at once under the cited text (side bottom, align end), the focus in its textarea "Comment on selected text" ("Add an optional comment...", Cancel, Save); the cited text keeps a highlight while it is open. Enter saves (the chip then reads the comment), ⌘↵ saves and sends, ⇧↵ breaks the line, Escape or Cancel removes the chip just cited, a press outside commits a changed draft, past 8,000 characters "Comments can contain up to 8,000 characters." and "Shorten comment". The composer chip ends with a pencil button ("Add comment to citation" / "Edit citation comment") that opens the same popover above the chip (side top, align end); there Escape and Cancel only close it |
| DC-2 | A selection in a block with inline code (laid out word by word) or in a code block offers no Cite; after citing, the selection stays | A selection anywhere in an answer's text (inline code, code blocks) is citable; the selection is cleared after citing. Recorded as blocked on X33 (#274: no selection rectangle, no programmatic clear) and X22 (#127: no reactive geometry): build every part the clone can, record precisely what stays blocked |

## Steps

1. Reference first: the lane reference with a Cite fixture answer (paragraph with inline code, a code block, a closing
   paragraph that wraps) over CDP: Cite, the popover's place and metrics, keys, removal on Escape, the outside press, the
   pencil, the chip label, light and dark.
2. DC-1, native chip (`T3ComposerText.swift`, `T3ComposerStyler.swift`, `T3ComposerChipPress.swift`): the citation chip's
   label (comment or quote, 64 characters, 16em), its pencil action box and accessibility button; a press on the pencil
   opens the popover at the chip; a press elsewhere in the text while it is open asks the app to settle it; ops to open
   it at the source after Cite, to replace or remove the chip, to send.
3. DC-1, app (`citation-comment.contract`, `app-window.contract`, `composer-chip-popover.ts`, `diff-review.ts`): the
   popover drawn in the window layer, its draft, keys, Save, Cancel, the outside press, Escape, ⌘↵; Cite opens it under
   the cited text.
4. DC-2 (`markdown.contract`): `selectionchange` on the word-by-word texts and on code blocks' text, offsets in the
   answer's text stream as the reference reads it; the selection's clearing after citing.
5. Tests (Bun, AppKit), one agent drive, before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DC-1 pencil and label | reference comparison; AppKit test of the chip (label, width, pencil press, accessibility); agent drive | images |
| DC-1 popover from Cite | reference comparison; Bun tests of the ops; agent drive: select, Cite, type, Enter; Cite, Escape | images |
| DC-1 popover from the pencil | the same, the pencil press | images |
| DC-1 source highlight | reference comparison; agent drive | images |
| DC-2 inline code and code blocks | Bun test of the offsets against the reference's stream; agent drive: select across inline code, Cite | images |
| DC-2 clearing the selection | agent drive (`tree` / screenshot after Cite) | images |
| DC-2 blocked rest | recorded with the issue numbers | text |
| DC-1 and DC-2 under real input | real-input steps for the coordinator's session | — |

## Reference

- `apps/web/src/components/chat/AssistantCitationChip.tsx` (the chip, the pencil `ContextChipAction`, the popover:
  `side={activeSourceAnchor ? "bottom" : "top"} align="end"`, `width="md"` (w-80), `padding="compact"`, `aria-label="Edit
  citation comment"`, `finalFocus` back to the editor), `AssistantCitationCommentEditor.tsx` (textarea, keys, the 8,000
  limit, Cancel and Save), `assistantCitationCommentDismissal.ts`, `AssistantCitationSource.tsx`
  (`observeAssistantCitationCommentSource`: the cited text's highlight while the editor is open), `AssistantSelectionToolbar.tsx`
  (Cite, `removeAllRanges()` after citing).
- `apps/web/src/components/ComposerPromptEditorTiptap.tsx` (`ComposerCitationNodeView`: save, `removeOnCancel` after Cite,
  `onRestoreFocus`; `requestCitationComment`), `components/chat/ChatComposer.tsx` (`citeAssistantText`,
  `submitCitationAndSend`), `components/ContextChip.tsx` (`ContextChipLabel` `max-w-[16em]`, `ContextChipAction`),
  `lib/assistantTextSelection.ts` (`readAssistantText`: the answer's text stream), `index.css`
  (`::highlight(t3-assistant-citation-comment)`: primary at 45%), `components/ui/popover.tsx`.
