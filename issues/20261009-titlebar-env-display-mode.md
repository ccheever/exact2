# macOS: a title-bar area beside the traffic lights, and a full-screen fact (rest of #113)

**Status:** Open
**Systems:** host/apple macOS, viewport facts, kernel env
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/267

## Current scope

Add selected Window Controls Overlay env values/displayMode first, including fullscreen transitions and fallback. Choose manifest title-row/traffic-light configuration before arbitrary positioning. Saved-frame repair already landed.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #113. #164 fixed its first part (a `viewport-fit="cover"` window now restores its saved frame on every launch). Two parts remain, and #164 left both "for a ruling" as new vocabulary:

- **A title-bar area.** A desktop app with a custom title row (for example a sidebar header that shares a 52 pt row with the traffic lights) needs to set the row's height and the traffic lights' position, and to read the width the traffic lights reserve, so its content starts beside them. Electron's `titleBarStyle: "hiddenInset"` with `trafficLightPosition` does this; the web's equivalent is Window Controls Overlay's `env(titlebar-area-x | y | width | height)`.
- **A full-screen fact.** In full screen the traffic lights leave the window, so such an app drops the space it reserved for them and puts it back on exit. The page needs to know whether the window is in full screen, and to hear it change (the web: the `(display-mode: fullscreen)` media feature).

Without these an app adds native code that inserts an empty toolbar only to move the traffic lights, hard-codes their width, and observes the window's full-screen notifications itself.

### Current and expected behavior

On main `0365ad1a4` (the env names, the page and viewport facts and the macOS window code are unchanged on `e200397ec`):

- Under `viewport-fit="cover"` the title-bar band is 32 pt: `layout` reports `viewport 900×600 · safe-area 32 0 0 0`, while the authored row is `height = 52`. The traffic lights sit centred in the 32 pt band (about y = 16), not in the 52 pt row (y = 26), and nothing moves them.
- `env(titlebar-area-height)` and `env(titlebar-area-x)` are refused: `lower-attr-value`, "env() names safe-area-inset-top/right/bottom/left or viewport-segment-width/height/top/left/bottom/right x y" (`kernel/src/style/env.rs`).
- `app.json`'s `host.macos.window` takes `width`, `height`, `minWidth` and `minHeight` only.
- A `fullscreen` field on `exactPage()` fails the bake: `[bake-page-field] … exactPage does not answer; it answers visibilityState, onLine, canShare, canOpenFiles, hasFocus`. A `displayMode` field on `exactViewport()` fails the same way (`bake-viewport-field`). `contract vocab display-mode` / `fullscreen` find nothing.

Expected: the app can declare the title row's height and the traffic lights' placement, read the free title-bar area, and read a full-screen fact that flips when the window enters or leaves full screen (⌃⌘F, the green button, the View menu).

Proposals, not requirements: Window Controls Overlay's names, `env(titlebar-area-x | y | width | height)`, where on macOS `titlebar-area-x` is the width the traffic lights reserve, with an `app.json` field such as `host.macos.window.titleBar: { height: 52, trafficLights: { x: 16, y: 19 } }`; and `(display-mode: fullscreen)` as a viewport fact (`exactViewport().displayMode`), shown in agent `state`. On the web Chrome gives the `titlebar-area` values only to an installed app with `display_override: ["window-controls-overlay"]`; elsewhere they fall back as CSS `env()` does.

### Reproduction and evidence

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Title row beside the traffic lights | `bun scripts/exact.mjs new <dir>`; `app.json` `host.macos.window` 900×700; view: `main viewport-fit="cover" display="flex" flex-direction="column" width="100%" height="100%"` holding `row testId="titlerow" height=52 padding-left=90 align-items="center"`; `bun exact.mjs mac`; `bun exact.mjs agent macos --size 900x600 "clock data" "layout titlerow" "screenshot t.png window"` | macOS 26.6.2 | main `0365ad1a4` | `safe-area 32 0 0 0`; row `0,0 900×52`; the window capture shows the lights centred at about y = 16 pt, above the row's text at y = 26 | the row's height and the lights' position set by the app | `layout` output, window capture |
| Title-bar area values | the same row with `height="env(titlebar-area-height)" padding-left="env(titlebar-area-x)"`; `bun exact.mjs contract build <file> --json` | compiler | main `0365ad1a4` | `lower-attr-value` for both | accepted; on macOS the free title-bar area | compiler output |
| Full-screen fact | `shape Page` with `fullscreen: bool` (or `shape Viewport` with `displayMode: string`) read through `resource page = exactPage() as shape Page`; `bun exact.mjs web-build` | bake | main `0365ad1a4` | `bake-page-field` (`bake-viewport-field`): the fact is not answered | a fact that flips on entering and leaving full screen | bake output |

### Acceptance criteria

- A minimal app lays out a row with `height="env(titlebar-area-height)"` and `padding-left="env(titlebar-area-x)"`; on macOS `layout` shows the declared height and an AppKit test reads the standard window buttons centred in it.
- The app reads a full-screen fact; agent `state` shows it flip when the window enters and leaves full screen on macOS, and the web agrees with `matchMedia('(display-mode: fullscreen)')` in Chrome.
- iOS and Linux: the title-bar values fall back as `env()` does and the fact reads `standalone` (or the chosen value).

### Constraints and related work

- Related: #113 (closed after #164, which fixed the frame restore and states that the title-bar area, the manifest field and the full-screen fact are "new vocabulary and are left for a ruling").
- Related: LLP 1008 §9 (`viewport-fit=cover` mapping); `runner/src/page.rs`, `runner/src/viewport.rs` (fact lists).
- Not tested: entering full screen with real input (there is no fact to read); the frame restore (fixed by #164, not retested here).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:17:06Z

## Decision needed

**What blocks it (main `0365ad1a4`).** No rule refuses it; the vocabulary does not exist and #164 left it "for a ruling":
- `kernel/src/style/env.rs:318`: `env()` names only `safe-area-inset-*` and `viewport-segment-*`.
- `runner/src/page.rs:94-100`: `exactPage()` answers `visibilityState`, `onLine`, `canShare`, `canOpenFiles`, `hasFocus`; `runner/src/viewport.rs:404` has no display mode.
- `scripts/app.schema.json:401`: `host.macos.window` takes `width`, `height`, `minWidth`, `minHeight`.

**Options.**
- **A. The web's names.** `env(titlebar-area-x | y | width | height)` (Window Controls Overlay) for the free title-bar area, an `app.json` `host.macos.window.titleBar` (`height`, `trafficLights: { x, y }`) for the row and the buttons, and `(display-mode: fullscreen)` as `exactViewport().displayMode`.
- **B. Manifest and a page fact only.** `host.macos.window.titleBar` as in A, a numeric fact for the reserved inset (`exactViewport().titleBarInset`), and `exactPage().fullscreen: bool`. Smaller, but not CSS's names.
- **C. No change.** Apps keep native code: an empty toolbar to move the buttons, a hard-coded inset, and their own full-screen observer.

**Recommendation.** A. Every name already exists on the web (Window Controls Overlay, the `display-mode` media feature), so Chrome is the oracle for the values and the web host has them for free in an installed app; only the button placement needs a manifest field, as Electron's `trafficLightPosition`.

**Cost.** Four `env()` names in the kernel's env table, macOS computing them from the standard window buttons' frames (and following the window's zoom and full-screen transitions), one manifest field applied before the first frame, and one viewport fact flipped from `NSWindow`'s full-screen notifications; iOS and Linux return the `env()` fallback and `standalone`. An AppKit test for the buttons' placement and an agent row for the fact.

### ccheever — 2026-10-08T08:07:39Z

**Decision: Choose Window Controls Overlay env values and displayMode.**

Keep open with the bounded scope below.

Use the web's names for the free titlebar area and display mode. A macOS manifest field may configure the title row and traffic lights.

Implement the fact/env slice before arbitrary button positioning. Verify full-screen transitions and fallback env values; the frame-shrink bug in #113 is already fixed.
