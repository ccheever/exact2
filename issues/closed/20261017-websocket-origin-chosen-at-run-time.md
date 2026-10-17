# A WebSocket a data module sends on needs an origin chosen at run time

**Status:** Closed
**Resolution:** Built as ruled: a net.fetch or net.websocket line may name a whole scheme as a CSP scheme source (http:, https:, ws:, wss:), admitting every host and port of it; any other scheme in that form is refused, and a scheme no line names still is. grants/src/lib.rs and the vendored Ibex grammar (vendor/ibex/EXACT-PATCHES.md) parse and admit it, the runner seals it for the browser and host/web/navigation.js matches it on both web targets, and the render server's connect-src copies it as a scheme source. state.streams names the line that admitted each stream (grant) on every host. The render server doesn't hold a scheme-wide net.fetch line: a source that needs one keeps its placeholder and the client fetches it after adoption. Driven on the web JS target and macOS against a Bun echo server on a port no line names: the fetch and the socket worked under net.fetch http: and net.websocket ws:, state.streams read grant net.websocket ws:, and under net.websocket wss: the ws: socket was refused.
**Systems:** grants, data modules, runner streams
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** issues/20261009-runner-owned-bidirectional-streams.md, LLP 1016.000 D7, LLP 1069.008, issues/20261017-socket-reply-in-the-frames-turn.md

## Problem

Data modules can send on a WebSocket (LLP 1016.000 D7), but the T3 Code clone's RPC transport still cannot move out of its Swift module (`T3Transport.swift`, `T3Fleet.swift`, `R7DeviceClient.swift`, about 2,000 lines; its `app.ts` grants nothing). Its servers are decided at run time: the embedded local server, on a port chosen at each launch; remote environments the person adds by URL; Tailscale HTTPS names under the person's tailnet. No grant line can name them when the app is built. The streams issue calls this "a separate gap" in its notes; this is its record.

## Cause

A grant is one exact origin, fixed in `app.ts`'s `grants` and checked against what the bake recorded (`js/src/lib.rs:459`). `GrantSet::parse` refuses `*` in a `net.websocket` target (`grants/src/lib.rs:374-381`; and its vendored copy, `vendor/ibex/crates/ibex2/src/grant.rs:369-372`, which the native executor parses per request, `host/apple/src/executor_stream.rs:60-66`), and `Grant::admits` compares a socket's origin by equality (`grants/src/lib.rs:191`). The one pattern, `scheme://*.domain` (Exact patch 1), is `net.fetch`'s alone (`grants/src/lib.rs:369`). The web host refuses a starred socket line too (`host/web/navigation.js:1300`), and the page's CSP `connect-src` is built from the same lines when served (`host/render/src/serve.rs:1448`). There is no port range, no run-time form, and no way for a host module to add an origin.

## Options

1. **An origin the person confirms at run time.** The app asks for one (`requestOrigin(url)`); the host, not the module, shows the origin and asks; a yes is stored with the app's data, listed in `state`, revocable, and checked by the same `GrantSet`. Covers typed remote URLs and Tailscale names. On the web it needs a `connect-src` that admits it, which a served CSP cannot widen after load.
2. **Loopback at any port** (`net.websocket ws://127.0.0.1:*`, and `net.fetch`): for a helper server the app itself runs. Small, but admits every local service.
3. **An origin a native module grants**: the module that started the helper hands its origin to the runner. Native code per app, which is what D7 set out to remove.
4. **`*.domain` for `net.websocket`**: covers a fixed suffix, not a tailnet or a URL the person types.

Recommendation: 1. It is the only form that covers origins the person names, and the person is the authority for them, as a browser user typing a server's address is. The embedded server's changing port then wants 2.

## Acceptance

A data module opens and sends on a socket to an origin no build-time line names, after the run-time step chosen; an origin outside it is refused on every host, the web JS target included; `state` lists what was admitted and how.

## Ruling (Charlie, 2026-10-16)

No run-time step: "don't mobile operating systems already implement some kind of security
around this? i'm not sure i see the need to lock it down more at this time." The platforms
gate kinds of connection, not hosts: App Transport Security refuses plaintext to the
internet, the Local Network prompt asks before an app reaches the LAN (iOS 14, macOS 15),
Android refuses cleartext by default from API 28, and a page's `connect-src` is its
author's. One exact origin per line is already stricter than each of them, and a prompt
after the person typed the address is one no hand-built app shows.

So a build-time line may admit every origin of a scheme, written as CSP writes a scheme
source: `net.websocket ws:`, `net.websocket wss:`, `net.fetch http:`, `net.fetch https:`.
Both families take it, since T3 fetches its socket ticket from the same server it then
opens (`T3Transport.swift` `openSocket`, `/api/auth/websocket-ticket`). The served
`connect-src` copies the line as a scheme source. The embedded server's port and the
person's typed servers are both covered, so no native module grants an origin and no
option above is built. The platform's own refusals still apply: a browser refuses `ws:`
from an `https:` page except to loopback, and ATS refuses plaintext beyond the local
network.

The parsers move together: `GrantSet::parse` (`grants/src/lib.rs:369-381`), its vendored
copy (`vendor/ibex/crates/ibex2/src/grant.rs:369-372`), the web host's
(`host/web/navigation.js:1300`), and `csp()` (`host/render/src/serve.rs:1448`), which
copies a target verbatim today.

Acceptance, replacing the above: under a scheme-wide line, a data module fetches from and
opens a socket to an origin no exact line names, on every host, the web JS target
included; a scheme no line names is refused; `state.streams` names the line that admitted
the socket.
