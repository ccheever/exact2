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
verified_commit: null
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

## Implementation

- **Page presses** (`pages-usage.contract` UsagePage): the page's root takes `pointerdown=pressDown
  pointerup=pressUp`. `pressDown(e)` notes whether a primary press (`e.buttons == 1`) starts outside the shown
  pinned popover's card (`usage-seg-pop-<id>`), its segment (`usage-seg-<id>`) and its legend row
  (`usage-legend-<id>`), by `frame()` and the event's `clientX`/`clientY` (`fn usageHit`); `pressUp(e)` closes it
  unless the press ends in the card, before the press's own action (DOM's order), so a press on another
  segment pins that one. Its own hover states go with it (the 6-pt gap under the card is the hover
  wrapper's); another segment's hover-shown popover stays.
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
  session 1; `EXACT2-GAPS.md` "Host finding", not filed).
- **The popover's own segment**: a press on the pinned segment closes it and clears the hover states, so a
  pointer resting on the segment no longer holds it open (base: image 07); hover opens it again on the next
  enter, as Base UI's `blockMouseMove` does.
- **Theme editor** (`settings-appearance-editor.contract`): the close button drops `aria-keyshortcuts="Escape"`;
  the editor has no other key handler and is not `aria-modal`, so Escape anywhere in it reaches the page under
  it (Settings' Back: Settings closes, the editor stays). Escape on a page with no Escape of its own (a thread)
  does nothing, as the reference's. Settings' Back (`settings-core.contract` SettingsNav `leave`) and the Usage
  page's Escape back blur the focus first, as `useEscapeToGoBack` does, so the editor's field keeps none.

`app.contract` is unchanged (1,456 lines).

## Acceptance

| Row | Result | Proof |
| --- | --- | --- |
| A primary press on the page outside the pinned popover and its triggers closes it (empty ground, a card's body, narrow card text) | pass (agent, macOS) | images 01, 10; drive record session 2 |
| A press outside the page closes it: the sidebar, a toast, the floating theme editor, the editor's swatch | pass (agent, macOS) | images 02, 08, 09; drive record |
| Presses that do not count: inside the card; a right-click; another segment (pins that one instead); Escape closes it and the segment keeps the focus | pass (agent, macOS) on `e5d0ce63a`, where a segment held its own presses; since `e6ceb5efd` a press on a segment goes through the page's handlers: unit-tested, real-input step 3 | drive record |
| A press on the pinned segment closes it, also with the pointer resting on it | pass (agent, without hover, on `e5d0ce63a`); with a real resting pointer and the present path: real-input step 3 | drive record; base with hover: image 07 |
| A press on the same account's legend row (narrow) closes it | not passing, pre-existing (#263's layout, not this task's dismissal): the popover hangs off the segment and covers the legend row's upper half, so the press lands in the popover; the reference anchors the popup to the trigger that opened it. Follow-up for #263's owner (popover anchoring) | drive record "Not passing" |
| A press in the floating editor's text fields closes it (macOS fields reach no `pointerdown`) | implemented after the drives (their `focus` counts); unit-tested, not driven | tests; real-input step 3 |
| A press that ends inside the card, or in the 6-pt gap, behaves as Base UI's; a press in another segment's hover-shown popover keeps that one | implemented after the drives (independent review N1, N2, second round); unit-tested, not driven | tests; real-input step 3 |
| Escape in the theme editor never closes it; it reaches the page under it (Settings closes, the editor stays), also with Settings reopened over the editor | pass (agent, macOS) | images 03, 04 |
| That Escape blurs the editor's field, as `useEscapeToGoBack` does | implemented after the drives (review N3); unit-tested, not driven (image 03 shows the field still focused) | tests; real-input step 4 |
| Escape in the colour popover closes only the popover and focuses its swatch | pass (agent, macOS), unchanged | image 05 |
| Regression tests that fail on the base | pass: 5 new tests fail on `07dcef1ab`'s sources and pass here; 1 guards the unchanged colour-popover Escape | `usage-pooled.test.ts` "light dismiss of a pinned segment popover (popover-escape-parity)" (3), `theme-color-picker.test.ts` "Escape in the theme editor (popover-escape-parity)" (3) |
| Real outside press (real input, lock) | pending | Real-input batch steps |
| Real Escape in the theme editor (real input, lock) | pending | Real-input batch steps |
| Checks | pass | Attempts 2-4; PR "Checks" |

## Progress

2026-10-08: implemented on `feat(example)/t3-code-popover-escape-parity` (from `07dcef1ab`). Two agent sessions
(one retry) and the base session; session 1 found the scroll-ground and the doubled own-segment press (under
a toast), both fixed before session 2. An independent review (below) found one blocking issue and several
others; all fixed in attempt 3 or recorded. `20261007-theme-color-picker.md` still calls the editor's Escape
"provisional, user decision pending"; the user decided it here (records sync is the coordinator's).

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

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | uncommitted, before the ground | agent session 1 (pid 3041): the press under the cards did not dismiss (no `pointerdown` journalled); a second press on segment 0-0-1 kept it open (the segment lay under a toast) | drive record session 1 | fixed in 2 |
| 2 (2026-10-08) | `e5d0ce63a` | `bun test examples/t3-code` 3,041 pass / 1 skip / 0 fail (base 3,036); strict `tsc` clean; contract build 3,850 slots (base 3,844); `cargo test -p t3-code-macos --lib` 11 pass; five checks (run 13:40-13:55 KST before the commit, logs in the worktree's `target/pep/checks`, not committed): build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok; agent session 2 (pid 75713) and base session (pid 98359): rows above | images 01-10, drive record | review findings |
| 3 (2026-10-08) | `e6ceb5efd` (review fixes B1, S2, N1-N3, records) | `bun test examples/t3-code` 3,042 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,850 slots; `app.contract` 1,456 lines (unchanged); `cargo test -p t3-code-macos --lib` 11 pass; macOS dev and `--bundle` builds; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok; the 5 new tests fail on `07dcef1ab`'s sources (1 guard passes) | tests | real-input rows |
| 4 (2026-10-08) | second-round fixes (own hover states only; release check on the card) | `bun test examples/t3-code` 3,042 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,850 slots; `cargo test -p t3-code-macos --lib` 11 pass; macOS `--bundle` builds; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored, clippy ok, fmt ok, caps within, boot ok | tests | real-input rows |

## Next action

Review the draft PR (#290). Run the real-input steps below when the shared lock is free; they also cover the
review fixes that were not driven (session budget). Follow-up for #263's owner: anchor the segment popover to
the trigger that opened it, so a narrow legend row is not covered by its own popover.

## Real-input batch steps

Lock `target/t3-ui-parity/lanes/.realinput-lock` (owner note "popover-escape-parity: real input").

1. Lanes: `bun target/pep/lane/lane.mjs start` in this worktree (Studio 16580/16581, Build box 16590/16591);
   `bun target/pep/lane/lane.mjs pair b` writes `target/pep/lane/b/pairing-url` (0600).
2. App: `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`, copied to
   `target/pep/lane-app/T3 Code (Lane Popover).app` (bundle id `com.exact.t3code.macos.lane-pep`, re-signed);
   launch it by path with `CFFIXED_USER_HOME=target/pep/lane/real/home`. Pair Build box in the wizard (paste with
   `set-value`), Continue, Continue, "Do not import projects". Dismiss the two toasts (they cover the first
   card's second segment).
3. Light dismiss: Usage (sidebar gauge) → Limits. Click the Codex 5h segment: its popover pins. Click the empty
   page below the cards: it closes. Pin again, click the sidebar's empty area: it closes. Pin again, click the
   same segment without moving: it closes; move the pointer within the segment: it stays closed; leave and
   re-enter: it opens by hover. Pin the Codex 5h segment, point at the Weekly Work segment (its popover shows
   by hover), click it: the first closes, Work pins and stays. Pin Codex 5h again, point at Weekly Work, move
   into its card and click the blurred email: Codex closes, Work's popover stays and the email shows. With the
   theme editor open over Usage (step 4 first), pin, click into the editor's Theme name field: it closes.
   Read back with window screenshots (`screencapture -l`).
4. Theme editor Escape: Settings (sidebar) → Appearance → Create theme; click the Theme name field; press
   Escape: Settings closes, the editor stays and the field shows no focus ring; press Escape again: the editor
   stays. Click the Accent swatch; drag on the plane (the colour follows); press Escape: only the colour popover
   closes, the swatch has the focus. Read back with screenshots.
5. Context-menu hold (review S1): right-click a sidebar thread row (needs a thread: create one in the lane),
   press Escape on the menu; then on Usage pin a segment and click the empty page once: record whether that first
   click closes it (S1 predicts not; a second click does).
6. Clean-up: quit the copy, delete its Keychain item (service `com.exact.t3code.macos.access-token`, account
   `http://127.0.0.1:16591\n<environment id>`) and its preferences, `bun target/pep/lane/lane.mjs stop`,
   release the lock.
