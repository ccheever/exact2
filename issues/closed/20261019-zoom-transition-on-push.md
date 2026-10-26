# Zoom transition covers full-screen modals only, not stack pushes

**Status:** Closed
**Resolution:** fixed by bbd846b41: a pushed route with a navigationSource zooms from it on iOS 18
**Systems:** Apple host navigation (NavigationIOS, ModalIOS), docs
**Severity:** P3
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

`navigationSource="<id>"` gives a zoom transition for a `navigationPresentation="fullscreen"` route and is silently ignored on a pushed route.

## Cause

The only `preferredTransition` is in `ModalIOS.swift:582-629`, gated on `layer.kind == "fullscreen"`. Pushes go through `setViewControllers` (`NavigationIOS.swift:600`) with no transition set. iOS 18's `UIViewController.preferredTransition` applies to pushes as well.

## Fix

Set `.zoom` on the arriving top before the push when the route names a `navigationSource`, reusing the modal's source and alignment closures, and keep the swipe-back dispatch working with the zoom's own dismissal.

## Fixed (2026-10-19)

`NavigationHost.prepareZoomPush` (ZoomPushIOS.swift) gives a route pushed with a `navigationSource` the zoom transition on iOS 18 and later, reusing the modal's source and landing alignment (`ModalHost.zoomImage`, `zoomAlignment`), gated on `back` being available; `RouteController.appeared` lets the landing reset after a cancelled swipe. Wired in `NavigationHost.sync` before `setViewControllers`. Compiled for macOS only here; not driven on a simulator.
