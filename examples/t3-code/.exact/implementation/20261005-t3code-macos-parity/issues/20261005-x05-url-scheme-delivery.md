---
name: 20261005-x05-url-scheme-delivery
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-app-activation, 20261005-managed-codex-chatgpt, 20261005-provider-sign-in-and-install, 20261005-t3-connect-sign-in]
upstream_url: https://github.com/ccheever/exact2/issues/104
reproduced_on: null
---

# X5: A custom-scheme URL delivered to a data source or module when no route takes it

## Summary

T3 Code registers the URL schemes `t3code` and `t3code-dev` and handles two kinds of link: a hosted-web Codex sign-in handoff and a return link into the app.
exact2 can write the scheme into `Info.plist`, but a URL that arrives after boot goes only to a navigation root's `navigate` handler. If none exists, the host refuses it.
The clone registers no scheme and has no workaround. If the user keeps the hosted-web handoff (decision U10) or builds T3 Connect sign-in (issue X38), the app needs the URL as an event.
`20261005-app-activation` waits on this issue only if the handoff is kept.

## Why this issue arose

### The T3 Code behavior
- Registration. The macOS build declares `protocols: [{ name: "T3 Code", schemes: ["t3code", "t3code-dev"] }]` (`scripts/build-desktop-artifact.ts:2726-2731`).
  The Linux build declares the same, "so browsers can hand t3code:// OAuth callbacks to the app" (`:2775-2785`). The app UI itself is served from `t3code://app` (`apps/desktop/src/electron/ElectronProtocol.ts:15-24`); the development build uses `t3code-dev`.
- Link 1, the hosted-web Codex handoff: `t3code://auth/codex?request=<JSON>`. The hosted web client builds it (`packages/shared/src/codexAuthHandoff.ts:87-91`)
  when the client is a remote web page (`needsManualCallback = remoteWeb`, `apps/web/src/components/settings/CodexSetupSection.tsx:263,607-619`).
  The JSON holds `authorizationUrl` (at most 16,384 characters), `returnUrl` (4,096), `environmentId`, `instanceId` and `flowId` (128) (`codexAuthHandoff.ts:5-12`).
  The desktop accepts the link only if the scheme matches the build, the host is `auth`, the path is `/codex` and exactly one `request` parameter exists (`:93-115`).
  It then opens the system browser on OpenAI's authorize page (only `https://auth.openai.com/api/accounts/authorize` with a loopback `redirect_uri` `http://127.0.0.1:<port>/auth/callback`, PKCE `S256`; `:27-58`), listens for the loopback callback,
  and returns the code to the hosted page in a URL fragment (`codexAuthDeliveryUrl`, `:117-133`). Wiring: `apps/desktop/src/app/DesktopClerk.ts:141-165`.
  Failure text: "Could not receive hosted web ChatGPT sign-in. Retry or use the redirect URL in the web app." (`DesktopClerk.ts:158`); a failed run logs "Could not complete ChatGPT desktop handoff." (`:162`).
- Link 2, the return link: `t3code://app/welcome`, `/settings` or `/settings/...`. `providerAuthReturnUrl` (`packages/shared/src/providerAuthReturnUrl.ts:4-35`) accepts the desktop scheme with host `app`,
  or an http(s) loopback or `https://app.t3.codes` URL; it removes user info and all query keys except `machine`, `project`, `checkout`, `environmentId`, `instanceId` (none on `/welcome`).
  The desktop loads the destination in the main window and reveals it (`DesktopClerk.ts:167-184`).
- Delivery paths: the launch arguments (`DesktopClerk.ts:185-186`), the macOS `open-url` event, which calls `preventDefault()` when a link was handled (`:187-189`), and `second-instance` arguments (`:190-199`).
  A second instance that gets no handled link reveals the main window.
- The Clerk SDK bridge registers the scheme and holds the single-instance lock: "OAuth deep-link callbacks on Windows/Linux are forwarded to the running app" (`DesktopClerk.ts:130-138`, a code comment). Whether the SDK uses the scheme on macOS is not established here; confirm at `prepare` of `20261005-t3-connect-sign-in`.
- Tests in the reference: `packages/shared/src/codexAuthHandoff.test.ts` ("keeps the hosted return route, account, and environment with the code in a fragment", "rejects other handlers, schemes, arbitrary return sites, and non-OpenAI authorization",
  "rejects duplicated authorization parameters and non-loopback callback addresses"); `apps/desktop/src/app/CodexAuthCallback.test.ts` ("binds before opening sign-in, ignores a foreign response, and returns only the code callback",
  "returns hosted web to the exact instance and environment without putting the code in its query", "cancels and releases its listener so exact-port reauthorization can run again", "allows two accounts to complete independently").

### What exact2 does today
Quoted from `examples/t3-code/EXACT2-GAPS.md` section X5 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`; not re-measured for this plan):
- "`host.macos.urlSchemes` writes `CFBundleURLTypes` (`host/apple/build.mjs:297-301`). On main, a URL that arrives after boot goes only to a navigation root's `navigate` handler; otherwise the host logs
  \"navigate refused: no navigation root handler\" (`ExactMac/main.swift:423-438`, `ExactKit/Session.swift:1330-1345`). `ExactModule` has no URL callback. Governing: LLP 1038 D8."
- "Support needed. When no route takes a scheme URL, deliver it to a data source or the app module as an event."
- Bundled library (`20261005-platforms-v3`): URL schemes are **not covered: unknown**.
- Observed in the clone on 2026-10-05: `app.json` has no `host.macos.urlSchemes`. The clone has no URL handler and no navigation root that takes a scheme URL.
  The Codex sign-in in the clone uses a loopback listener, not a scheme (`20261005-managed-codex-chatgpt`).

### Where the clone hits it
The clone is not affected today, because it registers no scheme and builds no handoff. The default in `20261005-app-activation` is "not built".
If the handoff is kept, the clone needs its own scheme name, never `t3code` (the original app owns it; with a shared `~/.t3` only one of the two apps runs at a time, but the OS would still hand `t3code://` links to whichever app it picks).
There is no workaround: without delivery the app cannot read a link that opened it. A user sees this difference: a hosted-web page that opens `t3code://auth/codex` starts T3 Code (or nothing), not the clone.

## Why it must be resolved
The goal is a full clone of the desktop app, so every link the original handles is a parity row. Two things wait on this issue:
(1) `20261005-app-activation` row "hosted-web deep link", only if the user keeps E4 (plan decision U10); (2) `20261005-t3-connect-sign-in`, if the user builds T3 Connect and its sign-in needs a scheme callback.
If the user drops both, this issue closes by that decision and no framework work is needed. If the user keeps either, the clone cannot ship it without a delivery path, and no app-side workaround exists.

## Requested support
Web analogy: the Web App Manifest `protocol_handlers` member and `navigator.registerProtocolHandler()`. The browser opens the app at a handler URL that carries the link (`url` template with `%s`). The page reads it from its own location.
On the macOS host first; other hosts are noted only because the contract is shared.
- **A (preferred, web-standard).** A manifest `protocol_handlers` entry (scheme plus route template). The host navigates the existing window to that route with the link in a parameter, also when no navigation root handler is declared, and on a cold start from the launch URL.
  The route can read the link from a data source.
- **B.** A module callback in `ExactModule` (for example `handleURL(url) -> Bool`) that returns whether the link was handled, plus a way to push the URL string to a data source. This is native-only and simpler, but it is not web parity.
- Both: deliver once per open, keep percent-encoding and length (at least 20,480 characters, enough for the largest handoff), deliver the launch URL on a cold start, and let the app refuse links it does not recognize.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Make a minimal app whose `app.json` has `host.macos.urlSchemes: ["exacttest"]` and no navigation root handler. Build and run it as a lane build.
2. Run `open 'exacttest://auth/x?request=1'` while the app runs. Quit the app and run it again, so the link starts the app.
3. Expected (the reference's `open-url` and launch-argument paths): the app receives the full URL string once in each case.
   Actual (per `EXACT2-GAPS.md`): the host logs "navigate refused: no navigation root handler"; the app sees nothing.
Never use the `t3code` scheme for this test.

## Acceptance for the fix
- Warm start and cold start each deliver the exact URL (`bun scripts/agent.mjs macos state` shows the received string; an AppKit test posts the Apple event).
- A 16,384-character URL arrives unchanged. Two links in quick succession arrive as two events, in order. A scheme the app does not declare is not delivered.
- With a navigation root present, the route still wins (no regression of LLP 1038 D8 behavior).
- If A is chosen: a conformance case against Chrome's `protocol_handlers` where the browser supports it; otherwise a documented native-only difference.

## App adoption after resolution
If the user kept E4: register a clone-specific scheme in `app.json`, read links in a data source, port `readCodexAuthHandoff`, `providerAuthReturnUrl`, `codexAuthDeliveryUrl` and their tests, and add the matching rows to `20261005-app-activation`.
If the user built T3 Connect: wire the sign-in callback in `20261005-t3-connect-sign-in`. `issue-close` checks that those rows pass on the pinned `main` and that no link handling lives outside the supported path.
If the user dropped both: close the issue by decision and record it in the two tickets.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval), after plan decision U10 is answered.

## Merged upstream; not fixed for this ask (2026-10-07, adopt-main-fixes-r4)

[#104](https://github.com/ccheever/exact2/issues/104) was closed by main #201 (`20017b7fc`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)):
a launch URL that an app with no navigation root cannot hear is now journaled, as a running app's already
was. A scheme URL is still delivered only to a navigation root's `navigate`; nothing reaches a data source
or module (this issue's request). Adoption: none. The clone has no scheme workaround, and
`20261005-app-activation` is not built (user decision U10 pending); its rows and `t3-connect-sign-in`'s
stay blocked as before.
