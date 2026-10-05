# Native menu run-loop diagnosis, 2026-10-06

Production `T3ContextMenu.swift` is unchanged from implementation `8498fdc8a` / verification `7444d5999` (SHA-256 in report). No application or framework repair was justified by this reproduction. Exact `implement` foundations/state/testing guidance applied. This is a repair of the verification procedure.

## Reproduction

From repository root on an unlocked macOS desktop:

```sh
python3 examples/macos/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/tab-menu/20261006-menu-run-loop-repair/run-native-menu.py
```

The helper compiles the production native menu with a small AppKit application. Each process opens one actual NSMenu at one second and posts real CG key events at two seconds. `select` posts Down/Return; `escape` posts Escape. The callback validates the selected ID or null and schedules another main-queue callback, mirroring subsequent native work scheduling. Both callbacks must complete within 1.5 seconds of input. All processes have an eight-second termination bound. The helper does not read or mutate the clipboard or app data.

`settle` uses the same selected-item sequence and additionally schedules a common-mode `CFRunLoopPerformBlock`, as `host/apple/Sources/ExactKit/Agent.swift` does for incoming agent commands. That block waits for completion with nested default-mode `RunLoop.main.run`, reproducing `clock settle`'s wait-for-replies mechanism with a shorter three-second bound. The test requires this wait to reach its deadline with completion still false, followed by the correct native answer after unwinding.

## Observed result

All three processes exit 0. `report.json` fingerprints the unchanged production source and fixture; per-mode logs retain timing.

| Case | Input at | Reply at | Main-queue continuation at |
| --- | --- | --- | --- |
| Normal selection | 2.0 s | 2.471 s | 2.473 s |
| Normal Escape | 2.0 s | 2.358 s | 2.359 s |
| Reentrant settle | 2.0 s | 5.457 s | 5.461 s |

Reentrant settle enters at 2.118 s, exits at 5.135 s with `complete=false`; only afterward does `NSMenu.popUp` return and the application send its reply. The menu choice itself is correct in every selection case. This is the causal mechanism that can produce the earlier pending-menu observation: the agent wait is entered inside the native popup's tracking loop and prevents the popup stack from returning until the wait ends.

## Correction to earlier interpretation

The previous [attempt](../../parallel/20261006-live-verification/attempt.md) and its pending request logs remain preserved. They establish a 20-second bound exceeded under that procedure and eventual correct clipboard bytes. They do not establish an application menu defect. This reproduction demonstrates the same delay with unchanged menu code solely by adding the reentrant agent-style wait. The prior menu responsiveness verdict should be treated as an invalid timing fixture pending corrected full-app input verification.

For corrected full-app verification, use platform timing, allow the native selection event loop to unwind before inspection, and do not call `clock settle` while NSMenu is still tracking. A short real event-loop interval after physical selection permits the popup to return; full-app state and clipboard assertions must then establish actual mutation completion. This standalone fixture proves native selection/cancellation and the delay's cause; it does not substitute for mounted tab rename, clipboard, close, persistence or device acceptance.
