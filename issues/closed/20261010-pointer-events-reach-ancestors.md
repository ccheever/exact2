# Pointer events do not bubble: a node's `pointermove` takes its ancestors' `pointerdown`, and on macOS a press in a popover reaches no ancestor

**Status:** Closed
**Resolution:** fixed: Charlie chose option 1 (2026-10-15); `pointerdown` and `pointerup` bubble on every host, and the repro reads 6 of 6 on the web and macOS
**Systems:** Contract, pointer events, GUI hosts, host/apple macOS
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x71-pointermove-without-press-capture.md; issues/20261009-macos-popover-pointer-passthrough.md (#281)

## Current scope

2026-10-12, #461: part 2 is fixed. On macOS, `pointerPressed` now walks the authored tree (`menus.parent(of:)`), so a press on Row C reaches the root's `pointerdown`. The repro's macOS count is now 4, the same as the web's (`PopoverMacTests.testAPressInAPopoverReachesItsAuthoredAncestorsPointerdown`).

2026-10-15: part 1 is fixed by option 1 (Decision, below).

## Summary

Two related gaps, found together.

1. **Design: pointer events do not bubble.** In Contract the innermost enabled node that hears any of
   `pointerdown`, `pointerup` or `pointermove` takes the pointer, and only that node hears the press
   (`llp/1005-plan-and-runner-v1.spec.md:490-501`). So a node that only wants to hear moves, with a
   `pointermove`, takes every press inside it from its ancestors: their `pointerdown` and `pointerup` never
   fire. In DOM, `pointerdown` bubbles to every ancestor, and a `pointermove` listener changes nothing about
   who hears a press. Contract's `key` and `wheel` already bubble, innermost first
   (`docs/contract-grammar.md:882-883, 1048-1050`); the pointer events are the exception.
2. **Bug (macOS): a press in a `popover` reaches no ancestor's `pointerdown`.** With no pointer handler
   inside the popover, the press should go to the innermost ancestor that hears `pointerdown`, by Contract's
   own rule. The web host delivers it there; macOS delivers it nowhere.

Neither changes on #327's head (`14493d253`). #327 makes a contact inside a popover stop at the top layer,
so page nodes **under** it are not pressed (#281). The popover's **ancestors** are a different case: in DOM
they hear the bubbled `pointerdown`; nodes merely under the popover do not.

## Why this arose

Base UI's menus (T3 Code `1e2ecbd975`, `ui/menu.tsx` over `@base-ui/react` Menu) take the highlight back to
the row under the pointer on any pointer move (`onMouseMove`, `focusItemOnHover`). After ArrowDown moves
the highlight away, a small move of the resting pointer brings it back.

The clone's painted menus (audit-wave-followups-4, #383, `954f6bbff`) move the highlight only when the
pointer enters a row. Hearing every move needs a `pointermove` on the popup or its rows. The clone closes
its owner-drawn popups with a window light dismiss: the root's `pointerdown=outsidePressDown
pointerup=outsidePressUp` (`app-window.contract:361-369, 594`), which counts every press. A `pointermove` on
the popup took the presses inside the menu from that count. Forwarding them back through injected window
actions grew the plan from 27.1 to 33.7 MB, so the clone kept the enter-only rule and declares the
difference in `EXACT2-GAPS.md` ("Painted menus: one highlight").

Any popup that wants hover-follow behaviour and an outside-press dismiss at the window meets the same
choice.

## Reproduction

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` exports an empty `sources`.

```text
component X71app
  state downs = 0
  state moves = 0
  state picked = "-"
  action rootDown
    downs = downs + 1
  action move(e: PointerEvent)
    moves = moves + 1
  action pick(name: string)
    picked = name
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 display="flex" flex-direction="column" gap=12 background-color="#ffffff" color="#111111" pointerdown=rootDown
      text `root pointerdown: ${downs} · pointermove: ${moves} · picked: ${picked}` white-space="nowrap" testId="out"
      row gap=16 align-items="flex-start"
        column testId="plain" padding=8 gap=4 border-width=1 border-style="solid" border-color="#999999"
          text "box"
          button press=pick("a") testId="row-a"
            text "Row A"
        column testId="moving" padding=8 gap=4 border-width=1 border-style="solid" border-color="#999999" pointermove=move
          text "box with pointermove"
          button press=pick("b") testId="row-b"
            text "Row B"
        button popovertarget="pop-c" testId="open-c"
          text "Open popover"
        button popovertarget="pop-d" testId="open-d"
          text "Open popover with pointermove"
      column id="pop-c" popover="auto" testId="pop-c" padding=8 gap=4 background-color="#fefefe" border-width=1 border-style="solid" border-color="#999999"
        text "popover"
        button press=pick("c") testId="row-c"
          text "Row C"
      column id="pop-d" popover="auto" testId="pop-d" padding=8 gap=4 background-color="#fefefe" border-width=1 border-style="solid" border-color="#999999" pointermove=move
        text "popover with pointermove"
        button press=pick("d") testId="row-d"
          text "Row D"
```

Ops: `bun exact.mjs agent <macos|web> --size 760x220 "tap row-a" "tap row-b" "tap row-b hover" "tap open-c" "tap row-c" "tap open-d" "tap row-d hover" "tap row-d"`, each followed by `"clock settle" "tree out"`. Six presses in all.

| Press on | Chrome 155, plain HTML (reference) | Exact web, main `abc1eadff` | Exact macOS, main `abc1eadff` | Exact macOS, #327 `14493d253` |
|---|---|---|---|---|
| Row A (a box) | heard | heard | heard | heard |
| Row B (a box with `pointermove`) | heard | **not heard** | **not heard** | **not heard** |
| Open popover (a button) | heard | heard | heard | heard |
| Row C (a popover) | heard | heard | **not heard** | **not heard** |
| Open popover with pointermove | heard | heard | heard | heard |
| Row D (a popover with `pointermove`) | heard | **not heard** | **not heard** | **not heard** |
| Root `pointerdown` count at the end | 6 | 4 | 3 | 3 |

Every row's own `press` runs on every host; the `pointermove` handlers fire. Platforms: macOS 26.6.2
(25G83), Apple Silicon; Chrome 155.0.8059.39 (the agent's, and a headless Chrome driven over CDP with the
same page as plain HTML for the reference column).

## Constraints

- The rule is decided and the same on every host: LLP 1005 §Events
  (`llp/1005-plan-and-runner-v1.spec.md:490-501`: "The innermost claims the event as it bubbles, so its
  ancestors' handlers leave it"); `docs/contract-grammar.md:841-851`; LLP 1056 §8.6
  (`llp/1056-canvas-2d.rfc.md:796`: once a pointer goes down on a node that hears any of the three, its
  moves are that node's until it lifts).
- DEFERRED admitted the three events with consumers (`rules/DEFERRED.md:609-621`: `pointerdown`/`pointerup`
  2026-10-03, `pointermove` 2026-10-04; "Still no multi-touch"). Changing who hears a press is a change to
  an admitted design, so it is the owner's call (below).
- The hold must stay: a canvas stroke's moves follow the node the press went down on, wherever the pointer
  goes (`docs/contract-grammar.md:843-845`).
- #281 and #327 must stay true: a contact inside a popover never reaches a page node that is only under it.
- Code: macOS walks `superview` from the hit view (`host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:100-118`),
  and a popover's view is not inside its tree parent's view, so the root is never reached. The JS target
  claims the event for the innermost listener (`host/web-js/pointer.js:48-50`).

## Acceptance criteria

- **Part 2 (bug, no decision needed):** on macOS a press on Row C reaches the root's `pointerdown`, as on
  the web host. The repro's macOS count then matches the web's (4 under the current rule).
- **Part 1 (after the decision):** with option 1 or 2 below, the root's `pointerdown` count is 6 of 6 on
  macOS and the web, as in Chrome. Every row's `press` still runs once; `pointermove` still counts; a
  canvas that hears `pointerdown`/`pointermove` still gets its own held moves.
- #281's repro still reads `under 0` on macOS.

## Decision needed

How a press reaches ancestors:

1. **DOM bubbling.** `pointerdown` and `pointerup` go to the innermost node that hears them, then to every
   ancestor that declares them, innermost first, as `key` and `wheel` do. The innermost still holds the
   pointer for its moves.
2. **Narrower.** Only a `pointerdown` or `pointerup` handler claims a press. A node that hears only
   `pointermove` no longer takes the press from its ancestors.
3. **Keep the rule, declared.** Apps forward presses to the window themselves (in the clone that grew the plan
   from 27.1 to 33.7 MB).

## Decision

**Charlie, 2026-10-15: option 1, DOM bubbling.** `pointerdown` and `pointerup` go to the innermost
node that hears them, then to every ancestor that declares them, innermost first, as `key` and
`wheel` do. The innermost node that hears any of the three pointer events still holds the pointer
for its moves until it lifts (the canvas-stroke hold, LLP 1056 §8.6). #281 and #327 stay true: a
contact inside a popover never reaches a page node that is only under it, while the popover's authored
ancestors hear the bubbled press, as in the DOM. Chrome's behaviour is the oracle.

What was built to it:

- **The up's path is the down's.** Each node the down passed that hears `pointerup` hears the up,
  innermost first, wherever the pointer lifts: the nodes are fixed at the down, as a DOM pointer capture
  fixes its target. A node the tree removes while the pointer is down is skipped; its ancestors still
  hear the up. A node that heard the down hears the up even if it was disabled since, so what the down
  began ends.
- **No `stopPropagation()`.** As for `wheel`, no handler stops the bubbling; `stopPropagation()`
  stays the key and focus events'. Admitting it for pointers is a separate decision.
- **Disabled.** A node disabled when the pointer goes down (a control or any node with `disabled`)
  hears neither event; its enabled ancestors still do. Each host checks at each node's turn, so an inner
  handler that disables an ancestor keeps the down from it.
- **Recognizers take nothing.** On the web a `pan`, a swipe or a transform stops its contact's
  `pointerdown` so that ancestors' recognizers do not also arm; the pointer events therefore no longer
  ride DOM bubbling. One listener at the root's capture walks the event's path, innermost first, before
  any element's own listener (`input-glue.js`, `pointer.js`), as the native hosts deliver them beside
  their recognizers.
- **Hosts.** The JS target (`host/web-js/pointer.js`) and the wasm host (`host/web/input-glue.js`
  `pointer`): an ancestor holds only its up, and tracks the pointer's latest point for an up that a
  window blur ends. macOS (`MouseChainMac`, `Presenter.pointerPath`; a press in a native `NSPopover`'s
  window reaches the page with the point converted into the page's window), iOS (`PointerRecognizer`:
  the innermost sends the down and up for every node from it up through authored parents, a lifted
  popover's included) and Linux (`presenter/pointer.rs`, `pointer_path`).
- **Text.** LLP 1005 §Events, `docs/contract-grammar.md` §Pointer, LLP 1056 §8.6 and
  `rules/DEFERRED.md`'s admission.

Verified 2026-10-15 (macOS 27.0, Xcode 27.0; the agent's Chrome), the repro above with a `presses`
count added to `pick`:

| Host | Root `pointerdown` before | after | Row presses after |
|---|---|---|---|
| Web (JS target) | 4 | 6 | 4 of 4 (A, B, C, D once each) |
| macOS | 4 | 6 | 4 of 4 |

`pointermove` still counts (web 4, macOS 2: the agent's hover differs by host, unchanged by this).
A canvas with `pointerdown pointermove pointerup touch-action=none` inside the root, dragged 260
points: root 1, begin 1, held moves 16 (web) and 15 (macOS) past the canvas's right edge, up 1. #281's
repro reads `under 0 · downs 1` on macOS with the root's `pointerdown` counting both presses.

Tests: `host/web/tests/pointer.test.mjs` (Chrome: bubbling, a `pointermove` child, a pan, a blur, a
removed ancestor), `host/web-js/conformance/pointer-bubble` (the JS target against the wasm runner),
`GesturePrecedenceMacTests.testPointerDownAndUpBubbleWhileTheInnermostHoldsTheMoves`,
`PopoverMacTests.testAPressInAPopoverReachesItsAuthoredAncestorsPointerdown` and
`PopoverNativeMacTests.testAPressInThePopoverReachesThePageWithThePagesPoint` (macOS),
`GesturePrecedenceIOSTests.testTheInnermostEnabledPointerNodeTakesTheTouch` (iOS), and
`touch_tests.rs`'s `the_down_and_the_up_bubble_and_the_innermost_holds_the_moves` (Linux).
