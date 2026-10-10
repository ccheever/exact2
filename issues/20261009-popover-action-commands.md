# `showPopover(id)`, `hidePopover(id)`, `togglePopover(id)` from an action and a popover `toggle` event (the popover sibling of #282)

**Status:** Open
**Systems:** Contract, GUI hosts, menus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/319

## Current scope

Select showPopover/hidePopover/togglePopover and toggle payloads through the existing popover lifetime. Pin anchoring, errors, idempotence, ordering and focus against HTML: inferred key-handler anchoring is a proposed extension. Coordinate #282/#281; interestfor stays out.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

This is the popover sibling of #282, which asks for `showModal(id)` and `close(id)` from an action for dialogs.

A Contract popover (`popover="auto"`) opens only when a button that names it is pressed (`popovertarget`). It closes only through such a button, by light dismiss, or by Escape. An action cannot open or close one. The popover also has no `toggle` (or `beforetoggle`) event, so the app never learns that it opened or closed, or how.

The web has all of this in HTML's popover API:
- `HTMLElement.showPopover()`, `hidePopover()` and `togglePopover()`;
- the `beforetoggle` and `toggle` events, with `oldState`/`newState`.

Today an app can open a popover from a key in one way only:
1. it mounts a second, invisible button that names the popover (`popovertargetaction="show"`, `opacity=0`, `pointer-events="none"`, `aria-hidden`);
2. it lays that button over the real trigger, so the popover anchors where the trigger's own press would anchor it;
3. it lets the key press that button through an `aria-keyshortcuts` chord, armed only while the trigger has the focus.

This works on macOS and the web (below), but it relies on two host behaviors that nothing declares: a shortcut presses an invisible, `pointer-events="none"` button, and a popover anchors to whichever invoker showed it.

The consumer is T3 Code's menus, which match Base UI's Menu. Base UI builds on Floating UI's list navigation, which by default opens a menu from its focused trigger on ArrowDown and ArrowUp (`openOnArrowKeyDown`), as WAI-ARIA's menu button pattern does. The clone needs:
1. **↓/↑ on a closed trigger.** No `key` handler can open a popover, so each of the clone's popover-menu triggers carries two invisible invokers, a positioned box it fills, and a focus-armed state.
2. **Knowing that a menu opened, and how.** With no `toggle` event, the clone infers an opening from the popup taking the focus, and the modality from a count that every trigger's `key` handler bumps.
3. **State that follows the open state.** The sidebar's snooze menu keeps its row's hover actions shown while open. The clone cannot read "open", so it pins the row on focus inside the menu or the pointer over one of its rows.

Two other clone tasks need the same capability. A pinned usage popover tracks its open state by hand. A hover card cannot be shown as a top-layer popover from a `hover` action, because LLP 1021 §5.1 also refuses `interestfor`.

### Current and expected behavior

- **Current:**
  - `showPopover("m")` in an action is refused at build (`type-unknown-command`), and so are `hidePopover` and `togglePopover`.
  - `toggle=…` on a popover is refused (`lower-unknown-attr`).
  - The invisible-invoker workaround opens the menu on ArrowDown on both hosts. On macOS, `state.navigation.popover` reads `{"phase":"open","popover":7,"source":5}`, where `source` is the invisible invoker, not the trigger.
- **Expected, as on the web:**
  - host commands `showPopover(id)`, `hidePopover(id)` and `togglePopover(id)`. Showing a popover from a trigger's `key` action anchors it to that trigger, the way #282 shares the invoker path for dialogs;
  - a `toggle` event on a `popover` node that carries the new state (`"open"`/`"closed"`). A cancelable `beforetoggle` is optional.

### Reproduction and evidence

Compiler, with two one-file inputs. `a.contract` is a trigger that opens its menu on ArrowDown:

```text
component App
  state open = false
  action down(k: string)
    if k == "ArrowDown"
      showPopover("m")
  view
    column
      button popovertarget="m" key=down testId="trigger"
        text "Menu"
      column id="m" popover="auto" role="menu"
        button popovertarget="m" popovertargetaction="hide" role="menuitem"
          text "One"
```

`b.contract` is the same menu, reporting its open state:

```text
component App
  state open = false
  action toggled(value: bool)
    open = value
  view
    column
      button popovertarget="m" testId="trigger"
        text "Menu"
      column id="m" popover="auto" role="menu" toggle=toggled
        button popovertarget="m" popovertargetaction="hide" role="menuitem"
          text "One"
```

The workaround, as an app made with `bun scripts/exact.mjs new <dir>`:

```text
component X66app
  state armed = false
  action focusTrigger
    armed = true
  action blurTrigger
    armed = false
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="#ffffff" color="#111111"
      box position="relative" width=120 height=32
        button popovertarget="m" focus=focusTrigger blur=blurTrigger testId="trigger" width=120 height=32
          text "Menu"
        // The workaround: an invisible invoker over the trigger, pressed by ArrowDown while the trigger has the focus
        button popovertarget="m" popovertargetaction="show" aria-keyshortcuts=(armed ? "ArrowDown" : "F35") aria-hidden=true tabindex=-1 opacity=0 pointer-events="none" position="absolute" left=0 top=0 width="100%" height="100%" testId="invoker"
          text "Menu"
      column id="m" popover="auto" role="menu" testId="menu" padding=8 gap=4 background-color="#f4f4f5"
        button role="menuitem" testId="one"
          text "One"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| `showPopover` from an action | `contract build a.contract -o a.plan` | macOS 26.6.2, Apple Silicon (compiler) | main `b896050d7` | exit 1: `` a.contract:5:7 [type-unknown-command] `showPopover` is not a host command; the hosts answer blur, copyText, …, showPicker, showModal, …, close, … `` | builds; ArrowDown opens the menu | [record](https://raw.githubusercontent.com/ccheever/exact2/ff842e44f24a2b7a22e064242c5a6f329adc554f/file-x48-x68/x66-record.txt) |
| `toggle` event | `contract build b.contract -o b.plan` | same | same | exit 1: `` b.contract:9:48 [lower-unknown-attr] `column` has no attribute `toggle` `` | builds; `open` follows the popover | same |
| Invisible-invoker workaround, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --json --size 400x300 "tap trigger" "clock settle" "type trigger key Escape" "clock settle" state "type trigger key ArrowDown" "clock settle" state` | macOS | same | `popover` null after Escape; `{"phase":"open","popover":7,"source":5}` after ArrowDown, with the focus still on the trigger | (the behavior asked for, without node 5) | same |
| Same, web | the same ops with `agent web` | Chrome 154 (the agent's) | same | the menu opens on ArrowDown (screenshot; the web's `state` reports no popover) | same | [screenshot](https://raw.githubusercontent.com/ccheever/exact2/ea794c31c57badf7bc242e0541776ee62c180e07/file-x48-x68/x66-web-after-arrow.png) |

### Acceptance criteria

- `a.contract` and `b.contract` build on every host.
- ArrowDown on the focused trigger opens `m`, anchored to the trigger, and the handler can then move the focus to the first item. `hidePopover("m")` closes it, and `togglePopover` toggles it.
- `toggle` reports `"open"` and `"closed"` for every way the popover changes: an invoker, `showPopover`/`hidePopover`, light dismiss, and Escape.
- On macOS and the web the agent's `state` shows the popover open after `showPopover`, with the trigger as its source.
- A `showPopover` of an id that is not a popover, or one that is already open, is ignored or refused as HTML does, with a journal line.

### Constraints and related work

- Workaround: the invisible invoker above, plus a positioned box over every trigger, a focus-armed state and an opened-by-keyboard count. A menu whose invoker unmounts closes before its item's press lands, so the invokers must stay mounted.
- #282 (`showModal(id)`/`close(id)` from an action) is the dialog half. Its decision, "Share the invoker path", applies here too.
- #112 (popover position fallbacks, "start with invoker popovers") covers anchoring, not opening from an action.
- LLP 1021 §5.1 refuses `interestfor`, so hover-opened popovers are out of scope here.
- Not tested: iOS, Linux, and `popover="manual"`.
