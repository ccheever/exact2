---
name: 20261005-managed-codex-chatgpt
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-managed-codex-chatgpt
pr_url: https://github.com/ccheever/exact2/pull/256
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

Parent specification: [spec](../../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
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
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (streams, open-URL op, setup fixture, `RedactedText`) | pending |
| recorded decision | Plan decision U2 / U23 (apparatus): the provider-setup fixture `target/t3-ui-parity/provider-setup-fixture.mjs` introduced by `20261005-provider-sign-in-and-install` is reused | none | Approved there | pending |
| recorded decision | A real ChatGPT test account and an isolated server for the attended session | none | User provides at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X21](../../issues/20261005-x21-two-way-websocket.md), [X9](../../issues/20261005-x09-root-component-across-files.md), [X14](../../issues/closed/20261005-x14-parked-native-reply.md), [X6](../../issues/20261005-x06-module-quit-shutdown.md), [X5](../../issues/20261005-x05-url-scheme-delivery.md), [X11](../../issues/20261005-x11-shadow-blur-parity.md), [X35](../../issues/closed/20261005-x35-secure-text-entry.md).

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

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented in [PR #256](https://github.com/ccheever/exact2/pull/256) (draft), all six scope items:
- **Receiver** `modules/apple/T3CodexAuth.swift` (rules, BSD-socket listener, start/take/cancel ops on `t3.codexAuth`, window reveal; agent runs record the open via `T3RemoteEditors.openOrRecord`), one area entry in `T3Module.swift`; AppKit `macos/tests/codex-auth` (6).
- **Flow** `codex-setup.ts` (status and the two presentations), `codex-setup-ops.ts` (commands and effects; the snapshot answer runs them, `codex-setup-host.ts`), `codex-auth-request.ts`, `codex-fleet-host.ts` (a background computer's setup in the welcome), `codex-handoff-events.ts`, `provider-readiness.ts` (moved out of `pages-welcome.ts`).
- **Surfaces** `codex-setup.contract` (Settings row, welcome card, Reconnect ChatGPT, Runtime fold, Add dialog setup, plan dialog, Manage usage); `providers.ts`, `pages-welcome.ts`, `providers-wizard.contract`, `app-settings.contract`, `model-picker.contract`, `pages-usage.*`. The welcome's existing-CLI card now shows Ready/Checking/the summary as the reference does (the desktop audit observation below).
- **Plan** `chatgpt-plan.ts`, `chatgpt-plan-view.ts`; keys in `shell-prefs.ts` (`chatgptSharingWelcome`).
- **Ids** `codex_<uuid>` via `ProviderHost.ids`.
- `app.contract` stays at 1,500 lines (in-place edits; `paletteHover` and `providerUi`'s `select` became single statements to free the lines for `providerTick`'s plan-shared close and `provide still`).
- Allowed differences: `returnUrl` omitted (client mode; no scheme) and `""` in the handoff; the atomic instance create; start/take instead of one long IPC call (X14).
- The dialogs keep focus inside with `key` handlers on their first and last controls (`aria-modal` has no Tab containment on macOS, LLP 1080.003); the background toast region is not inert (app-wide, `20261008-dialog-shortcut-focus`).

Lane: `target/lane/mcc` in this worktree (embedded server 16210 with `T3_LOCAL_RUNTIME_DIR`; fixture server 16231 behind the provider-setup fixture proxy 16230, `target/t3-ui-parity/provider-setup-fixture.mjs`, recreated, uncommitted, U2/U23). Fixture-only states: a managed instance signed in with `subscriptionSharing`, a saved profile, the local sign-out notice, the plan shared after Add ChatGPT account.

Real sign-in attempt (03:36–03:41 KST): agent-mode app, the recorded URL opened in Chrome (claude-in-chrome): "Use ChatGPT to sign in to T3 Code" → personal account → "Connect your T3 Code to ChatGPT" → consent (profile + "Use your ChatGPT plan"). The grant click was refused by the session's permission classifier (needs the user's own approval); nothing was granted; the flow expired. `orca computer` then reported AX blocked for every app: the screen was locked (coordinator), so the real-account rows moved to the real-input batch.

2026-10-08 (real-input batch, records PR): real ChatGPT sign-in, relaunch, Disconnect, Reconnect and Change account pass (the user clicked each consent); no banked credit, so no redeem; three clone bugs (unblurred email, auth.subscribe burst, stale list row). Results and proof: "Real-input batch (2026-10-08)" below.

2026-10-08 ([fix-provider-auth-state](20261008-fix-provider-auth-state.md)): of the batch's findings, the
subscribe burst and the stale list row (X64) are fixed and the refresh loop behind Settings is gone; the
unblurred email after Disconnect is the reference's behaviour (plain text, `CodexSetupSection.tsx:604`), so
unchanged. The remote handoff (step 7) passed on a LAN address there; results in that record.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `3e99e28b6` + merge `90cb6ccf2` | `bun test examples/t3-code` 2530 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 2675 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11; AppKit codex-auth 6/6, transport 56/0, local-backend 51/0; caps within; five checks: build, test (3383 pass / 0 fail / 33 ignored, 94 binaries), clippy, fmt, boot green | PR #256: 9 before/after pairs, 5 after-only images, [runs](https://github.com/ccheever/exact2/blob/fb82d57fc1ca9f7834e87d545c1a84488dae3cea/managed-codex-chatgpt/records/runs.txt), tests on base (fail) / branch (pass) | real-input batch rows |
| 1a (2026-10-08 03:36) | same | real sign-in reached OpenAI's consent page; grant refused by the session's permission classifier; screen locked afterwards | runs.txt "Attempted real sign-in" | the user's grant; screen locked |

Acceptance rows: Receiver rules, Validation rules, Install then sign-in (up to the OpenAI page: real install, real `auth.start`, real listener, stand-in callback), Welcome/Settings/dialogs (fixture), Fallback, Cancel/failure, No replay (test), Plan surfaces (fixture; relaunch by test), Account id, Ported tests, Keyboard/Escape (reduced motion: bound, not filmed — the driver's film came back empty), Gates: pass. Trace and pixels: not run — user decision 2026-10-06 (before/after pairs instead). Real sign-in, Remote handoff, and the follow-up's real-account Sign out / Change account: deferred to the real-input batch — screen locked (user away).

## Real-input batch steps

Deferred to the real-input batch — screen locked (user away). Run once, in this order, holding the real-input lock (`target/t3-ui-parity/lanes/.realinput-lock` of the base checkout, owner "managed-codex-chatgpt: login"). The final OAuth grant on OpenAI's consent page needs the person's own click (an agent's click was refused). Never read or print tokens, codes or the email.

1. **Build** (worktree `t3-code-managed-codex-chatgpt`, PR head): `export PATH="$HOME/.bun-1.4.2/bin:$PATH"; EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`.
2. **Lane copy** (L=`<worktree>/target/lane/mcc`): copy `target/clients/*/com.exact.t3code.macos/macos/T3 Code (Exact).app` to `$L/apps/T3 Code (Lane MCC).app`; PlistBuddy `CFBundleIdentifier=com.exact.t3code.macos.lanemcc`, `CFBundleName` and `CFBundleDisplayName` "T3 Code (Lane MCC)"; `codesign --force --deep -s -`.
3. **Launch** through LaunchServices (record the pid; quit it and its `t3` child at the end): `open -n -a "$L/apps/T3 Code (Lane MCC).app" --env T3_LOCAL_HOME=$L/t3home --env T3_LOCAL_PORT=16210 --env T3CODE_TELEMETRY_ENABLED=false --env CODEX_HOME=$L/codex --env CLAUDE_CONFIG_DIR=$L/claude --env XDG_CONFIG_HOME=$L/xdg/config --env XDG_DATA_HOME=$L/xdg/data --env XDG_CACHE_HOME=$L/xdg/cache --env XDG_STATE_HOME=$L/xdg/state --env T3_LOCAL_RUNTIME_DIR=$L/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 --env TMPDIR=$L/tmp/ --env HOME=$L/home --env CFFIXED_USER_HOME=$L/home`. The lane's default Codex slot is already managed with v0.156.1 installed; no ChatGPT registration is stored.
4. **Sign-in (Welcome):** Connect → Continue; Agents → Codex row → **Continue with ChatGPT**. Chrome opens auth.openai.com "Use ChatGPT to sign in to T3 Code" → choose the personal ChatGPT account (provisional: the one matching the user's email) → "Connect your T3 Code to ChatGPT" (name "T3 Code") → Continue → consent "Connect T3 Code to ChatGPT" listing "Access your basic profile information" and "Use your ChatGPT plan" → **the person clicks Continue**. Read back: the browser shows "Return to T3 Code"; the app window comes to the front (note any macOS network prompt; answer it for the lane copy only); the Codex row reads Ready; "Your ChatGPT plan is connected" appears → Continue. State: Settings › Providers › Codex reads "Signed in as" + the redacted placeholder and "Authenticated · ChatGPT"; Manage usage opens `https://chatgpt.com/#settings/Usage`; the model picker shows "Using ChatGPT plan" with a Codex model active; Usage › Cost shows "ChatGPT shared usage".
5. **Relaunch:** quit and launch again (step 3); the plan dialog does not return; the copy's `t3-code.json` (its data folder under `$L/home/Library`) holds one `shell.chatgptSharingWelcome` key.
6. **Sign out / Change account (follow-up row):** Settings › Providers › Codex › **Disconnect** (the reference has no confirmation, `CodexSetupSection.tsx:943-950`; "Sign out → Cancel" is n/a). Read back: the row shows **Reconnect account** / **Use a different account**; the provider list reads "Not authenticated". Then **Reconnect account** → Reconnect ChatGPT (the saved profile focused) → Continue with ChatGPT → Chrome (the account preselected) → the person grants → signed in again (state as in step 4). Then **Change account** → "Use a different account" → Continue with ChatGPT → Chrome account chooser → choose the **same** account (never another one) → the person grants → signed in with the same account.
7. **Remote handoff (optional, needs a decision):** a second lane server must be reached at a non-loopback address (for example `t3 serve --host <the Mac's LAN IP> --port 16232` with its own T3 home), paired from Connections; with it selected, Codex (managed) → Continue with ChatGPT opens OpenAI from the primary → the person grants → read back that the remote instance is authenticated and that no code appears in the app journal or the server logs. Binding a LAN interface is a user decision.
8. **Cleanup:** quit the lane copy, stop its `t3` child, release the lock. Keep `$L/t3home` (the lane's managed sign-in) for later tasks unless told otherwise.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| Real sign-in (Welcome › Continue with ChatGPT; the user clicked each OpenAI consent) | Try 1 expired (the expiry message as specified). Try 2 PASS: `provider.auth.complete`, window to the front, plan dialog, Ready, "Authenticated as [blurred] · ChatGPT", "Using ChatGPT plan", Usage card | [mcc-04](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/01-mcc-04.png), [mcc-13](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/02-mcc-13.png), [mcc-14](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/03-mcc-14.png), [mcc-25-providers](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/04-mcc-25-providers.png), [mcc-22-picker](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/05-mcc-22-picker.png), [mcc-16-usage](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/06-mcc-16-usage.png) |
| Relaunch: plan dialog once | PASS | [mcc-23-relaunch](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/08-mcc-23-relaunch.png) |
| Disconnect | PASS (one logout); FAIL (clone bug): the saved account row then shows the email unblurred (blurred in the image) | [mcc-26-disconnected-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/09-mcc-26-disconnected-crop.png) |
| Reconnect account (consent 2) | Sign-in PASS; FAIL (clone bug): the provider list row stays "Not authenticated" until Providers is reopened (the page refresh does not fix it) | [mcc-28-after-c2](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/10-mcc-28-after-c2.png), [mcc-29-after-refresh-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/11-mcc-29-after-refresh-crop.png), [mcc-reopen](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/12-mcc-reopen.png) |
| Use a different account / Change account (consent 3, same account) | PASS | [mcc-32-after-c3](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/13-mcc-32-after-c3.png) |
| Reduced motion (help disclosure) | PASS: opens at once | [mcc-on-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/14-mcc-on-strip.png) |
| Reset-credit redeem (user-approved, at most one) | Not done: no banked credit on the account (`/usage-limits` and Usage › Limits show no limit window or credit) | [mcc-21-usage-limits](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/07-mcc-21-usage-limits.png) |
| Port in use; remote handoff | Not run (remote handoff needs a LAN-bound second server: user decision) | — |

Full record: [managed-codex-chatgpt.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/managed-codex-chatgpt/managed-codex-chatgpt.txt). Reduced-motion details: [reduced-motion.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/reduced-motion/reduced-motion.txt).

## Next action

Review PR #256 into `feat(example)/t3-code`; run the "Real-input batch steps" when the screen is unlocked; then mark the real-account rows and the follow-up's Sign out / Change account row verified.

## Desktop audit observation, 2026-10-07

The [desktop clickthrough](../../reviews/20261007-desktop-clickthrough.md) reached Welcome >
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
