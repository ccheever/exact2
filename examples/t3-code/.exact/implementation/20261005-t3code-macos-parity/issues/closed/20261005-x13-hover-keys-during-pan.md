---
name: 20261005-x13-hover-keys-during-pan
plan: 20261005-t3code-macos-parity
status: closed-not-reproduced
kind: framework-gap
blocks: [20261005-diff-review-engine, 20261005-floating-device-player, 20261005-legacy-sidebar, 20261005-live-automations-and-clones, 20261005-pr-code-tab, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-links-previews-and-routing, 20261005-provider-settings-upkeep, 20261005-round12-wrapup, 20261005-settings-scoped-controls-and-theme-editor, 20261005-terminal-layout, 20261005-upstream-timeline-and-markdown, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: null
reproduced_on: null
---

# X13: Hover and key events keep flowing while a pan gesture owns the press

## Summary
In T3 Code the user can press a row's Settle button and drag down the list; the rows in
between arm, a release applies the action, Escape cancels, and a hover card that was open
closes as the pointer leaves it. In exact2 a `pan` takes the press: the host sends no hover
enter/exit to other nodes and no key events reach the pan's section while it runs. The clone
fakes the three visible effects with state and an invisible shortcut button. exact2 needs
hover and key events to keep reaching the app during a pan, as a browser's document does.

## Why this issue arose

### The T3 Code behavior
- **Row-action sweep.** Pressing a thread row's action button (Settle, Un-settle, Wake) and
  moving the pointer more than 6 px starts a sweep; the rows between the pressed button and the
  pointer, in the same section, show the destination badge; release applies the action to the
  armed rows ("Settled 3 threads, ⌘Z to undo"); a press that never moved is the button's own
  click. Escape, window blur, `pointercancel`, `pagehide`, `resize` and a hidden document
  cancel the sweep and also swallow the release click.
  Evidence at `1e2ecbd975`: `apps/web/src/components/Sidebar.tsx:714-718` (`SIDEBAR_DRAG_DISTANCE = 6`),
  `:3482-3550` (`startActionSweep`, `resolveSidebarSweepKeys`), `Sidebar.pointer.ts:19-153`
  (`SidebarPointerSensor`: document-level `pointermove`, `pointerup`, `pointercancel` and
  `keydown` listeners, `if (event.code === "Escape") this.cancel()` at :115).
- **Hover card.** Sidebar rows carry a hover card (`ThreadHoverCardPopup`, `Sidebar.tsx:438`). When
  the pointer drags away from its trigger the browser fires leave events and the card closes;
  the sweep also hides the row's hover-only actions (`Sidebar.tsx:1804,1968`).
- **Tooltips on scroll (A17, `32b77f4fa3`).** A real scroll event in the timeline closes the last
  hovered tooltip unless its trigger holds keyboard focus (`apps/web/src/components/ui/tooltip.tsx`
  `TooltipScrollDismissArea`, `MessagesTimeline.tsx:1329-1331`). A wheel that does not scroll
  does nothing. Test: `MessagesTimeline.test.tsx` (the commit adds 125 lines).
- The same pattern recurs where a drag is in progress: the floating device player and the
  legacy sidebar's project drag (see the plan tickets below).

### What exact2 does today
- `EXACT2-GAPS.md` X13 (written from framework source at exact2 `c1522fdac`, checked against
  `main` `d2cb661eb`): "The host sends no hover events during a pan, and a section's local state
  gets no keys mid-pan. REF closes the hover card when a sweep starts and cancels the sweep on
  Escape. r12 added an Escape workaround; hover after a cancelled sweep is still wrong."
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**.
- Observed in the clone: the clone's round-11 documents list the gap (`README.md` "Known in-app
  differences": the card or tooltip open at the press stays until release and Escape does not
  cancel; `AGENT-HANDOFF.md` step 8 of the real-input list, "never run; agent pass"). Code
  reading of the r12-sidebar change (not driven, no real-input run exists): see below.

### Where the clone hits it
`sidebar.contract` ThreadSection (mc-orch tree, 2026-10-05): `sweepKeys`, `sweepCancelled`,
`sweepHover` (lines 17-61) and an invisible 1×1 button with `aria-keyshortcuts="Escape"` that
exists only while a sweep is armed, because a window shortcut is the only key path a pan does
not block (`sidebar.contract:59-61`, `cancelSweep`). While the sweep is live the row hover id is
forced empty (`hoverId=(sweepLive ? "" : …)`, :58), so the card hides; after a cancel the hover id
is derived from the pointer's y and the rows' frames (`sweepHover`, :26) instead of host events.
`r11-upstream-sweep.ts` handles the release; `r12-sidebar.test.ts:50` tests the Escape path.
Differences a user can see: a card or tooltip that is not a sidebar row (a header tip, a
popover) still ignores the drag; the hover after a cancel is computed from layout, not from the
pointer, so it can disagree with where the pointer really is; and the invisible button is a
second Escape handler that competes with other window shortcuts while armed.

## Why it must be resolved
The goal is to clone T3 Code completely, and the sweep is a core sidebar gesture. Four plan items
carry the difference or its workaround: the row-action sweep rows in `20261005-round12-wrapup`
(the r12-sidebar lane), the A17 scroll-dismiss drive in `20261005-upstream-timeline-and-markdown`,
the project-drag Escape in `20261005-legacy-sidebar`, and the "keep the pill visible while
dragging" state in `20261005-floating-device-player`. Hover-revealed controls in
`20261005-pr-handoffs-and-quick-actions`, `20261005-pr-links-previews-and-routing`,
`20261005-pr-code-tab` and `20261005-diff-review-engine` rely on correct hover state after a
gesture. Keeping the workaround costs a window-shortcut hack per gesture (it breaks if the
shortcut is bound elsewhere), frame-derived hover that duplicates the host's job, and a recurring
"declared difference" in every pair run. A declared deviation is not an end state.

## Requested support
The web way: during a pointer drag the document keeps receiving `pointermove`, `keydown` and
(without pointer capture) `pointerenter/leave`, and `:hover` updates.
- **A (preferred).** While a `pan`/drag gesture runs on macOS: (1) deliver hover enter/exit to the
  nodes under the pointer (a node that was hovered when the press started receives its exit when
  the pointer leaves it); (2) route key events to the nearest ancestor `key=` handler of the pan
  target, or to a window-level handler, instead of dropping them; (3) a `pan` ends with a
  cancel signal when Escape is pressed, the window resigns key, or the app loses focus.
- **B (smaller).** Only (3): a documented `pancancel` event on Escape/blur/resign, plus
  hover exit for the pan's own origin node. Fixes Escape and the open card, not general hover.
Trade-off: A changes event delivery for every pan consumer (needs a conformance case so existing
gestures keep working); B is local but leaves hover wrong for other nodes during a drag.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: a column of five rows; each
row has `hover=` that sets a highlighted id and a small button with `pan=` and `panrelease=`;
the column has `key=` that records `Escape`. Steps: (1) hover row 2 (highlight); (2) press the
row-2 button and drag over rows 3–4; (3) press Escape; (4) release. Expected (Chrome with the
same DOM): row 2's highlight clears when the pointer leaves it, rows 3–4 highlight in turn,
the key handler logs Escape, the drag ends as cancelled. Actual per `EXACT2-GAPS.md`: no hover
change, no key. Clone scenario: `AGENT-HANDOFF.md` real-input step 8 (five fixture threads, open a
hover card, press Settle, drag, Escape); the agent cannot send a real drag (see X8), so this is an
attended session or an AppKit test with constructed `mouseDown`/`mouseDragged`/`keyDown` events.

## Acceptance for the fix
- AppKit test: hold a pan with constructed events; assert the `hover=` handler of the node under
  the pointer runs on move and the origin's exit runs on leave; assert an ancestor `key=` handler
  receives Escape; assert the pan ends cancelled.
- Conformance case against Chrome for the same DOM (events and order).
- Clone: the r12-sidebar sweep drive with a real trackpad (attended session): card closes at the
  start, Escape cancels with no change, the hover after cancel matches the pointer.

## App adoption after resolution
Remove the invisible Escape button, `sweepCancelled`/`sweepHover`/`sweepLive` hover overrides
and `cancelSweep` from `sidebar.contract`; delete the frame-derived hover; replace the
floating-player drag state and the legacy-sidebar Escape workaround with native events. Re-run
`r12-sidebar.test.ts`, the real-input step 8 and the A17 drive. `issue-close` checks that these
rows pass and that no workaround remains in `sidebar.contract`.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's
approval; publication only after approval).

## Upstream (2026-10-08)

Upstream: not reproduced on main `0365ad1a4` as described, closed (2026-10-08); not filed. One-file app: five rows with `hover`, each with a `pan`/`panrelease` handle, and `key` on their column. Keys: an Escape typed while the pan holds the press reaches the column's `key` handler on macOS (`key view 2 (keyed)` between the pan moves and `panrelease`). Hover: during the pan no other row gets hover on macOS. The web host pans with `setPointerCapture` (`host/web/input-glue.js:267-324`): it clears the origin row's hover at the press, and no row is hovered until release. So neither host delivers hover to other nodes during a pan, which is pointer capture's rule. Differences left, not filed: macOS keeps the origin row hovered through the pan, and neither host re-hovers the row under the pointer at release until the pointer moves.
