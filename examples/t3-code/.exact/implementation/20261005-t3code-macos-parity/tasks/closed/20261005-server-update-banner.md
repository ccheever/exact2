---
name: 20261005-server-update-banner
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-server-update-banner
pr_url: https://github.com/ccheever/exact2/pull/172
verified_commit: null
---

# The composer shows server updates and offline state; the details card warns about version skew

## Outcome

In a chat, the composer banner stack tells the user when the connected server is older than the
app, offers the right update path, and shows the update's progress and failure. The offline
banner behaves as in the reference (grace period, "Disconnect server"). The thread details card
and its toggle show a version-differ warning.

## Scope and exclusions

1. **Server update banner (G6).** Item `server-version:<environmentId>` shows when the server
   version is behind the app (nightly-aware compare, done: `connections.ts:57`), the notice is not
   dismissed, and the draft is not on Auto balance.
   - Idle: icon download; title "Server update available" with tooltip "<label> server X → Y"
     (label "server" when only one environment exists). Text: "Update to stay in sync" for a
     self-updating server that is not desktop-managed; "Update the desktop app" for a
     desktop-managed server whose app cannot update remotely (and then no button); no text when the
     server has no self-update, or when a desktop-managed app can update.
   - Action by capability: **Update** (desktop-managed adds the confirm below); no self-update:
     **Copy update command** (npm-global) or **Copy relaunch command** with the reference toasts.
     The line "Update the desktop app on that machine to update this server." belongs to the
     multi-machine popover (`20261005-auto-balance`) and to Settings rows, not to this banner.
   - Running (priority urgent): spinner, "Updating <server> · Downloading…" then "· Restarting…"
     (wire stages `downloading` and `installing` both read "Downloading…"); no Update button, only
     Disconnect server while the environment is unavailable and allowed. Failed: red alert icon,
     "Could not update <server> · <message>", **Retry**, dismiss "Dismiss update notice". Dismissal
     of the notice is stored by `environment:client:server` version key; a failed attempt is
     dismissed in memory for that attempt only.
   - Update flow: `server.updateServerWithProgress` stream when the server advertises
     `serverSelfUpdateProgress`, else `server.updateServer` (a transport loss on boot-service and
     respawn counts as the handoff); desktop-managed: confirm "Update the T3 Code desktop app that
     runs the <label>? It will close and relaunch on that machine.", then
     `server.commitDesktopUpdate`; wait for `subscribeServerLifecycle` ready with the target version
     (4 minutes); then toast "<label> updated" with "Reconnected on t3@<version>." or "Desktop app
     relaunched on <version>."; `continueRunningThreads` only when the setting and capability allow.
     Double clicks start one update per environment.
2. **Offline banner (CN2).** "<label> is reconnecting" appears only after 2 s of connecting or
   reconnecting, and never while an update runs; "<label> is offline" (warning; error variant for
   a failed connection) has **Reconnect** (failure toast "Could not reconnect environment") and,
   after 20 s unavailable and only for a non-primary, non-local environment, **Disconnect server**
   (tooltip "Hide this server's threads. Switch it on again in Connections."; switches the
   environment off, returns to Home; failure toast "Could not disconnect server").
3. **Version-differ card (CN3).** In the thread details card: "Client and server versions differ",
   "Client X · <server label> Y", dismiss button `aria-label` "Dismiss version mismatch warning"
   (shares the notice's dismissal). The details toggle shows a 6 px warning dot while the card or
   the offline banner is active.

Excluded: Auto balance and the multi-machine update banner (`20261005-auto-balance`, which reuses
this ticket's update state); install-aware command text, `ServerInstallation` and the remote scope
fix (`20261005-remote-scopes-and-update-commands` supplies `manualServerUpdateCommand`; this ticket
calls it); Settings › Connections update rows (done: `connections.ts:244-251,396-425`) except that
they share the new update state; outdated hosts (done: `T3Fleet.swift:274-414`); the desktop app's
own update feed.

## Context and guidance

Parent specification: [spec](../../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/components/ChatView.tsx:2575-2660,2975-3157,10720-10745`,
`.../ServerUpdateAction.tsx`, `.../chat/ComposerServerUpdateStatus.tsx`, `.../chat/ComposerBannerStack.tsx`,
`.../chat/ThreadDetailsPanel.tsx:131-150`, `.../chat/PanelLayoutControls.tsx:61`,
`.../hooks/useEnvironmentDisconnectDelay.ts` (20 s), `ChatView.logic.ts:73,459-469` (2 s),
`apps/web/src/versionSkew.ts`, `packages/client-runtime/src/state/server.ts:60-100,100-282,684-860`.
Port with their names: the update state helpers (`serverUpdateStateForProgressEvent`,
`serverUpdateStateForServerVersion`, `serverUpdateFailureMessage`, `isLegacyUpdateHandoffLoss`,
`matchesServerUpdateResumeEvent`, `validateServerUpdateReadyEvent`, `nudgeReconnectDuringUpdateRestart`);
`versionSkew.ts` (`buildVersionMismatchDismissalKey`, `isServerUpdateFailureDismissed`,
`serverUpdateGuidance`, `dismissVersionMismatch`); `hasEnvironmentReconnectWarningGraceElapsed`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (await long operations; a
failed request keeps its previous value; no double send of one mutation), components (state in
child instances, resources at the root), layout-and-interaction, accessibility (alerts, labels,
keyboard), motion (banner exit 220 ms, reduced motion), testing-and-debugging.
**Unknown in the library:** data-module timers, app-local Swift (the update run uses `T3Fleet`/
`T3Transport`), and the banner stack's native pieces; the clone's code on the pinned main is the basis.
Clone evidence: the banner stack is `composerNotices` (no server notice;
`composer-controls-view.ts:184-230`); offline banner `requests.ts:132-141` (Reconnect only);
one-shot update `connections.ts:396-425` (`server.updateServer`, no progress); draft context
`composer-controls-branch.ts`; menu rows `MenuChoice` (`requests.contract:160-175`).
The loopback stand-in for the primary (`isLoopback`, `shell-details.ts`) decides "primary" until
`20261005-local-primary-environment` replaces it.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Merged | pending |
| merged task PR | 20261005-remote-scopes-and-update-commands | pending | `manualServerUpdateCommand` with install kinds merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| scheduling preference | 20261005-main-fix-adoption | pending | Merged first (popover, tooltip) | pending |
| decision | U2: stub T3 RPC server for version skew | none | The lane server cannot be older than the app and a real update would restart it. Approve the stub (`getConfig` with an older `serverVersion` and capabilities, `updateServerWithProgress` events, lifecycle `ready`) as apparatus, or accept unit tests plus an attended session against a real older server. | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X19](../../issues/20261005-x19-data-source-timers.md) | Timers for the 2 s grace and the 20 s delay | `EXACT2-GAPS.md` X19 | nonblocking (workaround: root `every` task advancing `now` only while an environment is unavailable; pure functions take `now`) | none |
| [X21](../../issues/20261005-x21-two-way-websocket.md) | Streamed update progress and commit over the socket | X21 | nonblocking (workaround: Swift transport streams; reuse the outdated-host stream code) | none |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap (`20261005-hot-file-split` makes room) | Per-environment update state in a Swift or `.ts` store, not root state |
| [X17](../../issues/20261005-x17-popover-position-try.md), [X11](../../issues/20261005-x11-shadow-blur-parity.md) | Tooltip placement and shadow | X17, X11 | nonblocking (declared visible difference) | Declare |
| [X40](../../issues/closed/20261005-x40-app-update-feed.md) | The T3 desktop update feed (server-triggered desktop update) | X40 scope decision | nonblocking for the banner; the server-triggered desktop-update part waits for X40 | follow the X40 decision |

## Implementation notes

- Add the update notice and the offline notice to the same ordered list as the existing notices
  (`composerNotices`) with the reference priorities (activity, then urgent/error/warning, then notices).
- Keep per-environment `ServerUpdateState` (`idle | running{stage} | failed{message}`) in one store
  shared by this banner, Settings › Connections and `20261005-auto-balance`'s Update-all action. Expose
  one single-flight `updateEnvironment(target)`; the sibling adds the batch.
- A real update restarts the server and may relaunch a desktop app: never run it against the user's real
  servers; lanes use the stub or an isolated older server.
- `20261005-local-primary-environment` later replaces the loopback stand-in; use one helper for
  "can disconnect".

## Acceptance and reproduction

Every row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`
(dev and lane builds refuse the real `~/.t3` and port 3773; see `20261005-embedded-server-runtime`).

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Idle banner | Stub or older server; self-update capability | Open a thread | Title, tooltip, text, Update button as above | macOS 1280×840 and 840×620, light and dark | pixel pair vs `target/t3-ui-parity/electron-oracle.mjs` with the same stub; `--json` tree |
| Command variants | Stub without self-update (npm-global and relaunch kinds) | Press the button | Clipboard text and toast wording equal the reference | macOS | clipboard + trace |
| Progress and success | Stub emits `downloading`, `installing`, `complete`, then lifecycle `ready` | Press Update | Rows "Downloading…" then "Restarting…"; then toast "<label> updated"; banner gone; one update per double click | macOS | agent drive with `clock`; `target/t3-ui-parity/trace-diff.mjs` vs oracle |
| Failure and dismiss | Stub fails at `installing` | Press Update, Retry, dismiss | Red row with message; Retry restarts; dismissal hides that attempt only; version dismissal persists after relaunch | macOS | screenshots; `t3-code.json` |
| Desktop-managed dialog | Stub `desktop-managed` | With and without `desktopAppUpdate`; confirm dialog by keyboard | Confirm wording, or the no-button text; focus lands in the dialog, Enter confirms, Escape cancels and focus returns to the banner (check the oracle first) | macOS | screenshots; `tree --ax`; `(attended session)` for real keys |
| Offline banner | Lane backend A | Stop A for 1 s, then 5 s, then 25 s | No banner under 2 s; "reconnecting" then "offline"; Disconnect server only after 20 s and not for the primary; Disconnect switches the environment off and goes Home | macOS, `clock` | screenshots; trace |
| Version card | Same stub | Open details card and toggle; dismiss | Card text, dismiss label, 6 px warning dot; dismissal shared with the notice | macOS | pixel pair; `tree --ax` |
| Banner keyboard and motion | Same stub | Tab through banner buttons; dismiss; set prefers-reduced-motion | Visible focus, `aria-label` on icon buttons; 220 ms exit slide; with reduced motion opacity only | macOS | `tree --ax`; film (`over 300 every 30`) both modes |
| Logic ports | — | `bun test` ports of `versionSkew.test.ts` (18, original names), `ServerUpdateAction.test.tsx` single-environment cases ("reports success only after the shared update flow reconnects", "reports one result when the update action is double-clicked", "quietly releases the action when the operation is interrupted", "keeps the manual instruction for desktop servers without remote update support", "updates remote desktop apps through the shared update flow", "leaves thread continuation off by default", "applies the saved thread continuation preference automatically") and the `ServerUpdateProgress` cases, `server.test.ts` "update restart reconnect nudges" and four update cases ("only treats a legacy transport interruption as an unacknowledged handoff", "projects streamed update milestones into the shared operation state", "keeps active update state and hides stale failures after a version change", "requires tokenless desktop updates to reach the target version"), `ComposerBannerStack.test.tsx`, the reconnect-grace cases of `ChatView.logic.test.ts` | Pass | host machine | log |
| Clone checks | `git add -A` | `bun test examples/t3-code`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries, `bun scripts/caps.mjs`, the five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States covered: loading (spinner), empty (no notices), error (failed update), disabled (no Update while
running), hover (tooltip), keyboard focus on every banner button and the confirm dialog, Escape, alerts
announced (`role` alert for failure, status for progress), motion: 220 ms exit; reduced motion: opacity only.
Task-owned source paths: new `server-update.ts`, `version-skew.ts` (+ tests), `composer-controls-view.ts`,
`requests.ts|contract`, `shell-details.ts|contract`, `connections.ts`, `modules/apple/T3Fleet.swift` hunks.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build, the stub server if approved.

## Progress

Implemented on `feat(example)/t3-code-server-update-banner` (base 9670b0723); verification: unverified.

- `version-skew.ts`: versionSkew.ts ported (resolveVersionMismatch, buildVersionMismatchDismissalKey,
  isVersionMismatchDismissed/dismissVersionMismatch persisted in `t3-code.json` under
  `shell.versionMismatchDismissals`, isServerUpdateFailureDismissed per attempt, serverUpdateGuidance,
  capability readers). `connections.ts` re-exports CLIENT_VERSION/compareSemver/versionMismatch from it.
- `server-update.ts`: ServerUpdateState and the server.ts helpers by name; the per-environment store is
  T3Fleet's job table (`T3OutdatedHosts`, new `mode: "connected"`): progress stream or legacy
  `server.updateServer`, desktop commit, wait for the descriptor to report the target (4 min).
  `updateEnvironment(target, deps)` is the single-flight entry (auto-balance adds its batch on it);
  `announceServerUpdates` toasts "<label> updated" once the connection reports the version.
  Settings › Connections Update / Update all use the same store; a running row shows its progress.
- `server-update-notices.ts` + `client-ops-server-update.ts` (`su:*` ops): offline notice moved from the
  request stack into `composerNotices` with the 2 s grace (never while an update runs) and
  "Disconnect server" after 20 s for a non-loopback environment; the server-version notice
  (idle/running/failed, tooltip, guidance, Update/Copy…/Retry, dismiss); the desktop-managed confirm
  (AppConfirm); the version-differ card in the details card and the 6 px warning dot on the toggle.
- Timers (X19 workaround): the snapshot names the reconnecting/unavailable episodes; root tasks
  `after(2000)` / `after(20000)` hand the elapsed episode back as snapshot arguments.
- Motion: a dismissed notice slides out 220 ms (translate 64/112 px + fade; reduced motion fades only)
  before its dismiss command runs (root `noticeDismiss` task).
- Live drive found that a current (1e2ecbd975) server closes a socket that does not name
  `orchestrationProtocol`; the connected-mode job now names protocol 2 (f14cc8052).

Not done / limits: success toast not observed in the live drive (5 s toast vs 11 s wait; unit-tested);
"Disconnect server" not shown live (a lane server must be loopback because remote HTTP is refused,
and loopback is the primary stand-in; unit-tested); keyboard focus into/out of the confirm dialog and
Enter/Escape (unverified, attended); the multi-machine banner and batch confirm (auto-balance);
`ComposerBannerStack.test.tsx`'s only case (clipped-description details popover) not ported: the clone's
NoticeRow has no details popover; oracle pixel pairs and trace-diff not run (oracle not built);
`waitForDesktopUpdateTarget` commit retries (3) are one commit in Swift.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | f14cc8052 | bun test 1881 pass / 0 fail (base 1829); strict tsc clean; contract build 2338 slots, 43 resources; cargo test -p t3-code-macos --lib 10 pass; AppKit fleet 9/0, transport 47/0; caps pass; five checks: see PR | Drive (after, 1280×840, stub proxy on 16221 → real server 16220): `notice-server-version` "Server update available" tip "server 0.0.45 → 0.0.46-nightly.20261004.1", "Update to stay in sync", Update; `details-version-mismatch` "Client 0.0.46-nightly.20261004.1 · server 0.0.45"; `thread-details-attention`; tap Update → "Updating server · Downloading…"; attempt 1 fails at installing → "Could not update server · Server update failed: The package could not be verified." + Retry + toast "Server update failed"; Retry → "Updating server · Restarting…" with no offline notice during the restart; after restart banner, card and dot gone; outage → "Studio is reconnecting" after the grace, `serverUpdate.unavailable` episode set, no Disconnect (loopback = primary). Shots `target/su-stub/shots/{before,after}`; the first after drive failed on the bare socket (fixed) | success toast and Disconnect live; keyboard (attended) |

## Next action

After `20261005-remote-scopes-and-update-commands` and the base tickets merge: settle decision U2
(stub server) with the user, then `prepare` and `implement`.
