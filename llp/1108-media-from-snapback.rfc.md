# LLP 1108: Photos and video from Snapback, as the app's own files

**Type:** RFC
**Status:** Draft r4, 2026-10-08: NOT READY after two review rounds (GPT-6 Astra and Grok 4.7, both NOT READY on r3 and on r4). Round 1 is folded (§0.2); round 2 is open (§0.3) and must be folded and reviewed again before slice 1 is built. r3 brought it up to the client as it is on 2026-10-08 and to the app farm's evidence (§0.1), and made the stages slices. r2 (2026-10-07) folded Charlie's rulings (§0).
**Systems:** the Snapback4 client (`snapback4/client`: the protocol core; `snapback4/src`: the native module; `snapback4/web`: the wasm; `snapback4/ts`: the driver); the data-module runtime on every executor (`exactSaveTo`/`exactBodyFrom`, `Blob`, `Response.blob()`, `URL.createObjectURL`/`revokeObjectURL`: `js/src/prelude.js`, `js/src/wire.rs`, the runner's request, `host/web-js/ts-fetch.js`); `image` and `video` sources on the Apple, Linux and web hosts (`blob:` URLs); a profile photo (the first consumer)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Revised:** 2026-10-08
**Implementer:** Claude (Opus 5.5) lanes: slice 1 (lane D), once §0.3 is folded and a review agrees (a draft of it is on the local branch `lane/media`, not landed); slice 2 from 2026-10-09; slice 3 from 2026-10-12; slice 4 when Interview's maintainer takes it; slice 5 after slice 3 (§4)
**Related:** LLP 1011 §2 (`app:/data|cache|tmp` image sources on every host, the 1 MiB `data:` bound), LLP 1042 (video takes the same sources), LLP 1069.002 (the file picker hands back `app:/tmp/picked` paths; its D4 byte bodies, landed 2026-10-07, and `readFile` under `fs.read`), LLP 1027.001 (the `fs.*` grants and `storage.fs`), LLP 1027.000 (no clock or timers in a data module); Snapback 0.4.13: `CLIENT-AND-OPERATIONS.md` §Assets, `dist/assets.js`, `dist/asset-download.js`, `dist/local-assets.js`, device ops `upload_facts`, `forget_upload_facts`, `asset_home`; the web's [File API](https://www.w3.org/TR/FileAPI/) (`Blob`, `URL.createObjectURL`, `blob:` URLs); the app farm, `~/appfarm/synthesis/round-01.md` and `round-02.md`

## 0. Rulings (Charlie, 2026-10-07)

| Question (r1 §6) | Ruling | Where it lands |
| --- | --- | --- |
| Q1 — the first consumer | **Interview's profile photo** | §3 |
| Q2 — one-view assets (`image <=N once`) | **Build the memory-only source now** | D5: `Blob`, `URL.createObjectURL`, `blob:` sources on every host |
| Q3 — retention default | **Keep what was shown, up to 64 MiB** | D3 |
| Q4 — fetch straight to and from files | **Now, not with video** | D6 R2, slice 2 |
| Q5 — web storage eviction | **Re-download quietly**; no `navigator.storage.persist()` | D3 |

"Now" in Q2 and Q4 places those parts in this RFC's slices, ahead of video;
r3 orders them after the slice that needs neither (§4).

### 0.1 What changed between r2 and r3

- **R1 landed for byte bodies.** Since 2026-10-07 (`cfb4931da`, LLP 1069.002
  "Implementer and date") an `ArrayBuffer` or view body travels as
  `body_base64` into the runner's `Request.body`. Hermes no longer posts
  `"255,216,…"`. What is left of R1 is a `Blob` body, which comes with D5.
  `Response.arrayBuffer()` already reads a reply's bytes on every executor.
- **The client of 2026-10-08** (`c39356791`, `8bc0a8f74`, `b896050d7`):
  - **Shared partitions.** One device per partition file per page, shared by
    every client that opens it. So the media cache belongs to the partition,
    not to a client: one manifest, one directory, one set of in-flight
    downloads, whatever the number of sources that opened it (D2).
  - **Queued rounds.** A partition runs one round at a time, and a `sync()`
    waits its turn. The retention check runs at the end of each round, inside
    that queue, so it never interleaves with another round's device calls
    (D3).
  - **Server reads.** A query over an `online only` table, or one the device
    cannot vouch for, is answered by `POST /q/<name>`. A row read that way is
    not on the device. `asset_home` cannot confirm its asset, so it is never
    kept. It is shown transiently instead (D3).
  - **Device refusals are final.** A write naming `{ id }` whose upload facts
    this device lacks queues without a prediction, as in Snapback. A write the
    device's prediction refuses fails at once. Neither changes the media design.
- **The farm** (round 02 §4.2 #6, `round-02.md:26`, `:316`, `:390`):
  - **The gap.** Four of the five media-tagged v4 builds cut uploads, used
    bundled media, or hand-wrote byte HTTP. All five were GPT builds. Eleven
    v3 builds had media rows.
  - **The work-arounds.** The builds that did show photos took one of two
    roads:
    - hand-written `POST /assets` and `GET /assets/<id>` into an `app:/cache`
      file (wound-photo-timeline-0620, coaster-snap-vote-0085, museum-miles-0554);
    - base64 `data:` URLs, capped under LLP 1011's 1 MiB bound
      (monster-silhouette-0395 at 750,000 bytes, border-binder-0545 at
      280,000).
  - **What D1 settles.** Round 1 recorded that an `<img>` cannot send the
    persona header (`round-01.md:216-222`).
  - **The asks.** Most "most needed fix" lines ask for the same thing: a
    first-party picked file → upload → place → image path with a local cache
    (dementia-memory-lane-0627:99, wound-photo-timeline-0620:60,
    monster-silhouette-0395:87).

  The hand-written caches are D1's design without D3. They never purge, and
  nothing records upload facts, so an offline write naming the photo never
  predicts. No farm build asked for one-view photos or video. Slice 1 is
  therefore what the farm asked for.
- **No timers in a data module** (LLP 1027.000). r2's D4.3 had the driver
  wait out `E_RATE_LIMIT` "up to 10 s in all", as Snapback's client does with
  `setTimeout`. A data module cannot wait. r3 returns the refusal with its
  `retryAfter`, and the app asks again from Contract (D4).

### 0.2 Review round 1 (r3 → r4)

Both reviews (`llp/reviews/rfc-2026-10-08-1108-r3.{astra,grok}.md`) were
NOT READY. Both accepted the core/driver split, D3's online use of kept files,
the full-file `Range` interim rule, and that slice 1 adds no apparatus. What
r4 changes:

- **One media queue per partition** (D2). Both reviewers had the same
  blocker: a publication's write could race a removal, a clear or another
  publication, and recreate a file the manifest had dropped. Every step that
  changes the manifest now runs, with the file changes it asks for, in one
  queue.
- **Clear and close let in-flight work go** (D2). A reply that arrives after
  `clearMedia()` is stale and publishes nothing.
- **Upload sessions are checked twice** (D4.5): before the facts are kept,
  and after. A change after means `forget_upload_facts`, as Snapback's
  `local.js` does.
- **An observed `E_AUTH` or restore stops kept files showing** (D3).
- **Transient files are each their own** (D3): `<id>-<n>.<ext>`, so one
  consumer's release never removes another's.
- **The cap is enforced at each publication** as well as after rounds (D3).
- **Time** (D2): the driver passes the latest `now` an asset call gave, and
  never reads a clock. Nothing expires before the first asset call of a page.
- **The policy across launches** (D3): it is the opener's. A change applies
  at the first check, as Snapback's `keep: "none"` purges on open.
- **Exactness.** The completeness predicate, the decoded home receipt, the
  identity including `generation`, a refusal sent as `200 {denied}`, and
  `Retry-After` as seconds or a date are now stated in D2 and D4. `once`
  bytes are never written, whatever the status (D5). Linux plays no `video`
  (LLP 1042). The `fs.*` grants are cited from LLP 1027.001, not LLP 1030.002.
- **A removed file stops being served** (D3). On the web, `appURL` revokes
  the object URL of a path whose file is gone (`host/web/picker-glue.js`, four
  lines). That is slice 1's only host change.
- **Slices 2 and 3 have an implementer and a start date** (§4). Their order
  behind slice 1 is r3's reading of "now" (§0), which Charlie confirms or
  overrides (§6).
- **Tests** (D8) add the interleavings the reviewers named.

### 0.3 Review round 2 (r4): open

Both reviews (`llp/reviews/rfc-2026-10-08-1108-r4.{astra,grok}.md`) were NOT
READY. They agree that the core/driver split, D3's online use of kept files
and the full-file `Range` rule are sound, and that slice 1 adds no apparatus.
What r5 must settle:

- **Authority fences publication** (both). A download that started before a
  round observed `E_AUTH` or a restore must not publish, kept or transient.
  The refusal must persist until a round succeeds, not only until the next
  round of any kind, and it must remove every kept file, not only refuse
  lookups.
- **One media queue, and when it runs** (Grok). §0.1, D2 and D3 still name
  the round queue and "at open" in places where r4's fix says the media queue.
  `keep: "none"` and the stray sweep must run at open, not at the first
  `asset` call. Time needs an explicit `now` at open and for maintenance
  (Astra), not the last asset call's.
- **Session adoption** (Astra). Waiting for `refreshSession` does not mean the
  new bearer is in `headers()`. One session owner per partition, or an
  awaited adoption, before waiters resume.
- **Lifecycle around an upload's facts** (Astra). A close or clear after
  `upload_reply` and before the upload settles must forget the facts, and
  close must wait for that.
- **A returned path's lifetime** (Astra). The cap can evict a kept file that
  an answer just returned, before the image loads. Returned handles need a
  pin or a `release`, and the cap must respect it.
- **Revoking released transient URLs** (Astra). `appURL` revokes only when
  the same path is resolved again, and transient paths are unique, so a
  released file's object URL must be revoked on release, clear and close.
- **`once` precision** (Grok). A `416 E_ASSET_RANGE` spends no view. A `200`
  with `accept-ranges: none` has spent it. Neither is written. Any other
  `2xx` is written only with `accept-ranges: bytes`. Fix D5 and D8 to match.
- **R2 needs admission** (Astra). `exactSaveTo` must gate the write on the
  headers and keep refusal bodies, staging files that the media queue
  publishes.
- **Smaller fixes** (Grok):
  - D3 still names the stable transient path in one sentence.
  - Asset ids must be one ASCII alphanumeric segment, as the server's are.
  - D8's stray-file parenthetical is reversed.
  - "Empties its `app:/tmp` directory" means only
    `app:/tmp/snapback4/<partition>`.
  - Upload facts are recorded only while the device is open, under the
    opened viewer.
  - Slice 1 depends on the browser combining the server's two
    `Access-Control-Expose-Headers`.
- **Order** (Grok). Slice 1 is not the authorized next slice until Charlie
  answers §6's question about rulings Q2 and Q4.

## 1. Summary

A Snapback app stores photos and video as **assets**. A column declared `image <=N` or
`video <=N` holds one. The client uploads the bytes first (`POST /assets`), and the
server answers with the facts it established (`id`, `type`, `bytes`, `width`, `height`,
`duration`). The client then writes `{ id }` into the mutation. Reading an asset is
`GET /assets/<id>` under the viewer's own session; the home row's read rule decides
who may see it. A column declared `once` serves each principal one view.

The Snapback4 client in exact2 does none of this yet. This RFC proposes:

- **Showing.** The client downloads an asset with the member's session into
  the app's own files and hands back that path:
  `app:/cache/snapback4/<partition>/<asset id>.<ext>`. `image` and `video`
  already show an `app:/` file on every host (LLP 1011 §2, LLP 1042), so
  their file loading does not change.
- **Keeping.** Snapback's own retention rules, defaulting to keep what was
  shown up to 64 MiB, least recently used first. A file is purged when its
  row leaves the partition, is deleted or expires, and when the app clears the
  partition's media. The bytes live in files and the manifest lives in the
  partition.
- **Not keeping.** An ordinary asset the cache does not keep is a transient
  file under `app:/tmp/snapback4/<partition>/`, one per answer. It is removed
  when the app releases it, at sign-out, when the partition closes and when
  it next opens.
- **One-view assets.** These are shown from memory and never written to disk,
  through the web's own primitive: a `Blob` and `URL.createObjectURL`, which
  every executor gets (D5).
- **Uploading.** The picked file (`app:/tmp/picked/…`) goes to the server as
  bytes. The core then records the server's facts with the device
  (`upload_facts`), so a write naming the asset predicts offline and survives
  a reopen.
- **Exact's runtime gains, on every executor:**
  - `exactSaveTo` and `exactBodyFrom`, so a file moves between the network
    and the disk without crossing the JavaScript bridge (slice 2);
  - `Blob` (as a body too), `Response.blob()` and object URLs (D5, slice 3).
  Byte request bodies, r2's R1, landed on 2026-10-07 (§0.1).

## 2. Decisions

### D1 — Show an asset as an app file, never as an authenticated URL

Snapback offers two ways to show an asset. `client.asset(id)` downloads it and
returns a blob URL. `assetSource(id)` returns `{ uri, headers }` for views that can
send headers (React Native's `Image`). Neither fits Exact as it stands:

- **Headers on the view.** Every host's loader would need an authenticated mode.
  The web cannot have one: `<img>` and `<video>` send no `Authorization` header.
  The farm hit exactly this (`round-01.md:216-222`).
- **A token in the URL** (`?token=…`). It leaks the bearer into browser history,
  HTTP caches, server logs and the `Referer`, and Snapback does not accept it.
- **A `data:` URL.** It is bounded at 1 MiB on every host (LLP 1011 §2), which is
  smaller than a phone photo. Farm builds that took this road capped their
  photos at 280–750 KB.

An `app:/` file has none of these problems. It works the same on the web
(the page's file store, shown through an object URL by `picker-glue.js`
`appURL`), on Apple and on Linux. It shows offline from the first frame. A
data module produces one string, the same on every host, and `video` takes
the same sources (LLP 1042, its `src` row).

A file is named for its type (`.jpg`, `.png`, `.webp`, `.mp4`), from the
reply's `content-type`. The web's `appURL` types the blob by extension, and
native players want `.mp4` (Snapback's Expo cache does the same).

What is kept stays on disk, which is what retention means. A one-view asset must
never reach the disk; D5 gives it a memory-only source instead.

### D2 — Where the work happens

The client is a protocol core that performs no I/O (`snapback4/client`). That
stays true: the core decides and the driver moves bytes. Bytes never enter
the core, because the wasm boundary passes JSON and copying a photo through
it would be pointless. The core holds the manifest and the exchanges in
flight. Since partitions are shared (§0.1), there is one core per partition,
and every client of the partition asks the same one.

| Step | Core (`Client`, `media.rs`) | Driver (TS `Snapback`) |
| --- | --- | --- |
| Show asset `id` | `asset {id, now}`. After an observed `E_AUTH` or restore, `{denied}`. A kept entry whose home the device still confirms (`asset_home`) answers `{path, type, bytes, kept: true}`. Otherwise it answers `{fetch}` for `GET /assets/<id>` with `range: bytes=0-` (D5). | In the queue: checks a kept path is whole (`stat`: present, `bytes` long); if not, `asset_lost {id}` and asks again. Out of it: performs the fetch. |
| The reply | `asset_reply {exchange, reply: {status, headers, size} \| {status, body} \| {error}, now}`. It reads the status and the headers `content-type`, `x-snapback-asset-home`, `accept-ranges` and `content-range`, and asks the device for the home (`asset_home`). Kept: it records the manifest entry **before** the file counts, evicts down to the cap, and answers `{keep: path}`. Shown, not kept: `{transient: path}`. A `once` asset: never written (D5). A refusal: `{denied}`. | In the queue, with the reply's core call: removes what it names, then writes the bytes to the path (`atomicWriteFile`). A failed write is `asset_lost {id}`. |
| After every round, and at open | `media_due {now}` answers `{remove: [paths]}`: entries whose home changed, left the partition or expired, and the least recently used above the cap. The entries leave the manifest first. | Removes the files, in the queue. |
| At open | `media_files` answers every path the manifest names. | Removes every other file under the partition's cache directory (a write cut off by a crash), and empties its `app:/tmp` directory. |
| Release | — | Removes a transient file the app is done with (`release(path)`). |
| `clearMedia()` (sign-out) | `media_clear` answers every kept path, empties the manifest, and lets every download and upload in flight go: their replies are stale. | Removes those files and every transient file, in the queue. |
| Upload | `upload` answers `{fetch}` for `POST /assets`. `upload_reply {exchange, reply, changed}` records `upload_facts` with the device and answers `{ok: true, asset}`, or answers the refusal. | Reads the picked file (`readFile`; `exactBodyFrom` from slice 2) and posts it. `changed` is whether a session refresh started, ran or is running, or `headers()` now gives other credentials. Checked again after `upload_reply` (D4.5). |

**Replies.** A media reply carries what these steps need: the status, the
four headers above and `retry-after`, and the body's length (`size`) or, for
a refusal, its JSON. The round's `Reply` type stays as it is. Only media ops
read headers, so they take their own reply shape, and the sync protocol is
untouched. As Snapback's `local-assets.js` reads them:
- **The home receipt.** `x-snapback-asset-home` is percent-encoded JSON of
  `{store_id, table, row, column}`. One that does not decode to that is no
  receipt, and nothing is kept.
- **Whole.** A whole object is a `200`, or a `206` whose `content-range` is
  `bytes 0-(n-1)/n` with `n` equal to the body's length. Anything else is
  refused as `E_HTTP_RESPONSE`, retryable.
- **The same home.** `asset_home` adds `generation`, `expiresAt` and
  `tables`. An entry stands only while the confirmed home's
  `(store_id, table, row, column, generation)` is unchanged. A changed one is
  dropped, never rewritten.

**The queue.** The driver runs every step that changes the manifest in one
queue per partition, together with the file changes that step asks for:
- the kept check (`asset`, `stat`, `asset_lost`);
- a publication (`asset_reply`, its removals, the write);
- `media_due`, `media_clear` and the open sweep.

So a write never races a removal, and a path the core answered exists when
it is returned. Downloads and uploads themselves run outside the queue, so
one slow transfer does not hold up the others.

**Exchanges.** Each media exchange is named, as the round's are
(`<incarnation>.asset<n>`, `.upload<n>`). A reply naming an exchange the core
did not hand out is refused (`E_STALE`), and so is one it already answered or
one `media_clear` let go. Downloads and uploads run beside rounds. Several
may be in flight at once.

**Time.** Rounds take no `now`. After a round, `media_due` uses the latest
`now` an asset call of this page passed. The driver never reads a clock
(LLP 1027.000). Before a page's first asset call, nothing is checked and
nothing expires. Every `asset` call checks its own entry at its own `now`, so
an expired file is never handed out.

**The manifest.** It lives in the partition's metadata as `exact:media`:
`[{id, path, type, home, bytes, used}]`.
- `home` is what `asset_home` confirmed: `{store_id, table, row, column,
  generation, expiresAt, tables}`.
- `used` is the `now` of the read that showed the asset, because the core has
  no clock (LLP 1027.000).
- **The entry is written before the bytes,** as in Snapback's `MediaCache`.
  A file that a crash left missing or short is found at the next read
  (`stat`). A file the manifest does not name is removed at the next open.

### D3 — Retention follows Snapback's rules; Exact keeps what was shown

**Default (ruling Q3):** `keep: "viewed"` with `maxBytes: 64 MiB`. Snapback's own
default is `keep: "none"`. An offline-first app that forgets every photo looks
broken offline, so Exact's driver defaults the other way. An app can set
`keep: "none"`, or another cap, when it opens the client. Within a page, the
partition's first opener sets the policy, and a later open with another
policy is refused (`E_PARTITION_ASSETS`). Across launches the policy is the
opener's. The first check applies it to what the manifest holds:
`keep: "none"` purges everything, as Snapback's does on open, and a smaller
cap trims the least recently used.

Under `viewed`:

- **What is kept.** All of these must hold:
  - the response is the whole object (`200`, or a `206` covering every byte);
  - it says `accept-ranges: bytes`;
  - its size is within the cap;
  - the device confirms the home (`asset_home`): the row is synced here, the
    viewer may read it, and it holds this asset in a non-`once` column.

  An ordinary asset that fails any of these is transient. A `once` asset
  (`accept-ranges: none`) goes to memory (D5).
- **When it is checked.**
  - After every round, inside the partition's round queue.
  - At open.
  - At each `asset` call, for the one entry asked about.

  Every entry is checked after a round. Snapback skips entries whose home's
  `tables` the commit did not touch. The client has no per-round footprint
  yet, and an entry costs one device call.
- **When it is removed.**
  - Its home no longer matches, or its generation changed.
  - It expired, by the `now` the driver passes.
  - It is the least recently used and the total is over the cap. The cap is
    enforced at each publication, against every other entry, as well as
    after rounds.
  - The app calls `clearMedia()`.
  - The partition is replaced (`E_STORE_RESTORE`).
- **Online, a kept file is shown without a download.** This differs from
  Snapback, whose `client.asset` downloads again on every online fetch and
  uses kept bytes only offline. Here the device's rows are the authority, as
  they are for every read. `asset_home` is asked at each `asset` call, so an
  asset whose row was deleted, cleared, left the sync horizon or became
  unreadable stops being handed out at the first round that learns it.
  Until then it shows, exactly as that row's text does. The cost of the other
  choice is a download per avatar per screen. The farm's hand-written caches
  existed to avoid that (museum-miles-0554:70-71).
  - **What the device cannot know.** `asset_home` checks the row, not the
    session. A round refused with `E_AUTH`, or a partition restore
    (`E_STORE_RESTORE`), stops every kept file being handed out at once,
    until a round succeeds. A revocation no round has observed waits for one,
    as the row does.
  - **What "stops showing" means.** The next `asset` answer for it no longer
    names the file, and the file is removed. A view already showing it keeps
    its pixels until its resource answers again, as a browser's `<img>` does.
    On the web, `appURL` revokes the object URL it made for a path whose file
    is gone, the next time the path is resolved
    (`host/web/picker-glue.js`).
- **Offline.** A kept file is shown. An asset that is not kept answers
  `E_OFFLINE` with `retryable: true`, as Snapback's does.
- **Evicted by the browser (ruling Q5).** A browser may clear the page's file
  store under storage pressure. A manifest entry whose file is missing or
  short counts as not kept: the entry is dropped (`asset_lost`), and the asset
  is fetched again online or answers `E_OFFLINE`. Exact does not call
  `navigator.storage.persist()`.

**Transient files.** An ordinary asset the cache does not keep is written to
`app:/tmp/snapback4/<partition>/<id>.<ext>`. This covers:
- a home the device cannot confirm, such as a row from a server read;
- an asset over the cap;
- everything under `keep: "none"`.

Each answer gets its own file, `<id>-<n>.<ext>`, so one consumer's
`release(path)` never removes another's. The file is removed when the app
releases it, at `clearMedia()`, when the partition closes, and when it next
opens. It never counts as kept: offline, a fresh `asset` call for it answers
`E_OFFLINE`, though a path the app already holds keeps showing until it is
removed. A transient file is not a one-view photo, which never reaches the
disk (D5).

### D4 — Upload

1. **Pick.** The member picks a file (LLP 1069.002). The data module gets
   `app:/tmp/picked/<n>.jpg`.
2. **Send.** The driver posts the file to `/assets` as
   `application/octet-stream` with the session.
   - In slice 1 it reads the file with `storage.fs.readFile` and posts the
     bytes, which cross the bridge once each way. A 2 MB avatar costs little.
   - From slice 2, `exactBodyFrom` (D6) sends the file without that crossing.

   Uploads are online only, as in Snapback. Offline, the answer is
   `E_OFFLINE`, and the app keeps the picked file until the member is online.
   Nothing queues the bytes.
3. **Success.** The core calls `upload_facts` with the device. A write naming
   `{ id }` then predicts offline, before and after a reopen.
4. **Refusal.** On `E_ASSET_TYPE`, `E_ASSET_TOO_LARGE` or `E_ASSET_BOUNDS` the
   app shows why. A refusal may come as `200 {denied}`. Success is a reply
   whose body names `asset.id`, nothing less. `E_RATE_LIMIT` is returned
   with `retryable` and `retryAfter` in milliseconds. `Retry-After` is
   seconds, or an HTTP date that the driver turns into milliseconds from the
   caller's `now` (`upload(path, now)`). A data module has no timers
   (LLP 1027.000), so the app asks again from Contract, for example from an
   action a `task` runs. An upload is never retried by the driver. A lost reply
   may follow a stored upload, and the server's one-asset-one-home rule
   (`E_ASSET_PLACED`) makes a duplicate harmless but wasteful.
5. **A changed session.** If the session changes during the upload (D7), the
   upload ends with `E_AUTH`, "start the upload again". The driver checks
   twice, as Snapback's `local.js` does:
   - **Before the facts are kept.** If it changed while the bytes were in
     flight (`changed`), the core records nothing.
   - **After they are kept.** If it changed while the device kept them, the
     driver calls `forget_upload_facts`.

   "Changed" means a session refresh of the partition started, ran or is
   running, or `headers()` gives other credentials than it sent. So the asset
   is never recorded against the wrong session. The facts are keyed by the
   partition's viewer, and every client of a partition presents that
   viewer's session.

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
  the host, so a one-view photo never crosses the bridge. A `Blob` is also a
  `fetch` body (what remains of r2's R1).
  `URL.createObjectURL(blob)` returns `blob:<app id>/<uuid>`, the web's form (an
  origin, then a UUID). `URL.revokeObjectURL(url)` frees it.
- **In `image` and `video`.** A `blob:` source is resolved from the store, on every
  host, as an `app:/` source is resolved from the file roots:
  - **web:** the browser's own object URL;
  - **Apple:** `image` decodes from the held bytes (`CGImageSource` over `Data`);
    `video` plays through an `AVURLAsset` on a custom scheme whose
    `AVAssetResourceLoaderDelegate` answers byte ranges from the store;
  - **Linux:** `exact_raster` decodes an `image` from the bytes. Linux
    plays no `video` at all (LLP 1042: no decoder or audio output), so a
    `blob:` video there is listed in `state.media` as an `app:/` one is.

  A revoked or unknown `blob:` URL is an `error` (LLP 1011 §2), "the object URL was
  revoked", as the web's `<img>` fails on one.
- **Lifetime.** As on the web, until `revokeObjectURL` or until the app's process
  ends. Nothing is written to disk: no spooling to a temporary file, and no image
  cache keyed by the URL that outlives a revoke. A view still showing a picture
  keeps its decoded pixels after a revoke, as a browser's `<img>` does.
- **A bound.** The store holds at most **256 MiB** per app. A `Blob` that would
  exceed it is refused (`QuotaExceededError`), so a leak of object URLs fails loudly
  instead of growing without limit.

The Snapback client uses it for `once` assets only (`accept-ranges: none`).
Every other asset the cache does not keep is a transient file (D3). The
client hands back the `blob:` URL and revokes it when the app releases it, or
at close. A one-view photo therefore spends its view only when it is shown,
and leaves nothing behind.

**Every download asks for `Range: bytes=0-`,** as Snapback's does ("a
full-file Range request makes ordinary retries non-consuming"). An ordinary
asset answers `206` covering every byte, which D3 keeps. A `once` asset
refuses the range with `E_ASSET_RANGE` before its view is spent. In slice 3
the client then asks once more without the range, into memory.

**Until slice 3,** that refusal answers `E_ASSET_ONCE_UNSUPPORTED`, naming
this slice. The view is not spent and nothing is written. A reply that says
`accept-ranges: none` is answered the same way whatever its status, and its
bytes are never written.

### D6 — The Exact runtime: files to and from the network (slice 2)

**R1 — byte request bodies.** Landed 2026-10-07 for `ArrayBuffer` and views
(§0.1). A `Blob` body comes with D5.

**R2 — to and from files (ruling Q4).** Two `fetch` options:
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
each direction. Slice 1's avatars do not need it. Video does.

### D7 — Sessions

Every asset exchange carries the bearer that the driver's `headers()` gives
at that moment.
- **During a refresh.** A download that starts while a refresh is in flight
  (`refreshSession`, one per partition) waits for it, and then uses the new
  bearer.
- **A download refused with `E_AUTH`** after a refresh rotated the token is
  asked once more with the new token.
- **An upload is not asked again** (D4.5).

### D8 — Testing and agents

- **Runtime tests** (slices 2 and 3), on Hermes and the JS target:
  - a `Blob` body arrives byte for byte;
  - `exactSaveTo` writes the whole file or nothing, including a cut-off transfer;
  - `exactBodyFrom` sends the file;
  - `Response.blob()` never surfaces its bytes in JavaScript;
  - an object URL shows in an `image` and plays in a `video` on the web, macOS, iOS
    and Linux;
  - a revoked one fails;
  - the 256 MiB bound refuses.
- **Client tests** against a real `snapback4 dev` with an `image <=1MB` column
  and an `image <=1MB once` column, in `tests/client.rs` (the core through
  the native module) and `ts/snapback.test.ts` (the TS driver and the wasm
  over a file store). Slice 1 covers:
  - upload, write, sync, download, retention across a reopen;
  - a home row deleted on another device, so its file is removed after the
    next round;
  - expiry by `now`;
  - the LRU cap;
  - offline shows the kept file;
  - a missing file (browser eviction) is fetched again;
  - a server-read home is transient;
  - a reply naming no exchange is refused;
  - a `once` asset answers `E_ASSET_ONCE_UNSUPPORTED`, and a second plain
    `GET` still succeeds (its view was not spent);
  - two sources showing the same asset at once share one whole kept file;
  - `clearMedia()` while a download is in flight: the reply is stale and
    writes nothing;
  - a session that changes while the upload's bytes are in flight records
    nothing;
  - the cap enforced at a publication, with no round between;
  - each transient answer is its own file, released alone;
  - a stray file (a crash between write and entry) swept at the next open,
    and a missing one (an entry without its file) fetched again;
  - a second client of the partition with another policy is refused.

  Slice 3 adds a `once` asset shown from memory, never written, whose second
  request is `E_ASSET_VIEWED`.
- **Drives.** The agent picks a file (`tap <input> drop <png>` on the web,
  `type @t` elsewhere, LLP 1069.002). It saves the photo and checks the
  `image`:
  - shown, then still shown after a reload;
  - shown to a second persona;
  - on the web in slice 1, online, offline after a reload (`--fail-fetch`
    on the origin), and from a second persona;
  - on macOS and iOS, and with `fail fetch` on the asset route, in slice 4.
    This machine's toolchain builds no Apple app, so slice 1 drives the web
    only. The native path runs the same driver over Hermes and the native
    module (`tests/media.rs` drives the core through that module), but slice 1
    does not claim it driven;
  - on a physical iPhone through Interview's LAN mode
    (`npm run backend -- --lan`), in slice 4.

## 3. The consumer: a profile photo (ruling Q1)

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

Interview lives outside this repository. Slice 1 proves the same flow in a
scratch app made by `exact new`, driven on the web: two personas, a `people`
table with that column, and the picker. The client README's asset section
carries that app as its example, as it carries the guestbook. Interview
adopts it in slice 4.

## 4. Slices

1. **The client, on today's runtime** (lane D, 2026-10-08):
   - **Core.** `media.rs` holds the asset, upload and retention calls of D2,
     with their exchanges and the manifest.
   - **TS driver.** `asset`, `upload`, `release` and `clearMedia`, over the
     runtime as it is: `fetch` with an `ArrayBuffer` body,
     `Response.arrayBuffer()`, and `storage.fs` (`readFile`,
     `atomicWriteFile`, `stat`, `readdir`, `rm`, `mkdir`). It runs on every
     executor a TypeScript source has: the web's realm over the wasm device,
     and Hermes over the native module.
   - **Grants.** The app grants `fs.read app:/tmp/picked`, plus `fs.read` and
     `fs.write` for `app:/cache/snapback4` and `app:/tmp/snapback4`.
   - **`once`.** Refused, by D5's interim rule.
   - **The web host.** `appURL` revokes the URL of a removed file (D3).
   - **Tests and docs.** D8's slice-1 client tests, the README's asset
     section, and the drive.
   - **Not in this slice:** the Rust driver. A Rust source's driver
     (`host_request`/`host_reply`) moves bytes only from slice 2, which gives
     it `save_to` and `body_from`. Until then a Rust source has no asset
     path.
2. **The runtime's files** (Claude (Opus 5.5) lanes, from 2026-10-09): R2 on Hermes, the JS target and the
   native HTTP executors, and Rust `save_to` and `body_from`. The TS driver
   switches to them, and the Rust driver is added. D8's R2 tests.
3. **Memory** (Claude (Opus 5.5) lanes, from 2026-10-12): D5 (`Blob`, `Response.blob()`, object URLs,
   `blob:` sources in `image` and `video` on the web, Apple and Linux). The
   client shows `once` assets from memory. D8's D5 tests.
4. **Interview's profile photo** (Interview's maintainer; its repository is
   outside this one), §3, driven on the web, macOS,
   iOS and a physical iPhone.
5. **Video** (Claude (Opus 5.5) lanes, after slice 3): a consumer that plays an asset (`video <=N`),
   through the same paths. Nothing new in the runtime after slices 2 and 3.

Each slice lands on its own. No slice changes what an app wrote against an
earlier one: `asset` keeps answering a path, or a `blob:` URL from slice 3.

## 5. Cost

- **Slice 1:** about 450 lines in the core, 250 in the TS driver, and 600
  of tests. No runtime change; four lines in the web host's `appURL`.
- **Slice 2:** about 300 lines over Hermes, the JS target and the native HTTP
  executors, plus about 100 in the drivers.
- **Slice 3:** about 1,100 lines. The Apple video resource loader is the
  largest piece. The Hermes `Blob` and the host store are the second.
- **Slice 4:** about 200 lines in Interview.
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
- **D3's online use of kept files without a download** (a deviation from
  Snapback, argued there).
- **Slice 1's transient files** for ordinary assets the cache does not keep, in
  place of r2's memory for everything not kept.
- **For Charlie: the order of slices 2 and 3.** r3 reads ruling Q2's and
  Q4's "now" as "in this RFC, ahead of video", and puts both behind slice 1,
  which the farm asked for and which needs neither. If "now" means "first",
  slice 2 is next and slice 1's driver switches to it when it lands.
