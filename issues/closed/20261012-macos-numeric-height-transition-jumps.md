# macOS: a numeric `height` transition jumps unless its box is border-box with `interpolate-size: allow-keywords` (Chrome animates it)

**Status:** Closed
**Resolution:** Apple adopts pixel-height transitions for either box sizing without keyword opt-in; numeric_height_transitions_need_neither_keyword_opt_in_nor_border_box and the macOS agent repro verify 70 px opening and 210 px closing at 100 ms; running_auto_height_retargets_to_a_length_when_keyword_opt_in_is_removed verifies interruption at 60 px and continuation to 115 px after 100 ms when the same commit removes the opt-in.
**Systems:** host/apple, kernel motion (height transitions)
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone, #99 review)
**Date:** 2026-10-12
**Related:** https://github.com/ccheever/exact2/pull/99#issuecomment-6029652990; rules/DEFERRED.md (General layout transitions); llp/1041-graceful-overload.rfc.md §8.12

## Summary

On macOS a `transition="height …"` between two pixel heights jumps to the new height at once unless
the box is `box-sizing="border-box"` and has `interpolate-size="allow-keywords"` (its own or inherited).
Chrome animates the same box with neither: in CSS `interpolate-size` only matters when an endpoint is a
keyword (`auto`, `min-content`), and `box-sizing` does not decide whether `height` animates. Nothing tells
the author: the build says nothing about the transition, and the agent's `layout` shows the box at
its end height.

## Why this arose

The T3 Code clone's terminal drawer opens with `height=(open ? 280 : 0) transition="height 400ms
ease-out"`. On the web it slides; on macOS it appeared at full height. The clone works around it with
`box-sizing="border-box" interpolate-size="allow-keywords"` on the clipping parent. That workaround was
recorded as advice to authors in the clone's copy of `docs/agent-pitfalls.md`; Charlie's review of #99
(https://github.com/ccheever/exact2/pull/99#issuecomment-6029652990) asked for a fix or a QUEUE line
instead, since it is a host diverging from Chrome. This file is that record.

## Reproduction

App made with `bun scripts/exact.mjs new heightprobe`; `app.ts` as generated (one `greeting` source).

```text
shape Greeting
  text: string

component Heightprobe
  resource greeting = greeting("Heightprobe") as shape Greeting
  state open = false
  action toggle
    open = not open
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" gap=8
      text greeting.text testId="greeting"
      button press=toggle testId="toggle"
        text (open ? "Close" : "Open")
      row gap=8 align-items="flex-start"
        column testId="plain" width=60 height=(open ? 280 : 0) overflow="hidden" background-color="#1e88e5" transition="height 400ms linear"
        column testId="border-box" width=60 box-sizing="border-box" height=(open ? 280 : 0) overflow="hidden" background-color="#e53935" transition="height 400ms linear"
        column testId="opt-in" width=60 interpolate-size="allow-keywords" height=(open ? 280 : 0) overflow="hidden" background-color="#43a047" transition="height 400ms linear"
        column testId="both" width=60 box-sizing="border-box" interpolate-size="allow-keywords" height=(open ? 280 : 0) overflow="hidden" background-color="#8e24aa" transition="height 400ms linear"
```

| Scenario | Commands | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|---|
| Open, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 420x500 "clock data" "tap toggle" "clock +100" "layout plain" "layout border-box" "layout opt-in" "layout both"` | macOS 26.6.2 (25G83), Apple Silicon, agent mode | main `8a911f62b` | at 100 ms `plain`, `border-box` and `opt-in` are 280 px (at 0 ms already); only `both` is 70 px | all four 70 px | record, film |
| Close, macOS | then `"clock settle" "tap toggle" "clock +100" "layout plain" "layout both"` | same | same | `plain` 0 px at once; `both` 210 px | both 210 px | record |
| Same, web | `bun exact.mjs web-build`; `bun exact.mjs agent web …` (the same operations) | Chrome 155 (the agent's) | same | all four 70 px at 100 ms, 280 px at 400 ms | (the reference) | record, film |

The films are `screenshot film.png over 400 every 100`: a single `screenshot` settles a running
transition (LLP 1116.003 D8), so it shows every column at 280 on both hosts.

Not tested: iOS and tvOS (the same Rust host, `host/apple/src/height.rs`, so likely the same), Linux
(LLP 1041 §8.12: its automatic adoption is still open).

## Where

- `kernel/src/motion.rs:601-616`, `height_transition_target`: no target unless the box is border-box;
  it returns the `interpolate-size` opt-in beside the height.
- `host/apple/src/height.rs:196-201`, `sync_height_transitions`: a target without the opt-in is skipped
  (unless its transition is already running), so the layout takes the new height at once. Line 186: only
  with one layout root and no registered content region.

## Constraints

- `rules/DEFERRED.md:670`, "General layout transitions": layout transitions are admitted for one
  registered numeric-height sheet and the Shop clone's accordions; "Still out: animating any other layout
  property, and laying out per frame." A numeric height on any box is wider than either.
- LLP 1041 §8.12 (`llp/1041-graceful-overload.rfc.md:1578`, "Shop host integration") chose
  `interpolate-size: allow-keywords` as the Apple host's opt-in for automatic Height ownership, for
  numeric endpoints too; content-box boxes are refused because the catch needs resolved padding
  (`HeightOwnerError::UnsupportedBoxSizing`). LLP 1002 §4 leaves general layout-affecting transitions
  undecided.
- CLAUDE.md: the web is the layout parity oracle, and an unavoidable deviation is declared in LLP 1001.
  This one is not declared there, nor in `docs/contract-for-agents.md`.

## Options

1. Animate a numeric `height` transition on any box on Apple, as Chrome does (content-box included, no
   opt-in): a ruling that widens DEFERRED's "General layout transitions" to authored height transitions.
2. Keep the gate and make it visible: declare it in LLP 1001, and have the compiler warn on a
   `transition` naming `height` (or `all` with a changing height) on a box the Apple host will not
   animate, naming `box-sizing="border-box" interpolate-size="allow-keywords"`.

## Acceptance criteria

- With option 1: on macOS the repro's four columns are 70 px at 100 ms and 210 px at 100 ms into the
  close, as on the web.
- With option 2: the repro's `plain`, `border-box` and `opt-in` columns each draw a warning at build time
  that names what to write; LLP 1001 lists the deviation.
