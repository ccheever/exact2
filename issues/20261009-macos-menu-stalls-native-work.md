# macOS: while a context or button menu is open, main-queue work stalls, so native module calls stop until it closes

**Status:** Open
**Systems:** host/apple macOS, native modules, menus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/292

## Current scope

Keep native.later work running while real context/button NSMenus track, preserving deferred presentation/item dispatch. Reproduce the standalone proof in a full app first; painted agent menus cannot prove this.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On macOS, ExactKit's `MenusMac` opens a context menu (`context(_:at:)`) and a button's menu-shaped popover (the menu branch of `show`) with `NSMenu.popUp(positioning:at:in:)`, from inside a `DispatchQueue.main.async` block:

- `host/apple/Sources/ExactKit/Mac/MenusMac.swift:174-179`;
- `MenusMac.swift:208-222`.

`popUp` tracks the menu in a nested run loop until the menu closes. CoreFoundation does not drain the main dispatch queue in a run loop nested inside its own main-queue callout.

ExactKit hands every `native.later` call to an app's native module through that queue: `nativeLaterCallback` → `DispatchQueue.main.async`, `NativeModule.swift:315-318`. So while such a menu is open:

- no module call goes out;
- `.common`-mode timers keep the runner ticking, so calls pile up;
- the calls all arrive together when the menu closes.

An app whose data comes through its native module (a live server connection, a local database behind a module) freezes, then bursts, every time the person opens a context menu. On the web the page keeps running while a menu is open.

### Current and expected behavior

- **Current (macOS):** while a context or button menu is open, main-queue work does not run. Every `native.later` call, and anything else ExactKit sends through `DispatchQueue.main.async`, waits until the menu closes.
- **Expected:** main-queue work runs while a menu is open, as it does under any other run-loop tracking. For example, the menus could open from a common-modes run-loop turn (`CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, …)`) rather than from a main-queue block. The menu still opens after the click's app batch, which is why it is deferred today. This proposal is a hypothesis; another fix (taking module calls off the main queue) would satisfy the same acceptance.

### Reproduction and evidence

**Headless variant** (no window; the run loop is nested in `kCFRunLoopDefaultMode`, a common mode, as AppKit makes the event-tracking mode). `nested.swift`, built with `xcrun swiftc -O nested.swift -o nested`:

```swift
import Foundation
var inside = false, mainAsync = 0, commonTimer = 0
DispatchQueue.global().async { for _ in 0..<30 { usleep(100_000); DispatchQueue.main.async { if inside { mainAsync += 1 } } } }
let common = Timer(timeInterval: 0.1, repeats: true) { _ in if inside { commonTimer += 1 } }
RunLoop.main.add(common, forMode: .common)
let mode = RunLoop.Mode.default
let body: () -> Void = {
    inside = true
    let end = Date(timeIntervalSinceNow: 2)
    while Date() < end { _ = RunLoop.main.run(mode: mode, before: end) }
    inside = false
    print("\(CommandLine.arguments.contains("--perform") ? "from CFRunLoopPerformBlock" : "inside DispatchQueue.main.async"): main-queue blocks run=\(mainAsync) (of ~20 posted), .common timer fires=\(commonTimer)")
    exit(0)
}
RunLoop.main.add(Timer(timeInterval: 0.05, repeats: false) { _ in
    if CommandLine.arguments.contains("--perform") {
        CFRunLoopPerformBlock(CFRunLoopGetMain(), CFRunLoopMode.commonModes.rawValue, body); CFRunLoopWakeUp(CFRunLoopGetMain())
    } else { DispatchQueue.main.async(execute: body) }
}, forMode: .common)
RunLoop.main.run()
```

**On-screen variant:** the same comparison with a real `NSMenu.popUp` that closes itself after 2 s, in the shape MenusMac uses (row 4 below). It was measured on the same machine and OS on 2026-10-07, and was not re-run for this report.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Source: menus open inside a main-queue block | read `MenusMac.swift:174-179`, `:208-222`, `NativeModule.swift:315-318` | — | main `0365ad1a4` (unchanged on `e200397ec`) | `menu.popUp` runs inside `DispatchQueue.main.async`; `native.later` hops through `DispatchQueue.main.async` | menus open outside a main-queue callout, or module calls avoid the main queue | source |
| Headless, nested inside `DispatchQueue.main.async` | `./nested` | macOS 26.6.2, Apple Silicon | (AppKit/CF behaviour) | `main-queue blocks run=0 (of ~20 posted), .common timer fires=20` | blocks run | CLI output |
| Headless, nested from `CFRunLoopPerformBlock` | `./nested --perform` | same | same | `main-queue blocks run=19 (of ~20 posted), .common timer fires=20` | (the alternative) | CLI output |
| On-screen `NSMenu` (2 s), self-closing | an AppKit program that posts a main-queue block every 100 ms and opens an `NSMenu` with `popUp(positioning:at:in:)` from `DispatchQueue.main.async`, then from `CFRunLoopPerformBlock` | macOS 26.6.2 (measured 2026-10-07) | same | inside `DispatchQueue.main.async`: tracked ~2 s in `NSEventTrackingRunLoopMode`, main-queue blocks run during tracking = 0 (of ~20), `.common` timer fires = 5; from `CFRunLoopPerformBlock`: 21 and 22 | blocks run during tracking | the earlier measurement; not re-run here (the screen was in use by another session) |

### Acceptance criteria

- With a context menu or a button menu open on macOS, `native.later` calls a module receives keep arriving at their normal cadence, with no burst when the menu closes. A test can post main-queue blocks while a menu tracks and count them.
- The menu still opens after the click's app batch, and a chosen item still presses on the next turn, as MenusMac does today (LLP 1021 §5.1).
- Agent mode, where menus are painted popovers, is unchanged.

### Constraints and related work

- Workaround: an app's module that owns its own menus can open them from a common-modes run-loop turn. Menus ExactKit opens for Contract `contextPopover`/`popover role="menu"` have no workaround.
- Not tested: the stall measured through a full Exact app with a real right-click (that needs an attended or real-input session); iOS, whose menus are `UIMenu` and do not nest a run loop this way.
- Related: LLP 1021 (menus), #109 (watched topics and `native.later`).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:14Z

**Decision: Keep native work running while NSMenu tracks.**

Keep open for a correctness fix.

A menu should not suspend a live application's main-queue module work. The proposed common-mode run-loop scheduling is worth testing; the report's standalone proof is not yet a full-app reproduction.

Drive a real context and button menu while native.later calls arrive; preserve deferred presentation and next-turn item dispatch. Painted agent menus alone cannot validate this.
