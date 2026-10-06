---
name: 20261005-provider-sign-in-and-install
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

# Provider sign-in, runtime install and setup entry points

## Outcome

In Settings › Providers a user sees each provider's **Account** row and can sign in (browser, credentials, consent, pasted redirect), cancel, retry, change account and sign out. For Antigravity the **Runtime** row installs, updates, reinstalls, cancels and removes the managed runtime. The provider status banner, the model-picker footer and the composer's provider control open that setup for the right instance. The Add provider wizard's ACP "Sign in" step runs the real flow. Account emails and Source Control accounts show as a blurred same-shape placeholder that a click reveals and hides again (one shared component, reused by the Codex, usage and composer tickets). States and wording follow the reference.

## Scope and exclusions

Included (missing or partial today; the clone has no `provider.auth.*` or `provider.install.*` call):

1. **Streams.** `provider.auth.subscribe` and `provider.install.subscribe` per selected instance, `provider.auth.start/respond/complete/cancel/logout`, `provider.install.start/cancel/remove`, and `server.refreshProviders {instanceId}` after a succeeded sign-in.
2. **Account row** (`ProviderAuthenticationSection`): phases idle, starting, waiting, verifying, succeeded, failed, cancelled; Sign in / Retry sign-in / Change account / Cancel / Sign out (confirm dialog); method picker when more than one method; browser interaction (Open browser, Copy sign-in link, consent sent before the URL opens, paste-redirect form that calls `provider.auth.complete`); credentials form; deviceCode text; "Open docs" when no in-app sign-in is advertised.
3. **Runtime row** (`ProviderSetupSection`, Antigravity): downloading/extracting/verifying/succeeded/failed/cancelled, byte progress, Install / Update / Reinstall / Retry / Cancel installation / Remove (confirm), custom-binary-path notes, "Retry setup status", "Update required", "Setup unavailable".
4. **Editor mounting rules** (`ProviderSettingsPanel.tsx:1066-1118`): Antigravity gets Runtime + Account; a provider with `setup.canAuthenticate` (or an installed ACP agent) gets Account; Cursor with an API key keeps its note (`providers.ts:115-116`).
5. **Entry points:** banner message and inline "Open provider setup" button (`getProviderStatusMessage`, `hasProviderSetup`); model-picker footer "Set up X" / "Open provider setup" (`shouldOfferModelPickerSetup`); the composer control already routes and selects the instance (`composer-controls-view.ts:263-280`; `openProviderSettings` sets `providerSelected`, `app.contract:676-683`); reuse it for the other two.
6. **ACP wizard step 2:** replace the static text (`providers-wizard.contract:239-242`) with `ProviderWizardAuthenticationStep` (Discovering sign-in methods…, Open docs, Done / Skip for now).
7. **Native seam:** one app-module op that opens an http(s) URL in the default browser.
8. **Redacted account text (D14).** Port the placeholder generator of `RedactedSensitiveText` and add a Contract component `RedactedText` (value, `aria-label`, reveal and hide tooltips; the revealed flag is child state, one per instance). Apply it to the Account row's "Signed in as <email>", the provider editor status line ("Authenticated as", email, "· label"; plain text today, `providers.ts:112`; reference `ProviderInstanceCard.tsx:752-762`) and the Source Control account rows (bullets and one page-wide flag today: `source-control-view.ts:238-239`, `settings-source-control.contract:266-269`). Labels and tooltips: "Toggle account email visibility" ("Click to reveal email" / "Click to hide email") and "Toggle source control account visibility" ("Click to reveal account" / "Click to hide account").

Excluded: Codex managed install and ChatGPT sign-in (`20261005-managed-codex-chatgpt`); terminal-type interaction (`20261005-sign-in-terminals`; until then terminal methods are not offered, a deviation recorded in the file header); ACP Native sessions, URL-auth action, Log out (`20261005-provider-settings-upkeep`); redaction at the sites other tickets own (the Codex rows, the composer banner label, the Usage popover use `RedactedText` through their dependency on this ticket); hosted-web delivery files.

## Context and guidance

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/ProviderAuthenticationSection.tsx:31-475`, `W/settings/ProviderSetupSection.tsx:95-328`, `W/settings/ProviderWizardAuthenticationStep.tsx`, `W/settings/ProviderSettingsPanel.tsx:1043-1118`, `W/chat/ProviderStatusBanner.tsx:12-140`, `W/chat/ModelPickerContent.tsx:103-114, 1051-1076`, `packages/contracts/src/providerSetup.ts`, `packages/contracts/src/rpc.ts:366-381`, `packages/client-runtime/src/state/server.ts:989-1060`, `W/settings/RedactedSensitiveText.tsx:6-61`, `W/settings/ProviderInstanceCard.tsx:188-200, 752-762`, `W/settings/SourceControlSettings.tsx:160-168`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (a long flow is a stream plus short commands; keep text drafts apart from committed values), accessibility (icon buttons need `aria-label`; status text is announced), design (all states), layout-and-interaction (native inputs), testing-and-debugging (`--json` drives; static evidence is not runtime evidence), platforms (macOS). Unknown in the library: app-local Swift modules, the module op that opens a URL, stream handling in the Swift transport. The clone's runtime evidence on the pinned main (from `20261005-clone-on-exact2-main`) is the basis.
Consumer framework revision and toolchain: the main pin chosen in `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Facts that shape the work (observed): the server owns the flow and the secrets. At HEAD only Codex and Antigravity set `setup.canInstall`; Codex, Antigravity, Cursor (browser, no callback) and ACP agents (terminal, browser with consent, credentials for `env_var` methods) advertise sign-in. No server code emits `deviceCode`, but the contract allows it, so the text row is ported. The reference opens the browser only after consent is recorded (`ProviderAuthenticationSection.tsx:136-156`). Stream retry in the clone sends `_retryDue` to TypeScript, which decides to resubscribe (`modules/apple/T3Transport.swift:634-648`).
Redaction facts (observed): the placeholder is deterministic: an FNV-1a seed from the value, a mix per character, letters from `abcdefghjkmnpqrstuvwxyz23456789`, keeping `@ . - _` (`RedactedSensitiveText.tsx:6-25`). Hidden, the text is monospace 11 px, muted, not selectable and blurred; revealed, it is muted; hover raises it to the foreground colour. The clone's one existing redaction is a bullet string behind a page-wide flag.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract). With `20261005-hot-file-split` merged, the ops go into a `client-ops-providers.ts` area file instead of `client.ts`, and the stream and native op use the per-area registration points.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged into `daehyeon/t3-code` | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle, trace proxy, diff, RPC tally) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| recorded decision | Plan decision U2 / U23 (apparatus): provider-setup stream fixture `target/t3-ui-parity/provider-setup-fixture.mjs` (a scripted responder in the lane kit that answers `provider.auth.*` / `provider.install.*` with scripted state sequences on ports 16000–16999; reused by `20261005-managed-codex-chatgpt`, `20261005-provider-settings-upkeep` and `20261005-usage-reset-and-feedback`) | none | User approves at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts (local, unpublished) read 2026-10-05; no upstream search (no network); library topics above. Issue records: [X21](../issues/20261005-x21-two-way-websocket.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X17](../issues/20261005-x17-popover-position-try.md), [X5](../issues/20261005-x05-url-scheme-delivery.md), [X35](../issues/20261005-x35-secure-text-entry.md), [X42](../issues/20261005-x42-text-blur-filter.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X21 | RPC and streams to the T3 server from a data module | `C/modules/apple/T3Transport.swift` carries all RPC and the 16-stream cap (`:529`) | nonblocking (workaround: the existing Swift transport; two more keys per open editor) | Record the open stream count in the acceptance log |
| X35 | Secure (password) text entry in Contract | The credentials form and the paste-redirect field use `type="password"` inputs, which the clone already has (`providers-wizard.contract:333`, `providers.contract:411`); whether that is true secure entry is unchecked | nonblocking (workaround: the existing `type="password"` input, or a native secure field in the module) | Check at `prepare` |
| X9 | Resources in child components | `C/app.contract` 1,327 and `C/client.ts` 1,455 of 1,500 lines | nonblocking until the cap | Add no resource; extend the existing `providerPage` view model; at most one dispatch line in `client.ts` |
| X17 | Popover flips | The picker footer lives inside the existing picker popover; no flip needed | nonblocking | none |
| X5 | URL scheme delivery | Not used: the `t3code://` handler serves hosted web only | not applicable | none |
| X11 | Tooltip and popover shadow | Redaction tooltip, confirm dialogs | nonblocking (visible difference declared) | Cite in the matrix |
| [X42](../issues/20261005-x42-text-blur-filter.md) | `filter: blur()` on text and boxes | The reference blurs the hidden placeholder (`blur-xs`, `RedactedSensitiveText.tsx:48`); the only `filter` precedent in the clone is on SVG groups (`settings-a-collections.contract:110`) | unknown until checked; if absent, nonblocking only when the user waives the blur (the placeholder text still hides the value) | Check `filter` on a text node at `prepare`; apply the X42 adoption steps when it lands |
| open an external URL from app code | App-local Swift op (`NSWorkspace.shared.open`; precedent `C/modules/apple/R6MediaPreview.swift:248-250`) | Not an exact2 gap; unknown in the library | nonblocking (workaround matches the reference result) | Confirm the reference's scheme rules in `ElectronShell.openExternal` at implementation |

## Implementation notes

- **Port, do not rewrite.** New TypeScript modules with file headers naming the reference file, function names and each change: `provider-auth.ts` (derivations from `ProviderAuthenticationSection`: `active`, `signedIn`, `isDiscovering`, `needsExternalSetup`, description and status text), `provider-install.ts` (from `ProviderSetupActions`), `provider-status-message.ts` (`getProviderStatusBannerKey`, `getProviderStatusMessage`, `hasProviderSetup`), `provider-picker-setup.ts` (`shouldOfferModelPickerSetup`; the clone already has `pickerReady` at `model-catalog.ts:16`).
- **Streams** follow `C/r4-surfaces-device.ts:25-62` (store per client, newest subscription wins, `_retryDue` handled in TypeScript). Keys `provider-auth:<instanceId>` and `provider-install:<instanceId>`. Subscribe when an editor or wizard step is open; unsubscribe when it closes (the reference sets `idleTtlMs: 0`).
- **Contract:** a new `providers-setup.contract` for the two rows and the wizard step; do not grow `providers.contract` (570 lines). Per-row pending replaces the global `commandPending` for these ops (the reference coalesces clicks with `pendingRef`, `ProviderAuthenticationSection.tsx:56, 103-124`); a waiting flow must not disable the page.
- **Native op:** one branch in `T3Module.swift`'s `later` (165 lines) calls a new small Swift file. In agent mode it records the URL instead of opening a browser (the module already branches on `context.agent`).
- **Copy link** uses the existing `copyText` op; it also sends consent first, as the reference does (`:226-240`).
- **Email text (D14):** `redacted-text.ts` ports `redactedPlaceholder`; `redacted-text.contract` holds `RedactedText` with a `testId` per instance. The Account row (`ProviderAuthenticationSection.tsx:166-176`), the editor status line and Source Control accounts all use it; the page-wide reveal flag in `source-control-view.ts` is removed (the reveal is per instance). Tooltips use the app's tooltip. Never draw an account email in clear text before a reveal.
- **Password-type inputs** exist already (`providers-wizard.contract:333`). Motion: the reference has none for these rows; the progress bar has no transition, so no reduced-motion branch.
- States to cover: loading ("Starting sign-in…", "Checking your account…", "Discovering sign-in methods…"), empty ("No in-app sign-in advertised…"), error (alert text), disabled (read-only, pending, query error), cancelled (button reads "Retry sign-in"), hover, keyboard focus on every button, permission ("Provider setup is read-only."). `aria-label`: "Sign-in method", "Copy sign-in link", "Cancel sign-in", "Remove downloaded runtime", "Antigravity download" (progress), and the two redaction labels above.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Account row phases | Lane backend + provider-setup fixture scripting idle → starting → waiting(browser) → verifying → succeeded, then failed and cancelled; isolated HOME and ports 16000–16999 | `bun scripts/agent.mjs macos --json tree state "tap provider-sign-in" "clock +1000" tree state logs "screenshot out.png"` per phase | Description, buttons and `aria-live` text match the reference strings per phase; failed shows the message; cancelled shows "Retry sign-in" | macOS, 1280×840 | `--json` transcript, screenshots |
| Consent before open | Same, `requiresConsent: true` | Tap "Open browser" | Trace shows `provider.auth.respond {browser, accept}` before the native open op; the agent stub records the URL | macOS | trace + `logs` |
| Paste redirect and stale flow | Same, `acceptsCallback: true` | Paste text, Continue; replace the flow, then submit the old draft | `provider.auth.complete` carries the trimmed text once; nothing is sent for a replaced flow | macOS | trace |
| Sign out, change account | Authenticated fixture provider | Tap Sign out, cancel, then confirm | Dialog text matches; one `provider.auth.logout` on confirm only | macOS | trace |
| Credentials form | ACP fixture with an `env_var` method | Fill secret fields, Connect | Password-type inputs; `respond {credentials}` with values; draft cleared on success | macOS | trace + tree |
| Runtime row | Fixture install stream: downloading n/total → extracting → verifying → succeeded; failed; cancelled; custom binary path | Install, Cancel installation, Retry, Remove (confirm) | Labels, byte text (`x.x MB of y.y MB`), `progressbar` value, Remove only after confirm, "Retry setup status" on stream error | macOS | screenshots, transcript |
| Entry points | Fixture with an unauthenticated and an uninstalled provider | Open the banner button, the picker footer, the composer control | Each opens Settings › Providers with that instance selected; strings equal `getProviderStatusMessage` output | macOS | transcript |
| ACP wizard step | Fixture ACP instance, then no advertised methods | Add provider › ACP › Continue to sign-in | Discovering → Account row, or "Open docs"; Done vs Skip for now | macOS | screenshots |
| Subscription trace | Reference oracle and clone on the same lane backend, scenario "open provider editors" | `target/t3-ui-parity/trace-diff.mjs` | Same `provider.auth.subscribe`, `provider.install.subscribe` and refresh calls; allowed differences have reasons | macOS | diff report |
| Oracle pixel pairs | States reachable without a live provider (idle, read-only, no in-app sign-in, banner, picker footer) | `target/t3-ui-parity/electron-oracle.mjs` vs clone at 1280×840 and 840×620, light and dark | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; scripted states are not pixel-compared and the matrix says so | macOS | image pairs |
| Redacted email | Authenticated fixture provider with an email; two Source Control items with accounts | `bun scripts/agent.mjs macos --json tree "tap provider-email" tree "tap provider-email" tree` | Before the tap, no tree string contains the email and the placeholder has its shape and length (`@ . - _` kept); one tap reveals, the second hides, tooltip text flips; each Source Control item reveals on its own with "Toggle source control account visibility" | macOS, 1280×840 | transcript, screenshots |
| Real sign-in and install `(attended session)` | A person with a Google account, a Cursor account and a disposable ACP agent; isolated T3CODE_HOME; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Sign in with the OS browser, install and remove the real Antigravity runtime, cancel mid-way | Flows complete; Tab and Return reach every button; recorded steps | macOS, real input | session notes |
| Ported tests | — | `bun test` | `ProviderSetupSection.test.tsx` "Antigravity setup" (8 tests, original names); `ProviderSettingsPanel.environment.test.tsx` ":552 keeps the signed-in ACP account visible…", ":584 routes explicit ACP browser authentication consent…", ":313 opens the requested provider instance…", ":350 does not substitute another account…"; `ProviderStatusBanner.test.tsx` (`getProviderStatusMessage`, 4 tests; Antigravity cases); `ModelPickerContent.test.ts` "shouldOfferModelPickerSetup"; `ProviderInstanceCard.test.ts` ":91 shows a redacted provider email in the editor header status line"; new tests for `redactedPlaceholder` (no reference test exists); each converted to a fake native harness (`composer-controls-fixture.ts` style) with `now` arguments | macOS host machine | test log |
| Keyboard, Escape, reduced motion | Authenticated and unauthenticated fixture providers; Settings › Providers open | `bun scripts/agent.mjs macos --json "key tab" tree "key return" tree "key escape" tree "prefer prefers-reduced-motion reduce" "key tab" tree` | Tab visits every Account and Runtime control in reading order with a visible focus ring; Return activates; Escape closes the Sign out and Remove confirms, sends nothing, and returns focus to the control that opened them; the picker footer button is reachable by Tab; nothing in these rows animates, so reduced motion changes nothing | macOS | transcript |
| Gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/provider-auth.ts`, `C/provider-install.ts`, `C/provider-status-message.ts`, `C/provider-picker-setup.ts`, `C/redacted-text.ts`, `C/redacted-text.contract`, `C/providers-setup.contract`, `C/providers.ts`, `C/source-control-view.ts`, `C/settings-source-control.contract`, `C/presentation.ts`, `C/model-catalog.ts`, `C/model-picker.contract`, `C/modules/apple/` (open-URL file, `T3Module.swift` branch), their `*.test.ts`, the fixture `target/t3-ui-parity/provider-setup-fixture.mjs`.
Required environment: Xcode 27.0, pinned Bun and Hermes, reference oracle build, lane backend with isolated HOME/CODEX_HOME/CLAUDE_CONFIG_DIR/XDG_*/T3CODE_HOME; no running T3 Code (Nightly); never port 3773, `~/.t3` or the `t3code` scheme. Attended row: Google, Cursor and ACP test accounts (names only). Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

On hold (2026-10-06): the user paused every task that needs a sign-in. Work stopped during
design; no source file was changed. Branch `feat(example)/t3-code-provider-sign-in-and-install`
(local only, not pushed).

Findings to resume from (observed in this worktree):
- Base: `bun test examples/t3-code` 1200 pass, 0 fail; the macOS bundle builds
  (`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos`, exit 0).
- No `target/t3-ui-parity/` lane kit exists in any checkout; the provider-setup fixture
  (decision U2/U23) is still unapproved apparatus. Bun tests with a fake native
  (`composer-controls-fixture.ts` style) can script the auth/install streams instead.
- Planned registration points: stream events in `client.ts` drain (one `provider-auth:` /
  `provider-install:` key line, as `DEVICE_STATE_KEY`), a local `setup:` op prefix in
  `client.ts` `command()` (local ops do not set `commandPending`, so a waiting flow does
  not disable the page), one `providerSetup` action in `app.contract` sending
  `localChanged`, subscriptions started from the `providerPage`/`providerWizard` sources
  in `app.ts` (subscribe when the editor or wizard step shows, unsubscribe otherwise), one
  `providerOpenUrl` branch in `T3Module.swift` (agent mode records the URL).
- `presentation.ts` `providerBanner` has its own message logic; replace it with a port of
  `getProviderStatusMessage` / `hasProviderSetup`. The banner (`chat.contract` AlertStack)
  and the model picker (`model-picker.contract`) need an open-setup action prop.
- X42 is supported on main (issues/README.md), so `RedactedText` can use
  `filter="blur(4px)"`. X35 is #134 (agent tree prints a password input's value).
- The reference's generic Account row does not call `server.refreshProviders` after
  success; only `CodexSetupSection.tsx:295-296` does.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the two prerequisites merge and the fixture apparatus is approved; then `implement`.
