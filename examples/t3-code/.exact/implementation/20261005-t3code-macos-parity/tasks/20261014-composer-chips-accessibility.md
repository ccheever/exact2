---
name: 20261014-composer-chips-accessibility
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-composer-chips-accessibility
pr_url:
verified_commit:
---

# The composer's chips to VoiceOver, the terminal chip's glyph, the context popover's order

## Outcome

The user asked on 2026-10-14 to fix what is fixable among `realinput-1011c`'s known differences (STATUS "Known
differences", tables 2 and 3); the coordinator's triage (fix wave, clone task `composer-chips-accessibility`) gave this
task these rows. The user decided on 2026-10-11 that every row is built now, matching the reference.

| Id | Clone (`realinput-1011c`) | Reference (check it first) |
| --- | --- | --- |
| G1-8 | The terminal's Add to chat chip draws a speech bubble (the generic context glyph) and is no accessibility element: the Message area's value is the Markdown link `[Terminal 2 lines 1-738](t3-context://v1/terminal/…)` ([G1-8b-pair.png](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-8b-pair.png)) | A terminal glyph (`>_`); a popup button "Terminal excerpt, Terminal 2 lines 1-738. Show details"; the Message area's value reads the chip's label ("Terminal 2 lines 1-738 ") |
| G5-D4 | Under VoiceOver, VO-Right from the Message area skips the skill chip and, inside the text, reads the raw `$frontend-design`; the chip (an `AXButton` child of the text view) is reachable only through the Item Chooser ([G5-D4-vo-text-clone.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/G5-D4-vo-text-clone.png)) | The chip is its own element in the editor (ContextChipPopover's trigger, "Skill <label>. Show details"), read by its label in the text; VO-Space opens its details |
| DCP-Gv | VoiceOver reads the prompt's citations as raw Markdown link text (`[Assistant quote](t3-citation://…)`) ([DCP-Gv-captions.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/DCP-Gv-captions.png)) | The citation reads as its chip: the link "View cited assistant text: <label>" and its comment button |
| G5-D1 | VoiceOver's order puts the composer's "Message" between the context window popover's description and its bar ([G5-D1-caption-bar-pair.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/pairs/G5-D1-caption-bar-pair.png)) | "Context Window, and 4 more items, dialog": the popover's items stay together as one dialog |
| X30-ATTACH | `EXACT2-GAPS.md` X30 and STATUS §3 still say the any-type file input waits for an owner ruling on #116 | Main admitted it on 2026-10-11 (`3657d330a`, in the T3 tip): correct the cells; optionally move `T3ComposerAttach`'s NSOpenPanel to `input type=file multiple` with `showPicker`, only if a drive shows identical behaviour |

## Steps

1. Reference first: `ComposerPromptEditorTiptap.tsx` (the chips' node views), `contextChipParts.tsx`,
   `chat/TerminalContextInlineChip.tsx`, `composerContextPresentation.tsx`, `contextPresentationRegistry.ts`,
   `chat/AssistantCitationChip.tsx`, `chat/ContextWindowMeter.tsx`, `ui/popover.tsx`; `realinput-1011c`'s AX reads
   (b1-2set/g18, b6-voiceover, b6x-voiceover).
2. Before: the T3 tip (`609f392d5`) built in this worktree before any change, driven on lane
   `composer-chips-accessibility-before` (a prompt holding a terminal reference, a skill, a file mention and a citation;
   `screenshot … window`, `tree --ax composer`).
3. G1-8: the terminal glyph; the chip an accessibility popup button with the reference's name; its press (mouse and
   VO-Space) opens the reference's details (ContextChipPopover: the terminal's name, its lines, the captured output).
4. G5-D4, DCP-Gv: the text view's accessibility text reads each chip as its label (the hidden Markdown source and
   markers left out), the chips are its accessibility children in text order with the reference's roles and names,
   exposed (not ignored) so a navigation-order reader reaches them.
5. G5-D1: the context window popover is a dialog in the accessibility tree, as the reference's PopoverPopup.
6. X30-ATTACH: the record cells.
7. Tests (Bun, AppKit XCTest over the editor's accessibility); one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G1-8 | reference comparison; XCTest (role, label, press, glyph); agent drive (`tree --ax`, `screenshot … window`) | images and AX text records |
| G5-D4 | XCTest over the text view's accessibility (children in navigation order, label, press; the text read as labels); agent drive | AX text records |
| DCP-Gv | the same mechanism and tests, a citation in the prompt | AX text records |
| G5-D1 | a one-file probe of the popover's markup (`tree --ax` order), before and after; the app's contract | AX text records |
| X30-ATTACH | the cells read against main's `3657d330a` | text |
| G1-8, G5-D4, DCP-Gv, G5-D1 under VoiceOver | real-input steps for the final session | — |

## Reference

T3 Code `1e2ecbd975` (MIT), `target/t3-ref/src-1e2ecbd975/apps/web/src/components/`:

- `ComposerPromptEditorTiptap.tsx`: ComposerMentionNodeView (a button "Preview <path>"), ComposerSkillNodeView
  (ContextChipPopover, `accessibleLabel` "Skill <label>"), ComposerCitationNodeView (AssistantCitationChip `composer`),
  ComposerContextReferenceNodeView (the presentation registry).
- `contextChipParts.tsx`: ContextChipPopover (the chip is the PopoverTrigger, `aria-label` "<accessibleLabel>. Show
  details"; Chromium serves it as `AXPopUpButton`), ContextChipShell, UnresolvedChip ("Unavailable context, <label>",
  CircleDashedIcon).
- `chat/TerminalContextInlineChip.tsx`: TerminalIcon; "Terminal excerpt, <label>" (popover with the terminal's name,
  "Line N" / "Lines a–b" and the captured output); expired: "Terminal excerpt, <label>, expired".
- `composerContextPresentation.tsx`, `contextPresentationRegistry.ts`: each kind's name and details mode.
- `chat/AssistantCitationChip.tsx`: the composer's source link "View cited assistant text: <label>" and the comment
  button ("Add comment to citation" / "Edit citation comment").
- `chat/ContextWindowMeter.tsx`, `ui/popover.tsx`: the meter's PopoverPopup (a dialog).
- `realinput-1011c` reads: `b1-2set/g18/{ref,clone}.txt`, `b6-voiceover/G5-D{1,4}-ax-{clone,ref}.txt`,
  `b6x-voiceover/DCP-Gv-ax.txt`, and the session notes `00-b1-notes.txt`, `00-b6-notes.txt`, `00-b6x-notes.txt`.
