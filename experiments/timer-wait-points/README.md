# Timer wait points: the platform behavior LLP 1147 rests on

LLP 1147 lets a Contract timer wait for the next frame, a UI update in progress, the
end of a scroll, or idle. Its hosts read platform signals (scroll state, animation
frames, idle callbacks, update links) whose behavior at the edges decides whether a
held timer runs too early or never. These probes measure that behavior in real
browsers, an iOS simulator and a compiled Exact app, so the RFC cites measurements
rather than readings of documentation. They test the platforms, not this repository's
code, so they are experiments and not part of any test lane (`docs/testing.md`,
"Delete vacuous tests"); the RFC's §5 quotes their results.

| Directory | What it measures | How to run it |
|---|---|---|
| `web/` | Chromium, Firefox and WebKit through Playwright: scroll and frame order, `scrollend` for a removed scroller and a held finger, animation states and their end signals, idle callbacks during animation, an offscreen cross-origin iframe, a blocked main thread, how long a scroll holds still before it ends | `bun experiments/timer-wait-points/web/probe.mjs [chromium\|firefox\|webkit …] [--only probe,…]` |
| `ios/` | a plain UIKit app on an iOS simulator: a passive `UIUpdateLink`, run-loop modes during tracking and deceleration, scroll state across a held drag and backgrounding, the display link's rate | its `README.md` |
| `compiled/` | Exact's Messages app on an iOS simulator under LLP 1144's fling report: whether a precise timer's batches land during a fling today | its `README.md` |

Each writes its results under its own `results/`. A browser probe returns plain data
per engine; `results/<engine>.json` records the engine's version and the date with
them. The probes use the browsers `scripts/agent-launch.mjs`'s `suiteBrowser` pins
(Playwright's, as `package.json` sets). A run with `--only` merges into the engine's
earlier results, and `ran` gives each probe's browser version and date.

## The web probes

`support` records whether `requestIdleCallback`, `scrollend`, `getAnimations` and
`ScrollTimeline` exist. `scrollThenFrame` records, during a smooth scroll, how long
after each `scroll` event the next animation-frame callback runs: HTML runs a
rendering update's scroll steps just before its frame callbacks. `removedScroller`
removes a scroller at its first `scroll` and records whether `scrollend` comes.
`animations` reads `playState` for a CSS animation at playback rate 0 and for a
scroll-driven one, and records how a CSS animation and an `Element.animate()` one
announce their end: document-level events, `Animation` events, and their `finished`
promises, including one cancelled. `idleDuringAnimation` counts `requestIdleCallback`
calls and their budgets while an animation-frame loop moves a box. `offscreenIframe`
loads a cross-origin iframe (two loopback origins), then counts its animation frames
and 100 ms timeouts for two seconds on screen, off screen and back, with what an
`IntersectionObserver` on its root, on a 1 px fixed corner element and on a fixed
element covering the viewport say, and `visibilityState`; it also measures the frame
with its top 100 px scrolled out of view, and with the child's own document scrolled
2,000 px while the frame is on screen. `scrolledDocument` lays a top-level page out as
Exact's `scroll document` does (`html` at 100% height) and reads the same three
observers before and after scrolling it 2,000 px. `heldDrag` (Chromium, through CDP
touch input) drags a scroller, holds the finger still for 600 ms and lifts it, and
records when `scrollend` comes. `blockedMainThread` blocks the main thread for 1.2 s
at a scroll's first event and records how late an animation frame asked for before the
block runs, and where the scroll is when `scrollend` comes. `stillBeforeEnd` records
the scroller's position in every animation frame of ten smooth scrolls, three wheel
scrolls and, in Chromium, three touch flings: the longest time it held one position
before moving again, and from its last movement to its `scrollend`. It also replaces a
smooth scroll with an instant one to the position it has reached and records whether
it moves on and whether `scrollend` comes.

The wheel half of `blockedMainThread` is inconclusive: Playwright's wheel input waits
on the renderer, so the scroll does not get far, and Firefox timed out. Its smooth
half is what the RFC cites. The wheel runs of `stillBeforeEnd` are inconclusive for
the same reason.