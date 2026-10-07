# LLP 1108: Photos and video from Snapback, as the app's own files

**Type:** RFC
**Status:** Draft r1, 2026-10-07. Not reviewed. Charlie asked for it (2026-10-07) after the Snapback4 client landed without media.
**Systems:** the Snapback4 client (`snapback4/client`: the protocol core; `snapback4/src`: the native module; `snapback4/web`: the wasm; `snapback4/ts`: the driver), the data-module `fetch` on Hermes (`js/src/prelude.js`, `js/src/wire.rs`) and on the JS target (`host/web-js/ts-fetch.js`), `image` and `video` (`app:/` sources, unchanged), Interview (the first consumer)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Implementer:** Claude (Opus 5.5) lanes, after review and rulings
**Related:** LLP 1011 §2 (`app:/data|cache|tmp` image sources on every host), LLP 1042 (video takes the same sources), LLP 1069.002 (the file picker hands back `app:/tmp` paths), LLP 1027.001 and LLP 1030.002 (the `fs.*` grants and the app's file roots), LLP 1027.000 (no clock or timers in a data module); Snapback 0.4.13: `CLIENT-AND-OPERATIONS.md` §Assets, `dist/assets.js`, `dist/asset-download.js`, `dist/local-assets.js`, device ops `upload_facts`, `forget_upload_facts`, `asset_home`

## 1. Summary

A Snapback app stores photos and video as **assets**. A column declared `image <=N` or
`video <=N` holds one. The client uploads the bytes first (`POST /assets`), and the
server answers with the facts it established (`id`, `type`, `bytes`, `width`, `height`,
`duration`). The client then writes `{ id }` into the mutation. Reading an asset is
`GET /assets/<id>` under the viewer's own session; the home row's read rule decides
who may see it.

The Snapback4 client in exact2 does none of this yet. This RFC proposes:

- **Showing:** the client downloads an asset with the member's session into the app's
  own files, `app:/cache/snapback4/<partition>/<asset id>`, and hands back that path.
  `image` and `video` already show an `app:/` file on every host (LLP 1011 §2,
  LLP 1042), so **no host's image or video loader changes**.
- **Keeping:** Snapback's own retention rules (`keep: "viewed"` up to `maxBytes`,
  least recently used first, purged when the row leaves the partition, is deleted or
  expires, and at sign-out), with the bytes as files and the manifest in the
  partition.
- **Uploading:** the driver reads the picked file (`app:/tmp/…`), posts it, and the
  core records the server's facts with the device (`upload_facts`), so a write
  naming the asset predicts offline and survives a reopen.
- **One runtime change in Exact:** a data module's `fetch` on Hermes sends a request
  body as text today (`String(init.body)`, `js/src/prelude.js`). It must send an
  `ArrayBuffer` or typed array as bytes, as the browser does. A second, optional
  change lets a fetch read from or save to an app file, so a 60 MB video never
  crosses the JavaScript bridge (D6).

## 2. Decisions

### D1 — Show an asset as an app file, never as an authenticated URL

Snapback offers two ways to show an asset. `client.asset(id)` downloads it and
returns a blob URL. `assetSource(id)` returns `{ uri, headers }` for views that can
send headers (React Native's `Image`). Neither fits Exact:

- **Headers on the view.** Every host's loader would need an authenticated mode.
  The web cannot have one: `<img>` and `<video>` send no `Authorization` header.
  Getting around that means a service worker or a JS download per image, and the
  latter is this proposal anyway.
- **A token in the URL** (`?token=…`). It leaks the bearer into browser history,
  HTTP caches, server logs and the `Referer`, and Snapback does not accept it.
- **A `data:` URL.** It is bounded at 1 MiB on every host (LLP 1011 §2), which is
  smaller than a phone photo.

An `app:/cache` file has none of these problems. It works the same on the web (the
page's file store, shown through an object URL by `picker-glue.js` `appURL`), on Apple
and on Linux. It shows offline from the first frame. A data module produces one
string, the same on every host. `video` takes the same sources (LLP 1042, its `src` row).

The cost is that the bytes are on disk. For an ordinary asset that is what
`keep: "viewed"` asks for. For a one-view asset it is not (D5).

### D2 — Where the work happens

The client is a protocol core that performs no I/O (`snapback4/client`). That stays
true: the core decides; the driver moves bytes.

| Step | Core (`Client`) | Driver (TS `Snapback`, Rust `Module`) |
| --- | --- | --- |
| Show asset `id` | `asset(id)`: a retained file's path, or a `Fetch` for `GET /assets/<id>` | performs the fetch, saving the body to the path the core named (D6) |
| The reply | `asset_arrived(exchange, reply)`: reads the status and headers (`content-type`, `x-snapback-asset-home`, `accept-ranges`, `content-range`); asks the device for the home (`asset_home`); records a manifest entry **before** the file counts | keeps the file, or removes it when the core says not to keep it |
| After a round | `media_due()`: the files to remove because a home changed, expired or left the partition, or because the cache is over `maxBytes` | removes them |
| Sign-out, `close(forget)` | `media_clear()`: every path | removes them |
| Upload | `upload()`: a `Fetch` for `POST /assets`; `uploaded(exchange, reply)`: `upload_facts` with the device, or the refusal | reads the file, sends it as bytes |

Bytes never enter the core. The wasm boundary passes JSON, and copying a photo
through it would be pointless.

The core's `Reply` gains the response headers the asset paths need. Today it
carries only `{status, body}`. The driver passes the four headers above; other
exchanges ignore them.

The manifest lives in the partition's metadata (`exact:media`): `{id, path, home,
bytes, used}`, where `home` is the server's `{store_id, table, row, column,
generation, expiresAt, tables}`. As in Snapback's `MediaCache`, the entry is written
before the bytes, so a write cut off by a crash is found and removed on the next
open. `used` is the `now` of the read that showed it: the core has no clock
(LLP 1027.000).

### D3 — Retention follows Snapback's rules exactly

`keep: "none"` (the default, as in Snapback) or `keep: "viewed"` with `maxBytes`.
Under `viewed`:

- **Kept** only when the response is the whole object (`200`, or a `206` covering
  every byte), `accept-ranges: bytes`, the size is within `maxBytes`, and the device
  confirms the home (`asset_home`). Anything else is shown once from `app:/tmp` and
  removed when the read is replaced (D5).
- **Checked after every round** whose device calls can change media authority
  (`apply`, `observe_store` unless it is a same-store observation, `adopt`,
  `clear_rows`, `admit`, `settle`), and at open. An entry whose home's `tables` were
  not touched and that has not expired is skipped, as Snapback does.
- **Removed** when its home no longer matches, when it expired (by the `now` the
  driver passes), and least recently used first above `maxBytes`. Also on
  sign-out, and when the partition is replaced (`E_STORE_RESTORE`).
- **Offline:** a retained file is shown; an asset that is not retained answers
  `E_OFFLINE` with `retryable: true`, as Snapback's does.

Exact adds nothing of its own here. A screen that showed a photo yesterday shows it
offline today, by Snapback's rules, and nothing else stays.

### D4 — Upload

1. The member picks a file (LLP 1069.002). The data module gets
   `app:/tmp/picked/<n>.jpg`.
2. The driver reads it (`storage.fs.readFile`) and posts it to `/assets` as
   `application/octet-stream` with the session. Uploads are online only, as in
   Snapback. Offline, the answer is `E_OFFLINE`, and the app keeps the picked file
   until the member is online. Nothing queues the bytes.
3. On success the core calls `upload_facts` with the device. A write naming
   `{ id }` then predicts offline, before and after a reopen. On a refusal
   (`E_ASSET_TYPE`, `E_ASSET_TOO_LARGE`, `E_ASSET_BOUNDS`) the app shows why.
   `E_RATE_LIMIT` with `retryable` waits as Snapback does, up to 10 s in all.
4. A session that changes during the upload (D7) ends it with `E_AUTH`, "start the
   upload again". The facts are forgotten (`forget_upload_facts`), so the asset is
   never recorded against the wrong session.

### D5 — One-view assets (`image <=N once`)

The server serves a `once` asset one time per principal, and Snapback promises that
"a one-view photo is never durably cached anywhere" (`dist/assets.js`). An app file is
durable storage, so D1 cannot keep that promise as it stands.

**Recommendation:** in the first version the client **refuses** to show a `once`
asset (`E_ASSET_ONCE`: "this client cannot show a one-view asset yet"), and it says so
before it fetches. That way no view is spent. Exact has no memory-only image source,
so there is nothing honest to show one with. When one exists (a `blob:`-style handle
an `image` can take, with no file behind it), the client uses it for `once` assets
only. That is a separate RFC.

The alternative, writing to `app:/tmp` and removing the file when the screen lets
go, leaves the bytes on disk across a crash. It is listed as Q2.

### D6 — The Exact runtime: bytes in, and bytes straight to a file

**R1 (required).** A data module's `fetch` takes an `ArrayBuffer` or typed array as
`body` on every executor, as the browser does:
- **Hermes:** `js/src/prelude.js` stops coercing the body to a string. A byte body
  crosses the wire as base64 (`bodyBase64`, the field the response already uses) into
  the runner's `Request.body: Vec<u8>`.
- **JS target:** `ts-fetch.js` passes the bytes through.
- **A Rust source:** its `Request` already carries bytes.

Today an app on iOS that posts a photo sends the string `"255,216,255,…"`. That is a
silent corruption, and worth fixing on its own.

**R2 (recommended for video).** Two options on `fetch`:
- `exactSaveTo: "app:/cache/…"`: the host writes the response body to that file and
  resolves with the status and headers only.
- `exactBodyFrom: "app:/tmp/…"`: the host reads the request body from that file.

Both are checked against the `fs.*` grants as `readFile` and `writeFile` are. Both
are bounded by `maxResponseBytes` or the file's size, at most 64 MiB (Snapback's own
cap). On the web they are a page fetch piped into the file store, and a file read
into the body.

Without R2, a 60 MB video crosses the JavaScript bridge as an 80 MB base64 string in
each direction. With R2 the bytes stay in the host. Photos are fine without it. R2
makes video practical on Hermes.

A Rust source needs neither: it already asks the host for requests (`Answer::Later`),
and the host can be told to save the body to a path in the same `Request`
(`save_to`), which is R2's Rust spelling.

### D7 — Sessions

Every asset exchange carries the bearer the driver's `headers()` gives at that
moment. A download answered `E_AUTH` after a refresh rotated the token (the
client's `refresh`, from its second round) is asked once more with the new token. An upload is not, by D4.4.

### D8 — Testing and agents

- **Client tests** against a real `snapback4 dev` with an `image <=1MB` column:
  - upload, write, sync, download, retention across a reopen;
  - a home row deleted on another device, so its file is removed after the next
    round;
  - expiry by `now`;
  - the LRU bound;
  - offline shows the retained file;
  - a `once` column is refused without a fetch.
- **Runtime test** for R1: a byte body arrives byte for byte on Hermes and the JS
  target.
- **Drives:** the agent picks a file with `type @t` (LLP 1069.002), publishes, and
  then checks the `image`'s `load` event (LLP 1011 §2) on the web, macOS and iOS,
  online and with `fail fetch` on the asset route.

## 3. Interview: the first consumer

Interview has no media today. The smallest real use is a **profile photo**:
`people.photo: image <=2MB ?`, set from Settings with a picker, and shown in the
avatar wherever initials are shown now. Initials remain the fallback while the photo
is loading, offline when it is not retained, and when there is none.

It exercises every path: upload, a write naming the asset, download, retention (the
same few avatars on every screen), and removal when a member changes or removes
their photo. Post images can follow. They would add the same thing at a larger size.

## 4. Stages

1. **R1** in the runtime, with its test. Small and independent. It fixes a silent
   corruption today.
2. **The core and drivers:**
   - the core's asset, upload and retention calls and the header-carrying `Reply`;
   - the TS and Rust drivers;
   - the client tests (D8).
   Photos only.
3. **Interview's profile photo**, driven on the web, macOS and iOS.
4. **R2 and video**: `exactSaveTo` and `exactBodyFrom`, `save_to` for Rust, and a
   video consumer.
5. **One-view assets**, after an in-memory image source exists (D5).

## 5. Cost

- **Stage 1:** about 40 lines in `prelude.js`, `wire.rs` and `ts-fetch.js`, plus a test.
- **Stage 2:** about 400 lines in the core (comparable to the refusal journal and
  rounds), 150 in the drivers, and 300 of tests.
- **Stage 3:** about 150 lines in Interview.
- **Stage 4:** a host change on every executor, the largest piece. It is deferred
  until something needs video.
- **Size:** the wasm grows by the core's new code only, no new dependency. The
  native module is unchanged in kind.

## 6. Open questions

- **Q1 — Consumer.** Is Interview's profile photo the right first use, or should it
  be post images?
- **Q2 — One-view assets.** Refuse them until an in-memory source exists (D5,
  recommended), or show them from `app:/tmp` and remove the file when the screen lets
  go?
- **Q3 — Retention default.** Snapback's default is `keep: "none"`. Should an Exact
  app default to `keep: "viewed"` with, say, 64 MiB, since an offline-first app that
  forgets every photo looks broken offline?
- **Q4 — R2 now or with video.** Recommended: with video (stage 4). Photos are fine
  through base64.
- **Q5 — Web storage quota.** A browser can evict the page's file store under
  pressure (private windows especially). The manifest then names files that are
  gone. The read path already treats a missing or short file as not retained and
  drops the entry (as `MediaCache.read` does); confirm that is enough, or ask for
  persistent storage (`navigator.storage.persist()`) when `keep: "viewed"` is on.
