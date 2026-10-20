# A native pop skips the app's Back action when the control is outside the route

**Status:** Closed
**Resolution:** fixed by bbd846b41: the named Back control may live outside every route, on iOS and the web
**Systems:** Apple host navigation (NavigationIOS, NavigationRules), runner router, docs
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

iOS's edge swipe and the bar's Back button pop the screen without running the app's `back` action, so the route's state outlives the screen; the author found no hook for cleanup when a route goes away.

## Cause

A native pop reaches the app in one of three ways (`NavigationRules.back`, `host/apple/Sources/ExactKit/IOS/NavigationRules.swift:105-109`; `docs/contract-for-agents.md:1438-1447`): press the control named by the root's `navigationBack`, else call the root's `navigate` handler with the location beneath, else `host_back`, which commits the pop with no app code. The press path requires the control to sit inside the active route (`backControl`, `NavigationIOS.swift:889-894`; `NavigationRules.swift:69-82`); a Back button in a shared header, the common shape, is skipped and the pop falls through to a silent commit. The web's `popped()` (`host/web/navigation.js:243-281`) has the same order. No route-removal event reaches Contract: the runner's `RouterChange.removed` goes to hosts only.

## Fix

Honour the named control wherever it is live in the tree, on both hosts, since the author named it. Document that the `navigate` handler on the root is the hook for every host pop that has no back control.

## Fixed (2026-10-19)

`NavigationRules.backControl` takes the active route's control first, else one in no route at all (a header the screens share); a control in another route stays that route's. The iOS host passes `inAnyRoute` over its route views, and the web's `backControl` falls back the same way (an element whose nearest `navigationKey` ancestor is the root or none). `NavigationRulesTests.testTheBackControlIsResolvedAtUseByHTMLId` covers the shared, the other-route and the disabled case; `host/web/tests/app-bar.test.mjs` passes. The guide says the `navigate` handler on the root is the hook for a host pop with no control.
