---
name: 20261014-device-player-gestures
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-device-player-gestures
pr_url:
verified_commit:
---

# The floating device player's drag, resize and pill

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences (STATUS "Known
differences", B5 and B9); the coordinator's triage gave this task these rows, DP-2 first (one cause for DP-2, DP-4a,
DPC-3, DPC-4 and DP-6). The user decided on 2026-10-11 that every row is built now, matching the reference T3 Code.

| Id | Clone (realinput-1011c) | Reference |
| --- | --- | --- |
| DP-2 | A (-320, -160) drag from the dot moves the player about 1.5× the pointer: y −242 to the top clamp (964,102) | 964,184: x at the readable lane's stop, y exactly −160, the dot under the pointer |
| DP-3 | A press on the pill's "Open preview in right panel" and a drag moves the player; the padding's drag passes | A press on a pill button starts no drag (its buttons stop the pointer-down); the padding drags |
| DP-4a | A W or SW resize moves the whole player past the 672 stop (634, 474; 336, 340), the opposite edge not held | x ≥ 964 in every zone from its start (1024,280); the aspect kept |
| DPC-3 | A 37 pt top-edge drag grows the player to 307×682 (its top moves about 6× the pointer): no compact or essential card | 37 pt: the compact card; 32 more: the essential card |
| DPC-4 | After a 150 pt shrink the docked card does not return: the player drifted 80 pt left while growing | The docked card returns |
| DP-6 | A drag out of the window and back: once outside, the held drag no longer follows back inside | It follows back (Chrome keeps the pointer captured) |
| DP-5 | A slow 150 pt drag across the simulator's stream does not page the Springboard; the flick passes | The slow drag pages; the flick passes |
| DP-10 | At 840×620 the player cell is 175×380 | 240×522 (the minimum), at 624,124 |

Source records: `STATUS.md` Known differences (DP-2…DP-7, DPC-3, DPC-4; DP-10 in "Found"), the realinput-1011c notes
`target/t3-ui-parity/realinput-1011c/b5-sidebar-device/00-b5-notes.txt` (DP-1…DP-10, the reference pass) and
`b9-device-fixture/00-b9-notes.txt` (DPC-1…DPC-7); closed records `20261005-floating-device-player.md` and
`20261011-device-player-compact-and-rail-keys.md` (the densities, the popover, the fixture hub).

## Steps

1. DP-2 first: find why a drag moves more than the pointer; fix it as BrowserMiniPlayer keeps its gesture serial.
2. DP-3: the pill's drag off its buttons, onto a grab box behind them (the reference's buttons stop the pointer-down).
3. DP-4a, DPC-3, DPC-4: re-run after DP-2 (all eight zones; the card's densities; the shrink).
4. DP-6: re-test a drag out of the window and back after DP-2; if no pan arrives while outside, the Mac pan
   recognizer's issue goes to the coordinator (no upstream filing from a task).
5. DP-5: find what drops a slow drag's moves (the player's node over the stream, or `R7DeviceClient.sendTouch`).
6. DP-10: the 840×620 cell against the reference's.
7. Bun tests; one agent drive per side with the same ops; before / after / reference images; real-input steps.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DP-2 | Bun test (the contract's `pan` with the clock moving, into the canvas source); agent drive with the clock moving between moves | before / after / reference |
| DP-3 | Bun test (the pill's structure); agent drive: a press on the pill button and a drag; a press on the padding and a drag | before / after / reference |
| DP-4a | Bun test (eight zones, steps against one resize; the reference's code); agent drive, eight zones | before / after text |
| DPC-3, DPC-4 | Bun test (the card's density per drag); agent drive with the docked card | before / after / reference |
| DP-6 | agent drive (a drag out of the window and back, held) | before / after text |
| DP-5 | the touches a slow drag sends, panel against floating player | text record |
| DP-10 | Bun test; agent drive at 840×620 (a draft and a thread) | before / after / reference |
| DP-2, DP-3, DP-4, DP-5, DP-6, DP-7's rest half, DPC-3, DPC-4 under a real pointer | real-input steps below | — |

## Reference

- `apps/web/src/components/preview/ThreadPreviewMiniPlayer.tsx` MiniPlayerShell: `beginGesture` keeps the pointer's
  start and the frame on screen for the whole gesture, `handlePointerMove` applies the pointer's total since then
  (move: `clampPreviewMiniPlayerPosition`; resize: `resizePreviewMiniPlayer`), with no obstacles; the pill's buttons
  (`Button ghost icon-xs`) `onPointerDown={(event) => event.stopPropagation()}` (lines 427, 444), so the pill's padding
  drags (the group's pointer-down) and its buttons do not. `icon-xs` is `size-7 sm:size-6`: 24 pt from the sm width.
- `apps/web/src/components/preview/previewMiniPlayerLayout.ts`, `apps/web/src/components/chat/chatCanvasLayout.ts`
  (ported as is: `previewMiniPlayerLayout.ts`, `chat-canvas-layout.ts`), `ChatCanvas.tsx` (the composer overlay's
  height is the canvas's `composerHeight`, none in the draft hero state: `ChatView.tsx` passes
  `composerOverlayElement={isDraftHeroState ? null : …}`).
- `apps/web/src/components/device/DeviceStreamView.tsx` (pointer down / move / up → `sendTouch` begin / move / end, no
  throttle), `packages/client-runtime/src/device/stream.ts` (`sendTouch`: the same tagged message as the clone's).
