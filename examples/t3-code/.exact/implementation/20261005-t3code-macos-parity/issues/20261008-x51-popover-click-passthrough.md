---
name: 20261008-x51-popover-click-passthrough
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/281
reproduced_on: 0365ad1a4 (main)
---

# X51: a click inside an open popover also reaches the page under it (macOS)

## Summary

On the macOS host, a click on a node inside an open `popover="auto"` that has no `press` of its
own (a box with `pointerdown`, a plain box) is heard by that node, and the same click also reaches
the node of the page that lies under the popover at that point: a `button` there is pressed, a text
field there takes the focus, and a drag there starts a text selection across the window. The
expected result is the web's: the popover is in the top layer, so the click is the popover's alone.

## Why this issue arose

### The T3 Code behavior
The theme editor's colour popover (`ThemeColorPicker.tsx`, `ui/color-picker.tsx`) is a Base UI
popover over the floating editor. A press or a drag on its saturation/brightness plane or its hue
slider changes the colour and nothing under the popover.

### What exact2 does today
Driven with the agent on this branch's build before the workaround (2026-10-08, `agent.mjs macos`,
lane ports 16050, each a real `NSApp.sendEvent` click):
- the Accent popover opened above its swatch, covering the editor's Light/Dark toggle; `tap
  theme-color-accent-plane clicks 1 at 81 102` moved the plane's thumb (`pointerdown` heard, colour
  `#242834`) and also pressed **Dark** under it: the editor switched to its dark palette;
- `tap theme-color-accent-plane` (its centre, over the editor's "Theme name" field): the plane took
  the colour and the **name field took the focus** (`state` `focus.logical` = the field);
- a drag on the hue slider highlighted the text of the whole window, as a page text selection.
  `user-select="none"` on the popover hides that symptom only.

A one-file app does **not** reproduce it (see "How to reproduce"), so the trigger is something the
clone's popover does that the small app does not: the click's `pointerdown` action sends a mutation
whose reply refreshes the whole app (the draft repaints every surface) and calls `focus()`, and the
popover sits over a `z-index: 110` floating panel inside the Settings page. Suspected path (read, not confirmed): `NodeView.mouseDown` passes a click it does not consume to
`super.mouseDown`, up the AppKit responder chain; the top layer (`MenusMac.swift` `PopoverLayer`) is a
subview of the page's `PageScrollView`, and the forwarded event reaches the page's views there.

### Where the clone hits it
`theme-color-picker.contract` `ThemeColorSwatch`: the plane and the hue slider are boxes with
`pointerdown`/`pointermove`/`pointerup` and no `press`; the popover opens over the editor's own
fields and buttons (above its swatch when the window has no room below, as Base UI's flip does).

## Why it must be resolved

A click meant for a popover changes something the user cannot see under it (a theme's appearance, a
field's focus, a selection). Any popover with custom pointer controls (a colour picker, a slider, a
drawing pad) is affected.

## Requested support

The macOS host delivers a click inside an open popover to the popover's nodes only, as the web's top
layer does, whether or not the node under the pointer has a `press`.

## How to reproduce

In the clone (reproduces): this branch at `454daaff3` (before the workaround in `b2074c7a1`), lane
`T3_LOCAL_HOME`/`T3_LOCAL_PORT`, `EXACT_MAC_BIN` = the bundle's executable,
`bun scripts/agent.mjs macos --size 1280x840` with: `tap welcome-continue`, `tap welcome-agents-continue`,
`tap welcome-skip-import`, `tap connection-settings`, `tap settings-appearance`, `tap create-theme`,
`tap theme-editor-swatch-accent`, `tap theme-color-accent-plane clicks 1 at 81 102`, `state`: the accent
row changed and the editor's appearance is `dark` (the Dark toggle under the popover was pressed).

Minimal app (does not reproduce, 2026-10-08): `exact new`, then the `app.contract` below, `bun exact.mjs
mac`, `bun exact.mjs agent macos --size 600x700 "tap open" "tap pad" tree` reads `under 0 · downs 1`
(expected). Three variants were tried and none passed the click through: the popover over a plain
button; inside a `z-index: 110` absolute panel with a `scroll`; with the popover `key`/`tabindex=-1`,
a `focus()` on press, `touch-action="none"`, `cursor`, `pointermove`/`pointerup` and a drag.

```text
component PopoverPassthrough
  state under = 0
  state downs = 0
  action hitUnder
    under = under + 1
  action down(e: PointerEvent)
    downs = downs + 1
  view
    main testId="root" width="100%" height="100%" padding=24
      text `under ${under} · downs ${downs}` testId="counts"
      column gap=16 margin-top=200
        button press=hitUnder testId="under" width=240 height=60
          text "Under the popover"
        box position="relative" width=240
          button popovertarget="p" testId="open" width=240 height=40
            text "Open"
          column id="p" popover="auto" position="absolute" position-area="top span-right" margin-bottom=8 width=240 height=200 background-color="#dddddd"
            box pointerdown=down width=240 height=200 testId="pad"
```

Next step to confirm: grow the small app toward the clone (a mutation whose reply refreshes a
resource that recolours the panel, from the `pointerdown` action) until it passes the click through.

## Acceptance for the fix

- The repro reads `under 0 · downs 1` after `tap pad`, on macOS with the agent and with a real click.
- A drag inside the popover starts no page text selection.

## App adoption after resolution

Remove `press=popPress retainFocus=true` (and `popPress`) from the colour popover in
`theme-color-picker.contract`; keep `user-select="none"` only if the reference needs it.

## Status and next action

Local draft (2026-10-08, theme-color-picker), unconfirmed: seen in the clone, not in a one-file app.
Not searched upstream and not published (this round files nothing upstream; publication needs the
user's approval through `issue-open`).
Workaround in the clone: the popover box takes `press` (an action that only marks the press as a
pointer one) and `retainFocus=true`, so the click ends at the popover and no focus moves except the
control's own `focus()`.

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/281 (#281, [Bug] macOS: a click inside an open popover also presses the page control under it). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) with a minimal public-API app before filing. Reproduced in the one-file app above. The earlier tries clicked the popover's pad at its centre, which lies above the button under it. At 600x700 the pad spans y 110-310 and the `under` button's centre is (144, 272): `tap open`, `tap pad clicks 1 at 120 162` reads `under 1 · downs 1`, journal `pointerdown view 10 (down)` then `press view 4 (hitUnder)`, focus on `under`. A click away from the button reads `under 0 · downs 1`, and the web reads `under 0 · downs 1` for the same click. The clone's workaround (`press` + `retainFocus` on the popover) reads `under 0 · downs 1` in the repro. Searched: popover click, popover press, click through: no duplicate.

Next: issue-close once #281 lands: remove `press=popPress retainFocus=true` from the colour popover.

## Decided upstream (2026-10-08): waits for main fix of #281

[Charlie on #281](https://github.com/ccheever/exact2/issues/281#issuecomment-6055579701): "Make the popover top layer consume its own pointer contacts."
- Waits for main fix of [#281](https://github.com/ccheever/exact2/issues/281), then an adoption round. On adoption, `press=popPress retainFocus=true` leaves `theme-color-picker.contract`.
- **Fixed by [#327](https://github.com/ccheever/exact2/pull/327)** (open on main, 2026-10-08); this resumes in the main-adoption round after it merges. Then `press=popPress retainFocus=true` leaves the colour popover in `theme-color-picker.contract`; re-run one row: a press on the popover's header (no handler) still counts as outside for #290's light dismiss.
