---
name: 20261014-device-player-gestures
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-device-player-gestures
pr_url: https://github.com/ccheever/exact2/pull/539
verified_commit:
---

# The floating device player's drag, resize and pill

Parent: [macOS parity specification](../spec.md). Guidance for this review repair: Exact knowledge library
`20261005-platforms-v3` (foundations, platforms, motion, accessibility, testing and debugging), checked against the
checkout's `docs/testing.md` and existing Contract/data tests. The repair removes an ineffective test and preserves
existing captures; it changes no application or framework behavior.

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

## Cause and fix

**DP-2 (and DP-4a, DPC-3, DPC-4).** `R6DeviceMiniPlayer.pan` built the gesture's serial from `floor(performanceNow())`
at every pan. The canvas source (`chat-canvas-view.ts` `applyGesture`) takes the frame on screen as a gesture's start
at the first answer of a new serial and applies the pan's total from there, so each move under a fresh serial restarted
the gesture from the already moved frame and added the whole total again: a drag moved by the sum of its running
totals, clamped (y −242 for −160), and a resize grew from a grown frame (the top edge about 6× the pointer, the W and SW
zones dragging the whole player left). An agent drag never showed it, because the agent's clock, and with it
`performanceNow()`, stands still between moves unless the drive moves it; realinput's hand moved it. Fix: the serial is
made at the gesture's first pan and kept in state until the release, as BrowserMiniPlayer's has been
(`browser-capture.contract`). With it the clone runs MiniPlayerShell's gesture exactly: every frame of a drag, the eight
zones, the densities and the shrink equal the reference's own code (the harness below) and the reference app (CDP).

**DP-3.** The pan was on the pill row itself; a pan on an ancestor takes a drag that starts on a button inside it (LLP
1057.001 rule 3), so a press on a pill button dragged. The pill now holds no pan: a grab box behind its buttons (over the
padding, the gaps and the 1 pt border) pans, the buttons lie over it (positioned, later in order), as BrowserMiniPlayer's
grab box does. The buttons are the reference's `icon-xs` at the sm width (24 pt, a 14 pt glyph, as BcPillButton's), so
the pill is 56 × 32 and the dot's centre meets the close button's top edge as in the reference: the dot's upper half is
padding (drags), its lower left the button (does not).

**DP-4a, finding text.** "A W or SW resize stops at 672" was read from the reference's run, which started at 1024,280:
there a 60 pt W resize ends at x = 972 − 300 = 672 (964 on screen) by its arithmetic, not by a stop. From the clone's start
(964,102) the reference's own code gives 904,102 273×592 for the same resize (the right edge held), and the reference app
gives the same (868,64 page = canvas 612,12, 273×592). The clone now does exactly that; with one press out 60 then in
200 it returns to 964 (the reference app's 928,64 page). "The opposite edge holds" is met where the reference meets it
(no zone in either drifts after DP-2's fix).

**DP-6.** After the fix, in agent mode, a drag pressed on the handle, taken past the window's right edge and back keeps
following (the pan's moves arrive while the pointer is outside: AppKit sends the drags to the view the button went down
on). No framework issue from this evidence; the real-pointer half stays a real-input row. In realinput the press on the
dot landed on the pill's close button (its ×, under the dot once the pill shows), so the drag ran on the pill row's pan;
after DP-3 that press no longer drags at all, as in the reference.

**DP-5.** Not reproduced. On a lane with the real expo-device-hub 0.12.0 behind a logging proxy and the lane's own booted
iPhone 17 Pro, a slow 150 pt drag (15 moves of 10 pt, 40 ms of real time apart) sends begin, all 15 moves and end, 55–70 ms
apart, from the panel's flat view and from the floating player, and the floating drag pages the Springboard (the
panel's, 41 % of its 360 pt screen, does not, as expected). `R7DeviceClient.sendTouch` has no throttle, and no node of the
player lies over the stream's middle. The realinput run dragged a player the DP-4 drift had left at 340,102, over the
chat, where its frame follows the composer's height; that position is not reachable by drags after the fix. No change;
the real-input step re-runs it.

**DP-10.** No change: the cells compared different states. The reference's 240×522 at 624,124 was a draft ("alpha / New
thread"): the hero composer is not the canvas's obstacle (`composerOverlayElement={isDraftHeroState ? null : …}`), so the
player keeps its 240 pt minimum. The clone's 175×380 was a server thread (`alpha-8`), where the docked composer is the
obstacle and "a tight container wins over the minimum" (previewMiniPlayerLayout.ts). The clone already passes no
composer height in the hero state (`app-main.contract` `overlaid = not hero`, `chat-canvas-view.ts`). Both apps at 840×620:
a draft 240×520 at canvas 380,36 in both; a thread 172×372 at 448,12 in the reference (its composer 172 pt) and 175×380 at
445,12 in the clone (its composer 165 pt), each its own layout of the same rule.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| DP-2 | pass (Bun test; agent drive with the clock moving, reference by CDP): a (−320, −160) drag in 20 moves, held: before y to the top clamp (canvas 672,12), after 672,96, the reference 672,96 (928,148 page); one serial for the whole drag | [image](https://raw.githubusercontent.com/ccheever/exact2/b84e431345fa6de3da00c9bdf62a67a9abb7b5b6/device-player-gestures/dp2-drag.png), [record](https://raw.githubusercontent.com/ccheever/exact2/2b8613cbd83a4dec0c8c3e0dcff31e570e109977/device-player-gestures/dp-record.txt) |
| DP-3 | pass (Bun test; agent drive; reference): a press on "Open preview in right panel" dragged (−100, +100): before the player moved to the bottom, after and the reference it stays and the button does not fire (the player still floats); the padding's drag (+60, +200) ends at 732,256 in both | [image](https://raw.githubusercontent.com/ccheever/exact2/aecc9b6e3ccde590a52d1b55830c92edb496ebb0/device-player-gestures/dp3-button-press.png), [record](https://raw.githubusercontent.com/ccheever/exact2/2b8613cbd83a4dec0c8c3e0dcff31e570e109977/device-player-gestures/dp-record.txt) |
| DP-4a | pass against the reference's code and app (Bun test: eight zones, six moves equal one resize; the harness: 26 of 26 scenarios equal the reference's own layout code, 11 of 26 before; agent drive, eight zones): before W drifted to 552 then 569, SW to 502 then 528; after W out 60 → 612,12 277×600, in 200 → 672,12 240×520 (the reference app 612,12 273×592 → 672,12; the height differs by the composer's height, below); the stop wording: see "Cause and fix" | [before/after images and limits](#review-repair-2026-10-14), [record](https://raw.githubusercontent.com/ccheever/exact2/2b8613cbd83a4dec0c8c3e0dcff31e570e109977/device-player-gestures/dp-record.txt) |
| DPC-3 | pass (Bun test; agent drive; reference): the docked card, the top edge up 37: before 721,145 291×631 and the full card, after and the reference 755,219 257×557 and the compact card (Open in Cursor, Add project script, main, Changes); 32 more: before 691,12 313×678, after and the reference 740,187 272×589 and the essential card (Add project script, main, Changes) | [compact](https://raw.githubusercontent.com/ccheever/exact2/d58e80616a089292988dc6ef7bb77a004356e2f4/device-player-gestures/dpc3-compact.png), [essential](https://raw.githubusercontent.com/ccheever/exact2/107a1bd0f4902376114f818e34177820f708e0b9/device-player-gestures/dpc3-essential.png) |
| DPC-4 | pass (Bun test; agent drive; reference): 37 more leaves the card under 160 pt (the header's popover, 723,150 289×626 in both); opened, then the top edge down 150: before the player at 691,169 (81 pt left) and no card, after and the reference 723,256 240×520 and the docked card back in full | [image](https://raw.githubusercontent.com/ccheever/exact2/85c52e5bf51e94e3e2d5e44457eeebf4b4642521/device-player-gestures/dpc4-back-inline.png) |
| DP-6 | pass in agent mode (before and after: the moves arrive outside the window): after, (+56, −136) out past the right edge, held: 772,120 (x clamped, y exactly −136); back (−450, +150), held: 672,256 (the lane stop, y clamped), as the reference's realinput 964,344; before the same path overshot to the top. A real pointer: real-input step 5 | [before/after images and limits](#review-repair-2026-10-14), [record](https://raw.githubusercontent.com/ccheever/exact2/2b8613cbd83a4dec0c8c3e0dcff31e570e109977/device-player-gestures/dp-record.txt) |
| DP-5 | not reproduced in agent mode (a simulator and the real hub): every move of a slow drag reaches the hub from the panel and from the floating player, and the floating drag pages; no change. A real pointer: real-input step 4 | [image](https://raw.githubusercontent.com/ccheever/exact2/62d631a9bb13186a2e7d211503e4bf00d16d0dd2/device-player-gestures/dp5-float-pages.png), [record](https://raw.githubusercontent.com/ccheever/exact2/2b8613cbd83a4dec0c8c3e0dcff31e570e109977/device-player-gestures/dp-record.txt) |
| DP-10 | pass, no change (Bun test; agent drive at 840×620; reference): a draft 240×520 at canvas 380,36 in both (the reference's 624,124 on screen); a thread fits above the docked composer in both (reference 172×372, clone 175×380); the recorded cells were a thread (clone) and a draft (reference) | [image](https://raw.githubusercontent.com/ccheever/exact2/140efb234c206edb7eb0319c49f4674999311d96/device-player-gestures/dp10-cells.png) |
| Real pointer: DP-2, DP-3, DP-4, DP-5, DP-6, DP-7's rest half, DPC-3, DPC-4 | open: "Real-input batch steps" | — |

Drives (agent mode, `scripts/agent.mjs macos` through the audit's `clone-drive.sh`, a fresh storage per session): lane
`device-player-gestures` (base port 16500: reference 16500, CDP 16501, clone server 16502) with a fixture device hub in
both T3 homes (a lane-local stand-in for `expo-device-hub@0.12.0`, the earlier task's, listing "Fixture iPhone (lane
fixture)"; no simulator), the Verification fixture's "Timeline verification" thread. Every drag step is one `tap move`
then `clock +40`: the agent's clock stands still otherwise, and the bug lives in time passing between moves. Before:
this worktree's build of the base `609f392d5` (the evidence worktree no longer exists), driven before any edit; after:
the bundle of `eae68d2e8`. Reference: the lane's T3 Code (Alpha) 1e2ecbd975 by CDP (Playwright's mouse) at 1280×840 and
840×620. DP-5: lane `dpg-sim` (clone server 16506) with the real hub behind a logging proxy on 16507 and this task's own
booted iPhone 17 Pro (shut down after; another job's booted iPhone 18 Pro was not touched). Harness (not committed):
`R6DeviceMiniPlayer`'s `pan` from the contract into `chatCanvasView`, each scenario against MiniPlayerShell's gesture
over the reference's own `chatCanvasLayout.ts` and `previewMiniPlayerLayout.ts` (26 scenarios: before 11 equal, after 26).

## Real-input batch steps

B5 lane (a real simulator): launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git
checkout) with `T3_LOCAL_HOME=<lane>/clone-t3-home` and `T3_LOCAL_PORT=<lane port>`, device support on and the real hub,
the iPhone 17 Pro booted; window 1280×840 at 36,38 (screen points below); Verification fixture › Timeline verification.
Right panel › Device › open the iPhone; ⌥⌘B floats it at 1064,346 240×522. Toggle thread details panel (the card closed).

1. **DP-2.** Rest on the dot (1292,356): the pill shows. Press on the dot's upper half (1292,354, the pill's top
   padding: openHand), drag (−320, −160) in 20 steps of 40 ms. Expected (reference): the player follows under the
   pointer to 964,186 (x at the lane's stop, y exactly −160), and stays there after the release.
2. **DP-3.** Rest on the dot; press on "Open preview in right panel" and drag (−100, +100), release off it: the player
   does not move and the button does not fire. Then press on the pill's top padding (2 pt below its top edge, between the
   buttons) and drag (+60, +200): the player moves +60 and down to the bottom clamp.
3. **DP-4 (eight zones).** Drag the player to 964,102. For N, S, E, W, NW, NE, SW, SE in turn: press 2 pt inside the
   edge's middle (4 pt inside a corner), drag 60 pt outward (hold: read the frame), then 200 pt inward, release. Expected
   (the reference's code and app): N none (at the top); S 964,102 ~272×592; E the same; W 904,102 ~273×592 (the right edge
   held), back to 964,102 240×522 after the inward drag; NW, NE none at the top; SW about 931,102; SE as S. No zone leaves
   the player left of 904 or drifting after the release. (The cursors are DP-4b, the main fix, not this task.)
4. **DP-5.** Float the device again (the default frame). A slow drag left of 150 pt on the stream (10 pt every 40 ms),
   starting 40 pt left of the stream's right edge: expected the Springboard pages (page 2). A fast flick right returns it.
   If it does not page, note the player's frame and repeat once in the right panel's flat view (rail › Flat view).
5. **DP-6.** Press the dot's upper half, drag out past the window's right edge (+56, −136, outside), hold, then back
   inside (−450, +150) with the button held: expected the player follows back to the lane's stop (964,…); release
   inside: no click. Then press, drag out, release outside, come back with no button: nothing moves.
6. **DP-7, the rest half.** Drag the dot (−40, +60) and rest the pointer: the pill shows under the resting pointer.

B9 lane (the fixture hub, the Pixel, as `realinput-1011c` B9): the docked card open, the Pixel floated under it.

7. **DPC-3.** Drag the player's top edge up 37 pt: the card shows the compact rows (Open in Cursor, Add project script,
   main, Changes); 32 pt more: the essential rows (Add project script, main, Changes).
8. **DPC-4.** 37 pt more: the card leaves (the header's Toggle thread details not pressed); click it: the popover opens
   under the player; drag the top edge down 150 pt: the popover closes and the docked card returns in full, the button
   pressed.

## Tests

- `device-player-gestures.test.ts` (16): DP-2, the contract's `pan` with the clock moving keeps one serial for a drag (a
  new one for the next) and a (−320, −160) drag moves the frame by the total once (672,96), the padding drag after it
  (732,256); DP-4a, each of the eight zones in six moves equals one resize of the total from the press's frame, and the
  W numbers from the lane stop (612 → 645 with the right edge held) and from the reference's own start (672,12 273×592);
  DPC-3 and DPC-4, the docked card's density per top-edge drag (compact, essential, the popover, back in full) at the
  reference app's frames; DP-3, the pill holds no pan, its first child is the grab box, the buttons are positioned 24 pt
  buttons over it, ten pans in the player; DP-10, a draft at
  840×620 keeps 240×520 at 380,36 and a thread equals the canvas layout with the docked composer. Against the base's
  `r6-device.contract` 9 of the original 17 failed (DP-2, S/W/SW zones, the W numbers, DPC, DP-3). The review repair
  removes the arithmetic-only pill-size test, leaving 16 tests; the structural DP-3 test still checks the authored
  button dimensions and stacking. Pill geometry remains covered by the recorded native drive, not by arithmetic.

## Checks

Run once on `6d2785d46` (the app code of `eae68d2e8`; `origin/feat(example)/t3-code` still `609f392d5`, nothing to merge):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4873 pass, 1 skip, 0 fail (329 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,453 lines (unchanged) |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust or Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed, 0 failed, 46 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | twice: the base for the before drive, this branch once before the after drive |

## Review repair, 2026-10-14

The independent review of `9927329b6` found no blocking correctness problems. It requested removal of the
arithmetic-only test and visual evidence for DP-4a and DP-6. The test has been removed. The original agent-mode
captures were still available under `target/dpg/{before,after}/s3` (DP-4a) and `s2` (DP-6); the copies below are
byte-for-byte originals, inspected alongside their `ops.json`, `frames.txt` and the published `dp-record.txt`.

These are the earlier runs: before `609f392d5`, after application code `eae68d2e8`, macOS agent mode at 1280×840,
fixture device hub, Timeline verification thread. They are not new real-pointer checks. The fixture's missing-provider
banner and connecting-video placeholder appear in both runs; these captures prove player placement, not live video
or provider operation.

| Captured state | Before | After | Recorded canvas frame, before → after |
| --- | --- | --- | --- |
| DP-4a: W resize out 60, held | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp4-west-out60-before.png) | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp4-west-out60-after.png) | 552,12 277×600 → 612,12 277×600 |
| DP-6: pointer outside the window, held | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp6-outside-held-before.png) | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp6-outside-held-after.png) | 772,12 240×520 → 772,120 240×520 |
| DP-6: pointer back inside, still held | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp6-back-held-before.png) | [image](../evidence/20261014-device-player-gestures/20261014-review-repair/dp6-back-held-after.png) | 672,30 240×520 → 672,256 240×520 |

DP-4a's reference frame remains the recorded CDP measurement, 612,12 273×592 (868,64 page); the composer-height
difference is explained below. A reference screenshot for that W resize was not saved: the original reference drive
records its frame without calling `shot`. DP-6 has no matching reference capture in this attempt. Those reference
images remain missing. Per the parent specification's 2026-10-06 continuation direction, this repair preserves the
measured behavior and reports the gap without repeating a visual-polish capture loop. DP-4b is the separate cursor
fix and has no new evidence here.

Focused verification on `9927329b6` plus this test deletion, with Bun 1.4.2:

```sh
bun test examples/t3-code/device-player-gestures.test.ts \
  examples/t3-code/device-player-compact-and-rail-keys.test.ts \
  examples/t3-code/chat-canvas-view.test.ts \
  examples/t3-code/chat-canvas-layout.test.ts \
  examples/t3-code/r6-media-device.test.ts --timeout 60000
```

Result: 67 passed, 0 failed, 3,938 assertions across five files (including all 16 remaining gesture tests).
`bun scripts/caps.mjs` and `git diff --cached --check` also passed. Native builds and the full repository checks above
were not repeated for this test deletion and evidence update.

## Next action

Ready for the coordinator's verification of this repair and draft PR #539 amendment. Then run the real-input batch
steps above (B5 with a simulator, B9 with the fixture hub); DP-7's rest half and DP-4's cursors (DP-4b, the main fix)
re-run in the same batch. This task stays active until those acceptance limits are resolved.

## Not done / not verified

- The real-pointer halves of DP-2, DP-3, DP-4, DP-5, DP-6, DP-7 (rest) and DPC-3/DPC-4: the steps above, for the
  coordinator's batch (the screen is locked; agent mode only).
- DP-5: not reproduced in agent mode, so no cause was found to fix; step 4 re-runs it with a real pointer.
- DP-4a and DP-6: before/after clone images are now preserved above; matching reference screenshots are missing.

## Seen while building, not a finding row

- The clone's docked composer overlay is 164–165 pt where the reference's is 172 pt (1280 and 840 wide), so wherever the
  composer is the player's obstacle the clone's player is a few points taller (DP-4 W: 277×600 against 273×592; DP-10's
  thread: 175×380 against 172×372). The rules are the same; the composer's height is not this task's.
