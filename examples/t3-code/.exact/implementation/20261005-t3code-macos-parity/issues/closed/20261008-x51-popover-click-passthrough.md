---
name: 20261008-x51-popover-click-passthrough
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/281
reproduced_on: 0365ad1a4 (main)
---

# X51: a click inside an open popover also reaches the page under it (macOS)

Moved to main `issues/20261009-macos-popover-pointer-passthrough.md` (2026-10-09); tracked there.

## Summary

On the macOS host, a click on a node inside an open `popover="auto"` that has no `press` of its
own (a box with `pointerdown`, a plain box) is heard by that node, and the same click also reaches
the node of the page that lies under the popover at that point: a `button` there is pressed, a text
field there takes the focus, and a drag there starts a text selection across the window. On the web
the popover is in the top layer, so the click is the popover's alone.

## Why it arose

### The T3 Code behavior
The theme editor's colour popover (`ThemeColorPicker.tsx`, `ui/color-picker.tsx`) is a Base UI
popover over the floating editor. A press or a drag on its saturation/brightness plane or its hue
slider changes the colour and nothing under the popover.

### Where the clone hit it
[theme-color-picker](../../tasks/closed/20261007-theme-color-picker.md), `theme-color-picker.contract`
`ThemeColorSwatch`: the plane and the hue slider are boxes with `pointerdown`/`pointermove`/`pointerup` and no
`press`; the popover opens over the editor's own fields and buttons (above its swatch when the window has no room
below, as Base UI's flip does). Driven with the agent on the branch's build before the workaround (2026-10-08,
`agent.mjs macos`, each a real `NSApp.sendEvent` click):
- the Accent popover opened above its swatch, covering the editor's Light/Dark toggle; `tap
  theme-color-accent-plane clicks 1 at 81 102` moved the plane's thumb and also pressed **Dark** under it;
- `tap theme-color-accent-plane` (its centre, over the editor's "Theme name" field): the plane took
  the colour and the **name field took the focus**;
- a drag on the hue slider highlighted the text of the whole window, as a page text selection.

## Clone workaround

Nonblocking: the colour popover box takes `press=popPress` (an action that only marks the press as a pointer one)
and `retainFocus=true` (`b2074c7a1`), so the click ends at the popover and no focus moves except the control's own
`focus()`. Once the macOS top layer consumes its own pointer contacts, `press=popPress retainFocus=true` (and
`popPress`) leave `theme-color-picker.contract`; then re-run one row: a press on the popover's header (no handler)
still counts as outside for #290's light dismiss. The real-pointer row of
[fix-hover-cards](../../tasks/closed/20261008-fix-hover-cards.md) for a press on the Usage popover's padding is
re-run then too.

## Evidence and history

- Clone reproduction: the branch at `454daaff3` (before the workaround), lane `T3_LOCAL_HOME`/`T3_LOCAL_PORT`,
  `bun scripts/agent.mjs macos --size 1280x840` with `tap welcome-continue`, `tap welcome-agents-continue`,
  `tap welcome-skip-import`, `tap connection-settings`, `tap settings-appearance`, `tap create-theme`,
  `tap theme-editor-swatch-accent`, `tap theme-color-accent-plane clicks 1 at 81 102`, `state`: the accent row
  changed and the editor's appearance is `dark`.
- First kept as a local draft (2026-10-08): a one-file app clicked at the pad's centre did not reproduce it.
- Filed as [#281](https://github.com/ccheever/exact2/issues/281) on 2026-10-08, reproduced on main `0365ad1a4` in
  the one-file app at a point over the button under the pad: `tap pad clicks 1 at 120 162` reads `under 1 · downs 1`,
  journal `pointerdown view 10 (down)` then `press view 4 (hitUnder)`; the web reads `under 0 · downs 1`. The
  clone's workaround reads `under 0 · downs 1` in the repro.
