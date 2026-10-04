# Pitfalls when writing exact2 apps

**Adding an entry:** only a pitfall you hit and reproduced on current `main`, written
as symptom → cause → what to do, with where it was found. Delete the entry in the
change that fixes the footgun, or that makes the compiler, runtime or driver
diagnose it. **Candidate diagnostic** marks one that could become a check cheaply.

Read [the agent guide](contract-for-agents.md) first. This list holds what that
guide's rules don't make obvious.

## Layout

- **An image tile grows to its picture's size.** An album tile in a flex row became
  900×1200 pt. Cause: a flex item's automatic minimum is its content size (CSS), and
  an image's content size is its intrinsic size. Fix: give the image or its flex
  parent `min-height=0` (`min-width=0` across a row), or position it absolutely in
  a sized box. (Signal Clone DIARY, build 7.)
- **A sized `symbol:` image is stretched.** An ellipsis became three tall bars.
  Cause: `object-fit` defaults to `fill`, as for `<img>`. Fix: add
  `object-fit="contain"` to every symbol with a `width`/`height`. (Signal Clone,
  2026-10-02.)
- **On iOS the whole window scrolls when the keyboard opens, and the header goes
  with it.** Cause: without `interactive-widget`, the root is not resized for the
  keyboard. Fix: `interactive-widget="resizes-content"` on the root, or
  `overlays-content` with a `role="toolbar" toolbarPlacement="keyboard"` for a
  toolbar that rides the keyboard without relayout (LLP 1008 §9.1). (Signal Clone.)

## Lists and scrolling

- **A transcript or feed should open at its newest row.** Writing `scrollTop` to a
  huge number lands short on a virtualized list by the estimate error of the rows
  it has not built (84 pt with `estimated-item-height=52`). Fix:
  `scroll-start="end"` on the `list` (LLP 1010 §6.5), not a `scrollTop` write or a
  `scrollIntoView` after the first command. (Signal Clone, build 4.)
- **The app works hard at rest.** Cause: it commits state on a timer (a clock
  written every 250 ms), so every tick is a batch and the host runs its whole
  post-apply pass (navigation, controls, menus, every scroller's position) four
  times a second; before `32805146` this also cut the reader's scrolling. Fix:
  write state only when it changes (a minute-resolution clock; poll fast only while
  something is in flight), and remove `scroll=` handlers left over from
  experiments: each commits per scroll frame. (Signal Clone, build 2; QUEUE has
  the host side.)

## Native presentation and navigation (iOS)

- **Edge-swipe back does nothing.** Cause: the pop gesture presses the control named
  by the root's `navigationBack`; with no such enabled control in the active route
  it is refused (the log says "back gesture refused"). Fix: `navigationBack="back"`
  on the root and an `id="back"` button on every pushed screen (LLP 1038 §6).
  **Candidate diagnostic:** the compiler could warn on a stacked route with no
  control of that id.
- **The agent's screenshots and tree don't show the native bars.** Under
  `scripts/agent.mjs` the navigation bar, tab bar, `UIMenu`s and header search are
  not presented; the authored header, tablist and popover paint instead, by design.
  To see native chrome, launch normally and take `xcrun simctl io <udid>
  screenshot`. (Signal Clone, builds 5 and 10.)
- **An overlay's backdrop stops at the navigation or tab bar.** Cause: content
  inside a route draws under the native bars. Fix: render full-screen overlays
  (menus, action sheets) as root children after the tab container, or as a
  `navigationPresentation="fullscreen"` route. (Signal Clone, build 10.)
- **With `viewport-fit="cover"`, route content goes under the native bar.** Cause:
  the bar's cover is added to the route's padding, but a cover-fit root has no top
  safe area. Fix: put `env(safe-area-inset-top)` on the route column, not on each
  authored header. (Signal Clone, build 5.)

## Input

- **A hold's `pointerup` never arrives.** Cause: the press started on a node that a
  state change replaced during the hold; the up is not delivered to a node that no
  longer exists (declared in LLP 1005). Fix: keep the pressed node across the state
  change (change its contents, not which branch renders it).
  **Candidate diagnostic:** the runtime could log a dropped up in development.

## Driving and testing

- **Every date in a screenshot is 1 January 2026** (31 December 2025 west of UTC).
  Cause: the agent's clock starts at `2026-01-01T00:00:00Z`, in UTC. Fix: `--epoch <ISO time> --time-zone <zone>` on
  `scripts/agent.mjs` for dates that read as intended and stay reproducible.
- **`axe` stops delivering taps after a long press.** After `axe touch --down --up
  --delay`, a following `axe tap` reaches no window; `axe touch --down --up`
  does. Use the agent's own taps where it can; with `axe`, tap by touch.
  (Signal Clone, build 10.)

## Working on exact2 itself

- **Conformance fails on apps you didn't touch.** Cause: `host/web-js/conform.mjs`
  compares against wasm dists under `--wasm-root` (default `/tmp/e3-wasm`, shared by
  every checkout), and without `--build` it uses whatever another checkout or an
  older commit left there. Fix: name the apps and pass `--build` with a
  `--wasm-root` of your own (`conform.mjs <app> --build --strict --wasm-root
  /tmp/<yours>`). With no apps named, an empty root compares nothing and passes.
- **A web size or speed number is inflated.** Cause: a development web build
  carries the agent adapter, install pages and the source map. Fix: measure the
  release: `host/web-js/build.mjs <app> --plan <wasm bake>/app.plan --production`,
  as `scripts/deploy.mjs` builds it.
- **Host code must never set a scroll offset during a pan or fling.** An absolute
  `contentOffset` write while `isTracking`/`isDecelerating` cuts the reader's
  motion: frame rate holds, the motion is wrong ("janky but not dropping frames").
  Defer follows to the end of the drag or deceleration, and apply anchoring and
  estimate corrections as relative adjustments in the same layout pass (Signal
  Clone evening of 2026-10-02; `32805146`, `c03685dc`).
