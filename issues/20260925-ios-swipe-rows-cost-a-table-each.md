# iOS swipe rows each carry a mounted UITableView, which halves a fast list's frame rate

**Status:** Fixed in 80223cee (option 1, lazy projection). A row's table and cell are made when a touch lands on it (its hit test) and dropped once the touch has ended, the row is closed and UIKit's animation has settled; they stay while VoiceOver or Switch Control runs. The cell's background is clear and it follows the row's height. Measured on an iPhone 17 Pro simulator (60 Hz, loaded Mac) with the listbench copy, its remount workaround and corner caps removed. Fling fps went from 20.4/20.2 (10.2/10.1 in a first run) to 60/60 at ±24,000 pt/s and from 44.5/46.5 to 60/60 at ±12,000, with no hitch at any speed after. Under the list at the top there were 259 → 187 views and 282 → 210 layers (8 → 0 tables, 9 views and 9 layers per swipe row). Swipe open, snap-back, close, tapping Delete, full swipe, a grown row's projected height and the card's corners were checked with simulator touches (idb). The iPhone at 120 Hz is still to measure.
**Systems:** Apple host (iOS `SwipeActionsHost`, LLP 1008 §9), Scrolling (collections, LLP 1010 §6.5)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** LLP 1050.000 (the fill-policy benchmark that measured it); `~/bench/listbench/` on Charlie's Mac (outside the repo; `RESULTS-iphone-parity-fix*.txt`)

## What happens

A row authored with `swipeContent` gets native swipe actions from `SwipeActionsHost` (`host/apple/Sources/ExactKit/IOS/SwipeActionsIOS.swift`). Each such row eagerly builds and mounts its own `UITableView` and `UITableViewCell`: in `Row.init` and `Row.mount()`, the row's content is moved into that cell and the authored horizontal `scroll` is hidden.

Every batch, `prepare()` restores every live row's hierarchy and `sync()` mounts every one again. So a list pays one table view per mounted row, plus a walk of all of them per batch, even though no swipe is in progress.

It was measured on an iPhone 17 Pro Max (120 Hz). The workload was the Expo PR 49975 demo, 10,000 rows in `list virtualized=true` with rows matched to Expo's, built at `abd6ca62`. The probe scrolled programmatically, 3 alternating rounds.

| 24,000 pt/s | with `swipeContent` | without (2 runs) | Expo `List` (has swipe) |
|---|---|---|---|
| fps | 60 | 120 | 90–107 |
| sampled frames with a gap | 93% | 31% | 3–18% |

Up to 12,000 pt/s all three hold about 120 fps.

Two correctness problems showed up in the same port:
- The cell is sized once and does not follow the row when the row's height changes (the app remounted the row when its content grew).
- The cell paints an opaque square background over the row, which hid a rounded container's corners (the app painted corner caps).

## Fix

Pay for the native cell only when a swipe can start. Options, in order of preference:
1. Mount the table and cell lazily: on the first horizontal pan over the row, or when accessibility or the agent asks for the row's actions. Tear them down once the row is closed and settled. Keep the gesture native.
2. Keep one mounted cell per row but make it cheap: no per-batch restore/mount walk over unchanged rows, and no work for rows outside the viewport.

Either way:
- the cell follows the row's height;
- the cell's background is clear, so the row's own paint shows (rounded corners included);
- the action buttons, accessibility (`accessibilityActivate`) and the agent's `actionView` keep working.

## Done when

- The listbench parity app with `swipeContent` scrolls at 120 fps at 24,000 pt/s on the iPhone, as it does without.
- A swipe still reveals and runs the native trailing Delete, including full swipe.
- A row that grows while its swipe is closed shows the new height.
- A row inside a rounded card keeps the card's corners.
- iOS XCTests and `smoke.mjs ios` pass.
