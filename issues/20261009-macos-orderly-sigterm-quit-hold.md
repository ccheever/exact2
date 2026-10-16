# macOS: a bounded quit hold for native module work, and SIGTERM as an orderly quit (rest of #105)

**Status:** Open
**Systems:** host/apple macOS, native modules, lifecycle
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/269

## Current scope

Route SIGTERM through orderly quit independently. Propose a module hold under one shared total five-second deadline; amend LLP 1069.010 Q4 before that extension. Check responsiveness, repeated quit and stuck child; SIGKILL remains immediate.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #105. #200 made a native module's `destroy()` run on every orderly AppKit quit (⌘Q, the app menu, an Apple Event quit, the last window closing), synchronously in `applicationWillTerminate`. Two parts of #105 remain:

1. **A bounded hold for module work at quit.** A module that must wait for something at quit (stop a child process with SIGTERM, then SIGKILL after a grace period; remove a network mapping; flush a socket) can only block the main thread inside `destroy()`. The host offers no asynchronous completion and no time budget. The host already holds a quit for up to 5 s, but only for storage that an answer started (LLP 1097 D10); a module cannot join that hold.
2. **SIGTERM.** `kill -TERM` (what `launchd`, a logout script or a supervisor sends) ends the process at once: no `beforeunload`, no storage hold, no `destroy()`.

Apps that embed a local server or helper process need both, so the child does not outlive the app with its port open.

### Current and expected behavior

Current (macOS, main `0365ad1a4`; `ExactMac/main.swift` and `ExactNativeModule.swift` are unchanged on `e200397ec`):
- `applicationShouldTerminate` returns `.terminateLater` only while `StorageHold` sees storage operations, else `.terminateNow` (`host/apple/Sources/ExactMac/main.swift:415-425`); `applicationWillTerminate` calls `destroy()` on every session (`:434-436`); "Nothing is held (LLP 1069.010 Q4)".
- `ExactModule` has `destroy()` and no quit method (`host/apple/modules/ExactNativeModule.swift`, `open func destroy() {}`).
- No SIGTERM handler exists in `host/apple/Sources`.
- Probe module whose `destroy()` waits 3 s (standing in for a child that takes 3 s to stop): an Apple Event quit ends the process after 3.42 s with `destroy` run on the main thread, which is blocked for those 3 s; `kill -TERM` ends it after 0.04 s and `destroy()` never runs.

Expected: a module can ask the host to hold a quit until its work completes or a fixed budget passes, without blocking the main thread, and SIGTERM is an orderly quit that runs the same path. The web has no blocking equivalent (`pagehide` and `beforeunload` cannot hold a page for work), so this is native-specific.

Proposals (hypotheses; the choice is the decision below):
- A: `open func willQuit(done: @escaping () -> Void)` on `ExactModule`, called from `applicationShouldTerminate`; the host returns `.terminateLater` and replies when every module calls `done` or after the budget (5 s, as LLP 1097 D10), sharing the storage hold. A `DispatchSource` signal source turns SIGTERM into `NSApp.terminate`.
- B: keep the synchronous `destroy()` and state its budget; add only the SIGTERM path.
- C: no hold; document that `destroy()` blocks the quit for as long as it runs, and that SIGTERM skips it.

### Reproduction and evidence

App: `bun scripts/exact.mjs new <dir>`; `app.json` adds `"modules": ["quit-probe"]`; `app.ts` answers no sources.

```contract
component RestX06
  view
    main testId="root" padding=24
      text "quit probe" testId="label"
      quit-probe testId="probe" width=40 height=40
```

`modules/apple/QuitProbe.swift` (core):

```swift
final class QuitProbe: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["quit-probe": ExactNativeFactory { _, events in ProbeBox(events: events) }]
    }
    override func destroy() {
        note("module destroy begin (main thread \(Thread.isMainThread))")
        Thread.sleep(forTimeInterval: 3)   // a child process taking 3 s to stop
        note("module destroy end")
    }
}
let exactModule: ExactModule.Type = QuitProbe.self
```

`note` appends a timestamp line to the file `$QUIT_LOG`. A script opens the app with `open({host: 'macos', env: {QUIT_LOG}})` from `scripts/agent.mjs`, takes its pid, ends it one way, and polls `kill -0` every 20 ms.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Apple Event quit | `bun exact.mjs mac`; app under the agent; `'aevt'/'quit'` sent to its pid (`AESendMessage`) | macOS 26.6.2, Apple Silicon | `0365ad1a4` | gone after 3.42 s; log: `module destroy begin (main thread true)`, `module destroy end` 3.0 s later | the quit waits for the module's work, up to a stated budget, without blocking the main thread | `$QUIT_LOG` lines quoted |
| SIGTERM | the same; `kill -TERM <pid>` | macOS 26.6.2 | `0365ad1a4` | gone after 0.04 s; log has only `module init` | the same orderly path as an Apple Event quit | `$QUIT_LOG` |

### Acceptance criteria

- For ⌘Q, the app menu, an Apple Event quit, the last window closing and SIGTERM: a module's quit work (a fake child that traps SIGTERM and exits after 3 s) completes before the process exits, and the hook runs once even when quit is requested twice.
- Work that does not finish is cut at the budget, and the app exits by it.
- An app without the hook quits at once, and the LLP 1097 D10 storage hold still works.
- The main thread stays responsive while the hold waits (if the asynchronous form is chosen).

### Constraints and related work

- Policy: LLP 1069.010 Q4 rules "No `.terminateLater`" (`llp/1069.010-the-mac-as-a-document-platform.rfc.md:16`, restated at `:682`); LLP 1097 D10 later admitted a 5 s hold for storage (`llp/1097-storage-that-finishes-after-the-answer.rfc.md:657`).
- Workaround: block in `destroy()` (the window is frozen while it waits; no budget), and reap leftover children at the next launch from a pid file. Neither helps on SIGTERM.
- Not tested: the Dock's Quit, logout and shutdown, Linux's orderly exit.
- Related: #105 (closed by #200, which runs `destroy()` synchronously at quit).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:18:34Z

## Decision needed

**Blocking rule.** LLP 1069.010 Q4, Charlie's ruling: "every write is already committed. No `.terminateLater`" (`llp/1069.010-the-mac-as-a-document-platform.rfc.md:16`, restated at `:682`); the host cites it where the quit is decided (`host/apple/Sources/ExactMac/main.swift:427-433`: "Nothing is held (LLP 1069.010 Q4)"). The one exception is LLP 1097 D10's 5 s hold for storage an answer started (`llp/1097-storage-that-finishes-after-the-answer.rfc.md:657`). A hold for module work needs Q4 relaxed the same way.

**Options.**
- **A, a bounded module hold.** `ExactModule.willQuit(done:)`, awaited by the same `StorageHold` with the same 5 s bound; SIGTERM becomes an orderly `NSApp.terminate` through a signal source.
- **B, SIGTERM only.** Keep Q4: no hold; the synchronous `destroy()` stays the only quit work, and SIGTERM is routed to the orderly quit so `destroy()` runs there too. State `destroy()`'s budget in the docs.
- **C, document today's behavior.** `destroy()` blocks the quit for as long as it runs; SIGTERM skips it.

**Recommendation.** A. D10 already admits a bounded `.terminateLater`, so a module joining that one hold adds no new mechanism, keeps the 5 s ceiling, and frees the main thread a blocking `destroy()` freezes today. If Q4 should stand, take B: SIGTERM as an orderly quit is independent of the hold and needs no ruling.

**Cost.** A: one `ExactModule` method (a no-op on hosts without a quit), the delegate joining `StorageHold`, a signal source, an AppKit test with a fake child process; about a day. B: the signal source and its test; a few hours.

### ccheever — 2026-10-08T08:07:41Z

**Decision: Route SIGTERM through orderly quit; propose one bounded module hold.**

Keep open with the bounded scope below.

Orderly SIGTERM can be fixed independently. The asynchronous module hold is a justified extension of the existing storage hold, but LLP 1069.010 Q4 requires a ruling.

Prefer one shared total five-second deadline, not five seconds per module, with once-only completion and responsive main thread. Test repeated quit and an unresponsive child. Do not claim SIGKILL can be made orderly.

## 2026-10-13 status

The SIGTERM half is on main (#313, a3d61c023, with its tests): a `DispatchSource` signal source routes a
supervisor's SIGTERM through the same cancellable orderly quit as the menu
(`host/apple/Sources/ExactMac/main.swift`, "A supervisor's SIGTERM follows
the same cancellable quit"), so `destroy()` runs there too. What remains is
option A, the bounded module hold (`ExactModule.willQuit(done:)` joining the
one storage hold, one shared 5 s deadline, once-only completion, responsive
main thread, tests for repeated quit and an unresponsive child), which needs
LLP 1069.010 Q4 relaxed.

## 2026-10-15 status: option A built, awaiting Charlie's acceptance of the Q4 amendment

Built on `macos-module-quit-hold`. `ExactModule.willQuit(done:)`
(`host/apple/modules/ExactNativeModule.swift`; the default calls `done` at
once, and a `done` let go uncalled counts as called) is entry 208 of the
module table (size 216; an older module is not asked). `applicationShouldTerminate`
(`ExactMac/main.swift`, and the sample host's) asks each session's module once,
after every `beforeunload` has let the quit go, and holds the quit with the
storage of LLP 1097 D10 in one `StorageHold`: one five-second deadline in all.
A repeated quit joins that hold. The last window closing, or any window closing
while the quit is held, now leaves its session for the quit instead of
destroying it first, which had skipped the hold.
iOS and tvOS never call the hook. LLP 1069.010 Q4 carries the proposed amendment.

Verified on macOS 27.0.1 (Apple Silicon), a generated app whose module's child
traps SIGTERM and exits 3 s later (or ignores it), driven through
`scripts/agent.mjs`:

| Scenario | Exit | Hook / destroy | Evidence |
|---|---|---|---|
| SIGTERM ×3 and an Apple Event quit, 0.3 s apart | code 0 after 3.06 s | 1 / 1 | child exited before `destroy`; agent `state` answered in 5 ms during the hold; 60 main-thread ticks at 50 ms |
| Apple Event quit | 0 after 3.08 s | 1 / 1 | child exited first |
| Last window closing (`close()`) | 0 after 3.13 s | 1 / 1 | child exited first |
| SIGTERM, then the window closed during the hold | 0 after 3.04 s | 1 / 1 | child exited first |
| `beforeunload` cancels, then allows | 0 after 3.10 s | 1 / 1 | not asked while cancelled |
| Child ignores SIGTERM | 0 after 5.07 s | 1 / 1 | child still running after exit; 100 ticks |
| No hook; hook drops `done` | 0 after 12 ms; 9 ms | 0 / 1; 1 / 1 | — |
| Storage in flight and a stuck child | 0 after 6.01 s | 1 / 1 | one deadline, not two |
| Storage in flight, no hook (D10) | 0 after 6.02 s | 0 / 1 | the D10 hold still holds |

The storage rows include the JS module's one-second teardown drain after the
five-second hold, as #313 measured. `ModuleQuitHoldTests` (`StorageHoldTests.swift`)
covers the host side: once-only asking, `done` from another thread, the main
run loop running during the hold, the bound, the session ending, and a module
without the entry.

Remaining: Charlie's acceptance of the LLP 1069.010 Q4 amendment. ⌘Q and the
app menu were not driven literally (they reach the same `NSApp.terminate` and
delegate as the Apple Event quit); neither were the Dock's Quit, logout or
shutdown.
