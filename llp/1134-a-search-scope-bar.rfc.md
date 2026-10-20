# LLP 1134: A search's scope bar

**Type:** RFC
**Status:** Accepted as built, 2026-10-19, after two design rounds and three code rounds (Astra, Grok; records in `~/audits/exact2-no-tells-2026-10-11/llp1134/`). Building it measured one thing the draft did not say: without a tab bar UIKit moves the scope control out of the search bar into the navigation bar, so the host finds it in either (D4). `SearchScopeIOSTests` pass in the UIKit run (`build.mjs --test --ios`), as does the rest of that run.
**Systems:** a route's header search (`host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift` `searchField`, `HeaderSearch`), its covers (`reportCovers`) and collapse (`collapse`), segmented tablists (`SegmentsIOS.swift`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:**
- LLP 1131 (content under a fixed control), whose D2 leaves a large title over a fixed control with a hard edge: Shelf's Library (QUEUE.md, "Content under a fixed control, what LLP 1131 leaves" (1)).
- LLP 1115.000 D11.2 (a header's search field is the item's `UISearchController`), LLP 1116.008 D3 (a fixed control under the bar goes with an inline title).
- Apple, [`UISearchBar.scopeButtonTitles`](https://developer.apple.com/documentation/uikit/uisearchbar/scopebuttontitles), [`showsScopeBar`](https://developer.apple.com/documentation/uikit/uisearchbar/showsscopebar), [`UISearchController.scopeBarActivation`](https://developer.apple.com/documentation/uikit/uisearchcontroller/scopebaractivation) (iOS 16); [`UISearchBarDelegate.searchBar(_:selectedScopeButtonIndexDidChange:)`](https://developer.apple.com/documentation/uikit/uisearchbardelegate/searchbar(_:selectedscopebuttonindexdidchange:)).

## Summary

A screen with a large title, a search field and a filter of a few segments under them is,
hand-built on iOS, a `UISearchController` whose search bar carries the filter as its scope
buttons, shown always: UIKit draws the segments in the navigation bar under the title, and
they ride the bar as the title collapses and the list scrolls under it. Exact draws the
filter as a segmented control in the page, under the bar, so the title cannot collapse and
the list stops in a hard line at the control (Shelf's Library). This RFC makes such a
filter the search bar's scope buttons.

## 1. What happens now

A UIKit probe on the iOS 27 simulator (iPhone 18 Pro;
`~/audits/exact2-no-tells-2026-10-11/llp1134/`): a large title "Library", a
`UISearchController` with `scopeButtonTitles` ["All", "Reading", "Want", "Finished"],
`showsScopeBar` and `scopeBarActivation = .manual`, over a table, as one tab of a
`UITabBarController` (Shelf's arrangement):

- at rest (`tabs-rest.png`), the search field under the large title and the scope bar, a
  glass segmented pill, under the field;
- scrolled (`tabs-scrolled.png`), the title collapses to the inline one, the field and the
  pill stay under it, and the rows pass under all three;
- active (`tabs-active.png`), the field moves to the top beside its close button, the pill
  stays under it, the list under both and the keyboard up.

Alone in a navigation controller (`scrolled.png`), UIKit puts the field in a bottom toolbar
and the pill under the inline title. Shelf's Library on Exact keeps its large title expanded
and its filter fixed in the page, the rows stopping under it.

## 2. Decisions

### D1 — the shape

On an iPhone (`userInterfaceIdiom == .phone`), iOS 16 and later, a route whose header the
bar replaces whole (no part of it authored, `HeaderShape.authored` empty) and whose search
field the bar projects (LLP 1115.000 D11.2), followed, as the route's next child the author
shows in flow (by `display` and `position`, not by what the host hides), by a horizontal
`tablist` that:

- holds two to five tabs, each a pressable tab with no `contextmenu` (the scope bar has
  no long press) whose face is a title alone (`segmentFace`
  `.title`; no image, no symbol) and whose accessible name is that title, none disabled or
  `aria-disabled`, exactly one `aria-selected`, and nothing else shown;
- names no `tabpanel`: no tab has `aria-controls`. A tablist whose tabs switch panels is a
  section switcher, not a search's scope, and keeps its segmented control
  ([Apple: search fields, scope](https://developer.apple.com/design/human-interface-guidelines/search-fields));
- writes no presentation of its own, nor do its tabs: no colour, background, border, font,
  opacity or `appearance` (author > platform: an authored look keeps the segmented control, which
  carries it).

What sits between the header and a filter, or wraps it, keeps today's segmented control.

### D2 — the filter becomes the scope bar

The search bar's `scopeButtonTitles` are the tabs' titles in order, its
`selectedScopeButtonIndex` the selected tab, `showsScopeBar` true and the controller's
`scopeBarActivation` `.manual`, so the segments show whether or not the search is active;
both are set again when the search is presented and dismissed, where UIKit resets them. The
placement is UIKit's (`.automatic`), as the field's is.

The navigation host owns the tablist: it hides it (`isHidden`, as a lifted header) and
covers it whole beside the header (`coverLifted`, `reportCovers`, `hiddenForBar`), and
`SegmentHost.sync` skips it as it skips the tab container's tablist (`adopts(tablist:)`),
restoring a segmented control it had drawn. The decision is made before `collapse`, so
`collapse`'s first shown child after the header is what follows the tablist, and the
scroller rule there is unchanged (`node === scroller`, `navigationScroll` included). A
tablist that stops matching D1, or goes, is shown again, its cover released, and the scope
bar cleared (`scopeButtonTitles` nil, `showsScopeBar` false); `SegmentHost` draws it again.

### D3 — selection, both ways

A person choosing a segment (`searchBar(_:selectedScopeButtonIndexDidChange:)`) presses
that segment's tab now (`presenter.press`, as `SegmentHost.changed` does), mapping the
index to the tab the titles were taken from. The Contract's selection moves the segment,
while the search is active and the keyboard up too: setting the index touches neither the
text, nor the first responder, nor `isActive`. A choice the finger has not finished, or UIKit
has not reported, stays (the control tracking, or a `settled` index, as `SegmentHost`
keeps); the settled index is written before the press, whose batch then sets the segment
to the Contract's selection, the one before when the press declines it. Setting the index
does not press again.

### D4 — accessibility and the agent

The field keeps its own label; each scope segment is labelled by its title, which D1 makes
the tab's name, and the tablist's `aria-label` is not carried (UIKit's scope bar has no
label). Each segment carries its tab's `testId` as its accessibility identifier, set on
UIKit's segment views after layout as `LabelledSegmentedControl` does. The agent's
`tap <tab>` aims at that segment (`drawn`), and `activate` presses the tab, as for a
segmented control; `tree` shows the tablist hidden for the bar, its tabs with the
segments' frames.

### D5 — what is left out

iPadOS and the Mac, tvOS (no search controller), a partly authored header, a header whose
search the bar does not project, a tablist not right after the header, and a search with a
results controller (Exact's has none, `HeaderSearch`).

## 3. The gate

- Shelf's Library on the iOS 27 simulator beside the probe: at rest, scrolled with a drag,
  and active with the keyboard up, each against the matching `tabs-*.png`; choosing a
  segment filters the list, with and without a query, and keeps the query and the keyboard.
- XCTests (a fixture with a header search and a following tablist): the scope titles and
  index; a scope change presses the tab once, and one the press declines goes back; a Contract selection moves the segment while
  the search is active; the tablist hidden and covered whole and the list the bar's collapse
  scroller; and each of a tab with an image, six tabs, a disabled tab, a `contextmenu`, `aria-controls`, an
  authored colour, a wrapper between, a partly authored header keeps the segmented control;
  a tablist that stops matching gets its segmented control back and the bar its scope bar
  cleared. Accessibility identifiers on the segments.
- The iOS smoke, the UIKit XCTests, the tvOS compile and the six checks.
