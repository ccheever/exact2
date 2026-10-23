# Startup holds the main thread on the data module's first evaluation

**Status:** Closed
**Resolution:** Fixed: the main thread no longer waits on the module's evaluation (LLP 1027.007 P1); evaluating earlier (P2/P3) declined 2026-10-23.
**Systems:** Apple host session activation (`Session.firstDrawn`), Owner thread, native views (`activateAfterCommit`), TypeScript data module load
**Severity:** P1
**Author:** Claude (Opus 5.5), from the Brooks four-way port's device measurements
**Date:** 2026-10-21

## Symptom

On an iPhone 17 Pro Max (Release build, iOS 27), Brooks' Home shows its header 0.70 s after launch, its rail photos at 0.64 s and its hero video at 1.49 s. UIKit's port of the same screen shows them at 0.65, 0.55 and 0.32 s. Between the first frame and the media, the photos are decoded but not on screen, the tab bar is missing, and a touch would wait.

## Cause

A Time Profiler trace of a cold launch shows the main thread draw the first frame, then run nothing for about 325 ms while the `exact.owner` thread runs `Runtime.dataReady → exact_data_ready → Module::load → exact_js_load` (Hermes `evaluateJavaScript`) for 314 ms. In `Session.firstDrawn` the main thread calls `natives.prepareAppModule()` (whose `bindExtensions` is an `Owner.sync`) and then `runtime.dataReady()`, both synchronous calls to the owner, so the main thread waits for the module's whole evaluation. Native views are activated only after that batch commits (`natives.activateAfterCommit`), so the app's video view is created about 380 ms after the module instance, though it reads no data.

Brooks' module builds its 2.2 MB catalog at load. Bun evaluates the same module in 60 ms; Hermes takes 314 ms.

With `"typescript": {"placement": "worker"}` the module evaluates on its own thread, the main thread does not wait, and the rail photos arrive at 0.55 s, as in UIKit. The default placement is `main`, so every app with a slow module load blocks its main thread this way.

## Fix

Do not hold the main thread or native-view activation on the module's first evaluation. Start that evaluation as soon as the session exists, alongside the first layout, rather than after first pixel. Let first-pixel activation take the existing pending path (`activate_data` answering `pending`, then the session's wake) when the module is still loading. Activate native views at first draw. In development builds, report a module whose evaluation exceeds a frame budget, with its time.

Evidence: `~/bench/brooks-device/diag/tp-exact.trace` and `tp-worker.trace` (Time Profiler), and `~/bench/brooks-device/launch/results-2.txt`, `results-exact-worker.txt`, `results-exact-poster.txt` (filmed launches), on Charlie's laptop.

## Partly fixed (2026-10-21)

`Session.firstDrawn` (now in `SessionActivation.swift`) posts the source's activation to the owner (`Host::prepare_data`, `exact_prepare_data`) and commits it when that is done, so the main thread does not wait out the evaluation itself; native views attach after the first pixel's turn instead of after the data commits (LLP 1024 D3, amended). A module whose top level takes longer than 50 ms is journalled. On a cold launch the main thread still waits: iOS reports the app active at the first frame, and `tellPage` is a synchronous runtime call queued behind the evaluation. Holding page facts until the data is ready changed what the page is told (`WindowMannersMacTests`' `pagehide` with `persisted`), so it was not kept. Filmed on the iPhone 17 Pro Max with Brooks: hero 0.62 → 0.59 s, rail photos 0.63 → 0.59 s; worker placement gives 0.39 and 0.55 s. Evaluating the module during launch, off the owner, is the remaining fix (LLP 1027.007, draft).

## Main thread freed (2026-10-21)

LLP 1027.007 P1: while the data source activates, the page-facts and preferences notices iOS posts at first pixel send their runtime calls to the owner without the main thread waiting, and their batches land in the owner's order. Brooks on the iPhone 17 Pro Max: rail photos 0.55 s, as UIKit; hero 0.53 s. The main thread is idle during the evaluation. What remains is the evaluation itself (314 ms for Brooks, 220 ms of it its own eager `completeTheLook` for every product; LLP 1027.007 §6a), which content that needs data waits for; evaluating before first pixel is LLP 1027.007 P2 and P3, not ruled on.

## Remainder declined (2026-10-23)

The main thread no longer waits on the module's evaluation, native views attach at first draw, and a top level over 50 ms is journalled; this ticket's symptom is gone. Content that needs data still waits for the evaluation, and runtime input can still queue behind it. Starting the evaluation earlier or off the owner (LLP 1027.007 P2 and P3) was declined: the remedy is the app's own top level (LLP 1027.007 §6a; `docs/agent-pitfalls.md`, "Shape data in the source").
