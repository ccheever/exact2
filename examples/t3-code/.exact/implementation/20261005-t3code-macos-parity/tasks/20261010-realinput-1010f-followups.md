---
name: 20261010-realinput-1010f-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010f-followups
pr_url: https://github.com/ccheever/exact2/pull/413
verified_commit: null
---

# Findings of the real-input session realinput-1010f

## Outcome

The session `realinput-1010f` ran the real-input steps of #400 (realinput-1010d-followups) and #406
(realinput-1010e-followups) on the bundle of `d057787cb`. Passed: RD-1, RD-2's positions, RD-3, RE-1 to RE-4, RE-5's Code
tab and Viewed tick, and RE-6 captured both bad states in traces. This task takes what did not pass. Each row is compared
with the reference first; a row that matches it closes as such. Session notes:
[F0](https://raw.githubusercontent.com/ccheever/exact2/2f3a80555ea176b202a0e652e92c2dcbf790b352/realinput-1010f/F0-1010f-notes.txt).

## Findings

| Id | From | Clone under real input | Evidence |
| --- | --- | --- | --- |
| RF-1 | #400 RD-4 | The shell's text menu over a selection has Cut (dimmed), Copy, Paste (dimmed), Select All, and also "Services ›", which the reference's menu (`DesktopWindow.ts` `installContextMenu`) does not have. | [selection menu](https://raw.githubusercontent.com/ccheever/exact2/cf9b2f18bc6058fd2cd62da1ec049be981492bd1/realinput-1010f/F1-RD4-1-selection-menu.png) |
| RF-2 | #400 RD-2 | A drag from the floating player's pill padding by (-300, -200) moved the player only (-20, -200). Positions after re-float pass. | [re-float](https://raw.githubusercontent.com/ccheever/exact2/3f828a1ab6c1543b4259bb97a20a3bb7a8d06d02/realinput-1010f/F1-RD2-refloat.png), [notes](https://raw.githubusercontent.com/ccheever/exact2/2f3a80555ea176b202a0e652e92c2dcbf790b352/realinput-1010f/F0-1010f-notes.txt) |
| RF-3 | #406 RE-5 / #308 step 2 | In PR #168's Code tab, a drag in the gutter from new line 3 to line 5 paints no range and opens no draft (3 tries); the gutter's hover stays on line 3 during the drag. A single click on the + opens a line-3 draft. | [drag](https://raw.githubusercontent.com/ccheever/exact2/1c6a4cedbb9c642250c9fb1d777f593e933f28d4/realinput-1010f/F2-RE5-3-308-gutter-drag-FAIL-no-range.png), [click](https://raw.githubusercontent.com/ccheever/exact2/88b58f63586de47fca28b7528164885402a77371/realinput-1010f/F2-RE5-4-308-plus-click-opens-line3-draft.png) |
| RF-4 | #406 RE-5 / #308 | The pending review comment card's Discard (trash) button ignored two real clicks. ⌘↵ had added the card and badge 1. | [card](https://raw.githubusercontent.com/ccheever/exact2/0ddce2a4005ccc454a2cd26c769c9ef97c597fd7/realinput-1010f/F2-RE5-5-308-pending-card-badge.png), [discard](https://raw.githubusercontent.com/ccheever/exact2/52b3b20452e73bd91d7767bbe4399fbdeae2dacd/realinput-1010f/F2-RE5-6-308-discard-no-effect.png) |
| RF-5 | #406 RE-6 (RC-5) | Both bad states of the Usage page's unpriced popover are traced. Flicker: a 1 pt move on the (i) (view 14006) fires "hover out" on it and "hover in" on view 13954, the Cost metric segment that took an earlier press; 67 ms later hover goes back to 14006. Stuck: every pointer step alternates hover between 14006 and 13954, and at rest the last event is "hover in 13954", so the popover never opens. The segment sits at 758–810 × 52–76, so its hover region claims a point outside its frame. | [trace 1](https://raw.githubusercontent.com/ccheever/exact2/a1ae92ed10b3c6366063ebdce8184e88676f98ec/realinput-1010f/F2-RE6-trace-1-flicker.json), [hover lines 1](https://raw.githubusercontent.com/ccheever/exact2/b2ae5b3a0cb24697bb931036f4d109272f519479/realinput-1010f/F2-RE6-trace-1-hover-lines.txt), [trace 2](https://raw.githubusercontent.com/ccheever/exact2/7d3386d1b45cc21ae1660b7941ab8854d4d82d99/realinput-1010f/F2-RE6-trace-2-stuck.json), [hover lines 2](https://raw.githubusercontent.com/ccheever/exact2/5ed78452fd19664f4e6504006a2375b634dd46b2/realinput-1010f/F2-RE6-trace-2-hover-lines.txt), [read-out](https://raw.githubusercontent.com/ccheever/exact2/f367364de86277b0d463489f733be59da216b790/realinput-1010f/F2-RE6-trace-1-read.txt) |

## Scope and exclusions

Included: RF-1 to RF-5. Excluded: RD-4's "links show no menu at all", which is
[shell-context-menu](20261010-shell-context-menu.md) (#407, in review: Copy Link and the app's link menu). Where a row's
cause is in the framework (RF-5 may be a host tracking area that keeps a stale rect after a press; RF-3 may be the host's
drag delivery to a gutter), record it in `EXACT2-GAPS.md` with a one-file repro and leave the framework alone; the main
issue is filed separately.

## Steps

- RF-1: AppKit adds Services to a context menu it pops for a view that answers `validRequestor(forSendType:returnType:)`.
  Take it out of the shell's menu (for example `NSMenu.allowsContextMenuPlugIns = false`, or not passing the event to
  `popUpContextMenu`'s Services path). Check #407's template (`T3ShellMenu.swift`) if it has merged by then; the change
  goes where the menu is built.
- RF-2: compare with the reference (CDP: the same drag on the floating player's padding) and read the clone's clamping.
  Fix if different.
- RF-3: compare with the reference's gutter range drag (`apps/web/src/components/…` diff gutter handlers; CDP drag on
  #168 in the GitHub lane). Find why the clone's drag stays on the start line (pointer capture, the move events the row
  gets, the host's drag delivery). Fix in the clone where it can be; record a host cause in `EXACT2-GAPS.md`.
- RF-4: find why the Discard button takes no real click (hit area, an overlay over it, a press handler, `disabled`).
  Compare with the reference's pending card. Fix.
- RF-5: read the traces (`bun scripts/agent.mjs trace <file>`) and find why view 13954's hover claims the (i)'s point:
  the clone's hover regions on the Usage page's metric segments, or the host's tracking areas after a press. Fix the clone
  part; record a host part in `EXACT2-GAPS.md` with a one-file repro (X62 and main #322 / PR #327 are the existing hover
  issues; check that #327 does not already cover it).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RF-1..RF-5 | reference comparison first; a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Cause and fix

- RF-1: AppKit's `NSMenu.popUpContextMenu(_:with:for:)` appends a Services item for a view that answers
  `validRequestor(forSendType:returnType:)`, which ExactKit's text node does for its selection; the shell's menu was popped
  that way (`T3TextContextMenu.present`). Electron pops its menus with `popUpMenuPositioningItem:atLocation:inView:`, which
  adds none, and `DesktopWindow.ts` `installContextMenu`'s template has none. `present` now pops the menu with
  `allowsContextMenuPlugIns = false`. Every shell menu goes through `present` (#407's page, input and link menus too, merged
  here). An AppKit check counts what AppKit asks the text node while the menu is up: 73 Services questions with the tip's
  `present`, none now. AutoFill in the composer's menu (#407's) is RG-3 of realinput-1010g-followups, not this row.
- RF-2: matches the reference; no change. The same drag in the reference over CDP (a 1280 x 800 tab floated over chat, 320 x
  200, the pill's padding dragged by (-300, -200)) moves the player (-20, -200), as the clone did under real input:
  `chatCanvasLayout.ts` stops a drag at the readable chat lane (`minimumPreviewX` = padding 20 + minChatWidth 640 + gap 12,
  container x 672), which `chat-canvas-layout.ts` ports.
- RF-3: the clone had no gutter drag. Its number only selected on a press (Shift extended it) and its "+" commented on a
  click. In the reference (@pierre/diffs' InteractionManager) a press on a number starts a selection that follows the
  pointer over the file's lines, a press on the "+" starts a range from the selection's top whose end follows it, and in the
  Code tab both end in `beginComment` (`onLineSelectionEnd`, `onGutterUtilityClick`): over CDP a click on line 3's number
  opens the draft on line 3, and a drag from 3 to added 5, from the number or the "+", paints 3–5 and opens the draft. Now
  `DiffCell` (diff-rows.contract) hears `pointerdown`/`pointermove`/`pointerup` on the number and the "+"; the pressed node
  holds the pointer, and its moves name the line under it with `elementFromPoint` over the cells' ids
  (`dl:<side>:<line>:<path>`). `diff-line-drag.ts` ports the gestures (press, Shift-press, a press on the one selected line
  that clears it on release, the "+" range); the Code tab opens its draft on the release (`beginRange`), the thread's Diff
  panel only on the "+"'s range (AnnotatableCodeView has no `onLineSelectionEnd`). A click's press and release come within
  a few ms, so each step goes out on its own queued mutation (`lineDragChanged`): on `localChanged` the release replaced
  the press before it ran, and the Diff panel's `changed` takes one command at a time. Declared (EXACT2-GAPS, "Hover and the
  window's mouse moves; the diff gutter's drag"): a press on the gutter no longer reaches the window root's outside press,
  which only matters for a skill chip's details beside the Diff panel.
- RF-3, after the PR's review: where the "+" sits is Pierre's `placeUtility`, not hover. While a file has a selection
  (`getCurrentSelectionRange`, which a drag updates on each move) `placeUtilityFromSelection` draws the "+" on the
  selection's bottom line only, and nowhere when that line is not drawn; without one it follows the hovered line. The clone
  drew a "+" on whichever line was hovered, so (1) during a drag it stayed on the pressed line (the session's "the gutter's
  hover stays on line 3", first declared as a host gap and filed by the coordinator as X80, main #414), and (2) with lines
  selected in the Diff panel a click on another line's "+" sent two commands on two mutations that could disagree: the
  pointer's `gutter`/`end` (lineDragChanged) opened the draft on the selection (pressGutter's top to bottom), the press's
  `comment:` (`changed`; `pr-code-begin` on `localChanged` in the Code tab) on the clicked line alone, whichever ran
  first. Now the projections mark the file `pinned` and the one cell that draws its "+" (`diff-line-drag.ts`
  `utilityPlacement`, `pin`/`leftPin`/`rightPin` in `diff.ts` and `pages-pr-code-rows.ts`), and `DiffCell` draws a "+" only
  there while the file has a selection, keeping a cell's "+" mounted while its own drag is in flight (the pressed node
  holds the pointer). The press (`comment:`, `pr-code-begin`) queues on `lineDragChanged` behind its drag and is the
  gesture's release while a "+" drag is in flight; a press no gesture carried (a keyboard or accessibility press) comments
  on what the gesture would (`gutterClick`: the selection's top to bottom, else its line). X80 no longer shows in the clone
  (its EXACT2-GAPS row says so).
- RF-4: the pending card's dashed border is an absolutely positioned box over the card (`border-style` is solid-only), so it
  paints over the card's row and took the presses meant for the trash (CSS paint order; the agent's real-event tap hit it
  too: the before drive's tap changed nothing). It is `pointer-events="none"` now. The reference's card (a dashed border)
  discards on one click.
- RF-5: a clone cause. The activity reporter (`T3ActivityReporter.swift`) set `acceptsMouseMovedEvents` on each main
  window to hear the pointer; AppKit then sends every mouse move to the window's first responder, and a pressed button is
  the first responder (ExactKit focuses it, as Chrome does). `NodeView.mouseMoved` takes any move it receives as the
  pointer over it, so the pressed Cost segment (view 13954) took the hover on every move wherever the pointer was, trading
  it with the (i)'s own tracking area (14006) and keeping it at rest: the traces' alternation. It is not X62/#322's overlap
  (the segment's area never held the point), and #327 does not cover it. The reporter now hears the pointer through a
  tracking area of its own over the window's content (`T3ActivityPointer`, the area's owner alone gets its moves) and
  leaves `acceptsMouseMovedEvents` off. The host part (a node could take only its own tracking area's moves) is recorded
  in EXACT2-GAPS with a one-file repro, not filed.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RF-1 | implemented, AppKit test: before 73 Services asks and plug-ins on (2 failures), after none. Agent mode logs the menu instead of popping it, so the real menu is real-input step 1 (open) | [RF1-services-test.txt](https://raw.githubusercontent.com/ccheever/exact2/6a2953c54e29c1917198b611b90e0fa140d9236b/realinput-1010f-followups/RF1-services-test.txt) |
| RF-2 | pass by reference comparison, no change: the reference's same drag moves (-20, -200), as the clone under real input | [RF2 image](https://raw.githubusercontent.com/ccheever/exact2/73198a72c56263afb9769cb1b195419b12d73494/realinput-1010f-followups/RF2-pill-drag.png), [RF2-reference.txt](https://raw.githubusercontent.com/ccheever/exact2/9e49e5ba345cbac8e0648eb1a44bbb5d3cc87b65/realinput-1010f-followups/RF2-reference.txt) |
| RF-3 | pass (agent drive, reference over CDP, tests): before a drag from new line 3 to 5 painted nothing and opened no draft; after lines 3–5 paint and the draft opens under added line 5, as the reference. Real pointer: step 2 (open) | [RF3 image](https://raw.githubusercontent.com/ccheever/exact2/3976a74cf988c4bd2fc7cf6005713243413ff180/realinput-1010f-followups/RF3-gutter-drag.png), [RF3-RF4-reference.txt](https://raw.githubusercontent.com/ccheever/exact2/dcfe6ece0207419e41f235d8d8fe10929608e5ae/realinput-1010f-followups/RF3-RF4-reference.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f671c1a636f5c09d8df6d8c3e1905b81a544f7de/realinput-1010f-followups/drive-record.txt) |
| RF-3, review | pass (agent drive with the button held, reference over CDP, tests): during a drag from new line 3 to added 5 the "+" sits on added 5, the selection's bottom, as the reference's (before: the feature tip paints nothing and shows no "+"); after the release the draft opens under added 5. A "+" click opens one draft on the selection in every order of its sends (before: line 10's "+" with 3–5 selected opened "+10" in two of three orders) | [RF3 during the drag](https://raw.githubusercontent.com/ccheever/exact2/35152a00ff7902bfc341c0f0261844efb2a85483/realinput-1010f-followups/RF3-plus-during-drag.png), [after the release](https://raw.githubusercontent.com/ccheever/exact2/0d97ff9d3265ce733cbefeb663cd0697bfbb5f09/realinput-1010f-followups/RF3-after-release.png), [RF3-plus-click-test.txt](https://raw.githubusercontent.com/ccheever/exact2/0d97999f1541efa8809ec7d29e244e87c5cce428/realinput-1010f-followups/RF3-plus-click-test.txt), [review-drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/3d876404c434d179984f3ea52d0bb413ac521a2d/realinput-1010f-followups/review-drive-record.txt) |
| RF-4 | pass (agent drive, reference): before the trash's tap changed nothing (the card stayed); after the card and the badge go, as the reference. Real click: step 3 (open) | [RF4 image](https://raw.githubusercontent.com/ccheever/exact2/fa7333dff09452167bac5b988096ad1d30d03e3b/realinput-1010f-followups/RF4-pending-discard.png), [card before the click](https://raw.githubusercontent.com/ccheever/exact2/9cfadcf7cc1bfd262d0d33d61884d20e91737b59/realinput-1010f-followups/RF4-pending-card.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f671c1a636f5c09d8df6d8c3e1905b81a544f7de/realinput-1010f-followups/drive-record.txt) |
| RF-5 | implemented, AppKit test: before the window accepted mouse moves and a move away from the focused view reached it (5 failures), after it does not and the reporter's own area hears the moves. Agent hover is a hit test, so the popover itself is real-input step 4 (open) | [RF5-pointer-test.txt](https://raw.githubusercontent.com/ccheever/exact2/7c96b569f3a0a763c5b7d28cca84bd0e4999dc77/realinput-1010f-followups/RF5-pointer-test.txt) |

## Real-input batch steps

Launch this branch's bundle normally as a lane copy (`open -n --env …`, its own bundle id), the lane home outside any git
checkout, `R9_INPUT_LOG=$L/logs/r9-input.log`; for steps 2 and 3 pair the GitHub lane's server (`bun lane.mjs start primary
--port <lane port>`, `bun lane.mjs project primary`, a fresh `bun lane.mjs pair primary` link). No GitHub write: do not
press Submit review.

1. **RF-1, Services.** In a thread's timeline drag-select part of a message and right-click inside the selection: Cut
   (dimmed), Copy, Paste (dimmed), Select All, and nothing after them (no separator, no "Services ›"). Right-click in the
   composer with a word selected: no "Services ›" either (AutoFill there is realinput-1010g-followups RG-3).
2. **RF-3, the gutter.** Pull Requests › #168 › Code; scroll to docs/usage.md (310 files after the list's Load more) and
   unfold it. (a) Click new line 3's number: the draft opens on line 3 ("Add a comment…", focused), as the reference.
   Cancel. (b) Press on line 3's number, drag slowly to added line 5's number, release: lines 3, 4, deleted 5 and added 5
   paint while the pointer moves, and the draft opens under added line 5 at the release (its label `docs/usage.md:5`).
   Cancel. (c) The same from line 3's "+". Cancel. In (b) and (c) the "+" moves to the selection's bottom line while the
   pointer moves (added line 5 at the end), as the reference's; it does not stay on line 3.
   (d) The thread's Diff panel (a thread of the lane's `work` project, Diff open on `src/app.ts`): drag the numbers from
   line 2 to line 4 and release: the lines stay selected, no draft, and the "+" shows on line 4 only, without hovering.
   Hover line 7: no "+" there. Click the "+" on line 4: the draft opens under line 4 on lines 2 to 4. Cancel.
3. **RF-4, the trash.** Open a draft (line 3's "+"), type `lane check`, ⌘↵: the pending card and badge 1. Click the card's
   trash once: the card and the badge go.
4. **RF-5, hover after a press.** Usage › click Cost (it takes the focus) › 30 days. Rest the pointer on the unpriced (i):
   the popover opens; move 1–2 pt on the (i) several times and rest: it stays open; leave: it closes. If it closes on a small
   move or stops opening, press ⌥⌘T at once and keep the trace: its `hover in/out` lines should name only the (i) and the
   popup, never the Cost segment.

## Tests

- `diff-line-drag.test.ts` (new, 8): the number drag (press, moves, release, a click, another file's line), the press on
  the one selected line, Shift from the far end, the "+" from the selection's top, the cell ids.
- `pages-pr-code.test.ts` "the gutter's drags" (3 new, through the panel's resource and `chatlocal:pr-code-drag`): a drag on
  the numbers paints as it goes and opens the draft on its last line, a held draft refuses another; a click opens the
  draft on its line and adds the deleted line's position; a "+" drag comments on its range; a commit scope turns it off.
- `diff-review.test.ts`: the line comment test now selects with a click and a Shift-click drag (no draft opens in the Diff
  panel), and a new test drags on the numbers, clears a single selected line, and opens the draft from a "+" drag (`3 to 5`).
- `realinput-1010f-followups.test.ts` (new): DiffCell's pointer handlers, ids and `elementFromPoint`; the queued
  `lineDragChanged` in both panels (the "+"'s press too); the "+" drawn only on the selection's bottom while the file has
  one, and kept while its own drag is in flight; the pending card's border with `pointer-events="none"`.
- Review round (where the "+" sits; the "+" click's two sends): `diff-line-drag.test.ts` 3 new (`utilityPlacement`: no
  selection, a press, a drag down and up; stacked rows by row, split rows by row and side, an undrawn bottom; `gutterClick`);
  `diff-review.test.ts` 3 new (the pin follows a drag and goes with the selection; a "+" click in every order of its sends,
  on the bottom line and off the selection, opens one draft on "3 to 5": 2 pass / 1 fail with the head's handler; a "+" drag
  whose press comes before its release); `pages-pr-code.test.ts` 2 new (the pin in the Code tab during a drag; a "+" click
  in every order, and a press ending a "+" drag).
- Updated for the gutter: `usage-pooled.test.ts` (the window's pointer takers list the gutter's number and "+") and
  `audit-wave-followups.test.ts` (the chat sends' routing between the Files comments' and the rest).
- AppKit `macos/tests/contextmenu` `text-menu.swift` `testTheShellsMenuPopsUpWithoutServices` (2 failures with the tip's
  `T3TextContextMenu.swift`) and `macos/tests/activity` `testPointerMovesReachTheReporterAndNoFocusedView` (5 failures
  with the tip's `T3ActivityReporter.swift`).

Checks on the code head `a81d88261` (the branch merged with `feat(example)/t3-code` `4e2cf6d32`; all exit 0): `bun test
examples/t3-code --timeout 60000` 4427 pass / 1 skip / 0 fail (302 files); strict tsc; `contract build` of `app.contract`
(1397 lines; the plan 28,600,299 bytes); `cargo test -p t3-code-macos --lib` 18 pass; AppKit contextmenu 49 tests and
activity 10 tests, 0 failures; caps; the five checks (cargo build, cargo test 3679 pass / 0 fail / 34 ignored, clippy, fmt,
caps, boot). The bundle was built for the live drive at `2c23c369b`; what changed after it is the merge, test expectations
and records.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975` on lane `realinput-1010f-followups` (backend 16700, CDP 16701), paired with the GitHub lane (16703), a fixture page on 16704 | RF-3 click and drags, RF-4 discard, RF-2 float and drag (texts above) | [reference-scripts.txt](https://raw.githubusercontent.com/ccheever/exact2/0c07426ee1d0853f6933670dd121e00e5f25d436/realinput-1010f-followups/reference-scripts.txt) |
| Before drive | feature tip `596672b62`, built in worktree `t3-code-realinput-1010f-before` (the shared evidence base `c03d7e908` predates #400 and #406) | RF-4 the trash's tap changed nothing; RF-3 the drag changed nothing. Three runs to settle the steps: the tree reveal did not unfold the file, then the file header had to be brought into the virtualized list by key, then the pending card's trash sat under the review button at the window's bottom (the card is centered first now) | [drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/772c1c058e2dc57896eed27ecc50aabb4fe988bc/realinput-1010f-followups/drive.sh.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/f671c1a636f5c09d8df6d8c3e1905b81a544f7de/realinput-1010f-followups/drive-record.txt) |
| After drive (one) | `2c23c369b` (the queued drag sends) | RF-4 the card goes; RF-3 lines 3–5 paint and the draft opens | the images above |
| Review round, before drive | feature tip `596672b62` (the same worktree, not rebuilt) | during the drag nothing paints and no "+" shows. Two runs: the first stopped at the drag, as added line 5 was below the window (the unfolded file is brought to the top again now) | [review-drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/3e6234d5b8d3255028baef36da6cf60cc0e26ab2/realinput-1010f-followups/review-drive.sh.txt), [review-drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/3d876404c434d179984f3ea52d0bb413ac521a2d/realinput-1010f-followups/review-drive-record.txt) |
| Review round, after drive (one) | `06113041e` merged with the feature tip (`0ac7ab269`; bundle built from it) | during the drag lines 3–5 paint and the "+" is on added line 5; the release opens the draft under it | the RF-3 review row's images |

## Not done / not verified

- RF-1, RF-3, RF-4 and RF-5 under real input: open until steps 1–4 run (agent mode pops no menu, moves no real pointer,
  and its hover is a hit test).
- Declared, not built (EXACT2-GAPS): in the thread's Diff panel a press on the gutter leaves a skill chip's details open
  (the reference's outside press closes them): a node that hears `pointerdown` keeps the press from its ancestors (X71,
  main `issues/20261010-pointer-events-reach-ancestors.md`), and handing the press on grew the plan by 777 KB. The "+"
  during a drag is no longer declared: it is placed from the selection, as the reference's (review round), so X80 (main
  #414) no longer shows in the clone; its EXACT2-GAPS row says so, and its plan record and the issues README row (the
  coordinator's) still say the "+" stays on the pressed line.
- Observations for the coordinator, not findings of this record: the clone paints selected lines amber
  (`light-dark(#fef3c7, …)`), the reference blue; the reference draws the gutter's "+" at the right of the line number, the
  clone over the change bar at its left (the RF-3 image shows both).

## Next action

Real-input steps 1–4 join the next session. Close this record after it.
