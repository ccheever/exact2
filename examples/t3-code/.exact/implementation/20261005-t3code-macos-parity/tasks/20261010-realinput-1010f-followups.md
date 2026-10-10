---
name: 20261010-realinput-1010f-followups
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
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

## Next action

Start now.
