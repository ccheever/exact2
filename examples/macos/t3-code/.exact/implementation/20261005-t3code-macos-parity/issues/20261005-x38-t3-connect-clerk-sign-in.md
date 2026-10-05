---
name: 20261005-x38-t3-connect-clerk-sign-in
plan: 20261005-t3code-macos-parity
status: draft
kind: scope-decision
blocks: [20261005-t3-connect-sign-in]
upstream_url: null
reproduced_on: null
---

# X38: T3 Connect, Clerk sign-in, relay connections, hosted pairing and the `t3code://` handoff

## Summary

T3 Code lets a user sign in with a Clerk account, make a machine reachable through T3's hosted relay ("T3 Connect"), and connect to the account's other machines. The desktop app also handles `t3code://` links for a hosted-web sign-in handoff.
All of it depends on T3's own services and on keys that T3 builds into its releases. The user excluded it from this clone. The clone hides the whole feature, as the reference does in a build without that configuration.
This issue records the pending decision: build the feature, or close it as out of scope. Ticket `20261005-t3-connect-sign-in` is blocked until then.

## Why this issue arose

### The T3 Code behavior
- Configuration gate. The feature exists only when the build has public configuration: a Clerk publishable key, a Clerk JWT template and a relay URL (`apps/web/src/cloud/publicConfig.ts:40-59,72-75`; the desktop build also carries the key, `apps/desktop/src/app/DesktopClerk.ts:22,60-78`).
  Without it: the sidebar sign-in and avatar render nothing (`apps/web/src/components/clerk/T3ConnectSidebarSignIn.tsx:9-17`), the two settings rows render nothing (`apps/web/src/components/settings/ConnectionsSettings.tsx:1853-1854`),
  settings search drops the entries (`apps/web/src/components/settings/settingsSearch.ts:1007`), and `t3 connect` answers "T3 Connect commands are unavailable: this build is missing T3 Connect public configuration." (`apps/server/src/binCli.ts:30-60`).
- Sign in. Sidebar button "Sign in to T3 Connect" (`T3ConnectSidebarSignIn.tsx:61`) opens the Clerk sign-in modal (`apps/web/src/components/clerk/useT3ConnectAuthPrompt.tsx:6-10`). Signed in, the sidebar shows the Clerk avatar menu with account pages "T3 Connect" and "Mobile clients"
  (`T3ConnectAccountPages.tsx:12-19`). The desktop loads the full clerk-js runtime only when the build is configured (`apps/web/src/main.tsx:46-55`). The Electron Clerk bridge stores its session under the state directory (`DesktopClerk.ts:79-90`) and holds the single-instance lock (`:130-138`).
- Link this machine (Settings → Connections; the rows belong to the local environment, and the switch is disabled without the Manage relay permission). Row "T3 Connect": off text "Make this environment available to your other devices through T3 Connect.", on text "This environment is available to your other devices through T3 Connect."
  Disabled tooltips: "Sign in to T3 Connect to manage this environment." and "Your session does not have permission to manage T3 Connect access." Toasts: "T3 Connect linked", "T3 Connect tunnel disabled", "T3 Connect unlinked" (`ConnectionsSettings.tsx:1758-1830`).
  Row "Publish agent activity": "Send activity to mobile notifications and Live Activities without T3 Connect." with toasts "Agent activity enabled" and "Agent activity disabled" (`:1800-1850`).
- Onboarding. After sign-in a wizard "Set up T3 Connect" offers "Publish this environment" and "Publish agent activity" with Continue ("Enabling…") and a checkbox "Don't show this again" (`apps/web/src/components/cloud/ConnectOnboardingDialog.tsx:213-313`, checkbox at `:251-254`; opt-out stored per account under `t3code:connect-onboarding-opt-out:v1`, `apps/web/src/cloud/connectOnboarding.ts`).
- Link flow. The client asks the server for relay-client status (RPC `cloud.getRelayClientStatus`) and, if needed, installs the relay client with a confirm dialog and progress (RPC `cloud.installRelayClient`, `RelayClientInstallDialog.tsx`; messages such as
  "T3 Code cannot install the relay client automatically on <platform>-<arch>." and "Relay client installation was cancelled.", `apps/web/src/cloud/linkEnvironment.ts:48-100`). It then calls the environment's HTTP routes `/api/connect/link-proof`, `/relay-config`, `/link-state`, `/unlink`, `/preferences`
  and two routes without the environment-auth middleware, `/api/t3-connect/health` and `/api/connect/mint-credential` (`packages/contracts/src/environmentHttp.ts:595-640`), with signed DPoP proofs (`apps/web/src/cloud/dpop.ts`). A missing relay URL fails with "T3CODE_RELAY_URL is not configured." (`linkEnvironment.ts:256-270`).
  On the server side, the managed tunnel runs in `apps/server/src/cloud/ManagedEndpointRuntime.ts` and `managedTunnelStartup.ts`, and agent activity goes out through `apps/server/src/relay/AgentAwarenessRelay.ts` (started at `serverRuntimeStartup.ts:424,525-527`).
- Connect to other machines. Settings → Connections → Environments lists the account's environments with "Available", "Unavailable", "Checking…", "Connecting…", relay badges "Relay online", "Relay offline", "Checking relay status", "Add route", and toasts "T3 Connect route added", "Environment added", "Could not connect environment" with "Copy trace ID" (`CloudEnvironmentConnectList.tsx:160-500`).
  A T3 Connect machine becomes one more route of a saved environment (`relayManaged` in `apps/desktop/src/settings/DesktopSavedEnvironments.ts:51`; `RelayConnectionTarget` in `DesktopConnectionCatalogStore.ts:303-305`). The account page lists registered servers with "Deregister" and a toast "Server deregistered" (`T3ConnectUserProfilePage.tsx:96-200`).
- Terminal: `t3 connect` prints a browser link and a code; the page "Connecting your terminal" authorizes it (`apps/web/src/components/cloud/ConnectCliAuthSurface.tsx`, route `routes/connect.tsx`). User documentation: `docs/user/remote-access.md` section "T3 Connect".
- Hosted pairing and the handoff. Pairing links for HTTPS endpoints can point at the T3-hosted web app, `https://app.t3.codes/pair?host=<endpoint>#token=…` (`ConnectionsSettings.tsx:517-579`; decision U8 in `20261005-this-machine-network-access`).
  The desktop also handles `t3code://auth/codex?request=…` and `t3code://app/(welcome|settings…)` through the Clerk bridge (`DesktopClerk.ts:127-199`); see issue X5 for the details.
- Reference tests: `apps/web/src/cloud/linkEnvironment.test.ts`, `managedAuth.test.ts`, `connectCliAuth.test.ts`, `dpop.test.ts`, `publicConfig.test.ts`, `relayClientInstallDialog.test.ts`, `components/cloud/CloudEnvironmentConnectList.test.tsx`, `components/clerk/T3ConnectUserProfilePage.test.tsx`; `apps/desktop/src/app/DesktopClerk.test.ts`.

### What exact2 does today
- Quoted from `examples/macos/t3-code/EXACT2-GAPS.md` (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): row X21, "Two-way WebSocket for data modules … framework feature (LLP 1016.000: receive-only)", with the app-side workaround "Swift transport (`T3Transport.swift`, `T3Fleet.swift`)"
  and, in the detail, "receive-only WebSocket … no frame is ever sent" (`docs/reference.md:405-408`). Row X5: scheme links reach only a navigation root's `navigate` handler (`ExactMac/main.swift:423-438`). Row X1 (Browser surface) is open.
  The document has no row for Clerk, a relay, or DPoP.
- Bundled library (`20261005-platforms-v3`): third-party sign-in, hosted services and relay connections are **not covered: unknown**.
- Observed in the clone on 2026-10-05: the catalog holds the gated entries `t3-connect` (flags `desktop,cloud,localEnvironment`) and `publish-agent-activity` (`cloud,localEnvironment`) (`settings-catalog.ts:105-106`).
  The search filter (`settings-search.ts:46-56`) treats `mac` and `localEnvironment` as true, ties `environment`, `providerSettings` and `macProviderSettings` to the connection state and `autoSettle` to its setting, and treats every other flag, including `cloud` and `desktop`, as false. So these entries stay hidden.
  `r10-connect-pairing.ts` parses a hosted pairing link in Add Environment (it reads `?host=` and a token) but the clone creates no hosted link and no relay route.

### Where the clone hits it
Nothing in the clone calls Clerk or a relay. `20261005-environment-routes` ports the relay route kind in pure code for test parity and never creates one. `20261005-this-machine-network-access` draws the Manage relay and View relay permission toggles (they are scopes) but not the T3 Connect rows.
By default `20261005-app-activation` registers no URL scheme. A user of the clone sees: no "Sign in to T3 Connect", no T3 Connect or Publish agent activity rows, no account pages, no connect list, and a `t3code://` link goes to T3 Code (if installed), not to the clone.
This equals the reference in a build without public configuration. It differs from the shipped Nightly build, which has the configuration.

## Why it must be resolved
The goal is a full clone, and the user excluded this feature for now. A declared difference is not an end state, so the plan needs a recorded decision: build or close.
The feature needs things this example does not have, and the decision depends on them:
(1) T3's keys. The Clerk publishable key, JWT template and relay URL are values that T3's release build supplies. The macOS release is signed for T3's App ID `com.t3tools.t3code` with Associated Domains for passkeys (`docs/operations/release.md:440-478`). The clone's bundle id is `com.exact.t3code.macos`. The plan holds no T3 key and no consent to use T3's services from another app.
(2) A sign-in surface. The reference runs the Clerk web SDK inside Electron (clerk-js in the renderer). The clone has no web view, and the Browser surface is itself blocked (X1).
(3) Outbound services. The relay is a hosted service that the clone would call. Its protocol is reachable only through the reference code read here.
Pending: ticket `20261005-t3-connect-sign-in` (blocked), the hosted-link choice U8, the handoff choice U10 and issue X5. If the user closes this issue, those three resolve as "not built" and the gated rows stay hidden.

## Requested support
Product decision only, unless the user chooses to build. If built, exact2 would need (each item to confirm at `issue-open`):
- A sign-in path for a native app. Options: the system browser with a loopback or scheme callback (needs X5 for the scheme) and tokens kept in Keychain; or an embedded web view (needs X1). Whether Clerk offers a flow for a non-Electron native client is not established here.
- Two-way WebSocket and HTTPS to the relay from app code. The clone already sends frames over its Swift transport (X21 workaround), so this may need no new framework support.
- Signing DPoP proofs (ES256 over a per-install key) in app code: a Keychain-held key and a signing call. Whether the data runtime offers Web Crypto is **unknown**.
- Keys and consent from T3, or the clone's own Clerk application, relay and Apple App ID. The last is a separate service project and out of this example's scope.

## How to reproduce
This is a decision, not a defect. To record at `issue-open`:
1. Run T3 Code (Nightly) with a lane home and port. Note the sidebar button, the two Connections rows and `t3 connect` output.
2. Run the oracle or a reference dev build without `VITE_CLERK_PUBLISHABLE_KEY`, `VITE_CLERK_JWT_TEMPLATE` and `VITE_T3CODE_RELAY_URL`, and confirm that all of the above disappear.
3. In the pinned server archive, run `t3 connect --help` and record whether T3 Connect is available (that shows whether the archive carries the configuration; the clone's embedded server will carry whatever the archive has).

## Acceptance for the fix
- If closed: the decision is recorded in this issue and in the ticket; the gated rows stay hidden; no code. The plan's exclusion list names this issue.
- If built: `20261005-t3-connect-sign-in` passes its acceptance rows (sign-in, link and unlink, connect list, onboarding, hosted link, handoff) against the reference oracle and against a lane relay or T3-approved test account, with no secret in the repository.

## App adoption after resolution
If closed: set the ticket to closed with the decision; keep `cloud` false; nothing else changes. If built: unblock the ticket, set the catalog flag source for `cloud`, register the scheme chosen under X5, create relay routes in `20261005-environment-routes`, and re-run the Connections rows in `20261005-this-machine-network-access`.
`issue-close` checks the decision text and that no hidden row, flag or dead code remains for the closed case.

## Status and next action
Draft; not reproduced; not searched upstream; not published. Decision pending (build or close).
Next: `issue-open` (record the three checks above and prepare the report for the user's approval; publication only after approval), then ask the user to decide.
