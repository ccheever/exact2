---
name: 20261005-x05-url-scheme-delivery
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-app-activation, 20261005-managed-codex-chatgpt, 20261005-provider-sign-in-and-install, 20261005-t3-connect-sign-in]
upstream_url: https://github.com/ccheever/exact2/issues/104
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/268
---

# X5: A custom-scheme URL delivered to a data source or module when no route takes it

Moved to main `issues/20261009-protocol-handler-url-templates.md` (2026-10-09); tracked there.

## Summary

T3 Code registers the URL schemes `t3code` and `t3code-dev` and handles two kinds of link: a hosted-web Codex sign-in handoff and a return link into the app.
exact2 can write the scheme into `Info.plist`, but a URL that arrives after boot goes only to a navigation root's `navigate` handler. If none exists, the host refuses it.
The clone registers no scheme and has no workaround.

## Why it arose

### The T3 Code behavior
- Registration. The macOS build declares `protocols: [{ name: "T3 Code", schemes: ["t3code", "t3code-dev"] }]` (`scripts/build-desktop-artifact.ts:2726-2731`).
  The app UI itself is served from `t3code://app` (`apps/desktop/src/electron/ElectronProtocol.ts:15-24`); the development build uses `t3code-dev`.
- Link 1, the hosted-web Codex handoff: `t3code://auth/codex?request=<JSON>`, built by the hosted web client (`packages/shared/src/codexAuthHandoff.ts:87-91`)
  when the client is a remote web page (`apps/web/src/components/settings/CodexSetupSection.tsx:263,607-619`). The desktop opens the system browser on OpenAI's authorize page,
  listens for the loopback callback, and returns the code to the hosted page in a URL fragment (`codexAuthDeliveryUrl`, `:117-133`). Wiring: `apps/desktop/src/app/DesktopClerk.ts:141-165`.
- Link 2, the return link: `t3code://app/welcome`, `/settings` or `/settings/...` (`packages/shared/src/providerAuthReturnUrl.ts:4-35`); the desktop loads the destination in the main window and reveals it (`DesktopClerk.ts:167-184`).
- Delivery paths: the launch arguments (`DesktopClerk.ts:185-186`), the macOS `open-url` event (`:187-189`), and `second-instance` arguments (`:190-199`).

### Where the clone hit it
The clone registers no scheme and builds no handoff (`app.json` has no `host.macos.urlSchemes`; the Codex sign-in in the clone uses a loopback listener, `20261005-managed-codex-chatgpt`).
It would have needed delivery for the hosted-web handoff (`20261005-app-activation`, plan decision U10) or for T3 Connect sign-in (X38), with a scheme of its own, never `t3code`.
Without delivery the app cannot read a link that opened it: a hosted-web page that opens `t3code://auth/codex` starts T3 Code (or nothing), not the clone.

## Clone workaround

None, and none is needed today: E4 (`t3code://`) went out of scope with T3 Connect (X38, closed by the user, 2026-10-08) and U10 is decided (no CLI install action, as the reference). No clone consumer is left.

## Evidence and history

- Filed as [#104](https://github.com/ccheever/exact2/issues/104) (2026-10-06). Closed by main #201 (`20017b7fc`), in the feature branch since main `463acda68`
  ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)): a launch URL that an app with no navigation root cannot hear is now journaled.
  A scheme URL still reached only a navigation root's `navigate`. Adoption: none.
- The rest filed as [#268](https://github.com/ccheever/exact2/issues/268) ([Design], 2026-10-08), reproduced on main `0365ad1a4` before filing: cold launch logs
  `launch URL refused: no navigation root handler`; warm GetURL logs `navigate refused: no navigation root handler`; `location_of` drops the scheme and fragment.
- #268 was narrowed on 2026-10-08 to `protocol_handlers` templates, which still need a navigation root; nothing in the clone waits on it.
