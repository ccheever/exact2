---
name: 20261005-environment-routes
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: feat(example)/t3-code-environment-routes
pr_url: https://github.com/ccheever/exact2/pull/148
verified_commit: null
---

# One environment, several routes

## Outcome

A saved environment keeps an ordered list of routes (LAN, Tailscale name, public URL, SSH, and addresses the server reports). The app
connects over the first route that answers as the same environment, moves back to a better route when one becomes reachable, and shows
the routes under the machine's row (count, "In use", drag reorder, remove, "Add route"). Pairing the same machine at a second address adds a
route instead of a duplicate row. GitHub-sharing trust follows the route set.

## Scope and exclusions

Included (reference commits `979ca66ced`, `745c225f92`; clone rows are missing):

1. **Route model (TS).** Port `packages/client-runtime/src/connection/routes.ts`: `connectionRoutes`, `entryWithRoutes`, `routeEntry`, `connectionRouteKind`,
   `insertRoute`, `upsertRoute`, `findRouteToSameAddress`, `connectionRouteLabel`, `connectionRouteAddress`, `mergeLearnedRoutes`, `isLearned`,
   `credentialConnectionId`, `routesAfterRemoving`, `sshTargetKey`, with `packages/shared/src/hostClassification.ts` (`isLocalLoopbackHost`,
   `isPrivateNetworkHost`, `isTailnetHost`). Rank: loopback 0, LAN 1, tailnet 2, public 3, SSH 4. A new route goes after routes of the same or a faster kind.
2. **Store.** The clone keys everything by `(origin, environmentId)`: `T3SavedEnvironments` (`modules/apple/T3Credentials.swift:68-108`, UserDefaults
   `t3.saved.environments`), the Keychain account (`T3Credentials.swift:56`), `environmentKey` (`settings-b-fleet.ts:24`, 27 call sites). Move to one entry per
   `environmentId` holding `routes[]` (id, kind, origin, optional SSH target, `learned`, credential owner). Existing entries migrate to one route each;
   Keychain items stay readable (a learned route reads its source route's credential, as `learned:<env>:<origin>@<connectionId>`).
3. **Walk and fallback (Swift).** Port `connectOverRoutes` (`connection/driver.ts:82-137`): check all routes in parallel (`checkRoute`, 2.5 s, unauthenticated
   descriptor), try silent routes last, report a transient error over a blocked one, stop the walk on an incompatible server, and end with
   `<label> did not answer on any saved route.` Port the supervisor's better-route check (`supervisor.ts:35-49`): every 60 s, on network change and on app
   activation, `preflight` (answers and accepts the credential, no socket) before replacing a live session, 5-minute cooldown per failed route. New file
   `T3Routes.swift`, beside the reconnect ladder `T3Reconnect` (`modules/apple/T3Protocol.swift:245`, used by `T3Transport.swift`).
4. **Learned routes.** `server.getConfig.directEndpoints` (`ServerDirectEndpoint {kind: lan|tailnet, httpBaseUrl}`, `packages/contracts/src/server.ts`, commit
   `745c225f92`) feeds `mergeLearnedRoutes` after each config; they show "· found automatically", cannot be removed, and borrow the source credential.
5. **Different machine.** Add route and the SSH route check `expectedEnvironmentId` before the one-time code is spent: "That address reaches <label>, a different
   machine. Add it as its own environment instead." (SSH: "That host reaches <label>, …"). Reference: `connection/onboarding.ts:119-125`, `apps/web/src/connection/platform.ts:208`.
6. **UI (Settings > Connections, `connections.ts`, `connections.contract`).** Route count control after the row status ("Routes" / "N routes", chevron rotates 150 ms,
   no rotation under reduced motion, `aria-expanded`), row menu item "Routes" / "Hide routes", the list (`aria-label` "Routes to <label>, preferred first"; grip `aria-label`
   "Reorder <label>, position N"; "In use"; remove `aria-label` "Remove <label> route", tooltip "Remove route", confirm "Remove <label> route?" + address; the last route is only
   removed with the machine), "Add route" button, dialog "Add a route to <label>" (text in `ConnectionsSettings.tsx`, search "Add a route to"), toasts "Route added" /
   "<label> now has another way to connect." / "<label> can now be reached over SSH <alias>.". Row subtitle: `via <route label>` for the route in use, else the first route's label (`EnvironmentRow.tsx`).
7. **GitHub sharing key.** Port `gitHubRoutingConnectionKey` (`githubRoutingPermissions.ts:27-45`): one route keeps the clone's current key (`routingKey`, `connections.ts:114`),
   several use the sorted set of non-learned route keys; adding or changing an address revokes trust, reordering does not.

Excluded: remote scopes, the re-pair decision and install-aware update commands (`20261005-remote-scopes-and-update-commands`), T3 Connect routes (the relay kind stays in the ported pure code for
test parity and is never created), the T3 Connect route offer, mobile screens, the embedded "This machine" primary (`20261005-local-primary-environment`), the SSH password prompt (`20261005-ssh-password-and-remote-open`).

## Context and guidance

Parent specification: [spec](../../spec.md). Reference at `1e2ecbd975`: `packages/client-runtime/src/connection/{routes,driver,supervisor,registry,onboarding}.ts`, `docs/user/remote-access.md`
("Reach one machine several ways"), `apps/server/src/environment/DirectEndpoints.ts` (server side, nothing to port).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `trace-diff.mjs`, `trace-proxy.mjs`, `electron-oracle.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (actions read a snapshot; persistence awaited on macOS; late old replies), layout-and-interaction (bounded list; `each` keys are durable ids),
accessibility (icon-only buttons need `aria-label`; keyboard access; reduced motion), design (all states), testing-and-debugging. App-local Swift modules, native timers and Keychain use are **unknown in the
library**; the clone's runtime evidence on the pinned main is the basis. Consumer framework revision: the pin chosen by `20261005-clone-on-exact2-main`.
Port changes to record in file headers: `allowInsecure` is always true (a native app is not an HTTPS page); `Effect` services become plain functions with a `now` argument; the SSH route's origin is the tunnel's
loopback address, so its kind comes from the stored `kind`, not from the address.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | `20261005-local-primary-environment`, `20261005-ssh-password-and-remote-open` and `20261005-desktop-shell-details` after this ticket | none | They share the `environmentKey` call sites and `T3Ssh.swift` | pending |
| scheduling preference | Either order with `20261005-remote-scopes-and-update-commands` | none | Both edit `connections.ts` and the pairing code in `T3Transport.swift` | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched. Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket for data modules | The route walk reuses the Swift transport | nonblocking (workaround: `T3Transport.swift`) | none |
| [X19](../../issues/20261005-x19-data-source-timers.md) | Timers in data sources | 60 s check, 5 min cooldown, 2.5 s check run in Swift | nonblocking (workaround: native timers) | agent-only interval override for tests |

## Implementation notes

- TS owns the model and the UI projection; Swift receives each environment's ordered routes and reports `activeRouteId` in the transport status. Keep `client.ts` unchanged except one call through the area seam.
- Reordering reuses the clone's list drag used for queued messages and Project order; add a keyboard path (focus the grip, Space to lift, arrows, Space to drop, Escape to cancel) because the reference uses dnd-kit's keyboard sensor.
- Never send a credential to a route before its descriptor matches the saved `environmentId`.
- Agent-only seam `T3_ROUTE_CHECK_INTERVAL_MS` (like `T3_SSH_COMMAND` in `T3Ssh.swift:11-12`) shortens the 60 s check.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported pure tests | — | `bun test` on `routes.test.ts` (13 cases, e.g. "classifies direct routes by address", "keeps GitHub routing trust when a route is learned"), `hostClassification.test.ts`, `registry.test.ts` route cases | Original names, all pass | macOS | test log |
| No duplicate row | Fixture backend on 16xxx; a second proxy origin on 16yyy to the same server | Pair at the first origin, then "Add route" with the second link | One row, "2 routes", subtitle `via …`; Keychain has no second environment | macOS 1280×840 and 840×620 | `--json` tree/state, screenshots |
| Different machine refused | Two fixture backends | "Add route" with the other backend's link | Reference message; code not spent (backend pairing list unchanged) | macOS | transcript, backend state |
| Fallback and failback | Kill the proxy for the first route (recorded PID); `T3_ROUTE_CHECK_INTERVAL_MS=1000` | Watch status; restart the proxy | Moves to route 2 with no row flicker; returns to route 1 within the interval; cooldown after a refused credential | macOS | `state` + log timeline |
| Learned routes | Lane server bound to the Mac's LAN address (isolated HOME) | Connect over the loopback route | A LAN route appears "found automatically", is not removable, survives reorder | macOS | state, screenshot |
| Credential discipline | Trace proxy on both routes | `target/t3-ui-parity/trace-diff.mjs` on a route walk | No `Authorization` header or token reaches a route before its descriptor matched; no credential in the trace | macOS | ndjson |
| Keyboard, Escape, focus | Environment with three routes | Tab to the route count; open the list; Space-lift a route with the keyboard and move it; open "Remove route" and "Add a route" dialogs | Count control and grips reachable in order; the reorder commits on drop and cancels on Escape; the remove confirm and the Add route dialog close on Escape and return focus to the control that opened them; visible focus ring | macOS | `tree --ax`, screenshots |
| Reduced motion and states | `prefer prefers-reduced-motion reduce` | Expand and collapse | Chevron changes without rotation; with motion on it rotates over 150 ms (oracle value); loading, empty (one route), error ("did not answer"), disabled (environment off) and "In use" states match the oracle | macOS | `--json` transcripts, png pairs vs `target/t3-ui-parity/electron-oracle.mjs` |
| Visual pairs | Same saved environment with two routes in the oracle and the clone | Collapsed and expanded | Pixel pairs within the matrix tolerance, light and dark, both sizes; every moved cell fixed or declared in `EXACT2-GAPS.md` with an issue link | macOS | png pairs |
| Real drag (attended session) | Lane build, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Drag a route row with the pointer | Order persists across relaunch | macOS | recording |

Task-owned source paths: `connection-routes.ts`, `host-classification.ts` (+ tests), `connections.ts`, `connections.contract`, `settings-b-fleet.ts`, `modules/apple/{T3Credentials,T3Transport,T3Fleet,T3Routes}.swift`, `macos/tests/transport`.
Required environment: Xcode 27.0, pinned Bun, reference oracle runtime, lane ports 16000–16999.

## Progress

2026-10-06, implemented (verification: unverified). Built from the parallel-wave base `d78ac86ff`; the
hot-file-split, clone-on-main and desktop-oracle prerequisites were not merged first (plan.md
"Parallel implementation").

- Route model: `connection-routes.ts` (routes.ts, gitHubRoutingConnectionKey, registry route edits) and
  `host-classification.ts`, with `connection-routes.test.ts` (all 13 routes.test.ts cases and the registry
  route cases, original names) and `host-classification.test.ts`.
- Store: `T3SavedEnvironments` keeps one entry per environment id with `routes[]` (`id, origin, kind,
  learned, credential, authorization?, ssh?`); entries saved per origin migrate on read. Keychain items stay
  keyed by origin; a learned route's `credential` names the owner origin. The entry's `origin` is its home
  (the first address, kept while that route exists), so `environmentKey` and the fleet keys did not change;
  the focused connection, whose origin is now the route in use, is matched by environment id.
- Walk and fallback: `T3Routes.swift` (connectOverRoutes, checkRoute 2.5 s, preflight, 60 s / network /
  activation better-route check, 5 min cooldown, `T3_ROUTE_CHECK_INTERVAL_MS`). `T3Transport.swift` hooks:
  connect loads the routes, start walks them, a failed route moves on, the socket's open records
  `activeRouteId`, `setRoutes`, `pairEnvironment` `expectedEnvironmentId`, status `activeRouteId` /
  `homeOrigin`.
- Learned routes: `learnRoutes` (connection-routes-ops.ts) runs from `fleet.sync` for the focus and each
  connected background environment after `server.getConfig.directEndpoints`.
- Different machine: Add route and the SSH route pass `expectedEnvironmentId`; the transport refuses before
  `/oauth/token` with the reference messages.
- UI: route count control, Routes / Hide routes menu item, the routes list (In use, found automatically,
  remove with "Remove <label> route?", Add route), "Add a route to <label>" dialog, toasts, `via <route>`.
  Reorder uses the host's list reorder (`reorderdrop` / `reorderFor`), which supplies pointer drag and the
  keyboard (Space, arrows, Escape).
- GitHub sharing: one route keeps the clone's key; several use the sorted non-learned route keys.

Remaining differences and limits:
- A learned plain-HTTP LAN route is listed ("found automatically") but this client never sends a credential
  over plain HTTP to a non-loopback host (`T3Endpoint`), so the walk counts it silent. The reference
  desktop connects over it.
- Switching to a better route replaces the session: the row reads "Reconnecting" for the moment of the
  switch (the reference keeps its supervisor state "connected" through the swap).
- Timers are native (#124); the walk reuses the Swift transport (#126).
- Not run: visual pairs against the oracle, reduced-motion pair, keyboard reorder and Escape/focus return
  (`tree --ax`), real pointer drag and relaunch persistence (attended), credential-discipline trace with the
  trace proxy (covered by the transport test `testTheWalkSkipsAnotherMachineAtASavedAddressAndNeverSendsItACredential`
  only), SSH route add on a real host.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `2ec7b58e1` | bun test 1238 pass / 0 fail (base 1200); strict tsc clean; contract build 2164 slots, 42 resources; transport AppKit 43/0 (RouteTests 7), fleet 8/0, r10-connect 5/0, ssh 4/0 (1 skipped), r8-pointer 3/0; caps within budget; `cargo test -p t3-code-macos --lib` 10/0; five checks: build, test, clippy -D warnings, fmt --check, boot all pass; macOS bundle builds | unit and AppKit logs (not committed) | live drive partial (below) |
| live drive | `4d31b59d5` bundle | Lane servers on 16120 (bound 0.0.0.0, label "Route Lab") and 16122 ("Studio Lab"), a TCP proxy 16121→16120, `T3_ROUTE_CHECK_INTERVAL_MS=1000`, one agent session: paired through the proxy, Settings › Connections, `environment-routes-toggle` → `environment-route-add` → host `http://127.0.0.1:16120` + code → Add route. Tree after: one row, `environment-status-…` "via This device · Connected · 0.0.45", the count "4 routes" (proxy, direct and the server's reported LAN addresses, learned), `environment-route-in-use-1` "In use", routes 3 and 4 without a remove button, dialog title "Add a route to Route Lab". The dialog stayed open after the success (`connectionOp` did not treat `environment-route-add` as a pairing; fixed in `2ec7b58e1`, contract build only). The drive script's wait on that close timed out, so different-machine, fallback/failback and remove-confirm steps did not run live. The first attempt's script skipped pairing (no app code reached). Per the one-drive rule no third drive was run. | `01-add-route-dialog.png` (pairing code redacted) | live: different machine, fallback/failback, remove confirm unverified on macOS (covered by RouteTests and connections-routes.test.ts) |

## Next action

`prepare` after the three merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `transport` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
