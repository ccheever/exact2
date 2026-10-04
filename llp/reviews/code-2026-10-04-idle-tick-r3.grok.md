# Code review r3 (final): an idle timer tick only moves the clock (3f4e5af5e), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `3f4e5af5e`.
- **Method:** one brief (sha256 `356d9eb4adbe37d1e6a7fdb84c5fa88a14e0da50b9e8f61a8c87455071102ffe`), shared with astra. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's progress lines before them are omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (landed, no further round):** finding 1 is taken as suggested. `prepare()` runs synchronously in `applyGeometry` outside a batch, before the frame is written. The queued body is grouped lists, then segments, then controls (the order follows astra's finding 2). A test covers a carried custom row's replayed height.

---

## Round 2

Both findings are resolved as disposed.

The deferred-load drain waits until the wave is empty. In `GpuIOS.swift` and `GpuMac.swift`, each callback removes its key, loads, and calls `drainSurfaceWorkNow` only when `deferred.isEmpty`. An earlier failure no longer settles a module that is still loading. The synchronous `load("")` path never enters `deferred`.

The new tests cover what round 2 asked for, and no more. `GroupedListIOSTests.testAnOutOfBatchChangeReachesTheProjectionOnTheNextTurn` checks a list-owner frame and a switch `checked` value on the next turn, coalesced with the replay. `IdleTickTests` re-arms `clockDue`, invalidates the timer it replaced, and clears `clockDue` when the deadline is nil. The deferred GPU-artifact and sampler-seq host tests stay deferred, as the round-2 disposition said.

## Findings

**1. Should-fix — `prepare()` runs after replayed frames are written, so a carried row keeps its old box.** `host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:31`, `host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1005`, `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:502`.

`releaseBackground` replays deferred frames while custom rows are still inside their cells, then `requestProjectionSync` later calls `groupedLists.prepare()`. `restore()` writes `carried[id].frame`, the box saved at the last `carry`, over the frame `applyGeometry` just set, and clears `carried`. `sync(changed: [])` only `mount()`s, so `carry` records that old box again. Cell height and a native-button row’s `controls.sync` size both follow it. Nothing sends those frame ops again, so the row stays at the pre-sheet size. A native-button row is a custom row (LLP 1084). The new test moves the list owner, which is not carried, so it stays green.

That same `prepare()` is also the wrong tool for a trait refresh. It pulls every carried row out of its cell and back. `insertSubview` passes the row through `didMoveToWindow(nil)` (`NodeView.didMoveToWindow`), which resigns a first responder in the row, so a text field in a custom row loses the keyboard. A touch or deceleration on that row is cancelled with the view. `mount()` alone does not need this: switches refresh in place, and `assign` skips an unchanged collection frame.

`segments.sync()` outside a batch is safe. Item lists, selection, and hidden-tab adoption all no-op when unchanged; tint and bar frame update in place; `measure` only publishes an intrinsic size when the bar’s height actually changed, and that publish is asynchronous, after rows are mounted again.

The trait condition is the right set. `kind == "control"` covers accents and, via the control’s own registration, text size, weight, and scale. `kind == "list"` covers a grouped list whose switch sits on a hidden row. `accessibilityRole == "tablist"` covers a tablist whose kind is not `control` or `list`. An environment change reaches those owners as well as their children, so descendants do not need to match. System appearance still goes through `ExactView.reportScheme` to a full `apply`.

**Fix:** Do not call `groupedLists.prepare()` inside `requestProjectionSync`. On an out-of-batch `applyGeometry`, call `prepare()` once, synchronously, before writing `v.frame`, so the row is in its authored parent when the new box is assigned. The async body stays `segments.sync()`, `controls.sync()`, `groupedLists.sync(changed: [])`. `carry` then saves the new frames. Extend `testAnOutOfBatchChangeReachesTheProjectionOnTheNextTurn` with a carried custom row whose height changes and is still that height after the turn, and still inside its cell.

Verdict: LAND WITH FIXES
