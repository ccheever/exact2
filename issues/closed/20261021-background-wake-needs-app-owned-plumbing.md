# Background refresh needs an app-owned dyld initializer, plist postprocessing and a second JS runtime

**Status:** Closed
**Resolution:** LLP 1133.017 (accepted 2026-10-23; stages 0 to 2 built 2026-10-24) replaced LLP 1114.001 stage 2 and ships all three asks: the host registers BGTaskScheduler at launch (ExactKit Wakes.swift), host/apple/plist.mjs derives the permitted identifiers from the grants, and a wake boots a headless session that runs the app's data module. Brooks uses @exact/background (registerPeriodicSync and the root periodicsync event; docs/recipes/background-work.md). iOS expiry reaching the wake is a QUEUE.md line; the before-first-unlock check is owed on the Background work line.
**Systems:** app manifest (`host.ios`), Apple host launch, data sources (headless invocation), LLP 1114.001 stage 2
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks refreshes its Health mileage in the background. To do it on exact2 the port wrote a Rust dyld initializer that loads its Swift module early enough to register with `BGTaskScheduler`, a post-build step that adds `BGTaskSchedulerPermittedIdentifiers` to the Info.plist, and a separate Bun bundle of its TypeScript aggregation run under JavaScriptCore without a screen. The UIKit and SwiftUI ports registered the task in `application(_:didFinishLaunchingWithOptions:)`.

## Cause

`host.ios.backgroundModes` writes `fetch` but no manifest key writes the permitted identifiers. The app's native module is created at first layout, after the point where `BGTaskScheduler.register` must run. No headless path runs the app's data module without a session. LLP 1114.001 stage 2 designs all three and is Draft r2, unapproved.

## Fix

Rule on and build LLP 1114.001 stage 2: early registration owned by the host, permitted identifiers from the manifest, and a headless data invocation with expiry and completion. The lane's repro is `friction/background-refresh.md` and `friction/background-metadata/` in the Brooks exact2 app (`~/bench/brooks-archive/exact` on Charlie's laptop).
