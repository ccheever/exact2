# macOS: frame() of an inline text run is unavailable, and agent layout omits inline runs (rest of #133)

**Status:** Open
**Systems:** host/apple, text geometry, agent
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/272

## Current scope

Expose viewport-space unions of inline-run rectangles via frame/layout. Compare wrapped links with Chrome at matching fonts and after scroll; share #274 geometry. Reactive geometry remains separate.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #133, split out because #133 closed when its agent-hover part landed (#178: the macOS agent hit-tests wrapped inline text on hover; confirmed on main below). The other part remains: on macOS an inline run inside a `text` (a link of several words that wraps) has no geometry.

- `frame(id)` of the run answers `unavailable`, where the web answers the run's `getBoundingClientRect()` (the union of its line boxes).
- The agent's `layout` lists no inline runs on macOS, where the web lists each run with its rectangle.

An app that opens a preview card at a link inside rendered text, or anchors a marker to a phrase, needs the run's rectangle. Today a macOS app has none, and splitting the run into per-word boxes changes line breaking and still gives no single rectangle.

### Current and expected behavior

Current, on main `0365ad1a4` (the relevant macOS and web host files are unchanged on `e200397ec`): a 260-wide paragraph whose inline run `#4 [link]` wraps from line 1 to line 2.

- Web: `layout` has `#4 [link] Text 24,24 228.34375×36`; `frame("link")` in an action gives `24,24 228.34375x36 unavailable=false`.
- macOS: `layout` lists `#2 [para]` and then `#6 [out]`, with no line for `#3`, `#4` or `#5`; `frame("link")` gives `0,0 0x0 unavailable=true`.
- Hover (fixed by #178): `tap link hover`, `tap out hover`, `tap para hover` count three hover changes on macOS, as on the web.

Expected: macOS answers `frame(id)` for an inline run as the web does (`getBoundingClientRect()`, viewport space), and the agent's `layout` lists inline runs with that rectangle. iOS and Linux the same wherever a `text`'s runs are laid out. Optional (a proposal): one rectangle per line box (`getClientRects()`) for a card aligned to the line under the pointer.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` answers no sources:

```contract
component InlineSpanFrame
  state hovered = false
  state hovers = 0
  state rect = "-"
  action onHover(on: bool)
    hovered = on
    hovers = hovers + 1
  action measure
    let g = frame("link")
    rect = `${g.x},${g.y} ${g.width}x${g.height} unavailable=${g.unavailable}`
  view
    column testId="root" padding=24 gap=12
      text width=260 font-size=16 testId="para"
        text "Before this, please read "
        text "see the earlier fix in pull request twelve" href="https://example.com/pull/12" hover=onHover id="link" testId="link"
        text " after it."
      text `hovered=${hovered} hovers=${hovers}` testId="out"
      text `frame=${rect}` testId="frame"
      button press=measure testId="measure"
        text "Measure"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Frame of the run | `bun exact.mjs agent web --size 600x400 layout "tap link hover" "tap measure" "tree frame"` | web (Chrome 154) | `0365ad1a4` | `#4 [link] Text 24,24 228.34375×36`; `frame=24,24 228.34375x36 unavailable=false` | (reference) | agent `layout`, `tree` |
| Same on macOS | `bun exact.mjs mac`, then the same drive with `agent macos` | macOS 26.6.2 | `0365ad1a4` | no `#3`–`#5` in `layout`; `frame=0,0 0x0 unavailable=true` | the web's rectangle within 1 pt (same font) | agent `layout`, `tree` |
| Hover (fixed by #178) | `agent macos --size 600x400 "tap link hover" "tap measure" "tap out hover" "tap para hover" "tree out"` | macOS 26.6.2 | `0365ad1a4` | `hovered=true hovers=3` | same as the web (passes) | agent `tree` |

### Acceptance criteria

- On macOS, `layout` lists `#4 [link]`, and `frame("link")` equals the web's rectangle within 1 pt with the same font, for a run that wraps over two lines.
- `frame()` of a run inside a scrolled container is in viewport space on both hosts, as for any node.
- An AppKit test measures a wrapped run and compares the union of its line rectangles with the reported frame.

### Constraints and related work

- Related: #133 (closed by #178, the agent hover only); #127 (positions outside actions and anchor positioning; distinct: this is `frame()` inside an action); #132's remainder (a selection's rectangles).
- Not tested: iOS, Linux; per-line rectangles on any host (the web reports only the union through `frame`).
- Workaround today: one hover box per word, which changes line breaking and has no single rectangle.

## Discussion at transfer

### ccheever — 2026-10-08T08:07:26Z

**Decision: Expose the union of inline-run rectangles through frame and layout.**

Keep open for a correctness fix.

An admitted inline run should have the geometry the web's getBoundingClientRect exposes. This is separate from reactive geometry or selection rectangles.

Use a wrapped link, scroll it, and compare viewport-space unions with Chrome using the same font. Reuse the native text geometry with #274.
