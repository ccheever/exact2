# LLP 1105: The status bar's style, declared from state

**Type:** RFC
**Status:** Draft r1, 2026-10-06, for design review (Astra, Grok)
**Systems:** the kernel schema (two props), Contract lowering (`contract/lower/src/tags.rs`), the iOS presenter and its view controllers, the standalone iOS adapter; macOS, Linux and the web take the props and do nothing
**Author:** Claude (Opus 5.5) for Charlie Cheever, who approved the feature
**Date:** 2026-10-06
**Related:** LLP 1069 §10 (system chrome: "status bar style and `theme-color`", unasked until now); LLP 1008 §9 (what a phone paints behind the status bar: the first root's background, `viewport-fit`); LLP 1034 §8 (per-subtree `color-scheme`); LLP 1035.001, LLP 1075.003 (routes, presentations, tabs)

## 1. The ask

The Bluesky clone shows a profile whose banner is dark. When the compact
white header slides in over it, the status bar's text must turn dark in the
same frame. Today the app has no say: the status bar takes UIKit's default,
which follows the window's appearance, not what is under it, and an app that
fakes it with UIKit's automatic scroll-edge styling lags about eight frames,
white on white. Expo apps set it per screen (`expo-status-bar`'s `style`).

## 2. Decisions

### D1 — A prop on any node, bound to state

```
column status-bar-style=(compact ? "dark" : "light") status-bar-animation="fade"
```

- `status-bar-style`: `"light"` (light text, for a dark background),
  `"dark"` (dark text) or `"auto"` (the platform's choice for the scheme
  under it, §D3). Expo's names, which mean the text's colour.
  Unset is the same as not declaring.
- `status-bar-animation`: `"none"` (the default: the new style shows in the
  frame the batch commits) or `"fade"` (UIKit's cross-fade, 0.3 s).
- Lowered to the props `statusBarStyle` and `statusBarAnimation` (schema
  `str`). Other values are a compile error.

A prop, not a command: it is state the app already has (the header is
compact, the lightbox is open), so it stays right across navigation, reloads
and the agent's clock without the app replaying a call.

### D2 — The topmost declaration that shows wins

The host reads every node that declares a style and shows (it is in the
window and nothing above it is hidden), and picks one, the way UIKit's
`childForStatusBarStyle` defers to what is on top:
1. **The highest presentation:** a node inside the topmost presented route
   (a sheet, a full-screen route, a lightbox presented as a route) beats one
   under it. The primary stack is the lowest.
2. **Within it, the most specific:** the deeper node wins (an overlay
   inside a route over the route's own declaration); between equals, the
   later in document order (painted on top).

So a profile route declares `"light"`, its compact header declares `"dark"`
while it shows, and a lightbox route declares `"light"` over both. A route
underneath a push, an unselected tab and a hidden overlay are not in the
window and do not count. Mid-transition both routes show; the result
settles when the transition ends (the host re-reads then).

### D3 — `auto` is the scheme under it

`"auto"`, or no declaration at all, takes the platform's default for the
appearance where the winner sits: inside a `color-scheme` subtree forced
dark (LLP 1034 §8) it is light text, forced light it is dark text, and
otherwise the system's (`UIStatusBarStyle.default`). With no declaration
anywhere, nothing changes from today.

### D4 — The same frame

The presenter re-reads the declarations after every batch's projection,
after a navigation or presentation transition ends, and when a modal is
dismissed. A change calls `setNeedsStatusBarAppearanceUpdate` on the root
controller (inside a 0.3 s animation for `fade`) in that turn, so the style
commits with the batch that flipped the state.

UIKit asks the topmost full-screen controller, or a presented one that
captures the status bar. Exact's controllers that UIKit can ask (the
adapter's root `Controller`, `ModalController`, which now captures it) all
answer the one resolved style, so whoever UIKit asks agrees. An embedder
that hosts `ExactView` in its own controller reads
`ExactView.statusBarStyle` and is told by `onStatusBarStyle`, as it is told
`onCanvasColor` and `onTitle`.

### D5 — The other hosts

- **macOS:** no status bar. The props are read and ignored.
- **Linux:** none either. Ignored.
- **The web:** a page cannot style the phone's status bar (only a
  home-screen web app's static `apple-mobile-web-app-status-bar-style`
  meta, which cannot follow state). Ignored. `theme-color`, which some
  browsers paint behind it, is a colour, not a style, and is a separate
  feature (LLP 1069 §10).
- **The agent** reports the resolved style in `layout` (`statusBar: light`),
  so a drive can check it without a screenshot.

## 3. Not doing

- **Hiding the status bar** (`prefersStatusBarHidden`). Nothing asks; it
  would be one more value later.
- **Android's navigation bar and status bar colour.** No Android host.
- **`theme-color`.** A colour for browsers' chrome, its own feature.

## 4. Tests

- Contract: the attributes lower to the props; a bad value is refused.
- iOS: the resolved style for a root declaration, a deeper overlay over its
  route, a presented route over the primary stack, a hidden overlay that
  does not count, `auto` under a dark `color-scheme` subtree; a state flip
  changes the root controller's `preferredStatusBarStyle` in the same
  `apply`; `fade` animates.
