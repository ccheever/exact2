---
name: 20261005-x19-data-source-timers
plan: 20261005-t3code-macos-parity
status: published
kind: framework-policy
blocks: [20261005-auto-balance, 20261005-client-activity-reporting, 20261005-embedded-server-runtime, 20261005-environment-routes, 20261005-live-automations-and-clones, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh, 20261005-pr-links-previews-and-routing, 20261005-reference-logic-test-ports, 20261005-reference-logic-tests-done-areas, 20261005-server-update-banner, 20261005-telemetry, 20261005-this-machine-network-access, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/124
reproduced_on: 4c893fef6
---

# X19: Timers and a clock inside data sources (`setTimeout`, `setInterval`, `Date.now`)

## Summary

T3 Code runs dozens of timers in its client code: report loops, debounces, retry cooldowns, live-refresh intervals, expiry windows, and per-query refresh intervals. Exact2 data sources have no timer or clock API (`EXACT2-GAPS.md` X19: "policy (LLP 1092 accepted, not built)"). The clone replaces each timer with one of three stand-ins: a Contract `every(ms, action)` task that passes `now` as an argument, a native sleep op capped at 2 s, or a Swift timer. Swift timers are not moved by the agent's virtual clock, and all three stand-ins put time logic outside the ported reference code. The needed support is the web timer API in the data runtime, driven by the same clock the agent controls.

## Why this issue arose

### The T3 Code behavior
The reference client never polls from a global scheduler. Each feature owns its timer, and the timer defines user-visible behavior. Reference evidence at `1e2ecbd975`:

| Timer | Reference | What the user sees |
| --- | --- | --- |
| Activity lease: report every 25 s, 250 ms debounce, 45 s interaction window and lease TTL | `apps/web/src/lib/backgroundActivityReporter.ts:25,26,245,250` | Server keeps provider status, usage limits and Git/PR refresh alive only while a client reports |
| Workspace-snapshot retry cooldown 10 s (A10) | `apps/web/src/components/chat/ChatComposer.tsx:1136,2243` | Slash commands and skills appear after a pending scan |
| Slow-request toast: 15 s, long-running methods 120 s | `apps/web/src/rpc/requestLatencyState.ts:7,13` | "Some requests are slow" |
| Live refresh: minimum 10 s, interval 5 min, idle after 6 min | `apps/web/src/hooks/useLiveRefresh.ts:19,22,31` | An open pull request view stays current; stops when nobody touches the window |
| Query refresh intervals: linked PR summary 60 s, linked threads 10 s, signed asset URLs 30 min (stale 5 min) | `packages/client-runtime/src/state/pullRequests.ts:114,218`, `assets.ts:28-30,142`, built on `Atom.withRefresh` (`state/runtime.ts:574-576`) | Counts and attachment URLs stay fresh with no user action |
| Relative-time ticks: 60 s (scheduled tasks), 1 s (Connections, default) | `ScheduledTasksSettings.tsx:320`, `ConnectionsSettings.tsx:614,992`, `settingsLayout.tsx:161-168` | "next in 5m" counts down |
| Pull request link resolve debounce 450 ms | `apps/web/src/components/PullRequestThreadDialog.tsx:54` | "Resolving pull request..." until 450 ms after the last keystroke |
| Hover card open 350 ms, close 120 ms | `components/pullRequest/PullRequestLinkPreview.tsx:112-113` | PR preview card |
| "Viewed" flag flush 400 ms | `components/pullRequest/usePullRequestFilesViewed.ts:27,181` | Files marked viewed save in one batch |
| Offline banner grace 2 s; disconnect-server delay 20 s | `components/ChatView.logic.ts:73`, `hooks/useEnvironmentDisconnectDelay.ts:11-15` | "<label> is reconnecting" appears after 2 s; "Disconnect server" after 20 s |
| Limits refresh deduplicated 5 min per environment | `packages/client-runtime/src/state/usage.ts:77` | Opening Limits twice does not probe twice |
| Provider update toast: success visible 3 s; Git action success 10 s | `ProviderUpdateLaunchNotification.logic.ts:68`, `GitActionsControl.logic.ts:57` | Toasts that close themselves |
| Codex sign-in callback expires after 300 s | `packages/shared/src/codexAuthCallback.ts:71-74` | "Sign-in expired. Try again." |
| Desktop server supervision: readiness probe every 100 ms (1 s per probe, 60 s round), restart backoff 500 ms to 10 s, terminate grace 2 s, output drain 5 s, quit wait 5 s, login-shell probe 5 s | `apps/desktop/src/backend/DesktopBackendManager.ts:57-68`, `app/DesktopApp.ts:154`, `shell/DesktopShellEnvironment.ts:93-94` | Local server starts, restarts and stops |
| Tailscale status cache 60 s | `backend/DesktopServerExposure.ts:28` | Network access dialog |
| Reconnect ladder 1 s doubling to 5 min, reset after 30 s, better-route cooldown 5 min | `packages/client-runtime/src/connection/supervisor.ts:35,36,43,49` | Reconnect behavior |

Other reference debounces exist in finished areas (draft persist 300 ms `composerDraftStore.ts:97`, file save 500 ms `components/files/useFileSaveCoordinator.ts:10`, palette search 120/200 ms `state/queries.ts:33-37`); how the clone replaces each was not checked for this issue (to confirm at `issue-open`).

### What exact2 does today
- `EXACT2-GAPS.md` (written from framework source at `c1522fdac`, checked against `main` `d2cb661eb`), summary row X19: "Timers/clock in data sources | Debounces, cooldowns (450 ms, 10 s) | policy (LLP 1092 accepted, not built) | time passed as arguments, Contract tasks".
- The bundled library (`20261005-platforms-v3`) does not cover timers in data sources: unknown. It documents a virtual clock in the agent: "`clock +N` for a timer, `clock settle` for the relevant settling boundary" (testing-and-debugging), a `reload` for persistence, and Contract-side `refresh result` for re-reading a resource (state-and-data). It says scheduled tasks belong at the root (components).
- Observed in the clone (mc-orch tree, 2026-10-05): Contract tasks `every(60000, tick)` (`app.contract:182-183`), `every(1000, liveTick)` (`:186-190`), `every(250, prSettle)` (`:109-115`), `every(1000, providerTick)` (`:355-357`) and `every(500, shellTick)` (`:1184-1190`); `now` reaches data sources as an argument (`wallTime.epochAtZero + elapsed`, `:61-64`). `native.later({op: 'timelineSleep'})` sleeps at most 2 s (`modules/apple/T3TimelineTurns.swift` `sleep`, routed at `T3Module.swift:72`). `native.later({op: 'r10Wake'})` re-reads the shell after a wait (`r10-connect-timing.ts:16-17`). Swift timers: transport ping, reconnect ladder and stream retry (`T3Transport.swift:641-648,657`), device streams (`R7DeviceClient.swift:126-154`, `R6DeviceStream.swift:275,321`), quit hold (`T3Menus.swift:433`), SnapShot (`T3SnapShot.swift`, `T3SnapshotFeedback.swift`).

### Where the clone hits it
- **Wait inside a command.** The PR link dialog waits 450 ms inside its command with `settle(native, 450)` (`r9-connect-checkout.ts:149-155`) and a native wake so "Resolving pull request..." can draw during the wait. The 2 s cap on `timelineSleep` bounds every such wait. A 10 s wait is a polling loop of 500 ms sleeps (`r12-threads-scratch.ts:18,30-36`).
- **Debounce by polling.** `every(250, prSettle)` runs for the life of the app to emulate a debounce (`app.contract:109-115`).
- **Cooldown windows in data code.** Each window (5 min limits dedupe, 10 s retry, 10 s minimum refresh) needs `now` in the resource arguments, so the resource identity changes with time and its answer is asked again.
- **Swift timers for the rest.** The 25 s activity loop, the 300 s callback expiry and the server supervision timers live in Swift. The workaround differs from the reference in behavior only at the edges (a wait capped at 2 s; ticks rather than exact deadlines). It differs most in testing: a Swift timer is not moved by `clock +N`, so the acceptance row for the activity cadence must wait real time (`20261005-client-activity-reporting` "Cadence": connect, wait 80 s, deactivate 60 s).

## Why it must be resolved

The goal is a clone that behaves like the reference and reuses its client logic with only the changes exact2 requires. The reference's logic is written against timers (`useLiveRefresh.ts`, `usage.ts`, `backgroundActivityReporter.ts`); porting it means rewriting each timer into one of three different mechanisms, so the port stops being the reference code. Each rewrite also changes how the logic can be tested: the reference tests use fake timers; the clone's convention is a `now` argument, which works for pure functions but cannot cover a loop that must run by itself.

Tickets that carry the workaround: the ones in `blocks` above. None is blocked outright. The cost of keeping it: extra root `every` tasks that wake resources, a 2 s ceiling on waits, timer logic in Swift that agent runs cannot advance, and attended or real-time rows where the agent could otherwise run. The user decision pending is whether LLP 1092's accepted direction is to be built for the data runtime.

## Requested support

Stated the web way, in the data runtime on macOS first:
- **A (recommended):** `setTimeout` / `clearTimeout` / `setInterval` / `clearInterval` and `AbortSignal.timeout(ms)`, with `Date.now()` and `performance.now()` agreeing with `exactTime()`. Timers belong to the answer that created them (dropped when it is let go). The agent's `clock +N` and `clock settle` advance them; real hosts use the OS clock.
- **B:** a Contract task that can send a data-source call on a period and pass the previous result back (no timer in the data module). Smaller, but waits inside a command (debounce, expiry) would still need a native sleep.
Other hosts (web, iOS, Linux) follow whichever path is chosen.

## How to reproduce

To confirm on the pinned `main` at `issue-open`. Minimal app: a data module `ping()` that calls `setTimeout(() => stamp = Date.now(), 1000)`. Reference result (web): the callback runs after about 1 s. Expected on exact2: no such API in the data runtime, or a callback not advanced by `clock +1000`. Clone scenario: open the Pull Requests page's link dialog, type quickly; the command waits through `timelineSleep`; set the wait to 3000 ms and observe the 2 s cap.

## Acceptance for the fix

- A data-source function schedules a one-shot and a repeating callback and cancels both; dropping the answer cancels them.
- `clock +1000` runs a 1 s timer once; `clock settle` runs due timers; `Date.now()` follows the virtual clock.
- Timer order and clamping match Chrome in a conformance case (`setTimeout(0)` ordering, nested timers).
- A timer survives window blur and fires on time while the app is in the background (macOS), or the documented limit is stated.
- AppKit test: 10 timers created and cancelled leave no pending work after the window closes.

## App adoption after resolution

Remove `timelineSleep` and `r10Wake` waits (`r10-connect-timing.ts`), the `every(250, prSettle)` task and the `now` arguments that exist only for cooldowns; move the 25 s activity loop to TypeScript beside the ported reporter. Reopen the rows that waited real time. `issue-close` verifies: the PR link dialog debounce, the activity cadence row on the agent clock, and the limits dedupe row.

## Status and next action

Published 2026-10-06 as [#124](https://github.com/ccheever/exact2/issues/124) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Fix built (2026-10-06)

**Declined (2026-10-08):** #124 keeps module timers refused; this branch is not pursued (recorded only; the branch is kept).
Built on exact2 `origin/main`, branch `daehyeon/fw-x19-source-waits` (worktree `~/orca/workspaces/exact2/t3-fw-x19`), commits `822e2502d` and `5969c4819` (review fixes). Not pushed.
- `setTimeout`/`clearTimeout` inside an answer: a wait the runner holds (`exact-wait:<ms>`) and lands from its own clock, so the agent's `clock +N` moves it and a device wakes for it through `timer_due_ms`. Not I/O: `clock settle`/`clock data` do not wait on it. A newer request forgets it (a debounce restarts). Hosts note their input time so a wait after an idle spell is due from then. `setInterval`, and `setTimeout` outside an answer, stay refused.
- Known limits: two waits in one answer run in sequence on native hosts, and a wait raced against a fetch cannot fire first; on the JS target a let-go answer runs past its next wait.
- Not built here: LLP 1092 stage 2 (gated tasks: `task … when … key=`), which covers most debounce/window/periodic needs at the Contract level; it is accepted upstream and assigned to Charlie's lanes.
- Evidence: the five checks; `source_waits.rs` (5), `js/tests/it/waits.rs` through the real Hermes VM, `request-refusal.test.mjs`; a scratch app (450 ms debounce, backoff retry) passes its `clock +N` test on the web JS target, the wasm web host and macOS; one independent review, its findings fixed or documented.
- Before main: LLP 1027.000 "Amendment to D1" (proposed) needs Charlie's ruling and a DEFERRED take or waiver.

## #192 checked (2026-10-07, adopt-main-fixes-r4)

Main #192 (merged before the feature branch's base) is documentation: the `analyze-then-self-send`
diagnostic and the agents' guide point at a gated task (`task NAME when COND` with `every` or `after`) for
"repeat or wait while a condition holds". The clone has no `pause`-mutation loop of the kind #192 warns
about. Three of its mount polls were a gated task's job and are converted
([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)): the Pull Requests search
(`every(250)` checking `now() - typedAt`, so the query applied 250–500 ms after the last keystroke) is now
the reference's 250 ms debounce (`task prSearchSettle when … key=prQuery` with `after(250, …)`); a
terminal's close confirm and a thread the module opened (both read from the shell snapshot on a 500 ms
poll, so up to 500 ms late) now run at once (`task terminalCloseAsk`, `task shellOpen`). The other mount
clocks (`clock`, `liveClock`, `providerClock`, `sshClock`, `shellTicks`' toast clock) stay: each advances a
shown time or retries a read on a fixed cadence rather than bridging an event. #124 (timers in data
sources) is still open.

## Decided upstream (2026-10-08): declared difference

[Charlie on #124](https://github.com/ccheever/exact2/issues/124#issuecomment-6055589736): "Keep module timers refused; finish failure-parity repair separately. … Keep open until #228's
parity slice is resolved, then close the timer request not planned."
- **Declared difference (permanent):** time passed as arguments, gated tasks (`task … when … key=` with
  `after(…)`), `timelineSleep` and `r10Wake` stay wherever they differ from the reference's timers.
- The local fix ("Fix built" above) is declined by the decision and not pursued.
- Main renamed `now()` to `performanceNow()` (`9731c8056`); the clone's 26 sites are renamed by round 7 or by the
  root rewrite, whichever runs on that main first.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): "existing draft PR #228, incomplete" (JS sync-send and queued-send cases missing); timers stay refused.
