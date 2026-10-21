# Eager image rails starve the raster queue and later images stay blank

**Status:** Closed
**Resolution:** fixed by b00f7bd30: failed decodes hold no queue place, refusals are retried, and off-screen pictures make room for on-screen ones
**Systems:** Apple host RasterLoader, host/raster policy
**Severity:** P1
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

A non-virtualized horizontal rail of ten image tiles fills the raster queue (`image queued: raster request admission`, `RasterLoader.swift:703`), and a gallery lower on the page never paints. A virtualized list avoided it.

## Mechanism

`RasterSession::request` refuses with `QueueFull` at 64 pending entries (`host/raster/src/policy.rs:503-563`; `PENDING_JOBS`, `types.rs:9`). `pending()` counts queued, waiting-on-budget, decoding, undelivered and failed entries (`policy.rs:153-170`). Eager views keep their decoded pixels pinned, so `available()` (`RasterLoader.swift:760-764`) stays low and later decodes sit in `WaitingBudget`, still pending; Swift's halving loop (`:646-657`) re-asks at half size until one pixel and then gives up, and nothing unpins, so nothing drains. A `QueueFull` is retried only on the next worker wake (`:702-704`, `:853-855`). A virtualized list recycles views and cancels their requests, which frees budget.

## Fix

Requests that wait on budget, and failed ones, must not hold admission against new requests indefinitely: plan the request at the laid-out size, keep budget-waiters out of the pending cap or evict cold pixels before refusing, and wake the queue when a request is refused so it is retried without a scroll. Add a loader test where pinned eager images exceed the budget and a later request still paints.

## Fixed

Policy (`host/raster/src/policy.rs`): a failed decode holds no place in the queue. `pending()` counts queued, decoding and ready-but-untaken entries; a failed entry is only its views' answer and a delivered one only cache, and neither drains by itself. A view whose decode failed has no place to hand its next source at a full queue (`request`'s `removable`). A cancelled queued entry already left at once (`cancel_request` removes it); `tests/ownership.rs` `delivered_and_failed_entries_hold_no_place_in_the_queue` holds all three.

Loader (`host/apple/Sources/ExactKit/RasterLoader.swift`):

- A pass that makes room in the core's queue (takes a picture, lets a request go) wakes another pass when any interest was refused `QueueFull`, so the refused ask again whether or not a further decode lands.
- A view off screen that waits on the budget, or whose full-size plan has no room beside what views hold, parks: it lets go of its request (and its place in the queue) and asks again once on screen, instead of decoding at a halved size that `resized` would never raise.
- A view on screen that waits on the budget, or has no room for its plan, releases the pictures held by views off screen (`release`: the farthest from the window first, until the shortfall is covered). Parking drops the view's lease, its image sublayer and its player and cancels its request, so the pixels go cold and the core evicts them for the decode on screen (`next_decode`). Only when nothing off screen holds pixels does the request halve in place, as before.
- A parked view is asked for again when it comes on screen: a scroll (macOS: `NSView.boundsDidChangeNotification` from any clip view, observed only while a view is parked; UIKit: a KVO watch on `contentOffset` of each scroll view above a parked view, likewise) or a layout (`resized`) checks only the parked views and wakes a pass when one is on screen.

Tests (`RasterLoaderTests.swift`): `testMoreImagesThanTheQueueHoldsAllPaintWithoutAScroll` (twelve pictures over a queue of three, all paint with no scroll, resize or reload) and `testOffScreenPicturesMakeRoomForAnOnScreenOne` (seventeen off-screen pictures fill the budget; one on screen paints at full size after they are let go of, and a parked one scrolled on screen paints again).
