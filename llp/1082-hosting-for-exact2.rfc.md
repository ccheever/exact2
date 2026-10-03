# LLP 1082: Hosting for exact2 — one origin for the web app, its rendered pages, and its updates

**Type:** RFC
**Status:** Draft r1, for Phil to build against. Nothing here is ruled; §8 lists what needs a ruling.
**Systems:**
- Render server (`host/render`, LLP 1048.000 D9–D11, 1048.004): an app's `<app>-render --serve`, run as a hosted process.
- Delivery (LLP 1026, 1030, 1030.000, 1030.003): the origin the publisher writes and every client reads — the web root, `.exact/` streams, blobs, install pages.
- One URL (LLP 1023): the web app, the update streams and the deep-link authority on one origin.
- `scripts/origin.mjs` and `scripts/deploy.mjs`: gain the hosted adapter (§6).
- EAS Hosting: what of it this reuses, and what is new (§2).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Phil (hosting), from when Charlie hands it over (asked 2026-10-03); the exact2-side items in §6 by whichever lane Charlie names.
**Date:** 2026-10-03
**Related:**
- LLP 1030.003 D7: the EAS Hosting findings this document extends, and the directory origin behind a tunnel that stays the fallback.
- LLP 1030.000 D3 (the web root and the origin adapter), §5 Q3 (conditional put, ruled required 2026-09-03).
- LLP 1048.000 D10 (the server: isolation, bounds, outbound fetches, operations), D11 (its HTTP contract).
- LLP 1071 §7 (the two renderers measured; Rust the default, ruled 2026-09-28).
- LLP 1047.000 §9 (`app.wasm` deltas and `--generations`).
- LLP 1069.006 D4, D6 (the auth callback and the client-metadata document on the app's origin).
- `rules/DEFERRED.md` §Runtime: per-user targeting, cohorts, a console, server-side anything and push stay out.

## Summary

Charlie, 2026-10-03: *"Figure out what we need to do for exact2 hosting. Is
the hosting we have for EAS websites and api routes enough or do we need
something more powerful?"* — then: *"Combine this with what we need for an
OTA updates product and then write up an LLP I can send to Phil to have him
build hosting for this."*

An exact2 app's hosting is one origin that serves three things:

1. **Files.** The built web app, content-addressed and immutable, behind one
   mutable pointer.
2. **Pages rendered per request.** Routes declared `cached` or `request` are
   rendered by the app's own native binary, `<app>-render --serve`.
3. **Updates.** Signed heads (`no-store`), immutable blobs and release
   records, written by a publisher that needs a **conditional put**.

EAS Hosting today (Cloudflare Workers plus static files) does (1) and parts
of (3). It cannot do (2), and it is missing two things (3) requires. It is not
short of compute: a page costs 2–8 ms of CPU and the process 50–90 MB of
memory (§1). What it lacks is **a place to run a long-lived native Linux
process**, **per-path cache headers on static files**, and **a write that
only lands if the file is still the version the writer read**.

The proposal: keep Cloudflare as the front door, and add three things behind
it:
- an object store (R2) for files and updates, with a small publish API that
  has conditional writes;
- a container tier that runs one render image per app release;
- a Worker that routes between the two by path.

Ship it in three phases (§7). Phase 1 needs no containers and makes every app
that renders only at build — Caltrain today — fully hostable, OTA included.

## 1. What exact2 asks of a host

### 1.1 The render server

`<app>-render` is a native Rust binary built per app and per release. The
app's plan is baked into it (`exact_render::main(baked, data)`,
`host/render/src/lib.rs:877`), and its native data source is linked in.
Per request it boots a fresh runner and a fresh module realm. TypeScript
sources run in the embedded Hermes VM (C++, `js/Cargo.toml`). It fetches the
app's granted APIs over rustls and returns a document (LLP 1048.000 D9–D10).

| Property | Value | Source |
|---|---|---|
| Process model | one long-lived process; `--renders` worker threads (default 4); bounded `--queue` (32), 503 when full | 1048.000 D10 "As built" |
| Listens | `127.0.0.1:<--port>` only, today (`serve.rs:155`) | §6 item 2 |
| CPU a page | Caltrain 2.4 ms; RealWorld 2.4–2.7 ms after 1048.004 stage 1; a 1,000-row grid 36–66 ms | 1071 §7; 1048.004 §0 |
| Latency | Caltrain p50 2 ms; RealWorld p95 ~186 ms, mostly its API | 1071 §7 |
| Memory | 46–86 MB resident, warm; about +7 MB for 12 idle realms | 1071 §7; 1048.000 D10 |
| Cold start → first page | 10–151 ms Caltrain; ~545 ms RealWorld | 1071 §7 |
| Throughput | Caltrain 688 pages/s at concurrency 32; RealWorld 22/s at 4 workers (API-bound) | 1071 §7 |
| Local state | an in-memory page cache (64 pages / 32 MiB); `--generations <dir>` keeps the last four `app.wasm` builds on disk for deltas | 1048.000 D11; 1047.000 §9 |
| Outbound | HTTPS to the app's granted origins only; no private addresses; no cookies | 1048.000 D10 |
| Secrets | none: the render is anonymous, and a source that asks for a kept secret is refused | `host/render/src/lib.rs` header |
| Lifecycle | SIGTERM drains (in-flight renders finish); `/.exact/health` answers `ok`; one log line per render | 1048.000 D10 |
| Trust | module code is trusted app code, not sandboxed; one server serves one app | 1048.000 D10 |

Its HTTP contract is already written (1048.000 D11), so the host only has to
respect it, not reproduce it:
- `cached`/`build` routes: `public, max-age=0, s-maxage=L, stale-while-revalidate=L`.
- `request` routes: `no-store`.
- 404/410: `s-maxage=60`. 500/503: `no-store`, and 503 carries `Retry-After`.
- ETags that name the encoding, and 304s.
- Brotli/gzip negotiated, with `Vary: Accept-Encoding`.
- Surrogate keys (`Cache-Tag`, `Surrogate-Key`), sent only to a request that
  carries `CDN-Loop` (RFC 8586), which Cloudflare adds.
- Canonical URLs built from `--origin`, never from `Host` or `X-Forwarded-*`.

**Why not a Worker.** Workers run neither native binaries, threads, nor a
filesystem. The JavaScript renderer (`host/web-js/render.mjs`) is no way out
either: it is built on `Bun.serve`, `node:vm`, `node:fs` and `node:zlib`, and
Workers allow neither `vm` nor `eval`. LLP 1071 §7 estimates the third option
— the render core as wasm on an edge runtime, with TypeScript sources routed
through the runtime's own engine — but nothing is built. That is a port of
the render host, not a hosting task, and it is not proposed here.

### 1.2 The update origin

What follows is LLP 1030/1030.000/1030.003 as the code has them
(`origin.mjs`, `deploy.mjs`, `update/src/store.rs`). Where a document and the
code disagree, the code is current.

**Layout:**

| Path | What | Mutable | Cache-Control |
|---|---|---|---|
| `/`, `index.html`, `app.js`, `app.wasm`, `app.plan`, `manifest.json`, `/assets/ /rust/ /gpu/ /modules/ …` | the web app's canonical names (`serve.mjs` `PUBLIC_FILES`) | per release | `no-cache` + ETag |
| `/.exact/root/web/releases/<sha256>/…` | each web release's immutable files | no | `public, max-age=31536000, immutable` |
| `/.exact/root/web/exact.json` | the web root's pointer: full inventory and digests | **yes, conditional** | `no-store` |
| `/.exact/blobs/<sha256>` | content-addressed payloads (plans, modules, assets, Rust modules) | no | `immutable` |
| `/.exact/<channel>/<compatibilityId>/exact.json` | a stream's **signed head** (≤ 64 KiB) | **yes, conditional** | `no-store` |
| `/.exact/<channel>/<compatibilityId>/releases/<release>.json` | each publish's immutable record | no | `immutable` |
| `/.exact/<channel>/<compatibilityId>/.lock` | publisher-private | — | **never served** |
| `/.exact/install/{index,web,ios,macos}`, `/.exact/install.json` | install pages, and the data they render (1030.003 D6a, 2026-10-02 amendment) | per release | `no-cache` |
| `/.exact/auth/callback` | the OAuth return page (1069.006 D4) | per release | `no-store`; `Referrer-Policy: no-referrer`; `Cross-Origin-Opener-Policy: same-origin`; no trailing-slash redirect |
| `/.well-known/apple-app-site-association` | generated at bake | per release | `no-store`, `application/json`, **no redirect** (Apple's CDN refuses one) |
| `/.exact/health` | the render server's | — | `no-store` |

Note that a stream's path carries the channel segment
(`.exact/<channel>/<compatibilityId>/`). LLP 1030 D3a and 1030.000 D2 still
show it without; 1030.001 §2 rules for the code.

**Semantics a client relies on:**
- `Accept: application/vnd.exact.envelope+json` at `/` returns the envelope,
  and otherwise HTML, with `Vary: Accept` (LLP 1023 D1). A dumb host may
  instead answer HTML with `<link rel="alternate" type="application/vnd.exact.envelope+json">`.
- Every card URL stays on the head's origin (`envelope.rs` `resolve_url`).
  Clients follow at most five redirects, never downgrade, and never go
  cross-origin for a payload (1026 D1).
- A single byte range is answered (206/416). GET and HEAD only.
- A head is fetched `no-store`, by every installed client:
  - built: two seconds after first pixel, and on `deliveryCheck`;
  - proposed (1030.003 D4): on foreground at most once a minute, every 15
    minutes, a background refresh, and the web tab's check.

  This is the host's hottest path. It is small and must never be cached.

**What the publisher needs:**
- **Write once, immutably**: blobs and release records, each landing whole or
  not at all, and skipped when already present.
- **Read back** every blob's digest before the head moves.
- **Conditional put** on the head and on the web root's pointer: replace it
  only if its ETag still equals the one the publisher read (`If-Match`).
  Ruled required, 1030.000 §5 Q3. The `seq` is computed under it (`nextSeq`:
  the admitted head's, or the highest authenticated record's, plus one). A
  lost race is a retry, never two heads with one `seq`.
- **Order**: blobs → read back → record → head. A failure at any step leaves
  the previous head serving; nothing rolls back.
- **No private keys at the origin.** Heads are Ed25519-signed by the
  publisher, and clients treat the CDN as untrusted (1026 D11). The host
  stores bytes. It never signs, rewrites or re-encodes a `.exact/` body other
  than by transparent compression.
- **Retention**: nothing old is deleted, for now (1030.000 D3: "no garbage
  collector is introduced"). Old blobs are what a client still on an old
  `seq` downloads.

**Sizes.**
- Limits: an envelope ≤ 64 KiB; a payload ≤ 64 MiB; a whole set ≤ 256 MiB
  (LLP 1023 D3).
- Typical: a plan is about 12–45 KB; `app.wasm` is 240 KB brotli, and a
  delta 10–50 KB (1047.000 §9).
- Each live stream adds roughly one more copy of what changed. Dedup by
  digest keeps an unchanged asset to one copy (1030.000 §4).

### 1.3 One URL

"The URL is the app" (1030.000 D2): one origin serves the web app, every
stream's head and the deep-link authority, and native binaries compile that
origin in. A host that cannot hold all three on one origin forces the split
1030.003 D7 permits — "at the cost of 1023 D1's one URL". This RFC's design
does not need the split.

## 2. EAS Hosting against §1

| Need | EAS Hosting today | Gap |
|---|---|---|
| Static web files, immutable assets | yes | — |
| Per-path `Cache-Control` on static files (`no-store` heads) | no: 3600 s browser cache, overridable for API routes only (1030.003 D7.2) | a Worker in front of the files, or files served by a route |
| Dot paths (`.exact/`, `.well-known/`) | uploaded (`eas-cli worker/assets.ts`); whether they are served is unverified | verify; route them explicitly |
| Conditional put on a single file | no: deployments are immutable, and an alias repoint has no `If-Match` (1030.003 D7.1) | a publish API over an object store |
| Long-lived native process | no: Workers only | a container tier |
| Persistent local disk (`--generations`, page cache) | no | a durable volume, or move the generations to the object store |
| Custom domains, TLS, CDN, preview URLs | yes | reuse |
| CDN purge by tag | Cloudflare has it; EAS doesn't expose it | expose it (phase 3) |

So: reuse the domain, TLS, CDN and Worker layer, and the `eas deploy` UX
where it fits. Add an object store with conditional writes, per-path headers,
and containers.

## 3. Design

### D1 — One Worker in front, routing by path

Each app gets one hostname (custom domain or `<slug>.expo.app`). A Worker
answers every request:

1. `/.exact/<channel>/<cid>/exact.json`, `/.exact/root/web/exact.json` →
   object store, `no-store`, passing `ETag` and `If-None-Match` through.
2. `/.exact/blobs/*`, `/.exact/root/web/releases/*`, `…/releases/*.json` →
   object store, `immutable`, with single-range support.
3. `*/.lock` and anything not in the release's inventory → 404.
4. `/.well-known/apple-app-site-association` → object store, `no-store`,
   `application/json`, never a redirect.
5. `/.exact/install*`, `/.exact/auth/callback` → object store, with §1.2's
   headers.
6. The web root's canonical names (`/app.js`, `/assets/…`) → resolved through
   the current pointer (`/.exact/root/web/exact.json`, cached in the Worker
   for at most a few seconds) to the immutable release path. Sent `no-cache` +
   ETag, as `serve.mjs` sends them.
7. **Everything else is a page.** If the app's release has a render image,
   forward to it (D2). Otherwise serve the release's `index.html` /
   `shell.html`, or a 404 document if the release has one. Never answer an
   unknown path with the SPA fallback's 200 (1048.000 D11).
8. `/` with `Accept: application/vnd.exact.envelope+json` → the envelope from
   the object store, with `Vary: Accept`.

The Worker forwards the render server's response untouched. The server
already sets `Cache-Control`, ETags, CSP and `X-Robots-Tag`, and the CDN
caches by them. The Worker must not add `Vary`, rewrite `Cache-Control` or
recompress. It must keep `CDN-Loop`, so the server sends surrogate keys
(which the CDN strips before the browser).

### D2 — The render tier: one container image per app release

- **Build.** From the app's repository:

  ```
  cargo build --release -p <crate> --bin <app>-render
  ```

  The crate is the app's Linux crate, or its web crate — whichever has the
  binary; `host/web-js/build.mjs` ~439 has the lookup. Target
  `x86_64-unknown-linux-gnu` or `aarch64` (Phil picks). The image holds the
  binary and the release's `dist/`. The plan is baked in, so **an image is a
  release**, never reused across releases. Cold release builds are slow:
  about 52 s for Caltrain's release render binary cold (1071 §7). The build
  wants a cached cargo target per app.
- **Run.**

  ```
  <app>-render --serve /dist --port $PORT --origin https://<app host> --renders <cpus> --queue 32 [--lifetime 60] [--generations /data/gen]
  ```

  It needs exact2's `--bind` change (§6 item 2), or a loopback sidecar
  proxy.
- **Size.** Start at 1 shared vCPU and 256 MB. That holds four workers and
  the 32 MiB page cache with room to spare. Scale on CPU and queue depth
  (503s carry `Retry-After: 1`, so a spike sheds rather than piles up).
- **Instances.** Keep at least one warm instance for apps with rendered
  routes: a cold start is 0.1–0.6 s before the first byte. Scaling to zero is
  a later option for previews only.
- **Network.**
  - Inbound only from the Worker.
  - Outbound HTTPS to the internet. The server enforces the app's grants
    itself; the host adds egress to private ranges denied, as a second wall.
- **No secrets, no database, no shared state.** The page cache is per
  instance and the CDN is the shared one. `--generations` wants a small
  durable volume per app, or (later) reads from the object store. Without it
  the server simply serves no deltas.
- **Health and drain.**
  - Readiness and liveness: `GET /.exact/health`.
  - Rollout: start the new image, wait for health, shift traffic, then send
    SIGTERM to the old one. The old one drains in-flight renders; give it 30 s.
- **Logs.** One line per render (location, status, time, answers, pending,
  bytes) goes to a log the developer can read (`eas` CLI or dashboard).
  Nothing else is collected: metrics beyond "a log the developer owns" are
  deferred (1030.000 §6).
- **Version skew.** During a rollout, old pages ask for old assets. Those are
  immutable and never deleted (D3), and a checkpoint from another plan
  renders fresh (1048.000 D10). So two images serving at once is safe.

### D3 — The object store and the publish API

One bucket prefix per app (R2, since Cloudflare is already the front). The
publisher never talks to R2 directly. It calls a small authenticated publish
API, which is what `origin.mjs`'s owed "object-store adapter" targets
(`origin.mjs:15-18`):

| Call | Meaning |
|---|---|
| `HEAD/GET <path>` | read, with the ETag |
| `PUT <path>` + `If-None-Match: *` | create once (blobs, release records, web release files). 412 if present; the publisher treats a present blob with the right digest as success |
| `PUT <path>` + `If-Match: <etag>` | **conditional replace**: heads and the web pointer. 412 if it moved |
| `PUT <path>` + `If-None-Match: *` on a head | the stream's first head (genesis) |
| `POST /purge` `{tags: […]}` | phase 3: purge CDN entries by surrogate key |

R2 offers conditional writes through its Workers binding (`onlyIf`) and its
S3 API; the API must surface them exactly, and never as read-then-write.
Requirements:
- An upload lands whole or not at all.
- No DELETE in v1.
- Paths outside `.exact/` and the web root are refused.
- Bodies are stored byte-exact.

**Auth.** A per-app token that EAS issues: the publisher's CI or a
developer's machine holds it. It authorizes writes to that app's bucket
prefix and nothing else. The signing key is separate, and never leaves the
publisher (1030.000 D7).

**A release, end to end:**
1. Upload blobs and web release files (create-once).
2. Read back the digests.
3. If the release has a render image: push it, start it, wait for health.
   Leave traffic where it is.
4. Write the release records, then conditional-put each stream's head and
   the web pointer.
5. Shift page traffic to the new image as the pointer moves. The Worker
   reads, from the pointer, which image serves pages; a separate switch
   would drift.
6. Drain the old image.

A failure before step 4 leaves the old release serving everything. A failure
partway through step 4 leaves some streams on the new `seq` and some on the
old. That is allowed: each stream is independent and finishes "on its own
clock" (1030.003 D1). The publisher retries the rest.

### D4 — Channels, previews and promotion

- **Channels** are path segments (`.exact/<channel>/…`). Several live
  together on one origin.
- A **preview deploy** is a second hostname (EAS's deployment URL) with its
  own pointer and image, reading the same blob store. A preview never moves
  production heads.
- **Promotion** to production is a new signed head on the production stream,
  written by the publisher (1030.000 D4). It is never a copy of the preview
  head, and never an alias repoint.

### D5 — What is not built

- Cohorts, per-user targeting, staged rollout percentages, an update console,
  push delivery, and analytics of installs or crashes — all refused or
  deferred (`DEFERRED.md` §Runtime; 1030 §7).
- App backends: databases, websockets, cron, or an app's own API (Interview's
  Snapback2 owner and its database, 1030.003 D7 "The backend boundary"). They
  are the app's. A later RFC may offer containers for them on the same tier;
  this one doesn't.
- Garbage collection of old blobs and releases.
- Android (`assetlinks.json` is not in `PUBLIC_FILES`; Android is deferred).
- Running the JavaScript renderer, or a wasm render core, on Workers (§1.1).

## 4. Security

- The host never holds a signing key. A compromised origin or CDN can
  withhold updates, but cannot forge one. Clients refuse a head without a
  valid signature, and a `seq` below the floor (1026 D11).
- Publish tokens are per app, write-only to that app's prefix, and can be
  revoked.
- The render container runs trusted app code for one app, with no secrets.
  Isolate apps from one another at the container boundary; nothing inside an
  instance needs isolating.
- `.lock` files and paths outside the inventory are never served.
- The auth callback's headers (§1.2) are the Worker's to keep exact. A wrong
  `Referrer-Policy` leaks the OAuth code.

## 5. Verification

Phil's acceptance is exact2's own tests, run against a hosted origin:

1. `bun scripts/smoke.mjs deploy` with `--origin https://<host>`, once the
   object-store adapter lands (§6 item 1). It publishes, reads heads back,
   and drives a native client through an update.
2. A conditional-put race. Two publishers move one head at once: exactly one
   wins, and the other gets 412 and retries with `seq + 1`. Run 100 times.
3. Header checks per path in §1.2's table. These include: a head is never
   cached by the CDN (fetch it twice across a publish); AASA is answered with
   no redirect; `.lock` returns 404.
4. Page parity. `<app>-render --compare` already proves the two render paths
   agree. Hosting adds that a page fetched through the Worker equals the page
   from the container directly, byte for byte, headers aside.
5. Load. RealWorld at concurrency 64 through the CDN with `no-store` (every
   request renders) holds p95 under 250 ms, with 503s only above queue depth.
   Then with caching: the second request for a `cached` route never reaches
   the container.
6. Rollout under load. Deploy a new release while (5) runs: no 5xx other
   than queue 503s, and no page mixes releases.
7. Drain. SIGTERM during renders completes them.

## 6. What exact2 owes Phil

Each item is small, and none blocks phase 1's start:

1. **The object-store adapter in `scripts/origin.mjs`** (owed in its header)
   against D3's API. `--yes` against an https origin stops refusing.
2. **`--bind <addr>` on the render server.** It binds `127.0.0.1` only today
   (`serve.rs:155`). 1048.000 D10 says moving to a provider "needs
   configuration, not code", and this is that configuration.
3. **A container recipe.** `host/web-js/build.mjs` builds the render binary
   for the build machine. Hosting needs a Linux release build of it and the
   `dist/` together. Either a `deploy.mjs` step emits it, or Phil's builder
   runs the cargo command in D2.
4. **The release manifest for the Worker.** The web pointer gains which
   render image serves the release (or "none"). It is one field in a body
   that is already conditional-put.
5. **Fix the path in 1030 D3a / 1030.000 D2** to include the channel segment
   (1030.001 §2 asked for this).

## 7. Phases

**Phase 1 — files and updates, no containers.**
- D1 without step 7's forwarding.
- D3 in full.
- D4 previews.

This hosts every app whose routes are `client` or `build`: the whole web app,
OTA for iOS and macOS, install pages, AASA, the auth callback. It is the "EAS
Hosting is slice 2b" of 1030.003 D7, done as a product, and it replaces the
directory origin behind a tunnel for every app but those that render per
request.

**Phase 2 — the render tier.**
- D2.
- D1 step 7.
- The rollout coupling in D3 steps 3 and 5.

RealWorld and Interview become hostable, and so does any app with a `cached`
or `request` route.

**Phase 3 — the rest of the contract.**
- Purge by surrogate key, exposed to the publisher.
- `--generations` backed by durable storage, so returning visitors get
  `app.wasm` deltas: 238.6 KB → 9.5 KB, ready 1,608 → 912 ms (1047.000 §9).
- Logs in the dashboard.

## 8. Questions

For Charlie:
1. **One origin or the split?** This design keeps one URL. If Phil would
   rather ship the render tier separately (pages on a different host from
   updates), 1030.003 D7 permits that, at 1023 D1's cost. Recommendation: one
   origin, since D1 makes it no harder.
2. **Who runs the publisher in production:** the developer's machine, EAS
   Workflows, or both? It changes only where the publish token and signing
   key live (1030.003 D8).
3. **Release retention.** Never deleting is fine for now. Should a later
   ruling set a floor (e.g. keep everything any head of the last N `seq`s
   names)?

For Phil:
4. Does EAS Hosting's current Worker layer take D1's routing, or is a
   separate Worker per exact2 app simpler?
5. Which container platform? Requirements: per-app images, one warm instance
   minimum, health-gated rollout, SIGTERM drain, egress control. Cloudflare
   Containers would keep it all in one network; Fly or Cloud Run are the
   alternatives.
6. Does R2's conditional put hold under concurrent writers from different
   regions? §5 item 2 is the test.
