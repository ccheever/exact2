# LLP 1130: A page under the status bar

**Type:** RFC
**Status:** Accepted as built, 2026-10-18, after three design rounds and three code rounds (Astra, Grok; records in `~/audits/exact2-no-tells-2026-10-11/llp1130/`). `PageEdgesIOSTests` pass in the UIKit run (`build.mjs --test --ios`), whose one failure, `StoreRefreshTests`, fails the same without this change. Charlie asked for the three QUEUE items and then "go back to iOS work"; this is the iOS item QUEUE.md names "An iOS page is cut at the status bar".
**Systems:** the iOS viewport (`host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift` `fit`), the hosts' covers (`NavigationBarIOS.swift` `reportCovers`, `kernel/src/kernel/cover.rs`), the iOS scrollers UIKit insets.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** the author, from 2026-10-18
**Related:**
- LLP 1116.005 D4 (stopwatch: "the page scrolled under the status bar with no backdrop"), whose third bullet this builds.
- LLP 1075.003 §3.5 (a native container takes the whole view); LLP 1123 D3 (a route's content root keeps its clearance, and a scroller that reaches an edge runs under it); LLP 1116 D2 and 1116.000 D1.2 (a route's bottom scroller runs under the home indicator and UIKit insets it); LLP 1123 D1 (the document is the session view's).
- Apple, [`UIScrollView.contentInsetAdjustmentBehavior`](https://developer.apple.com/documentation/uikit/uiscrollview/contentinsetadjustmentbehavior), [`adjustedContentInset`](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset), [`UIScrollEdgeEffect`](https://developer.apple.com/documentation/uikit/uiscrolledgeeffect); Human Interface Guidelines, [Layout: safe areas](https://developer.apple.com/design/human-interface-guidelines/layout#Guides-and-safe-areas).

## Summary

On iOS a page with no navigation bar is laid out inside the safe area, so its content stops
in a hard line at the foot of the status bar and above the home indicator. A hand-built
screen's scroll view fills the screen: its content starts below the status bar, scrolls
under it (which iOS 26 and later blur with the scroll edge effect) and runs under the home
indicator. Pages inside a native container already behave this way (LLP 1123 D3). This RFC
gives a page with no container the same result by moving where it is drawn, not how it is
laid out.

## 1. What happens now

`ExactView.fit` frames the viewport to the whole view when the page asked for
`viewport-fit=cover` or a native container shows (`wantsWholeView`), and to the safe area
otherwise. In the second case nothing the page draws reaches the status bar or the home
indicator. A throwaway app with one `main` holding a full-height `scroll` of forty rows,
scrolled 200 points on the iOS 27 simulator (iPhone 18 Pro), shows white above y = 62 and
below y = 840, with the rows cut at both lines
(`~/audits/exact2-no-tells-2026-10-11/llp1130/before.png`).

Under `viewport-fit=cover` (Color Gallery) the same scroll view reaches the top, and UIKit
draws its edge effect under the status bar with no code of Exact's. Under a container a
route is covered for the safe area (`routeEdges`), its top scroller goes under the bar
(`collapse`), and its bottom scroller runs to the screen's foot (`reachBottom`,
`bottomScroller`), where UIKit insets it.

## 2. Decisions

Two review rounds (Astra, Grok) settled the mechanism: **layout does not change; only where
the page is drawn does.** Draft 1 put covers on the root, which are padding and grow a
content-box `height: 100%` root. Draft 2 grew the layout offer for a page with an edge
scroller, which moves the padding onto the scroller, changes every root's offer (one offer
serves them all, `host/apple/src/layout.rs`), and can oscillate when a root's fill depends on
the offer. Draft 3 sizes and lays out every root exactly as today and changes native frames
only.

### D1 — the document is drawn in a scroll view that takes the whole view

On iOS (not tvOS, whose safe area is the overscan margin), a document with no native
container and no `viewport-fit=cover` keeps the **page's area** it has today, the session
view's bounds inset by its safe area: the size the kernel is told, `viewportSize`, the
window's size (viewport units, the fold) and `env()` (zero). Its scroll view
(`presenter.viewport`) takes the session view's whole bounds instead, and the document view
(`presenter.root`) sits at the area's origin inside it. The content size is the document's
(`fitDocument`, floored at the area as today) plus the safe area's four margins. So:

- `contentOffset` (0, 0) is the rest, and the scroll range is today's: the page's `scrollTop`
  (`ScrollPumpIOS` reports `contentOffset`) reads as it does now and needs no conversion.
- A page that fits does not scroll: its content is the whole view's size.
- While the document scrolls, its content passes under the status bar and the home
  indicator; UIKit draws the edge effect over the document's scroll view with no code of
  Exact's (D4).
- The bands show the viewport's background, which is the first root's ground already
  (`platformBackground(in: root)`).

A point of the kernel's viewport (a box, a tap, `clientX`, the agent's points) and a point
of the scroll view's content differ by the page's origin, the scroll plus the margins
(`UIScrollView.pageOrigin`, `PageEdgesIOS.swift`), where they differed by the scroll alone:
every conversion that added or took off `contentOffset` (taps, `AgentIOS` `box`,
`AgentPanIOS` `windowPoint`, pointer `client`, presence, owners' top-layer boxes, press
feedback, surface input, accessibility frames) takes `pageOrigin` instead, and the size the
agent reports is the area's (`pageSize`). A picture of the page (`screenshot` without
`window`) is the area, so its top-left is a box's (0, 0). What reads what is visible
(text rasters) keeps the scroll view's bounds, bands included, which now show content.

### D2 — the keyboard

The keyboard's overlap is measured against the page's area (`Presenter.pageFrame`), not the
scroll view's frame, so it is what it is today under every policy. Under `resizes-visual`
it is the document's `contentInset.bottom`; the content's bottom margin keeps the home
indicator's band, so the scroll range is today's. What is shown clear of the keys (a field's
reveal, `scrollIntoView`) is measured in the area (`UIScrollView.pageBounds`), not in the
scroll view's bounds, whose foot is under the keys and the home indicator. Under
`resizes-content` the area ends at the keyboard's top, as the viewport's frame does today,
and so does the scroll view: no bottom margin while the keyboard is up.

### D3 — an edge scroller's scroll view reaches into the band

An Exact scroller (`overflow-y: scroll | auto`) laid out in the document, with nothing
between them that clips, transforms, scrolls or hides, whose box starts at the area's top
has its scroll view extended up through the status bar's band (`pageBand`): its frame starts
at the view's top and its `contentInset.top` grows by the band, so its content shows where
it did and scrolls under the status bar, as a hand-built screen's table view does. One whose
box ends at the area's foot, in a document no taller than the area, reaches down through the
home indicator's band the same way. Its box, its background and its layout are unchanged;
the scroll view stays `.never` and the insets are the host's (no `.always`, no `HostCover`).

- The band is a UI inset, not content: CSS `scrollTop` 0 is the rest under it
  (`scrollTopInset`, which every write, retention, scroll event and agent read goes
  through), the kernel is told the offset past it, the CSS port (`clientHeight`) is the box,
  and the content is floored at the box (`scrollFloor`), so content that fits does not
  scroll by the band. A virtualized list's port is its box, as its offset corrections
  assume, so a row that has scrolled under the status bar is kept only as long as the list
  keeps rows past its port (`QUEUE.md`).
- When the band changes (a rotation, an iPad window with no top inset, a line put above the
  scroller, a new scroll view) the logical `scrollTop` is kept.
- A touch in a band over such a scroller reaches it, as on a hand-built table view: a drag
  that starts on a row under the status bar scrolls it. The document view takes points in
  the page's margins and a scrolling node points out to its band (`PlainView.hitTest`,
  `NodeView.hitTest`); the status bar's tap and the home indicator's swipe stay the system's.
- A list UIKit draws (a grouped list's collection view) reaches into the bands the same way:
  the band is its scroll view's, native or Exact, and a native one leaving gives it back
  first, keeping `scrollTop`.
- It is reconciled after each batch's frames and when the margins change, on its own path
  (navigation's `sync` returns early with no routes).
- Offsets a layout in flight saved to restore (`beforeLayoutScroll`, `followedScroll`) move
  with the band, and a sticky box inside the scroller is placed by the offset the kernel is
  told.

### D4 — the edge effect

UIKit draws the top scroll edge effect under the status bar for a scroll view that reaches
it, with no code of Exact's. Measured on the document's own scroll view (inset adjustment
`.never`, no authored edge effect): a `viewport-fit=cover` page whose document starts under
the status bar shows its first line blurred there
(`~/audits/exact2-no-tells-2026-10-11/llp1130/document-cover-probe.png`). It is measured
again on an unstyled node scroller in the gate (Color Gallery writes
`-exact-scroll-edge-effect="soft"`, so it proves nothing about the default). An authored
`-exact-scroll-edge-effect` wins as it does on any scroller.

### D5 — what is left out

- `viewport-fit=cover`: unchanged; the page asked to draw under the bars itself.
- The agent's explicit window size: unchanged; its scale and centre stay fitted to the safe
  area.
- The agent's default (authored) chrome is not excluded: with no container, a person and
  `agent ios` see the same page.
- Presented sheets and dialogs: they are not the document and keep their own insets.
- A page inside a native container: its routes already do this (LLP 1123 D3). A route there that
  says it has no bar (`navigationBar="none"`) puts its first scroller under the status bar
  as a route with a bar puts it under the bar (`collapse`), its inset the status bar's.
- macOS and tvOS.

## 3. The gate

- The throwaway app's picture after: rows under the status bar, blurred, and under the home
  indicator; scrolled to the top, the first row below the status bar as before.
- XCTests: the document view's place and content size (a page that fits does not scroll; a
  tall one's range is today's); an edge scroller is found at the area's top and foot and not
  under a clipping or transformed ancestor or with anything above it; its `scrollTop` 0
  rests under the band and survives the band going (rotation); the keyboard's inset in
  both policies; the agent's explicit size and `cover` unchanged; content-box
  `height: 100%` roots, `100vh` roots and two roots lay out as before.
- The throwaway app driven by the agent before and after: taps land on the row named, the
  document's and the scroller's `scrollTop` read as before, the keyboard in both policies.
- The iOS smoke (`scripts/smoke.mjs ios`).
- Apps that run on iOS with no container, before and after: Metronome (a short root),
  Messages Stress (a header above its list: no top band), Color Gallery (`cover`,
  unchanged).
- The six checks, the UIKit XCTests (`build.mjs --test --ios`), and the tvOS compile.
