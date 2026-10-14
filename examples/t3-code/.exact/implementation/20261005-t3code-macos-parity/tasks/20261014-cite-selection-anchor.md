---
name: 20261014-cite-selection-anchor
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: stale
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-cite-selection-anchor
pr_url: https://github.com/ccheever/exact2/pull/540
verified_commit: 18b934b1d
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

## Cause and fix

**Reference first (CDP, the lane reference, 1280×840; `ref-cite2.mjs`, scenarios A to E with the clone's points).** A drag
from "parser" to the end of "paragraph" (two lines) released at 420,208: Cite's top-left 420,208 (the release point);
after Cite the comment 100..420 from 220.3 (the last rect ended at 420, its bottom 216.3). The closing paragraph (B):
Cite 501,385 at the release, the comment 181.2..501.2 from 397.8. The code (C): Cite 607,282, the comment 289.6..609.6
from 293.7. A double-click on "closing" (D): Cite at the click point 316,362; Tab: Cite focused (ring), the selection
kept; Escape: Cite gone, the selection kept, the focus on the body. The pencil (E): the popup's right edge on the
pencil's (394.5) and its bottom 4 pt above it.

**G4-03b / KW-9.** Cite sat inside the answer (`position="absolute" right=0.5rem` under the block holding the selection's
end) and the comment it opened anchored to that text's frame: a whole prose block or code block. Under the answer's last
block Cite also lay over the row's footer (the copy and time row, a later sibling of the answer), which took its press:
in the before drive (B) the press only cleared the selection. Now (`diff-cite.contract`, `markdown.contract`
AnswerMarkdown, `app-window.contract`):

- each text's part keeps its last selected line (`Selection.rects`' last, main `48755525c`) from the text's top-left
  (`citePart`); the part ending last holds the selection's end (`citeEnd`);
- the answer's box hears `pointerdown` and `pointerup` (and passes both on to the window's `outsidePressDown` /
  `outsidePressUp`, which held the pointer before): a press in the answer hides Cite, a selection change while the button
  is down waits, and the release offers Cite at the release point (`observeSelectionActions`); a selection made without
  the pointer is offered at once at its last line's right and bottom + 4; both held in the timeline's box
  (`resolveSelectionActionPosition`, `citeAxis`) and 8 pt inside the window with Cite's size (`citeActionLeft`/`Top`);
- the window draws Cite (`CiteAction`, z-index 50) above the timeline, as the reference portals it to the body: on the
  thread it was offered on, under no page, Settings or dialog; a press anywhere but on Cite, the timeline's `scroll`, a
  window resize and the window losing the focus hide it; after a double-click it waits 500 ms
  (`SELECTION_MULTI_CLICK_INTERVAL_MS`: the answer's `dblclick`, the root's `task citeWait`, since a task is the root's,
  #108), and a third click within the wait begins it again;
- the press measures the end text again and sends the last line's window frame (`citeLineFrame`), where
  citation-comment.contract opens the comment (`align="end"` under it, unchanged), then `removeAllRanges()`.

**G4-03a.** Keyboard navigation is off on the Mac, so a plain Tab from the paragraph moved the focus on (the composer in
the before drive, the timeline's scroll area under real input) and the focus change cleared the selection and Cite. Now
the answer's box has `keycapture=keys` (main `3f283d582`): a plain Tab (no Shift, Option, Control or Command) while this
answer's Cite shows and is enabled calls `preventDefault()`, `stopPropagation()`, `focus("cite-action")`; Cite is the
window's, outside the box, so a Tab while Cite holds the focus is not taken (`toolbar.contains(event.target)`). A focus
entering a button keeps the selection (the host's rule). Cite draws the button's ring while focused
(`focus-visible:ring-2 ring-offset-1`); Escape on Cite hides it and keeps the selection; Escape in the answer hides it
without `preventDefault()`.

**DCP-E.** Measured in both apps: the pencil's popup is `side="top" align="end"`, 4 pt off, in the reference (its right
edge 394.5 on the pencil's 394.5, bottom 691.4 under the pencil at 695.4) and in the clone (71..391 on the pencil's 391,
bottom 650.5 under 654.5). It sits left of its chip in both, past the composer's left edge in both: no change.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| G4-03a | pass (agent): double-click "closing", Tab: Cite holds the focus (`state.focus.logical` = the Cite button), its ring drawn, the selection kept; Escape: Cite hidden, the selection kept, nothing focused (the reference: the body). Before: Tab moved the focus to the composer and cleared both. Bun: the capture's arms (plain Tab, each modifier, no Cite of this answer, too long), Escape in the answer and on Cite | [dblclick](https://raw.githubusercontent.com/ccheever/exact2/c17006e3f8f849d96ee084710c0d65a0c0c21d66/cite-selection-anchor/g403a-d1-dblclick.png), [Tab](https://raw.githubusercontent.com/ccheever/exact2/6511b22bc9eadaf9a49e8db405f88cdeb2a30149/cite-selection-anchor/g403a-d2-tab.png), [Escape](https://raw.githubusercontent.com/ccheever/exact2/8dc0d863e718cd808b79a6cf9b018122e4b41d24/cite-selection-anchor/g403a-d3-escape.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f4e5d0a130926631ef19f3f8dffe0ed4a93e2e08/cite-selection-anchor/cite-record.txt) |
| G4-03b | pass (agent): Cite at the release point (A 420.4,208; B 500,385.5; C 607,282.5; D 316,362.8; the reference 420,208 / 501,385 / 607,282 / 316,362); after Cite the comment under the last line, right-aligned (A 100..420 from 219.6, B 181.2..501.2 from 397.4, C 288.9..608.9 from 293.3; the reference 100..420 from 220.3, 181.2..501.2 from 397.8, 289.6..609.6 from 293.7); Cite under the last block now takes its press (before: the row footer took it). Bun: the place functions against the reference's `resolveSelectionActionPosition` and its numbers, the part's line, the offer at the release, the double-click wait, the window's hides, the press's frame and `removeAllRanges()` | [A Cite](https://raw.githubusercontent.com/ccheever/exact2/ca386c26d0fac8284d841b536916b4c9d58d5a7f/cite-selection-anchor/g403b-a1-cite-place.png), [A comment](https://raw.githubusercontent.com/ccheever/exact2/414236f9fb0b1f30408070d36d6c0a45a3551469/cite-selection-anchor/g403b-a2-comment-anchor.png), [B Cite](https://raw.githubusercontent.com/ccheever/exact2/a07c7939f8470c4aafa51b332860fb24e8a2154d/cite-selection-anchor/g403b-b1-last-block.png), [B press](https://raw.githubusercontent.com/ccheever/exact2/8f124bb3effdc4a2ba76592ac8cc2eaa6f432ec1/cite-selection-anchor/g403b-b2-press.png), [C code](https://raw.githubusercontent.com/ccheever/exact2/8102c92e24cfaf7b736fadc21d3972c472df1cb0/cite-selection-anchor/g403b-c2-code.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f4e5d0a130926631ef19f3f8dffe0ed4a93e2e08/cite-selection-anchor/cite-record.txt) |
| G4-03b, the selection after Cite | pass (Bun: the press calls `removeAllRanges()` after the command); live, the citation mark covers the cited text while the comment is open, so the screenshot cannot tell the selection under it (the same colours before and after) | tests below |
| DCP-E | pass, no change: the same rule and numbers as the reference (above) | [pencil](https://raw.githubusercontent.com/ccheever/exact2/407334f3db9d7ce31f6bf306e4af9ef32c98a8bc/cite-selection-anchor/dcpe-e1-pencil.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f4e5d0a130926631ef19f3f8dffe0ed4a93e2e08/cite-selection-anchor/cite-record.txt) |
| G4-03 and DCP-E under real input | open: real-input batch steps below | — |

Drives (agent mode, 1280×840): before on this worktree's bundle of the feature tip `609f392d5` (lane
`cite-selection-anchor-before`); after on this branch's bundle at `10fdff230` (lane `cite-selection-anchor-after`, a
fresh home seeded with the lane's Cite fixture answer by `seed-cite.py`); the reference over CDP on lane
`cite-selection-anchor`. An earlier after drive at `f43c88ba7` (before the ring, the outside press, the wait and the
resize/focus hides) gave the same places; the final drive is the one retry the brief allows.

## Real-input batch steps

Launch this branch's bundle as a lane copy with the Cite fixture answer seeded (`seed-cite.py` on the lane's
`statev2.sqlite`); thread "Timeline verification". Never send a message (no ⌘↵, no Return in the prompt).

1. **G4-03 step 1-3.** Double-click "closing" in the closing paragraph. Expected: the word selected; about half a second
   later Cite appears with its top-left at the click point. Press Tab: Cite takes the focus (a blue ring), the word stays
   selected (VoiceOver or Accessibility Inspector: the button "Cite selection in composer" focused). Press Escape: Cite
   disappears, the word stays selected.
2. **G4-03 step 4-5.** Drag from "parser" to the end of "paragraph" (two lines). Expected: no Cite while the button is
   down; at the release Cite's top-left at the pointer. Press Cite: "Edit citation comment" opens under the second line,
   its right edge at the end of "paragraph"; the selection is gone; type `why`, Return: the chip reads "why".
3. **G4-03b, the last block.** Drag across the closing paragraph's two lines; press Cite (it lies over the row's footer):
   the comment opens under "last line.", right-aligned to it. Escape: the chip just cited is gone.
4. **G4-03b, a code block.** Drag across `parsePatch(text: string)`; press Cite: the comment opens under the code line,
   right-aligned to "string)"; Escape; the code is not left selected.
5. **G4-03b, hides.** Select a word by a double-click, then: scroll the timeline (Cite hides, the word stays selected);
   select again and resize the window (Cite hides); select again and switch to another app with ⌘Tab (Cite hides and
   stays hidden on return); select again and click the sidebar's empty ground (Cite hides).
6. **DCP-E.** Click the first citation chip's pencil: the popover above the chip, its right edge at the pencil's right
   edge (it reaches left past the composer's edge, as the reference's does).

## Tests

- `cite-selection-anchor.test.ts` (18): the place functions run from their `.contract` lines against the reference's
  `resolveSelectionActionPosition` and the toolbar's 8 pt hold (release points A to D, the last line's corner, the
  timeline's box, the window's edges, "Shorten selection"'s width); a part's last line and the end part; the press's
  frame measured again and citation-comment.contract's popup from it against the reference's B numbers; AnswerMarkdown's
  actions with Contract's action semantics (a drag offers at the release, a press that changes nothing offers nothing, a
  double-click holds, a selection made without the pointer is offered at once and withdrawn when cleared; the Tab
  capture's arms; Escape); the window's offer, withdrawal, scroll, outside press, wait and focus-loss gates, the press
  (the command's value, then `removeAllRanges()`), Cite drawn by the window; the root's wait and focus-loss count; DCP-E's
  rule against both apps' numbers.
- `diff-cite-popover.test.ts`: its DC-2 wiring test no longer reads the old in-answer button (moved to the test above).
- `usage-pooled.test.ts`: the pointer inventory names the answer's box, which hands each press on to the window's
  `outsidePressDown` / `outsidePressUp` (as the theme editor's colour controls do).

## Checks

On `18b934b1d` (the feature branch had no new commits; nothing to merge). The cargo checks ran on `f7a654d4a`, whose
tree differs only in `app.contract`'s argument order in one call and two test files (no Rust).

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4874 pass, 1 skip, 0 fail (329 files); the first run had 3 source guards fail (two read T3Window's call from its first argument, the pointer inventory lacked the answer's box): fixed |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 33,811,060 bytes (base 33,309,129); `app.contract` 1,468 lines |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust or Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed, 0 failed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | on `f43c88ba7` (the first after drive) and on `10fdff230` (the final drive); `18b934b1d` changes no app behaviour (the same plan bytes) |

## Not done / not verified

- The real-input batch steps above (G4-03 with a real double-click, Tab and drag; DCP-E's pencil): the coordinator's
  session.

## Delivery

Draft PR [#540](https://github.com/ccheever/exact2/pull/540) into `feat(example)/t3-code`.

## Review repair, 2026-10-14

The review of `965b638178ade5aae1fe5be3c682655e7d5425af` found two stale-offer paths. During the
500 ms double-click delay, `citeOffered` is empty because the toolbar is hidden. Escape and a selection clear
therefore failed to withdraw the pending offer. Resize, thread, page, Settings and dialog gates only hid an offer;
restoring the original context could show it again.

`AnswerMarkdown` now withdraws its offer on Escape or an empty selection without checking whether Cite is shown.
Other keys during the delay cancel the pending offer too, matching `observeSelectionActions`. The window still
rejects a withdrawal from another answer. Tab retains its visible, enabled toolbar gate.

The root's dismissal counter now also advances when the viewport, thread or covering UI changes. Each offer captures
that counter; returning to an earlier context cannot make the old counter current again. This uses the existing
root keyed-task and 1 ms timer pattern, with no polling or framework change. Geometry and the pencil popover are
unchanged.

Guidance: shared Exact library `20261005-platforms-v3`, foundations, state/data, platforms, accessibility and
testing/debugging. Applicability was checked against this checkout's existing keyed tasks and action snapshot
semantics, the parent [specification](../spec.md), and the pinned reference's `selectionActions.ts` and
`AssistantSelectionToolbar.tsx`. This repair does not change the target matrix or prior open real-input rows.

Checks on the repaired source, Bun 1.4.2:

| Check | Result |
| --- | --- |
| Regression tests against the old Contract files at `965b638178ade5aae1fe5be3c682655e7d5425af` | 19 passed, 5 failed as expected: Escape, clear, another key during the delay, and returning to context with/without a pending wait |
| `bun test examples/t3-code/cite-selection-anchor.test.ts examples/t3-code/diff-cite-popover.test.ts examples/t3-code/usage-pooled.test.ts` | 66 passed; 24 belong to Cite, including viewport/thread/page/Settings/dialog return and pending outside press |
| `bun test examples/t3-code --timeout 60000` | 4,880 passed, 1 skipped, 0 failed |
| Strict `tsc`, README command | exit 0 |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | exit 0; existing colour diagnostics remain |
| `EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle` | exit 0; the first capture refused files changing during capture, so compilation and edits were finished before rebuilding |
| Native agent drive, isolated fixture and storage | pass: Escape during the pending delay, resize away/back, a fresh offer afterward, Tab focus, Escape from Cite |
| `cargo build --all-targets --keep-going`, `cargo fmt --all -- --check`, caps, boot | exit 0 |
| Remaining broad Cargo checks | coordinator owns integrated checks; the lane's corpus test was stopped at the coordinator's request, and clippy/workspace were not started |

The agent drive used a copy of the existing `fixture-cite` database under `target/cite/repair-lane`, its own project
copy, local server port 16642, telemetry disabled, and named agent storage `cite-review-repair`. It sent no provider
messages and used no global input. State showed `citeWaitAsked=1`, `citeWaitDone=0` before Escape; after 600 ms,
`citeFor` was empty and Cite absent. Resize away/back advanced `citeDismissals` from 2 to 3 to 4; Cite stayed absent.
A new selection offered Cite again, Tab focused its live node, and Escape removed it. The first driver invocation
refused an authored `id` as a named target; the completed drive resolved that node from the live tree and used its
numeric id. No runtime source change was needed.

Local-only logs and the drive's state/tree captures are under `target/cite/repair-*`; they have not been published.
The coordinator reviewed the staged runtime diff and behavior tests without blocking findings. Binding that review
to the final commit and the real-input batch remain coordinator work. The earlier `verified_commit` and image links
above identify the prior implementation, not this repair.

## Next action

Implementation is ready for verification. The coordinator binds independent review to the repair commit, publishes
the amended draft PR within the authorized workflow, and handles integrated checks and the remaining real-input batch.


## Coordinator publication, 2026-10-14

Draft PR [#540](https://github.com/ccheever/exact2/pull/540) was read back at `706f3d3f44a718d19f90996e82258718ec6e3e49`, based on `feat(example)/t3-code`. Pending/stale cancellation verified by tests and native drive; final real input remains open. The combined repository gates and app drive are pending. This local integration bookkeeping does not close the task or replace the earlier verification revision.

### Remote delivery, 2026-10-14

GitHub reports this PR merged at 03:04 UTC into `feat(example)/t3-code`, published as
first-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`. This records delivery
only; the task's remaining acceptance and real-input qualifications above still apply.
