---
name: 20261005-x19-data-source-timers
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-policy
blocks: [20261005-auto-balance, 20261005-client-activity-reporting, 20261005-embedded-server-runtime, 20261005-environment-routes, 20261005-live-automations-and-clones, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh, 20261005-pr-links-previews-and-routing, 20261005-reference-logic-test-ports, 20261005-reference-logic-tests-done-areas, 20261005-server-update-banner, 20261005-telemetry, 20261005-this-machine-network-access, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/124
reproduced_on: 4c893fef6
---

# X19: Timers and a clock inside data sources (`setTimeout`, `setInterval`, `Date.now`)

Moved to main `issues/20261009-typescript-failure-parity.md` (2026-10-09); tracked there.

Main's file carries #124's remaining failure-parity work; module timers stay refused, so time as arguments and gated
tasks are the clone's design (a permanent declared difference, decision of 2026-10-08).

## Summary

T3 Code runs dozens of timers in its client code: report loops, debounces, retry cooldowns, live-refresh intervals, expiry windows, and per-query refresh intervals. Exact2 data sources have no timer or clock API. The clone replaces each timer with one of three stand-ins: a Contract task that passes `now` as an argument, a native sleep op capped at 2 s, or a Swift timer. Swift timers are not moved by the agent's virtual clock, and all three stand-ins put time logic outside the ported reference code.

## Why it arose

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

Other reference debounces exist in finished areas (draft persist 300 ms `composerDraftStore.ts:97`, file save 500 ms `components/files/useFileSaveCoordinator.ts:10`, palette search 120/200 ms `state/queries.ts:33-37`).

### Where the clone hit it
- **Wait inside a command.** The PR link dialog waits 450 ms inside its command with `settle(native, 450)` (`r9-connect-checkout.ts:149-155`) and a native wake so "Resolving pull request..." can draw during the wait. The 2 s cap on `timelineSleep` bounds every such wait. A 10 s wait is a polling loop of 500 ms sleeps (`r12-threads-scratch.ts:18,30-36`).
- **Debounce by polling.** `every(250, prSettle)` ran for the life of the app to emulate a debounce (`app.contract:109-115`; converted in r4, below).
- **Cooldown windows in data code.** Each window (5 min limits dedupe, 10 s retry, 10 s minimum refresh) needs `now` in the resource arguments, so the resource identity changes with time and its answer is asked again.
- **Swift timers for the rest.** The 25 s activity loop, the 300 s callback expiry and the server supervision timers live in Swift. A Swift timer is not moved by `clock +N`, so the acceptance row for the activity cadence must wait real time (`20261005-client-activity-reporting` "Cadence": connect, wait 80 s, deactivate 60 s).

## Clone workaround

Declared difference (permanent): time passed as arguments, gated tasks (`task … when … key=` with `after(…)`), `timelineSleep`
and `r10Wake` stay wherever they differ from the reference's timers. Observed in the clone (mc-orch tree, 2026-10-05):
Contract tasks `every(60000, tick)` (`app.contract:182-183`), `every(1000, liveTick)` (`:186-190`), `every(1000, providerTick)` (`:355-357`)
and `every(500, shellTick)` (`:1184-1190`); `now` reaches data sources as an argument (`wallTime.epochAtZero + elapsed`, `:61-64`).
`native.later({op: 'timelineSleep'})` sleeps at most 2 s (`modules/apple/T3TimelineTurns.swift` `sleep`, routed at `T3Module.swift:72`).
`native.later({op: 'r10Wake'})` re-reads the shell after a wait (`r10-connect-timing.ts:16-17`). Swift timers: transport ping, reconnect ladder and stream retry
(`T3Transport.swift:641-648,657`), device streams (`R7DeviceClient.swift:126-154`, `R6DeviceStream.swift:275,321`), quit hold (`T3Menus.swift:433`), SnapShot (`T3SnapShot.swift`, `T3SnapshotFeedback.swift`).

## Evidence and history

- Filed as [#124](https://github.com/ccheever/exact2/issues/124) on 2026-10-06, reproduced on exact2 `4c893fef6` before filing.
- A local fix was built on 2026-10-06 (branch `daehyeon/fw-x19-source-waits`, commits `822e2502d` and `5969c4819`, not pushed): `setTimeout`/`clearTimeout`
  inside an answer as a wait the runner holds and lands from its own clock. Declined by the decision of 2026-10-08 and not pursued (the branch is kept).
- Main #192 checked in [20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md): three mount polls became gated tasks. The Pull Requests search
  (`every(250)` checking `now() - typedAt`) is now the reference's 250 ms debounce (`task prSearchSettle when … key=prQuery` with `after(250, …)`); a terminal's
  close confirm and a thread the module opened now run at once (`task terminalCloseAsk`, `task shellOpen`). The other mount clocks (`clock`, `liveClock`,
  `providerClock`, `sshClock`, `shellTicks`' toast clock) stay: each advances a shown time or retries a read on a fixed cadence.
- Main renamed `now()` to `performanceNow()` (`9731c8056`); the clone's sites are renamed by round 7 or by the root rewrite, whichever ran on that main first.
