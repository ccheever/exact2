# No way to turn off the automatic top inset under a transparent navigation bar

**Status:** Closed
**Resolution:** fixed by bbd846b41: navigationBar=transparent
**Systems:** Apple host navigation collapse (NavigationBarIOS), schema (navigationBar), docs
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

A full-bleed product photo at the top of a route cannot start under the navigation bar: the content scroller is inset below the bar and nothing in Contract turns that off, so the author used a negative margin.

## Cause

`collapse` (`NavigationBarIOS.swift:996-1038`) sets the content scroller's `contentInsetAdjustmentBehavior` to `.automatic` when a bar is shown, and `viewport-fit="cover"` lifts it only for barless routes (`:1001`). `overscroll-behavior`, `scroll-padding` and `scroll-edge-effect` do not reach it; `navigationBar="none"` removes the bar. A hand-built screen with a hero image sets `.never` and a transparent `scrollEdgeAppearance`.

## Fix

Admit `navigationBar="transparent"`: the bar's background is clear at the top (UIKit's scroll-edge appearance) and the content is not inset, as the web lays a scroller under a fixed transparent header; the author pads with `env(safe-area-inset-top)` where content must clear it.

## Fixed (2026-10-19)

`navigationBar="transparent"` (schema, `contract vocab navigationBar`, the compiler's value check, the guide): iOS gives the route's navigation item a clear scroll-edge appearance, sets the content scroller's `contentInsetAdjustmentBehavior` to `.never` instead of `.automatic`, and reports no `.inset` cover for it, so the content starts at the top edge under the bar and the bar's material comes as it scrolls. `scripts/fixtures/transparent-bar.contract` and `NavigationCollapseIOSTests.testATransparentBarInsetsNothingOfItsScroller` (iOS; not run here).
