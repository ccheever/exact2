# ARIA disabled is lost in macOS standard toolbar viewers

**Status:** Closed
**Resolution:** Standard toolbar viewers and grouped-list cells carry ARIA disabled without blocking input; lifted popovers, production context-menu previews and custom rows retain authored ancestry for state and descendant refresh. Covered by HeaderMacTests.testARIAUnavailableToolbarViewerRemainsPressable, GroupedListIOSTests.testARIAUnavailableReachesStandardCellsAndCarriedRows, NativeContextsIOSTests.testLiftedContentKeepsItsAuthoredARIAUnavailableAncestor and ContextMenuLifetimeIOSTests.testLiftedPreviewKeepsAuthoredARIAUnavailableAndRefreshesItsButton.
**Systems:** AppKit, accessibility
**Author:** ariadisabled review fix
**Date:** 2026-10-11

A button projected into a standard NSToolbarItem remains input enabled for aria-disabled, but the exposed accessibility object is AppKit's private NSToolbarItemViewer and still reports AXEnabled=true. NSToolbarItem itself has no accessibilityAttributeValue implementation and its view is nil for standard items. Keep the native toolbar presentation and find a supported way to carry the accessibility state onto the viewer; test VoiceOver's exposed element and press separately.

## Also open (2026-10-12, final review of the aria-disabled lane)

- **iOS lifted popover content loses an ancestor's `aria-disabled`.** A popover's content lifted into the top layer computes its effective accessibility state from its new superview chain, not its authored ancestry; the descendant-refresh walk does the same. This needs the menu host's saved parent: `NavigationHost.logicalParent(of:)` records routes, not popovers.

The lifted-route and grouped-list accessory-switch findings are fixed in review round 4,
covered by `NavigationTabsIOSTests.testARIAUnavailableTabPanelReachesLiftedRouteAndRefreshesItsButton`
and `GroupedListIOSTests.testToggleAccessoryCarriesARIAUnavailableAndStaysOperable`.

## Also open (2026-10-12, review of the rebased lane)

- **iOS standard grouped-list button rows.** UIKit exposes the `UICollectionViewListCell`, but `configure`/`interact` never carry `.notEnabled` to it, so a "Profile" button row with `aria-disabled=true` is announced as enabled.
- **iOS custom grouped-list rows lose an inherited state.** `GroupedListView.carry` moves a custom row into a cell; the effective-state and refresh walks (`Accessibility.swift`) know lifted routes only, so `aria-disabled` on the row's section is missed. Both walks need the grouped list's saved authored parent.
