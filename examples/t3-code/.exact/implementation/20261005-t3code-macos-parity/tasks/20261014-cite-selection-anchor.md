---
name: 20261014-cite-selection-anchor
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-cite-selection-anchor
pr_url: null
verified_commit: null
---

# Cite: Tab to Cite, Cite and its comment at the selection's last line, the pencil's popover

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences and the kept
workarounds (round 12's [r12-kept-workarounds](20261013-r12-kept-workarounds.md) KW-9); the user decided on 2026-10-11
that these are built now, matching the reference T3 Code. Main's `keycapture=` (X25, `3f283d582`) and `Selection.rects` /
`removeAllRanges()` (X33, `48755525c`) were adopted in round 12 and are not used by Cite yet. Source records:
[diff-cite-popover](closed/20261011-diff-cite-popover.md) (the anchor and a code block's selection, declared on X33 /
#274), `STATUS.md` "Known differences" (G4-03, DCP-E), `realinput-1011c` B2 and B13 notes.

| Id | Clone (feature tip `609f392d5`) | Reference (check it first) |
| --- | --- | --- |
| G4-03a | After a double-click on a word, a plain Tab clears the selection and Cite and moves the focus to the timeline's scroll area (`realinput-1011c` B2 step 2, [G4-03-cite-tab-pair.png](https://raw.githubusercontent.com/ccheever/exact2/7f34eae2b1671e15926020b44cadef91b08146d6/realinput-1011c/b2-panels-browser/pairs/G4-03-cite-tab-pair.png)); step 3 (Escape) was not reachable | AssistantSelectionToolbar.tsx:87-107: a capture-phase document `keydown` takes a plain Tab (no Shift, Alt, Control or Meta, not composing, not prevented) while Cite shows, is enabled and does not hold the focus: `preventDefault`, `stopPropagation`, `focus()` on Cite (ring). Escape on Cite then hides it, keeps the selection, and the focus goes to the body |
| G4-03b (KW-9) | Cite sits under the block holding the selection's end, at its right edge (`right: 0.5rem`), and the comment opened by Cite anchors to the frame of the text holding the end (a whole prose block or code block: under its last line, right-aligned to the block's right edge). A code block's selection stays drawn after Cite (#274, now on main) | Cite's top-left at the release point of the pointer that made the selection, else at the selection's last client rect (`right`, `bottom + 4`), kept inside the window (`resolveSelectionActionPosition`); the comment after Cite `side="bottom" align="end"` on the selection's last line rect (AssistantCitationChip `popupAnchor`); after citing, `window.getSelection().removeAllRanges()` |
| DCP-E | The citation pencil's popover sits left of its chip, reaching past the composer's left edge (`realinput-1011c` B13, [DCP-E-dark-record-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/DCP-E-dark-record-pair.png)) | Compare: the pencil's PopoverPopup is `side="top" align="end"` on the pencil (ContextChipAction), 4 pt off, kept 5 pt inside the window |

## Steps

1. Reference first: the lane reference (CDP, 1280×840, the lane's Cite fixture answer): Cite's place after a drag and a
   double-click, Tab and Escape, the comment's place after Cite from prose and from a code block, the selection after
   Cite, the pencil popover's place against its button.
2. G4-03a (`markdown.contract` AnswerMarkdown, `diff-cite.contract`): a `keycapture=` handler on the answer's root takes
   the plain Tab while Cite shows, is enabled and does not hold the focus (Cite's `focus`/`blur`), `preventDefault`,
   `stopPropagation`, `focus` on Cite; Cite's Escape keeps its dismissal.
3. G4-03b: each text's part keeps its last selected line (`Selection.rects`' last) from the answer's top-left; Cite sits
   at the selection end's line (its right, bottom + 4), kept 8 pt inside the window; Cite sends that line's window frame
   (measured again at the press) as the comment's anchor; `removeAllRanges()` after citing.
4. DCP-E: measure both apps' pencil popover against the pencil; change the clone only where it differs.
5. Bun tests, `contract build`, one agent drive (before on this branch's base build, after on the bundle), before /
   after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G4-03a | reference over CDP; Bun (the capture action's arms over Contract's action semantics); agent drive: double-click, Tab (`state` focus), Escape | before / after / reference images and the focus read |
| G4-03b | reference over CDP; Bun (the place functions against `resolveSelectionActionPosition` and the reference's numbers, the part's line, the clear); agent drive: a two-line prose drag and a code block drag, Cite | before / after / reference images |
| DCP-E | reference over CDP; agent drive: the pencil's popover against the pencil's frame | before / after / reference image and numbers |
| G4-03 and DCP-E under real input | real-input batch steps below | — |

## Reference

T3 Code `1e2ecbd975` (`target/t3-ref/src-1e2ecbd975/apps/web/src`, MIT):

- `components/chat/AssistantSelectionToolbar.tsx` (the Tab capture, Escape, `removeAllRanges()` after `onCite`, the
  position from `rects.item(rects.length - 1)`), `lib/selectionActions.ts` (`resolveSelectionActionPosition`,
  `observeSelectionActions`: the pointer point on mouse-up, the multi-click delay).
- `components/chat/AssistantCitationChip.tsx` (`popupAnchor`: the range's last client rect; `side={activeSourceAnchor ?
  "bottom" : "top"} align="end"`), `components/ui/popover.tsx` (`sideOffset = 4`).
