---
name: 20261010-realinput-1010d-followups
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010d-followups
pr_url: https://github.com/ccheever/exact2/pull/400
verified_commit: 3a88f3a9f5b29324650017fe99ffd5362e77f968
---

# New findings of the real-input session of 2026-10-10 (realinput-1010d)

## Outcome

The session `realinput-1010d` drove #349's fix round, the rest of #383 and #384, #296 U6 and two #346 rows on
lane copies with real keys and pointer. #349's acceptance rows passed and it merged (`70e2ddd4b`). These findings are
new. Each is compared with the reference first: a row that matches the reference is closed as such, not changed.

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RD-1 | #349 pill | The floating player's pill shows when the pointer rests on the toolbar dot. | When the pointer arrives on the dot and rests, the grab hand shows but no pill, until the pointer moves again (3 of 3). Once showing, the pill holds. | [pill](https://raw.githubusercontent.com/ccheever/exact2/4f2df998e1042f628051a603eff6e80fba76b23e/realinput-1010d/D4-349-pill.png) |
| RD-2 | #349 Float | To check first: where the reference puts the player when it floats again after being moved. | Floating again with Toggle right panel puts the player back in the default corner at 240 × 333, not where it was left. | [D4](https://raw.githubusercontent.com/ccheever/exact2/10c4009a2b19eb891713cd0fed56905057929b93/realinput-1010d/D4-349-fix-round.png) |
| RD-3 | #346 row 3 | The agent's cursor glides to each target and pings on it. | In the 1280 × 800 Responsive viewport the cursor reaches each target's x but sits about 270 pt below it (driven by #352's lane-only fake ACP agent, "cursor" scenario). | [D6](https://raw.githubusercontent.com/ccheever/exact2/9faadb38bd7f3c0e59354496ebe247b0d798e906/realinput-1010d/D6-346-r3-agent-cursor.png), [log](https://raw.githubusercontent.com/ccheever/exact2/a4b636d6dacfbf3da5e61c2cb14bc60044564296/realinput-1010d/D6-346-r3-automation-log.txt), [agent](https://raw.githubusercontent.com/ccheever/exact2/28736eb368a0d3e74dc58a3d9b213c339a3837fc/realinput-1010d/D6-fake-acp-cursor.mjs.txt) |
| RD-4 | #384 step 4 | Right-click on selected timeline text: Cut (disabled), Copy, Paste (disabled), Select All (`DesktopWindow.ts`, context-menu). | AppKit's text menu: Look Up, Copy, Speech, Services. | [D2](https://raw.githubusercontent.com/ccheever/exact2/a308565c52b88f44620c19ab3079e5888278fca4/realinput-1010d/D2-384-help-tags-menus-context.png) |
| RD-5 | #383 step 4 | The snooze menu's items fit inside the menu. | "Mon 9:00 AM" runs past the menu's right edge. | [D1](https://raw.githubusercontent.com/ccheever/exact2/7051ebd9ca964887ec425fe240ce19e889b24afa/realinput-1010d/D1-fx2-one-highlight.png) |

## Scope and exclusions

Included: RD-1 to RD-5. Where a row's cause is in the framework (for example, if RD-4's text menu cannot be replaced
from app code, or RD-1's hover needs a move the host never sends), record it in `EXACT2-GAPS.md` with a one-file repro
and leave the framework alone; the main issue is filed separately.

Excluded:
- #383 step 4's second Escape leaving Filters open: the same cause as RC-3, fixed in
  [realinput-1010c-fixes](20261010-realinput-1010c-fixes.md).
- #308's gutter drag: not run, because the GitHub lane's Code tab answered HTTP 503. It is a re-check for the next
  real-input session (STATUS), not a fix.
- #296 U6's race (pairing before the server registers itself as This machine): real input cannot time it; its unit
  test stands.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RD-1..RD-5 | reference comparison first; then a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Cause and fix

- **RD-1 (built; real-input row open).** The pill showed only from a hover of the 12 pt handle (`hover=overDot` on
  `browser-mini-handle`). Under the real pointer the session's arrival (8 posted moves 20 ms apart, then rest) brought no
  hover there until another move. An agent drive cannot reproduce it: the agent's hover is a hit-test, not the window
  server's tracking area (`AgentMac.swift` hover). The pill now reads the player's own `hover` and `pointermove`
  (`browser-mini-player`, `overPlayer`/`movePlayer`): each move places the pointer on the handle ("dot"), the rest of the
  pill's box ("pill") or the page (`bcPlayerZone`), the pill shows on the handle and stays on its box while it shows (the
  reference's `group-hover`), and leaving the player hides it. The player's box has heard the pointer since it crossed the
  page, so the arrival is a move there. The agent's order (move, then enter) is kept: entering reads the zone the last move
  found. The framework side (hover rides per-node tracking areas, X62, main `issues/20261009-macos-subtree-hover.md`) and a
  one-file repro of the miss are in `EXACT2-GAPS.md` ("Floating player's pill").
- **RD-2 (matches the reference; no change).** The reference (CDP, same lane data): floated at (1028, 495) 240 × 333,
  dragged (-300, -200) and NW grip (-80, -80) to (863, 204) 305 × 423, then Toggle right panel twice: back at (863, 204)
  305 × 423, because the panel's showing the tab only hides the player and its store entry stays
  (`shouldRenderPreviewMiniPlayer`, `closePreviewPanel` reopening the same source is a no-op). Only the pill's "Open in
  right panel" (`openInPanel` closes the entry) or the chrome's Float pressed on a floating tab (`handlePictureInPicture`)
  forget it: re-floated, the player is at the default (1028, 495) 240 × 333, as in the clone. The clone before and after
  this branch gives the same frames as the reference on the same steps ([frames](https://raw.githubusercontent.com/ccheever/exact2/ca48e573bd42dc0583df9b910cec9181f78f9490/realinput-1010d-followups/rd2-frames.txt)). The session's
  return to the default likely came through one of those two routes; a real-input re-check is in the steps below.
- **RD-3 (built; real-input row open).** `T3BrowserAgentCursor` placed the arrow at `bounds.height - y` when its layer's own
  `isGeometryFlipped` was false. The stage's host (`T3BrowserView.Host`) is a flipped view, so the page's layer is drawn
  y-down although its own flag is false: the cursor landed at the page's height minus the target's y (in the 1280 × 800
  Responsive viewport, about 270 pt below). It now converts the page's point with AppKit (`convertToLayer` from the view's
  own coordinates) and orients the arrow, its tip and the ping by the layer's drawn axis (`layerPointsDown`), recomputed at
  each show because the page moves between the panel, the floating player and the off-screen host.
- **RD-4 (built; real-input row open).** ExactKit answers a right-click on selected text with no authored `contextmenu`
  with an NSTextView's read-only menu (Look Up, Copy, Speech, Services; LLP 1115 D8). The T3 desktop shell writes its own
  (`DesktopWindow.ts` `installContextMenu`). `T3TextContextMenu.swift` takes the right-click in a local monitor wherever
  ExactKit would show its text menu (its menu carries Look Up) and pops the shell's on the same text node: Cut (disabled),
  Copy (the node's `copy:`, its `copy` event first), Paste (disabled), Select All (the node's `selectAll:`), with the
  accelerators Electron shows. The monitor ends the click there, so ExactKit's menu does not follow; under the agent it
  logs the items (`t3.textmenu:`) and tracks no menu. `EXACT2-GAPS.md` "Text context menu" says why a monitor and what
  stays out (below). Review round (`1f25f2645`): the first round's monitor did not end the click. Its closure
  `self?.handle(event) ?? event` flattened `handle`'s `NSEvent?`, so `handle`'s nil ("taken") became the event again and
  the click went on to `NodeView.rightMouseDown`, whose `super` call pops ExactKit's Look Up menu: under real input the
  shell's menu and then ExactKit's on the same click, and under the agent the log line and then ExactKit's menu tracking
  (the first after drive's 120 s hang, below). The closure now returns `handle`'s result
  (`guard let self else { return event }; return self.handle(event)`); two AppKit rows send the click through
  `NSApp.sendEvent` to the installed monitor and fail on the previous closure.
- **RD-5 (built).** The snooze menu had a fixed `width="10.8rem"`; MenuPopup sizes to its content from one minimum
  (`min-w-[min(10rem,…)]`, 192 px in the reference on a weekday). It is now `min-width="10rem"` and sizes to its rows.

Found while building, outside this record's rows: the reference also shows the shell's menu where ExactKit shows none
(a right-click on unselected text or an empty area: Cut, Copy and Paste disabled, Select All; Copy Link over a link). A
`contextmenu` on the window's root could carry it; not built here (`EXACT2-GAPS.md` "Text context menu").

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RD-1 | Built. Agent: the pill on the dot, a pill button's tooltip, the pill hidden over the page (the agent's hover reaches the player's `pointermove` first, then its hover). Bun: the row evaluates `bcPlayerZone` (`contractFn`) on both sides of each edge of the 12 pt handle and the pill's box, recording and a larger inset included; four mutated inequalities or insets each fail it. The real pointer's arrival is a real-input row (open). | [image](https://raw.githubusercontent.com/ccheever/exact2/6cb2717c941ab788220c360d2540de0bf2997bdc/realinput-1010d-followups/rd1-pill-agent.png); Bun `browser-capture.test.ts` "realinput-1010d RD-1" ([zone test](https://raw.githubusercontent.com/ccheever/exact2/6389f57e929332b39c61a723350a9fbaf92ec683/realinput-1010d-followups/rd1-zone-test.txt)) |
| RD-2 | Matches the reference: the same frames after Toggle right panel twice before and after this branch and in the reference; no change. Real-input re-check (open). | [image](https://raw.githubusercontent.com/ccheever/exact2/028aa1b03e52baef437f91c49dbea7134995b525/realinput-1010d-followups/rd2-refloat.png), [frames](https://raw.githubusercontent.com/ccheever/exact2/ca48e573bd42dc0583df9b910cec9181f78f9490/realinput-1010d-followups/rd2-frames.txt) |
| RD-3 | Built. AppKit: the tip on #go and #name in a flipped host scaled 0.5, arrow up; fails on the branch tip (y 84 against 375, the arrow upside down). The fake-agent drive is a real-input row (open). | [test output](https://raw.githubusercontent.com/ccheever/exact2/5bcce7d8f9c36a2ecc2cf3474fcf1a33203177b8/realinput-1010d-followups/rd3-agent-cursor-test.txt) |
| RD-4 | Built (fixed in the review round). AppKit: 5 rows; the 2 that send the right-click through `NSApp.sendEvent` to the installed monitor (normal and agent) find the host view's `rightMouseDown` never reached, and fail on the first round's monitor (reached once each). Agent drive of this round's bundle: a double-click selects "status" in the provider banner, `tap … contextmenu` there answers in 21 ms (the first round's: no answer within 120 s), the app logs `t3.textmenu: Cut (disabled) \| Copy \| Paste (disabled) \| Select All` once, and the next ops answer. The drawn menu, and that ExactKit's does not follow it, is a real-input row (open). | [text before/after](https://raw.githubusercontent.com/ccheever/exact2/5759b827d67ee61460603f0a2f9ec24a2b96d4fa/realinput-1010d-followups/rd4-monitor-fix.txt); [image](https://raw.githubusercontent.com/ccheever/exact2/bc9b78cda526bc7ecb6c96ce86c1dc78f1d72434/realinput-1010d-followups/rd4-text-menu.png) (its After panel is the agent's log line only, no proof that the click was replaced) |
| RD-5 | Pass. Agent: the menu sizes to its rows and "Mon 9:00 AM" sits inside, as the reference's. | [image](https://raw.githubusercontent.com/ccheever/exact2/c9eb38bd228eae91de038ff9df60bb7b37ae6a35/realinput-1010d-followups/rd5-snooze-menu.png) |

Drive: one agent-mode session of this branch's bundle (`8a33343ef` code) and the same steps on the branch tip's bundle
(`7195f10b6`, built in this worktree before any change), lane base 16560, provider CLIs pinned to a missing path on both
sides ([drive](https://raw.githubusercontent.com/ccheever/exact2/4d89d8d01ddcf79a499810aa054d82fcbd67cf2d/realinput-1010d-followups/drive.sh.txt), [reference driver](https://raw.githubusercontent.com/ccheever/exact2/0d82d4ee4ef066f82c4659668f17981c2d4f0e22/realinput-1010d-followups/ref-drive.mjs.txt),
[compositing](https://raw.githubusercontent.com/ccheever/exact2/37c4c2fce293fbc41a209ca9ac084ae23e7838f0/realinput-1010d-followups/refcomp.py.txt), [compose](https://raw.githubusercontent.com/ccheever/exact2/a61e0ffda108515a1d9cba00bbd0552bcb130a05/realinput-1010d-followups/compose.py.txt)). In that after drive the agent's `tap … contextmenu`
on the selected text did not answer within 120 s after the module had logged the shell's menu (the drive's last op; the
images were taken before it). The first round put that down to the branch tip's limit. That was wrong: the module's
monitor let the click through, and ExactKit's menu tracked after the log (RD-4's cause above). The review round's drive
(one agent session of this round's bundle, code `1f25f2645` merged as `275e7849e`, same lane and provider pinning; only
RD-4's ops, [drive](https://raw.githubusercontent.com/ccheever/exact2/2030b0ac28c2d50e4ff7c55c1c44c9d963606391/realinput-1010d-followups/drive-r2.sh.txt)) answers that op in 21 ms and exits 0
([text](https://raw.githubusercontent.com/ccheever/exact2/5759b827d67ee61460603f0a2f9ec24a2b96d4fa/realinput-1010d-followups/rd4-monitor-fix.txt)). The other rows' images did not change in this round.

## Real-input batch steps

Launch a lane copy of this branch's bundle normally, a Browser tab of the thread on a local page (the audit lane's
fixture), floated with the chrome's Float preview over chat. Move the pointer with posted moves (`move x y 8`).

1. **RD-1, arrival.** From the composer, move onto the floating player's dot (its 12 pt handle at the top right) in one
   `move` of 8 steps and rest 1 s: the pill shows (3 of 3). Then move onto Pop into separate window: its tooltip shows
   after the delay and the pill stays. Move onto the page: the pill hides. From the dot, leave the player up and right in
   one quick move (out of the chat): the pill hides. If the pill still waits for another move at step one, run the
   one-file repro in `EXACT2-GAPS.md` ("Floating player's pill") with the same moves and record its log.
2. **RD-2, re-float.** Drag the pill's padding (-300, -200), then the NW grip (-80, -80); real clicks on Toggle right panel
   (the panel shows the tab, the player hides) and on the panel's Toggle right panel: the player is back where it was
   left, 305 × 423. Then the pill's Open in right panel and Toggle right panel: the default corner, 240 × 333 (as the
   reference). Record the route if the first case returns to the default.
3. **RD-3, agent cursor.** In the panel at the 1280 × 800 Responsive viewport, run #352's lane-only fake ACP agent's
   "cursor" scenario: the arrow's tip glides to Go, the Name field and Page B and pings on each (no offset below them).
4. **RD-4, text menu.** In a thread's timeline, drag-select part of a message (or the provider banner's text) and
   right-click inside the selection: the menu is Cut (dimmed), Copy, Paste (dimmed), Select All, with ⌘X, ⌘C, ⌘V, ⌘A shown,
   and no Look Up, Speech or Services. Close it with Escape: no second menu (ExactKit's Look Up, Copy, Speech) pops on
   the same click. Copy, then paste in the composer: the selected text. Right-click again, Select All:
   the window's text is selected. Right-click a link, a sidebar row and the composer: their own menus, unchanged.

## Tests

- Bun: `browser-capture.test.ts` (RD-1 row: `bcPlayerZone` and `bcPillWidth` evaluated through `contractFn`, now in
  `contract-fn-fixture.ts` and shared with `usage-pr-pages.test.ts`; the pill and handle rows updated: the hover is the
  player's),
  `menu-keys.test.ts` (RD-5 row), `usage-pooled.test.ts` (the pointer takers: `browser-mini-player`).
- AppKit: `macos/tests/browser-automation` `testTheAgentCursorSitsOnItsTargetInTheStagesScaledHost` (RD-3; 4 failures
  on the branch tip's module), `macos/tests/contextmenu/text-menu.swift` (RD-4, 5 rows; the 2 through `install()` and
  `NSApp.sendEvent` fail on the first round's monitor).

Checks on the code head `2a45229d0` (review round, after the base merge `275e7849e`; all exit 0): `bun test
examples/t3-code` 4360 pass, 1 skip, 0 fail (295 files); strict `tsc`; `contract build` of `app.contract` (1341 lines);
`cargo test -p t3-code-macos --lib` 17 pass; AppKit `contextmenu` 24 run, 0 failed (`browser-automation` 26 pass in the
first round, unchanged since); `caps`; `cargo build --all-targets`, `cargo test` (3675 pass), `clippy -D warnings`,
`fmt --check`, `boot`.

## Delivery

Merged on 2026-10-10 as `3a88f3a9f` (#400, squash) after an independent review (one blocking item: the RD-4 monitor
returned the event it meant to take, so ExactKit's menu followed the shell's) and its repair round. Round 8 (#402) landed
on the base after that round's checks; the coordinator merged the base and ran again what it could reach: the Bun suite
(4,362 pass, 0 fail), caps, the bundle build (exit 0) and the AppKit `contextmenu` (24) and `browser-automation` (26)
binaries, 0 failures. The record stayed open for the real-input rows. The wider shell menu (unselected text, empty
areas, Copy Link, Copy Image, fields, the Browser panel) is [shell-context-menu](../20261010-shell-context-menu.md).

**Real-input results (realinput-1010f).** The steps ran in `realinput-1010f` on the bundle of `d057787cb` (lane
ri1010f-1, the lane's fake ACP agent) ([notes](https://raw.githubusercontent.com/ccheever/exact2/2f3a80555ea176b202a0e652e92c2dcbf790b352/realinput-1010f/F0-1010f-notes.txt)). New rows went to
[realinput-1010f-followups](../20261010-realinput-1010f-followups.md).

- RD-1 passed: one 8-step move onto the dot and a 1 s rest showed the pill (3 of 3). On Pop its tooltip showed after the
  delay and the pill stayed. Onto the page, or a quick exit from the dot, hid it
  ([arrival](https://raw.githubusercontent.com/ccheever/exact2/e016f039b370a65006669d66e29641d40c5e4a3d/realinput-1010f/F1-RD1-arrival-3of3.png)).
- RD-2 passed on positions: Toggle right panel twice put the player back where it was left; Open in right panel, then
  Toggle right panel, put it in the default corner. The sizes differ from this record's: the agent's tab is 1280 × 800,
  so the player is landscape, 320 × 200
  ([re-float](https://raw.githubusercontent.com/ccheever/exact2/3f828a1ab6c1543b4259bb97a20a3bb7a8d06d02/realinput-1010f/F1-RD2-refloat.png)).
  One oddity: a drag of the pill's padding by (-300, -200) moved the player only (-20, -200). Moved to RF-2.
- RD-3 passed: the tip sits on Go, the Name field and Page B, with a ping on each and no offset
  ([Go](https://raw.githubusercontent.com/ccheever/exact2/7550b1d5716aa16f7a68045eab68c92486c17632/realinput-1010f/F1-RD3-cursor-go.png)).
- RD-4 partial. Passed: Cut (dimmed), Copy, Paste (dimmed) and Select All with their shortcuts; no Look Up or Speech; no
  second menu after Escape; Copy and paste, and Select All, work; the sidebar row and the composer keep their own menus
  ([selection menu](https://raw.githubusercontent.com/ccheever/exact2/cf9b2f18bc6058fd2cd62da1ec049be981492bd1/realinput-1010f/F1-RD4-1-selection-menu.png)).
  Failed: the menu also has "Services ›". Moved to RF-1. A right-click on a link shows no menu at all; that is
  [shell-context-menu](../20261010-shell-context-menu.md) (#407).
- RD-5 needed no real input.

## Next action

Closed after `realinput-1010f`. RD-4's Services item is RF-1 and RD-2's drag offset is RF-2 in
[realinput-1010f-followups](../20261010-realinput-1010f-followups.md).
