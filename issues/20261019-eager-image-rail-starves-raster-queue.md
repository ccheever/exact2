# Eager image rails starve the raster queue and later images stay blank

**Status:** Open
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
