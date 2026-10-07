---
name: 20261005-managed-codex-chatgpt
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

# Managed Codex with a ChatGPT account

## Outcome

A user sets up Codex without a CLI: **Continue with ChatGPT** installs the managed Codex runtime on the environment, opens the OS browser for OpenAI sign-in, receives the redirect on a loopback address in the app, and finishes sign-in on the server. The same flow works from the Welcome card, Settings › Providers (**ChatGPT account** row: Reconnect account, Use a different account, Change account, Disconnect) and **Add ChatGPT account**. Once a plan is shared, the app shows the plan surfaces: the one-time "Your ChatGPT plan is connected" dialog, "Using ChatGPT plan" in the model picker, "ChatGPT shared usage" on the Usage page, and **Manage usage**.

## Scope and exclusions

Included (all missing today: `providers.ts:317-327` only creates the instance, `pages-welcome.ts:231-237` only sets `setupMode`, and the Add dialog closes after create, `app.contract:918`):

1. **Loopback receiver** (app-local Swift, new file `modules/apple/T3CodexAuth.swift`), a port of `receiveCodexAuthCallback`: validate the authorization URL; bind `127.0.0.1:<port from redirect_uri>` before opening the browser; open the OS browser; accept only `GET` on the exact callback path with the right `state` and exactly one `code` or `error`; answer a foreign request with HTTP 400; reply to the right one with the static "Return to T3 Code" page (headers `cache-control: no-store`, `referrer-policy: no-referrer`, `x-content-type-options: nosniff`, CSP); give up after 300 s; release the port on finish or cancel; allow two accounts at once; reveal and activate the window on success.
2. **Flow** (TypeScript, port of `ManagedCodexSetup`): `setup` → `provider.install.start` when not installed or an update is offered, continue to sign-in when the install succeeds; `provider.auth.start {methodId, callbackMode: "client"}`; open the page once per flow; `provider.auth.complete {flowId, callbackUrl}`; `server.refreshProviders {instanceId}`; stay "Finishing sign-in..." until the provider snapshot is authenticated; cancel; Disconnect (`provider.auth.logout`, "Signed out locally." notice).
3. **Surfaces:** Welcome card (onboarding presentation), Settings row, Reconnect dialog (saved profiles via `chatgpt-profile:<clientId>` methods, "Use a different account" = `chatgpt-change-account`), Add ChatGPT account dialog with the setup inside it ("Finish later", closes when sharing is on), "Having trouble signing in?" fallback (paste redirect URL, "Try sign-in in your browser"), read-only runtime rows for a managed instance (`CodexManagedRuntimeFields`: Binary path, CODEX_HOME path, Shadow home path), mode switch ("Use existing CLI").
4. **Remote-environment handoff:** for a non-loopback environment with a connected loopback primary, `provider.chatgpt.reconnect-profile` → `provider.chatgpt.handoff.subscribe` (never replayed after a reconnect) → `provider.chatgpt.import-profile`.
5. **D7 plan surfaces** (`usesChatGptSharing`: authenticated and `auth.subscriptionSharing === true`).
6. **Instance id for new ChatGPT accounts:** `codex_<uuid>` as `AddCodexAccountDialog.tsx:50-53` ("The ID is routing identity; the name is editable and need not be unique"). The clone derives `codex_chatgpt_<slug>` and a free suffix (`providers.ts:317-327`), so two accounts named "Personal" become `codex_chatgpt_personal` and `codex_chatgpt_personal_2` (test `providers.test.ts:246-252`, which this ticket rewrites). The uuid comes from the native id source the client uses for command ids (`client.ts:175`, exposed as `restAccess().ids`, `client.ts:1024`); add it to `ProviderHost`.

Excluded: hosted-web delivery (`codexAuthHandoffUrl`, `readCodexAuthHandoff`, `#codex-auth=` fragments, `ProviderAuthCallbackCoordinator`, `providerAuthReturnUrl`) and the `t3code://auth/codex` handler (it sits in the excluded Clerk bridge, `apps/desktop/src/app/DesktopClerk.ts:140-185`; see the E4 decision in `20261005-app-activation`); the generic Account row and the redaction component (`20261005-provider-sign-in-and-install`; this ticket uses `RedactedText` through that dependency); reset credits (`20261005-usage-reset-and-feedback`).

## Context and guidance

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/CodexSetupSection.tsx` (`ManagedCodexSetup` `:194-1008`, `signIn` `:397-454`, `setup` `:456-470`, `openPage` `:518-545`), `W/settings/ChatGptAccountPicker.tsx`, `W/settings/AddCodexAccountDialog.tsx`, `W/settings/ChatGptWelcomeCoordinator.tsx`, `W/chat/ChatGptSharingControl.tsx`, `W/usage/UsagePage.tsx:543-548`, `W/settings/CodexSetupSection.logic.ts`, `apps/desktop/src/ipc/methods/providerAuth.ts:16-49`, `packages/shared/src/codexAuthCallback.ts:23-100`, `packages/shared/src/codexAuthHandoff.ts:27-85`, `packages/shared/src/usageLimits.ts:24-29`, server `apps/server/src/provider/CodexChatGptAuth.ts:462-605`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (flow = stream + short commands; await persistence of the acknowledgment keys), design (all states), accessibility (dialog focus return is not certified by the library: assert it by drive), testing-and-debugging, platforms. Unknown in the library: app-local Swift modules, loopback sockets, activating the app window, long native waits. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
How the split works (observed): the client starts install and sign-in; the server builds the OAuth URL with PKCE, exchanges the code, checks the ID token and stores tokens (`CodexChatGptAuth.ts:462-505, 583-650`); with `callbackMode: "client"` the server picks a port in 49152–65535 and does not listen. The desktop reference always sends `client` (`CodexSetupSection.tsx:244-246`), so the Swift listener is needed for local and remote environments. Server OAuth endpoints are fixed to `auth.openai.com` (`CodexChatGptAuth.ts:33`), so only a real ChatGPT account completes a real sign-in. The Codex runtime is downloaded by the server from the official release (`CodexInstallation.ts:44-60`, sha256 checked); the app bundles nothing. Needs from the OS: default browser and a loopback socket. No URL scheme, no terminal.
`returnUrl` is optional in `ProviderAuthStartInput`; the clone omits it (registering or using the `t3code` scheme is forbidden). In client mode the server does not use it; record the omission in the trace allow list.
Long native waits: a parked native reply can be dropped (X14). Use start / take: `providerAuthStart` returns after bind and open; completion is announced through the module's `changed` topic and read with `providerAuthTake` (precedent: `composerSendIntent` `take()`, `T3Module.swift`).
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (dialog/popover Contract). With `20261005-hot-file-split` merged, ops go into an area file and the native ops use the per-area registration point in `T3Module.swift`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (streams, open-URL op, setup fixture, `RedactedText`) | pending |
| recorded decision | Plan decision U2 / U23 (apparatus): the provider-setup fixture `target/t3-ui-parity/provider-setup-fixture.mjs` introduced by `20261005-provider-sign-in-and-install` is reused | none | Approved there | pending |
| recorded decision | A real ChatGPT test account and an isolated server for the attended session | none | User provides at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X21](../issues/20261005-x21-two-way-websocket.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X14](../issues/closed/20261005-x14-parked-native-reply.md), [X6](../issues/20261005-x06-module-quit-shutdown.md), [X5](../issues/20261005-x05-url-scheme-delivery.md), [X11](../issues/20261005-x11-shadow-blur-parity.md), [X35](../issues/closed/20261005-x35-secure-text-entry.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X21 | Server RPC and streams | Existing Swift transport; handoff stream is the first stream that must not resubscribe on `_retryDue` | nonblocking (workaround: existing transport + TypeScript decides) | Test: no replay of `provider.chatgpt.handoff.subscribe` after a stream error |
| X35 | Secure (password) text entry in Contract | The paste-redirect field is `type="password"` in the reference (`CodexSetupSection.tsx:674-676`) | nonblocking (workaround: the existing `type="password"` input, `providers-wizard.contract:333`) | Check at `prepare` 2026-10-07: #134 closed by main #167: a Contract password field's value is masked in `tree`, `layout` and the `type` reply, and `autocomplete` sets its content type; the app's state and data module stay outside that (adopt-main-fixes-input). |
| X14 | Parked native reply | A long `later` reply can be lost if its answer is let go | resolved for a watched topic's re-ask (#109, main #183, adopted 2026-10-07 by adopt-main-fixes-r3); a re-ask for new arguments still drops the reply | Use start / take; do not rely on a parked reply across an argument change |
| X6 | Module hook at quit | Listener lives at most 300 s; process exit closes the socket | nonblocking | none |
| X5 | URL scheme delivery | Not used | not applicable | none |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | Put flow state in a TypeScript module and child-component drafts; one dispatch line in `client.ts` |
| X11 | Dialog shadow and glass | Reconnect and Add dialogs, plan dialog | nonblocking (visible difference declared) | Cite in the pixel matrix |

## Implementation notes

- New TypeScript with provenance headers: `codex-setup.ts` (extracted from the React component; no reference test exists, so the ported test list below is behavioral), `codex-auth-request.ts` (`codexAuthorizationRequest`, `codexCallbackUrl`; the Swift side carries the same rules), `chatgpt-plan.ts` (`usesChatGptSharing`, `CHATGPT_USAGE_URL`, welcome acknowledgment keys). `readCodexSetupMode` (`W/settings/CodexSetupSection.logic.ts`) replaces the inline checks (`pages-welcome.ts:89`).
- Acknowledgment keys for the plan dialog go in the versioned `t3-code.json` (same pattern as `providerUpdateDismissals`, `shell-prefs.ts`), keyed by environment, instance and profile id / email / "default".
- Reuse: `providerOp` / `runProviderOp` (`providers.ts:258`), the welcome state (`pages-welcome.ts:231-237`), `ChatGptAccountDialog` (`providers-wizard.contract:365-388`), `copyText`, the open-URL op from `20261005-provider-sign-in-and-install`, `ProviderMark(driver="codex")`.
- Instance creation keeps the atomic `providerInstanceMutation` (`providers.ts:324`); the reference sends a whole-map patch (`AddCodexAccountDialog.tsx:54-69`). Allow-list the difference with the reason. This ticket alone edits the `provider-chatgpt` op and the Add ChatGPT dialog; `20261005-provider-settings-upkeep` does not touch them.
- "Signed in as <email>" appears in three places (`CodexSetupSection.tsx:74, 592, 776`) and the reference always draws it through `RedactedSensitiveText`. Use `RedactedText` from `20261005-provider-sign-in-and-install` (dependency below) with the same labels and tooltips; never draw the email in clear text.
- In agent mode the browser open is recorded, not performed. The listener binds a port from the lane range; scripted `authorizationUrl` values use it.
- Error text to keep: "The ChatGPT callback port is in use on this computer. Close the other sign-in and try again, or paste the redirect URL in T3 Code.", "Sign-in expired. Try again.", "Sign-in cancelled on this computer.", "Could not open your sign-in browser.", "This sign-in is already open on this computer.", "Could not finish sign-in on this computer. Try again or paste the redirect URL below."
- States: loading ("Setting up...", "Downloading x of y MB.", "Installing Codex.", "Checking Codex."), empty ("Preparing sign-in."), error (alert text; install failed), disabled (read-only, not enabled, busy, status read errors), in-progress ("Finishing sign-in..."), cancelled (Cancel returns to the start state), hover and keyboard focus on every button, permission (read-only). `aria-label`: "Having trouble signing in?" (expand), "ChatGPT sign-in redirect URL", "ChatGPT account to connect", "Codex binary path". Motion: the disclosure chevron rotates 90° in the reference; give it a short transition and none under reduced motion.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Receiver rules | AppKit binary `macos/tests/codex-auth/` with a fake browser closure | Run the ported cases | Binds before opening; foreign state gets 400; headers set; code never appears in the page; cancel frees the port and the same port works again; two accounts finish independently; 300 s expiry (test-shortened); port in use gives the message | macOS | test log |
| Validation rules | bun + AppKit | Duplicate params, `localhost`, `https` redirect, non-OpenAI host, bad state/challenge/client_id | Rejected exactly as `codexAuthorizationRequest` / `codexCallbackUrl` | macOS | test log |
| Install then sign-in | Lane backend + provider-setup fixture (install downloading → succeeded; auth waiting → verifying → succeeded; snapshot authenticated later) | `bun scripts/agent.mjs macos --json tree state "tap continue-with-chatgpt" "clock settle" tree state logs` | Labels in order; browser open recorded once per flow; `complete` carries the callback URL; stays "Finishing sign-in..." until the snapshot flips | macOS, 1280×840 | transcript, screenshots |
| Welcome, Settings, dialogs | Same, saved profiles in the fixture methods | Reconnect account, Use a different account, Change account, Disconnect, Add ChatGPT account, Finish later | Method ids sent are `chatgpt`, `chatgpt-profile:<id>`, `chatgpt-change-account`; Add dialog closes when sharing turns on | macOS | transcript, trace |
| Fallback | Fixture waiting state | Expand "Having trouble signing in?", paste URL, Connect | One `complete`; password-type field; "Try sign-in in your browser" opens the URL | macOS | trace |
| Cancel, failure | Fixture failed and cancelled | Cancel mid-flow; fail | Listener released (`logs`); start state restored; alert text shown | macOS | logs |
| No replay | Handoff fixture, stream error | Drop the stream | No automatic resubscribe of the handoff stream | macOS | trace |
| Plan surfaces | Fixture with `subscriptionSharing: true` | Launch; Continue; relaunch; open the picker and Usage | Dialog once per profile, not after relaunch; footer and Usage row show; Manage usage opens `https://chatgpt.com/#settings/Usage` (recorded) | macOS | screenshots, prefs file |
| Trace and pixels | Oracle and clone, same lane backend, no account | `target/t3-ui-parity/trace-diff.mjs`; pixel pairs at 1280×840 and 840×620, light and dark, for states reachable without an account | Same calls; allowed differences have reasons (`returnUrl` omitted, atomic instance mutation); every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Real sign-in `(attended session)` | Isolated T3CODE_HOME, real ChatGPT account, default browser; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Full sign-in; Reconnect; Use a different account; Disconnect; occupy the port with another listener; wait out the expiry | The browser returns to a page, the app window comes to the front, the row shows the account; every message above appears; note whether macOS shows a network prompt | macOS, real input | session notes |
| Remote handoff `(attended session)` | A paired remote environment, real account; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Sign in for the remote instance | Profile imported; instance authenticated; no code in any log | macOS | session notes |
| Account id | Add ChatGPT account named "Personal" twice | Create both | Ids are `codex_<uuid>` and differ; names may repeat; both instances `setupMode: managed`; the existing instance is untouched | macOS | settings dump, `providers.test.ts` rewritten |
| Ported tests | — | `bun test` | `CodexAuthCallback.test.ts` (4; the hosted-web case is n/a), `codexAuthHandoff.test.ts` ("rejects duplicated authorization parameters and non-loopback callback addresses", the OpenAI-host case of "rejects other handlers…"), `usageLimits.test.ts` ":1193 ChatGPT sharing presentation"; original names | macOS host machine | test log |
| Keyboard, Escape, reduced motion | Fixture with saved profiles; sharing on | Tab through the Reconnect dialog (account radios, Connect), the Add ChatGPT account dialog (name field, Continue, Finish later) and the plan dialog (Manage usage, Continue); press Escape in each; `prefer prefers-reduced-motion reduce` and toggle "Having trouble signing in?" | Focus starts inside each dialog and stays there; Return submits; Escape closes (in the plan dialog it acts as Continue, because the reference stores the acknowledgment on close) and focus returns to the opener; the disclosure chevron turn is instant under reduced motion | macOS | transcript |
| Gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/codex-setup.ts`, `C/codex-auth-request.ts`, `C/chatgpt-plan.ts`, `C/pages-welcome.ts`, `C/pages-welcome.contract`, `C/providers.ts`, `C/providers.test.ts`, `C/providers-wizard.contract`, `C/providers-setup.contract`, `C/pages-usage.*`, `C/model-picker.contract`, `C/modules/apple/T3CodexAuth.swift`, `C/modules/apple/T3Module.swift` (branch), `C/apple/tests/codex-auth/`, their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, isolated lane backend; attended row needs a ChatGPT account (name only) and network. Never port 3773, `~/.t3` or the `t3code` scheme; the real callback port is random (49152–65535) and loopback only. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision). Tasks that need a sign-in (GitHub, provider accounts, T3 Connect) do not start until the user lifts the hold.

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

Starts when the user lifts the sign-in hold: `prepare` from `feat(example)/t3-code`, covering sign-in rows with lane fixtures (fake provider, seeded data).

## Desktop audit observation, 2026-10-07

The [desktop clickthrough](../reviews/20261007-desktop-clickthrough.md) reached Welcome >
Agents in both clients against the same isolated reference backend. Electron showed the
existing Codex account as signed in (identity masked) and `Ready`. Exact instead showed
`Use existing CLI` and `Continue with ChatGPT`. Reference revision:
`1e2ecbd9758830669684b494d4398f626b0576e0`; Exact:
`fbce02624d2e33449ee2cde34497083d6fd47457`.

Reproduce by pairing a fresh client with a backend whose Codex instance is already ready,
then continue from Connect to Agents without starting a new sign-in. Compare the readiness
card and available next actions. Local captures are
`target/desktop-audit/evidence/ref-04-agents.{png,txt}` and
`target/desktop-audit/native/native-05-agents.{png,json}`. No authentication was initiated.
This belongs to the existing Welcome/setup presentation scope; it does not create a second
provider task or lift the sign-in hold. Include already-authenticated existing instances
when verifying the eventual implementation.
