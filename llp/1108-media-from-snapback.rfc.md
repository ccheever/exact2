# LLP 1108: Photos and video from Snapback, as the app's own files

**Type:** RFC
**Status:** Draft r2, 2026-10-07: Charlie's rulings on r1's five questions folded (§0). Not reviewed.
**Systems:** the Snapback4 client (`snapback4/client`: the protocol core; `snapback4/src`: the native module; `snapback4/web`: the wasm; `snapback4/ts`: the driver); the data-module runtime on every executor (`fetch` bodies and `exactSaveTo`/`exactBodyFrom`, `Blob`, `Response.blob()`, `URL.createObjectURL`/`revokeObjectURL`: `js/src/prelude.js`, `js/src/wire.rs`, the runner's request, `host/web-js/ts-fetch.js`); `image` and `video` sources on the Apple, Linux and web hosts (`blob:` URLs); Interview (the first consumer: a profile photo)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Implementer:** Claude (Opus 5.5) lanes, after review
**Related:** LLP 1011 §2 (`app:/data|cache|tmp` image sources on every host), LLP 1042 (video takes the same sources), LLP 1069.002 (the file picker hands back `app:/tmp` paths), LLP 1027.001 and LLP 1030.002 (the `fs.*` grants and the app's file roots), LLP 1027.000 (no clock or timers in a data module); Snapback 0.4.13: `CLIENT-AND-OPERATIONS.md` §Assets, `dist/assets.js`, `dist/asset-download.js`, `dist/local-assets.js`, device ops `upload_facts`, `forget_upload_facts`, `asset_home`; the web's [File API](https://www.w3.org/TR/FileAPI/) (`Blob`, `URL.createObjectURL`, `blob:` URLs)

## 0. Rulings (Charlie, 2026-10-07)

| Question (r1 §6) | Ruling | Where it lands |
| --- | --- | --- |
| Q1 — the first consumer | **Interview's profile photo** | §3 |
| Q2 — one-view assets (`image <=N once`) | **Build the memory-only source now** | D5: `Blob`, `URL.createObjectURL`, `blob:` sources on every host |
| Q3 — retention default | **Keep what was shown, up to 64 MiB** | D3 |
| Q4 — fetch straight to and from files | **Now, not with video** | D6 R2, stage 1 |
| Q5 — web storage eviction | **Re-download quietly**; no `navigator.storage.persist()` | D3 |

## 1. Summary

A Snapback app stores photos and video as **assets**. A column declared `image <=N` or
`video <=N` holds one. The client uploads the bytes first (`POST /assets`), and the
server answers with the facts it established (`id`, `type`, `bytes`, `width`, `height`,
`duration`). The client then writes `{ id }` into the mutation. Reading an asset is
`GET /assets/<id>` under the viewer's own session; the home row's read rule decides
who may see it. A column declared `once` serves each principal one view.

The Snapback4 client in exact2 does none of this yet. This RFC proposes:

- **Showing:** the client downloads an asset with the member's session straight into
  the app's own files, `app:/cache/snapback4/<partition>/<asset id>`, and hands back
  that path. `image` and `video` already show an `app:/` file on every host
  (LLP 1011 §2, LLP 1042), so their file loading does not change.
- **Keeping:** Snapback's own retention rules, defaulting to keep what was shown up
  to 64 MiB, least recently used first. A file is purged when its row leaves the
  partition, is deleted or expires, and at sign-out. The bytes live in files; the
  manifest lives in the partition.
- **One-view assets:** shown from memory and never written to disk, through the
  web's own primitive: a `Blob` and `URL.createObjectURL`, which every executor gets
  (D5).
- **Uploading:** the picked file (`app:/tmp/…`) goes to the server as bytes. The core
  then records the server's facts with the device (`upload_facts`), so a write naming
  the asset predicts offline and survives a reopen.
- **Exact's runtime gains, on every executor:**
  - byte request bodies. Hermes sends `String(init.body)` today, which corrupts
    every upload on iOS and macOS;
  - `exactSaveTo` and `exactBodyFrom`, so a file moves between the network and the
    disk without crossing the JavaScript bridge;
  - `Blob`, `Response.blob()` and object URLs (D5).

## 2. Decisions

### D1 — Show an asset as an app file, never as an authenticated URL

Snapback offers two ways to show an asset. `client.asset(id)` downloads it and
returns a blob URL. `assetSource(id)` returns `{ uri, headers }` for views that can
send headers (React Native's `Image`). Neither fits Exact as it stands:

- **Headers on the view.** Every host's loader would need an authenticated mode.
  The web cannot have one: `<img>` and `<video>` send no `Authorization` header.
- **A token in the URL** (`?token=…`). It leaks the bearer into browser history,
  HTTP caches, server logs and the `Referer`, and Snapback does not accept it.
- **A `data:` URL.** It is bounded at 1 MiB on every host (LLP 1011 §2), which is
  smaller than a phone photo.

An `app:/cache` file has none of these problems. It works the same on the web (the
page's file store, shown through an object URL by `picker-glue.js` `appURL`), on
Apple and on Linux. It shows offline from the first frame. A data module produces
one string, the same on every host, and `video` takes the same sources (LLP 1042,
its `src` row).

What is kept stays on disk, which is what retention means. A one-view asset must
never reach the disk; D5 gives it a memory-only source instead.

### D2 — Where the work happens

The client is a protocol core that performs no I/O (`snapback4/client`). That stays
true: the core decides; the driver moves bytes. Bytes never enter the core. The wasm
boundary passes JSON, and copying a photo through it would be pointless.

| Step | Core (`Client`) | Driver (TS `Snapback`, Rust `Module`) |
| --- | --- | --- |
| Show asset `id` | `asset(id, now)`: a retained file's path; or a `Fetch` for `GET /assets/<id>` with where to put the body (a path under `app:/cache`, or memory for a `once` column) | performs it: `fetch(…, { exactSaveTo: path })`, or `(await fetch(…)).blob()` and `URL.createObjectURL` for memory (D5, D6) |
| The reply | `asset_arrived(exchange, reply)`: reads the status and headers (`content-type`, `x-snapback-asset-home`, `accept-ranges`, `content-range`); asks the device for the home (`asset_home`); records a manifest entry **before** the file counts | keeps the file or removes it, as the core says |
| After a round | `media_due(now)`: the files to remove, because a home changed, expired or left the partition, or the cache is over its cap | removes them |
| Sign-out, `close(forget)` | `media_clear()`: every path | removes them, and revokes every object URL it made |
| Upload | `upload()`: a `Fetch` for `POST /assets`; `uploaded(exchange, reply)`: `upload_facts` with the device, or the refusal | sends the picked file: `fetch(…, { exactBodyFrom: path })` |

The core's `Reply` gains the response headers these paths need. Today it carries
only `{status, body}`. The driver passes the four headers above; other exchanges
ignore them.

The manifest lives in the partition's metadata (`exact:media`): `{id, path, home,
bytes, used}`, where `home` is the server's `{store_id, table, row, column,
generation, expiresAt, tables}`. As in Snapback's `MediaCache`, the entry is written
before the bytes, so a write cut off by a crash is found and removed on the next
open. `used` is the `now` of the read that showed the asset: the core has no clock
(LLP 1027.000).

### D3 — Retention follows Snapback's rules; Exact keeps what was shown

**Default (ruling Q3):** `keep: "viewed"` with `maxBytes: 64 MiB`. Snapback's own
default is `keep: "none"`. An offline-first app that forgets every photo looks
broken offline, so Exact's driver defaults the other way. An app can set
`keep: "none"`, or another cap, when it opens the client.

Under `viewed`:

- **Kept** only when the response is the whole object (`200`, or a `206` covering
  every byte), `accept-ranges: bytes`, the size is within the cap, and the device
  confirms the home (`asset_home`). Anything else is shown from memory (D5) and kept
  nowhere.
- **Checked after every round** whose device calls can change media authority
  (`apply`, `observe_store` unless it is a same-store observation, `adopt`,
  `clear_rows`, `admit`, `settle`), and at open. An entry whose home's `tables` were
  not touched and that has not expired is skipped, as Snapback does.
- **Removed** when its home no longer matches, when it expired (by the `now` the
  driver passes), and least recently used first above the cap. Also at sign-out and
  when the partition is replaced (`E_STORE_RESTORE`).
- **Offline:** a retained file is shown. An asset that is not retained answers
  `E_OFFLINE` with `retryable: true`, as Snapback's does.
- **Evicted by the browser (ruling Q5).** A browser may clear the page's file store
  under storage pressure. A manifest entry whose file is missing or short counts as
  not retained: the entry is dropped, and the asset is fetched again online or
  answers `E_OFFLINE`. Exact does not call `navigator.storage.persist()`.

### D4 — Upload

1. The member picks a file (LLP 1069.002). The data module gets
   `app:/tmp/picked/<n>.jpg`.
2. The driver posts it to `/assets` as `application/octet-stream` with the session,
   with `exactBodyFrom` (D6), so the bytes go from the file to the network in the
   host. Uploads are online only, as in Snapback. Offline, the answer is
   `E_OFFLINE`, and the app keeps the picked file until the member is online.
   Nothing queues the bytes.
3. On success the core calls `upload_facts` with the device. A write naming
   `{ id }` then predicts offline, before and after a reopen. On a refusal
   (`E_ASSET_TYPE`, `E_ASSET_TOO_LARGE`, `E_ASSET_BOUNDS`) the app shows why.
   `E_RATE_LIMIT` with `retryable` waits as Snapback does, up to 10 s in all.
4. A session that changes during the upload (D7) ends it with `E_AUTH`, "start the
   upload again". The facts are forgotten (`forget_upload_facts`), so the asset is
   never recorded against the wrong session.

### D5 — A memory-only source: `Blob` and object URLs, on every executor (ruling Q2)

The server serves a `once` asset one time per principal. Snapback promises that "a
one-view photo is never durably cached anywhere" (`dist/assets.js`). Exact has no way
to show bytes that are not in a file, so this RFC adds one. Per the web-is-the-standard
rule (`CLAUDE.md`), it is the web's own: **`Blob`**, **`Response.blob()`**,
**`URL.createObjectURL(blob)`** and **`URL.revokeObjectURL(url)`**. On the web these
are the browser's. Natively they are the same API over a host-held store:

- **In a data module.** `new Blob([bytes], { type })` and `await response.blob()`
  give a `Blob`. On Hermes it is a handle to bytes the host holds, with `size`,
  `type`, `arrayBuffer()` and `slice()`. `Response.blob()` keeps the response body in
  the host, so a one-view photo never crosses the bridge.
  `URL.createObjectURL(blob)` returns `blob:<app id>/<uuid>`, the web's form (an
  origin, then a UUID). `URL.revokeObjectURL(url)` frees it.
- **In `image` and `video`.** A `blob:` source is resolved from the store, on every
  host, as an `app:/` source is resolved from the file roots:
  - **web:** the browser's own object URL;
  - **Apple:** `image` decodes from the held bytes (`CGImageSource` over `Data`);
    `video` plays through an `AVURLAsset` on a custom scheme whose
    `AVAssetResourceLoaderDelegate` answers byte ranges from the store;
  - **Linux:** `exact_raster` decodes from the bytes.

  A revoked or unknown `blob:` URL is an `error` (LLP 1011 §2), "the object URL was
  revoked", as the web's `<img>` fails on one.
- **Lifetime.** As on the web, until `revokeObjectURL` or until the app's process
  ends. Nothing is written to disk: no spooling to a temporary file, and no image
  cache keyed by the URL that outlives a revoke. A view still showing a picture
  keeps its decoded pixels after a revoke, as a browser's `<img>` does.
- **A bound.** The store holds at most **256 MiB** per app. A `Blob` that would
  exceed it is refused (`QuotaExceededError`), so a leak of object URLs fails loudly
  instead of growing without limit.

The Snapback client uses it for every asset the cache does not keep:
- a `once` column (`accept-ranges: none`);
- a partial response;
- an asset over the cap;
- everything under `keep: "none"`.

It hands back the `blob:` URL and revokes it when the asset leaves the read that
asked for it, or at close. A one-view photo therefore spends its view only when it
is shown, and leaves nothing behind.

### D6 — The Exact runtime: bytes, and files to and from the network

**R1 — byte request bodies.** A data module's `fetch` takes an `ArrayBuffer`, a
typed array or a `Blob` as `body`, on every executor, as the browser does:
- **Hermes:** `js/src/prelude.js` stops coercing the body to a string. A byte body
  crosses the wire as base64 (the `bodyBase64` field the response already uses), and
  a `Blob` by its handle, into the runner's `Request.body: Vec<u8>`.
- **JS target:** `ts-fetch.js` passes the bytes through.
- **A Rust source:** its `Request` already carries bytes.

Today an app on iOS that posts a photo sends the string `"255,216,255,…"`. That is a
silent corruption, worth fixing on its own.

**R2 — to and from files (ruling Q4: now).** Two `fetch` options:
- `exactSaveTo: "app:/cache/…"`: the host writes the response body to that file and
  resolves with the status and headers. `text()`, `json()` and `arrayBuffer()` then
  read nothing.
- `exactBodyFrom: "app:/tmp/…"`: the host reads the request body from that file.

Both are checked against the `fs.*` grants as `writeFile` and `readFile` are.

The written file appears only when the body is complete (written beside it, then
renamed). A failure leaves no partial file, so a manifest entry never names half a
photo.

The bound is `maxResponseBytes` or the file's size, at most 64 MiB (Snapback's own
cap). On the web, they are a page fetch piped into the file store, and a file read
into the body. For a Rust source they are `save_to` and `body_from` on its
`Request`, carried out by the same host code.

With R2 a 60 MB video moves between the network and the disk in the host. Without
it, the same video would cross the JavaScript bridge as an 80 MB base64 string in
each direction.

### D7 — Sessions

Every asset exchange carries the bearer the driver's `headers()` gives at that
moment. A download answered `E_AUTH` after a refresh rotated the token (the client's
`refresh`, from its second round) is asked once more with the new token. An upload
is not, by D4.4.

### D8 — Testing and agents

- **Runtime tests, on Hermes and the JS target:**
  - a byte body and a `Blob` body arrive byte for byte;
  - `exactSaveTo` writes the whole file or nothing, including a cut-off transfer;
  - `exactBodyFrom` sends the file;
  - `Response.blob()` never surfaces its bytes in JavaScript;
  - an object URL shows in an `image` and plays in a `video` on the web, macOS, iOS
    and Linux;
  - a revoked one fails;
  - the 256 MiB bound refuses.
- **Client tests** against a real `snapback4 dev` with an `image <=1MB` column and an
  `image <=1MB once` column:
  - upload, write, sync, download, retention across a reopen;
  - a home row deleted on another device, so its file is removed after the next
    round;
  - expiry by `now`;
  - the LRU cap;
  - offline shows the retained file;
  - a missing file (browser eviction) is fetched again;
  - a `once` asset is shown from memory, never written, and its second request is
    `E_ASSET_VIEWED`.
- **Drives:** the agent picks a file with `type @t` (LLP 1069.002), saves the photo,
  and checks the `image`'s `load` event (LLP 1011 §2) on the web, macOS and iOS,
  online and with `fail fetch` on the asset route. That includes a physical iPhone
  through Interview's LAN mode (`npm run backend -- --lan`).

## 3. Interview: a profile photo (ruling Q1)

- **Schema:** `people.photo: image <=2MB ?`.
- **Setting it:** Settings gets "Add photo" (the picker), "Change photo" and
  "Remove photo". Saving writes `{ id }` through `profile`.
- **Showing it:** the avatar shows the photo wherever initials show now: post
  cards, prompt cards, the profile head, comments and the account switcher.
- **Fallback:** the initials, while the photo is loading, offline when it is not
  retained, and when there is none.

It exercises every path: upload, a write naming the asset, download, retention (the
same few avatars on every screen fit easily in 64 MiB), and removal when a member
changes or removes their photo. Post images can follow on the same machinery.

## 4. Stages

1. **The runtime:** R1 (byte bodies), R2 (`exactSaveTo`, `exactBodyFrom`, Rust
   `save_to` and `body_from`), and D5 (`Blob`, `Response.blob()`, object URLs, `blob:`
   sources in `image` and `video` on the web, Apple and Linux), with D8's runtime
   tests. This is the largest stage; each part lands on its own.
2. **The client:**
   - the core's asset, upload and retention calls and the header-carrying `Reply`;
   - the TS and Rust drivers;
   - memory for what is not kept;
   - D8's client tests.
3. **Interview's profile photo**, driven on the web, macOS, iOS and a physical
   iPhone.
4. **Video:** a consumer that plays an asset (`video <=N`), through the same paths.
   Nothing new in the runtime.

## 5. Cost

- **Stage 1:** about 1,500 lines across the runtime and hosts:
  - R1: about 40;
  - R2: about 300 over Hermes, the JS target and the native HTTP executor;
  - D5: about 1,100, of which the Apple video resource loader is the largest piece;
    the Hermes `Blob` and the host store are the second.
- **Stage 2:** about 450 lines in the core, 200 in the drivers and 350 of tests.
- **Stage 3:** about 200 lines in Interview.
- **Size:**
  - the wasm grows by the core's new code only;
  - the runtime adds no dependency;
  - Hermes gains a `Blob` in the prelude;
  - the native store is a map of byte buffers.

## 6. Open questions

None from r1; the rulings answer them. To settle in review:

- **The `blob:` origin part natively.** The app id (`blob:com.interview.app/<uuid>`)
  is proposed: unique per app, and stable in logs.
- **The 256 MiB memory bound.** Proposed as the default, with no knob until an app
  needs one.
