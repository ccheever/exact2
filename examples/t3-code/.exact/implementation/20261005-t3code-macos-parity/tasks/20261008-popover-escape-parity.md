---
name: 20261008-popover-escape-parity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-popover-escape-parity
pr_url: https://github.com/ccheever/exact2/pull/290
verified_commit: a579d01e4
---

# A pinned usage popover closes on an outside press; Escape in the theme editor behaves as the reference

## Outcome

The user decided on 2026-10-08 that both behave as the reference T3 Code (`1e2ecbd975`):

1. **Usage segment popover (#263, `20261005-usage-pooled-view`).** A popover pinned by a press closes on a
   press outside it (light dismiss). #263 had declared this a difference against the local draft X53 only.
2. **Theme editor Escape.** Escape no longer closes the whole editor (built by
   `20261005-settings-scoped-controls-and-theme-editor`, D16; kept "provisional, user decision pending" by
   `20261007-theme-color-picker`). Escape in the colour popover (#252) still closes only the popover and gives
   the focus back to its swatch.
3. **The real-input batch's #263 rows** (PR #298, items 10-12; coordinator, 2026-10-08): (12) after Escape in the
   "Use a reset credit?" confirm the focus did not go back to the segment: fixed here. (10) the hover popover
   closes as the pointer moves into it and (11) on the second card a click pins the previously clicked
   segment's popover are hover, and the coordinator moved them to the fix-hover-cards PR (its HoverLayer helper
   owns hover grace for every clone popover); this task's diagnosis and base repro for them are under
   "Moved to fix-hover-cards".

## Reference behavior

- `UsageLimitsPooled.tsx` PoolSegment is a Base UI `Popover` (non-modal) whose triggers are the segment and its
  LegendRow (`PopoverTrigger openOnHover`). Base UI 1.5.0 (`@base-ui/react`, read from the npm tarball):
  `PopoverRoot` dismisses with `useDismiss(…, { outsidePressEvent: { mouse: 'intentional', touch: 'sloppy' } })`.
  `useDismiss` closes on a press whose target is outside the popup, outside the popover's own triggers
  (`triggerElements`) and outside its child floating elements, and not on a scrollbar; "intentional" means the
  `click` (a primary press that ended), and a press that starts inside and ends outside is not one. A press on
  another segment is outside this popover (a different Root), so it closes and that segment's click opens its
  own. A press on the popover's own trigger is `useClick`'s: `stickIfOpen` keeps a hover-opened popover open on
  the first press and the next press closes it. After any close, hover opens it again only on a new mouseenter
  (`useHoverReferenceInteraction` sets `blockMouseMove` on every close and clears it on mouseenter). A press
  that ends inside the popup does not dismiss (`insideReactTree`). A right-click is no click: it does not
  dismiss.
- `ThemeEditorPanel.tsx` is a plain `role="dialog"` div with no Escape of its own: its only keydown listener
  cancels Inspect (not built here, X30). An Escape in it reaches the page's `useEscapeToGoBack`
  (`routes/settings.tsx`: `navigateToMainApp`; Usage and pull requests: back), and the editor, mounted above the
  router (`ThemeEditorHost`), stays; `useEscapeToGoBack` blurs the focused element first. The colour popover
  (Base UI `Popover`) closes on Escape, prevents it (so Settings' Escape-to-go-back skips it) and returns the
  focus to its trigger.
- The reset confirm is a Base UI AlertDialog (`ResetCreditDialog`): closing it (Escape, Cancel, the confirm)
  gives the focus back to the element that had it before (the segment, once "Use reset" left with its popover).

## Implementation

- **Page presses** (`pages-usage.contract` UsagePage): the page's root takes `pointerdown=pressDown
  pointerup=pressUp`. `pressDown(e)` notes whether a primary press (`e.buttons == 1`) starts outside the shown
  pinned popover's card (`usage-seg-pop-<id>`), its segment (`usage-seg-<id>`) and its legend row
  (`usage-legend-<id>`), by `frame()` and the event's `clientX`/`clientY` (`fn usageHit`); `pressUp(e)` closes it
  unless the press ends in the card, before the press's own action (DOM's order), so a press on another
  segment pins that one. The pin (`pin`) and the dismissal (`pressDown`, `pressUp`) touch only the pin, never a
  hover state, so the hover helper (fix-hover-cards) owns how a hover-shown popover closes ("Moved to fix-hover-cards").
  `usage-pooled.contract` gives the card and the legend row their ids.
- **Presses elsewhere in the window** (`app-window.contract` T3Window): the window's root counts primary presses
  that end (`outsidePressDown` / `outsidePressUp`, `outsidePresses`), passed through PagesCover to UsagePage as
  `outside`; a pin holds only while the count is the one it was made at (`pinnedAt`, `derive held`). So the
  sidebar, a toast and the floating theme editor all count, as Base UI's document listener does. The theme
  editor's swatch, plane and hue slider take the pointer themselves (they hear `pointerdown`), so they hand
  their presses on (`outsideDown` / `outsideUp` props, `theme-color-picker.contract`). A press in a text field
  reaches no `pointerdown` on macOS, so the editor's Theme name, filter and hex fields count their `focus`
  (`outsideFocus`).
- **The page's ground** (`usage-ground`): a full-height column under the scroll's content. On macOS a press on a
  scroll view's empty area (below or beside its content) reaches no node, so no `pointerdown` hears it (found in
  session 1; local draft [X65](../issues/20261008-x65-scroll-empty-area-press.md), not published).
- **The reset confirm's focus (item 12)** (`app-main.contract` PagesCover, `pages-usage.contract`): the confirm's
  buttons (Cancel, its Escape, Use credit) go through PagesCover's `usageConfirm`, which closes it for the page
  at once (`usageClosing`, `derive usageConfirmOpen`); UsagePage is inert while `confirmOpen`, not while the
  usage answer still names the confirm. So the page is no longer inert when the command's answer asks for the
  segment's focus (`focus:usage-seg-…`, `commandCompleted`). Cause: the usage answer that drops the confirm
  comes after the command's, so under real time the focus went to a still-inert page and was refused; the
  agent clock settles both together, which is why #263's agent drive passed. A new ask (UsagePage's `redeem`
  through `usageCommand`) opens it again. No root line (`app.contract` 1,478, the base's).
- **Theme editor** (`settings-appearance-editor.contract`): the close button drops `aria-keyshortcuts="Escape"`;
  the editor has no other key handler and is not `aria-modal`, so Escape anywhere in it reaches the page under
  it (Settings' Back: Settings closes, the editor stays). Escape on a page with no Escape of its own (a thread)
  does nothing, as the reference's. Settings' Back (`settings-core.contract` SettingsNav `leave`) and the Usage
  page's Escape back blur the focus first, as `useEscapeToGoBack` does, so the editor's field keeps none.

`app.contract` is unchanged (1,478 lines on the merged base `ec32c8c37`).

## Acceptance

| Row | Result | Proof |
| --- | --- | --- |
| A primary press on the page outside the pinned popover and its triggers closes it (empty ground, a card's body, narrow card text) | pass: agent (macOS, sessions 2, 6 and 7, the last on the final code) and real input (session R1, on the build before `pin`/`pressUp` dropped their hover lines; the pointer had left the segment, so those lines did nothing there). The final build's real retry (R4) did not reach the segment: another lane's macOS permission prompt sat over it. The real press on the final build is **deferred to the next real-input batch** (`STATUS.md` "Next real-input batch") | images 01, 10, 15; drive record; real-input record |
| A press outside the page closes it: the sidebar, a toast, the floating theme editor, the editor's swatch | pass (agent, macOS; the sidebar again in session 7) | images 02, 08, 09; drive record |
| The same with a real pointer on the sidebar | **moved to fix-hover-cards** (coordinator, 2026-10-08): in R1 the pin dropped (the window counted the press), but #263's hover state stayed set after the real click on the segment and kept the popover shown; diagnosis under "Moved to fix-hover-cards" | real-input record |
| Presses that do not count: inside the card; a right-click; another segment (pins that one instead); Escape closes it and the segment keeps the focus | pass (agent, macOS, sessions 2, 6 and 7) | drive record |
| A press on the pinned segment closes it | pass (agent, the pin). With a real pointer resting on the segment the popover stays shown by #263's hover state (base: image 07): **moved to fix-hover-cards**; diagnosis under "Moved to fix-hover-cards" | drive record |
| A press on the same account's legend row (narrow) closes it | not passing, pre-existing (#263's anchoring): **moved to fix-hover-cards**; diagnosis under "Moved to fix-hover-cards" | drive record "Not passing" |
| A press in the floating editor's text fields counts as outside (a macOS field fires no `pointerdown`, read in the host's code) | implemented (their `focus` counts); unit-tested. The real press is **deferred to the next real-input batch** (`STATUS.md` "Next real-input batch") | tests |
| A press that starts outside and ends inside the card does not dismiss (Base UI's insideReactTree) | implemented (review N1); unit-tested; the agent cannot drag across, so it is **deferred to the next real-input batch** (the page-press row's last step) | tests |
| Item 12: Escape in the reset confirm gives the segment the focus | pass: agent (session 7) and real keys (session R2: Tab, Tab to "Use reset", Return, Escape: the confirm closes, the 5h Work segment shows the ring, the next Tab goes on from it); agent (the base passes there too) | real-input record, images 13-14 |
| Items 10, 11 (hover) | moved to fix-hover-cards (coordinator, 2026-10-08) | "Moved to fix-hover-cards" |
| Escape in the theme editor never closes it; it reaches the page under it (Settings closes, the editor stays), also with Settings reopened over the editor | pass (agent, macOS, sessions 2, 6 and 7); real keys on the final build (R3): the first case passes (Escape leaves Settings, the editor stays; a second Escape changes nothing) | images 03, 04, 16; real-input record |
| That Escape blurs the editor's field, as `useEscapeToGoBack` does | pass (agent, sessions 6 and 7: no focus after the first Escape); real keys (R3): the Theme name field's ring goes with the first Escape | image 16; drive record |
| Escape in the colour popover closes only the popover and focuses its swatch | pass (agent, macOS), unchanged; real keys: the batch's #252 row 9 (PR #298) | image 05 |
| Regression tests that fail on the base | pass: 6 new tests fail on the feature branch's sources (`ec32c8c37`) and pass here; 1 guards the unchanged colour-popover Escape | `usage-pooled.test.ts` "light dismiss of a pinned segment popover (popover-escape-parity)" (3), "the reset confirm gives the segment the focus back (#298 item 12)" (1); `theme-color-picker.test.ts` "Escape in the theme editor (popover-escape-parity)" (3) |
| Checks | pass | Attempts; PR "Checks" |

## Progress

2026-10-08: implemented on `feat(example)/t3-code-popover-escape-parity` (from `07dcef1ab`). Two agent sessions
(one retry) and the base session; session 1 found the scroll-ground and the doubled own-segment press (under
a toast), both fixed before session 2. An independent review (below) found one blocking issue and several
others; all fixed in attempt 3 or recorded. `closed/20261007-theme-color-picker.md` still calls the editor's Escape
"provisional, user decision pending"; the user decided it here (records sync is the coordinator's).

Later on 2026-10-08 the coordinator added the real-input batch's #263 rows 10-12 (PR #298). Attempt 5 built a
hover model for 10 and 11 (a hold taken by what is front-most under the page's pointer moves, a root rest timer
and a leave grace); a third review round found no blocking issue. The coordinator then moved hover (10, 11) to
fix-hover-cards and asked that the pin and the outside press stay independent of hover: attempt 6 drops the
hover model and keeps the light dismiss, the editor's Escape and item 12. The feature branch was merged three
times (`c0475fbaa`, `ec32c8c37`, `fa46ad5d0`). Agent session 7 drove every row on the final code (the same
read-backs as session 6); real-input sessions R1-R4 are in the real-input record.

## Moved to fix-hover-cards (coordinator, 2026-10-08)

The coordinator moved hover grace for every clone popover to the fix-hover-cards PR (a window-level HoverLayer),
and with it every row here whose cause is hover. The coordinator forwards this diagnosis:
- **Item 10** (the hover popover closes as the pointer moves into it): a segment leave cleared the hover at
  once, the popover went inert before the pointer reached it, and the host's hover follow after that batch
  (`followPointer`: a style change re-hit-tests) dropped it; the popup drawn outside its trigger's box also gets
  no tracking area (fix-hover-cards' own probe). Base UI: `restMs` 300, `handleClose: safePolygon()`, popup hover.
- **Item 11** (on the second card a click pins the previously clicked segment's popover): hover, not the pin.
  #263's hover popover is logically open as soon as the pointer enters a segment (only its fade waits 300 ms),
  so a still-transparent popover hanging over the next card's bar takes the click and its hover. Agent repro on
  the base (`07dcef1ab`): `tap usage-seg-0-0-1 hover`, `clock +100`, `tap usage-seg-0-1-1 hover`,
  `tap usage-seg-0-1-1`: Weekly Work is not pinned and 5h Work's popover shows ([image, left half]
  (https://raw.githubusercontent.com/ccheever/exact2/d9d66b36e8c99d3a16d300bd1d248a192f7af738/popover-escape-parity/11-pass-over-then-click.png)).
  Base UI mounts nothing before the rest delay.
- **A real sidebar press** (R1): the press dropped the pin (`T3Window` counted it and `held` went empty), but
  the popover stayed shown. `shown` falls back to #263's hover states (`overSeg`/`overPop`/`overMail`), and
  `overSeg` was still set after the real click on the segment and the move away (why the leave did not clear
  it was not traced). In Base UI an outside press closes the popover however it opened (`useDismiss`), so
  the hover helper should drop the hover-open state on an outside press as well. This task's pin code touches
  no hover state.
- **A press on the pinned segment with the pointer resting on it** (base: image 07): `pin` toggles the pin
  off, but `overSeg` is still set by the resting pointer, so the hover keeps the popover shown. In Base UI,
  any close sets `blockMouseMove` until a new mouseenter (`useHoverReferenceInteraction`), so a resting
  pointer does not reopen it. The helper should block hover after a close until the next enter.
- **A press on the same account's legend row, narrow layout**: #263 anchors the popover below its segment,
  and on the narrow layout it covers the upper half of the legend row under the bar. So the press lands in
  the popover's card, which by design does not dismiss (Base UI's `insideReactTree`). Base UI positions the
  popup from the trigger that opened it with collision handling, which leaves the other trigger visible.
  fix-hover-cards moves the popovers into `UsagePopups`, and the fix belongs there.

## Host findings and unconfirmed observations

Checked on 2026-10-08 against the filed issues #266–#302, an upstream search (`pointerdown`, scroll presses,
text-field pointer) and main `2531fb826`:
- **A press on a scroll view's empty area reaches no node** (macOS): reproduced in this task's agent session 1
  (a real `NSEvent` through `NSApp.sendEvent`), not filed, unchanged on main by reading. Local draft
  [X65](../issues/20261008-x65-scroll-empty-area-press.md).
- **A press in a text field never fires `pointerdown`** (macOS): unconfirmed, from reading the host's code
  (`TextAreaMac.swift` `Field`, `FieldEditor`, `TextArea`, and `NativeFieldsMac.swift` on main, never call
  `pointerPressed`). Not reproduced, because the live-session budget was spent, so there is no draft. The
  next real-input batch's editor-field row exercises the workaround.
- **A native context menu keeps the pointer held** (macOS): unconfirmed, from reading the code (review S1).
  `rightMouseDown` calls `pointerPressed`, and the NSMenu takes the secondary button's up, so `pointerHeld`
  stays set until the next primary up. No draft.
- **Free `pointermove` stopped after a hover handler called a root action** (attempt 5's abandoned hover
  model): unconfirmed. On the normal macOS app the page's `pointermove` stopped once the pointer entered a
  segment whose `hover` handler called an action prop bound to a root action (`usageEdge`); with that call
  removed, moves kept coming. Not traced to the host; worth a check for any helper whose hover handler calls a
  root or injected action. No draft.

## Independent review (2026-10-08, a separate agent, on `e5d0ce63a`)

Given the diff, this record, the drive record, the reference and Base UI 1.5.0 sources. Findings and handling:

| Finding | Severity | Handling |
| --- | --- | --- |
| B1: the segment's `pointermove` reopened a popover the user had just closed (by a press or Escape) as soon as the pointer moved; Base UI blocks hover until a new mouseenter (`blockMouseMove`); also a regression for Escape under a resting pointer | blocking | fixed in attempt 3: no `pointermove` on the segment; the "Reference behavior" text corrected |
| S1: a right-click on a `contextPopover` node now goes through the window root's pointer hold; the NSMenu takes the up, so the hold stays until the next primary up (read, not run) | should-fix | host behavior (framework left alone): recorded in `EXACT2-GAPS.md`; real-input step 5 checks it |
| S2: a press in a text field reaches no `pointerdown` on macOS | should-fix | fixed for the fields that can sit over the Usage page (the theme editor's): their `focus` counts; recorded as a host finding |
| S3: records inaccurate (GAPS "nothing differs", the colour-picker record's pending decision, checks not listed, `pr_url` null, the Base UI reading) | should-fix | fixed here and in `EXACT2-GAPS.md`; the colour-picker record noted in Progress (not this task's record) |
| N1: a press ending inside the card dismissed | nit | fixed: `pressUp(e)` also checks the release point |
| N2: a press in the 6-pt gap unpinned but the hover kept it shown | nit | fixed: a dismissal clears the hover states |
| N3: Escape left the editor's field focused | nit | fixed: Settings' Back and the Usage page's back blur first |
| N4: cost per click of the root handlers not measured | nit | not measured (two actions per press, one counter slot) |
| N5: a history row of #263's record was edited | nit | reverted; the note is in its Next action only |
| N6: images 08-10 are after-only | nit | they show new behavior; the base was not driven for those steps (01 and 02 show the base keeping the popover pinned on the page ground and the sidebar) |
| N7: no drive of a colour plane or hue drag since the change | nit | real-input step 4 drags the plane |
| Second round (on `e6ceb5efd`): no blocking finding; B1 resolved. Should-fix: a dismissal cleared every hover state, so a press in another segment's hover-shown popover (Use reset, the email) closed that one too; the "another segment" and "own segment" rows ran before presses on segments went through the page | should-fix | fixed in attempt 4: only the dismissed popover's states are cleared (which also keeps segment B unrestyled between the up and its press, X56); the rows marked as driven on `e5d0ce63a`, real-input step 3 adds "A pinned, pointer on B, click B" |
| Second round nits: the release check also exempted the triggers (Base UI exempts only the popup); a focus no press causes counts (a window refocus on the web); Escape leaves Settings only while the sidebar is open (SettingsNav renders under `when data.sidebarOpen`, pre-existing); a keyboard-pinned popover does not close when focus leaves it (Base UI's closeOnFocusOut, pre-existing from #263); record wording | nit | release check fixed (card only); the rest recorded here as found, not in this task's scope |

### Round 3 (2026-10-08, on attempt 5's staged hover model)

No blocking finding. Should-fix: a hover hold could go stale when the pointer left the page within one frame
(no page move outside); a segment under an open card could take the hover on macOS (tracking areas are
geometric); the records' placeholders and two "pass (agent)" rows that the agent cannot tell from the base.
All moot after the scope change (the hover model is gone; its notes are under "Moved to fix-hover-cards"); the records were
rewritten for attempt 6.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | uncommitted, before the ground | agent session 1 (pid 3041): the press under the cards did not dismiss (no `pointerdown` journalled); a second press on segment 0-0-1 kept it open (the segment lay under a toast) | drive record session 1 | fixed in 2 |
| 2 (2026-10-08) | `e5d0ce63a` | `bun test examples/t3-code` 3,041 pass / 1 skip / 0 fail (base 3,036); strict `tsc` clean; contract build 3,850 slots (base 3,844); `cargo test -p t3-code-macos --lib` 11 pass; five checks (run 13:40-13:55 KST before the commit, logs in the worktree's `target/pep/checks`, not committed): build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok; agent session 2 (pid 75713) and base session (pid 98359): rows above | images 01-10, drive record | review findings |
| 3 (2026-10-08) | `e6ceb5efd` (review fixes B1, S2, N1-N3, records) | `bun test examples/t3-code` 3,042 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,850 slots; `app.contract` 1,456 lines (unchanged); `cargo test -p t3-code-macos --lib` 11 pass; macOS dev and `--bundle` builds; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok; the 5 new tests fail on `07dcef1ab`'s sources (1 guard passes) | tests | real-input rows |
| 4 (2026-10-08) | `6e1636d7c` (second-round fixes: own hover states only; release check on the card) | `bun test examples/t3-code` 3,042 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,850 slots; `cargo test -p t3-code-macos --lib` 11 pass; macOS `--bundle` builds; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok | tests | real-input rows |
| 5 (2026-10-08) | uncommitted (merge of `c0475fbaa`; items 10-12 with a hover model) | agent sessions 3-4: rest, bridge, move-then-click and the confirm's focus passed; real pointer (lane copy): the page's free `pointermove` stopped after the pointer entered a segment (see "Moved to fix-hover-cards"); scope change | `t3-code-evidence:popover-escape-parity/11-*`, `12-*` (moved to fix-hover-cards) | dropped |
| 6 (2026-10-08) | merge of `ec32c8c37`; pin and dismissal independent of hover; item 12 in PagesCover | `bun test examples/t3-code` 3,140 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,938 slots; `app.contract` 1,478 lines (the base's); `cargo test -p t3-code-macos --lib` 13 pass; macOS dev and `--bundle` builds; five checks: build ok, test 3,521 pass / 0 fail / 34 ignored, clippy ok, fmt ok, caps within, boot ok; agent session 6 (pid 82941): every row above; real input R1, R2 | images 13-14, real-input record | the last edit to `pin`/`pressUp` |
| 7 (2026-10-08) | `cae284e86` (pin and dismissal touch no hover state; item 12 committed), merge of `fa46ad5d0` (`a579d01e4`) | `bun test examples/t3-code` 3,142 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,938 slots; `app.contract` 1,478 lines (the base's); `cargo test -p t3-code-macos --lib` 13 pass; macOS dev build; `git add -A` + caps within; five checks: build ok, test 3,521 pass / 0 fail / 34 ignored, clippy ok, fmt ok, caps within, boot ok; agent session 7 (pid 54085): every row, as session 6; real input R3 (final build: the editor's Escape passes), R4 (blocked by another lane's permission prompt) | images 15-16, drive record session 7, real-input record | the rows in "Next action" |

## Next action

Review the draft PR (#290); the coordinator flips it to ready. Every not-done row now has an owner:
- **Deferred to the next real-input batch** (`STATUS.md` "Next real-input batch", with steps): a real page press
  on the final build (with a press that ends inside the card), and a press in the floating theme editor's text
  fields.
- **Moved to fix-hover-cards** (diagnosis above): the real sidebar press, the pinned-segment press with the
  pointer resting on it, and the narrow legend-row press. fix-hover-cards merges the base after #290 and takes
  over the hover lines of the usage files. The coordinator relays the list of lines this task changed.
- **Local draft X65**: a press on a scroll view's empty area. The workaround (`usage-ground`) stays until the
  host routes the press.

## Real-input record (2026-10-08)

Lane copy "T3 Code (Lane Popover)" (bundle id `com.exact.t3code.macos.lane-pep`, the `--bundle` build of this
branch, re-signed), launched by path with `CFFIXED_USER_HOME=target/pep/lane/real/home` (no local server), paired
to the lane's Build box (fixture proxy 16591; the link pasted with `orca computer paste-text`, the clipboard
cleared after). Pointer: `cliclick` (real CGEvent moves and clicks, window-local points + the window's origin);
keys: `orca computer press-key` to the lane copy's pid. Read back from window screenshots
(`screencapture -l`). Each session held the shared real-input lock and released it at once.

| Session (UTC) | Build | Rows and outcome |
| --- | --- | --- |
| R1 (07:24, pid 25789) | before `pin`/`pressUp` dropped their hover lines; item 12's fix in | Page press: pin 5h Codex by a click, the pointer taken round the card, a click on the page ground below the cards: closed (image 15). Sidebar press: the pin dropped (the window counted it), but #263's hover state for the segment, left set by the real click, kept the popover shown: hover, moved to fix-hover-cards. Keys sent with `cliclick kp` did not reach the app, so its Escape rows did not run |
| R2 (07:26, pid 34704) | same as R1 | Item 12, keys through `orca computer press-key`: pin 5h Work, Tab, Tab to "Use reset", Return (the confirm), Escape: the confirm closes, the 5h Work segment shows the focus ring and the next Tab goes on from it (images 13, 14). The editor rows: the click meant for Settings hit the Usage page's Back, so they did not run |
| R3 (07:52, pid 51056) | final (the lane bundle built from this branch's code after the last edit) | Theme editor: Settings, Appearance, Create theme, a click in Theme name (ring), Escape: Settings closes, the editor stays, the field has no ring; a second Escape changes nothing (image 16). The window then lost activation (grey traffic lights) before the click on the Usage gauge, so the outside-press rows did not run |
| R4 (07:57, pid 76996) | final | Retry of the outside-press rows only. Usage opened; the clicks on the 5h Codex segment landed on another lane's macOS permission prompt ("T3 Code (Lane FPAS)" asking for the Documents folder; a `UserNotificationCenter` window over screen 546,160, 260x208), so nothing was pinned and the later presses had nothing to close. The prompt is another lane's and was left unanswered |

Each session took the shared real-input lock while it was absent and released it at once. The brief allows one
live session and one retry: R3 was the final session the coordinator asked for and R4 its retry, so the real
page press on the final build is deferred to the next real-input batch and the real sidebar press moved to
fix-hover-cards (see "Next action").

