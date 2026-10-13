# web: an Exact contextmenu inside a checkbox, switch, radio, range or date input never runs

**Status:** Closed
**Resolution:** fixed by #495
**Systems:** host/web-js, host/web (the wasm host's glue)
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** #487 (X85, its Not done); `issues/closed/20261012-macos-control-click-contextmenu.md` (X85); review b5-b 3 (`940335753`); https://raw.githubusercontent.com/ccheever/exact2/fc0f37ce5527f8097e7b5df43a6d707539976294/fw/x85-control-click-contextmenu/x85-native-record.txt (the X85 review record: Chrome itself, the web target and macOS)

## Summary

On the web target a right-click or a Control-click on a checkbox, switch, radio, range, date or time input inside a
node with `contextmenu` runs no handler. The row's menu does not run, and Chrome opens its own page menu instead
(Back, Reload, Inspect), because nothing prevents it. A `contextPopover` on such a node does not open either.

`onContextmenu` in `host/web-js/rt.js` returns early when the event's target is inside
`input,textarea,[contenteditable]`:

```js
// A field's own edit menu stays the browser's; the nearest handler alone hears it, as on the wasm host and macOS (review b5-b 3).
export const onContextmenu = (e, kind, f, l) => { return l(kind, ev => { if (ev.target.closest("input,textarea,[contenteditable]")) return; … }); };
```

The comment says what the skip is for: a text field's own edit menu (Cut, Copy, Paste, Spelling) stays the
browser's. A checkbox, switch, radio, range, date, time or file input has no edit menu, but the selector also
matches every `<input>`, so the skip catches these controls too. The same selector guards the `contextPopover`
(rt.js `cp`), and the wasm host has the same rule in `host/web/input-glue.js` (`mouse`) and `host/web/glue.js`
(the `contextpopover` listener). The conformance harness compares those with the JS target.

Chrome itself sends `contextmenu` from each of these controls, and it bubbles to the row. The X85 record
(a plain page, no Exact, Chrome 155 over CDP) shows it for a checkbox, switch, radio, range, select, date and link.
macOS runs the row's `contextmenu` for a right-click on a control, and since #487 for a Control-click too. So on the
same Mac the web target and the macOS host disagree, and the web target disagrees with Chrome.

## Why this arose

#487 (X85) made a Control-click on macOS run a node's `contextmenu`, AppKit's own controls included. Its review round
drove a row with `contextmenu=menu("row")` holding a checkbox, a switch and a range on the web as well. There no
`contextmenu` ran, and #487's Not done records the gap: "That selector was written for fields, and the difference
predates X85. Changing the web host is outside this issue."

## Reproduction

Main `fe5b2679833a6c9860819ceb238bb2e3cd13ead7`, Google Chrome 155 (headless, the agent's), macOS 26.6.2. A one-file
app made with `bun scripts/exact.mjs new <dir>` (`app.ts` exports an empty `sources`):

```text
component App
  state log = "-"
  state menus = 0
  state changes = 0
  state on = false
  state level = 20
  action menu(name: string)
    menus = menus + 1
    log = `contextmenu ${name}`
  action check(value: bool)
    on = value
    changes = changes + 1
    log = `change checkbox ${value}`
  action slide(value: number)
    level = value
    changes = changes + 1
    log = `input range ${value}`
  view
    main testId="root" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text `${log} · changes=${changes} · menus=${menus}` testId="log"
      row contextmenu=menu("row") testId="row" gap=14 padding=10 align-items="center"
        text "A row with a menu"
        input type="checkbox" checked=on change=check aria-label="Check" testId="check"
        input type="range" min=0 max=100 value=level input=slide aria-label="Level" width=120 testId="range"
        input value="" aria-label="Notes" testId="field"
```

`bun exact.mjs agent web "tap check contextmenu" tree` (a right-click through CDP at the checkbox's middle), or
`"tap check modifiers Control"` (a Control-click):

```text
  Text#2 [log] "- · changes=0 · menus=0"
  View#3 [row] (contextmenu)
    Control#5 [check] label="Check" checked=false [focused] (change)
```

Expected, as Chrome sends it and macOS runs it: `contextmenu row · changes=0 · menus=1`. The same holds for a switch
(`input type="checkbox" switch`), a radio, a range, a date and a time input. A right-click in the text field `field`
should still run nothing, so the browser's edit menu opens there.

## Current scope

Keep the skip only where the browser's own text-editing menu belongs: a text field `<input>` (navigation.js
`textField`), a `textarea` or an editable region. Any other `<input>` (a checkbox, switch, radio, range, date, time,
datetime-local or file input) runs the nearest Exact `contextmenu`, and the nearest `contextPopover` opens. Make the
change on both web hosts (the JS target's `rt.js` and the wasm host's glue), so that conformance keeps them in
agreement.

Out of scope:

- A Control-click on a range: Chrome's own mouse down moves the value before `contextmenu` arrives, and that stays
  as Chrome does it. macOS leaves the value as its right-click does, which #487 records.
- The wasm host's `dblclick`, which shares that line in `input-glue.js` and is unchanged.
