---
name: 20261005-this-machine-network-access
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
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-local-primary-environment](20261005-local-primary-environment.md) | pending | Merged | pending |
| recorded decision | U4 decided (relaunch); U8 (hosted link), U9 (Tailscale verification) | none | U8 and U9 answered at `prepare` | U4: user 2026-10-05 |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | `subscribeAuthAccess` stream through the Swift transport | existing transport | nonblocking | none |
| [X19](../issues/20261005-x19-data-source-timers.md) | Per-second expiry tick, 60 s status cache | native/Contract tasks | nonblocking | follow the clone's time-as-argument pattern |
| [X4](../issues/20261005-x04-bundle-helper-executables.md) | Server runtime in the bundle | upstream ticket | inherited | none here |
| [X45](../issues/20261005-x45-app-relaunch.md) | App relaunch after an exposure change | X45 (unconfirmed) | blocking for the relaunch rows if X45 is confirmed missing (stopgap meanwhile: restart the server in place and reconnect; a visible difference) | use the relaunch when X45 is adopted |

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

Task-owned source paths: `server-exposure.ts`, `tailscale.ts`, `advertised-endpoint.ts`, `pairing-urls.ts`, `connections-network.contract`, `connections.ts` (+ tests), `modules/apple/T3LocalNetwork.swift`, `apple/tests/local-backend` additions, `t3-code.json` schema.
Required environment: Xcode 27.0, pinned Bun, the staged runtime, lane ports 16000-16999; optional tailnet.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the four merged task PRs (`20261005-hot-file-split`, `20261005-clone-on-exact2-main`, `20261005-desktop-oracle-and-trace`, `20261005-local-primary-environment`). Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
