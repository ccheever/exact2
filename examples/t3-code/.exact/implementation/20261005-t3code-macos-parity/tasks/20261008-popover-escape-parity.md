---
name: 20261008-popover-escape-parity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-popover-escape-parity
pr_url: null
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
  the first press, the next press closes it, and a hover reopens it only after the pointer moves and rests
  again (`useHoverReferenceInteraction` `restMs`). A right-click is no click: it does not dismiss.
- `ThemeEditorPanel.tsx` is a plain `role="dialog"` div with no Escape of its own: its only keydown listener
  cancels Inspect (not built here, X30). An Escape in it reaches the page's `useEscapeToGoBack`
  (`routes/settings.tsx`: `navigateToMainApp`; Usage and pull requests: back), and the editor, mounted above the
  router (`ThemeEditorHost`), stays. The colour popover (Base UI `Popover`) closes on Escape, prevents it (so
  Settings' Escape-to-go-back skips it) and returns the focus to its trigger.

## Implementation

- **Page presses** (`pages-usage.contract` UsagePage): the page's root takes `pointerdown=pressDown
  pointerup=pressUp`. `pressDown(e)` notes whether a primary press (`e.buttons == 1`) starts outside the shown
  pinned popover's card (`usage-seg-pop-<id>`), its segment (`usage-seg-<id>`) and its legend row
  (`usage-legend-<id>`), by `frame()` and the event's `clientX`/`clientY` (`fn usageHit`); `pressUp` closes it
  then, before the press's own action (DOM's order), so a press on another segment pins that one.
  `usage-pooled.contract` gives the card and the legend row their ids.
- **Presses elsewhere in the window** (`app-window.contract` T3Window): the window's root counts primary presses
  that end (`outsidePressDown` / `outsidePressUp`, `outsidePresses`), passed through PagesCover to UsagePage as
  `outside`; a pin holds only while the count is the one it was made at (`pinnedAt`, `derive held`). So the
  sidebar, a toast and the floating theme editor all count, as Base UI's document listener does. The theme
  editor's swatch, plane and hue slider take the pointer themselves (they hear `pointerdown`), so they hand
  their presses on (`outsideDown` / `outsideUp` props, `theme-color-picker.contract`).
- **The page's ground** (`usage-ground`): a full-height column under the scroll's content. On macOS a press on a
  scroll view's empty area (below or beside its content) reaches no node, so no `pointerdown` hears it (found in
  session 1; `EXACT2-GAPS.md` "Host finding", not filed).
- **The popover's own segment**: a press on the pinned segment closes it and clears the hover states, so a
  pointer resting on the segment no longer holds it open (base: image 07); the segment's `pointermove`
  (`enterSeg(id, true)`) opens it by hover again once the pointer moves, as `restMs` does. A segment now holds
  its own presses (it hears `pointermove`), which is right: a press on it is never outside its popover, and a
  press on another segment pins that one.
- **Theme editor** (`settings-appearance-editor.contract`): the close button drops `aria-keyshortcuts="Escape"`;
  the editor has no other key handler and is not `aria-modal`, so Escape anywhere in it reaches the page under
  it (Settings' Back: Settings closes, the editor stays). Escape on a page with no Escape of its own (a thread)
  does nothing, as the reference's.

`app.contract` is unchanged (1,456 lines).

## Acceptance

| Row | Result | Proof |
| --- | --- | --- |
| A primary press on the page outside the pinned popover and its triggers closes it (empty ground, a card's body, narrow card text) | pass (agent, macOS) | images 01, 10; drive record session 2 |
| A press outside the page closes it: the sidebar, a toast, the floating theme editor, the editor's swatch | pass (agent, macOS) | images 02, 08, 09; drive record |
| Presses that do not count: inside the card; a right-click; another segment (pins that one instead); Escape closes it and the segment keeps the focus | pass (agent, macOS) | drive record |
| A press on the pinned segment closes it, also with the pointer resting on it | pass (agent, without hover); with a real resting pointer: see the real-input rows | drive record; base with hover: image 07 |
| A press on the same account's legend row (narrow) closes it | not passing, pre-existing (#263's layout): the popover hangs off the segment and covers the legend row's upper half, so the press lands in the popover; the reference anchors the popup to the trigger that opened it | drive record "Not passing"; reported in the PR as a follow-up |
| Escape in the theme editor never closes it; it reaches the page under it (Settings closes, the editor stays), also with Settings reopened over the editor | pass (agent, macOS) | images 03, 04 |
| Escape in the colour popover closes only the popover and focuses its swatch | pass (agent, macOS), unchanged | image 05 |
| Regression tests that fail on the base | pass: 4 new tests fail on `07dcef1ab`'s sources and pass here; 1 guards the unchanged colour-popover Escape | `usage-pooled.test.ts` "light dismiss of a pinned segment popover (popover-escape-parity)" (3), `theme-color-picker.test.ts` "Escape in the theme editor (popover-escape-parity)" (2) |
| Real outside press (real input, lock) | pending | Real-input batch steps |
| Real Escape in the theme editor (real input, lock) | pending | Real-input batch steps |
| Checks | see Attempts | PR Checks |

## Progress

2026-10-08: implemented on `feat(example)/t3-code-popover-escape-parity` (from `07dcef1ab`). Two agent sessions
(one retry) and the base session; session 1 found the scroll-ground and the doubled own-segment press (under
a toast), both fixed before session 2.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | uncommitted, before the ground | agent session 1 (pid 3041): the press under the cards did not dismiss (no `pointerdown` journalled); a second press on segment 0-0-1 kept it open (the segment lay under a toast) | drive record session 1 | fixed in 2 |
| 2 (2026-10-08) | this branch | `bun test examples/t3-code` 3,041 pass / 1 skip / 0 fail (base 3,036); strict `tsc` clean; contract build 3,850 slots (base 3,844); agent session 2 (pid 75713) and base session (pid 98359): rows above | images 01-10, drive record | real-input rows |

## Next action

Review the draft PR. Run the real-input steps below when the shared lock is free.

## Real-input batch steps

Lock `target/t3-ui-parity/lanes/.realinput-lock` (owner note "popover-escape-parity: real input").

1. Lanes: `bun target/pep/lane/lane.mjs start` in this worktree (Studio 16580/16581, Build box 16590/16591);
   `bun target/pep/lane/lane.mjs pair b` writes `target/pep/lane/b/pairing-url` (0600).
2. App: `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`, copied to
   `target/pep/lane-app/T3 Code (Lane Popover).app` (bundle id `com.exact.t3code.macos.lane-pep`, re-signed);
   launch it by path with `CFFIXED_USER_HOME=target/pep/lane/real/home`. Pair Build box in the wizard (paste with
   `set-value`), Continue, Continue, "Do not import projects".
3. Light dismiss: Usage (sidebar gauge) → Limits. Click the Codex 5h segment: its popover pins. Click the empty
   page below the cards: it closes. Pin again, click the sidebar's empty area: it closes. Pin again, click the
   same segment without moving: it closes and stays closed; move the pointer a little: it reopens after the
   fade's delay. Read back with window screenshots (`screencapture -l`).
4. Theme editor Escape: Settings (sidebar) → Appearance → Create theme; click the Theme name field; press
   Escape: Settings closes, the editor stays; press Escape again: the editor stays. Click the Accent swatch;
   press Escape: only the colour popover closes. Read back with screenshots.
5. Clean-up: quit the copy, delete its Keychain item (service `com.exact.t3code.macos.access-token`, account
   `http://127.0.0.1:16591\n<environment id>`) and its preferences, `bun target/pep/lane/lane.mjs stop`,
   release the lock.
