---
name: 20261005-x44-remote-image-policy
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-provider-settings-upkeep]
upstream_url: https://github.com/ccheever/exact2/issues/121
reproduced_on: null
---

# X44: Remote `image` loading policy (credentials, referrer, redirects, size cap, cache, load state) and remote SVG

## Summary

T3 Code shows the icon of each ACP registry agent from an official CDN, fetched under strict rules: only the allow-listed https host, no credentials, no referrer, no redirects, an image content type, at most 512 KB, a persistent cache, a fallback glyph while loading or after a failure, and a recolor of the monochrome glyph to the theme. Exact2's `image` can show an https URL (the clone's wizard already does), but the bundled library says nothing about what it sends, follows, caps or caches, whether it renders remote SVG, or whether the Contract can see the load state. The requested support is a documented and tested policy for remote `image` loads, plus load-state facts.

## Why this issue arose

### The T3 Code behavior
- **Where.** ACP search results and the wizard's selected agent, every provider instance icon (list, picker, composer), built from the saved `registryIconUrl` or the agent id (`apps/web/src/components/settings/AcpRegistryIcon.tsx:73-170`, `components/chat/ProviderInstanceIcon.tsx:56-100`, `providerInstances.ts:214`, `components/settings/AddProviderInstanceDialog.tsx:227`).
- **Allow-list.** Only `https://cdn.agentclientprotocol.com/...` without port, username or password; an agent id must match `^[a-z0-9][a-z0-9._-]*$` and maps to `https://cdn.agentclientprotocol.com/registry/v1/latest/<id>.svg` (`packages/contracts/src/acpRegistry.ts:13-44`).
- **Fetch policy.** `fetch(url, {credentials: "omit", redirect: "error", referrerPolicy: "no-referrer"})`; the response must be ok and not redirected, its content type must start with `image/`, and `content-length` and the body must be at most 512 KiB (`AcpRegistryIcon.tsx:11,16-28,54-60`).
- **Cache.** The `CacheStorage` entry `t3-acp-registry-icons-v1` keeps the response; later reads come from it; concurrent loads share one fetch; an unreadable cache falls back to the network (`:10,30-70`).
- **States.** Loading and failed show the generic ACP glyph (`data-slot="acp-icon-fallback"`); a validated blob URL is drawn once it loads; if the validating fetch is blocked (the CDN sends no CORS headers) the raw allow-listed URL is tried in an `<img>` with `referrerPolicy="no-referrer"`; an `error` marks the load failed and keeps the glyph (`:73-170`).
- **Recolor.** The glyph is monochrome; the component recolors its alpha with a local SVG filter (`feFlood currentColor`, `feComposite SourceAlpha`) so it follows the theme (`:128-136`).
- Reference tests: `AcpRegistryIcon.test.ts` ("single-flights the first fetch and serves later reads from persistent cache" `:43`, "rejects oversized image responses before caching" `:65`, "falls back to the network when CacheStorage cannot be opened" `:81`, "accepts only credential-free HTTPS URLs on the official CDN" `:105`, "falls back to the raw allowlisted CDN URL when the validating fetch is blocked" `:129`, "renders only the validated blob object URL and keeps the fallback until load" `:147`).

### What exact2 does today
- Not in `EXACT2-GAPS.md` as an item. The nearest quoted facts: X7: "WKWebView blocks plain `http://` to hosts that are not IP addresses (App Transport Security; r12-render finding). `app.json` cannot set ATS keys." (about web views, not `image`); X29: "`image` takes `app:/`."
- The bundled library (`20261005-platforms-v3`) does not cover remote `image` loading, remote SVG, or load-state facts: unknown.
- Observed in the clone (mc-orch tree, 2026-10-05): the wizard draws the search result's `agent.icon` with `image agent.icon width=24 height=24 object-fit="contain" tint-color=...` (`providers-wizard.contract:194-195`), so an https `image` with `tint-color` is authored; whether it draws in the pinned build, and its runtime policy, were not checked for this issue (to confirm at `issue-open`). The allow-list regular expression is applied only when the icon URL is saved as `registryIconUrl` (`providers.ts:459`), not when it is drawn (`providers.ts:233` passes the server's value through). Instance icons never use the saved URL: they use a generic glyph (`provider-icons.contract:45`).

### Where the clone hits it
`20261005-provider-settings-upkeep` (scope item "ACP icons") must draw `registryIconUrl` wherever an instance icon appears, in 9 `DriverMark` sites. Without a documented policy the clone cannot show the reference's guarantees: that no cookie or referrer reaches the CDN, that a redirect to another host is refused, that a 5 MB image is refused, and that the icon survives a relaunch offline. It also cannot show the fallback glyph while loading or after a failure unless the Contract sees the load state. Workaround A: a Swift fetch in the app module that applies all rules and writes the validated bytes under the data folder, then `image app:/…` (the app already fetches natively for `T3ImageAccent.swift`); needs a writable, loadable `app:/` path (to confirm). Workaround B: keep the plain `image <url>` and declare the policy difference. B shows the same icon in the happy path.

## Why it must be resolved

The goal is the reference's behavior including its security rules, because the registry is third-party content and the rules are tested in the reference. Impact: unknown until `issue-open` checks the host's default behavior; if the defaults already omit credentials, refuse redirects and cache, the issue closes as documentation plus a load-state fact. Waiting: the "ACP icons" rows and the instance-icon pixel cells of `20261005-provider-settings-upkeep`. Cost of workaround A: a Swift module, a cache-eviction policy and new tests; cost of B: an undocumented network behavior shipped to users and a rendering state (loading/failed) the Contract cannot express.

## Requested support

On the macOS host first, the web way:
- **A (preferred):** `image` over https behaves as `fetch(url, {credentials: "omit", redirect: "error", referrerPolicy: "no-referrer"})` for authored remote sources (or exposes these as props), caps the body (a `max-bytes` prop, default documented), requires an `image/*` content type, keeps a persistent cache with HTTP semantics, renders remote SVG, and reports `loading`, `loaded` and `failed` to the Contract (web: `<img>` `load` / `error` events, `referrerpolicy`).
- **B:** keep `image` as is and document its policy; the app does the fetch rules natively and passes `app:/` paths.
Other hosts: web already has the platform behavior; iOS follows the macOS choice.

## How to reproduce

To confirm on the pinned `main` at `issue-open`. A one-node app `image "https://cdn.agentclientprotocol.com/registry/v1/latest/<id>.svg" width=24 height=24`, with a local server that logs requests: check `Cookie` and `Referer` headers, answer a 302 to a second host, answer `content-length: 5000000`, answer `content-type: text/html`. Reference (Chrome via the policy above): no cookie, no referer, the redirect and the oversized body refused, a failed load keeps the fallback. Expected on exact2: unknown for each case; also whether the SVG draws at all. Clone scenario: Settings › Providers › Add provider › ACP, search, compare the icon rows with the oracle.

## Acceptance for the fix

- The local-server cases above, each with the documented outcome; headers logged per request.
- Remote SVG draws and `tint-color` recolors it (screenshot pair against the oracle).
- The Contract shows a fallback while loading and after a failure (`state` reports the load fact); an offline relaunch still shows a previously loaded icon.
- AppKit test for the redirect and size rules; conformance case where the web has an equivalent.

## App adoption after resolution

In `20261005-provider-settings-upkeep`: draw `registryIconUrl` (or the agent-id URL) at every instance-icon site through a shared component with the fallback glyph; apply the allow-list function when drawing as well as when saving; port the six tests above to logic tests. Remove workaround A if built. `issue-close` verifies: the policy cases against a local server, icons in list, picker and wizard, and the offline relaunch.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Rest checked upstream (2026-10-08)

Upstream: the rest (a remote SVG `image` on Apple) reproduced on main `0365ad1a4`: macOS raises `error` "not an image format this host decodes" for a remote SVG and for a bundled `assets/icon.svg` and draws nothing, while Chrome draws both and a remote PNG loads on both. Not filed: open PR https://github.com/ccheever/exact2/pull/239 ("Support efficient SVG image decoding on Apple hosts", not merged) decodes SVG for any `image` source on Apple, remote included. Parent: #121, closed by #177 (load/error, fetch policy documented).

Next: when #239 merges, re-check the remote SVG on macOS and adopt it.
