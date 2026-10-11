# macOS: a click inside an open popover also presses the page control under it

**Status:** Open
**Systems:** host/apple macOS, menus, pointer events
**Severity:** P1
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/281

## Current scope

Consume inside top-layer contacts independently of press handlers. Check real/agent clicks, pointer-only children, focus, drag selection, manual/auto popovers and outside dismissal. #321 also reports the failure for ordinary positioned overlays.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

An open `popover="auto"` is in the top layer: on the web a click inside it belongs to the popover's nodes alone. On macOS, a click on a popover node that has no `press` of its own (a box with `pointerdown`, for a colour plane, a slider track or a drawing pad) also reaches the page node under the popover at that point. A button there is pressed and takes the focus. The person changes something they cannot see.

### Current and expected behavior

- **Current (macOS):** a click inside the popover, at a point over a page `button`, runs the popover node's `pointerdown` and also presses the button under it, which takes the focus.
- **Current (web):** the same click runs only the popover node's `pointerdown`.
- **Expected:** on macOS a click inside an open popover is delivered to the popover's nodes only, whether or not the node under the pointer has a `press`, as the web's top layer does.

Hypothesis, from reading the source: `NodeView.mouseDown` passes a click that no pressable consumes to `super.mouseDown` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1373`), and the comment above it says such a click "reaches the viewport". The popover's `PopoverLayer` (`MenusMac.swift:540`) is a subview of the viewport. So the forwarded click may be resolved against the page under the layer, not against the layer.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

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

At `--size 600x700` the open popover's `pad` spans y 110–310, and the `under` button's centre is at (144, 272), under the pad. `at 120 162` is that point, measured from the pad's top left.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Click on the popover over the button, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 600x700 "tap open" "tap pad clicks 1 at 120 162" "tree counts" state logs` (each tap is a platform click) | macOS 26.6.2, Apple Silicon | main `0365ad1a4` | `under 1 · downs 1`; journal: `pointerdown view 10 (down)` then `press view 4 (hitUnder)`; `focus.logical` 4 (the button under the popover) | `under 0 · downs 1`, focus not on `under` | the drive's `tree`, `state` and `logs` |
| Click on the popover away from the button, macOS | same, `tap pad clicks 1 at 120 40` | same | same | `under 0 · downs 1` | same | the drive's `tree` |
| Same click, web | `bun exact.mjs agent web --size 600x700 "tap open" "tap pad clicks 1 at 120 162" "tree counts"` | Chrome 154 (the agent's) | same | `under 0 · downs 1` | (this is the reference) | the drive's `tree` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- On macOS the repro reads `under 0 · downs 1` after the click over the button, with the agent and with a real click, and the focus does not move to `under`.
- A drag that starts inside the popover starts no text selection on the page under it.
- A popover node with its own `press` keeps working, and a click outside the popover still light-dismisses it.

### Constraints and related work

- Workaround: give the popover's container a `press` (an action that changes nothing) and `retainFocus=true`, so the click ends at the popover. In the repro this reads `under 0 · downs 1`. It costs an extra handler on every popover with pointer-only controls.
- Not tested: iOS, and `popover="manual"`.
- Related: #112 (popover anchoring), #235 (keyboard-opened context popovers).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:13Z

**Decision: Make the popover top layer consume its own pointer contacts.**

Keep open for a correctness fix.

A click that also presses an obscured page button can perform the wrong action. Consumption cannot depend on a press handler being present.

Test real and agent clicks, pointer-only children, drag selection, manual/auto popovers and outside light-dismiss.
