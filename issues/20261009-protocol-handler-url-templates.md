# Deliver a custom-scheme URL, whole, to an app with no navigation root (rest of #104)

**Status:** Open
**Systems:** app manifest, host/apple, router
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/268

## Current scope

Use selected protocol_handlers templates to encode complete URLs into navigate locations; check warm/cold ordering, schemes, fragments and length. This still requires a navigation root; original no-router acceptance is not selected.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #104. #201 made a launch URL that no navigation root hears visible in the journal, as a running app's already was. The request itself is still open: an app that registers a custom URL scheme (`host.macos.urlSchemes`) cannot read the URL the OS hands it unless it declares a navigation root, and even then it gets a route location with the scheme and fragment removed.

Apps register a scheme to receive sign-in hand-offs and OAuth callbacks (`<scheme>://auth/...?request=<JSON>`, with an authorization URL of up to 16 KB in the query) and return links into the app. A single-window app whose screens are state, not routes, has no way to read such a link today, and an app with routes cannot tell two schemes apart or read a fragment.

### Current and expected behavior

Current (macOS, main `0365ad1a4`; the files involved are unchanged on `e200397ec`):
- Warm (a `GetURL` Apple event, as Launch Services sends for `open <url>`): the app sees nothing; the journal says `navigate refused: no navigation root handler`.
- Cold (the launch URL): the app sees nothing; the journal says `launch URL refused: no navigation root handler (/auth/x?request=1)`.
- With a navigation root, `navigate` gets `/auth/x?request=1`: `location_of` maps a non-HTTP URL to `/` + authority + path + query (`route/src/location.rs:27`), so the scheme and the fragment are gone.
- Source: `ExactSession.openURL` (`host/apple/Sources/ExactKit/Session.swift:1396-1401`) converts the URL with `location(of:)` and calls the navigation root's `navigate`, or stores it as the launch location; `LaunchURL.swift:14-18` journals the refusal. `ExactModule` has no URL callback (`host/apple/modules/ExactNativeModule.swift`), and `app.schema.json` has no `protocol_handlers`.

Expected: the app can receive the full URL string (scheme, authority, path, query, fragment) once per open, warm and cold, whether or not it declares a navigation root. The web's model is the Web App Manifest's `protocol_handlers` (and `navigator.registerProtocolHandler()`): the browser opens a handler URL with the whole link percent-encoded in it.

Proposals (hypotheses; the choice is the decision below):
- A, the web's model: `app.json` `protocol_handlers` with a URL template; the host opens that location with the full URL in a parameter, also without a navigation root, and at launch.
- B: an app-level event or reserved source that carries the full URL once per open, usable without `routes`.
- C: an `ExactModule` callback (for example `open(url:) -> Bool`) for apps that handle links in native code.

### Reproduction and evidence

App: `bun scripts/exact.mjs new <dir>`; `app.json` adds `"host": {"macos": {"urlSchemes": ["exactreprox05"]}}`; `app.ts` answers no sources; no navigation root:

```contract
component RestX05
  state got = "none"
  view
    main testId="root" padding=24
      text `received: ${got}` testId="got"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Cold launch URL | `bun exact.mjs mac`; `bun exact.mjs agent macos --url "exactreprox05://auth/x?request=1#frag" tree logs` | macOS 26.6.2, Apple Silicon | `0365ad1a4` | `received: none`; `launch URL refused: no navigation root handler (/auth/x?request=1)` | the app reads `exactreprox05://auth/x?request=1#frag` | journal lines quoted |
| Warm URL | the app under the agent (`open()` from `scripts/agent.mjs`), then a `GetURL` Apple event with the URL to its pid (`AESendMessage`, class and id `'GURL'`) | macOS 26.6.2 | `0365ad1a4` | `navigate refused: no navigation root handler`; nothing reaches the app | the app reads the full URL once | journal lines quoted |
| With a navigation root | the same app with `routes`, `navigate=follow` on the root (#104's variant B) | macOS | `4c893fef6` (#104), code unchanged since | `navigate` hears `/auth/x?request=1` | the full URL, scheme and fragment kept | #104 |

### Acceptance criteria

- On macOS, warm and cold, an app with no navigation root reads the exact string `exactreprox05://auth/x?request=1#frag`; the agent's `tree` and `state` show it.
- A 16,384-character URL arrives unchanged; two links in quick succession arrive as two deliveries, in order; a scheme the app does not declare is not delivered.
- An app with a navigation root and no new declaration keeps today's behavior (LLP 1038 D8).
- iOS, which shares `ExactSession.openURL`, follows the same rule; the web follows `protocol_handlers` where the browser supports it.

### Constraints and related work

- Workaround: declare `routes` and a navigation root only to catch links, and pack everything into path and query (no scheme, no fragment). An app whose screens are state cannot avoid adding a router.
- Governing design: LLP 1038 D8 ("An incoming URL is one `navigate` event"; `llp/1038-router.rfc.md:478-493`).
- Not tested: iOS, Linux, a second scheme, URL length limits.
- Related: #104 (closed by #201, which journals the cold refusal only).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:18:33Z

## Decision needed

**Blocking rule.** LLP 1038 D8 (accepted, `llp/1038-router.rfc.md:478-493`): an incoming URL is one `navigate` event, only the navigation root may carry the handler (`lower-navigate-root`), and the location is `location_of`'s, which drops the scheme and the fragment of a non-HTTP URL (`route/src/location.rs:27-38`). Delivering the whole URL, or delivering it without a navigation root, departs from D8.

**Options.**
- **A, the web's `protocol_handlers`.** `app.json` declares `{ "protocol": "<scheme>", "url": "/open?u=%s" }`; the host turns a URL of that scheme into that location, with the whole URL percent-encoded in the parameter, warm and cold, through D8's `navigate`. Keeps one delivery path; an app still needs a navigation root and a route for it.
- **B, an event on the root.** A new root event (for example `openurl=action(url)`) with the raw URL as its string payload, heard without `routes`; the launch URL is delivered once after boot.
- **C, a module callback.** `ExactModule` gains `open(url:) -> Bool`; native code only, invisible to Contract and the agent.

**Recommendation.** A. It is the web's own name and shape, the web host can emit the same manifest member, the URL reaches the router D8 already defines, and cold and warm share one path. If requiring a router for a link-only app is judged too heavy, add B's event as well rather than C.

**Cost.** A: schema key, the handler template applied in `ExactSession.openURL` for declared schemes (shared by macOS and iOS), the web manifest member, tests for warm, cold, length and fragment; about a day. B: a new `EventKind` in the plan tables, compiler, runner and three hosts; about two days. C: half a day, but Swift-only and not drivable by the agent.

### ccheever — 2026-10-08T08:07:40Z

**Decision: Choose protocol_handlers templates over the existing navigate path.**

Keep open with the bounded scope below.

Percent-encoding the whole original URL in a declared handler location preserves scheme and fragment without a second delivery mechanism.

Explicitly narrow the request: this choice still requires a navigation root and does not satisfy the no-router criterion. Test warm/cold delivery, rapid links and length; if no-router delivery remains required, select a root event separately.
