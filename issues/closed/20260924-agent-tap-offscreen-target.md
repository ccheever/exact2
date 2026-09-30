# Agent tap activates a target whose middle is off the viewport

**Status:** Closed
**Resolution:** Native carriers now refuse off-viewport targets like the web, and deck smoke scrolls first; macOS/iOS rejection regressions, a live macOS fixture and the complete macOS smoke pass.
**Systems:** Agent API, Apple host, web host
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1012 §3; LLP 1035.003 (a contact is never substituted); Crew port report D6 (2026-09-24)

The Crew port (report of 2026-09-24, D6) found `tap` reporting success on a button under the iOS software keyboard. That is fixed: on iOS a tap whose target's middle is under the keyboard or another view is refused before anything changes (the keyboard stays up), success replies carry `pressed: <id|null>`, and the web carrier makes the same check with `elementFromPoint`.

One case was left as it was, deliberately. A target whose middle is below or beside the viewport, with nothing covering it, is still activated on iOS (the old `win.hitTest(p) ?? v` fallback, `AgentIOS.swift`), now with `offscreen: "<why>"` in the reply. Caltrain's `deck-toggle` sits below the 778 pt viewport and the iOS smoke taps it, and external scripts may depend on the same thing. No finger can make that contact, and LLP 1035.003 says action dispatch is never substituted for one. The web carrier refuses the same case (a click there reached nothing anyway), so the two carriers now disagree. macOS `tap` keeps the unconditional fallback (`AgentMac.swift`); there is no software keyboard there.

Charlie's call, one rule for every carrier: refuse off-viewport targets (and fix the smoke to scroll first), or scroll the target into view and then tap, as Playwright does. Done when iOS, macOS and the web give the same answer for a target outside the viewport and the smoke drives Caltrain's deck toggle through it.

## Fix verification (2026-09-30)

Native carriers now follow the existing web rule: refuse a target outside the
viewport before activating it; callers scroll first. The check precedes native
control activation. The iOS offscreen-success fallback is removed. Caltrain's
deck smoke scrolls its toggle into view before tapping.

The shared `ContextPositionIOSTests` checks ordinary, context-menu and double
click rejection without a `tapped` success on both native platforms. A compiled
fixture driven on macOS rejects a button at y=1150 in a 420×900 viewport; the
full macOS smoke passes, including opening the deck and activating its card.
UIKit reports 101 tests passed, including the new tests, though this run's
xcodebuild remained waiting after XCTest printed its completed suite.

Review follow-up: inline IDs now use the requested run's shaped fragments,
converted through presentation and canvas placement, instead of the paragraph
owner's midpoint. A visible fragment is chosen for wrapped runs. Regression
coverage reproduces and fixes both a visible run in an offscreen-centred
paragraph and an offscreen run in an onscreen-centred paragraph, and verifies
that a presentation translation moves the tap target. The three focused
context/tap tests pass on both macOS and UIKit.
