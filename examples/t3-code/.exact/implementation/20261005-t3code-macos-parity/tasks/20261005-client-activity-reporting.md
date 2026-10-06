---
name: 20261005-client-activity-reporting
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: blocked
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3code-parallel-features
branch: daehyeon/t3code-client-activity-reporting
pr_url: null
verified_commit: null
---

# The app keeps the server's background work alive and finds workspace commands

## Outcome

While a window is open on a connected environment, the app tells that environment's T3 server
that a person is looking at it, in the same way as the reference desktop renderer. The server
then keeps refreshing provider status, usage limits and Git/PR remote status. When the person
stops using the app, those refreshes pause as they do in the reference. Also, the composer asks
the server for the skills and slash commands of its own workspace and retries while the server
says the commands are still pending.

## Scope and exclusions

Included:

1. **Activity lease (G1).** Method `server.reportClientActivity`, sent to every connected
   environment: the focused one and each fleet transport (`T3Fleet`, up to 8). Schedule from
   the reference: one report when an environment connects, then every 25 s (spaced). Extra
   reports on: app/window focus, blur, visibility change, network back online, change of the
   environment list or of an environment's connection phase, and when a scope is retained or
   released. All requests share one single-slot queue with a 250 ms debounce. Interaction
   (pointer move, key down, wheel) requests a report only if the previous interaction is more
   than 45 s old; the app start counts as an interaction. Errors are ignored.
2. **Payload.** `environmentId`, `clientId` (stable per install, at most 128 characters),
   `clientKind: "desktop-renderer"`, `visible`, `focused`, `recentlyInteracted` (interaction
   within 45 000 ms, never in the future), `appState` (`active` when visible, else
   `background`), `ttlMs: 45000`, `observedAt` (wall time), `scopes`. No battery, low-power or
   network fields (the reference web client sends none).
3. **Scopes.** `{type:"provider-status"}` always, plus one scope per open subscription of the
   same environment, reference-counted: `subscribeVcsStatus{cwd}` gives `vcs-status{cwd}`;
   `subscribeResourceTelemetry` gives `diagnostics`. Keys are delimiter-safe.
4. **Workspace discovery (A10).** The composer calls `server.refreshProviders{instanceId,cwd}`
   once per `environment:instance:cwd` when the selected provider has no complete snapshot for
   the workspace (a snapshot with `slashCommandsPending` is incomplete). A failed or
   incomplete answer is retried after 10 s. Pending commands still show with the skills.

Excluded: host power (`server.reportHostPowerState` has no client caller; the Electron main
process gives host power to its own server, which belongs to `20261005-embedded-server-runtime`),
the Settings › Background activity UI (done: `settings-a-background.ts`), any server code.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`apps/web/src/lib/backgroundActivityReporter.ts:22-33,69-78,176-250`,
`apps/web/src/connection/runtime.ts:54`, `packages/contracts/src/background.ts:67-82`,
`apps/server/src/background/BackgroundPolicy.ts:81-124,200-300` (what a lease means),
`apps/server/src/provider/makeManagedServerProvider.ts:216,258-272`,
`apps/server/src/usage/UsageLimitSources.ts:159`, `apps/server/src/vcs/VcsStatusBroadcaster.ts:519,559`
(the three gates). A10: `apps/web/src/components/chat/ChatComposer.tsx:1136,2183-2268`,
`packages/client-runtime/src/providerSkills.ts:114-120`, commit `836098543c`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (actions read a
snapshot; a root task ticks; await long work), components (root owns tasks), testing-and-debugging
(real-time cadence is not on the agent clock; keep source identity), accessibility (no UI here).
The library does not cover app-local Swift modules, timers, `NSEvent` monitors or window
notifications: they are **unknown in the library**; the clone's own `T3Transport.swift` and
`T3Signals` (`modules/apple/T3Protocol.swift:273-296`) on the pinned main are the basis.
Consumer framework revision: the pin from `20261005-clone-on-exact2-main`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Clone evidence (mc-orch tree, 2026-10-05): no `reportClientActivity` anywhere; transport has
a 1 s tick for ping only (`T3Transport.swift:654-679`); `subscribe` receives method and payload
(`:525-535`); the fleet is `T3Fleet.swift:8-70`; `shell-vcs.ts:97` and
`settings-a-telemetry.ts:111` own the two subscriptions; workspace snapshots are only read
(`composer-editor.ts:234-243`); the clone's clock pattern is `every(ms, tick)` plus `now`
passed to resources (`app.contract:182-190,1184-1190`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged into `daehyeon/t3-code` | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Trace proxy, trace diff and Electron oracle available | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records in `../issues/` only (not searched upstream).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X19](../issues/20261005-x19-data-source-timers.md) | Timers/clock in data sources (A10 10 s retry) | `EXACT2-GAPS.md` X19 | nonblocking (workaround: root `every` task that advances `now` only while a refresh is pending; the 25 s timer is a Swift timer) | Prove the retry on the agent clock |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket (sending the report) | `EXACT2-GAPS.md` X21 | nonblocking (workaround: Swift transport) | none |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` line cap | 1,327 of 1,500 lines | nonblocking until the cap | Keep A10 state in `composer-editor.ts`; add no root state if possible |
| [X28](../issues/20261005-x28-notification-actions-badges.md) | Window-focus fact for TypeScript (key window, occlusion, hidden app are read in Swift here) | `EXACT2-GAPS.md` X28 lists the window-focus fact with the notification need | nonblocking (workaround: Swift reads them and sends the report itself; TypeScript needs none) | none |

## Implementation notes

- New `modules/apple/T3ActivityReporter.swift`. A transport starts it at state `connected` and
  stops it in `retire`; fleet transports do the same. One shared object observes the window
  and interaction facts. Proposal (unknown in the library): `visible` = window occlusion
  state is visible, not miniaturized, app not hidden; `focused` = app active and window key;
  interaction = `NSEvent.addLocalMonitorForEvents` for mouse-moved, key-down, scroll-wheel.
  Do not count clicks without a move; the reference does not.
- Keep method and payload with each stream in `T3Transport` so scope release follows
  `unsubscribe` (`:537`) and stream failure. Refcount by a JSON-array key
  `[environmentId, type, cwd|null]`.
- Send through the transport's own `rpc` path so the reply is consumed; ignore the result and
  never toast.
- `clientId`: a UUID written once to `t3-code.json`; fallback `ephemeral-client` if unreadable
  (reference fallback: `ephemeral-browser-client`).
- A10: port `hasCompleteProviderWorkspaceSnapshot` into a new `composer-workspace-snapshots.ts`
  (file header: source, changes). Cooldown state per client in `composer-editor.ts` `caches`;
  `now` comes from the editor resource. Call through `client.restAccess(native).request`
  (as `palette-commands.ts:180`). The config subscription delivers the new snapshot.
- Register the new native ops and the reporter start through the per-area seams that
  `20261005-hot-file-split` creates.

## Acceptance and reproduction

Every row, attended or not, runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773; see
`20261005-embedded-server-runtime`).

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Cadence | Isolated lane backend (ports 16000–16999) behind `target/t3-ui-parity/trace-proxy.mjs`; app focused | Connect, wait 80 s, deactivate the app 60 s, reactivate | Reports at connect and about every 25 s; one report within 0.3 s of deactivate (`focused:false`) and one of reactivate; every payload `ttlMs 45000`, `clientKind desktop-renderer` | macOS 1280×840, real time (the Swift timer ignores the agent clock) | trace file; `target/t3-ui-parity/trace-diff.mjs` against `target/t3-ui-parity/electron-oracle.mjs` on the same scenario, allow list with reasons |
| Scopes | Git thread; details card closed | Open the card; open Settings › Diagnostics; close both | `vcs-status{cwd}` appears within one debounce and leaves after close; `diagnostics` likewise | macOS | trace |
| Interaction window | App focused, no input for 50 s | Then move the pointer (attended session) | `recentlyInteracted:false` after 45 s; a report with `true` within 0.3 s of the move | macOS, real pointer | trace + session notes `(attended session)` |
| Several environments | Lane backends A and B switched on | Toggle B off and on | Each report names its own `environmentId`; B gets none while off | macOS | trace |
| Server effect | Server override of the provider refresh interval to a short value; profile Balanced | Baseline build: wait 3 intervals. New build: same | Baseline `checkedAt` stays; new build advances each interval; after 45 s deactivated plus one interval it stops | macOS | `server.getConfig` samples, both runs |
| Silent failure | Stop backend B mid-run | Wait one cycle | No toast, no log refusal from reports | macOS | `logs` |
| Pure logic | — | Swift test binary `macos/tests/activity` with the four tests of `backgroundActivityReporter.test.ts` under their original names ("expires interaction independently of window focus", "rejects future timestamps", "retains an observed subscription until its returned finalizer runs", "keeps delimiter-containing environment and scope values distinct") | Pass | macOS | test log |
| Workspace discovery | Draft in project P; provider without a snapshot for P's cwd | Open the draft; then return a snapshot with `slashCommandsPending` | One `server.refreshProviders{instanceId,cwd}`; none again for that key; the retry runs at `clock +10000` and not at `+9000` | macOS agent drive | trace + `--json` transcript |
| A10 logic | — | `bun test` port of `providerSkills.test.ts:255-276` ("uses partial workspace skills and commands while keeping discovery retryable") | Pass | macOS host machine | log |
| Clone checks | `git add -A` | `bun test examples/t3-code`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries, `bun scripts/caps.mjs`, the five checks | All green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States: no visible UI. Disabled: a switched-off environment gets no report. Error: silent.
Reduced motion: not applicable.
Task-owned source paths: `examples/t3-code/modules/apple/T3ActivityReporter.swift`,
`macos/tests/activity/`, `composer-workspace-snapshots.ts` and its test, `composer-editor.ts`,
hunks for `T3Transport.swift`, `T3Fleet.swift`, `T3Module.swift`.
Required environment: Xcode 27.0, pinned Bun 1.4.2, reference runtime copy, no T3 Code (Nightly) running.

## Progress

2026-10-06: local implementation complete on the task-named branch, ready for integration
and verification. User authorized independent parallel implementation before procedural common
PR gates; main migration and desktop-oracle dependencies remain pending, not satisfied.
Base: round-12 snapshot `1c6b4a12a`, framework `c1522fdac` plus the existing native parking
patch. This is **not** the prepared main pin. Source behavior checked against reference
`1e2ecbd975`; guidance revision `20261005-platforms-v3` (foundations, state/data,
components, platforms and testing). Native timer/window APIs remain app-local guidance.

- Added a shared native activity reporter: 25-second cadence, 250-ms single-slot debounce,
  one batch in flight, silent errors, wall timestamps, interaction expiry and AppKit window
  focus/visibility notifications. Window mouse-move delivery is enabled while observing and
  restored at teardown. Scope retain/release follows transport subscribe/unsubscribe/Exit/retire.
- Install identity is retained in `t3-code.json`; draft/preference writes preserve it. Fleet
  and focused transports share one reporter and identity. Persistent identity failure falls
  back to `ephemeral-client`; isolated agent reporters use an ephemeral UUID.
- Workspace discovery reuses partial skills/commands, deduplicates in-flight requests and
  rejects obsolete selection/reset completions. Its pure readiness resource is independent
  from editor/menu reads. A root mutation awaits refresh completion and owns the 10-second
  retry deadline on the Exact clock, avoiding source timers and slow-RPC menu stalls.
- Independent implementation review found delayed-menu, completion-clock, stale-reset and
  pointer-delivery concerns; corrected, regression-tested and re-reviewed with no remaining
  concrete finding. This is implementation review, not final acceptance.

Integration seams owned by the coordinator: `T3Module.swift` creates and destroys one reporter
and passes `activity:` to the focused transport and fleet. Root Contract registers
`composerWorkspace(data.revision)` returning `{key, needed}` and an awaited
`refreshComposerWorkspace(key)` mutation returning `{key, retry}`. Its completion action
sets a keyed `now() + 10000` retry deadline; existing one-second ticks dispatch only when
needed, not pending and due. `macos/src/markdown.rs` registers both TypeScript source names.
These shared-file changes are delivered by the coordinator/timeline lane, not this commit.

The native mapping used here is visible = a visible, unminimized main-capable app window with
visible occlusion while the app is not hidden; focused = active app with a key main-capable
window. Real-input/oracle parity of this mapping remains unverified. No pixel-perfect loop.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 2026-10-06 local implementation | round-12 `1c6b4a12a`, reference `1e2ecbd975`, Bun 1.4.2, macOS/Xcode host | Frozen install passed; 6 workspace logic tests passed; strict TypeScript passed; native activity 7/7 passed; full Swift module build plus transport 36/36 passed before final single-flight callback refinement (refined reporter compiled/tested afterward). Full Bun 1152 passed, 1 source-registration integration failure | Existing commands in README; new `macos/tests/activity/main.swift` and `composer-workspace-snapshots.test.ts`; local logs `/tmp/t3-activity-{bun,native,transport}-tests.log` (not portable proof) | Shared source registration/root/module integration; fresh integrated build; cadence/server effects/attended pointer/agent retry/oracle traces and five gates unverified |

## Next action

Verification remains `blocked` after functional acceptance below. Capture paired high-resolution focus/input timestamps to verify the specified 0.3-second latency and run the pinned Electron lifecycle trace scenario. Keep the task active; no closure or PR publication. The source-function comparison is not an Electron run.

## Combined integration, 2026-10-06

Integrated in `daehyeon/t3code-parallel-features` with shared root/native registrations.
Combined validation: 1,197 Bun tests, strict TypeScript, Contract compilation, 10 app
Rust tests, formatting, staged caps and boot passed. The tab Contract hooks use the
pinned compiler's supported vocabulary; timeline activity fields satisfy generated
Contract types. Independent code reviews completed. No pixel-fidelity loop was run.
Main migration, newer oracle runtime and live feature acceptance remain pending;
this evidence does not close the task's verification gate.

Integrated native bundle build passed (app Rust bake and full Swift module). The
isolated macOS driver launched the app and read its disconnected tree. The bounded
interaction check did not pass: the first requested welcome target was absent; the
actual Open Connections target was outside the default viewport, then reported hidden
or inert at 1280×900. Stopped after three attempts without UI adjustment. App interaction
acceptance remains unverified. Local evidence: `/tmp/t3-parallel-final-native.log`,
`/tmp/t3-parallel-final-native-tests.log`, `/tmp/t3-parallel-final-bun.log`, and
`/tmp/t3-parallel-final-smoke.log`. No push or PR publication performed.

## Exact skill verification, 2026-10-06

**Result: blocked.** Native7, workspace6 and80s cadence pass. Live proxy confirms accepted reports,25s cadence, stable identity and focus/input/scope transitions. Provider refresh effects, two-server live lifecycle, root retry and reference comparison remain unverified.

See [live attempt](../evidence/parallel/20261006-live-verification/attempt.md), [capture report](../evidence/parallel/20261006-live-verification/checks-final/report.json), and [independent review](../reviews/20261006-parallel-verification.md). Source remained unchanged. No framework issue was resolved or closed by this app-only verification.


## Repair-session verification, 2026-10-06

No activity production change was needed. **Verification remains blocked**, with the following formerly missing functional checks now established:

- [Real server effects and reference payload semantics](../evidence/20261005-client-activity-reporting/20261006-server-effects-and-reference/attempt.md): production reporter/transport/fleet against two isolated reference servers. Provider timestamps remain stable without reporting, advance during actual native foreground demand, and remain stable after48s hidden. B off/on/reconnect and diagnostics scope retain/release pass. The executed reference payload function matches native lease fields; this is not an Electron capture.
- [Final actual root-clock retry](../evidence/20261005-client-activity-reporting/20261006-final-root-deadline/attempt.md): completion21000 sets deadline31000; +9000 produces no refresh, +10000 adds exactly one and sets41000. Current activity source comparison passes. The initially overshooting helper was corrected after preserving its failed assumption; app behavior was already correct.
- [Final main-app silent failure](../evidence/20261005-client-activity-reporting/20261006-final-silent-failure/attempt.md): named app61311 connected before recorded backend6207 stopped; after27s real waiting, disconnected with zero new toasts or activity-report refusal logs. Before-to-after elapsed71,746ms. App/backend/proxy and component servers were stopped by their owners.

Each final capture has passing scoped runner assertions and a validated evidence manifest. These reports establish their stated checks, not whole-task acceptance. Earlier captures remain historical; the final root-clock capture supersedes the stale full-file fingerprint from the first pass. Current source, test/build gates and review are coordinated in [repair review](../reviews/20261006-repair-verification.md).

Remaining mandatory gaps: measured <=0.3s focus/pointer-to-report latency, and the actual reference Electron lifecycle trace/diff. Existing GUI action logs have no paired high-resolution event timestamp, so successful transitions cannot prove that bound. Common main/oracle merge prerequisites remain pending as recorded above. The task stays active with `implementation: in-progress`, `verification: blocked`; no task or framework issue was moved to `closed` by this verification.
