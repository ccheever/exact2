# macOS: hover is not pointerenter/pointerleave — entering a descendant leaves its ancestors, and a node outside its parent's box hears no pointer

**Status:** Open
**Systems:** host/apple macOS, pointer events
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/322

## Current scope

Track the topmost hit subtree and ancestors, including unclipped overflowing children. Preserve stationary-pointer layout hover, inline runs and pointermove. Verify agent and real pointer independently.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On the web, a Contract `hover` handler is `pointerenter`/`pointerleave` (`host/web-js/rt.js:913`, `host/web/glue.js:573-575`). Both count every descendant, an overflowing one included. Moving from an element into its child sends no leave, and a node stays hovered while the pointer is over any of its descendants.

The macOS host keeps **one** hovered node. `Presenter.hover` (`host/apple/Sources/ExactKit/Mac/PresenterMac.swift:848-863`) sends the previous node's leave before the new node's enter, though its comment says "as the web's `mouseenter` and `mouseleave`". Each `hover` node is also an `NSTrackingArea` with `.inVisibleRect` on its own bounds (`NodeViewMac.swift:308-318`). That gives three differences from the web:

1. **Entering a descendant that hears hover leaves its ancestors.** This is true whether the child is inside its parent's box or overflows it. CSS keeps the ancestor hovered.
2. **A node outside its parent's box hears no pointer.** Its tracking area is clipped to every ancestor's bounds (`visibleRect`), even when no ancestor clips its drawing. Moving from a box into its overflowing child also crosses the parent's tracking edge, so the parent hears a leave. CSS hit-tests and hovers the overflowing child like any other.
3. **Overlapping hover nodes trade the hover on every move.** Both tracking areas hear each move, and the presenter keeps one node. CSS hovers the topmost hit node and its ancestors; an occluded sibling is not hovered.

A hover card is the common case: a Base UI `PopoverTrigger openOnHover`, a Radix HoverCard, a hoverable tooltip. The popup is drawn next to its trigger, outside the trigger's box, and keeps itself open through its own hover. On macOS the popup's hover never holds it open, and the trigger's leave closes it as the pointer moves in.

The consumer is the T3 Code clone. A pull request's base-branch freshness card, the Usage page's segment popover and a connection's "N scopes" card all closed as the pointer moved into them. The clone now draws hover cards in a window-level layer with its own frames, state and close-delay timers.

### Current and expected behavior

Measured on main with the agent (the table below). The macOS agent's hover hit-tests the window and calls the presenter's `hover` as a tracking area would (`AgentMac.swift:544-579`), so it shows difference 1, for an overflowing child and an in-box child alike:

- **into an overflowing card that hears hover** (`b` → `b-card`): macOS `B-` then `Bcard+`; the web `Bcard+`, and `b` stays hovered;
- **into a button inside that card:** macOS `Bcard-` then `Bbtn+`; the web `Bbtn+`;
- **into a child inside its parent's own box** (`p` → `c`): macOS `P-` then `C+`; the web `C+`, and `p` stays hovered.

The rest needs a real pointer: tracking areas follow the window server's cursor, which an agent-mode drive does not move. A real-pointer probe on the feature branch's framework (`c0475fbaa`; its hover code differs from main `b896050d7` only by `package` access modifiers) showed:

- **A card with no hover of its own** under `a`: `0A+ … 3A-`, and the card goes. On the web, `a` stays hovered.
- **Into `b`'s card and its button, then out:** `0B+ 1B- 2Bcard+` and nothing more. The button never heard an enter, and leaving the card sent no leave.
- **Five small moves inside `o`, over `u`:** twenty events alternating `U-`/`O+`/`O-`/`U+`, ending `O+` at rest.

**Expected (macOS, as on the web):**
- a `hover` handler's area is its node's subtree, overflowing descendants included;
- moving into a descendant sends no leave;
- the hovered set is the topmost hit node and its ancestors, decided by a hit test, not by every tracking area under the pointer.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X62app
  state log = ""
  state n = 0
  state aOver = false
  action ev(name: string, over: bool)
    n = n + 1
    log = `${n}${name}${over ? "+" : "-"} ${log}`
  action hA(over: bool)
    aOver = over
    ev("A", over)
  view
    main width="100%" height="100%" padding=24 position="relative" background-color="#ffffff" color="#111111"
      text `log: ${log}` font-size=13 testId="log"
      // 1: a box whose card (no hover of its own) appears below it, outside the box
      box position="absolute" left=24 top=140 width=160 height=20 background-color="#3b82f6" hover=hA testId="a"
        when aOver
          box position="absolute" left=0 top=20 width=160 height=80 background-color="#93c5fd" testId="a-card"
      // 2: a box whose card, outside the box, hears hover itself, with a button in the card
      box position="absolute" left=300 top=140 width=160 height=20 background-color="#10b981" hover=ev("B") testId="b"
        box position="absolute" left=0 top=20 width=160 height=80 background-color="#6ee7b7" hover=ev("Bcard") testId="b-card"
          box position="absolute" left=30 top=50 width=100 height=24 background-color="#047857" hover=ev("Bbtn") testId="b-btn"
      // 3: a node under a later, overlapping sibling
      box position="absolute" left=560 top=140 width=160 height=80 background-color="#f59e0b" hover=ev("U") testId="u"
      box position="absolute" left=590 top=160 width=100 height=40 background-color="#7c2d12" hover=ev("O") testId="o"
      // 4: a child inside its parent's box, both hearing hover
      box position="absolute" left=24 top=260 width=200 height=40 background-color="#e9d5ff" hover=ev("P") testId="p"
        box position="absolute" left=120 top=8 width=60 height=24 background-color="#7e22ce" hover=ev("C") testId="c"
```

Ops: `"tap a hover"`, `"tap a-card hover"`, `"tap b hover"`, `"tap b-card hover"`, `"tap b-btn hover"`, `"tap b-card hover"`, `"tap p hover"`, `"tap c hover"`, `"tap p hover"`, `"tap u hover"`, each followed by `"clock settle" "tree log"`.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Hover through the four cases, macOS (agent) | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 780x340 <ops>` | macOS 26.6.2, Apple Silicon | main `b896050d7` | final log `16O+ 15P- 14P+ 13C- 12C+ 11P- 10P+ 9Bcard- 8Bcard+ 7Bbtn- 6Bbtn+ 5Bcard- 4Bcard+ 3B- 2B+ 1A- 0A+` | the web's | [record](https://raw.githubusercontent.com/ccheever/exact2/71ecbd240077e8f1974fd171a824afdf96c909c9/file-x48-x68/x62-record.txt) (the log after every step, both hosts) |
| Same, web | `bun exact.mjs agent web --size 780x340 <ops>` (CDP pointer moves) | Chrome 154 (the agent's) | same | final log `12O+ 11P- 10C- 9C+ 8P+ 7B- 6Bcard- 5Bbtn- 4Bbtn+ 3Bcard+ 2B+ 1A- 0A+` | (the reference) | same |
| Real pointer, macOS | the draft's probe (cases 1–3), launched normally, cliclick moves of 2–8 pt, read from full-screen captures | macOS 26.6.2 | feature branch `c0475fbaa` (hover code identical to main but for access modifiers) | case 1 `0A+ … 3A-`; case 2 `0B+ 1B- 2Bcard+` and nothing more; case 3: twenty alternating events over five moves | the web's | same, last section |

### Acceptance criteria

- **macOS agent drive:** the repro's log matches the web's step for step.
- **macOS real pointer** (the real-input batch), moving slowly through each case:
  - `a` stays hovered while the pointer is over its card;
  - `b`, `b-card` and `b-btn` enter and leave as on the web;
  - moves inside `o` send nothing after `O+`;
  - leaving a card sends its leave.
- **Cases that keep working:** hover under a stationary pointer after a layout change (#139's fix), inline-run hover, and `pointermove` delivery.

### Constraints and related work

- Workaround (the clone): draw every hover card in a window-level layer whose boxes span the window. Hold it open with a close delay on a root timer while the pointer crosses into it, and report hover from inside the card to the card. Each app repeats this, and the web and macOS disagree for the same Contract.
- Related:
  - #139 (closed): hover after a layout change under a stationary pointer;
  - #281: a click inside a popover also presses the page under it;
  - #112 and #127: popover anchoring.
- Out of scope here: a hover-opened top-layer popover (HTML's `interestfor`), which LLP 1021 §5.1 refuses. It would let a hover card escape clipping without a hand-made layer, but it is a separate design question.
- Not tested: iOS (pointer hover on iPad), and a real pointer on main itself (the screen was locked for this run).
