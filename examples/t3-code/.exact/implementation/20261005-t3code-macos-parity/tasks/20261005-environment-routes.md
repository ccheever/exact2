---
name: 20261005-environment-routes
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
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

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`: `packages/client-runtime/src/connection/{routes,driver,supervisor,registry,onboarding}.ts`, `docs/user/remote-access.md`
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
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | `20261005-local-primary-environment`, `20261005-ssh-password-and-remote-open` and `20261005-desktop-shell-details` after this ticket | none | They share the `environmentKey` call sites and `T3Ssh.swift` | pending |
| scheduling preference | Either order with `20261005-remote-scopes-and-update-commands` | none | Both edit `connections.ts` and the pairing code in `T3Transport.swift` | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched. Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket for data modules | The route walk reuses the Swift transport | nonblocking (workaround: `T3Transport.swift`) | none |
| [X19](../issues/20261005-x19-data-source-timers.md) | Timers in data sources | 60 s check, 5 min cooldown, 2.5 s check run in Swift | nonblocking (workaround: native timers) | agent-only interval override for tests |

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

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `transport` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
