# Vendored ibex2 and ibex2-sqlite — Exact patches

- **Upstream:** `https://github.com/expo/ibex.git`, commit
  `fbe2baee` (2026-10-04, on `main`):
  `crates/ibex2` → `vendor/ibex2`, `crates/ibex2-sqlite` → `vendor/ibex2-sqlite`.
- **Why vendored (Charlie, 2026-09-22):** a fresh clone must build without a
  sibling `../ibex` checkout. The compiler includes `src/bindings/storage.d.ts`
  as text, `exact-js` compiles `src/engine/ibex2_jsi.cc` and the binding
  scripts, and seven manifests depend on the crates.
- **Patches:** two, below, both Exact only. Otherwise the copy is the
  commit's tracked tree, byte for byte, plus this file.
- **Not vendored:** the Hermes engine and `hermesc` builds. They are
  hand-built outputs in the ibex checkout (`ios/Frameworks-vanilla`,
  `tools/hermes-vanilla`, `linux-vanilla`), needed only by `exact-js`.
- **Update:** from a clean ibex checkout at the new commit,

  ```sh
  rm -rf vendor/ibex2 vendor/ibex2-sqlite
  git -C ../ibex archive --format=tar <commit> crates/ibex2 crates/ibex2-sqlite \
    | tar -x -C vendor --strip-components=1
  ```

  then restore this file with the new commit and date, and reapply patches
  3 and 4 (`git show ae0c186a9 a268b5512 001e43d03 -- vendor/ibex2`).
  Patch 4 replaces `src/grant.rs` wholesale, so keep the vendored file
  rather than merging upstream's: a grant-grammar change upstream must be
  ported to `grants/src/lib.rs` by hand.

## Upstreamed (no longer patches)

Patches 1 and 2 landed in ibex as `fbe2baee` (2026-10-04), byte for byte
except for code comments retargeted to Ibex LLPs (LLP 0067 §2, LLP 0059.000
§3.5 and §3.12). Upstreaming them added two things:

- **Userinfo is refused in origin grants.** `https://*.example.com@evil.com`
  had parsed as `*.evil.com`, and `https://api.example.com@evil.com` as
  `evil.com`. Both parsers now refuse any `net.fetch`/`net.websocket`
  target containing `@`: ibex2's, and `grants/src/lib.rs` with
  `host/web/navigation.js`'s `networkTuple`, held together by
  `host/web/tests/fixtures/grants.json`.
- **`fetch_limits` pins the new redirect rule.** It had pinned credentials
  surviving a granted cross-origin hop. exact2 never ran that Hermes-gated
  test against its copy.

1. *Subdomain `net.fetch` grants; credentials dropped on a cross-origin
   redirect* (12bacca23, Charlie 2026-09-26, LLP 1054.000 R5).
2. *A listening WebSocket under `net.websocket`* (0736a4ab7, LLP 1069.004
   slice 3).

## Patch 3: the rustls transport builds on macOS — Exact only

Charlie, 2026-09-29 (LLP 1048.000 D10, "The server's transport"): the
render host, a server rather than an app on a device, fetches over rustls on
macOS too.

- `Cargo.toml`: the rustls transport's dependencies (`ureq`, `rustls`,
  `rustls-native-certs`, `webpki-roots`, `socket2`) are target dependencies
  everywhere but Apple's device platforms (iOS, tvOS, watchOS, visionOS),
  where they were everywhere but Apple.
- `src/transport/mod.rs`: `rustls_http` and `RustlsHttpTransport` build on
  macOS too. `default_transport` is unchanged: `NSURLSession` on Apple.
- `src/transport/rustls_http.rs`: the trust store is read once per process
  (reading the macOS keychain took ~170 ms, and every transport paid it: a
  render server's first render on each worker, 2026-09-30), and a pending
  connect waits in `poll(2)` for the socket to be writable instead of
  sleeping 10 ms between checks.
- Not for upstream: it exists for one embedder's server role.

## Patch 4: one grant grammar shared with the web runner — Exact only

It also carries the `doc:/` path namespace (80dbd7642, LLP 1069.010 D1, the
documents a person chose, resolved by the host beside `app:/`), which went
into `grant.rs` before patch 4 moved the grammar and was never listed here.
It lives in `grants/src/lib.rs` now.

2026-10-02, LLP 1016 D6 / LLP 1018 D3: `src/grant.rs` reexports
`exact-grants` (`../../grants`), the previous parser and authority types moved
unchanged into a pure crate. URL normalization remains the `url` crate's,
including IDNA and address normalization. Secret/scope name validation is
shared too. Native filesystem realization stays here: the executor maps
filesystem prefixes through its resolver; the shared crate performs no I/O.

The runner, Rust-only bakes, and wasm host now use the same parser as native
bindings. One invalid I/O line grants nothing. The current JS target holds
the same grammar against `host/web/tests/fixtures/grants.json` using browser
URLs, and scopes module fetch and secret access to the parsed set.
