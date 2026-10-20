# LLP 1135: A filter under a large title

**Type:** RFC
**Status:** Accepted as built, 2026-10-19, after one design round (Astra; Grok's failed) and two code rounds (Astra, Grok; records in `~/audits/exact2-no-tells-2026-10-11/llp1135/`). Building it added two things to D1: the filter must stand right above the list it filters (LLP 1116.006 D1's switcher with text under it keeps its band, `BarBandIOSTests`), and the slots must be free of a hatch's. `SearchScopeIOSTests` pass in the UIKit run.
**Systems:** a route's bar projection (`host/apple/Sources/ExactKit/IOS/SearchScopeIOS.swift`, `NavigationBarIOS.swift`), segmented tablists (`SegmentsIOS.swift`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:**
- LLP 1134 (a search's scope bar), whose filter shape (D1) and hiding (D2) this reuses; LLP 1131 D2, which leaves a large title over a fixed control with a hard edge (QUEUE.md, "Content under a fixed control, what LLP 1131 leaves" (1)).
- Apple, [`UINavigationItem.largeSubtitleView`](https://developer.apple.com/documentation/uikit/uinavigationitem/largesubtitleview) and [`subtitleView`](https://developer.apple.com/documentation/uikit/uinavigationitem/subtitleview) (iOS 26).

## Summary

A screen with a large title and a filter of a few segments under it (Shelf's Discover)
is, hand-built on iOS 26, a navigation item whose `largeSubtitleView` is the segmented
control under the large title and whose `subtitleView` is the same control under the
inline title once the title collapses. Exact draws the filter in the page under the bar,
so the title never collapses and the list stops in a hard line under the control. This
RFC projects such a filter into the two subtitle views.

## 1. What happens now

A UIKit probe on the iOS 27 simulator (iPhone 18 Pro, a tab of a `UITabBarController`;
`~/audits/exact2-no-tells-2026-10-11/llp1135/`): with both views set
(`both-0.png`, `both-400.png`), at rest the segmented control spans the width under
"Discover"; scrolled, the title is inline and a compact glass segmented control sits under
it, the rows passing under both. With `largeSubtitleView` alone (`large-400.png`) the
control goes away with the large title. This filter stays: a person filters at either
height, so both views are set. Exact's Discover keeps
its large title expanded and the list stops under its fixed control.

## 2. Decisions

### D1 — the shape

LLP 1134 D1's filter (a horizontal `tablist` of two to five text tabs, one selected,
none disabled, no context menu, no panel, no look of the author's, the route's next child
shown in flow after a header the bar replaces whole, on an iPhone), where the header has
no search, no segmented title (§9.8) and no heading group (a subtitle, an avatar, a press:
`richTitle`, whose subtitle the views would replace), the next shown child after it is a
scroller (the list it filters; a switcher with other content under it keeps the band LLP
1116.006 D1 gives it), and the route's title is large (its
item's `largeTitleDisplayMode` is `.always`: a pushed route's level-1 heading is; a
presented stack's is not), on iOS 26 and later. A header with a search takes LLP 1134's
scope bar instead.

### D2 — the projection

Two segmented controls, one per view, each the tabs' titles with the selected tab
selected: `largeSubtitleView` and `subtitleView`, each sized by UIKit (its intrinsic
size; no frame of the hidden tablist's). A person's choice in either sets the other,
presses that tab now (`presenter.press`) and reads the answer back into both, a declined
one too; the Contract's selection moves both unless a finger is on either. The bar's
height changes as they come and go, and as the text size or Bold Text changes, so the
collapse scroller keeps its CSS `scrollTop` and its sampled insets (`scrollOrigin`,
`scrollCollapsed`) are taken afresh. A route hatch that sets a subtitle, a large subtitle
or either view keeps them: the filter then stays in the page, and only the views this
projection set are ever cleared. (A subtitle a hatch sets after the filter is projected
takes its place at the route's next batch, when the filter is judged again.) The tablist is hidden and covered whole as LLP 1134 D2
does, decided before `collapse`; a tablist that stops matching is shown again and both
views cleared.

### D3 — accessibility and the agent

As LLP 1134 D4: each segment is its tab's title, carries its tab's `testId`, and the
agent's `tap`, `activate` and `layout` read the one the bar shows: in the window, not
hidden or faded out, under the large title while it is out, else under the inline one.

### D4 — left out

An inline title (LLP 1131 bands it), iPadOS, the Mac, tvOS, iOS before 26.

## 3. The gate

- Shelf's Discover on the iOS 27 simulator beside the probe: at rest, scrolled, and
  choosing a segment changes the subject.
- XCTests: both views carry the tabs, a change presses once, the Contract's selection
  moves both, the tablist hidden and covered and the list the collapse scroller; a level-2
  heading, a search header (LLP 1134) and a tab with an image keep their present forms.
- The UIKit XCTests, the tvOS compile and the six checks.
