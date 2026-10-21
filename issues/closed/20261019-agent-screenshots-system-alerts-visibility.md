# Agent driver: screenshots hide system alerts, and page-visibility facts ignore real backgrounding

**Status:** Closed
**Resolution:** fixed by bbd846b41: screenshot display, a real background under the agent, and the touch runner names its app
**Systems:** scripts/agent.mjs, host/apple/devices.mjs, ExactKit PageFacts, touch runner
**Severity:** P3
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptoms

1. `screenshot out.png` while a system permission alert is up shows the app's own drawing and no alert.
2. `exactPage().visibilityState` stays `visible` under the agent while the touch runner has sent the app home, so a lifecycle test sees nothing.
3. The touch runner was reported to keep an earlier app's state after switching bundles.

## Causes

1. A plain screenshot is the app's `drawHierarchy` (`AgentIOS.swift:1227`); a window capture reads the display (`devices.mjs` `withDisplay`, `:49-115`) only when the app is active, so an alert, which makes it inactive, is never read (`devices.test.mjs:226`).
2. `PageFacts` substitutes every fact under the agent (`PageFacts.swift:29-35, 85-89, 129-130`; LLP 1069.000 D6), so `hidden` is the drive's `prefer` value and `AppBackground.now` is ignored.
3. By the code the kept runner retargets on every hello (`touches.swift:55-71`); the shared `default` store and container are keyed by app id (`agent-launch.mjs:183-198`), so two checkouts with one bundle id share state. Not reproduced.

## Fix

Read the display for a window capture when the app is inactive but still in the foreground (an alert over it), saying so in `pixels`. Let a real background state show through substitution: `prefer` can hide the page, not make a backgrounded one visible. Say in the touch runner's reply which bundle it drives.

## Fixed (2026-10-19)

1. `screenshot <png> display` reads the simulator's display as it is (`displayAsIs`), whatever is over the app, with no check against the app's drawing; `pixels` says `display`.
2. Under the agent `PageFacts.hidden` is the drive's value or the real background state: `prefer visibility-state hidden` still hides, and an app the touch runner sent home reads hidden.
3. The touch runner's hello names the app it drives, and the driver retires a kept runner that answers for another bundle.
