# Widget images: app:/data resolves to a different folder than the app writes

**Status:** Closed
**Resolution:** fixed by bbd846b41: the widget resolves app:/ images under the app's own layout in the group container
**Systems:** widgets (ExactSnapshot rasters), Apple host storage roots (picker.rs, store.rs)
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

A widget showing `app:/data/photo.jpg`, written by the app, is blank.

## Cause

Two formulas for one path. The app (and the extension's own TypeScript refresh) resolve `app:/data` through `picker::app_dirs` (`host/apple/src/picker.rs:42-51`): `<group or home>/Library/Application Support/exact/<appId>/data`, with caches under `Library/Caches/exact/<appId>/{cache,temporary}`. The widget's image render (`SnapshotRasters.resolve`, `host/apple/Sources/ExactSnapshot/SnapshotLeaves.swift:390-398`) uses `<group>/data`, `<group>/cache`, `<group>/temporary`. `testImagePathsResolveUnderTheirRoots` (`SnapshotDriverTests.swift:507`) pins the wrong layout.

## Fix

Resolve the widget's `app:/` image roots with the app's layout under the group container, keyed by `ExactAppIdentifier`; update the test.

## Fixed (2026-10-19)

`SnapshotRasters.resolve` takes the app's identifier and resolves `app:/data`, `app:/cache` and `app:/tmp` under `<group>/Library/Application Support/exact/<id>/data` and `<group>/Library/Caches/exact/<id>/{cache,temporary}` (`SnapshotRasters.appRoots`), the layout `picker::app_dirs` writes; `SnapshotEnvironment.appID` carries it, and the widget and activity extensions pass `WidgetContainer.appID`. `SnapshotDriverTests.testImagePathsResolveUnderTheirRoots` reads a `data` file at the app's path and refuses a path that climbs out.
