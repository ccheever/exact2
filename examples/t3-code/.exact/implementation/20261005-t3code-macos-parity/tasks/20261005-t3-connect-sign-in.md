---
name: 20261005-t3-connect-sign-in
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# T3 Connect: Clerk sign-in, relay connections, hosted pairing and the `t3code://` handoff

## Outcome

A user signs in to a T3 Connect account from the clone, makes this machine available to their other devices through T3's relay, sees and connects to the account's other machines, and manages mobile clients, as in the reference desktop app.
Hosted pairing links and the hosted-web Codex handoff work through the clone's own URL scheme. This ticket starts only after the user decides, in issue X38, to build the feature. If the decision is to close, this ticket is closed with it and the clone keeps the feature hidden, as the reference does in a build without public configuration.

## Scope and exclusions

Included, if the decision is to build (reference behavior at `1e2ecbd975`; the evidence and strings are in [X38](../issues/20261005-x38-t3-connect-clerk-sign-in.md)):

1. **Configuration gate.** The feature shows only when the build has a Clerk publishable key, a Clerk JWT template and a relay URL (`apps/web/src/cloud/publicConfig.ts:40-75`). Without them: no sidebar sign-in or avatar, no T3 Connect or Publish agent activity rows, no search entries,
   and `t3 connect` answers "T3 Connect commands are unavailable: this build is missing T3 Connect public configuration." (`apps/server/src/binCli.ts:30-60`). The clone's `cloud` catalog flag becomes true only when the configuration exists. Where the keys come from is a decision in X38 (T3-approved values, or the clone's own services); no key is stored in the repository.
2. **Sign-in.** Signed out: a sidebar item "Sign in to T3 Connect" (`T3ConnectSidebarSignIn.tsx`). Signed in: the avatar menu with the account pages "T3 Connect" and "Mobile clients" (`T3ConnectAccountPages.tsx`). Sign-out clears the connected environments (comment in `connectOnboarding.ts`).
   The sign-in surface is chosen at `prepare` (system browser with a loopback or scheme callback, or a web view); only its presentation may differ from the reference, which runs the Clerk modal inside Electron. Tokens live in Keychain, never in files or logs.
3. **Link this machine** (Settings → Connections, local environment only). Rows "T3 Connect" and "Publish agent activity" with the texts, tooltips and toasts listed in X38 (`ConnectionsSettings.tsx:1727-1850`): disabled without a session ("Sign in to T3 Connect to manage this environment.") or without the Manage relay permission
   ("Your session does not have permission to manage T3 Connect access."). The calls are the environment HTTP routes `/api/connect/link-proof`, `/relay-config`, `/link-state`, `/unlink`, `/preferences` with DPoP proofs (`packages/contracts/src/environmentHttp.ts:595-640`, `apps/web/src/cloud/dpop.ts`, `linkEnvironment.ts`).
4. **Onboarding wizard** "Set up T3 Connect": steps "Publish this environment" and "Publish agent activity", Continue ("Enabling…"), checkbox "Don't show this again" stored per account; it shows on every sign-in unless opted out (`ConnectOnboardingDialog.tsx`, `connectOnboarding.ts`).
5. **Relay client install.** RPC `cloud.getRelayClientStatus` and `cloud.installRelayClient`; dialog "Install relay client?" / "Installing relay client" with the stages Checking current installation, Waiting for installer, Downloading relay client, Verifying download, Installing relay client, Validating executable, Activating installation;
   the messages "T3 Code cannot install the relay client automatically on <platform>-<arch>." and "Relay client installation was cancelled." (`RelayClientInstallDialog.tsx:25-100`, `linkEnvironment.ts:48-100`). The server does the install; the clone draws the dialog.
6. **Connect to other machines.** The account's environments in Settings → Connections → Environments with "Available", "Unavailable", "Checking…", "Connecting…", "Relay online", "Relay offline", "Checking relay status", "Add route", "Saved without T3 Connect", "Not added", "You appear to be offline.", "Client not supported";
   toasts "T3 Connect route added", "Environment added", "Could not connect environment" with "Copy trace ID" (`CloudEnvironmentConnectList.tsx`). The relay becomes a route of a saved environment (`relayManaged`; `DesktopConnectionCatalogStore.ts:303-305`): `20261005-environment-routes` creates the relay kind that it only ports today.
   The dialog "Remove <label> from this device?" (`RemoveT3ConnectEnvironmentDialog.tsx`).
7. **Account pages.** "T3 Connect": "Environments registered to your account. Connections on this device are managed in Settings." with Deregister ("Deregistering…"), toast "Server deregistered" ("T3 Connect access was revoked and a host space is now available."), "Could not deregister server", empty state "No T3 Connect environments"
   (`T3ConnectUserProfilePage.tsx`). "Mobile clients": "Mobile devices that get notifications from your environments." with badges "Push notifications" and "Live Activities", empty state "No mobile clients" (`MobileClientsUserProfilePage.tsx`).
8. **Hosted pairing** (decision U8 stays in `20261005-this-machine-network-access`; this ticket adds the sign-in link only if the decision keeps hosted links): `https://app.t3.codes/pair?host=<endpoint>[&label=]#token=…`, the endpoint hint "Opens the hosted app, no install needed", the copy toast "Hosted app link copied".
9. **Handoff and return links** (decision U10, issue [X5](../issues/20261005-x05-url-scheme-delivery.md)): `t3code://auth/codex?request=…` and `t3code://app/(welcome|settings…)` in the reference become links on a clone-specific scheme (never `t3code`). Port `codexAuthHandoff`, `providerAuthReturnUrl` and their validation; failure text "Could not receive hosted web ChatGPT sign-in. Retry or use the redirect URL in the web app."
10. **Terminal.** `t3 connect` and its browser page "Connecting your terminal" (`ConnectCliAuthSurface.tsx`) belong to the user's CLI and to T3's hosted app. The clone only checks that an embedded server linked by the clone's own session behaves the same; the CLI install action is U10's.

Excluded: telemetry (`20261005-telemetry`), the update feed (`20261005-app-update-feed`), WSL, mobile apps, Auto balance, the Browser surface, and any T3 key stored in the repository.

**Planned split.** If the user decides to build it, split at `prepare` into separate PRs (no
stacks): (1) Clerk sign-in, account pages and sign-out; (2) T3 Connect link and relay
connections; (3) hosted pairing and the `t3code://` handoff (needs issue X5). Keep this name
for the first; name the others `20261005-t3-connect-sign-in-<part>`.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior: `1e2ecbd975`, files named above; `docs/user/remote-access.md` section "T3 Connect"; `docs/operations/release.md:440-478` for how T3 signs and configures its own release.
Library revision: `20261005-platforms-v3`. Selected topics: to select at `prepare`; third-party sign-in, hosted relay connections, Keychain use and URL schemes are **unknown in the library**, so the pinned `main` and the oracle are the evidence.
Consumer framework revision and toolchain: the pin from `20261005-clone-on-exact2-main`.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`. The oracle for this ticket is a reference build with public configuration and a test account that the user provides (names only here).
Every launch runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 (see `20261005-embedded-server-runtime`).
Open decisions, all answered in X38 before `prepare`: build or close; the source of the Clerk and relay values; the sign-in surface; the scheme name (X5, U10); hosted links (U8).
Whether the pinned server archive already carries public configuration is checked at `issue-open` of X38 (`t3 connect --help`). If it does, the server's `/api/connect/*` routes exist but stay unused until this ticket runs.
Port changes for headers: `Effect` services become plain functions; React atoms become TypeScript state in the clone's client; Clerk components are not ported (the presentation is the clone's own).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | [X38](../issues/20261005-x38-t3-connect-clerk-sign-in.md) | pending | The user decides to build (with the key source, sign-in surface and scheme name), or closes it | pending |
| resolved framework issue | [X5](../issues/20261005-x05-url-scheme-delivery.md) | pending | Resolved upstream on the pinned `main`, or the user drops the handoff and any scheme callback (then this row is marked not needed) | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-environment-routes`, `20261005-local-primary-environment`, `20261005-this-machine-network-access` and `20261005-app-activation` | none | The relay route kind, the primary, the relay permissions and the scheme registration are in place | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X38](../issues/20261005-x38-t3-connect-clerk-sign-in.md) | Decision to build or close | scope decision; not in the library | blocking | `issue-open`, then the user decides |
| [X5](../issues/20261005-x05-url-scheme-delivery.md) | URL scheme delivered to a module | `EXACT2-GAPS.md` X5 at `d2cb661eb`; not re-measured | blocking only if a scheme callback or the handoff is built | resolve upstream or drop |
| [X1](../issues/20261005-x01-chromium-cdp-browser-surface.md) | Browser surface | blocked by policy | blocking only if the sign-in surface is a web view | choose the system browser flow, or wait |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket for data modules | `EXACT2-GAPS.md` X21; workaround is the Swift transport | nonblocking (the relay socket can use the Swift transport) | confirm at `prepare` |
| [X39](../issues/20261005-x39-telemetry.md) | Relay tracing values in the public configuration | scope decision | nonblocking | none |

## Implementation notes

- Keep the relay and Clerk code in new files (`cloud-link.ts`, `cloud-connect.ts`, `cloud-account.ts`, `modules/apple/T3Cloud.swift`); do not grow `app.contract` or `client.ts` (both near the 1,500-line cap).
- Credentials: the Clerk session and the DPoP key go to Keychain under the clone's own service name; nothing is written to `t3-code.json`, logs or the trace.
- The onboarding opt-out is per account and per device.
- The dialogs (Set up T3 Connect, Install relay client?, Remove <label> from this device?, deregister confirmation) take focus on open, trap Tab, close on Escape, return focus to the control that opened them, and show no motion under reduced motion.
- The sign-in surface may differ in presentation. Everything after it (rows, texts, toasts, states) follows the reference.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test` for the ported `linkEnvironment`, `managedAuth`, `connectCliAuth`, `dpop`, `publicConfig`, `relayClientInstallDialog`, `CloudEnvironmentConnectList`, `T3ConnectUserProfilePage`, `MobileClientsUserProfilePage.logic`, `codexAuthHandoff`, `providerAuthReturnUrl` and `DesktopConnectionCatalogStore` (relay target) cases | Original test names pass | macOS | logs |
| Gate | Lane build with and without the public values; oracle build the same | Open the sidebar, Settings → Connections, settings search "T3 Connect" | Without: nothing is drawn and search finds nothing; with: the sign-in item and both rows | macOS 1280×840 and 840×620, light and dark | png pairs vs oracle |
| Sign-in states | Test account (name only) | Sign in, sign out, sign in again | Item, avatar and account pages appear and disappear as in the reference; sign-out clears the connected environments; no token in logs or files (`grep -r` of the data folder, `log show`) | macOS | recording, grep log |
| Link and unlink | Signed in; local environment on; Manage relay granted | Turn T3 Connect on and off; turn Publish agent activity on and off with the tunnel on and off | Texts, toasts and states of X38; turning the tunnel off while publishing stays on says "T3 Connect tunnel disabled"; the HTTP call sequence matches the oracle trace | macOS | `trace-diff.mjs`, png |
| Disabled states | Signed out; session without Manage relay | Hover the switch | The two tooltips of X38 | macOS | `tree`, png |
| Onboarding | Fresh account; account with opt-out | Sign in | Wizard shows each sign-in unless opted out per account; Continue shows "Enabling…" | macOS | recording |
| Relay client install | Server without the relay client; unsupported platform stub | Turn T3 Connect on | Confirm dialog, the seven stage labels in order, the two error messages | macOS | recording, `tree --ax` |
| Connect list | Account with several machines (test fixtures) | Open Environments | Statuses, relay badges, Add route, the three toasts, offline text; a route is added to the saved environment | macOS | png, state |
| Account pages | Same | Open "T3 Connect" and "Mobile clients" | Texts, empty states, Deregister flow and toasts | macOS | png |
| Hosted link and handoff | Clone scheme registered (X5); a loopback fake of the authorize page | Open a handoff link; open a return link | The listener binds before the browser opens, ignores a foreign response, returns only the code in a URL fragment; invalid links are refused; the main window loads the return route | macOS | transcript |
| Dialog keyboard, focus, Escape, reduced motion | Each dialog | Open with the keyboard, Tab, Escape; repeat with reduced motion | Focus enters the dialog, Tab stays inside, Escape closes and returns focus, no animation under reduced motion | macOS | `tree --ax` |
| Design check | Lane build, oracle build | `layout` of rows, dialogs and pages at both sizes, light and dark | Values equal the oracle's within 1 px | macOS | `layout` JSON, png pairs |
| Real sign-in (attended session) | The user's test account | Sign in through the chosen surface | Works on a real network; keys come from the approved source | macOS | recording |

Task-owned source paths: `cloud-link.ts`, `cloud-connect.ts`, `cloud-account.ts`, `modules/apple/T3Cloud.swift`, `settings-catalog.ts` (flag source), `connections.ts` and the relay route code of `20261005-environment-routes`, `app.json` (scheme), matching `.contract` and test files, `macos/tests/cloud`.
Required environment: Xcode 27.0, pinned Bun, the oracle build with public configuration, a T3 Connect test account and approved keys (names only, no secrets), a lane port range 16000-16999.

## Progress

Blocked. Not started.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | X38 decision |

## Next action

Blocked until X38 is resolved or decided; then `prepare`, or close this ticket if the decision is to close.
