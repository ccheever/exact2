---
name: 20261011-diff-cite-popover
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
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

## Cause and fix

**DC-1, the chip.** The native composer draws the chip from its hidden source link (`T3ComposerText.chips`). Its label
was the `text` query value cut at 40 characters, read with `URLComponents`, which leaves `URLSearchParams`' "+" for a space
(the base drew "closing+paragraph+after+the+code+block,…"). Now `citationQuery` reads the query as `URLSearchParams` does
and `citationLabel` is AssistantCitationChip's: the comment, else the quote, whitespace runs as one space, 64 UTF-16 units
and "…"; the drawn label stops at 16em with a tail ellipsis (ContextChipLabel `max-w-[16em] truncate`), and the chip ends
with its comment button (ContextChipAction: the chip's 0.33em gap, `ml-[0.17em]`, a 1.17em box, lucide's pencil at
0.85em, the chip's ink), which is also an accessibility button "Add comment to citation" / "Edit citation comment"
(`T3ComposerStyler.swift`, `T3ComposerIcons.swift`, `T3ComposerChipPress.swift`).

**DC-1, the popover.** The skill chip's press path (`T3ComposerChipPress`) now carries citations: a press on the comment
button selects the chip (the reference's node selection) and opens its comment anchored at the button; `editorCiteOpen`
opens the chip Cite just inserted anchored at the cited text, with `removeOnCancel`; `editorChipFinish` is the app's Save
(the chip's source rewritten), Cancel and Escape (closed, or after Cite the chip removed), with the caret after the chip
(`onRestoreFocus`), and ⌘↵ then presses Send as the composer's Return does (`T3Composer.pressSend`). The window draws the
popover (`citation-comment.contract`: w-80, compact padding, `align="end"` 4 pt off the anchor, the side flipped and kept
5 pt inside the window; the textarea, the 8,000 limit's line and "Shorten comment", Cancel and Save) and keeps the draft
(`app-window.contract`): Enter saves, ⌘↵ saves and sends, Escape (the window's Escape button, ChipPopoverEscape) cancels,
an outside press or the textarea's blur settles it as `resolveAssistantCitationCommentDismissal` (untouched or unchanged:
close; changed: commit; past 8,000: keep open). TS (`citation-comment.ts`) rewrites the href with
`withAssistantCitationComment`. While it shows, Send declares no chord (the reference's editor stops ⌘↵), and the cited
text is marked in its answer (MarkedText: the primary at 45% on the cited part of each run, from the quote's start Cite
measured, else its first place). Run 1 of the live drive found two gaps the AppKit test had not: the textarea's
`autofocus` yields to a focused view (HTML's rule), so the prompt kept the focus; and a press on the timeline's ground
reaches no window `pointerdown` and moves no focus, so it did not settle the draft. Since `383724256` the prompt gives
the focus up as the comment opens, a press in the prompt's text gives it the focus, and a press anywhere else outside
the popover takes the focus from the textarea (its blur settles), as Base UI's outside press. Run 2 found X84 (below).

**DC-2.** A block with inline code or a link is laid out word by word (`markdown.rs` `flow_tokens`), and a code block's
code is one `text`; neither reported `selectionchange`. Now every word and code span of a flowed block and a code block's
code report their part (`selected(blockId, runId, s)`), their offsets in the answer's text stream (`citeRunOffset`: the
runs before it; a code block's language line, read only when no file icon stands for it, as MarkdownCodeBlockTitleContent).
The clone's stream, whitespace collapsed, equals the reference's `readAssistantText` stream of the fixture answer, so the
selector (start, end, prefix, suffix) is the reference's (Bun test against the CDP capture). Cite sends the frame of the
text holding the selection's end (a word for a flowed block), where the comment opens.

**DC-2, what stays blocked.** (1) The selection's rectangle (X33, #274, main `issues/20261009-text-selection-rectangles-commands.md`):
the comment after Cite anchors to the frame of the text holding the selection's end (exact to the word in a flowed block;
the whole block for prose or a code block). (2) Clearing the selection (#274, `removeAllRanges`): the host clears it as
the composer takes the focus from the paragraph that held it, which covers prose (checked live, and on the base); a drag
in a code block's horizontal scroller left the focus in the composer, and that selection stayed drawn after Cite (run 2,
D2 and E1). (3) Following the cited text when the timeline scrolls while the comment is open (X22, #127): the frame is
the action's snapshot. All in `EXACT2-GAPS.md` "Cite and the citation comment".

**X84 (new, not filed).** A column's automatic height takes a `field-sizing="content"` textarea's whole content height,
its `max-height` aside: live, an 8,001-character comment made the popover 16,000 pt tall and blank (run 2, F1). A probe
app on macOS measured it (a 320-wide column, the textarea clamped at 130 pt, the column 16,018 pt) and the workaround: the
textarea in a box held at 4rem to 10rem (the column 180 pt). `e7b809e24`.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| DC-1 pencil and label | pass: the chip reads the quote or the comment (64 units, cut at 16em with "…"), "+" no longer shows for a space; its pencil opens the comment above the chip, right-aligned to the button, the chip selected; AppKit test (label, width, button rect, press, accessibility "Add comment to citation" / "Edit citation comment") | [pencil](https://raw.githubusercontent.com/ccheever/exact2/080874db613a7e16972bb1905bc98665197d3cb5/diff-cite-popover/dc1-pencil.png), [AppKit chip](https://raw.githubusercontent.com/ccheever/exact2/7edc52a326a71e71a21d58d5c261320e4dbff272/diff-cite-popover/dc1-appkit-chip.png) |
| DC-1 popover from Cite | pass: Cite inserts the chip and opens "Edit citation comment" under the cited word, the focus in "Comment on selected text"; typed, Enter: the chip reads the comment, the href carries it (`&comment=…`), the focus back in the prompt; Escape after Cite removes the chip; a press outside commits a changed draft; Bun tests of every op (save, send, commit, cancel, stale, too long) | [inline code](https://raw.githubusercontent.com/ccheever/exact2/a6d211c939b151a11d1330cb2ad1389c3dfd9863/diff-cite-popover/dc2-dc1-cite-inline-code.png), [Enter](https://raw.githubusercontent.com/ccheever/exact2/6092dd90226fad5af5a73329b3a968695249a22f/diff-cite-popover/dc1-enter-saves.png), [two lines](https://raw.githubusercontent.com/ccheever/exact2/748cd42ae28fe05d08511a574e8f73a7b36b4d65/diff-cite-popover/dc1-cite-two-lines.png), [Escape](https://raw.githubusercontent.com/ccheever/exact2/b03d7e1a6cd5365924a3dfb65488c5a46d75399b/diff-cite-popover/dc1-escape-removes.png), [outside press](https://raw.githubusercontent.com/ccheever/exact2/fd00e41f0435fb7e2fb1ffd849384f741d862f7a/diff-cite-popover/dc1-outside-press-commits.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8971c928bd111b4fcfc6b1b01d16de522ec8322e/diff-cite-popover/dcp-record.txt) |
| DC-1 popover from the pencil | pass, light and dark: the saved comment prefilled, Escape closes and keeps the chip | [pencil](https://raw.githubusercontent.com/ccheever/exact2/080874db613a7e16972bb1905bc98665197d3cb5/diff-cite-popover/dc1-pencil.png), [dark](https://raw.githubusercontent.com/ccheever/exact2/ca48e998de85042fe1bc10615a45ce3a3fa851d7/diff-cite-popover/dc1-dark.png) |
| DC-1 source highlight | pass: the cited words (and the whole inline code span) marked in the primary at 45% while the comment is open, from Cite and from the pencil; the spaces between flowed words are not marked (each word's trailing space is drawn without its background) | [inline code](https://raw.githubusercontent.com/ccheever/exact2/a6d211c939b151a11d1330cb2ad1389c3dfd9863/diff-cite-popover/dc2-dc1-cite-inline-code.png), [two lines](https://raw.githubusercontent.com/ccheever/exact2/748cd42ae28fe05d08511a574e8f73a7b36b4d65/diff-cite-popover/dc1-cite-two-lines.png) |
| DC-1 past 8,000 characters | Bun and source checks pass ("Comments can contain up to 8,000 characters.", "Shorten comment" disabled, nothing saved); live in run 2 the popover grew off the window (X84), fixed after the drive and measured with a probe app; the live check is real-input step F | [run 2 before the fix](https://raw.githubusercontent.com/ccheever/exact2/8048f20fc0d84682cad51e584d988b01361bc7e2/diff-cite-popover/f1-run2-before-x84.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8971c928bd111b4fcfc6b1b01d16de522ec8322e/diff-cite-popover/dcp-record.txt) |
| DC-1 ⌘↵ saves and sends | Bun (the op saves and asks the native editor to send) and AppKit (`pressSend` clicks the live Send only) pass; not driven live: it would start a provider turn | tests below |
| DC-2 inline code and code blocks | pass: a selection across inline code and one in a code block offer Cite and cite the reference's selector (`start=16&end=44`, prefix and suffix equal, Bun against the CDP stream) | [inline code](https://raw.githubusercontent.com/ccheever/exact2/a6d211c939b151a11d1330cb2ad1389c3dfd9863/diff-cite-popover/dc2-dc1-cite-inline-code.png), [code block](https://raw.githubusercontent.com/ccheever/exact2/2ffb19525e19c670847ee26a48fc928d236ddc82/diff-cite-popover/dc2-code-block.png) |
| DC-2 clearing the selection | pass for prose (cleared, as on the base); blocked for a code block's selection (stays drawn after Cite: #274's clear) | [Enter](https://raw.githubusercontent.com/ccheever/exact2/6092dd90226fad5af5a73329b3a968695249a22f/diff-cite-popover/dc1-enter-saves.png), [outside press](https://raw.githubusercontent.com/ccheever/exact2/fd00e41f0435fb7e2fb1ffd849384f741d862f7a/diff-cite-popover/dc1-outside-press-commits.png) |
| DC-2 blocked rest | recorded: the selection rectangle and clear (X33, #274), following a scroll (X22, #127), in EXACT2-GAPS "Cite and the citation comment" | `EXACT2-GAPS.md` |
| DC-1 and DC-2 under real input | open: real-input steps below | — |

Drives (agent mode, 1280×840, lane diff-cite-popover, the lane's Cite fixture answer seeded by `target/dcp/seed-cite.py`
into "Timeline verification"): before on the evidence worktree at `6db5cadea` (`drive.sh before`); after on this branch's
bundle, run 1 at `dea53111d` (found the focus and the outside press), run 2, the retry, at `383724256`; the reference is
the lane's T3 Code (Alpha) over CDP (`ref-drive.mjs`, the same steps and comment). Light unless named.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) with the Cite
fixture answer seeded (`seed-cite.py` on the lane's `statev2.sqlite`; the reference reads it after its thread cache is
cleared); thread "Timeline verification". Never send a message (no ⌘↵, no Return in the prompt).

1. **A.** Drag with the mouse from "parser" to "pure" in "Cite check: the parser keeps `parsePatch` pure…"; press Cite.
   Expected: the popover "Edit citation comment" under "pure", its right edge at the word's end, the caret in it; the
   words marked blue; the selection gone; the chip with a pencil in the prompt. Type a comment, Return: the chip reads it.
2. **B.** Click the chip's pencil: the popover above the chip, the comment in it; Escape closes it, the chip stays, the
   caret after the chip. Click the pencil again, click on the timeline's empty ground: it closes (nothing changed).
3. **C.** Drag across the closing paragraph's two lines; Cite; Escape: the chip just cited is gone.
4. **D.** Drag across `parsePatch(text: string)` in the code block; Cite; type "draft kept"; click the empty timeline:
   the chip reads "draft kept". Note whether the code selection stays drawn (blocked, #274).
5. **E.** Settings › Appearance › Dark; the first chip's pencil: the popover in dark.
6. **F.** The first chip's pencil; paste more than 8,000 characters (prose and one long word): "Comments can contain up to
   8,000 characters.", "Shorten comment" disabled, the popover at most about 230 pt tall with its text scrolling inside;
   Escape.
7. **G.** VoiceOver (or Accessibility Inspector) over the prompt: the pencil reads "Add comment to citation" (or "Edit
   citation comment") as a button, and pressing it opens the popover.

## Tests

- `diff-cite-popover.test.ts` (15): the chip resource for a citation press (comment, anchor, the cited answer and quote);
  Cite's remembered quote start; Save rewriting the chip with `withAssistantCitationComment` (and an empty comment
  removing it); ⌘↵ and the outside press's commit; Cancel and Escape (after Cite removing the chip); the stale and too-long
  cases; `editorCiteOpen` after the insert and `parseCiteValue`; the window's settle rules against
  `resolveAssistantCitationCommentDismissal`; the popover's attributes and keys; its place (`citationPopoverLeft`,
  `citationPopoverBelow`, `citationPopoverHeight` against the reference's CDP numbers); the highlight's functions; DC-2's
  stream against the reference's (collapsed), a selection across inline code and one in a code block giving the
  reference's selector, and the `selectionchange` and frame wiring.
- `diff-review.test.ts`: Cite opens the comment after the insert (and not when the composer refuses the quote).
- `composer-chip-popover.test.ts`, `dialog-trap.test.ts`, `external-link-menu.test.ts`: updated for the shapes and texts
  this changes (the chip's new fields, the window's Escape, the popover listed as a Popover, the word texts' new arms).
- AppKit `macos/tests/composer` (`citationpress.swift`, 6, and `ComposerSendPressTests`, 1): the label and query parsing;
  the chip's width and button; a real click on the button opening the comment (the label does not), the focus given up,
  presses in the text and on the trigger leaving it open with the focus in the text; an outside press taking the focus
  from the popover's textarea and one in the popover keeping it; Cite's open, Save, the commit keeping the caret, Cancel
  removing; the accessibility buttons; `pressSend`.

## Seen while building, not a finding row

- The clone cites an answer by its turn item (`messageId` "fixture-cite"), the reference by its message ("fixture-cite-message");
  `citedMessageId` (diff-review.ts) predates this task, and View source reads both.
- A press on the composer chip's label is the reference's link to the cited text ("View cited assistant text: …"); the
  clone's composer chip label takes no press (the timeline's chips do).
