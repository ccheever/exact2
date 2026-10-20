# Widget refresh: no native hook for storage changes, stale layout after an update, and --scale 1 only

**Status:** Closed
**Resolution:** fixed by bbd846b41: context.wrote reloads widgets, a generation names its binary, and the stand-in renders at the asked scale
**Systems:** widgets (WidgetContainer, Timeline), native modules (ExactModuleContext), scripts/agent-widget.mjs
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptoms

1. A native module's write to `context.data` does not refresh the widget; there is no API for it.
2. The widget gallery and placed widgets show the old layout after an app update, until the app is opened.
3. `agent widget … --scale 2` is refused: "snapshots use one pixel per point".

## Causes

1. Only `Host::persist` (`host/apple/src/host.rs:799-817`) calls `store::widget_changed`, which bumps the shared generation and posts the Darwin notification the app's `WidgetStoreObserver` turns into `WidgetCenter.reloadTimelines` (`WidgetContainer.swift:128-148, 202-252`). `ExactModuleContext` (`host/apple/modules/ExactNativeModule.swift:79-157`) offers `changed(topic)`, which re-asks TypeScript answers and reaches no widget.
2. Installed widgets pin the shared container's published generation over the embedded plan (`Timeline.swift:135`; `WidgetContainer.pin`, `:55-68`), checking only the compatibility cohort, which hashes binary inputs (`scripts/app.mjs:785`). A layout-only update keeps the cohort, so the old generation stays pinned until the new app launches and publishes.
3. The agent's widget read is an in-process stand-in at scale 1 (`WidgetGallery.swift:72, 185, 534`; `ActivityAgent.swift:87, 93`), and `agent-widget.mjs:58` refuses the flag.

## Fix

A module context method that marks storage changed (the same path as a TypeScript write); a published generation records the embedded plan's digest and the extension uses the embedded copy when its own differs; the stand-in renders at the asked scale.

## Fixed (2026-10-19)

1. `ExactModuleContext.wrote(path)` (default `app:/data`) tells the host a module wrote the app's files: `Bridge::app_changed` marks storage changed for an `app:/` topic (`DataSource::note_storage_changed`), so the next commit posts the widget-store change as a TypeScript write does, and answers watching the path are asked again.
2. A published generation records the publishing binary's `ExactWidgetGeneration` (`embedded`); `WidgetContainer.pin` refuses a generation another binary published, so after a binary update the extension renders its embedded plan until the new app launches and publishes. `WidgetContainerTests.testPublicationSkipsUnchangedAssetsAndKeepsPinnedGenerations`.
3. `agent … widget <entry> --scale 2` (1, 2 or 3) renders the stand-in at that scale (`WidgetVariant.scale`, `SessionFacts.displayScale`, the renderer's scale), for activities too.
