# iOS tab bar covers route content under viewport-fit=cover and absolute footers

**Status:** Closed
**Resolution:** fixed by bbd846b41: a route is covered under viewport-fit=cover too, and absolute children clear a parent's cover
**Systems:** Apple host navigation covers (NavigationCoversIOS), env safe-area, docs/recipes/tabs.md
**Severity:** P1
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

In a tab, a cart's Undo button and a purchase footer sit under the tab bar; the author hard-coded 84 pt of bottom clearance.

## Cause

A tab's route fills the controller's view, which UIKit extends under the bar (`OwnersIOS.swift:197-222`). The route is protected by a cover added to its padding, `min(safe.bottom, reached) - env.bottom`, only for in-flow content when the view is "whole" (`NavigationCoversIOS.swift:27-34, 99-109`). Under `viewport-fit="cover"` `whole` is false, so there is no cover, and `env(safe-area-inset-bottom)` is the session view's own inset (`ExactViewIOS.swift:340-347`), about 34 pt for the home indicator, not the 83 pt a child of a `UITabBarController` reads from `safeAreaInsets`. An absolutely positioned footer is laid out against the padding box, so `bottom=0` is the screen edge. A hand-built screen pins its footer to `safeAreaLayoutGuide`, whose bottom includes the bar.

## Fix

Give a tab's route the inset a UIKit child reads: `env(safe-area-inset-bottom)` includes the tab bar's height while a tab bar is shown, so `bottom="env(safe-area-inset-bottom)"` and `padding-bottom` clear it under `viewport-fit="cover"` as they do on the web with a fixed bar. Say so in the tabs recipe.

## Fixed (2026-10-19)

Two halves. The Swift host reports a route's bottom and side covers under `viewport-fit="cover"` too (`covered`: the bar's part past the home indicator `env()` already holds, 49 pt under a tab bar), and no longer zeroes the bottom when a scroller reaches the bottom edge. The kernel decides the rest (`cover::apply`): a route whose child scroller the platform insets at the bottom pads nothing there, so the scroller still reaches under the bar; and an absolutely positioned child of a covered box has each length inset moved past the cover, so `bottom=0` rests on the bar as on the web it rests above the tablist in flow (a percentage inset stays as written). A cover change restyles the box's absolute children and a child's inset cover restyles its parent. Tests: `kernel/tests/it/cover.rs::an_absolute_footer_in_a_covered_route_clears_the_bar_it_rests_on`, `ContentOwnersIOSTests.testASheetsRouteIsCoveredForItsOwnSafeArea`. Not driven on a simulator here.
