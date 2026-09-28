# Vendored ibex2 and ibex2-sqlite — Exact patches

- **Upstream:** `https://github.com/expo/ibex.git`, commit
  `639de62de0ba473417dd85b8f4c61aa08dc07a78` (2026-09-11, on `main`):
  `crates/ibex2` → `vendor/ibex2`, `crates/ibex2-sqlite` → `vendor/ibex2-sqlite`.
- **Why vendored (Charlie, 2026-09-22):** a fresh clone must build without a
  sibling `../ibex` checkout. The compiler includes `src/bindings/storage.d.ts`
  as text, `exact-js` compiles `src/engine/ibex2_jsi.cc` and the binding
  scripts, and seven manifests depend on the crates.
- **Patches:** two, below. Otherwise the copy is the commit's tracked tree,
  byte for byte, plus this file.
- **Not vendored:** the Hermes engine and `hermesc` builds. They are
  hand-built outputs in the ibex checkout (`ios/Frameworks-vanilla`,
  `tools/hermes-vanilla`, `linux-vanilla`), needed only by `exact-js`.
- **Update:** from a clean ibex checkout at the new commit,

  ```sh
  rm -rf vendor/ibex2 vendor/ibex2-sqlite
  git -C ../ibex archive --format=tar <commit> crates/ibex2 crates/ibex2-sqlite \
    | tar -x -C vendor --strip-components=1
  ```

  then restore this file with the new commit and date.

## Patch 1: subdomain `net.fetch` grants; credentials dropped on a cross-origin redirect — to upstream

Charlie, 2026-09-26 (LLP 1054.000 R5): "a wildcard grant is probably worth
it actually, and the developer should just use it carefully." An AT
Protocol account lives on one of many hosts (`*.host.bsky.network`), and
going through the entryway costs a hop.

- `src/grant.rs`: `Grant::FetchSubdomains(Origin)`, from `net.fetch
  scheme://*.domain[:port]`. It admits a host strictly under `domain` at
  that scheme and port, and never `domain` itself. `*` must be the whole
  leftmost label, the domain needs two labels or more and cannot be an
  address, and `*` anywhere else, or on `net.websocket`, refuses the grant
  line. Hosts compare as the URL parser normalized them (lowercase,
  punycode); a trailing dot does not match.
- `src/stdlib/fetch.rs`: a followed redirect to another origin drops
  `Authorization`, `Cookie` and `Proxy-Authorization`, as the Fetch
  standard does for `Authorization`. With subdomain grants, a sibling host
  is admitted, and it must not receive a token meant for the first.
- Tests: `grant::tests::a_subdomain_*`,
  `stdlib::fetch::tests::a_cross_origin_redirect_drops_credentials_*`.
- The web glue's `grantAdmits` (`host/web/glue.js`, copied in
  `module-glue.js`) applies the same rule, and the render server's CSP
  passes the pattern through as CSP's own `*.` (subdomains only).
- There is no public-suffix check: `*.co.uk` or `*.github.io` would parse.
  The app writer is trusted to name a domain they mean.

## Patch 2: a listening WebSocket under `net.websocket` — to upstream

Exact's LLP 1069.004 slice 3 (2026-09-27): Bluesky's Jetstream is the
consumer, and `net.websocket <origin>` was parsed and checked but opened
nothing.

- `src/stdlib/websocket.rs`: `Incoming` (text, binary, too large, closed),
  `MessageSource`, `SocketTransport`, and `open`, which admits
  `Operation::WebSocket` for a `ws:`/`wss:` URL on every open (a handshake
  follows no redirect). `accept_key` (RFC 6455's SHA-1 accept) for a
  client and a test peer. Receive-only: nothing is sent but the handshake,
  a pong, and the closing handshake.
- `src/host.rs`: `Bindings::websocket` carries the grant, over the host's
  socket transport (`Host::with_socket_transport` for a test's own).
- `src/transport/darwin_websocket.rs` + `src/engine/darwin_websocket.mm`:
  `NSURLSessionWebSocketTask` on Apple, an ephemeral session per socket that
  refuses redirects; the ceiling is the task's `maximumMessageSize`.
- `src/transport/websocket.rs`: off Apple, TCP (the rustls transport's
  cancellable connect) and rustls with the same native trust store,
  `webpki-roots` where the machine has none; loaded on the first `wss:`.
  Built in tests on Apple too.
- Tests: `stdlib::websocket::tests`, `transport::websocket::tests` (a local
  peer: upgrade, fragments, a ping, extended length, the closing handshake,
  an over-limit and a binary message, a refused handshake, a dropped
  connection, an abort) run on both transports.

