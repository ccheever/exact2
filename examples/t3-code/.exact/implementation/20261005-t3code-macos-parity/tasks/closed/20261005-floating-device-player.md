---
name: 20261005-floating-device-player
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: feat(example)/t3-code-floating-device-player
pr_url: https://github.com/ccheever/exact2/pull/146
verified_commit: null
---

# Floating device player: drag, resize, chat lane and float on panel close

## Outcome

The floating device player works like the reference. The user drags it by its handle, resizes it
from all eight edges and corners with the aspect ratio locked, and the player stays inside the chat
area. The chat column moves or narrows so the player does not cover messages. The player avoids the
composer and the thread details card. It floats when the user closes the right panel while a device
is active. Position and size live for the session only, as in the reference.

## Scope and exclusions

Included (the player, its float/restore/close and auto-show are done; reuse them):

1. **Ported layout modules with tests**: `previewMiniPlayerLayout.ts` (device functions
   `resolveDeviceMiniPlayerSourceSize`, `resolveDeviceMiniPlayerCornerRadius`, `resolvePreviewMiniPlayerFrame`,
   `clampPreviewMiniPlayerPosition`, `resizePreviewMiniPlayer`, and the constants), `chatCanvasLayout.ts`
   (`resolveChatCanvasLayout`, `DETAILS_CARD_CLEARANCE`), `previewMiniPlayerStore.ts` (`open`, `close`, `move`, `resize`,
   `removeThread`, `previewMiniPlayerSourceKey`), `threadDetailsCardLayout.ts` (`resolveThreadDetailsCardLayout`,
   `resolveThreadDetailsCardDensity`). Drop the browser source and `resolvePreviewMiniPlayerSourceSize`; record this in the headers.
2. **Gestures** in `R6DeviceMiniPlayer`: drag from the handle or the pill; eight grab zones; stale-source guard; stored width
   and position resolved on every layout pass (never written back clamped).
3. **Chat lane**: replace the clone's own lane derives (`app-main.contract:118-127`, `laneInset`, `stackWidth`, `stackLeft`) with the
   ported `resolveChatCanvasLayout` output for the timeline, the composer lane and the scroll-to-end button; expose `overlapsChat` and
   `overlapsDetailsCard`; fold the details card as `resolveThreadDetailsCardLayout` says.
4. **Source size and corner radius**: orientation-aware source size; Android radius `max(12, round(min(w,h) × 0.14))`; pill inset
   `max(8, round(radius × 0.55))`.
5. **Float on panel close**: closing the whole right panel (⌘⌥B, the header button, the sheet backdrop) with an active device
   surface opens the player and closes the panel (`ChatView.tsx:5631-5648,5790-5805,11511`). Closing a tab, ⌘W on a tab and "Close
   device panel" do not float.

Excluded: browser source, picture-in-picture, recording indicator, persistence across relaunch (the reference has none),
auto-show rules (done in round 12), the device surface itself.

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`apps/web/src/components/preview/{previewMiniPlayerLayout.ts:12-18,148-386,ThreadPreviewMiniPlayer.tsx:66-81,264-470}`,
`apps/web/src/components/chat/{chatCanvasLayout.ts:21-184,ChatCanvas.tsx:18-137,threadDetailsCardLayout.ts,ThreadDetailsCard.tsx:28-100}`,
`apps/web/src/previewMiniPlayerStore.ts:64-145`, `index.css:2270-2305`, `ChatView.tsx:5285-5357`.
Key numbers. Edge gap 12; frame radius 12; default box 320×320; minimum 240×150 (the width is the only free size, height = width ÷ aspect,
both rounded). A drag stops at `padding + minChatWidth + 12` from the left (padding 20, minimum chat 640) unless the last move was a resize.
A new player sits bottom-right beside the composer (`chatCanvasLayout.ts:71`). Grab zones: edges 8 pt thick centered on the edge, corners 16 pt
centered on the corner (at root size 16). Drag and resize gestures use no obstacles; the layout pass applies the composer and the card. The
store is not persisted. Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction, motion (gesture start, drag, cancel,
release, repeat), accessibility, testing-and-debugging. Pointer capture, `pan` outside the node and hit testing outside a clipped parent are
unknown in the library. The clone's evidence: `pan` gives incremental viewport deltas (`app.contract:984-988`) and `panrelease` ends it
(`diff.contract:155`); hit testing of visible overflow exists on main (`EXACT2-GAPS.md` "Already fixed"). Clone files to reuse:
`r6-device.contract:249-283`, `r12-threads-device.ts:28-100` (`placeMini` is replaced; its tests at
`r12-threads.test.ts:149-153` are rewritten to the reference tests), `r6-media-device.ts:139-159,265-290`, `r4-surfaces-panel.ts:187-202,258`,
`app-main.contract:189-190`, measured frames `chat` and `overlay` (`T3ComposerFrames.swift`). Consumer framework revision: the `main` pin of
`20261005-clone-on-exact2-main`. Lane constants (padding, 46 rem maximum, 40 rem minimum) come from one function (TN5), so the later
rem conversion that follows `20261005-interface-font-size` changes one place. The reference keeps the player state in memory only (TN6).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `drive.mjs` (pointer drags), `electron-oracle.mjs`,
`lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| recorded decision | Order with `20261005-interface-font-size`: this ticket goes first | none | — | — |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X22](../../issues/20261005-x22-reactive-layout-facts.md) | Reactive layout facts (container size, composer height, card rect) | `EXACT2-GAPS.md` X22 | nonblocking (workaround: `t3-frame` hooks) | Reuse frames `chat`, `overlay`; add a frame for the details card if it is not measured |
| [X8](../../issues/closed/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Handle, pill and grab zones are Contract nodes: drive them with the agent pointer form; drags over the native stream are attended |
| [X13](../../issues/20261005-x13-hover-keys-during-pan.md) | No hover events during a pan | `EXACT2-GAPS.md` X13 | nonblocking (workaround: state) | Keep the pill visible while `dragging` is true |
| [X24](../../issues/closed/20261005-x24-still-pointer-rehover.md) | Hover under a still pointer after layout | `R10Connect.swift` | nonblocking | Check the pill after the lane moves Update 2026-10-07 (adopt-main-fixes-shell): fixed on main #174; the host re-hovers a resting pointer after layout and scrolling, and R10Connect's re-hover is removed. |
| Cursor keywords for resize and grab | `cursor` on macOS (`20261005-main-fix-adoption`) | Keyword list unknown | unknown | `contract vocab --json cursor` at prepare; if a keyword is missing, record a new gap (new — record at prepare) |

## Implementation notes

- Tests to port (`bun:test`, original names): `previewMiniPlayerLayout.test.ts` 33 of 35 (skip the 2 `resolvePreviewMiniPlayerSourceSize`
  tests at `:29-45`), `chatCanvasLayout.test.ts` 20 (rename the key `browser:one` to a device key), `previewMiniPlayerStore.test.ts` 6
  (switch-source cases use device sources), `threadDetailsCardLayout.test.ts` 10. `BrowserViewportResizeDirection` becomes a local type
  (`browserViewportLayout.ts:25-33`).
- Data flow: Contract measures the container, composer height and card rect; TS runs `resolveChatCanvasLayout`; Contract draws the frame and
  lane insets. The gesture action accumulates the incremental `pan` deltas from the frame at gesture start and calls `clampPreviewMiniPlayerPosition`
  or `resizePreviewMiniPlayer` with that total; `panrelease` ends the gesture and clears `dragging`.
- Structure: a wrapper `box` holds the clipped frame (`overflow="hidden"`, radius), the handle, the pill and the eight zone boxes. The zones are
  siblings of the clipped frame so that the part outside the frame receives the press. A press on a pill button must not start a pan
  (`stopPropagation` in the reference). Zone `role="presentation"`.
- Stream report: the source size needs the screen orientation (`landscape_left`/`landscape_right` swap long and short). Check that the report in
  `r6-media-device.ts:145` carries it; if not, add it in `R6DeviceStream.swift`.
- States. Loading and error: the stream status layers already exist (`R6DeviceLoading`). Hover: the dot hides and the pill shows over the handle or
  the pill (opacity fade with `transition-opacity`; measure its duration on the oracle). Keyboard focus: the pill shows while a pill button has focus (the reference uses `group-focus-within`); Tab reaches
  "Open preview in right panel" and "Close floating preview". Dragging: cursor `grabbing`; resize zones show their resize cursor. Disabled and
  permission: none. Empty: no player. `aria-label` on the region "Floating device preview" and both pill buttons (exist). Motion: only the pill/dot
  opacity fade; no layout animation. Under reduced motion the swap is instant. Reversal and repeat: a second drag starts from the current frame; a
  drag stopped by a new source key is ignored. Escape: the reference player has no key handler, so Escape does not cancel a drag (the drag ends on
  pointer up or cancel); the pill is not a menu and the ticket adds no dialog. Keyboard resize and move do not exist in the reference.
- Float on close: add the float step to the panel-hide path (`r4-surfaces-panel.ts:187`); keep `close`/`close-panel` as they are.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | The named tests pass; old `placeMini` tests are rewritten, not deleted | macOS | log |
| Drag | Lane backend (`target/t3-ui-parity/lane-backend.sh`, ports 16000–16999) with the device fixture hub; player floated | Agent pointer down on `device-mini-handle`, move, up (`target/t3-ui-parity/drive.mjs`, as the settle sweep in `AGENT-HANDOFF.md:84,252`) | `layout device-mini-player` shows the moved frame; edges ≥ 12 pt from the canvas; left limit 672 pt from the canvas edge at root 16 | macOS 1280×840 | `--json`, `layout` |
| Resize ×8 | Same | Pointer drag each zone | Aspect equal within 1 pt; opposite edge fixed; minimum 240×150; stops at the canvas edge; corner uses the diagonal | macOS | `layout` before/after |
| Lane | Chat canvas ~1000 pt wide (right panel open at 1280×840) | Float, drag right, resize large | Chat column moves left, then narrows; `overlapsChat` true when it cannot; composer lane and scroll-to-end button follow; numbers equal the ported function's | macOS | `layout`, `state` |
| Card and composer | Inline details card open (container ≥ 984 pt); then narrow | Drag under the card; grow | Player clears the card; the card folds only when no slot exists; composer stays clear when chat overlaps | macOS | `layout`, shots |
| Pairs | Same scenes, default, dragged, resized | Screenshots | Pairs with the oracle (`target/t3-ui-parity/electron-oracle.mjs`) at 1280×840 and 840×620 (manual float via More › Float at 840 pt), light and dark | macOS | pairs |
| Float on close | Device surface active | ⌘⌥B; header button; then tab ×, ⌘W, "Close device panel" | First two float the device and hide the panel; the others do not float | macOS | `--json`, `state` |
| Orientation and radius | Fixture iOS and Android devices; rotate | Rotate; float Android | Landscape swaps aspect; Android radius matches the formula; pill inset follows | macOS | `layout`, shots |
| Session only | Moved player | Relaunch | No stored position or width | macOS | `state` |
| Window resize | Player bottom-right | Shrink then grow the window | Frame clamps while small and returns to the stored width when large | macOS | `layout` series |
| Pill and keys | Player | Hover; Tab to a pill button; reduced motion on | Pill shows on hover and focus; instant under reduced motion; pill stays during a drag | macOS | frames, `tree --ax` |
| Real input — partly agent (exact2 #186; adopt-main-fixes-r3): a mouse drag over the stream sends touches begin/move/end in order through `NSApplication.sendEvent` in a never-key window (`macos/tests/r7-device` `testAgentMouseDragOverTheStreamSendsTouches`); live `tap device-mini-screen drag … mouse` needs a booted simulator on the lane (optional in the r3 drive). Still not agent-drivable: a drag outside the window and back (the agent refuses a point outside the viewport), the eight resize cursors' look (no system-cursor readback). Was `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch, real trackpad | Drag across the native stream; flick; drag outside the window and back; all eight cursors | Cursor shapes, smoothness and capture match the oracle; no stuck drag | macOS | notes, video |
| Standard gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `previewMiniPlayer*.ts`, `chat-canvas-layout.ts`, `thread-details-card-layout.ts` and tests, `r6-device.contract`,
`r6-device-shapes.contract`, `r6-media-device.ts`, `r12-threads-device.ts` and its test, `r4-surfaces-panel.ts`, `app-main.contract`,
`modules/apple/R6DeviceStream.swift`, `AGENT-HANDOFF.md`.
Required environment: lane backend with the device fixture hub, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Implemented on `feat(example)/t3-code-floating-device-player` (2026-10-06); verification: unverified.

- Ported with tests: `previewMiniPlayerLayout.ts` (33 of 35 tests), `previewMiniPlayerStore.ts` (6),
  `chat-canvas-layout.ts` (21, all of `chatCanvasLayout.test.ts`), `thread-details-card-layout.ts` (10). Headers record
  the dropped browser source and `resolvePreviewMiniPlayerSourceSize`.
- `chat-canvas-view.ts` is the `chatCanvas` source (registered in `app.contract`, `app.ts`, `macos/src/markdown.rs`):
  container from the `chat` frame below the 52 pt header, composer height from `overlay`, the inline card from the new
  `details-content` frame (its full content height), lane metrics from `chatLaneMetrics` (TN5). It answers the lane
  (`left`, `width`, `insetEnd`), `overlapsChat`, `overlapsDetailsCard`, the card's fold and the player's frame,
  radius (Android formula) and pill inset. While a player or the card shows it watches `t3.status` and reads fresh
  frames and stream reports (orientation-aware source size).
- Gestures (`R6DeviceMiniPlayer`, `r6-device.contract`): the handle and the pill drag; eight zones (edges 8 pt, corners
  16 pt, siblings of the clipped frame) resize with `ns/ew/nwse/nesw-resize` cursors; `grab`/`grabbing` on the handle.
  A gesture is the canvas resource's argument `serial|sourceKey|direction|dx|dy` (the pan total); the first answer of a
  serial takes the frame on screen as the start, so a re-ask is idempotent; a gesture on another source is dropped
  (stale-source guard). The store keeps the stored width and position; the layout pass never writes its clamp back.
- Lane: `laneInset`, `stackWidth`, `stackLeft` and `composerLeft` come from the canvas answer (centred until the first
  answer). The card's `maxHeight` is the canvas's fold; it leaves when no 160 pt placement is left.
- Float on panel close: `surface-hide` (⌘⌥B, the header and panel toggle, the sheet backdrop, Escape) floats the active
  device (`closePreviewPanel`); tab close, ⌘W and "Close device panel" do not.
- Pill: shows on handle/pill hover, pill-button focus (buttons stay focusable at opacity 0 and refuse the pointer) and
  during a gesture; opacity fade 150 ms, instant under reduced motion.

Remaining differences: the card has one density (the reference's compact/essential rows are not built; it scrolls);
when no placement is left the inline card hides instead of becoming the popover; a window resize lays the lane out one
answer late (layout facts reach TypeScript through `t3-frame`, [#127](https://github.com/ccheever/exact2/issues/127));
drags over the native stream and real-trackpad capture are unverified (attended,
[#107](https://github.com/ccheever/exact2/issues/107)); hover under a still pointer after the lane moves is
[#139](https://github.com/ccheever/exact2/issues/139). Not run: oracle pairs, 840 pt and dark cells, Android rotation
live, relaunch, window shrink/grow live, `tree --ax` focus check.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | branch head | `bun test examples/t3-code` 1280 pass / 0 fail (base 1200); strict tsc clean; `contract build` 2164 slots, 43 resources; `cargo test -p t3-code-macos --lib` 10 pass; macOS bundle build pass; no Swift changed (no AppKit binary touched) | One live drive (macOS 1280×840, lane server 16170, booted iPhone 17 Pro): floated on panel close at canvas (772, 254) 240×522 beside the card; drag (−320, −160) stopped at canvas x 672 with the composer lane 680→640; NW resize to 276×600 at (561, 12), card folded away, composer clear; 93 `chatCanvas` answers, no errors | real input attended; oracle pairs not run |

## Next action

Review of the PR into `feat(example)/t3-code`; attended real-input rows; oracle pairs.
