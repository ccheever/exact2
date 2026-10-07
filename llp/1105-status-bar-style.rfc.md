# LLP 1105: The status bar's style, declared from state

**Type:** RFC
**Status:** Built r2, 2026-10-06. r1 had one blind design pass: Astra (max) NOT READY, Grok (xhigh) READY WITH CHANGES (§5); r2 takes both; Charlie's lead approved r2.
**Systems:** the kernel schema (two props), Contract lowering (`contract/lower/src/tags.rs`, `values.rs`), the iOS presenter (`ModalIOS.swift`, a small `StatusBarIOS.swift`), the standalone iOS adapter's root controller and `ExactView`; the web and JS targets skip the props; macOS, Linux and tvOS do nothing
**Author:** Claude (Opus 5.5) for Charlie Cheever, who approved the feature
**Date:** 2026-10-06
**Related:** LLP 1069 §10 (system chrome); LLP 1008 §9 (what paints behind the status bar); LLP 1034 §8 (per-subtree `color-scheme`); LLP 1035.001, LLP 1075.003 (routes, presentations, tabs)

## 1. The ask

The Bluesky clone shows a profile whose banner is dark. When the compact
white header slides in over it, the status bar's text must turn dark in the
same frame. Today the app has no say: the bar takes UIKit's default, which
follows the window's appearance, not what is under it. Expo apps set it per
screen (`expo-status-bar`).

## 2. Decisions

### D1 — Two props on any node, bound to state

```
column status-bar-style=(compact ? "dark-content" : "light-content") status-bar-animation="fade"
```

- `status-bar-style` names the **text**: `"light-content"` (light text, for
  a dark surface: a dark banner takes `light-content`), `"dark-content"`
  (dark text, for a light surface) or `"auto"` (§D4). React Native's words:
  `light`/`dark` would read as the surface, as `color-scheme="dark"` does.
- `status-bar-animation`: `"none"` (default: the new style shows in the
  frame its batch commits) or `"fade"` (UIKit's cross-fade).
- Lowered to the props `statusBarStyle` and `statusBarAnimation` (schema
  `str`). A literal outside its set is `lower-attr-value`; a bound value
  outside it is logged and counts as unset.

A prop, not a command: it is state the app already has (the header is
compact, the lightbox is open), so it stays right across navigation, reloads
and the agent's clock without the app replaying a call.

### D2 — Which screen is under the bar

The **scope** is what the bar sits over:
- the topmost presented route that covers it: a `fullscreen` route or a
  zoom (`.overFullScreen`) always; a sheet only on a compact-width screen
  at its `large` detent (at `medium`, and on an iPad, the screen behind
  still has the bar);
- else the committed route of the selected tab, with the root's own nodes
  around the route outlet (an authored header over the stack, an overlay).

A route under a push, an unselected tab and the screen behind a covering
presentation are outside the scope. (As built, a covering presentation's
scope is its navigation view; an unselected tab is out by not being in the
window or hidden, so a custom tab container that parks one on screen
unhidden would let it compete.)

### D3 — Within the scope, the declaration painted on top wins

Of the declarations in scope that show, the one painted last wins: a
descendant beats its ancestor's declaration (the compact header inside the
profile route over the route's own), and between two branches the one the
presenter paints above (its `z-index` rank, then its order among siblings,
as `PaintOrder` already orders them) wins. **Shows** means in the window,
not `display: none`, not `visibility: hidden`, and not a node UIKit is
showing a projection of instead (a popover or menu hidden for UIKit's own
menu, a row lifted into a context-menu preview, which is outside the
session's view and every presentation's); a declaration on viewless inline
text does not count; a route's header that the
navigation bar projects counts, at its route's place. No declaration in
scope is `auto` for the scope.

### D4 — `auto` is the scheme where the winner sits

`"auto"` on a node whose computed `color-scheme` is dark is light text;
light, dark text; unspecified, the platform's default
(`UIStatusBarStyle.default`), which keeps following the system's
appearance with no batch. It follows the appearance, not the pixels under
the bar. With no declaration anywhere, nothing changes from today.

### D5 — Committed state, applied in the batch's frame

The bar follows committed state. The presenter resolves the style once at
the end of each outermost batch's projection, synchronously, and calls
`setNeedsStatusBarAppearanceUpdate` in that turn (inside a 0.3 s animation
block that wraps only that call, for `fade`), so a flip shows in the frame
its batch commits. While a navigation or presentation transition is in flight (a push, a
pop, an interactive back swipe, a sheet being dragged, a presentation
animating), the last committed style stays, and the transition's end
(`didShow`, the modal's appearance or disappearance) applies the new one: a
push changes as it lands, a finger-driven dismiss when it commits. (r2 had a
push change as its animation started; as built, every transition takes the
one rule, which needs no committed-route bookkeeping.) Following the finger
would need a style per route and the transition coordinator; not now. A
sheet's detent change re-resolves without a batch. The animation is the
winner's.

### D6 — Who UIKit asks

UIKit asks the window's root controller, or a presented controller that
covers the screen. In the standalone adapter those are the root
`Controller` and `ModalController` (`.overFullScreen` and `.pageSheet`);
the navigation, tab and route controllers are children nobody forwards to.
Both override `preferredStatusBarStyle` to return the one resolved style,
so whichever UIKit asks agrees; neither forwards `childForStatusBarStyle`,
and `modalPresentationCapturesStatusBarAppearance` stays unset. The update
is sent to the root and to the topmost `ModalController`. An embedder that
hosts `ExactView` in its own controller reads `ExactView.statusBarStyle`
(style and animation) and is told by `onStatusBarStyle`, called with the
current value when set, as `onCanvasColor` is. tvOS compiles the code and
has no bar: the overrides are inert there.

### D7 — The other hosts and the agent

- **macOS, Linux:** no status bar. The props are ignored.
- **The web and the JS target:** a page cannot style the phone's status bar
  (only a home-screen app's static `apple-mobile-web-app-status-bar-style`
  meta, which cannot follow state). Both skip the two props rather than
  writing them as `data-` attributes. `theme-color` is a colour, a separate
  feature.
- **The agent:** iOS `layout` gains `statusBar`, the resolved text style
  (`light-content`, `dark-content` or `default`) and the node it came from.
  It is Exact's request, not a reading of the screen. Other hosts omit it.

## 3. Not doing

Hiding the status bar (`prefersStatusBarHidden`); following a finger
through a transition; Android's bars; `theme-color`.

## 4. Tests

- Contract: the attributes lower to the props; a bad literal is refused.
- iOS: the winner for a root declaration, a descendant over its route, a
  later-painted overlay over a deeper declaration in another branch, a
  hidden or `visibility: hidden` overlay, a `fullscreen` route over the
  stack, a sheet that does not cover the bar, `auto` under a dark
  `color-scheme` subtree, no declaration (`.default`); a flip changes
  `preferredStatusBarStyle` on the controller the walk reaches (the root,
  or the presented `ModalController`) within the same `apply`; a transition
  in flight keeps the committed style; `fade` animates.

## 5. Review (r1)

Astra (max) NOT READY, Grok (xhigh) READY WITH CHANGES. Taken:
- **Who is asked (both):** only the root and `ModalController`; one stored
  style; no capture flag; update the root and the top modal (D6).
- **The order (both):** paint order, not tree depth; scope by what covers
  the bar, sheets by detent (D2, D3).
- **Transitions (both):** committed state; keep the last style while one
  is in flight; snap on commit (D5).
- **"Shows" (Astra):** projections excluded, projected headers included (D3).
- **`auto` (both):** from the winner's computed scheme, unforced stays
  `.default`; none in scope is `.default` (D4).
- **Names (Grok):** `light-content`/`dark-content` (D1).
- **Validation, embedder, web skips, agent report (both)** (D1, D6, D7).
- **Same frame (Astra):** resolved synchronously at the end of the
  outermost batch; screen-capture verification stays an acceptance check.
