# The web-root publish is neither atomic nor replacing

**Status:** Closed
**Resolution:** Publish a complete immutable web release and atomically switch its guarded inventory pointer; the supported origin server binds browser/native graphs to release URLs, suppresses removed canonical paths including AASA, and enforces immutable/no-store caching. Failure injection at every write, concurrent HTTP graph readers, and a deployed Caltrain browser/deck drive pass.
**Systems:** Delivery, exact deploy, Web host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030 D3; LLP 1030.000 D3 item 5

The web-root publisher writes every changed fixed-name file in sequence and
sorts only `index.html` last (`scripts/deploy.mjs:450-455`). That is not an
atomic release: the old index still names mutable `glue.js` and `app.wasm`
while those are being overwritten, and a failure before the last write leaves
a permanently mixed root. Native URL clients can likewise observe root
`exact.json` and `app.plan` from different publish attempts.

The classifier walks only files present in the new bake
(`scripts/deploy.mjs:301-310`). It never reports or removes an origin-only
file. Removing the generated Apple association file, an asset, shader, deck
page, or manifest output leaves it publicly served forever. In particular a
stale `.well-known/apple-app-site-association` can keep claiming a domain
after the app manifest stops doing so.

There is also no realized cache policy: `DirectoryOrigin.put()` stores only
bytes, while the in-repo static server sends `no-store` for every file
(`host/web/serve.mjs:112-117`). LLP 1030.000 D3 requires immutable caching for
content-addressed payloads and no-store only for mutable pointers/well-known
files.

Done when a fresh reader sees either a complete prior root or a complete new
root, removed files cease being reachable, and cache behavior is generated or
served as declared. Prefer release/content-addressed names plus one atomic
pointer; if fixed convenience names remain, they cannot be the live graph.
Add failure injection after each write and a concurrent-reader test, plus an
association-file removal case. The stream-side form is tracked separately by
`stream-files-overwritten-before-head`.
