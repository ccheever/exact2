# LLP 1123: Content owners — one viewport per session, local geometry and safe areas per native container

**Type:** RFC
**Status:** Review (revised on LLP 1115.002 D8.2's host blocks); stage 2 (D3, D4, D9) accepted (Charlie, 2026-10-18)
**Systems:** Kernel (what a host block gains for an owner: local insets, the primary scroller; clearance inside the block; rendering ancestry for public geometry; `var(--exact-safe-area-inset-*)` as a term of a calculation tree); Runner (`client_rect`, `elementFromPoint`, `exactSurface` `container:` records); Apple host (iOS, iPadOS, tvOS: `ExactViewIOS.fit`, `ModalHost`, `NavigationHost`, `NavigationBarIOS.reportCovers`, keyboard ownership; macOS windows and dialogs); Web host and JS target (owners for emulated chrome, `nav-chrome.js`; registered custom properties in `index.html`); Linux, Windows (flat hosts); Contract (`-exact-safe-area`, `navigationSizing`, `keyboardAvoidance`, `--exact-safe-area-inset-*`, the layout regions' `env(-exact-<region>-inset-*)`); Agent (coordinates, hit testing, screenshots of presented content)
**Author:** Claude (Opus 5.5) for ide
**Implementer:** stage 2: Krzysztof Magiera, from 2026-10-18, reworking #618 into D4's `-exact-safe-area` with the top and bottom executors first; the rest to be assigned on acceptance
**Date:** 2026-10-08
**Revised:** 2026-10-13
**Related:**
- **Exact LLPs.** 1001 (the environment, covers, §2's calculation trees; this declares a deviation there); 1007 §10 addendum, "Modal routes" (modal routes on the web); 1008 §9 (native navigation, modal routes, the safe areas and `viewport-fit`); 1035.001 (native interaction ownership: sheets, focus, the keyboard in the active container's space); 1039 (viewport facts; D5 no container queries; amended narrowly here); 1069.011 (`--exact-accent`, the registered custom property this copies); 1075.003 (native platform control: each route owns its content area §3.6, `data-*` §3.3, what Exact owns §3.5, macOS §3.11; its safe-area semantics revised here); 1078 (foldables follow the web); 1081 (names Exact invents); 1105 (status bar style); 1115.002 D8.2–D8.3 (a containing block a host gives a subtree, the mechanism an owner is built on).
- **Web standards.**
  - [CSS Environment Variables 1](https://drafts.csswg.org/css-env-1/): `env()` is document-wide.
  - [CSS Values 4 §10](https://www.w3.org/TR/css-values-4/#calc-notation): `calc()`, `min()`, `max()`, `clamp()`; §10.8 limits; §10.12 range clamping.
  - [CSS Variables 1](https://www.w3.org/TR/css-variables-1/): custom properties inherit per element; a cycle is invalid at computed-value time. [CSS Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api/): `@property` registration with a syntax, inheritance and an initial value.
  - [CSS 2 §10.1](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details): containing blocks. [CSS Backgrounds 3 §2.11.1](https://www.w3.org/TR/css-backgrounds-3/#root-background): the root's background propagates to the canvas.
  - [CSS Position 4](https://drafts.csswg.org/css-position-4/#top-layer): the top layer. [HTML modal dialogs and inert subtrees](https://html.spec.whatwg.org/multipage/interaction.html#modal-dialogs-and-inert-subtrees).
  - [CSS Viewport 1](https://drafts.csswg.org/css-viewport/#interactive-widget-section): `interactive-widget`. [Virtual Keyboard API](https://www.w3.org/TR/virtual-keyboard/).
  - [WebKit, "Designing websites for iPhone X"](https://webkit.org/blog/7929/designing-websites-for-iphone-x/).
- **Apple documentation.** Linked where each API is discussed in §"Apple's APIs and the HIG". iPhone Duo and iOS 27.1 material is beta-SDK evidence, not hardware verification.
- **Lab.** Each stage is verified on the iPhone Duo (open and closed) and an iPhone simulator, natively and in Safari, with a lab app outside the repository that has a screen per case in §"Cases" it covers. `apps/duo-lab` stays LLP 1078's fixture.

## Summary

A web page has one viewport, and its safe area is the root's. A native app is made of screens and view controllers, each with its own size, safe area and keyboard overlap.

exact2 keeps the web's model for the document: one viewport per Exact session (`exactViewport()`, `vw`/`vh`, media facts, `env()`), never changed by a sheet. Below that it adds the native model. Every native container that hosts Exact content is a **content owner**: a route, a sheet or full-screen presentation, a tab panel, or an app-owned container. Its root is given a host block (LLP 1115.002 D8.2), the containing block its content is laid out in, and the chrome the author did not write (bars, a side strip such as the iPhone Duo's, a sheet's edges, an embedder's reserved space) is cleared automatically. That clearance is added to authored padding, as SwiftUI adds the safe area to `padding()`, credits padding the author already wrote with `env()` or `var(--exact-safe-area-inset-*)`, and can be turned off per edge with `-exact-safe-area`. Authors read an owner's local insets as four host-set, registered, inherited custom properties, `--exact-safe-area-inset-top|right|bottom|left`, and logic that must branch on an owner's size reads a fenced `container:` fact.

The rule is the same at every level and on every host: **the host lays out around its own chrome; `env()` reports what is left for the document's author.** Mobile Safari does exactly this on the Duo (§"Evidence").

## Motivation

- **A sheet resizes the document.** While a sheet is up, `ExactViewIOS.fit` measures the viewport from the sheet (`presenter.modals.coordinateView`), and `ModalHost.present` moves `presenter.viewport` into the sheet's controller. The whole document is laid out at the sheet's size: on the open iPhone Duo, a two-column screen under a 653-point sheet becomes one column, and its layout changes again when the sheet closes.
- **Root fitting follows presented chrome and bar visibility.** `NavigationHost.wantsWholeView` counts every navigation controller that shows its bar, presented ones included, so a sheet that shows a bar changes how the document behind it is fitted, and hiding a header changes it too. Only the window's size, which the viewport units read, takes the root's containers alone (`rootWantsWholeView`, LLP 1075.003 §9.11).
- **Clearance exists only for navigation routes, and only at rest.** `NavigationBarIOS.reportCovers` measures a route's bars after a transition settles. A sheet, a tab panel or an embedder's container has no clearance, and a pushed route slides in without it.
- **Authors cannot express local insets.** Nothing names "this sheet's safe area" or "this screen's size". Apps use `exactViewport().width`, which is wrong inside a sheet, and the web's `max(16px, env(safe-area-inset-bottom))` is refused.

## Evidence: Mobile Safari on the iPhone Duo (simulator, iOS 27.1, 2026-10-08)

A probe page printed `innerWidth`, `env(safe-area-inset-*)` resolved through CSS, `window.viewport.segments` and `navigator.devicePosture`. Two static copies of it differed only in the viewport `<meta>`, one with `viewport-fit=cover` and one with `auto`, and every reading below was the same under both.

| Pose | `innerWidth` | Right inset | Left inset | Bottom inset (`cover` and `auto`) | Segments, posture |
|---|---|---|---|---|---|
| Cover screen (466 × 678, 84-point trailing strip) | **382** | 0 | 0 | 0 | not exposed |
| Unfolded (951 × 669) | 951 | 0 | 0 | 34 | not exposed |

On the cover screen, Safari sizes the page beside the bar strip and puts its own controls (back, bookmarks, new tab, tabs) and the clock in it. The page sees no side inset with either `viewport-fit` value. Safari treats the strip as host chrome: it lays out around it and reports to the page only what is left. Unfolded, Safari lets the page reach the bottom of the screen under either value and reports the home indicator's 34 points as `env(safe-area-inset-bottom)`, `auto` included: Safari's `auto` does not fit the bottom edge. Exact's `auto` does (D1). Neither `window.viewport.segments` nor `navigator.devicePosture` exists in this Safari. That is this LLP's rule, observed in the platform's own browser.

## Cases

Every case below is one the HIG or Apple's APIs define. Each row gives the behavior this LLP requires, the decision that delivers it, and the screen in `apps/safe-areas` that tests it. "Local" is the owner's own safe-area inset (D3). "Document" is the session's viewport and `env()` (D1).

### Root and device

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C1 | Status bar, Dynamic Island, home indicator, portrait | [HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout), [`safeAreaInsets`](https://developer.apple.com/documentation/uikit/uiview/safeareainsets) | The root is fitted by D1's three rules: `auto` without a native primary container is fitted inside the safe area (`env()` 0); `auto` with one covers the screen (`env()` 0; content is in owners, which clear); `cover` covers the screen (`env()` is the insets; the author pads with it). No automatic clearance at the root. | D1, D3, D9 | `root-auto` (a separate fixture without routes, since the lab is tabbed), `root-cover` |
| C2 | Landscape side insets (sensor housing) | [HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout) | As C1, on the left and right. | D1 | `root-cover` rotated |
| C3 | Status bar hidden or shown | LLP 1105, [`prefersStatusBarHidden`](https://developer.apple.com/documentation/uikit/uiviewcontroller/prefersstatusbarhidden) | The top inset changes; owners republish (D2); content moves once, with no snap after the change settles. | D2 | `status-bar` |
| C4 | Rotation, size-class change | [`viewWillTransition(to:with:)`](https://developer.apple.com/documentation/uikit/uicontentcontainer/viewwilltransition(to:with:)) | The document and every owner resize; the predicted size is never combined with current insets. | D1, D2 | any screen, rotated |
| C5 | iPad Split View, Slide Over, Stage Manager; the Duo tiled to half | [Multitasking](https://developer.apple.com/design/human-interface-guidelines/multitasking), [`effectiveGeometry`](https://developer.apple.com/documentation/uikit/uiwindowscene/effectivegeometry) | A window resize is a real resize of the document and of every owner; nothing reads the screen's bounds. | D1, D2 | `columns`, tiled |
| C6 | An Exact session embedded in a host app's controller | LLP 1031, [`additionalSafeAreaInsets`](https://developer.apple.com/documentation/uikit/uiviewcontroller/additionalsafeareainsets) | The session's document is its view's size. Chrome the embedder draws over Exact content is reserved with `additionalSafeAreaInsets` on the owner's controller and cleared like a bar. | D1, D3 | sample host fixture |

### Bars and navigation

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C7 | Navigation bar, inline title | [`UINavigationBar`](https://developer.apple.com/documentation/uikit/uinavigationbar) | Content starts below the bar; the route's background reaches under it. | D3 | `nav-inline` |
| C8 | Large title that collapses with the primary scroller | [`prefersLargeTitles`](https://developer.apple.com/documentation/uikit/uinavigationbar/preferslargetitles), [`contentInsetAdjustmentBehavior`](https://developer.apple.com/documentation/uikit/uiscrollview/contentinsetadjustmentbehavior-swift.property) | The scroller spans the route; its top content inset is the expanded bar; `scrollTop` 0 rests below the expanded title; collapsing does not relayout the route. | D3 (scroller executor) | `nav-large` |
| C9 | Push and pop, including an interactive pop and its cancellation | [`viewIsAppearing(_:)`](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewisappearing(_:)) | The incoming route has its clearance from its first frame; a cancelled pop resamples the route that stays. | D2 | `nav-large` → `nav-inline` |
| C10 | Search bar in the navigation bar | LLP 1075.003 (header search) | The search field is bar chrome: content clears it while it shows. | D3 | `nav-search` |
| C11 | Tab bar: floating at the bottom (iOS 26), near the top (iPadOS), on the side (iPhone Duo) | [`UITabBarController`](https://developer.apple.com/documentation/uikit/uitabbarcontroller), [HIG Tab bars](https://developer.apple.com/design/human-interface-guidelines/tab-bars) | The content of each tab clears the tab bar wherever it is, once (D3, nested owners); the background reaches under it. | D2, D3 | every screen (the lab is tabbed) |
| C11a | A tab bar that minimizes while scrolling (iOS 26) | [`UITabBarController.tabBarMinimizeBehavior`](https://developer.apple.com/documentation/uikit/uitabbarcontroller/tabbarminimizebehavior) | Exact does not enable minimizing (LLP 1075.003 §3.5 leaves it open). When it is enabled, clearance follows whatever `safeAreaInsets` reports. | D3 | deferred |
| C11b | A bottom toolbar projected from authored content | LLP 1075.003 §3.5 | The toolbar is bar chrome: it enters `local` through `safeAreaInsets`, like the navigation bar. | D3 | `nav-inline` with a toolbar |
| C12 | A header lifted into the bar | LLP 1035.001 D9 | The lifted header is `display: none` and neither clears nor is credited; the route's top clearance is its whole local top inset. | D3 | `nav-inline` |

### iPhone Duo (iOS 27.1 beta SDK)

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C13 | Cover screen: the system places toolbars and tab bars in a strip on one side (84 points wide in §"Evidence") | [HIG: Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo) | Every owner's content clears the strip on whichever side it is; backgrounds and horizontal scrollers' surfaces reach under it; side clearance is wrapper padding. In Safari, Safari lays the page out beside the strip and `env()` is 0 (§"Evidence"). | D3 | `strip`, `carousel` |
| C14 | Open: the inner screen's bars | [Tech Talk: Raise the bar with iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111462/), [Tech Talk: Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/) | In landscape the bars are a side strip, as C13. In portrait, in a sheet centred on the inner screen, and in an app that opts out of side bars, the bars are horizontal (top and bottom); a sheet placed at the side of the inner screen, and sheets on the cover screen, can carry a side bar. In every arrangement clearance follows `safeAreaInsets`; nothing in Exact assumes which edge carries the bars. | D3 | `strip`, open, both orientations |
| C14a | The inner front-facing camera while it is active | [HIG: Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo), [`UIView.ReservedRegion`](https://developer.apple.com/documentation/uikit/uiview/reservedregion) | While the camera is active, UIKit reports an occlusion region. Exact clears what `safeAreaInsets` reports; whether the region enters `safeAreaInsets` is to be confirmed on the iOS 27.1 simulator. A rectangle that does not enter them is deferred with a consumer (D6). | D3, D6 | `strip` with the camera active |
| C15 | Open in a book pose: the fold | [`UIView.ReservedRegion`](https://developer.apple.com/documentation/uikit/uiview/reservedregion), LLP 1078 | The document reports two segments and `folded`; content uses `env(viewport-segment-*)`. Automatic clearance does not protect an interior fold. | LLP 1078, D6 (deferred) | `segments` |
| C16 | A sheet on a partially open fold | [Tech Talk: Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/) | UIKit moves the sheet off the fold; the sheet's owner follows its new allocation. | D2 | `sheet-page` in a book pose |

### Presentations

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C17 | Page sheet, large detent, with the presenting screen scaled behind it | [`.pageSheet`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/pagesheet) | The document behind keeps its size, layout and `exactViewport()`; the sheet's content gets the sheet's width (`100%`) and its own clearance; the background is inert. | D1, D2, D3 | `sheet-page` |
| C18 | Medium and custom detents, and dragging between them | [`detents`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detents), [custom detents](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detent/custom(identifier:resolver:)) | The sheet owner republishes on every step of the drag; its content follows. Authored detent points exclude the bottom safe area. | D2 | `sheet-detents` |
| C19 | Form sheet; a floating sheet in regular width | [`.formSheet`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/formsheet) | A floating sheet has no bottom inset; an edge-attached one has the home indicator's. `local` is right in both. | D3, D7 | `sheet-form` |
| C20 | Sheet in compact height (landscape iPhone) | [`prefersEdgeAttachedInCompactHeight`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/prefersedgeattachedincompactheight) | `.medium()` is inactive; the sheet's side and bottom insets are its own. | D3 | `sheet-detents` rotated |
| C21 | Full-screen presentation | [`.overFullScreen`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/overfullscreen) | The presented owner clears the status bar, home indicator and strip itself; the background stays mounted and inert. | D2, D3 | `fullscreen` |
| C22 | A sheet over a sheet | [HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets) | Each sheet is its own owner; clearance is computed once per owner; focus and hit testing go to the top one. | D2 | `sheet-page` → nested |
| C23 | Interactive dismissal and its cancellation | [`presentationControllerDidDismiss(_:)`](https://developer.apple.com/documentation/uikit/uiadaptivepresentationcontrollerdelegate/presentationcontrollerdiddismiss(_:)) | During and after the drag the document behind keeps its size; a cancelled dismissal resamples the sheet. | D2 | `sheet-page` |
| C24 | A sheet whose content decides its height | [`presentationSizing`](https://developer.apple.com/documentation/swiftui/presentationsizing) | `navigationSizing="fitted"` measures without clearance, adds it once, and refuses explicit detents. | D7 | `sheet-fitted` (stage 6) |
| C24a | A nonmodal sheet (the background stays interactive under an undimmed detent) | [HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets), [`largestUndimmedDetentIdentifier`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/largestundimmeddetentidentifier) | Out of scope: Exact's presented routes are modal. C17–C24 apply to modal presentations only. A nonmodal sheet waits for an app. | — | deferred |

### Keyboard

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C25 | Docked keyboard over a route with a scroller | [Keyboard layout guide](https://developer.apple.com/documentation/uikit/adjusting-your-layout-with-keyboard-layout-guide), [HIG Virtual keyboards](https://developer.apple.com/design/human-interface-guidelines/virtual-keyboards) | The focused field stays visible; the overlap becomes the scroller's bottom content inset; nothing else shrinks. | D7 | `keyboard` |
| C26 | Keyboard over a medium sheet | As C25 | UIKit grows the sheet; only the residual overlap after that is accommodated. | D7 | `sheet-detents` with a field |
| C27 | Floating or split keyboard (iPad) | [`followsUndockedKeyboard`](https://developer.apple.com/documentation/uikit/uikeyboardlayoutguide/followsundockedkeyboard) | The keyboard is a rectangle: the caret is revealed, no bottom inset is added. | D7 | `keyboard` on iPad |
| C28 | Hardware keyboard with the shortcuts bar | As C25 | Only the bar's overlap counts; a dismissed keyboard counts as zero. | D7 | `keyboard` |
| C29 | `interactive-widget="resizes-content"` | [CSS Viewport 1](https://drafts.csswg.org/css-viewport/#interactive-widget-section) | The document viewport shrinks, as in a browser; owners follow. | D1, D7 | `keyboard-resize` |

### Content and authoring

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C30 | A background or artwork that reaches the edges under chrome | [HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout), [`ignoresSafeArea`](https://developer.apple.com/documentation/swiftui/view/ignoressafearea(_:edges:)) | The owner's background colour fills its whole allocation; an absolutely positioned image can too; content stays clear. | D3 | `edge-art` |
| C31 | An edge the author handles alone | As C30 | `-exact-safe-area` turns that edge's clearance off; `var(--exact-safe-area-inset-*)` gives the value to pad with. | D4, D9 | `opt-out` |
| C32 | "At least 16, or the safe area" | [WebKit, iPhone X](https://webkit.org/blog/7929/designing-websites-for-iphone-x/) | `max(16px, env(…))` at the root; `max(16px, var(--exact-safe-area-inset-…))` in an owner; neither doubles with clearance. | D3, D5, D9 | `padding` |
| C33 | A horizontal scroller (carousel) beside a side strip | [`contentInsetAdjustmentBehavior`](https://developer.apple.com/documentation/uikit/uiscrollview/contentinsetadjustmentbehavior-swift.property) | Side clearance is wrapper padding, so the carousel is narrowed by the strip; its background still reaches under it. | D3 | `carousel` |
| C33a | A primary scroller that does not reach an edge: an in-flow control above it, a pane beside it, a composer below it | [`adjustedContentInset`](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset) | The scroller takes an edge's clearance only when it spans that whole edge; otherwise the wrapper pads the edge, so the control and the pane clear the bar and the composer clears the home indicator. A focused composer outside the scroller is lifted above the keyboard. | D3, D7 | `composer` |
| C34 | Readable width and margins | [`readableContentGuide`](https://developer.apple.com/documentation/uikit/uiview/readablecontentguide) | Authored `max-width` and auto margins, or the platform's values through `env(-exact-readable-content-inset-*)` and `env(-exact-layout-margins-inset-*)`; default padding is zero. | D8, D10 | `columns` |
| C35 | Logic that depends on a container's size | [`GeometryReader`](https://developer.apple.com/documentation/swiftui/geometryreader) | Percentages and flex first; when a branch is needed, the fenced `container:` fact, which may not feed back into the owner's chrome. | D6 | `columns` (stage 6) |

### Other hosts

| # | Case | Source | Required behavior | By | Lab screen |
|---|---|---|---|---|---|
| C36 | tvOS overscan, focus and modals | [HIG Layout (tvOS)](https://developer.apple.com/design/human-interface-guidelines/layout), [HIG Focus and selection](https://developer.apple.com/design/human-interface-guidelines/focus-and-selection) | Owners clear the overscan zone through `safeAreaInsets`; room for focus enlargement is the author's padding; presentations are full screen and Menu dismisses. | D3, D7 | the lab on Apple TV |
| C37 | macOS window with full-size content | [`NSView.safeAreaInsets`](https://developer.apple.com/documentation/appkit/nsview/safeareainsets) | When native routes are projected, owners clear the title bar and toolbar. | D3 | deferred with LLP 1075.003 §3.11 |
| C38 | Mobile Safari, including the Duo | §"Evidence" | The root's `env()` is the browser's; the web host's emulated sheets and bars are owners that clear their own chrome. | D1, D3 | every screen in Safari |
| C39 | Linux, Windows | — | No native chrome: `local` is 0; the same app lays out the same. | D3 | conformance |
| C40 | VoiceOver and focus with a modal up | [HTML modal dialogs](https://html.spec.whatwg.org/multipage/interaction.html#modal-dialogs-and-inert-subtrees), [HIG Modality](https://developer.apple.com/design/human-interface-guidelines/modality) | The background is inert; focus enters the presented owner and returns on close. | D2 | `sheet-page` with VoiceOver |

## Apple's APIs and the HIG, and how Exact maps each

Exact follows CSS where CSS has a word, follows UIKit where only UIKit does, and declares each deviation. The tables say, per API, what it does and what Exact does with it: **follows**, **deviates** (with the reason), or **defers** (until a consumer). iPhone Duo rows are iOS 27.1 beta-SDK evidence.

### A view's size and traits

| API | What it does | Exact |
|---|---|---|
| [`UIView.bounds`](https://developer.apple.com/documentation/uikit/uiview/bounds), [`UIView.frame`](https://developer.apple.com/documentation/uikit/uiview/frame) | `bounds` is the view's own rectangle; `frame` is in the superview's coordinates and is undefined under a non-identity `transform`. | Follows. An owner's allocation is its view's `bounds`. Its origin and transform relative to the document are published beside it (D2), never folded into the size. A presenting screen scaled behind a sheet keeps its allocation; a transform is not a resize. |
| [`traitCollection`](https://developer.apple.com/documentation/uikit/uitraitenvironment/traitcollection), [`horizontalSizeClass`](https://developer.apple.com/documentation/uikit/uitraitcollection/horizontalsizeclass) | Traits are inherited through containment and can differ per controller. Width does not determine the size class: an iPad form sheet is compact-width at any window size, and a Split View column can be compact. | Deviates, following CSS: Contract branches on numbers (LLP 1039 D1), and the `container:` fact (D6) gives an owner's numbers. No size-class field is exposed; a native hook reads the installed controller's traits when an app wants UIKit's own class. |
| [`UITraitOverrides`](https://developer.apple.com/documentation/uikit/uitraitoverrides-swift.struct), [`setOverrideTraitCollection(_:forChild:)`](https://developer.apple.com/documentation/uikit/uiviewcontroller/setoverridetraitcollection(_:forchild:)), [Adapting your app when traits change](https://developer.apple.com/documentation/uikit/adapting-your-app-when-traits-change) | A container can override a child's traits; the child's effective traits are the installed ones. | Follows. Exact reads effective traits from the installed owner and reconstructs nothing from width. |
| [`viewWillTransition(to:with:)`](https://developer.apple.com/documentation/uikit/uicontentcontainer/viewwilltransition(to:with:)), [`size(forChildContentContainer:withParentContainerSize:)`](https://developer.apple.com/documentation/uikit/uicontentcontainer/size(forchildcontentcontainer:withparentcontainersize:)) | Announces the container's impending size with the transition coordinator; a custom container can give each child a different size. | Follows, as prediction only. The announced size is never combined with current insets (D2). Owners are resampled when the transition completes or cancels. |
| [`viewIsAppearing(_:)`](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewisappearing(_:)) | Called once per appearance, after the view is in the hierarchy with its bounds, traits and safe area correct, before the transition finishes. Available back to iOS 13. | Follows. The first publication of an owner's allocation is here (D2), so first-frame content is laid out at its real size. |
| [`viewDidLayoutSubviews()`](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewdidlayoutsubviews()) | The continuing layout callback; runs repeatedly, including during interactive resizes. | Follows. Later publications sample here, after the owner's own layout (D2). |
| [`viewSafeAreaInsetsDidChange()`](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewsafeareainsetsdidchange()), [`safeAreaInsetsDidChange()`](https://developer.apple.com/documentation/uikit/uiview/safeareainsetsdidchange()) | The safe area of a controller's view, or of a view, changed. | Follows. Either invalidates the owner's snapshot; the next layout publishes (D2). |

### Safe areas, margins and bars

| API | What it does | Exact |
|---|---|---|
| [`safeAreaInsets`](https://developer.apple.com/documentation/uikit/uiview/safeareainsets), [`safeAreaLayoutGuide`](https://developer.apple.com/documentation/uikit/uiview/safearealayoutguide) | Per view, relative to its bounds. A controller's root view includes its bars, the status bar, the home indicator, a side strip and any additional insets; a subview wholly inside the safe region reports zero; a view not in a window reports zero. The guide is for constraints and constrains nothing by itself. | Follows. An owner's `local` clearance (D3) is its view's `safeAreaInsets`. Availability is tracked separately from the value, so an uninstalled owner's zero is never published. |
| [`additionalSafeAreaInsets`](https://developer.apple.com/documentation/uikit/uiviewcontroller/additionalsafeareainsets) | A controller reserves space beyond physical overlap, for chrome it draws itself. It is a deliberate reservation, not a measurement. | Follows. A reservation is cleared like a bar, because it is in `safeAreaInsets`. An embedder that draws its own chrome over Exact content reserves it this way, on the owner's controller, and writes nothing into Exact. |
| [`insetsLayoutMarginsFromSafeArea`](https://developer.apple.com/documentation/uikit/uiview/insetslayoutmarginsfromsafearea) | Default `true`: layout margins are at least the safe area. | Follows in effect: content is inside the safe area by default (D3). |
| [`layoutMargins`](https://developer.apple.com/documentation/uikit/uiview/layoutmargins), [`directionalLayoutMargins`](https://developer.apple.com/documentation/uikit/uiview/directionallayoutmargins), [`systemMinimumLayoutMargins`](https://developer.apple.com/documentation/uikit/uiviewcontroller/systemminimumlayoutmargins), [`viewRespectsSystemMinimumLayoutMargins`](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewrespectssystemminimumlayoutmargins) | Ordinary views have 8-point margins; a controller's root view takes the system minimum (contextual, 16 or 20 points) unless told not to. | Follows as values: `env(-exact-layout-margins-inset-*)` is the margins of the controller's root view that holds the session, at least the safe area, with a corner-adapted twin (D10). A box still has zero padding until the author writes some, following CSS; an author who wants UIKit's spacing pads with the value. |
| [`readableContentGuide`](https://developer.apple.com/documentation/uikit/uiview/readablecontentguide) | A width comfortable for reading, inside the margins, growing with the text size. No fixed width is documented. | Follows as values: `env(-exact-readable-content-inset-*)`, UIKit's own width (D10); `max-width` with auto margins stays the way to cap a column (D8). |
| [`UIView.LayoutRegion`](https://developer.apple.com/documentation/uikit/uiview/layoutregion), [`NSView.LayoutRegion`](https://developer.apple.com/documentation/appkit/nsview/layoutregion) (iOS 26, macOS 26) | The safe area, margins and (UIKit) readable content, each optionally adapted to keep content clear of rounded display or window corners, horizontally or vertically. | Follows as values: the five `env(-exact-<region>-inset-<edge>)` families (D10). |
| [`edgesForExtendedLayout`](https://developer.apple.com/documentation/uikit/uiviewcontroller/edgesforextendedlayout), [`extendedLayoutIncludesOpaqueBars`](https://developer.apple.com/documentation/uikit/uiviewcontroller/extendedlayoutincludesopaquebars) | Whether a controller's view extends under translucent bars (default `.all`) and opaque ones (default `false`). They decide the view's allocation under bars, not whether content clears them; they do not affect the window's root. | Follows for allocation: an owner's view always extends under its bars, and its background reaches the edges (D3). They are not a safe-area opt-out and D4 is not their counterpart: D4 changes clearance, never allocation. |
| [`UIScrollView.contentInsetAdjustmentBehavior`](https://developer.apple.com/documentation/uikit/uiscrollview/contentinsetadjustmentbehavior-swift.property), [`adjustedContentInset`](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset) | Default `.automatic`: a scroll view under bars gets content and indicator insets from the safe area; `.scrollableAxes` insets only the axes that scroll; `.never` leaves it to the developer. | Follows, as the one executor on an owner's primary scroller's top and bottom edges (D3): content scrolls under the bars and rests below them; side clearance is wrapper padding on every owner. Every other scroll view is `.never`, since Exact owns its insets (LLP 1075.003 §3.5). |
| Scroll-edge effects (iOS 26), [`UIScrollEdgeElementContainerInteraction`](https://developer.apple.com/documentation/uikit/uiscrolledgeelementcontainerinteraction) | A legibility effect where content scrolls under a bar; the system applies it to its own bars, and the interaction extends it to custom elements. | Follows by leaving it to UIKit: a visual effect, separate from safe allocation. Exact adds no second effect and does not clear content for it. |

### Presentation and sheets

| API | What it does | Exact |
|---|---|---|
| [`modalPresentationStyle`](https://developer.apple.com/documentation/uikit/uiviewcontroller/modalpresentationstyle), [`.automatic`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/automatic), [`.formSheet`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/formsheet), [`.pageSheet`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/pagesheet) | The default is `.automatic`, which most controllers resolve to `.formSheet` on iOS 18 and later and `.pageSheet` before. A form sheet is compact-width on iPad and full-width on iPhone, so its traits are read after installation, not assumed. | Deviates by explicit choice: `navigationPresentation="modal"` is `.pageSheet` (`ModalIOS.swift`), and `navigationSizing` (D7) selects form, page or fitted. Moving the default to `automatic` is Q1. |
| [`.overFullScreen`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/overfullscreen), [`.fullScreen`](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/fullscreen) | `.fullScreen` removes the presenting view from the hierarchy when the transition ends; `.overFullScreen` leaves it in place. | Follows: `navigationPresentation="fullscreen"` is `.overFullScreen`, so the background stays live (dynamic colours, text) and inert (LLP 1035.001 D4). The two are never confused. |
| [`UISheetPresentationController.detents`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detents), [`Detent`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detent) | Default `[.large()]`; ordered smallest to largest; `.medium()` is about half the height and inactive in compact height. | Follows: `navigationDetent="300 large"` lowers to these. |
| [`Detent.custom(identifier:resolver:)`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detent/custom(identifier:resolver:)), [`maximumDetentValue`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontrollerdetentresolutioncontext/maximumdetentvalue) | A custom detent returns a height **within the sheet's safe area**; an edge-attached sheet (compact width) adds the bottom inset outside that height, a floating one (regular width, centred) does not. | Follows: authored detent points exclude the bottom safe area, as today. An owner's allocation is the presented controller's view bounds, which include the inset when edge-attached and not when floating; `local` is right either way. |
| [`prefersScrollingExpandsWhenScrolledToEdge`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/prefersscrollingexpandswhenscrolledtoedge), [`prefersGrabberVisible`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/prefersgrabbervisible), [`prefersEdgeAttachedInCompactHeight`](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/prefersedgeattachedincompactheight) | Scrolling to the top expands to the next detent (default `true`); the grabber; edge attachment in landscape phones. | Follows: a sheet with more than one detent expands when its content is scrolled at the top edge, as UIKit's own resizable sheets do (`detents.count > 1`, `ModalIOS.swift`), and shows the grabber; a sheet with one detent does neither. Edge attachment stays UIKit's default. |
| Presenting view scaling on iPhone | A page sheet scales and darkens the presenting view behind it. | Follows: the scale is a transform on the presenting view; the background document's allocation and `exactViewport()` are unchanged (D1). |
| [`UIPresentationController.containerView`](https://developer.apple.com/documentation/uikit/uipresentationcontroller/containerview) | UIKit may wrap or move the presenting view during a transition. | Follows: Exact requires no stable superview and stops moving the document itself. Only the presented owner's navigation and content mount in the sheet (D2). |

### The keyboard

| API | What it does | Exact |
|---|---|---|
| [`keyboardLayoutGuide`](https://developer.apple.com/documentation/uikit/uiview/keyboardlayoutguide), [Adjusting your layout with keyboard layout guide](https://developer.apple.com/documentation/uikit/adjusting-your-layout-with-keyboard-layout-guide) | A per-view guide tracking the keyboard in that view's coordinates. | Follows: keyboard occupancy is converted into each owner's coordinates (D7), as LLP 1035.001 D5 measures it in the active container's space. |
| [`followsUndockedKeyboard`](https://developer.apple.com/documentation/uikit/uikeyboardlayoutguide/followsundockedkeyboard) | Default `false`: a floating or split keyboard is not followed; the guide rests at the bottom. | Follows in effect: a floating keyboard is a rectangle, not a bottom inset (D7). |
| [`usesBottomSafeArea`](https://developer.apple.com/documentation/uikit/uikeyboardlayoutguide/usesbottomsafearea) | Default `true`: a dismissed guide is the bottom safe area's height, not zero. | Follows: that baseline is excluded from occupancy, so a dismissed keyboard counts as zero (D7; `keyboardGuideTop` already applies the test). |
| Stage Manager and external displays ([WWDC23, "Keep up with the keyboard"](https://developer.apple.com/videos/play/wwdc2023/10281/)) | The keyboard can be on another screen or in another window's coordinate space; convert through the window and intersect with the affected view. | Follows: occupancy is intersected with each owner's allocation after conversion; a document keyboard height is never taken as an owner's overlap (D7). |

### Scenes, Split View and Stage Manager

| API | What it does | Exact |
|---|---|---|
| [`UIWindowScene.effectiveGeometry`](https://developer.apple.com/documentation/uikit/uiwindowscene/effectivegeometry) | The scene's coordinate space, system frame, orientation and interactive-resizing state. | Follows upstream: the scene's size is not the session's. A session is sized by its own `ExactView`; an embedded session in a sheet takes the sheet's size as its document size (D1). |
| [Multitasking on iPad, Mac and Apple Vision Pro](https://developer.apple.com/documentation/uikit/multitasking-on-ipad-mac-and-apple-vision-pro) | Split View, Slide Over and Stage Manager resize windows continuously; no width table applies. | Follows: a window resize is a real resize of the session root (document facts change, LLP 1039 D2) and of every owner (D2). Only transforms are excluded. |

### iPhone Duo (iOS 27.1 beta SDK)

| API | What it does | Exact |
|---|---|---|
| [`UIView.ReservedRegion`](https://developer.apple.com/documentation/uikit/uiview/reservedregion), [`frame`](https://developer.apple.com/documentation/uikit/uiview/reservedregion/frame) | Division (fold) and occlusion regions, local to the queried view, with identity and an active state. A region's frame is in that view's coordinates and already includes the interaction margins. | Follows for the document: LLP 1078 D5 maps the session view's active divisions to viewport segments, margins included (its Q5 ruling). Owner-local segments are deferred (D6): they need ordered rectangles, not four insets, and join the `container:` record with a consumer. Occlusion regions (the inner camera while active) are reported as regions; Exact clears only what `safeAreaInsets` includes, and whether they are included is beta behavior to confirm (C14a). LLP 1078 D5's sentence that they "stay what they already are, the safe-area insets" is amended to match. |
| [`UIHingeInteraction`](https://developer.apple.com/documentation/uikit/uihingeinteraction) | Hinge status and angle, observed on a view hierarchy; a fold from flat to a book angle changes region activity without changing bounds. | Follows (LLP 1078 D5): posture comes from the hinge; a status change relayouts. A fold without a bounds change invalidates every owner's snapshot (D2). |
| `ArrangementView`, `UIArrangementViewController` ([Tech Talk: Strike a pose with adaptive layouts on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111463/)) | Split or overlay arrangement of panes within given bounds; layout, not navigation. Named in Apple's Tech Talks; their documentation pages are beta and not linked here. | Defers: `navigationArrangement` waits for an app (D7). A pane an arrangement gives Exact is an app-owned container, so it is an owner. |
| Standard sheets on a partial fold ([Tech Talk: Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/)) | UIKit moves a standard sheet off a partially open fold and adapts its controls. | Follows: Exact's page sheet is a standard sheet, so UIKit moves it; the sheet owner's allocation follows the move (D2). Beta evidence. |

### tvOS

| Guidance | What it does | Exact |
|---|---|---|
| Overscan ([HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout)) | Primary content keeps at least 60 points from the top and bottom and 80 from the sides; UIKit expresses the zone through the root controller's `safeAreaInsets`. | Follows: `local` on tvOS is the controller's `safeAreaInsets`, so owners clear the overscan zone (D3). |
| Focus enlargement ([HIG Focus and selection](https://developer.apple.com/design/human-interface-guidelines/focus-and-selection)) | A focused element grows; leave room around it. | Follows as the author's business, as in UIKit: no API supplies a focus margin, so none is cleared. The host's focus effect draws outside the box; an app whose focused rows scale leaves room in its own padding. |
| Presentation ([HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets), [Modality](https://developer.apple.com/design/human-interface-guidelines/modality)) | The HIG says a tvOS sheet is modal and says nothing about its size (its full-screen sheet is watchOS's). UIKit on tvOS has no `UISheetPresentationController`, so there are no detents. | Exact's chosen mapping: `modal` and `fullscreen` both present `.overFullScreen` on tvOS (`ModalIOS.swift`), `navigationDetent` is ignored, Menu dismisses, and the remote drives focus into the presented owner and back. |

### SwiftUI

| API | What it does | Exact |
|---|---|---|
| [`ignoresSafeArea(_:edges:)`](https://developer.apple.com/documentation/swiftui/view/ignoressafearea(_:edges:)), [`SafeAreaRegions`](https://developer.apple.com/documentation/swiftui/safearearegions) | Content is inside the safe area by default; the modifier opts out per region (`.container`, `.keyboard`) and edge, usually on a background. | Follows: default clearance (D3); `-exact-safe-area` is `ignoresSafeArea(.container, edges:)` (D4); `keyboardAvoidance="none"` is the `.keyboard` region (D7). A background that reaches the edges is the owner's propagated background (D3), or an absolutely positioned layer. |
| [`safeAreaInset(edge:alignment:spacing:content:)`](https://developer.apple.com/documentation/swiftui/view/safeareainset(edge:alignment:spacing:content:)) | Places content at an edge and shrinks descendants' safe area by it. | Follows in two forms: an authored toolbar in flow, which takes its space as any box does; or an embedder's chrome, reserved on the owner's controller with `additionalSafeAreaInsets`, which enters `local`. No new element. |
| [`safeAreaPadding(_:_:)`](https://developer.apple.com/documentation/swiftui/view/safeareapadding(_:_:)), [`contentMargins(_:_:for:)`](https://developer.apple.com/documentation/swiftui/view/contentmargins(_:_:for:)-1lt8b) | Padding added to the safe area; margins for scroll content and indicators separately. | Follows: authored padding is added to clearance (D3); a primary scroller's clearance is content and indicator insets (D3). Padding and margins are additive in SwiftUI, and here. |
| [`presentationDetents(_:)`](https://developer.apple.com/documentation/swiftui/view/presentationdetents(_:)), [`presentationSizing(_:)`](https://developer.apple.com/documentation/swiftui/view/presentationsizing(_:)), [`PresentationSizing`](https://developer.apple.com/documentation/swiftui/presentationsizing) | Detents; and `.form`, `.page`, `.fitted` (ideal content size), `.automatic`. | Follows: `navigationDetent`; `navigationSizing` with `automatic`, `form`, `page` or `fitted` (D7). The names supply no measurement: `fitted` is Exact's own bounded measurement. |
| [`GeometryReader`](https://developer.apple.com/documentation/swiftui/geometryreader), [`containerRelativeFrame`](https://developer.apple.com/documentation/swiftui/view/containerrelativeframe(_:alignment:_:)), [`onGeometryChange(for:of:action:)`](https://developer.apple.com/documentation/swiftui/view/ongeometrychange(for:of:action:)) | A view's own size; a frame relative to the nearest container; a callback on geometry changes. | Deviates, following CSS: a box's size is a layout result and nothing branches on it (LLP 1039 D5). The one admission is the fenced `container:` fact for an owner whose size the host fixes (D6). Percentages and flex are `containerRelativeFrame`. |
| [`ViewThatFits`](https://developer.apple.com/documentation/swiftui/viewthatfits) | Picks the first candidate whose ideal size fits. | Deviates, following CSS, which has no such construct: an app branches on the viewport, a prop or the `container:` fact. |
| [`horizontalSizeClass`](https://developer.apple.com/documentation/swiftui/environmentvalues/horizontalsizeclass) | The effective size class at the reading view, which inside a sheet is the sheet's. | Deviates as above: numbers, read where the content is (D6), never a class captured in the presenter. |

### The HIG

| Page | What it asks | Exact |
|---|---|---|
| [Layout](https://developer.apple.com/design/human-interface-guidelines/layout) | Adapt the composition to the available space and text size; keep important content inside the safe area; let backgrounds and scrolling surfaces extend to the edges; respect margins. | Follows: owners adapt by allocation; content clears chrome by default; the owner's background propagates to its whole allocation; margins are the author's padding (D3). |
| [Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets) | A scoped task; detents, a grabber for resizable sheets; form and page sheets on iPad. | Follows: `navigationPresentation="modal"`, `navigationDetent`, `navigationSizing` (D7). |
| [Modality](https://developer.apple.com/design/human-interface-guidelines/modality) | Use modality for a focused task; make dismissal obvious; keep the task simple. | Follows: a modal owner's background is inert; `closedby` governs dismissal (LLP 1007 §10 addendum, "Modal routes", 1008 §9); focus enters the owner and returns on close (D2). |
| [Virtual keyboards](https://developer.apple.com/design/human-interface-guidelines/virtual-keyboards) | Keep the focused field visible; use the keyboard layout guide; reveal a hidden insertion point by scrolling just enough. | Follows: `keyboardAvoidance="auto"` reveals the focused editor through its nearest scroller and adds only the residual docked overlap (D7). It does not shrink every container. |
| [Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo) | Preserve state across poses; adapt panes and margins; keep interactive content off the fold band; avoid disruptive rearrangement. | Follows for the document (LLP 1078); the fold band belongs to no segment. Pane policy and owner-local segments are deferred with their consumers (D6, D7). |
| [Multitasking](https://developer.apple.com/design/human-interface-guidelines/multitasking) | Work at any size; do not assume full screen. | Follows: every size reaches Exact as a real resize; nothing reads the screen's bounds. |

## Design

### D1 — One viewport per session

`exactViewport()` (width, height, media preferences, `devicePosture`, segments), `vw`/`vh` and the other viewport units, and `env(safe-area-inset-*)` and `env(viewport-segment-*)` are the Exact session's document facts. They are measured from the session's own view after root fitting, under the first root's `viewport-fit` and `interactive-widget`.

- **Root fitting** has three rules. They depend on the root's `viewport-fit` and on whether the primary screen is a native container (a navigation stack or tab container projected from the root's routes). They never depend on whether a bar is visible at the moment, and never on a presentation.
  - `viewport-fit="cover"`: the document covers the session view. `env(safe-area-inset-*)` is the view's safe-area insets.
  - `viewport-fit="auto"` with no native primary container: the document is fitted inside the view's safe area on all four edges, so no obstruction overlaps it and `env()` is 0 (LLP 1008 §9). Safari's `auto` fits less: unfolded on the iPhone Duo it leaves the bottom edge under the home indicator and reports it (§"Evidence"). Exact's web host reports what the browser reports (C38).
  - `viewport-fit="auto"` with a native primary container: the document covers the session view, so the container's bars reach under the status bar. `env()` is 0, as under the second rule; covering the view while `env()` is 0 is a native deviation, declared in LLP 1001. Authored content is inside owners, which clear (D3).
  - The fitting reads one rule, "a native primary container exists", for the viewport and the window alike (`NavigationHost.wantsWholeView` and `rootWantsWholeView` become one). Hiding a primary route's header and its bar does not change the fitting or `exactViewport()`.
- No presentation, route or controller changes these facts. `viewport-fit` or `interactive-widget` written inside an owner's content is ignored: only the first root's props are policy (`PresenterIOS.viewportFit`).
- A sheet is like a modal `<dialog>` in the top layer: the same document, laid out in its own box, with the background inert. It is never an iframe with its own viewport.
- The unit is the session. Two embedded sessions can have different viewports, and a session embedded in a sheet takes that sheet's size as its document size.
- Viewport-unit families (`svh`, `lvh`, `dvh`) are out of scope; on the web `exactViewport().height` follows `innerHeight` and `vh` the large viewport, as in browsers.

### D2 — Content owners and their host blocks

A content owner is any of:
- a route (`navigationKey`) in a native navigation stack;
- a presented route (`navigationPresentation="modal" | "fullscreen"`);
- a tab panel;
- an app-owned container admitted through LLP 1075.003 §3.6, such as a pane an arrangement gives Exact, or a split-view column an app hosts Exact content in;
- on the web, a wrapper the web host's emulated chrome (`nav-chrome.js`) puts around a route or sheet.

Popovers and other anchored boxes (`popovertarget`, `position-area`, menus, choosers) are top-layer boxes of the document, not owners.

The session root is not an owner. Ordinary boxes are never owners.

**RealWorld route content (2026-10-15).** Conduit's shared `Header` and `Footer`
belong inside each route owner. Its previous global header was a sibling before
the native stack: the full-root article owner correctly painted above it and
took a tap at Home's old position. A physical iPad baseline at `06d9155a4`
confirmed the article banner at `y=0` over the header's `y=0..56` allocation.
`apps/realworld/app.contract` now composes those shared components inside each
route, keeping their authored styles and normal hit testing. This corrects the
app's ownership, without a native stacking exception or extra clearance.
Its retained route columns are absolute layers inside the viewport-height
root, each with vertical overflow. A hidden retained route consumes no flow
space above the next page's header, and long article content still scrolls.

A retained iOS route can leave the native stack, give its block back, then
rejoin below the selected route without appearing. If its controller already
has a loaded view with nonempty bounds, `OwnerBlocks` restores that real block
without claiming an offscreen placement. An offscreen route that kept its
block retains it. The distinction matters to `head::inactive`: only a first
route still in document layout can remain live beside a Mac detail pane; a
covered native owner stays inactive. The content-owner regression uses an
authored temporary route before the retained stack to remove and reintroduce
its owners through normal projection. It checks the released and restored
block, inactive tree and ordinary input on the shown route; a direct UIKit
stack edit is insufficient because projection can restore the authored stack.

**Nested owners clear once.** Owners nest: a route inside a tab, a sheet over either. Only an owner whose content is authored clears. A container whose whole content is another owner, such as a tab panel holding a navigation stack, contributes no clearance of its own. UIKit already includes the outer chrome (the tab bar) in the inner owner's `safeAreaInsets`, so the inner owner clears it, once. A tab panel whose content is authored directly is itself the owner that clears.

**An owner is a host block.** LLP 1115.002 D8.2 gives the kernel one way to lay a subtree out in a box that is not the window: the host gives the subtree's root a **containing block**, a width and a height, each the size or a maximum. The root is then out of its parent's layout and laid out as a root is, in the block; its frames are in the block's own space; the block's used size is published with the layout that changed it; viewport units, `env()` and `rem` stay the window's. A pane on the Mac (LLP 1115.002 D4) and a sheet on the Mac use it. An owner's root is given a block the same way, on every host with native containers:

| Owner | Block | As in |
|---|---|---|
| A route in a stack, a tab panel, an app's pane, a full-screen presentation | `(is, is)`: its controller's view bounds | a split's pane |
| A sheet with detents | `(is, is)`: the sheet's view bounds at the current detent | a pane, resized at every detent step |
| A sheet whose content decides its height (`navigationSizing="fitted"`, the `fit-content` detent, D7) | `(is, at most)`: the sheet's width, its maximum height | LLP 1115.002 D8.2's form sheet |

The block is the owner's untransformed bounds; in what follows, the owner's **allocation** is its block's used size. A transform (the presenting screen scaled behind a page sheet) is never a resize.

**Where an owner is shown.** The kernel never reads where an owner is; the runner needs it for public geometry, since a node's frame is in its block's space. There are two kinds of owner here, and they differ in stacking.
- **A presentation** (a sheet, a full-screen presentation) is above the document, as a modal `dialog` is in the top layer (CSS Position 4). Its root is reported as an open top-layer box with the dialogs and popovers, back to front, by its block's rectangle in the session view's space (`exact_top_layer`, `Scrolled::top_layer_frames`): `client_rect` maps its subtree through it, `elementFromPoint` tests it before the page's roots, and it is not walked again under its authored parent. The host also says where in that list the frontmost presentation begins, as it decides what UIKit presents and what its dimming makes inert (`inertBehind`); the runner keeps that box and those above it live to point reads and selectors and the rest inert, and reads no presentation attribute of its own.
- **An ordinary owner** (a route in a stack, a tab panel, a split's pane) keeps its place among its siblings: a toast written after the routes is above them, in UIKit and to the runner alike. Its host reports a **placement**, the block's rectangle where it shows it, in the local coordinates of an ancestor that holds the block in the host's hierarchy as in the document: the ancestor's border box before its own transform. That is the root's parent when the host shows the block in it; UIKit's tab container holds a tab's route beside its panel, so on iOS it is the routes' owner, which holds the native container, and the rectangle is UIKit's conversion into the owner's view. That conversion crosses only UIKit's containers, which neither turn nor skew what they hold, so the rectangle is exact, where one converted into a turned panel would be its bounding box. The runner keeps the root in its ordinary walk and steps from the root through the placement instead of the root's frame, straight to that ancestor, leaving out the boxes between, which move nothing the host shows, as the host's own hierarchy does: `client_rect` maps the subtree through it, the transform and scrolling of that ancestor and of every box above it applied once, and `elementFromPoint` reaches the root in its tree and rank order and tests its subtree there. The placement rides the block's own entry in `exact_host_covers` and is sent when it changes. Only the runner reads it; it is never a layout input. A block with no placement is mapped at its block's origin.

In both, the root sits in its block by its own frame (its margins, a width narrower than the block), and a rectangle of another size than the block's used size is a scale: the host's own transform (a sheet scaled at a lower detent), never the difference between the block and the root. A route under another route of its stack, or under a modal one, is inactive to the runner's walk (`head::inactive`), so hit testing passes it, as UIKit does. The agent's coordinates, taps and screenshots follow the same mapping.

**What an owner's block carries beyond D8.2.** Two facts the kernel lays out with, published in the same commit as the block:

- **Local insets** (D3, D9): the owner view's `safeAreaInsets`.
- **The primary scroller** (D3's executor), when the owner names one.

One host-to-kernel entry per owner carries the block and these facts, beside `exact_insets` and `exact_segments`, never size and insets in separate calls; the hosts already send blocks through `exact_host_covers` (kinds 4 and 5), and the owner's facts ride the same door.

- **When the host publishes it.** At `viewIsAppearing`, before the owner is first visible; after the owner's layout (`viewDidLayoutSubviews`); and on safe-area, trait, keyboard and hinge changes. That includes every step of a detent drag and every frame of a window resize. A predicted size from `viewWillTransition(to:with:)` is never combined with current insets.
- **Retirement is separate from publication.** A transition's outcome decides retirement and router state (LLP 1035.001 D2). When a dismissal or pop is cancelled, the surviving owner is sampled again. An owner detached mid-transition keeps its last committed block for its children; a new child under it lays out against that. A block taken back returns the root to its parent's layout (LLP 1115.002 D8.2 item 10).
- **Mounting.** The host moves the owner root's node view into the container's view, as `BlockHostMac` does for a pane, and the document's viewport never moves: a sheet mounts only its own navigation and content. `width: 100%` on the root of a 653-point sheet is 653; `100vw` is the document's width and may overflow.

**Two ancestries are kept apart.**
- *Logical ancestry* (the Contract tree) carries inheritance, state, custom properties and document order. A block changes none of it (LLP 1115.002 D8.2, "What does not change").
- *Rendering ancestry* carries containing blocks, scrolling, clipping, sticky lookup, fragmentation, text flow, auto-height measurement, publication, public geometry (`client_rect`, `elementFromPoint`), agent coordinates, screenshots and hit testing. A host block already cuts it for layout, flow, sticky lookup, child order and publication; placement extends the cut to public geometry and hit testing.

A presented owner escapes its logical ancestors' clipping, scrolling and transforms, as top-layer content does (CSS Position 4 §top-layer). An ancestor's `display: none` still suppresses it (D8.2 item 11). Stacking order selects the active owner: hit testing reaches the topmost owner at a point, focus enters the owner on top, and the background is inert while a modal owner is on top (HTML modal dialogs). Within an owner, document order and the platform's own focus behavior stand unchanged.

### D3 — Automatic local clearance, added to authored padding

On every host with native chrome, an owner's content is cleared of the chrome the author did not write. The required clearance per edge, `local`, is the owner's own safe-area inset: UIKit's `safeAreaInsets` for the owner's view, which already includes bars, a side strip, a sheet's edges, `additionalSafeAreaInsets` and tvOS overscan. The session root gets none: as on the web, `viewport-fit` decides whether the document is fitted to the safe area or covers the screen, and the author pads with `env()`.

**Formula, per owner and edge:**

```
wrapper clearance = max(0, local − credit)
content inset     = wrapper clearance + authored        (the "Used" column below)
```

`credit` is the part of the authored padding that already clears the safe area on that edge. Only padding on the owner's authored root is credited; padding a descendant writes with `env()` or `var()` is not, and adds to the clearance (an author who wants that edge to be theirs uses D4, or moves the padding to the root). Credit is computed in the owner's coordinates. Its baseline excludes clearance Exact added itself, so repeated layout with the same inputs gives the same result. Credit never crosses an owner boundary: a sheet's content gets no credit from the route under it.

**How `credit` is defined.** One rule for every expression D5 and D9 admit. An authored padding on an edge *references* that edge's inset when its expression contains `env(safe-area-inset-<edge>)` or `var(--exact-safe-area-inset-<edge>)` for the same edge, through either source or both, in any position: a `calc()` term, or an argument of `min()`, `max()` or `clamp()`, whether or not that argument is the selected one. Let `P` be the padding's used value: the expression resolved against the final containing block and floored at zero.
- If the expression references the edge's inset: `credit = min(local, P)`. The value as a whole clears up to its size; a source counts once, and both sources together count once.
- Otherwise `credit = 0`.

**Percentages.** A percentage in any of the four paddings resolves against the containing block's *width* (CSS 2 §8.4), and the containing block is the allocation less the left and right clearance, so `P` on a side edge can depend on the clearance being computed. The result is the fixed point of

```
W       = A − c_left − c_right                 (A: the allocation's width)
c_edge  = max(0, local_edge − credit_edge(W))   for left and right
```

`c_left` and `c_right` are always wrapper padding (the scroller executor below is vertical only), so this is the whole of what changes `W`. Every admitted expression is piecewise linear in `W`, so the equation is solved exactly in closed form on each linear piece, not by feeding a measured clearance back in. Where more than one `W` satisfies it, the largest `W` (the least clearance, the most credit) is the result. Top and bottom are computed in one pass once `W` is known. Without a percentage nothing depends on `W`, and every edge is one pass. The inputs are the allocation, the four `local` values, the document insets, the owner's insets and the authored expressions; clearance is never an input, so every layout with the same inputs gives the same clearance. At every solution `c_edge + P_edge ≥ local_edge`: the local clearance is always met.

Worked: `padding-right: min(20%, env(safe-area-inset-right))` on a 100-point allocation, `local` 34, document inset 34, no left padding. On the `20%` piece, `c = 34 − 0.2·(100 − c)`, so `c = 17.5`, `W = 82.5`, `P = 16.5`, used `17.5 + 16.5 = 34`. Iterating from zero clearance (`P` 20 → `c` 14, `P` 17.2 → `c` 16.8, …) converges to the same point; the closed form is the rule.

| Authored on the edge | Document inset | `local` | Used (content inset) |
|---|---|---|---|
| 0 | 0 | 34 | 34 |
| 16 | 0 | 34 | 50 |
| `env(…)` = 34 | 34 | 34 | 34 |
| `env(…)` = 59 | 59 | 0 | 59 |
| `calc(env(…) + 8px)` = 42 | 34 | 34 | 42 |
| `calc(env(…) − 8px)` = 26 | 34 | 34 | 34 |
| `calc(2 * env(…))` = 68 | 34 | 34 | 68 |
| `max(16px, env(…))` = 34 | 34 | 34 | 34 |
| `max(16px, env(…))` = 16 | 0 | 34 | 34 |
| `max(16px, env(…))` = 16 | 10 | 10 | 16 |
| `min(env(…), 20px)` = 20 | 34 | 34 | 34 |
| `max(16px, var(--exact-safe-area-inset-…))` = 34 | 0 | 34 | 34 |
| `max(16px, var(--exact-safe-area-inset-…))` = 16 | 0 | 10 | 16 |
| `calc(env(…) + var(--exact-safe-area-inset-…))` = 44 | 10 | 34 | 44 |
| `min(20%, env(…))` on a 100-point allocation = 16.5 | 34 | 34 | 34 |

Worked: row 6, `credit = min(34, 26) = 26`, clearance `34 − 26 = 8`, used `8 + 26 = 34`. Row 9, `credit = min(34, 16) = 16`, clearance 18, used 34. Row 10, `credit = min(10, 16) = 10`, clearance 0, used 16. Row 14, both sources in one expression, `P = 10 + 34 = 44`, `credit = min(34, 44) = 34`, clearance 0, used 44. Row 15 is the percentage example above: clearance 17.5, padding 16.5. Nested owners: a route inside a tab panel above an 83-point tab bar has `local` bottom 83, which already includes the tab bar; the tab panel contributes nothing; used 83, not 166.

`env()` is the *document's* inset, so a nested owner's local minimum ("at least 16, or this sheet's inset") cannot be written with `env()`. It is written with D9, and the credit rule keeps it from doubling:

```
padding-bottom="max(16px, var(--exact-safe-area-inset-bottom))"
```

An author who wants the edge entirely their own turns it off with D4 instead.

**One executor per edge.** Each obstruction on each edge is cleared by exactly one mechanism:
- **Primary scroller, vertical axis only, edge by edge.** If the owner names one (`navigationScroll`, or the existing hook binding), it takes the clearance of the **top** edge when it reaches that edge, and of the **bottom** edge when it reaches that edge. It reaches an edge when, laid out with zero clearance, its border box touches that edge of the owner's content box along the edge's whole length (the scroller spans the content box's full width), where the content box is the one inside the authored root's padding. A scroller beside a pane reaches neither the top nor the bottom edge; a scroller below another box does not reach the top edge; a scroller above another box does not reach the bottom edge. A primary scroller narrowed by `max-width` does not span an edge either, so the readable column goes inside the scroller (C34). On an edge it takes, the clearance is the scroller's content and indicator insets, and the owner's wrapper padding on that edge is zero. On an edge it does not take, the wrapper pads, so an in-flow control above the scroller, a pane beside it, or a composer below it is cleared (C33a). The scroller's effective adjustment is the same number the wrapper would have added: `max(0, local − credit)`, and zero on an edge D4 turns off. On iOS the executor is UIKit's own adjustment (`.scrollableAxes` or `.always`, as today), reconciled to that number. UIKit adds to `adjustedContentInset` the scroll view's own safe-area overlap ([`safeAreaInsets`](https://developer.apple.com/documentation/uikit/uiview/safeareainsets), [`adjustedContentInset`](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset)), which is what remains of `local` after the authored padding the scroller sits inside, not `local` itself. On each edge it takes, Exact reads that actual contribution and sets the scroller's own `contentInset` on the edge to the difference between the residual it requires and the contribution, positive or negative, so the sum is the residual; it never subtracts the credit from the contribution blindly. Worked, top edge, `local` 106: the root's `padding-top: env(safe-area-inset-top)` resolves to 59 and references the inset, so `credit = 59` and the residual is 47; the scroller inside that padding overlaps the safe area by 47, so UIKit contributes 47 and Exact adds 0. With a plain `padding-top: 16`, `credit = 0` and the residual is 106 (clearance is added to authored padding); UIKit contributes 90 and Exact adds 16. On an edge D4 turns off the residual is 0, and Exact adds the negated contribution. Stage 2 verifies credited padding, uncredited padding, an opted-out edge, content shorter than the scroller and a changing large-title inset. Exact normalizes scroll offsets so CSS `scrollTop` 0 rests at the expanded inset. The collapsing large title stays native (LLP 1035.001 D9).
- **Side edges are always wrapper padding**, on every owner, whatever scrolls. A horizontal or bidirectional primary scroller is narrowed by its left and right clearance like any other root, its propagated background still reaches the edges, and its horizontal content insets stay zero. This is the simpler of the two determinate choices: wrapper padding is then the only thing that changes the containing block's width, so the percentage equation above is exact as written, and a top or bottom residual (wrapper padding or a scroller's content inset) is computed once `W` is known.
- **Otherwise,** clearance is the wrapper's inset on that edge.
- **A lifted header** (one a bar replaces, `HostCover::Whole`) is `display: none`: its padding neither clears nor is credited. The transfer of its `env()` padding to the route's top cover (`header_inset`, `kernel/src/kernel/cover.rs`) goes, because `local` is the whole of the owner's top safe area. A route whose lifted header carried no `env()` padding changes from a bar-height cover to the full local top (44 to 106 in `kernel/tests/it/cover.rs`); the test changes with it.

**The wrapper and box sizing.** Clearance is applied inside the owner's block, outside the authored root. The kernel lays the root out in a **wrapper** of two boxes in place of the block's own root box: an outer box the allocation's size, positioned, and an inner box inset in it by the clearance on each edge, which is the root's containing block whether the root is in flow or absolutely positioned. LLP 1115.002 D8.2's rules for the root (a `static` root is `relative`; under a maximum it is sized as a modal `dialog`) apply in the inner box. The authored root's own box model is unchanged: it sits in a containing block of the allocation minus the clearance. Example: a 653-point sheet with a 20-point right clearance gives a 633-point containing block.
- A root with `width: auto` is 633 wide.
- A `box-sizing: border-box` root with `width: 100%` and `padding: 16` is 633 wide.
- A content-box root with `width: 100%` and `padding: 16` is 665 wide and overflows, exactly as it would in any CSS containing block 633 points wide.

The side covers that pad a route today (`HostCover::Edges` right and left from `NavigationBarIOS.reportCovers`, laid out by `kernel/src/kernel/cover.rs`) go: a side's clearance is the block's. Borders, min and max sizes, and absolute descendants anchored to the root behave as in any 633-point containing block.

**Backgrounds extend.** The wrapper paints the authored root's computed `background-color` across the whole allocation, clearance included, as LLP 1008 §9 paints the first root's background behind the status bar. This is Exact's chosen subset of CSS's root-background propagation (CSS Backgrounds 3 §2.11.1), which also propagates background images; Exact propagates the colour only, and the amendment to LLP 1001 declares the subset. Content clears the chrome; the surface does not. A background image or gradient stays in the root's own box. Artwork under the chrome is written by turning the edge's clearance off and padding the foreground: `-exact-safe-area="none"` (or the edges concerned) on the root, the artwork in the root, and `padding-top="var(--exact-safe-area-inset-top)"` on the content box beside it (D4, D9). The root itself cannot reach under the clearance otherwise: a block's root is positioned (a `static` one becomes `relative`, LLP 1115.002 D8.2), so it is the containing block of its absolutely positioned descendants (CSS 2 §10.1), and they stay inside the inner box.

**Per host:**
- **iOS, iPadOS, tvOS.** As above.
- **macOS.** Two containers host Exact content as host blocks today: a split's detail pane (`BlockHostMac`, `SplitMac`, LLP 1115.002 D4) and a modal route's sheet (`RouteSheetMac`, D7.4). They become owners with `NSView.safeAreaInsets` in stage 3, the pane reporting a placement and the sheet a top-layer box (D2). Routes in a window's own navigation are not projected yet (LLP 1075.003 §3.11); a window becomes an owner when they are, clearing its title bar and toolbar under a full-size content view. HTML dialog lifting (`DialogsMac.swift`) is a separate top-layer mechanism and follows D2's rendering ancestry.
- **Web.** The root gets no automatic clearance. The web host's emulated chrome (its sheets and bars, `nav-chrome.js`) is a set of owners whose `local` is that chrome; physical insets stay the root's `env()`.
- **Linux, Windows.** No native chrome around content: `local` is zero everywhere. The rules apply unchanged, so an app behaves the same.
- **Terminal.** Not applicable: no safe areas.

### D4 — `-exact-safe-area`: the per-edge opt-out

```
-exact-safe-area: auto | none | [ top || right || bottom || left ]
```

- An `-exact-` CSS property (LLP 1081), declared as a deviation in LLP 1001: CSS has no word for it.
- It applies only to an owner's authored root, is not inherited, and defaults to `auto`. On any other node it is refused at lowering (`lower-attr-value`), since it would create no owner.
- `none` turns off Exact's local clearance on every edge. An edge list turns it off on exactly the listed edges and keeps it on the others, as SwiftUI's `ignoresSafeArea(edges:)` lists the edges it ignores. Authored padding is untouched. The background still extends (D3).
- The SwiftUI counterpart is `ignoresSafeArea(.container, edges:)` on content.
- It does not apply to the session root, which has no automatic clearance: the root's choice is `viewport-fit`. It changes clearance, never allocation: `edgesForExtendedLayout` is not its counterpart.

```contract
column navigationKey="gallery" -exact-safe-area="top" background-color="#000"
  image "hero.jpg" width="100%"
```

### D5 — `calc()`, `min()`, `max()` and `clamp()`, and the local inset as a term

LLP 1001 §2 admits CSS Values 4 §10's math functions (`kernel/src/style/calc/`): `calc()`, `min()`, `max()` and `clamp()`, nested, with px and the absolute units, percentages, viewport lengths, `env(safe-area-inset-*)` and `env(viewport-segment-*)` (with or without a fallback) as values, `+` and `-` between lengths and `*` and `/` by a number, within §10.8's 32 values and 32 levels, simplified to an interned tree (§10.10) and range-clamped for the property (§10.12). A tree with a percentage is evaluated at layout against its basis; the rest resolve when the environment does. It is wire kind 24 (`style/calc/wire.rs`), and the web hosts write it back as CSS text.

This LLP adds one leaf to that tree: `var(--exact-safe-area-inset-<edge>)` (D9), standalone or in any position a length takes, without a fallback. It is the only custom property a length reads. It resolves per node, against the local insets that node inherits, so a tree that also holds a percentage is handed to layout with the node's local insets already in it as points. On the wire it is an `env()` leaf with the tags 4–7 (top, right, bottom, left), between the safe-area insets' 0–3 and the segments' 8–13.

What remains of the percentage fixed point (D3, "Percentages") is evaluated on this tree: every admitted expression is piecewise linear in the containing block's width.

### D6 — Reading local values in logic: a fenced `container:` fact

Most adaptation needs nothing: percentages, flex and grid resolve against the allocation, and a parent can pass a prop. For logic that must branch on a container's size or local insets:

```contract
shape ContainerGeometry
  available: bool
  generation: number
  width: number
  height: number
  safeTop: number
  safeRight: number
  safeBottom: number
  safeLeft: number

resource detail = exactSurface("container:detail") as shape ContainerGeometry
derive wideDetail = detail.available and detail.width - detail.safeLeft - detail.safeRight >= 600
```

- `container:<navigationKey>` names a live owner. A duplicate live name is refused.
- The values are the owner's committed allocation, before padding; the safe fields are `local` (D3), never the keyboard.
- `available` is false before installation, after disposal, and while the owner is retained but inactive. `generation` changes per incarnation, and stale callbacks are dropped.
- No size-class field: numbers only, as LLP 1039 D1 rules. A native hook reads traits.
- **Feedback fence.** What the fact reports must never depend on a branch that reads it.
  - **Content-sized owners are refused:** `navigationSizing="fitted"`, or a detent resolved from content.
  - **Independence.** The owner's reported allocation and insets must not depend, by any path, on a branch that reads the fact. The paths are:
    - the presentation: `navigationPresentation`, `navigationSizing`, `navigationDetent`;
    - the chrome Exact projects from authored content, which `local` includes: whether a header is shown (the bar), the heading's level (large or inline title, which changes `safeTop`), the tablist, and any attribute projected into a bar or tab item.
  - **Scope.** The chrome and presentation that count are the owner's own and every ancestor owner's, because an ancestor's bars and tablist contribute to the owner's safe area.
  - **Indirect paths count.** A read can reach chrome directly, through a `when` or a `derive`. It can also reach it through an action: the action reads the committed fact and stores state, and that state drives `aria-level`, a bar, a tab or a presentation attribute in a later batch. The cycle `safeTop` 106 → inline title → 54 → large title → 106 is refused whichever path it takes.
  - **At bake:** any static path from a read of `container:<key>` (in a `derive`, a `when`, or an action that writes state) to an attribute that feeds the owner's own or an ancestor's projected chrome or presentation is refused with `bake-container-feedback`. A path the bake cannot rule out is refused.
  - **At runtime:** nothing is held. The bake refuses every path from a read to the chrome or presentation it could feed, so a change the runtime sees did not come from a read, and the fact publishes it as it publishes any other change: a heading that an unrelated action switches from large to inline is reported as the new `safeTop`. A host-side cause the bake cannot see (a native hatch changing a bar) is not a Contract path and is not fenced; if a consumer shows one forming a cycle, the fact waits for a design that covers it rather than freezing on any change.
- **Prerequisites.** It needs a narrow amendment to LLP 1039 D5, and to the bake check that limits `exactSurface` names to canvas surfaces. It lands only with a consumer.
- **Fold segments.** Local fold segments are **deferred**: four edge insets cannot describe an interior hinge, and automatic edge clearance does not protect one. When a consumer needs them, ordered owner-local rectangles (`segments: list<ContainerSegment>` with `index`, `x`, `y`, `width`, `height`, relative to the allocation) join this record. They preserve LLP 1078's margin-inclusive calculation, never subtract margins twice, and count only active divisions. A sheet wholly on one side of a fold has one local segment while the document has two.

### D7 — Presentation attributes

Exact implements these on every host, so they are first-class Contract attributes next to `navigationPresentation` and `navigationDetent`. That follows the pattern of `keyboardDismissMode`, `closedby` and `retainFocus`. `data-*` remains the app's own words for its own hooks (LLP 1075.003 §3.3). This is a narrow admission by the owner; `DEFERRED.md`'s broader restriction on native property names in Contract stays, and `kernel/tables/schema.json` is the authority for both rows.

**`navigationSizing="automatic | form | page | fitted"`** on a presented route. It maps to UIKit's form and page sheets and SwiftUI's `presentationSizing`; the web gets bounded recipes in the page module.
- The default is `page`, today's page sheet. Moving to `automatic` (form on iOS 18 and later) is a separate, visible change (Q1).
- `fitted` refuses explicit detents. It measures the owner's content at its final usable width, the sheet's width less the side clearance (D3), with no top or bottom clearance and no keyboard, within a maximum bound (the sheet's `maximumDetentValue` on iOS), then adds the top and bottom clearance once. The width is the final one because side clearance changes wrapping, and so the height: measured at the full width, a line that wraps once the side clearance is taken would be clipped. Percentage and fill children cannot determine their own enclosing fitted size.

**`keyboardAvoidance="auto | none"`** on an owner, next to `keyboardDismissMode`.
- `auto`, the default, converts the keyboard's occupancy into the owner's coordinates and intersects it with the owner's allocation, after clearance and after any resize the platform already applied to that allocation (a medium sheet UIKit grows for the keyboard). The dismissed guide's safe-area baseline is excluded.
- A docked keyboard's residual overlap becomes the bottom content inset of the owner's primary scroller when the focused editor is inside that scroller and the scroller reaches the bottom edge (D3). Otherwise it becomes bottom padding on the wrapper: a composer below the scroller is lifted above the keyboard, and the scroller above it shortens. In both cases the focused editor is scrolled into view by its nearest scrolling ancestor.
- A floating or split keyboard is tracked as a rectangle: the caret is revealed, and no bottom padding is added.
- Nested owners consume the overlap once, innermost first.
- `none` turns off Exact's extra accommodation. Document policy (`interactive-widget`, including `resizes-content`, which legitimately shrinks the document viewport) and the platform's own behavior stay.
- `env(keyboard-inset-*)` is not added (Q3).

**`navigationArrangement`** (fold panes: split, overlay, span) is deferred until an app needs it.

### D8 — Readable width and margins

Plain CSS: `max-width: 42rem; margin-left: auto; margin-right: auto`. UIKit's `readableContentGuide` documents no fixed width, so a number such as 672 is the app's choice. Default padding stays zero on every host. No fact or element is added.

### D9 — Reading local values in CSS: `--exact-safe-area-inset-*`

Four host-defined custom properties, `--exact-safe-area-inset-top`, `-right`, `-bottom` and `-left`, registered as `@property { syntax: "<length>"; inherits: true; initial-value: 0px; }`, as `--exact-accent` is registered (LLP 1069.011) and as `index.html` registers `--exact-press-factor`. They are Ionic's `--ion-safe-area-*` in Exact's spelling (LLP 1081).

- **The host sets them on the wrapper**, on every target: each owner's wrapper (D3) carries that owner's `local` values (D3), and the session root's wrapper (the document's root element on the web) carries the insets of the area the document covers: the session view's safe-area insets when the root covers the screen (D1's first and third rules), 0 when it is fitted inside the safe area. Under `cover` that equals `env(safe-area-inset-*)`. Under `auto` with a native primary container it is the view's real insets while `env()` is 0: a root-level box outside every owner (a toast, an overlay) pads with `var()` and stays clear. This is a deliberate difference from `env()`, declared in LLP 1001. For these four properties the wrapper is the authored root's parent in inheritance: an authored root inherits from its wrapper, and the wrapper from the owner's logical parent. On the web the wrapper is a real ancestor element. In the native kernel, an inheritance walk that reaches an owner's authored root takes the owner's host value next, and only then continues to the logical parent. They change with every coherent commit (D2), transitions included.
- **They inherit** to every descendant as computed values (CSS Variables 1), so a box deep in a sheet reads the sheet's inset without any per-element `env()`.
- **Authors may override** them on any node, as attributes spelled exactly as the properties: `--exact-safe-area-inset-bottom="0px"`. Descendants then inherit the authored value. The value is any length the kernel admits; a relative length (`em`, `rem`) computes at the declaring node and the computed length is what inherits, as a registered `<length>` does. A declaration that is invalid at computed-value time (a cycle through `var()`, a value outside the syntax) behaves as `unset`, per CSS Variables 1 §3.1 and the Properties and Values API: the node inherits its parent's computed value, so an invalid override on an owner's authored root reads the wrapper's host value, and only a node with nothing to inherit reads `0px`.
- **In a length,** `var(--exact-safe-area-inset-<edge>)` is a term beside `env()` in `calc()`, `min()`, `max()` and `clamp()` (D5), and alone. The native kernel resolves it per subtree from the nearest ancestor-or-self computed value, as it resolves `var(--exact-<role>)` (`kernel/src/style/roles.rs`); a change to an owner's committed insets re-derives and dirties exactly the nodes whose style reads the term, as `set_env` does for `env()`. The web host writes the property on each owner's wrapper and on the root element (`--exact-safe-area-inset-top: env(safe-area-inset-top)`) and passes authored `var()` text through for the browser to resolve. The JS target does the same.
- **Credit.** A reference in an owner root's padding is credited exactly as `env()` is (D3), so "at least 16, or this owner's inset" needs no opt-out.
- **On Linux, Windows and the terminal** every value is `0px`.
- **Lowering.** The four rows are the first Contract attributes whose names begin with `--`. The Contract lexer admits only the `-webkit-`, `-apple-` and `-exact-` prefixes today, so stage 2 extends its attribute-name rule (and the Lean syntax if it constrains names). `kernel/tables/schema.json` declares the rows, and the native kernel and both web targets write the same canonical CSS text for them.

### D10 — The layout regions: `env(-exact-<region>-inset-<edge>)`

An app gets the same layout regions a UIKit or AppKit screen gets, as `env()` lengths beside `safe-area-inset-*`, so spacing that follows the platform is written once and changes with the device: the margins a controller's root view has, the readable width, and each of these and the safe area kept clear of rounded display or window corners ([`UIView.LayoutRegion`](https://developer.apple.com/documentation/uikit/uiview/layoutregion), iOS and tvOS 26; [`NSView.LayoutRegion`](https://developer.apple.com/documentation/appkit/nsview/layoutregion), macOS 26). They are document facts, as `env()` is (D1); per-owner values come with D9's custom properties at stage 2.

| Name | UIKit | AppKit |
|---|---|---|
| `-exact-safe-area-corners-inset-<edge>` | `.safeArea(cornerAdaptation:)` | `.safeArea(cornerAdaptation:)` |
| `-exact-layout-margins-inset-<edge>` | `.margins()` | `.margins()` |
| `-exact-layout-margins-corners-inset-<edge>` | `.margins(cornerAdaptation:)` | `.margins(cornerAdaptation:)` |
| `-exact-readable-content-inset-<edge>` | `.readableContent()` | the margins |
| `-exact-readable-content-corners-inset-<edge>` | `.readableContent(cornerAdaptation:)` | the margins, corner-adapted |

- **What a value is.** The distance at that edge from the layout viewport to the region, as the platform lays it out, and never less than zero. By default a region lies inside the safe area (UIKit's [`insetsLayoutMarginsFromSafeArea`](https://developer.apple.com/documentation/uikit/uiview/insetslayoutmarginsfromsafearea)); where an app turns that off, its margins can lie outside it, and the value says so. Under `viewport-fit="auto"` a region is measured from the safe area, where `env(safe-area-inset-*)` is zero, so a margin is what lies beyond it; that holds too where a native primary container takes the whole view (D1).
- **Corners.** A `-corners-` value takes left and right from the region's horizontal corner adaptation and top and bottom from its vertical one. Measured on UIKit, each axis moves only its own edges (the iPhone Duo's inner panel: the safe area's left 0 becomes 16 horizontally, its top 0 becomes 16 vertically, and neither moves the other), so the combined set loses nothing.
- **Which view.** UIKit's margins depend on the view: a controller's root view takes the system minimum (16 or 20 points), a plain subview 8. The session's view is usually a subview, so the iOS host reads the regions of the root view of the nearest controller that holds it. The Mac reads the session's view.
- **The source.** On iOS and tvOS 26 and macOS 26 every value comes from the layout-region API, whose readable content can differ from the older `readableContentGuide` (on the Duo's inner panel the region's left is 52 points where the guide's is 20); earlier systems use `layoutMarginsGuide` and `readableContentGuide`, with no corner adaptation.
- **Other hosts.** A host that reports no regions resolves each to its safe-area inset: the web and JS targets write `env(safe-area-inset-<edge>)` for the browser, inside a calculation too, and Linux, Windows, Android and the terminal take the kernel's fallback. This deviates from the platforms that have margins and is declared in LLP 1001 §2. The agent's `env` facts name all twenty values on every host.
- **Wire.** A dimension of kind 25 carries the offset, the region and the edge, and a calculation tree's `env()` term 16 + region × 4 + edge (`kernel/tables/schema.json`). A change re-derives exactly the nodes that read the environment, as the safe-area insets do.
- **Credit.** A covered header whose top padding reads the corner-adapted safe area's top clears the inset as `env(safe-area-inset-top)` does, since that value is never less; the margins' and readable content's can be, so they do not.
- **Deferred calculations.** A length the Apple host resolves against a basis it learns later (`exact_calc_create`) carries the regions in its environment snapshot.

## What it replaces

Once stage 1 proves it, these are deleted:
- `ModalHost.coordinateView` as the viewport source in `ExactViewIOS.fit`, and the sheet's part in the window size (`Env::screen`, LLP 1075.003 §9.11): with the document never in the sheet, no presentation changes either. `Env::screen` itself stays: under `interactive-widget="resizes-content"` the keyboard ends the viewport at the keys while `vh` stays the window's;
- moving `presenter.viewport` and the presenting navigation into the sheet (`ModalHost.present`, `NavigationHost.move(to:)` selecting the modal navigation);
- holding the background's geometry while a sheet is up (`ModalHost.deferGeometry` and its replay): the background is laid out at its own size throughout;
- the dismissal-width cover guard in `NavigationBarIOS.reportCovers`;
- side covers (`HostCover::Edges` right and left) and the `header_inset` transfer (`kernel/src/kernel/cover.rs`);
- an app-side workaround that holds an app's layout wide across a sheet's dismissal.

The provisional-title sampling (`scrollOriginProvisional`) goes only if the publication rule covers its case. The matching `QUEUE.md` entries go too.

## Host matrix

| Host | Owners | Clearance source | Presentation | Focus and dismissal |
|---|---|---|---|---|
| iOS, iPadOS | Routes, sheets, full-screen, tab panels, app containers | Controller `safeAreaInsets` | Page, form, full-screen, detents | Background inert; focus enters the sheet, returns on close; interactive dismissal and cancel |
| tvOS | Routes, full-screen presentations | Controller `safeAreaInsets` (overscan) | Over full screen | Remote focus; Menu dismisses |
| macOS | A split's detail pane and a modal route's sheet, each already a host block (`BlockHostMac`, `RouteSheetMac`, LLP 1115.002 D4, D7.4); a `dialog`'s sheet; windows with Exact content as routes are projected (§3.11) | `NSView.safeAreaInsets` | Window sheets | Key-view loop; Escape |
| Web (JS and wasm) | Emulated sheets and bars | Emulated chrome only; root uses `env()` | Page-module recipes | `inert` background; focus per HTML |
| Linux, Windows | Routes | None | Host's own | Host's own |
| Terminal | — | — | — | — |

Layout regions (D10): UIKit's on iOS, iPadOS and tvOS, AppKit's on macOS (its margins standing in for readable content), the safe area on every other host.

## Staged plan

Each stage builds on LLP 1115.002 D8.2's host block, which the kernel and the Mac already have.

1. **Ownership on iOS, on host blocks.**
   - Each presented owner's root is reported to the runner as an open top-layer box, and each ordinary owner's root as a placement that keeps its place among its siblings (D2), so `client_rect`, `elementFromPoint` and the agent's coordinates and screenshots map through where it is shown. Placements are sent when they change, with the block.
   - The root fits from its own chrome only (`fit` from the session view, under D1's one rule).
   - A sheet and a full-screen presentation mount their route's node view in the presented controller's view and give it a block, as `BlockHostMac` does for a pane; the document's viewport stays in the session view.
   - Routes in the primary stack and tab panels are owners too: each route's root gets a block its controller's view bounds.
   - A docked keyboard ends the blocks of the top presentation's routes, where the focused editor is, at the top of the sheet's keyboard layout guide, which UIKit lays out as it grows the sheet, which follows no undocked keyboard, and which leaves out the bottom safe area; the route's own scroller reveals the editor once the keys settle, and the sheet's bar stays where it is. This reproduces what the document viewport moved into the sheet did, a containing-block resize, without moving the navigation. D7 replaces it with the owner's accommodation.
   - Session-owned mount membership replaces viewport-descendant checks for keyboard ownership (`hasNativeKeyboardEditor`), the keyboard toolbar and native editors.
   - Modal input and accessibility gating, focus entry and restoration, and cancellation preservation move with the mount. A presented route's logical ancestors still hear its touches (pointer observers, a long press's `contextmenu`, a double tap's `dblclick`) through recognizers on the mount, and the runner's point reads treat what lies behind a presentation as inert.
   - Proof: a wide document with a narrow sheet.
     - The background's width, units and facts stay the document's through presentation, detent drags, interactive dismissal and its cancellation.
     - Sheet percentages are the sheet's.
     - Typing, VoiceOver and coordinate taps work in the sheet, throughout; structural reads agree at settled geometry and report a native animation's destination while it runs (above, "Native UIKit transition tests").
2. **Local clearance (D3, D4, D9)** in the block: the wrapper and its clearance, credit by reference (D5's leaf), one executor per edge, the lifted header uncredited, background propagation, `-exact-safe-area`, the four custom properties. Side covers go.
3. **Other hosts.**
   - macOS: the pane and the route sheet become owners with `NSView.safeAreaInsets`; windows and dialogs with projection.
   - The web, JS and wasm targets, with production chrome. The JS navigation chrome must not turn off under `?agent` for these tests.
   - tvOS, driven by the remote.
   - Linux and Windows conformance.
4. **D3's percentage fixed point** on D5's trees.
5. **Keyboard per owner (D7).** Stage 1's containing-block resize of a sheet's route gives way to D7's single executor, which must answer the keyboard design review of stage 1 (2026-10-13):
   - The owner's physical allocation stays separate from keyboard accommodation: a bottom-reaching primary scroller takes an inset, and only otherwise does the wrapper shorten, so percentage content and rows do not reflow for scrolling clearance.
   - Occupancy is owner-local. Focus reveal, geometry-change reveal and the keyboard toolbar read the consuming owner's occupancy, never the document's `keyboardInset`, which a sheet's keyboard need not change.
   - Policy precedence: whether a sheet's keyboard leaves the document's viewport alone (as UIKit leaves the presenting screen) applies to the implicit native default only; an explicit root `interactive-widget` and an owner's `keyboardAvoidance="none"` are honored. Stage 1 keeps the document resizing as before under every policy.
   - An undocked, floating or split keyboard, or a hardware keyboard's assistant bar, is a rectangle: the caret is revealed against it in the owner's coordinates on both axes, with no bottom padding or shortening; a notification's frame is converted only from its own screen.
   - Reveal covers every editor: a fixed-position field with no scrolling ancestor, a native module's editor, and a caret inside a large text area under a floating keyboard.
   - Keyboard insets compose with the keyboard toolbar's lift and `PinnedRoom`'s saved baseline per scroll backend, each removing only its own contribution.
6. **`navigationSizing` (D7) and the `container:` fact (D6),** each with a consumer. A fitted sheet is a block whose height is a maximum (LLP 1115.002 D8.2), the measurement the `fit-content` detent makes today (LLP 1075.003 §9.11).

## Tests

**Kernel and runner conformance on every host:**
- Document facts are unchanged by a presentation, and sheet-local percentages (width and height) resolve against the sheet.
- Every row of D3's table.
- An 84-point side strip, `additionalSafeAreaInsets`, and box-sizing with borders and min and max sizes.
- An opt-out per edge, including a single-edge list (`-exact-safe-area="top"` turns off only the top); the lifted header without `env()` padding (106, not 44).
- Nested owners: a route inside a tab panel clears the tab bar once (83, not 166).
- A primary scroller that does not reach an edge (C33a): the wrapper pads that edge; a composer below the scroller is lifted above a docked keyboard.
- Root fitting (D1): the three rules; hiding a primary route's header leaves `exactViewport()` unchanged; the session root's `--exact-safe-area-inset-*` under `auto` with a native primary container is the view's insets.
- D9: the root's values equal `env()` under `cover`, and are 0 when the root is fitted; a sheet's equal its `local`; inheritance to a deep descendant; an authored override and its descendants; an `em` override inheriting as computed pixels; a cyclic override under a parent set to `34px` reads `34px`, and on an owner's root reads the wrapper's host value; the web's `@property` registration and a `var()` in `max()` resolved by the browser; zero on Linux.
- D5's examples, including `clamp(100px, 20px, 50px)`, `clamp(none, 20px, 50px)`, `min(50%, env(…))`, and an undefined segment in an unselected branch.
- D3's percentage fixed point: `min(20%, env(…))` on a 100-point allocation; the same on a horizontal primary scroller (the same 17.5 and 16.5: side edges are wrapper padding; its horizontal content inset stays 0); a percentage on both side edges; the same inputs laid out twice give the same clearance.
- D10: each of the twenty names parses, resolves to the host's value and to the safe area on a host with none, travels the wire alone and inside a calculation and in the Apple host's deferred calculations, and reaches a browser as `env(safe-area-inset-*)`, escaped or bound to state too; a header's credit counts the corner-adapted safe area only; on the iPhone Duo's panels, rotated and folded, every value equals what a plain UIKit app's controller root view reports.
- D6's fence: a branch on `safeTop` that selects the heading's level is refused at bake; a chrome change no read caused (an unrelated action switching the heading's level) is published, not held.

**Implementation-phase obligations** (test cases owed at each stage, not normative additions): a `calc()` whose `env()` term cancels to zero still receives credit by reference; a translucent propagated background is painted once, not by the wrapper and the root both; a fitted sheet's final allocation height is distinguished from the safe-area-excluding value returned to UIKit's detent resolver, with clearance added once.

**Native UIKit transition tests.** Geometry, coordinate taps and screenshots must agree at settled geometry after each of these, and stay usable through them. During a native animation the structural reads (`frame`, `elementFromPoint`, placements) report its destination, as they report a CSS transition's end: the host publishes UIKit's model geometry, not the presentation layer's. Sampling the displayed position mid-animation is later work (QUEUE.md), because it needs a per-frame reader of presentation layers that stage 1 does not add:
- presentation and detent drags;
- interactive dismissal and its cancellation;
- rotation and a Stage Manager resize;
- Duo fold changes mid-sheet, including a fold with no bounds change;
- keyboard docking and undocking.

**Rendering ancestry:** a sheet opened under a transformed, scrolled, `overflow: hidden` ancestor; sticky headers, text flow, fragmentation and auto-height transitions in the sheet; a kernel rebuild with a sheet open.

**Keyboard:** `resizes-content`, a medium sheet growing for the keyboard, hardware-keyboard accessories, a floating keyboard near the top, nested editors, and a dismissed keyboard counting as zero.

**Before the dismissal guard is deleted:** a stock UIKit comparison on the Duo of the 1249-wide, 172-inset dismissal geometry.

## Amendments on acceptance

- **LLP 1001:** declare the automatic-clearance deviation, the D3 formula, `-exact-safe-area`, the colour-only background propagation, the `var(--exact-safe-area-inset-*)` term in the environment section, the third root-fitting rule, which covers the view while `env()` is 0 (D1), and the session root's custom properties differing from `env()` under `auto` with a native primary container (D9). Correct §2's sentence that the insets are zero "as a browser reports the insets for a page without `viewport-fit=cover`": Safari reports the edges its `auto` leaves unfitted (§"Evidence").
- **LLP 1035.001 D4, D5:** the viewport no longer moves into the sheet, and the keyboard is measured per owner (D1, D2, D7); their wording follows.
- **LLP 1039 D5:** the `container:` admission (D6).
- **LLP 1075.003 §3.5, §3.6:** content owners and their clearance, replacing route-only covers and the per-child side padding.
- **LLP 1007 and LLP 1008 §9:** host sections; the safe-area paragraph of §9 rewritten for owners, with its description of the fitted viewport as "a browser's rule for a page without `viewport-fit=cover`" corrected the same way as LLP 1001 §2.
- **LLP 1078:** owner-local segments noted as deferred to D6; D5's sentence that occlusion regions "stay what they already are, the safe-area insets" is narrowed to what `safeAreaInsets` reports (C14a).
- **LLP 1115.002 D8.2:** an owner's block carries local insets and a primary scroller (D2), its root is laid out in the wrapper's inner box (D3), and a block's root is reported to the runner where it is shown: a presentation's as a top-layer box, any other's as a placement, the block's rectangle in the local coordinates of an ancestor that holds it in the host as in the document, that the runner's ordinary walk steps through (D2).
- **`kernel/tables/schema.json`:** the authority for `-exact-safe-area`, `navigationSizing`, `keyboardAvoidance`, the four `--exact-safe-area-inset-*` rows, and the local inset's leaf in a calculation's wire form.
- **`host/web/index.html`:** the four `@property` registrations.
- **`docs/contract-for-humans.md`, `docs/contract-for-agents.md` and `docs/agent-pitfalls.md`:** replace the `viewport-fit=cover` pitfall; document D9's idiom.

## Open questions

- Q1. Should `navigationSizing`'s default move from `page` to `automatic` (form on iOS 18 and later) as a separate, visible change? Recommendation: later, on its own, with screenshots of every sheet in the repo's apps.
- Q2. Should Linux and Windows hosts project native chrome as owners when they gain it? Recommendation: yes, under the same rules.
- Q3. Should `env(keyboard-inset-*)` (LLP 1008 §9, unbuilt) come with `keyboardAvoidance`? Recommendation: no; D7 accommodates the focused editor without it, and the six variables wait for an app that lays out against the keyboard's rectangle.
