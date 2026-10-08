---
name: 20261005-this-machine-network-access
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-this-machine-network-access
pr_url: https://github.com/ccheever/exact2/pull/248
verified_commit: null
---

# Network access, Tailscale HTTPS, authorized clients and pairing links for "This machine"

## Outcome

The user can open this Mac's embedded server to other devices: switch Network access on or off, see the
addresses it is reachable at, set up or disable Tailscale HTTPS, create pairing links with chosen permissions, share them
as a link, code or QR, see the authorized clients and pairing links live, and revoke one or all others. Wording, rows, dialogs and
states match the reference desktop app.

## Scope and exclusions

Included (clone rows missing; the clone shows only "Administrative access", `connections.contract:122`):

1. **Exposure state (TS + Swift).** Port the pure parts of `apps/desktop/src/backend/DesktopServerExposure.ts`: `resolveLanAdvertisedHost`
   (first non-internal IPv4 that is not 127.x, 169.254.x or Tailscale), `resolveDesktopServerExposure` (local-only binds 127.0.0.1; network binds 0.0.0.0),
   `resolveRuntimeState` (network mode with no LAN and no Tailscale IPv4 is *unavailable*), `requiresBackendRelaunch` (port, bind host or URL changed),
   `resolveDesktopCoreAdvertisedEndpoints` (ids `desktop-loopback:<port>` "This machine", `desktop-lan:<url>` "Local network" default, `manual:<url>` "Custom HTTPS"/"Custom endpoint"),
   with `DesktopServerExposure.test.ts` (10 cases, e.g. "returns a typed error when network access is explicitly unavailable", "does not spawn the tailscale CLI while server exposure is local-only").
   `setMode` rejects with `DesktopServerExposureNoNetworkAddressError` ("No reachable network address is available for desktop network access on port N."); at launch an
   unavailable request falls back to local-only without losing the preference. Persist `serverExposureMode` (`local-only` | `network-accessible`), `tailscaleServeEnabled`,
   `tailscaleServePort` (default 443) in `t3-code.json`. A change restarts the embedded server with the new envelope (`host`, `tailscaleServe*`) via the single
   `applyLocalSetting` seam from `20261005-local-primary-environment`, which relaunches the app as T3 Code does (decision U4; a restart-in-place stopgap only while issue X45 is open).
2. **Interfaces and Tailscale (Swift).** Read interfaces with `getifaddrs` into the shape of `DesktopNetworkInterfaces` (`name -> [{address, family, internal}]`; 2 tests ported). Run
   `tailscale status --json` (1.5 s limit, cached 60 s, only when network access or Tailscale Serve is on, because the spawn can raise macOS's "Other apps" prompt for the
   App Store build) with the login-shell `PATH`. Port `packages/tailscale/src/tailscale.ts` pure functions (`isTailscaleIpv4Address`, `parseTailscaleMagicDnsName`,
   `buildTailscaleHttpsBaseUrl`, `DEFAULT_TAILSCALE_SERVE_PORT`) and `probeTailscaleHttpsEndpoint` (2xx on `/.well-known/t3/environment`, 2.5 s) with `tailscale.test.ts`, and
   `tailscaleEndpointProvider.ts` (`resolveTailscaleAdvertisedEndpoints`: "Tailscale IP" `tailscale-ip:http://<ip>:<port>`; "Tailscale HTTPS" `tailscale-magicdns:<url>`, available only when
   serve is enabled and the probe answers, else "Setup required") with its 4 tests. `tailscale serve` itself runs inside the server, not in the app. Port `createAdvertisedEndpoint` and helpers
   (`packages/shared/src/advertisedEndpoint.ts`).
3. **Settings > Connections rows** (`ConnectionsSettings.tsx:2968-3000,3311-3422,3559-3607,3716-3825`): "Network access" Switch (`aria-label` "Enable network access"; description "Limited to this machine." /
   "Loading…" / "Reachable at <url>" with a "+N" / "Hide" toggle, `aria-expanded`, or the fallbacks "Exposed on all interfaces." and "Exposed on all interfaces. Pairing links use <host>."), confirm dialog
   "Enable network access?" ("Let your other devices connect to T3 Code over the network. Pair devices to give them access. T3 Code will restart.") / "Disable network access?" ("Devices connected over your local
   network will disconnect. Existing tunnels, such as T3 Connect or Tailscale HTTPS, keep working. T3 Code will restart.") with Cancel and "Restart and enable" / "Restart and disable" / "Restarting…";
   expanded endpoint rows ("endpoint-rail": label, URL with tooltip, Default tag, "Set as default", "Setup required" badge, "Setup" / "Disable"); "Tailscale HTTPS" Switch row (`aria-label` "Enable Tailscale HTTPS";
   "Start Tailscale to set up HTTPS access through MagicDNS." / "Use Tailscale Serve to expose this backend through a MagicDNS HTTPS URL." / the URL); setup dialog "Set up Tailscale HTTPS?" with "HTTPS port" (1-65535, "Enter a port
   from 1 to 65535.") and a read-only "HTTPS endpoint" preview ("Pending MagicDNS endpoint"); disable dialog "Disable Tailscale HTTPS?" ("T3 Code will restart the local backend without Tailscale Serve.") with a destructive
   "Restart and disable". Errors appear as red status text and toasts "Could not update network access", "Could not set up Tailscale HTTPS", "Could not disable Tailscale HTTPS". The T3 Connect and WSL rows are not drawn.
   The chosen default endpoint is kept as `defaultAdvertisedEndpointKey` in `t3-code.json` (keys from `endpointDefaultPreferenceKey`).
4. **Authorized clients** (shown when network access is on or Tailscale HTTPS is available, `ConnectionsSettings.tsx:2674-2675`; a folded section with the summary "N clients · M pairing links").
   Stream `subscribeAuthAccess` (snapshot, `pairingLinkUpserted`, `pairingLinkRemoved`, `clientUpserted`, `clientRemoved`, with a revision) reduced by the ported `applyAuthAccessStreamEvent`
   (`packages/client-runtime/src/state/auth.ts:27-66`, 2 tests). HTTP: `POST /api/auth/pairing-token`, `GET /api/auth/pairing-links`, `POST /api/auth/pairing-links/revoke`, `GET /api/auth/clients`,
   `POST /api/auth/clients/revoke`, `POST /api/auth/clients/revoke-others` (`packages/contracts/src/environmentHttp.ts:436-510`) through the clone's REST access (`client.ts:1023`). Header: "Revoke others"
   (disabled when only this device is listed; "Revoking…") and "Create link". Dialog "Create pairing link": optional label (placeholder "e.g. Living room iPad"), presets "Read only" / "Standard", eight permission rows
   with the reference titles (View environment, Operate tasks, Use terminals, Write reviews, View access, Manage access, View relay, Manage relay), "Select at least one permission." and the warning "This client can create or
   revoke access for other devices." for `access:write`, "Creating…". Pairing row: label or "Pairing link", dot, "expires in …" (ticks every second, vanishes at expiry), "N scopes" popover "Granted scopes", **Share** (QR 168 pt via `qrCells`,
   endpoint radiogroup "Reach this machine via", "Copy link", "Copy code only"), reveal dialog with a Textarea and a 132 pt QR when the pasteboard write fails, Revoke ("Revoking…"). A link made elsewhere says "Create a new link to
   share from this client." Client row: dot (live = connected or current), label or "<os> · <browser>", device bits, scopes popover, "This device" tag, Revoke. Empty: "No pairing links or client sessions." Toasts: "Pairing URL copied",
   "Hosted app link copied", "Pairing code copied" (+ descriptions), "Could not copy pairing URL", "Could not create pairing URL", "Could not revoke pairing link", "Could not revoke client access", "Revoked 1 other client" / "Revoked N clients".
5. **Pairing URLs.** Port `pairingUrls.ts` (`resolveDesktopPairingUrl`: `<endpoint>/pair#token=<credential>`; `resolveHostedPairingUrl` for HTTPS), `hostedPairing.ts` `buildHostedPairingUrl`
   (`https://app.t3.codes/pair?host=<endpoint>[&label=]#token=`), `isQrShareableEndpoint`, `selectQrEndpointOption` (`ConnectionsSettings.logic.ts`, 6 cases) and `selectPairingEndpoint`, `endpointShareHint`
   ("Opens the hosted app, no install needed" / "Devices on the same network" / "Devices on your private network" / "Reachable from anywhere" / "Clients on this machine"). Loopback endpoints never get a QR.
   Open decision: the default for HTTPS endpoints is a link on the T3-hosted web app; keep it as in the reference, or drop it (the clone's own `parsePairing`, `protocol.ts:94`, already reads such links).

Excluded: T3 Connect rows and the Manage/View relay toggles' effect (rows are shown, as in the reference), WSL, mobile, the Local environment switch (`20261005-local-primary-environment`).

## Context and guidance

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`; the original desktop shares the same `~/.t3` in production, so pairing links and clients created here are visible in the original and the reverse (accepted). Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `trace-diff.mjs`, `trace-proxy.mjs`, `electron-oracle.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`). Library revision:
`20261005-platforms-v3`. Selected topics: state-and-data (actions read a snapshot; mutation feedback), layout-and-interaction (settings structure), design and accessibility (all states, dialog focus, icon-button labels, reduced motion),
testing-and-debugging. Native sockets, `getifaddrs`, process spawn and pasteboard are **unknown in the library**.
Decided: relaunch the app as T3 Code does (U4). Open decisions: the hosted pairing link (U8); Tailscale for verification (U9): a tailnet the user provides, or the stub provider only with live Tailscale marked unverified.
A link created with Read only cannot be redeemed by this app's own pairing, which asks for the five standard scopes (`20261005-remote-scopes-and-update-commands`); pair test clients with Standard links.
Do not grow `app.contract` or `client.ts`; put the section in a new `.contract` file and state in TS.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | [#147](https://github.com/ccheever/exact2/pull/147) | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | merged |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | [#99](https://github.com/ccheever/exact2/pull/99) | Merged | the clone is on exact2 in `feat(example)/t3-code` (this PR's base) |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | none | Merged | blocked: not built (user decision 2026-10-06); the Trace row and the oracle comparisons stay blocked |
| merged task PR | [20261005-local-primary-environment](closed/20261005-local-primary-environment.md) | [#237](https://github.com/ccheever/exact2/pull/237) | Merged | merged (`applyLocalSetting` is the shared seam) |
| recorded decision | U4 decided (relaunch); U8 (hosted link), U9 (Tailscale verification) | none | U8 and U9 answered at `prepare` | U4: user 2026-10-05; U8 and U9 decided 2026-10-08 (user: match the original): the hosted link is the reference's, checked line by line, and the implementation matches the reference (the probe deadline, the SWR snapshot cadence, the port rule and stepping were fixed in [provisional-decisions-parity](20261008-provisional-decisions-parity.md)); live Tailscale needs a tailnet the user provides |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | `subscribeAuthAccess` stream through the Swift transport | existing transport | nonblocking | none |
| [X19](../issues/20261005-x19-data-source-timers.md) | Per-second expiry tick, 60 s status cache | native/Contract tasks | nonblocking | follow the clone's time-as-argument pattern |
| [X4](../issues/20261005-x04-bundle-helper-executables.md) | Server runtime in the bundle | upstream ticket; #103 closed by main #215 (`host.macos.resources`), recorded in [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md) | inherited | none here |
| [X45](../issues/20261005-x45-app-relaunch.md) | App relaunch after an exposure change | X45 (unconfirmed) | blocking for the relaunch rows if X45 is confirmed missing (stopgap meanwhile: restart the server in place and reconnect; a visible difference) | use the relaunch when X45 is adopted Update 2026-10-07 (adopt-main-fixes-shell): #122 was closed after main #170, which only moves `reload()`'s log to stderr; exact2 still has no process relaunch, so the relaunch rows stay blocked. |

## Implementation notes

- The endpoint list and exposure state are computed in TS from native facts (interfaces, Tailscale status); the native module only reads interfaces, runs `tailscale status` and probes.
- Show the Version, Network access and Tailscale rows only when the local environment is on and the session has `access:write` (`ConnectionsSettings.tsx:3469-3529`); with it off the section keeps its header and the Local environment row only. The "Only this machine can connect…" variant (`:3394-3422`) is for non-desktop clients and is not drawn.
- Keep `access:write` assumptions: the desktop session has all scopes (`ConnectionsSettings.tsx:1912-1916`).
- Never write a pairing credential to logs, traces or `t3-code.json`; keep created credentials in memory by link id.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test`: exposure (10), tailscale provider (4), tailscale package, `applyAuthAccessStreamEvent` (2), pairing URLs (2), QR option logic (6); Swift: network interfaces (2) | Original names pass | macOS | logs |
| Network access on | Lane home, lane port, fixture provider | Switch on, confirm | Dialog text; server restarts bound to `0.0.0.0`; `lsof` shows the listener; rows: This machine, Local network (default) | macOS 1280×840 and 840×620, light and dark | state, `lsof`, png pairs vs oracle |
| No network address | Wi-Fi and other interfaces down (attended session) or interface stub | Switch on | Error text and toast; mode stays local-only | macOS | transcript |
| Tailscale | Stub provider or the user's tailnet | Setup with port 8443; then Disable | Preview URL has `:8443`; endpoint flips between "Setup required" and available; `tailscale serve status` shows the proxy; removed on disable and at quit | macOS | state, CLI output |
| Create and share | Network on | Create link with Read only, then Standard | Row appears at once in a second client's view and here; expiry ticks; copy puts the URL on the pasteboard; QR scans to the same URL (decode the png) | macOS | `pbpaste`, decoded QR |
| Pair another client | A second lane app (its own `T3_LOCAL_HOME`) or the reference web client; a Standard link | Open the link | Client row appears with label, scopes, "This device" only on the owner; connected dot | macOS | state, screenshot |
| Revoke | Two clients | Revoke one; Revoke others | Row disappears; the revoked client's socket closes; toast text | macOS | state, server effect |
| Trace | T0-style scenario on oracle and clone | `target/t3-ui-parity/trace-diff.mjs` | Same `/api/auth/*` calls and `subscribeAuthAccess`; no credential in the trace | macOS | ndjson |
| States | Each of: loading ("Loading…"), empty ("No pairing links or client sessions."), error (red status text and toast), disabled (switch while `Restarting…`), hover (scope popover after 250 ms, tooltips) | Drive each with the fixture or a forced failure | Same text, colours and sizes as the oracle (pairs) | macOS 1280×840 and 840×620, light and dark | png pairs, `layout` |
| Dialog keyboard and Escape | Five dialogs: Enable/Disable network access, Set up Tailscale HTTPS, Disable Tailscale HTTPS, Create pairing link, the clipboard-failure reveal dialog | Tab, Shift-Tab, Enter, Escape in each; port field with letters, 0 and 70000; Create link with no permission ticked | Initial focus and Tab order equal the oracle's (recorded from it with `tree --ax`); Escape closes and returns focus to the control that opened the dialog; no dismissal while "Restarting…" or "Creating…"; the port error text and the disabled Enable button; "Select at least one permission." with Create link disabled | macOS | `tree --ax` diff, screenshots |
| Reduced motion | `prefer prefers-reduced-motion reduce` | Open the fold, the endpoint list ("+N" toggle) and the Share panel; open and close dialogs | No height or scale animation (frame sheets from `screenshot … over 300 every 30` compared with the oracle's recorded durations); content and final state identical | macOS | contact sheets |
| Real input (attended session) | Lane build, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Right-click, real hover tooltips, QR scan from a phone | Matches the oracle | macOS | recording |

Task-owned source paths: `server-exposure.ts`, `tailscale.ts`, `advertised-endpoint.ts`, `pairing-urls.ts`, `connections-network.contract`, `connections.ts` (+ tests), `modules/apple/T3LocalNetwork.swift`, `macos/tests/local-backend` additions, `t3-code.json` schema.
Required environment: Xcode 27.0, pinned Bun, the staged runtime, lane ports 16000-16999; optional tailnet.

## Progress

Implemented on `feat(example)/t3-code-this-machine-network-access` (2026-10-08) from `feat(example)/t3-code` `da4f4512f`,
with `d82fb6a47` (#244, records only) merged in. Reference `1e2ecbd975`. Decisions (user, 2026-10-08: match the
original; [provisional-decisions-parity](20261008-provisional-decisions-parity.md)): U8, HTTPS endpoints pair through the hosted app on `app.t3.codes` exactly as the reference
(conditions, URL, strings; it needs no T3 Connect, so nothing here is blocked by X38); U9, Tailscale is verified with
the stub provider in lanes and was checked against the reference line by line; live Tailscale needs a tailnet the
user provides (a verification gap, not a function difference). U4 (relaunch) is decided; exact2
has no process relaunch (#122 / X45), so every change goes through `applyLocalSetting`'s restart-in-place stopgap.

| Scope item | Built | Where |
| --- | --- | --- |
| 1. Exposure state | `DesktopServerExposure` ported (`resolveLanAdvertisedHost`, `resolveDesktopServerExposure`, `resolveRuntimeState`, `requiresBackendRelaunch`, `resolveDesktopCoreAdvertisedEndpoints`, the three typed errors, `configureFromSettings`/`setMode`/`setTailscaleServeEnabled`/`getAdvertisedEndpoints`) with its 10 tests by name; `serverExposureMode`, `tailscaleServeEnabled`, `tailscaleServePort` and `defaultAdvertisedEndpointKey` are top-level keys of `t3-code.json`. At launch the native side binds by the same rule (`T3LocalExposure.atLaunch`: a network request with no LAN and no Tailscale IPv4 falls back to loopback, keeping the preference). A change restarts the embedded server in place with the new envelope (`applyLocalSetting` → `localBackendRestart`), reconnects this machine, and on a failed restart puts the mode back. | `server-exposure.ts`, `this-machine.ts`, `T3LocalBackend.swift` (`restart`, `startConfig(exposure:)`), `T3LocalNetwork.swift` |
| 2. Interfaces and Tailscale | `getifaddrs` in the shape of `os.networkInterfaces()` (2 tests by name); `tailscale status --json` from the login shell's PATH with the 1.5 s limit, stderr diagnostics that never quote the CLI, cached 60 s and read only while network access or Serve is on; the HTTPS probe (2xx on `/.well-known/t3/environment`, 2.5 s). Facts answer at once from the caches and refresh in the background, announcing `t3.local`, so a read never waits on the CLI. The tailscale package's pure functions, `resolveTailscaleAdvertisedEndpoints` (4 tests) and `createAdvertisedEndpoint` ported. `tailscale serve` runs in the server. | `T3LocalNetwork.swift`, `tailscale.ts`, `advertised-endpoint.ts`, `macos/tests/local-backend/network.swift` |
| 3. Connections rows | Network access (switch "Enable network access"; "Limited to this machine." / "Loading…" / "Reachable at <url>" with "+N"/"Hide" and `aria-expanded` / the three fallbacks; red status text), the endpoint rail (label, URL with tooltip, Default tag and rail, "Set as default", "Setup required", Setup/Disable), Tailscale HTTPS (switch "Enable Tailscale HTTPS" and the three descriptions), the dialogs "Enable/Disable network access?", "Set up Tailscale HTTPS?" (HTTPS port, "Enter a port from 1 to 65535.", the endpoint preview, "Pending MagicDNS endpoint") and "Disable Tailscale HTTPS?", "Restarting…", the three error toasts. No T3 Connect or WSL rows. | `connections-network.ts`, `connections-network.contract`, `connections-network-dialogs.contract`, `connections.contract`, `app-settings.contract` |
| 4. Authorized clients | The fold ("N clients · M pairing links", Revoke others / "Revoking…", Create link); the stream `subscribeAuthAccess` (focused or fleet primary, while the page is open) reduced by `applyAuthAccessStreamEvent` (2 tests by name); the six `/api/auth/*` calls through `localAccess` with the embedded server's bearer; Create pairing link (label, Read only / Standard, eight permissions, the error and the access:write warning, "Creating…"); pairing rows (dot with "Link created at", "Expires in …" each second and gone at expiry, "N scopes" with "Granted scopes" after 250 ms, Share: "Reach this machine via", the URL, Copy link, Copy code only, the 168 pt QR or the loopback note; Copy code; Revoke / "Revoking…"; "Create a new link to share from this client."); client rows (live dot and "Connected for …", label or "<os> · <browser>", device bits, scopes, "This device", Revoke); the reveal dialog; every toast. Credentials stay in memory by link id. | `auth-access.ts`, `connections-network.ts`, the two contracts, `keep-alive.ts`, `T3Module+Local.swift` |
| 5. Pairing URLs | `resolveDesktopPairingUrl`, `resolveHostedPairingUrl`/`buildHostedPairingUrl` (U8), `isQrShareableEndpoint`, `selectQrEndpointOption` (6 tests by name with the 2 pairing URL tests), `selectPairingEndpoint`, `endpointDefaultPreferenceKey`, `endpointShareHint`; this app's own pairing reads both link forms back. | `pairing-urls.ts` |

`app.contract` stays at 1,500 lines (two existing lines widened: the Connections page takes the clock, the provider
tick also ticks Connections while Authorized clients show); `client.ts` is unchanged.

### Acceptance results

Live rows ran as one agent drive of the branch build (run 4, 2026-10-08 18:42–18:45 UTC; lane home `target/lane`,
port 16101, `tools/network-lane/tailscale` first on PATH, `tools/network-lane/pair-client.mjs` as the second
client). The Mac's screen was locked for the whole session (`CGSSessionScreenIsLocked`), so agent screenshots and
pixel samples were empty (all transparent) and real input could not run: the drive read the view tree, the focus
and the server. Record: `this-machine-network-access/drive-a-run4.txt` on `t3-code-evidence`.

| Criterion | Result | Proof |
| --- | --- | --- |
| Ported tests | Pass with the original names: exposure 10, Tailscale provider 4, tailscale package 5 (TS) and its process half 5 (Swift), `applyAuthAccessStreamEvent` 2, pairing URLs 2, QR logic 6, network interfaces 2 (Swift) | `bun test`; `macos/tests/local-backend` 61; `tests.txt` |
| Network access on | Pass for state and effect: the dialog's words and "Restart and enable", "Restarting…"; server pid 73949 (127.0.0.1) → 77112 (0.0.0.0); `lsof` `t3 78817 *:16101`; rows This machine, Local network (Default), Tailscale IP; "Reachable at http://192.168.1.225:16101/" "+2"/"Hide"; Set as default moved the rail and the summary. png pairs **deferred to the real-input batch — screen locked (user away)** | drive A run 4 |
| No network address | Pass with the interface stub (error text, toast "Could not update network access", local-only kept, dialog closed). Live with every interface down **deferred to the real-input batch — screen locked (user away)** (attended: the user's Wi-Fi off) | `connections-network.test.ts`, `server-exposure.test.ts` |
| Tailscale | Pass with the stub (U9): preview `…:8443`; `abc`, `0`, `70000` show "Enter a port from 1 to 65535."; restart with Serve 8443 (pid 78817); the server ran `serve --bg --https=8443 http://127.0.0.1:16101`, `serve status` showed the proxy, `serve --https=8443 off` at quit; the endpoint stays "Setup required" (the stub's name never answers the probe); the app read `tailscale status` once a minute. **Blocked by U9**: the HTTPS endpoint turning available and Disable live (unit-tested) | drive A run 4; stub call log |
| Create and share | Pass for state and pasteboard: Read only → 1 scope, Standard → 5; expiry ticks 4m 52s → 4m 51s and the row goes at expiry (agent clock +301 s); Copy link put the pairing URL on the pasteboard ("Pairing URL copied"). Second client's view: the second client is a script (no view); here the row appeared at once from the stream. QR decode **deferred to the real-input batch — screen locked (user away)** | drive A run 4 |
| Pair another client | Pass: the second client redeemed the link (HTTP 200, the five standard scopes, socket open); "Lane phone" listed, "This device" only on T3 Code Desktop; live dot | drive A run 4 |
| Revoke | Pass: Revoke removed the row and the client's session turned unauthenticated with a 401 for a new websocket ticket; Revoke others: "Revoked 1 other client", same effect. The reference server checks a session when a socket opens and on each request (`apps/server/src/auth/SessionStore.ts` verify), so the open socket is not closed by the revoke itself; this corrects the ticket's expected result | drive A run 4; client logs |
| Trace | **Blocked**: user decision 2026-10-06 (oracle and trace tools not built) | — |
| States | Pass in the tree and unit tests: "Loading…" (unit; the native read answers at once), empty (unit; the desktop's own session always lists), error (unit + toast), "Restarting…" with the switch held, "Granted scopes" on hover, tooltips. png pairs **deferred to the real-input batch — screen locked (user away)**; oracle comparison blocked by the user decision of 2026-10-06 | drive A run 4; `connections-network.test.ts` |
| Dialog keyboard and Escape | Pass under agent keys: Enable network access opens on Cancel; Tab → Confirm → Cancel; Escape closes and focuses the switch; Set up Tailscale opens on the port field, Create pairing link on the label; "Select at least one permission." with Create link disabled. Real keys **deferred to the real-input batch — screen locked (user away)**; `tree --ax` vs oracle blocked by the user decision of 2026-10-06 | drive A run 4 |
| Reduced motion | Contact sheets **deferred to the real-input batch — screen locked (user away)** (agent pictures are empty while locked). The dialogs use the reference's 200 ms fade and 98 % scale (no reduced-motion variant in `dialog-styles.ts`); the spinner is still under reduced motion | — |
| Real input | **deferred to the real-input batch — screen locked (user away)**; the phone QR scan also needs the user's phone | — |

2026-10-08 (real-input batch, records PR): pairs made (base 05043629d); real keys, hover, QR decode, phone scan and the no-network row pass; the scopes popover is clipped (clone bug). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `f34f010b6` (build `e56f11ba`) | Drive A runs 1–3 found three drive mistakes and no app fault: `tap Copy link` unquoted; the agent clock's fixed epoch made the expiry read 279 d (fixed with `--epoch` now); a link left from the previous run made `Revoke` ambiguous (the drive now revokes other clients first and waits out the 5-minute links). Run 3 showed the revoked client's socket stays open until it reconnects (the reference server's rule); the client now checks its session | lane logs (not committed) | screen locked |
| 2 | `f34f010b6` | Drive A run 4: every functional row passes; checks below | `drive-a-run4.txt`, `tests.txt` | real-input batch (png pairs, QR decode, motion, real input); U9; #122 / X45; oracle not built |
| 3 | `3e4ff1431` (build after it) | A failed restart restores the mode read after the port is known; two unused icon branches removed. Re-run: `bun test` 2574 pass / 1 skip / 0 fail, strict `tsc`, `contract build` (2655 slots), `cargo test -p t3-code-macos --lib` 11, caps | PR #248 | same |

Final checks on `034c223f3` (the implementation `f34f010b6`, #244 merged, the lane tools); the clone checks again on `3e4ff1431`:
- `bun test examples/t3-code`: 2575 tests in 209 files; 2574 pass, 1 skip, 0 fail.
- Strict `tsc`: clean. `contract build`: 2655 slots, 46 resources, 2711 actions, 59828 nodes.
- `cargo test -p t3-code-macos --lib`: 11 passed.
- AppKit binaries (README recipe): local-backend 61 (LocalNetworkTests 10, new), transport 56, fleet 9, composer 46, menus 45; 0 failures.
- caps: within cap (`app.contract` 1500 lines, unchanged count; `client.ts` unchanged).
- The five repository checks all exit 0. `cargo test --lib --bins --tests --no-fail-fast`: 94 test binaries; 3383 passed, 0 failed, 33 ignored.

## Real-input batch steps

Deferred to the real-input batch — screen locked (user away). One session on an unlocked Mac; the firewall is off
on this Mac (`socketfilterfw --getglobalstate`: disabled), so no "Allow incoming connections" prompt is expected;
if one appears for the lane's `t3` server or the lane app, answer Allow for that binary only.

1. Build: in `~/orca/workspaces/exact2/t3-code-this-machine-network-access`, `export PATH=$HOME/.bun-1.4.2/bin:$PATH`,
   `bun examples/t3-code/terminal-host/build.mjs`, `bun examples/t3-code/stage-runtime.mjs --offline`,
   `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`. For "before", the base
   build in `t3-code-evidence-base` (`target/clients/0d02a3550eba6edce4b12b59/…/T3 Code (Exact).app`) with
   `T3_LOCAL_RUNTIME_DIR=<lane>/t3-home/runtime/versions/0.0.46-nightly.20261005.2667`.
2. Lane env (`<lane>` = `target/lane`): `HOME=<lane>/home`, `CODEX_HOME=<lane>/codex`, `CLAUDE_CONFIG_DIR=<lane>/claude`,
   `XDG_{CONFIG,DATA,CACHE,STATE}_HOME=<lane>/xdg-*`, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=16101`,
   `T3CODE_TELEMETRY_ENABLED=false`, `RUSTUP_HOME=$REAL_HOME/.rustup`, `CARGO_HOME=$REAL_HOME/.cargo`,
   `T3_TAILSCALE_STUB_STATE=<lane>`, `PATH=examples/t3-code/tools/network-lane:$REAL_HOME/.bun-1.4.2/bin:$REAL_HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
   `EXACT_MAC_BIN=<the bundle>/Contents/MacOS/T3 Code (Exact)`.
3. Pictures (agent mode; `bun scripts/agent.mjs macos --size 1280x840 --epoch "$(date -u +%FT%TZ)" …`): skip the wizard
   (`tap Continue`, `tap welcome-agents-continue`, `tap welcome-skip-import`), `tap connection-settings`, `tap Connections`;
   then screenshot: local-only (light/dark × 1280×840/840×620, same on the base build for the pairs); `tap
   network-access-switch` (Enable dialog); `tap network-access-confirm` (Restarting…, then +12 s real: Reachable at);
   `tap network-access-endpoints-toggle` (the rail, four variants); `tap tailscale-https-switch`, port `70000` then
   `8443` (error, preview), `tap tailscale-setup-confirm` (+12 s); `tap authorized-clients-toggle`, `tap
   revoke-other-clients`; `tap create-pairing-link` (dialog, `pairing-scope-5` warning, none ticked), Standard with
   label "Lane phone", `tap pairing-create-confirm` (row); `tap "Pairing link scopes: show 5 scopes" hover`, `clock +300`
   (popover); `tap Share` (panel; `layout` for the URL and QR frames); `tap "Copy link"`; run `tools/network-lane/pair-client.mjs`
   on the pasteboard URL (never print it); +20 s (client row, light and dark); `tap Revoke`; `prefer
   prefers-reduced-motion reduce`, `tap network-access-switch`, `screenshot … over 300 every 30`, Escape; the same with
   `no-preference`; `tap network-access-confirm` (local-only again). Blur the pairing URL and both QR codes before upload.
4. QR: decode the unblurred Share png with Vision (`VNDetectBarcodesRequest`) and compare the SHA-256 of its payload with
   the SHA-256 of `pbpaste` after Copy link; record hashes only.
5. Real input (`orca computer`, lane copy renamed "T3 Code (Lane NW)", real-input lock held): right-click and real hover
   on the scope count (popover after 250 ms) and the endpoint URL (tooltip); real Tab/Shift-Tab/Escape in the five dialogs
   (initial focus, wrap, Escape returns focus to the opener, no dismissal while Restarting… or Creating…); the port field
   with letters, 0 and 70000 typed by keyboard (digits are unaffected by Korean 2-Set; letters via `paste-text`).
6. Attended (the user): Wi-Fi and every other interface down, then Enable network access → the red error and "Could not
   update network access"; a phone scanning the QR opens the same URL.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| Before/after pairs (base `05043629d` vs `b7761f556`), 1280/840 × light/dark | Done | [01-connections-local-only-1280-light-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/p01-01-connections-local-only-1280-light-before-after.png), [01-connections-local-only-1280-dark-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/p02-01-connections-local-only-1280-dark-before-after.png), [01-connections-local-only-840-light-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/p03-01-connections-local-only-840-light-before-after.png), [01-connections-local-only-840-dark-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/p04-01-connections-local-only-840-dark-before-after.png) |
| Enable dialog keys; no dismissal while Restarting…; listener `*:16450` | PASS | [nw-s1-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/01-nw-s1-small.png), [nw-restart](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/02-nw-restart.png) |
| Endpoint URL hover tooltip | PASS | [nw-07-endpoint-hover-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/03-nw-07-endpoint-hover-crop.png) |
| Create pairing link keys / Escape / create | PASS | [nw-s2-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/04-nw-s2-small.png), [nw-create](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/05-nw-create.png) |
| "5 scopes" hover popover | Delay PASS; FAIL (clone bug): the popover opens above and is clipped by the list (title and three scopes cut) | [nw-scopes](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/06-nw-scopes.png), [nw-16-scopes-full-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/07-nw-16-scopes-full-crop.png) |
| Right-click on rows | PASS (no menu, as the reference) | [nw-rightclick](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/08-nw-rightclick.png) |
| QR decode vs Copy link | PASS: equal SHA-256 (URL never printed) | [nw-19-share-blur-preview](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/09-nw-19-share-blur-preview.png), [nw-20-toast](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/10-nw-20-toast.png) |
| Tailscale setup port field, keys; serve 8443 | PASS (stub); the HTTPS endpoint never becomes available with the stub, so Disable cannot open (U9) | [nw-port](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/11-nw-port.png), [nw-s3-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/12-nw-s3-small.png) |
| Disable dialog keys | PASS | [nw-s4-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/13-nw-s4-small.png) |
| Attended: phone scans the QR | PASS: the phone redeemed (browser-session 200 from 192.168.1.84), client row "Phone · Mobile · iOS", link consumed; revoked afterwards | [qr-07-clients-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/14-qr-07-clients-crop.png), [qr-08-revoked-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/15-qr-08-revoked-crop.png) |
| Attended: no network address (Wi-Fi and Tailscale off) | PASS: red "No reachable network address is available for desktop network access on port 16450." + toast "Could not update network access"; switch stays off; listener stays 127.0.0.1 | [offline-sheet](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/p05-offline-sheet.png) |
| Reduced motion (System Settings) | Dialog fades ~150 ms with no visible scale (the reference has no reduced-motion variant); fold/+N/Share not filmed | [nw-on-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/16-nw-on-strip.png) |

Full record: [this-machine-network-access.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/this-machine-network-access/this-machine-network-access.txt). Reduced-motion details: [reduced-motion.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/reduced-motion/reduced-motion.txt).

## Next action

Review of draft PR #248; the user answers U8 and U9. The coordinator runs "Real-input batch steps" in the real-input
batch (pictures, QR decode, motion, real input, the attended rows). Live Tailscale on a tailnet the user provides (U9);
the relaunch rows when exact2 has a process relaunch (#122 / X45); the oracle rows if the oracle is built.
