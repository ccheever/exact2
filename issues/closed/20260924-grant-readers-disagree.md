# The web and the runner still read grants a native host refuses

**Status:** Closed — implementation and automated parity checks pass; one required live producer proof remains
**Resolution:** One Rust parse now emits the typed, complete grant set both web targets consume; Exact API requests, stores and files follow native admission and diagnostics. Closure waits for the Weatherlight JS build-and-boot drive below.
**Systems:** Web host, runner store, bake, grants
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report F1 (2026-09-24); vendor/ibex2/src/grant.rs; js/src/lib.rs Module::inspect

Seth's Crew port (report of 2026-09-24, F1) declared `secret.keep crewHost`. Native hosts parse the whole grant set with `ibex2::grant::GrantSet::parse`, which refuses a camelCase secret name, and before this report they then held no grants at all, `net.fetch` included, with no message. The same report's fix makes the TypeScript bake refuse such grants (`Module::inspect`, `js/src/lib.rs`), makes native hosts journal the parse error and name it in every refusal, and renames `apps/realworld`'s `jwtToken` to `realworld.jwt`.

Two readers still disagree with the grammar, one line at a time and never refusing:

- `runner/src/store.rs` `Store::new` takes any `secret.keep <name>` as granted.
- `host/web/glue.js` `granted()` takes any `net.fetch <origin>` line; its comment promises "the same refusal everywhere".

With the bake refusing, an app built by the scripts can no longer reach them with a bad set. But hand-built grants still can, and a Rust-only app's grant constants are never validated at bake: `exact_bake::compatibility_id_sources` (`bake/src/compat.rs`) hashes them without parsing, because `exact-bake` does not depend on ibex2.

Done when a Rust-only app whose grants do not parse fails its bake naming the line, as a TypeScript app's now does, and the web and runner readers take their grants from one parse of the whole set (or refuse the set as native hosts do) rather than line by line.

## Verification (2026-10-03, review round 3)

`exact-grants` remains the grammar. The runner serializes its typed result for
the bake and wasm batch, including every raw nonblank line so comments and
`auth.*`, `surface.*` and `device.*` survive native-style child scoping. The
page validates and matches that typed form on every use; a brand is not an
authority. Parser-produced sets and all nested tuples are frozen, and the app's
set is module-private rather than exposed through `globalThis.exact`. A merely
plausible object is deny-all. A malformed unscoped set is deny-all with the
native line diagnostic, while a valid child selected from that raw parent
follows `storage::scope`. Module declarations compare normalized forms, so
whitespace accepted by the Rust parser does not prevent activation.

Both web request paths now apply the same child set and follow redirects. After
the fetch completes they admit the response's final URL; an ungranted final
origin is discarded and reported as `Refused`, with native's `outside the app's
grants (net.fetch)` message. Browser fetch does not reveal redirect targets in
manual mode, so the web cannot refuse before an intermediate hop is contacted.
When a redirect hop is rejected by the browser before any `Response` exists —
including a CORS rejection or unreachable destination — the page cannot observe
that destination and reports `Network`; unlike native, it cannot perform the
per-hop check. This is a declared web deviation. Every redirect for which a
`Response` exists keeps the rule above. Grants are parity, not a security
boundary: app code is the page and can call the browser's fetch directly.

The JS target reports `FetchError` with string `FailureKind`, gives Rust an
absent scope as that Rust executor's own set, and applies the same admission to
portable storage, file operations and secrets. The TypeScript store returns
`null` for absent values, reserves `exact.kept.*`, and counts key reads.
Reserved `exact.kept.*` reads return none without counting a store dependency.
Malformed TypeScript URLs are `FetchError("Network", ...)`. A bodyless
same-origin `/assets/...` GET remains host I/O on both targets. Relative URLs
other than that exemption are `Network`, as on native and wasm. Admission is by
the requested operation (`fetch` or WebSocket), not a URL's scheme; explicit
port zero and the Rust parser's full whitespace set survive normalization.
Malformed browser headers are also `FetchError("Network", ...)`.

The source fetch binding is lexical: it never replaces the page's browser
fetch, so host modules loaded after first paint retain host authority. The JS
build discovers and copies every transitive import of its raw lazy modules; a
clean Fieldnotes build imports the storage, SQLite, file and document roots
from the resulting dist. The SQLite worker declares its `sqlite3.wasm` URL, so
the binary is part of that lazy graph rather than an unrelated hand copy.

Tests run the production wasm request and early-fetch path, the `glue.js`
`grants` arm, the Rust ABI decoder and executor in `rust-data.js`, the
TypeScript facade in `ts-data.js`, `module-glue.js` preparation and activation,
and a late import of the real `gpu-glue.js` after TypeScript data installation.
The latter observes a shader fetch through the browser binding. Temporary
removal of each connection made its named test fail before the source was
restored. Specifically, removing the seal check failed the wrong-seal case;
removing subdomain matching failed the Rust corpus probes; dropping the Rust
ABI scope decode failed the child-scope request; removing the portable-storage
lazy root failed the clean-dist import; allowing source/tuple substitution
failed the self-sealed-object case; inferring the capability from `ws:` failed
the operation test; refusing port zero failed the port-zero case; resolving
relative URLs failed the relative-URL case; dropping the U+2028/U+2029 seal
spelling failed the separator case; moving header validation outside the
`FetchError` conversion failed the invalid-header case; and excluding `.wasm`
dependencies failed the clean-dist `sqlite3.wasm` assertion. Removing the
production `files.js` prefix check changed its refusal into an agent export
hold and failed the file-command test. The Chrome-gated `exact-js-web`
browser suite now gives `prepare`, `createStorageRequests`, `createFileSystem`
and `createSqlite` the same normalized grant-set interface production uses.
Disabling the production app-source transform makes the built TypeScript
Chrome probe contact its ungranted endpoint and return `raw browser fetch`
instead of `FetchError:Refused`; restoring the lexical binding passes it.
Restoring raw worker grant-string comparison makes its formatted worker fixture
fail with `module exports mismatch`, while the normalized comparison passes.
With `$CHROME` set, `cargo test -p exact-js-web --test browser -- --nocapture`
passes all three tests, including Fieldnotes create/reload storage and main and
worker module placement.

Chrome drives exercised Fieldnotes create/save/reopen, auth-fixture through
"Ready to sign in" (the agent-controlled auth smoke passed 14/14), Messages'
conversation and persisted draft, and RealWorld's feed and sign-in route with
no grant refusal. The separate auth popup smoke is unsupported by this
headless browser and timed out after the agent-controlled flow passed.

The strict three-app conformance command was attempted on this producer but
could not compare any steps: all three wasm oracle builds stopped in
`host/web/build.mjs` at `buildBake`; the subsequent Fieldnotes and Messages JS
builds refused those stale `/tmp/e3-wasm` plans with `FormatDigestMismatch`.
RealWorld's independent JS build succeeded. This is outside the grant diff,
and the focused JS, wasm-path and Chrome browser tests above are green.

### Still required before closing

Build and boot Weatherlight's JS target on a producer with the pinned lean
Hermes executor, and confirm its late host shader requests have no grant
refusal. This Linux checkout has no Hermes library: the normal build stops in
`exact-js` with `no Hermes for linux`; `EXACT_JS_ENGINE=stub` is correctly
rejected by Weatherlight's TypeScript bake. Consequently no runnable artifact
exists to drive here. The focused late-GPU production test passes, and this
Chrome would still be expected to report no WebGPU adapter, but that automated
proof does not substitute for the required live build-and-boot drive.

### Closed 2026-10-02 (the Mac/Hermes proof)

On the M5 mini (macOS, the pinned lean Hermes, Chrome for Testing 153), at
`dcd938ed2`: `bun host/web/build.mjs weatherlight` built (one page rendered by
`weatherlight-render`) and `bun scripts/agent.mjs web "clock settle" state logs
"screenshot …"` booted it with no grant refusal in its logs. Conformance with
the wasm oracle, `conform.mjs realworld fieldnotes messages weatherlight
auth-fixture --build --strict`: 52 of 53 steps equal (RealWorld 21/21,
Messages 4/4, Weatherlight 12/12, auth-fixture 5/5); the one difference,
Fieldnotes' `tap save-backup` leaving the button focused on the JS target, is
the same on origin/main (`1ed9e1f7`), not this change.

Landed with both final reviews still marking edge cases; they are in QUEUE
("JS-target grant parity, what is left"), and the hostile-page cases are out
of scope by the ruling above (on the web, grants are parity, not a sandbox).
