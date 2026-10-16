# A host command that relaunches the app's process (rest of #122)

**Status:** Closed
**Resolution:** Added relaunch(): macOS uses cancellable orderly quit and waits for process exit before restarting; RelaunchTests verifies a queued relaunch joins a quit's storage hold, and relaunch.test.mjs proves changed PID, clean state, preserved writes and arguments with the dev menu off in development and release Swift builds.
**Systems:** host/apple macOS, lifecycle, agent
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/271

## Current scope

Select explicit process relaunch after #269 orderly quit/durable teardown, independent of dev menu. Define host/agent refusal or following; check changed PID/preserved writes. Keep reload/update lifecycle distinct.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #122. #170 moved `reload()`'s timing line to stderr so it no longer breaks the agent driver. The request itself is still open: an app cannot relaunch its own process.

Desktop apps relaunch after a setting that changes how the whole app starts: how an embedded server is exposed (local only, the network, a private HTTPS name), which environment is primary, an update applied. A relaunch makes the window, the client state, native modules and their child processes start clean together, as Electron's `app.relaunch()` plus `app.exit()` does. Today the closest command, `reload()`, reboots the Contract session inside the same process, keeps every native module and child process, and is refused in a build without the dev menu.

### Current and expected behavior

Current (macOS, main `0365ad1a4`; the files involved are unchanged on `e200397ec`):
- `relaunch()` (or any such name) is refused at compile time: `type-unknown-command`, "`relaunch` is not a host command; the hosts answer blur, copyText, …, reload, …".
- `reload()` on macOS runs `DevMenu.reload()`, which calls `session.boot(…)` in the same process (`host/apple/Sources/ExactKit/Mac/DevMenuMac.swift:257-270`): state starts over and the pid stays.
- With `EXACT_DEV_MENU=0`, `reload()` does nothing but write `exact: reload: no dev menu in this build` to stderr (`host/apple/Sources/ExactKit/Session.swift:1061-1066`).

Expected: a documented host command that lets pending storage writes land (as a quit does, LLP 1097 D10), closes the windows, ends the process and starts the same bundle again with the same arguments, in any build. There is no web equivalent of a process relaunch; on the web it would be the page's reload.

Proposal (hypothesis; the name and per-host behavior are the decision below): `relaunch()`; macOS relaunches the bundle (for example `NSWorkspace.openApplication(at:configuration:)` with `createsNewApplicationInstance`, after the quit path, from a short-lived helper or `open -n`); iOS refuses it with a logged reason; the web reloads the page; Linux re-execs where it can. Under the agent the driver follows the new process or the command is refused with a clear message.

### Reproduction and evidence

App: `bun scripts/exact.mjs new <dir>`; `app.ts` answers no sources.

```contract
component RestX45
  state presses = 0
  action bump
    presses = presses + 1
  action restart
    reload()
  view
    main testId="root" padding=16
      text `presses ${presses}` testId="presses"
      button press=bump testId="bump"
        text "Bump"
      button press=restart testId="restart"
        text "Restart"
```

A script opens the app with `open({host: 'macos'})` from `scripts/agent.mjs` (its `onProcess` reports each process it starts), taps `bump` twice, then `restart`, and reads `presses` and the pid.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| A relaunch command | a `relaunch()` call in an action; `bun exact.mjs contract build <file> --json` | — | `0365ad1a4` | `type-unknown-command`: `relaunch` is not a host command | a host command that relaunches the process | compiler output quoted |
| `reload()`, dev menu on | `bun exact.mjs mac`; the script above | macOS 26.6.2, Apple Silicon | `0365ad1a4` | `presses 2` → `presses 0`; pid 86627 before and after; one process started | (with the new command) a new pid, one app process, clean state | script output |
| `reload()`, dev menu off | the same with `EXACT_DEV_MENU=0` | macOS 26.6.2 | `0365ad1a4` | `presses 2` → `presses 2`; pid unchanged; journal `command reload()` only | the new command works without the dev menu | script output |

### Acceptance criteria

- On macOS, an app that calls the command: the old process exits, one new process of the same bundle starts with the same arguments, and its first frame shows the clean start; a storage write made just before the call is present after it.
- It works with `EXACT_DEV_MENU=0` and in a release build.
- iOS refuses it with a logged reason; the web reloads the page; the agent driver either follows the new process or reports a clear refusal.

### Constraints and related work

- Workaround: restart the app's own child processes from a native module and reset every piece of client state by hand, keeping the window; or `reload()`, which keeps the process and is unavailable without the dev menu.
- Not tested: iOS, Linux, a release build.
- Related: #122 (closed by #170, which only moves `reload()`'s log to stderr), LLP 1097 D10 (the quit's storage hold), the rest of #105 (a bounded quit hold for module work).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:18:37Z

## Decision needed

**Blocking rule.** None in `rules/DEFERRED.md`; the API must be chosen first. There is no web equivalent of a process relaunch, so the name and per-host behavior are new. Today `reload()` is the page's reload on the web and the dev menu's in-process reboot on Apple, refused without the dev menu (`host/apple/Sources/ExactKit/Session.swift:1061-1066`, `Mac/DevMenuMac.swift:257-270`); LLP 1097 D10 gives `reload()`'s teardown a 1 s bound (`llp/1097-storage-that-finishes-after-the-answer.rfc.md:655`).

**Options.**
- **A, a new `relaunch()` command.** Native: the orderly quit (storage hold, `destroy()`), then the same bundle starts again with the same arguments. iOS refuses it with a logged reason; the web reloads the page; the agent driver follows the new process or refuses the command under the agent.
- **B, `reload()` relaunches in shipped builds.** Release builds relaunch the process; development builds keep the fast in-process reboot. One name, two behaviors.
- **C, no relaunch.** Document the in-app restart: modules restart their own children, the app resets its state.

**Recommendation.** A. `reload()` keeps one meaning (the web's `location.reload()`, the dev loop's reboot), and the relaunch is a separate, explicit command that works in any build.

**Cost.** Compiler roster entry and docs, the runner command, the macOS host (relaunch through `NSWorkspace.openApplication` with `createsNewApplicationInstance` after the quit decision), the web alias, the driver's handling; one to two days, macOS first.

### ccheever — 2026-10-08T08:07:43Z

**Decision: Choose an explicit process relaunch command, after orderly quit is complete.**

Keep open with the bounded scope below.

A process restart should not depend on the development menu or overload reload differently in release builds.

Depends on #269 and durable-write teardown. Define each host's behavior and clear agent refusal or process-following; test a changed PID and preserved writes. Keep the default app update path on its existing lifecycle.

## Verification (2026-10-13)

- Review regression: `testRelaunchFromBeforeUnloadJoinsTheQuitStorageHold`
  failed when a queued quit ran after the original quit took its hold. The
  callback now rechecks the hold; the test models pending storage and verifies
  that only the original quit completes after it drains. The companion test
  checks that relaunch still requests quit when there is no hold.
- `calls::relaunch_is_a_host_command_without_arguments` failed with
  `type-unknown-command` before the change and passes after it.
- `host/apple/relaunch.test.mjs`, with `EXACT_RELAUNCH_APP` naming the built
  Caltrain executable, passes against both `build.mjs --bundle` and
  `build.mjs --bundle --distribution` (release Swift, development-trust Rust).
  It checks a cancelled quit, a subsequent restart requested by `beforeunload`
  during an ordinary SIGTERM quit, changed PID,
  one replacement, the arguments, persisted state written just before restart,
  and reset transient state, with `EXACT_DEV_MENU=0`.
- `RelaunchTests` and `StorageHoldTests` pass on macOS. `RelaunchTests` also
  passes under `EXACT_AGENT=1` and on the iOS 27 simulator. Linux's
  `process_relaunch_is_refused_in_the_journal` passes: this macOS-first
  implementation explicitly refuses Linux process restart.
- `bun test host/web-js/commands.test.mjs` passes in Chrome: a new document
  reads storage committed before the command. Both web executors reload.
- Tests and Clippy pass for `contract-syntax`, `contract-types`, `contract`,
  `exact-apple`, `exact-linux`, `exact-web`, and `exact-web-js`. The first
  compiler suite caught the editor grammar's missing name; after updating
  that roster, its full rerun passes. Formatting, caps and issue checks pass.
- `cargo run -p contract-difftest -- quick` could not run the comparisons:
  `lake build: No such file or directory` (Lean is absent).
- The normal `scripts/agent.mjs macos --plan …` drive refused a stale
  signed-GPU receipt, including after the rebuild; the agent-refusal XCTest
  and the standalone process test above ran successfully. Native UI inspection
  returned `cgWindowNotFound`; the process fixture verifies clean boot state
  through persisted observations instead.
