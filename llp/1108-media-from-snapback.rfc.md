# LLP 1108: Photos and video from Snapback, as the app's own files

**Type:** RFC
**Status:** Draft r6, 2026-10-08: NOT READY after review rounds 3 and 4 (GPT-6 Astra and Grok 4.7, both NOT READY on r5 and on r6). Round 2 is folded in r5 (§0.3), round 3 in r6 (§0.4); round 4's findings are open (§0.5). Charlie's order ruling is recorded (§0). Slice 1 is built against r6 on the local branch `lane/media` (not landed) and driven on the web, macOS and an iOS simulator. r4 was NOT READY after two review rounds (GPT-6 Astra and Grok 4.7, both NOT READY on r3 and on r4). r3 brought it up to the client as it is on 2026-10-08 and to the app farm's evidence (§0.1), and made the stages slices. r2 (2026-10-07) folded Charlie's rulings (§0).
**Systems:** the Snapback4 client (`snapback4/client`: the protocol core; `snapback4/src`: the native module; `snapback4/web`: the wasm; `snapback4/ts`: the driver); the data-module runtime on every executor (`exactSaveTo`/`exactBodyFrom`, `Blob`, `Response.blob()`, `URL.createObjectURL`/`revokeObjectURL`: `js/src/prelude.js`, `js/src/wire.rs`, the runner's request, `host/web-js/ts-fetch.js`); `image` and `video` sources on the Apple, Linux and web hosts (`blob:` URLs); a profile photo (the first consumer)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Revised:** 2026-10-08
**Implementer:** Claude (Opus 5.5) lanes: slice 1 (lane D) first, 2026-10-08, once §0.5 is folded and a review agrees (its code is on the local branch `lane/media` in `~/projects/exact2-wt-laneD`, not landed); slices 2 and 3 (another lane, concurrently, from 2026-10-08); slice 4 when Interview's maintainer takes it; slice 5 after slice 3 (§4)
**Related:** LLP 1011 §2 (`app:/data|cache|tmp` image sources on every host, the 1 MiB `data:` bound), LLP 1042 (video takes the same sources), LLP 1069.002 (the file picker hands back `app:/tmp/picked` paths; its D4 byte bodies, landed 2026-10-07, and `readFile` under `fs.read`), LLP 1027.001 (the `fs.*` grants and `storage.fs`), LLP 1027.000 (no clock or timers in a data module); Snapback 0.4.13: `CLIENT-AND-OPERATIONS.md` §Assets, `dist/assets.js`, `dist/asset-download.js`, `dist/local-assets.js`, device ops `upload_facts`, `forget_upload_facts`, `asset_home`; the web's [File API](https://www.w3.org/TR/FileAPI/) (`Blob`, `URL.createObjectURL`, `blob:` URLs); the app farm, `~/appfarm/synthesis/round-01.md` and `round-02.md`

## 0. Rulings (Charlie, 2026-10-07)

| Question (r1 §6) | Ruling | Where it lands |
| --- | --- | --- |
| Q1 — the first consumer | **Interview's profile photo** | §3 |
| Q2 — one-view assets (`image <=N once`) | **Build the memory-only source now** | D5: `Blob`, `URL.createObjectURL`, `blob:` sources on every host |
| Q3 — retention default | **Keep what was shown, up to 64 MiB** | D3 |
| Q4 — fetch straight to and from files | **Now, not with video** | D6 R2, slice 2 |
| Q5 — web storage eviction | **Re-download quietly**; no `navigator.storage.persist()` | D3 |
| r4 §6 — the order of slices 1–3 (2026-10-08) | **Slice 1 first ("after"), then slices 2 and 3 ("let's do all of these")** | §4 |

"Now" in Q2 and Q4 places those parts in this RFC's slices, ahead of video.
On 2026-10-08 Charlie answered r4's §6 question: slice 1 goes first, and
slices 2 and 3 follow ("let's do all of these"). Slices 2 and 3 are a
separate lane that runs concurrently; slice 1's driver moves to
`exactSaveTo` and the memory source as they land (§4).

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
    waits its turn. At the end of each round, still in that queue, the round
    hands the retention check to the partition's media queue and waits for
    it (D2, "The queue").
  - **Server reads.** A query over an `online only` table, or one the device
    cannot vouch for, is answered by `POST /q/<name>`. A home that
    `asset_home` cannot confirm is never kept, whatever answered the read
    (an `online only` row is not in the partition at all). Its asset is
    shown transiently instead (D3).
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
- **Exactness.** The completeness predicate, the decoded home receipt and
  the identity including `generation` are now stated in D2; an upload's
  refusal sent as `200 {denied}`, and `Retry-After` as seconds or a date, in
  D4. `once`
  bytes are never written, whatever the status (D5). Linux plays no `video`
  (LLP 1042). The `fs.*` grants are cited from LLP 1027.001, not LLP 1030.002.
- **A removed file stops being served** (D3). On the web, `appURL` revokes
  the object URL of a path whose file is gone (`host/web/picker-glue.js`, four
  lines). That is slice 1's only host change.
- **Slices 2 and 3 have an implementer and a start date** (§4). Their order
  behind slice 1 is r3's reading of "now" (§0), which Charlie confirms or
  overrides (§6).
- **Tests** (D8) add the interleavings the reviewers named.

### 0.3 Review round 2 (r4 → r5)

Both reviews (`llp/reviews/rfc-2026-10-08-1108-r4.{astra,grok}.md`) were NOT
READY. They agree that the core/driver split, D3's online use of kept files
and the full-file `Range` rule are sound, and that slice 1 adds no apparatus.
What r5 changes, each with a test in D8 that failed against r4's draft:

- **Authority fences publication** (both; D3 "Authority"). A round refused
  with `E_AUTH` or `E_STORE_RESTORE` fences the partition's media: every kept
  and transient file is removed at once, `asset` fetches nothing, a download
  handed out before the fence publishes nothing, and an upload records no
  facts. The fence is kept in the partition (`exact:media-fence`), survives a
  reopen and any other refusal, and only a round that succeeds lifts it.
- **One media queue, and when it runs** (Grok; D2 "The queue", "Time"). Every
  step that reads or changes the manifest runs in the partition's media
  queue; a round only hands it `media_due` and waits. The open sweep and the
  policy (`keep: "none"` purges, a smaller cap trims) run at open, before
  `open` returns, with no `asset` call. Time is never reused across calls:
  `asset(id, now)` checks its own entry, and `sync(now)` and `open({now})`
  check every entry's expiry; without `now`, everything but expiry is
  checked.
- **Session adoption** (Astra; D7). `refreshSession(now, adopt)` installs the
  new session through `adopt` before the refresh settles, so every waiter
  resumes with the new bearer in `headers()`.
- **Lifecycle around an upload's facts** (Astra; D4.5). After the facts are
  kept, a close, a `clearMedia()`, a fence or a changed session forgets them
  (`upload_forget`) before the upload answers; `close()` and `clearMedia()`
  wait for that step. A failed forget says the facts may remain.
- **A returned path's lifetime** (Astra; D3 "A returned path"). A kept answer
  is pinned until `release(path)`, `clearMedia()` or close; the cap evicts
  unpinned entries only, and a photo that needs room only pinned entries
  hold is transient instead. Authority still removes a pinned file.
- **Revoking released URLs** (Astra; D3 "Removed files on the web"). Removing
  an app file through `storage.fs` on the web revokes the object URL `appURL`
  made for it and for every path under it, at once.
- **`once` precision** (Grok; D5). `416 E_ASSET_RANGE` spends no view; a `2xx`
  with `accept-ranges: none` has spent it and says so; neither is written; any
  other `2xx` is written only with `accept-ranges: bytes`.
- **R2 needs admission** (Astra). `exactSaveTo` is slice 2's, which the
  slices 2–3 lane owns; it folds this finding into D6.
- **Smaller fixes** (Grok): D3 names only the per-answer transient path; an
  asset id is one ASCII alphanumeric segment, as the server's; D8's stray-file
  parenthetical is the right way round; the open sweep empties only
  `app:/tmp/snapback4/<partition>`; upload facts are recorded only while the
  device is open, under the opened viewer; D2 states the web's dependence on
  the browser combining the server's two `Access-Control-Expose-Headers`.
- **Order** (Grok). Charlie answered §6's question on 2026-10-08: slice 1
  first, then slices 2 and 3 (§0).

### 0.4 Review round 3 (r5 → r6)

Both reviews (`llp/reviews/rfc-2026-10-08-1108-r5.{astra,grok}.md`), over r5
and its code, were NOT READY. They found the design sound (the queues, pins
and the cap, `now`, the open sweep, `once`, adoption) and the code short of
it at its awaits. What r6 changes, each with a test that failed before it:

- **A fence, a clear or a close during a publication's awaits** (Grok). The
  core answers a publication with the fence count it saw (`fences`). After
  the write, the driver asks again (`media_fence`) and checks `clearMedia()`
  and close: if any moved, the file goes and the answer is the refusal (D2).
- **The upload's second check** (both). It now checks the fence the same way,
  and the upload's answer is the promise the check settles, so nothing comes
  between the check and the answer (D4.5).
- **Removals that fail** (both). Only a file already gone counts as removed.
  Any other failure is kept, tried again at every media step, reported by
  `clearMedia()` (which rejects, `E_MEDIA_CLEANUP`) and by a round
  (`Round.media`). A transient file stays tracked until it is gone. An open
  sweep that failed runs again after the next round (D3).
- **The fence's own write** (both). A write that does not land is tried
  again at every media step and before a rebuilt device saves; the fence
  holds in memory meanwhile. The web's rebuild of a device carries the fence
  with the outcomes it carries (D3).
- **URLs on the web** (both). A removal announces itself on a channel
  (`BroadcastChannel`, `exact-files:<store>`), so a worker's or another
  tab's removal revokes the page's URL; and `appURL` gives no URL to a path
  removed (or under a folder removed) while it was reading it (D3).
- **Native drives** (Astra). Slice 1 is now driven on macOS and an iOS
  simulator as well as the web (D8). Driving them found a fault outside
  media: natively, a superseded answer's round gets its reply after the
  device refuses that answer's calls, its cancel fails too, and the client
  then answers every later round `busy`. The driver now cancels such a round
  at the start of the next one. It also found that Snapback4's first native
  open (adopting the server's backend, about 570 ms) runs past the 100 ms
  step budget, which the drive's app raised; that is the client's, not this
  RFC's, and is left in `QUEUE.md`.
- **D3's "what is kept"** (Astra) said a response that is not whole becomes
  transient; it is refused, as D2 says.

### 0.5 Review round 4 (r6): open

Both reviews (`llp/reviews/rfc-2026-10-08-1108-r6.{astra,grok}.md`), over r6
and its code, were NOT READY. They agree the design is sound (the queues,
the fence and where it is kept, pins and the cap, `once`, `now`, the open
sweep, adoption, the web's URLs, the order ruling) and that round 2's and
most of round 3's findings are in the code. What remains is in the driver's
and the core's handling of interleavings:

- **Settlement after the last await** (Astra). `interrupted()` checks, then
  its caller resumes a microtask later: a close in between lets an upload
  answer success with its facts kept. The check must run in the same step
  that settles the answer, for an upload and for a publication.
- **A kept hit skips the re-check** (both). `asset` answers a kept path after
  awaiting `removeAnswered` and `stat`; a fence, a clear or a close raised
  meanwhile does not stop the path being returned. Carry the fence count on
  the kept answer and check it, `clearMedia()` and close, at settlement.
- **A round of a closed client skips its purge** (Grok). `mediaDue` returns
  at once when the client is closed, so a round that observes `E_AUTH` after
  `close()` began removes nothing. Run it through the partition's device,
  and have `close()` wait for the partition's rounds.
- **A web rebuild loses the policy and the pins** (Astra). The rebuilt core
  starts with the default policy and no pins: `keep: "none"` keeps, and an
  unreleased file can be evicted. Carry both through `held`/`hold`.
- **An old release unpins a new answer** (Astra). `asset_lost` drops a
  path's pins, and the refetched file has the same path, so releasing the
  first answer removes the second's pin. Keep release accounting across a
  loss, or give answers handles.
- **A failed close cleanup is abandoned** (Astra). A transient file that
  cannot be removed at close leaves the partition marked clean; `close()`
  must report it and stay retryable.
- **An unsaved fence is lost at a native close** (Astra). If its writes keep
  failing, the fence lives only in memory, and closing drops it. Save it
  before close completes, or refuse to complete.
- **Smaller** (Grok): `appURL`'s removal walk stops before `app:/cache`,
  `app:/tmp` and `app:/data` themselves; `media_fence` should return the
  stored refusal, so a restore is not reported as `E_AUTH`; D8's "lifted
  before the reply" case needs a TypeScript test that releases the body
  after the lifting round.

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
| Show asset `id` | `asset {id, now}`. An id that is not one ASCII alphanumeric segment is refused (`E_INPUT`), as the server refuses it. Fenced (D3, "Authority"): the refusal, with every kept path in `remove` and `purge`. A kept entry whose home the device still confirms at `now` answers `{path, type, bytes, kept: true}` and is pinned (D3, "A returned path"). Otherwise `{fetch}` for `GET /assets/<id>` with `range: bytes=0-` (D5). | In the queue: checks a kept path is whole (`stat`: present, `bytes` long); if not, `asset_lost {id}` and asks again. Out of it: performs the fetch. |
| The reply | `asset_reply {exchange, reply: {status, headers, size} \| {status, body} \| {error}, now}`. Refused, publishing nothing, if a fence stands or was observed after the download was handed out. Its answer carries the fence count it saw (`fences`). It reads the status and the headers `content-type`, `x-snapback-asset-home`, `accept-ranges` and `content-range`, applies D5's `once` rules, and asks the device for the home (`asset_home`). Kept: it records the manifest entry **before** the file counts, makes room among unpinned entries, and answers `{keep: path}`, pinned. Not kept (no confirmed home, over the cap, no room among unpinned entries, `keep: "none"`): `{transient: path}`. A refusal: `{denied}`. | In the queue, with the reply's core call: removes what it names, then writes the bytes to the path (`atomicWriteFile`). A failed write is `asset_lost {id}`. After the write it asks `media_fence`: a fence observed since `fences`, a `clearMedia()` or a close since the download began removes the file (`asset_lost` for a kept one) and answers the refusal. |
| After every round | `media_due {now?}`. Fenced: every kept path, with `purge`. Otherwise entries whose home changed or left, those expired by `now` when the round was given one, and the least recently used unpinned entries above the cap. Entries leave the manifest first. | Removes the files, in the queue; on `purge`, every transient file too. |
| At open | `media_open {now?}`: nothing when the partition has never shown an asset. Otherwise `media_due`'s removals, every path the manifest names, and the two directories. | In the queue, before `open` returns: removes those files, every other file under `app:/cache/snapback4/<partition>` (its entry was dropped, and a crash came before the file was removed), and everything under `app:/tmp/snapback4/<partition>` (an earlier run's transient files). Nothing else under `app:/tmp`. |
| Release | `media_release {path}`: unpins a kept path once. | Removes a transient file (`release(path)`). |
| `clearMedia()` (sign-out) | `media_clear`: every kept path; the manifest empties, pins drop, and every download and upload in flight is let go: their replies are stale. | Waits for uploads already keeping their facts (D4.5), then removes those files and every transient file, in the queue. |
| Upload | `upload` answers `{fetch}` for `POST /assets`. `upload_reply {exchange, reply, changed}` records `upload_facts` under the opened viewer, only while the device is open and no fence was observed since the upload began, and answers `{ok: true, asset}` or the refusal. `upload_forget {id}` forgets them (`forget_upload_facts`). | Reads the picked file (`readFile`; `exactBodyFrom` from slice 2) and posts it. `changed` is whether a session refresh started, ran or is running, or `headers()` now gives other credentials. Checks again after `upload_reply` (D4.5). |

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
- **On the web, through CORS.** `content-type` is safelisted. The others reach
  the driver only if exposed. Snapback 0.4.13's asset response names
  `content-range, accept-ranges, x-snapback-asset-home` in one
  `Access-Control-Expose-Headers`, and its CORS layer adds a second
  (`X-Snapback-Time, X-Snapback-Clock`). The Fetch standard combines them, and
  slice 1 depends on that: a browser that kept only the second would hide
  `accept-ranges`, and D5 then writes nothing. `Retry-After` is not exposed,
  so on the web an upload's `E_RATE_LIMIT` carries no `retryAfter`.

**The queue.** One media queue per partition runs every step that reads or
changes the manifest, with the file changes that step asks for:
- the kept check (`asset`, `stat`, `asset_lost`);
- a publication (`asset_reply`, its removals, the write);
- `media_due`, `media_open` and its sweep, `media_release`, `media_clear`.

A round never touches the manifest itself. At its end, still in the round
queue, it puts `media_due` on the media queue and waits for it. No media step
calls `sync()`, and `asset()` and `upload()` take the media queue only after
the `sync()` that opens a never-synced partition has returned, so neither
queue waits on the other in a cycle. So a write never races a removal, and a
path the core answered exists when it is returned. Downloads and uploads
themselves run outside both queues, so one slow transfer does not hold up the
others.

**Exchanges.** Each media exchange is named, as the round's are
(`<incarnation>.asset<n>`, `.upload<n>`). A reply naming an exchange the core
did not hand out is refused (`E_STALE`), and so is one it already answered or
one `media_clear` let go. Each exchange records the partition's fence count
when it is handed out (D3, "Authority"). Downloads and uploads run beside
rounds. Several may be in flight at once.

**Time.** The driver never reads a clock (LLP 1027.000), and never reuses one
call's `now` for another. Every `asset(id, now)` checks its own entry at its
own `now`, so an expired file is never handed out. `sync(now)` and
`Snapback.open({now})` check every entry's expiry at that `now`. A round or an
open without `now` checks everything else: homes, the cap, the policy, the
fence, and (at open) stray files. An app that wants expired files gone
without showing an asset passes `now` to the `sync()` its `task` runs.

**The manifest.** It lives in the partition's metadata as `exact:media`:
`[{id, path, type, home, bytes, used}]`.
- `home` is what `asset_home` confirmed: `{store_id, table, row, column,
  generation, expiresAt, tables}`.
- `used` is the `now` of the read that showed the asset, because the core has
  no clock (LLP 1027.000).
- **The entry is written before the bytes,** as in Snapback's `MediaCache`.
  A file that a crash left missing or short is found at the next read
  (`stat`). A file the manifest does not name is removed at the next open.
- **Absent** means the partition has never shown an asset. The first
  `asset` writes `[]` before any file, so an app that never shows one pays
  no sweep at open.

### D3 — Retention follows Snapback's rules; Exact keeps what was shown

**Default (ruling Q3):** `keep: "viewed"` with `maxBytes: 64 MiB`. Snapback's own
default is `keep: "none"`. An offline-first app that forgets every photo looks
broken offline, so Exact's driver defaults the other way. An app can set
`keep: "none"`, or another cap, when it opens the client. Within a page, the
partition's first opener sets the policy, and a later open with another
policy is refused (`E_PARTITION_ASSETS`). Across launches the policy is the
opener's, and the open applies it to what the manifest holds:
`keep: "none"` purges everything, as Snapback's does on open, and a smaller
cap trims the least recently used.

Under `viewed`:

- **What is kept.** All of these must hold:
  - the response is the whole object (`200`, or a `206` covering every byte);
  - it says `accept-ranges: bytes`;
  - its size is within the cap, with room among unpinned entries;
  - the device confirms the home (`asset_home`): the row is synced here, the
    viewer may read it, and it holds this asset in a non-`once` column.

  A whole ordinary asset that fails the cap or the home is transient. One
  that is not whole is refused, retryable (D2), and one that does not say
  `bytes` is not written at all (D5).
- **When it is checked.**
  - After every round, on the media queue, which the round waits for (D2).
  - At open, before `open` returns.
  - At each `asset` call, for the one entry asked about.

  Every entry is checked after a round. Snapback skips entries whose home's
  `tables` the commit did not touch. The client has no per-round footprint
  yet, and an entry costs one device call.
- **When it is removed.**
  - Its home no longer matches, or its generation changed.
  - It expired, by a `now` the app passed (D2, "Time").
  - It is the least recently used unpinned entry and the total is over the
    cap. The cap is enforced at each publication, against every other
    entry, as well as after rounds and at open.
  - The app calls `clearMedia()`.
  - The partition's media are fenced (below).
- **Online, a kept file is shown without a download.** This differs from
  Snapback, whose `client.asset` downloads again on every online fetch and
  uses kept bytes only offline. Here the device's rows are the authority, as
  they are for every read. `asset_home` is asked at each `asset` call, so an
  asset whose row was deleted, cleared, left the sync horizon or became
  unreadable stops being handed out at the first round that learns it.
  Until then it shows, exactly as that row's text does. The cost of the other
  choice is a download per avatar per screen. The farm's hand-written caches
  existed to avoid that (museum-miles-0554:70-71).
- **Authority.** `asset_home` checks the row, not the session. A round that
  ends refused with `E_AUTH`, or with `E_STORE_RESTORE`, **fences** the
  partition's media:
  - every kept file is removed at once (the round's `media_due` names them
    all and empties the manifest), and so is every transient file;
  - `asset` answers the refusal and fetches nothing;
  - a download handed out before the fence publishes nothing when its reply
    comes, kept or transient, whatever its status;
  - an upload records no facts (D4.5).

  The fence is kept in the partition (`exact:media-fence`), so a reopen keeps
  it, and no other refusal (a conflict, a rate limit, a refused write) lifts
  it. Only a round that succeeds does. A revocation no round has observed
  waits for one, as the row does.
- **What "stops showing" means.** The next `asset` answer for it no longer
  names the file, and the file is removed. A view already showing it keeps
  its pixels until its resource answers again, as a browser's `<img>` does.
- **A returned path.** A kept answer is pinned: the cap does not evict it
  until the app releases it (`release(path)`, once per answer), calls
  `clearMedia()`, or the partition closes. So two photos in one answer never
  evict each other before either loads. A photo that needs room only pinned
  entries hold is written as a transient file instead, so the kept total
  never exceeds the cap. A pin never holds a file against authority: a
  changed home, an expiry, the fence and `clearMedia()` remove a pinned file
  as they remove any other. Pins live in memory. An app that never releases
  keeps its pinned files for the page; photos past the cap are then
  transient, and go at close.
- **Offline.** A kept file is shown. An asset that is not kept answers
  `E_OFFLINE` with `retryable: true`, as Snapback's does.
- **Evicted by the browser (ruling Q5).** A browser may clear the page's file
  store under storage pressure. A manifest entry whose file is missing or
  short counts as not kept: the entry is dropped (`asset_lost`), and the asset
  is fetched again online or answers `E_OFFLINE`. Exact does not call
  `navigator.storage.persist()`.

**Transient files.** An ordinary asset the cache does not keep is written to
its own file, `app:/tmp/snapback4/<partition>/<id>-<n>.<ext>`, one per answer,
so one consumer's `release(path)` never removes another's. This covers:
- a home the device cannot confirm, such as a row from a server read;
- an asset over the cap, or one that needs room only pinned entries hold;
- everything under `keep: "none"`.

The file is removed when the app releases it, at `clearMedia()`, at a fence,
when the partition closes, and when it next opens. It never counts as kept:
offline, a fresh `asset` call for it answers `E_OFFLINE`, though a path the
app already holds keeps showing until it is removed. A transient file is not
a one-view photo, which never reaches the disk (D5).

**Removals that fail.** Only a file already gone counts as removed. A
removal that fails otherwise (a permission, the disk) is kept and tried
again at every media step. `clearMedia()` rejects with `E_MEDIA_CLEANUP`
while any remain, so a sign-out never looks complete with a photo still on
the device, and a round says so in `Round.media`. A transient file stays
tracked until it is gone. An open sweep that failed runs again after the
next round. The fence's own write is tried again the same way, and the
web's rebuild of a device (after a failed save) carries the fence with the
outcomes it carries; the fence holds in memory meanwhile.

**Removed files on the web.** `appURL` (`host/web/picker-glue.js`) keeps the
object URL it made for each `app:/` path an `image` or `video` shows.
Removing a file through the app's `storage.fs` (`rm`, or the source of a
`rename`) revokes the URL made for that path and for every path under it,
at once, so a released, cleared or swept file's bytes do not stay reachable
for the page's life. The removal is also announced on the store's channel
(`BroadcastChannel`, `exact-files:<store>`), so a removal in a worker or
another tab revokes this page's URL too. A path removed while `appURL` was
reading it, or under a folder removed then, gets no URL. `appURL` also
revokes the URL of a path it finds gone.
That is slice 1's only host change. Native hosts read the file when the
source answers and hold no URL.

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
5. **A changed session, or a closed client.** The facts are recorded only
   while the device is open, under the viewer the partition opened with,
   never one the caller names, and only if no fence (D3) was observed since
   the upload began. The driver checks twice, as Snapback's `local.js` does:
   - **Before the facts are kept.** If the session changed while the bytes
     were in flight (`changed`), the core records nothing.
   - **After they are kept, before the upload answers.** If the session
     changed meanwhile, or the client was closed, or `clearMedia()` ran, or
     a fence was observed (`media_fence` against the count `upload_reply`
     answered with), the driver forgets the facts (`upload_forget`)
     and answers `E_AUTH` ("start the upload again") or, for a close or a
     clear, `E_CLOSED`. If forgetting fails, the answer says the facts may
     remain on this device.

   "Changed" means a session refresh of the partition started, ran or is
   running, or `headers()` gives other credentials than it sent. So the asset
   is never recorded against the wrong session. The upload's answer is the
   promise that this check settles, in the same step, so nothing comes
   between the check and the answer. `close()` and `clearMedia()` wait for
   uploads in the second step to settle before they finish. An
   upload still on the network is let go instead: `media_clear` makes its
   reply stale, and a closed client never delivers it, so nothing is
   recorded.

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

**Until slice 3,** the client shows no `once` asset and writes no byte of
one:
- **`416 E_ASSET_RANGE`.** Refused as `E_ASSET_ONCE_UNSUPPORTED`, naming this
  slice. The view was not spent: the server refuses a range before it counts
  a view.
- **A `2xx` with `accept-ranges: none`** (a server or proxy that dropped the
  range). The same refusal, but the server delivered the bytes, so the view
  **was** spent, and the refusal says so. Nothing is written.
- **Any other `2xx`** is written only when it says `accept-ranges: bytes`.
  One that says neither (a header lost on the way, or not exposed on the
  web) is refused `E_HTTP_RESPONSE`, retryable, and nothing is written: a
  photo whose kind the client cannot tell may be a one-view one.

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

**Built early: `exactBodyFrom` (2026-10-08).** Built ahead of slice 2 for the
Bluesky clone's uploads (a 2 MB photo ran its source's step over the 100 ms
budget through `readFile` and a byte body), approved by Charlie via the lead.
Branch `bsky/body-from`. The surface is this section's: `fetch(url,
{exactBodyFrom: "app:/…"})`, `Request::body_from` for a Rust source, at most 64
MiB, under `fs.read`. The native executor core (Apple, Linux, render) and the
web (the wasm host's page, the JS target) read the file when they send; a
Rust module across the logic seam (LLP 1029.000) refuses it, as it refuses a
timeout. `exactSaveTo` is not built and stays in slice 2, with the rest of
this RFC.
Code review (GPT-6 Astra and Grok 4.7, `llp/reviews/code-2026-10-08-body-from.*.md`),
folded: a native TypeScript app's compatibility id now carries
`typescriptRuntime` (1), so a bundle using `exactBodyFrom` never reaches a
shell whose older prelude would send an empty body; the executor opens and
pins the app's directory handles when the host names them; the deadline and
an abort cover the file read on every carrier; Windows refuses until it has a
capped read; the path (at most 4096 bytes) is charged at admission; a
WebSocket refuses a file body; the web checks the size before making a Blob.
Round 2 (`code-2026-10-08-body-from-r2.*.md`), folded: the bundle's
receipt requires `typescriptRuntime` and the classifier refuses a cohort
whose prelude is older (an id change alone was not enough: delivery lets
ids differ); both web carriers check the deadline by the clock and an
already-aborted signal before sending; the JS target's stream takes a
`Request`'s URL and headers; the path cap counts UTF-8 bytes. Round 3
(`code-2026-10-08-body-from-r3.*.md`), folded: the native read runs on a
thread of its own while the worker watches the deadline and the abort, so a
stalled read no longer holds the request (its late bytes are dropped); one
deadline instant per request on both web carriers, checked by the clock just
before sending; a JS-target stream honours the caller's signal; the
prelude counts a lone surrogate as `TextEncoder` does.
After landing (`code-2026-10-08-body-from-r4.astra.md`): at most four native
file readers run per process, each counted until its thread ends, so readers
stuck on a stalled disk refuse new file bodies instead of piling up; a reader
told its request ended stops at its next 1 MiB chunk (Ibex patch 10 adds
`AppDirectories::open_file`); the JS target refuses a pre-aborted fetch before
making a rejection nobody handles.

**Slices 2 and 3's design** (`exactSaveTo`; `Blob`, `Response.blob()`,
object URLs, `blob:` sources) is LLP 1108.001. It amends D2's reply sum (a
headers arm without `size` for a reply not written) and D5's URL form and
lifetime (the browser's own URL on the web; natively until the engine ends),
and D5's and D8's 256 MiB bound (Hermes only; the web's is the browser's).

### D7 — Sessions

Every asset exchange carries the bearer that the driver's `headers()` gives
at that moment.
- **One session per partition.** Every client of a partition presents the
  same viewer's session through `headers()`. `refreshSession(now, adopt)`
  takes the function that installs the new session where `headers()` reads
  it. The partition runs one refresh at a time, and awaits `adopt(session)`
  **before** the refresh settles, so every exchange waiting for the refresh
  (a download, an upload, another caller's `refreshSession`) resumes with
  the new bearer already in `headers()`. A caller that shares a refresh in
  flight shares its adoption. Without `adopt`, a waiter may still read the
  retired token; the next bullet covers downloads, and an upload then ends
  `E_AUTH`.
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
  and an `image <=1MB once` column, in `tests/media.rs` (the core through
  the native module) and `ts/media.test.ts` (the TS driver and the wasm
  over a file store). Slice 1 covers:
  - upload, write, sync, download, retention across a reopen;
  - a home row deleted on another device, so its file is removed after the
    next round;
  - expiry by `now` (the core, over a manifest entry whose home expires);
  - the LRU cap, and the cap enforced at a publication with no round between;
  - two different photos over the cap in one answer: both files stay until
    released, the second transient;
  - offline shows the kept file;
  - a missing file (browser eviction, or an entry whose write a crash cut
    off) is fetched again;
  - a server-read home is transient;
  - a reply naming no exchange is refused, and an id that is not one ASCII
    alphanumeric segment;
  - `once`: a `416` answers `E_ASSET_ONCE_UNSUPPORTED` and a second plain
    `GET` still succeeds (its view was not spent); a `200` with
    `accept-ranges: none` answers it with the view spent; neither, nor a
    `2xx` without `accept-ranges`, writes a byte;
  - two sources showing the same asset at once share one whole kept file;
  - `clearMedia()` while a download is in flight: the reply is stale and
    writes nothing;
  - the fence: a round refused with `E_AUTH` removes every kept and
    transient file; a download held across that round publishes nothing,
    nor does one handed out before it whose reply comes after a round lifted
    it; an upload records nothing; the fence holds across a reopen until a
    round succeeds;
  - at open, with no `asset` call: `keep: "none"` purges, and a stray file
    (a file the manifest does not name) is swept;
  - a session that changes while the upload's bytes are in flight records
    nothing; a close after the facts are kept forgets them before the upload
    answers;
  - a refresh with `adopt`: a download waiting on it, from another client of
    the partition, presents the new token;
  - a fence raised while a publication is under way (between the core's
    answer and the write): the file goes and the answer is the refusal;
  - a fence raised after an upload's facts are kept, with `headers()`
    unchanged: the facts are forgotten and the upload answers `E_AUTH`;
  - a removal that fails: `clearMedia()` rejects, a round says so, and the
    next step removes the file once it can;
  - a fence whose write fails holds, and is kept once a write lands; a
    rebuilt device takes it over (`held`/`hold`);
  - natively, a round whose answer ended before its reply: the next round
    cancels it instead of answering `busy` (`ts/snapback.test.ts`);
  - each transient answer is its own file, released alone;
  - a second client of the partition with another policy is refused.

  The web host's tests: a file removed loses its object URL at once, a
  removed directory takes its files' URLs with it, a removal announced from
  another realm revokes the page's URL, and a path removed while its URL was
  being made gets none (`host/web/tests/picker-app-url.test.mjs`); in Chrome,
  a file removed through the app's `storage.fs` stops being served by the
  object URL an `image` showed (`host/web/request-refusal.test.mjs`).

  Slice 3 adds a `once` asset shown from memory, never written, whose second
  request is `E_ASSET_VIEWED`.
- **Drives.** The agent picks a file (`tap <input> drop <png>` on the web,
  `type @<input id> <path>` elsewhere, LLP 1069.002). It saves the photo and
  checks the `image`:
  - shown, then still shown after a reload, and offline after a reload
    (`fail fetch` on the origin);
  - shown to a second persona;
  - gone from the files after sign-out (`clearMedia()`);
  - on the web, macOS and an iOS simulator (iPhone Air) in slice 1, done
    2026-10-08 against `snapback4 dev` 0.4.13: pick → upload → shown →
    offline reload still shown → a second persona sees it (on the web in the
    same page; on iOS as Bob after Alice saved hers on the web and macOS) →
    sign-out, then an offline reload shows initials. The native path runs the
    same driver over Hermes and the native module;
  - in Interview, and on a physical iPhone through Interview's LAN mode
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

1. **The client, on today's runtime** (lane D, 2026-10-08, first by
   Charlie's ruling, §0):
   - **Core.** `media.rs` holds the asset, upload and retention calls of D2,
     with their exchanges and the manifest.
   - **TS driver.** `asset`, `upload`, `release` and `clearMedia`; `now` on
     `sync` and `open` (D2, "Time"); `adopt` on `refreshSession` (D7). Over the
     runtime as it is: `fetch` with an `ArrayBuffer` body,
     `Response.arrayBuffer()`, and `storage.fs` (`readFile`,
     `atomicWriteFile`, `stat`, `readdir`, `rm`, `mkdir`). It runs on every
     executor a TypeScript source has: the web's realm over the wasm device,
     and Hermes over the native module.
   - **Grants.** The app grants `fs.read app:/tmp/picked`, plus `fs.read` and
     `fs.write` for `app:/cache/snapback4` and `app:/tmp/snapback4`.
   - **`once`.** Refused, by D5's interim rule.
   - **The web host.** Removing a file through `storage.fs` revokes its
     `appURL` object URL (D3, "Removed files on the web").
   - **Tests and docs.** D8's slice-1 client tests, the README's asset
     section, and the drive.
   - **Not in this slice:** the Rust driver. A Rust source's driver
     (`host_request`/`host_reply`) moves bytes only from slice 2, which gives
     it `save_to` and `body_from`. Until then a Rust source has no asset
     path.
2. **The runtime's files** (Claude (Opus 5.5), the slices 2–3 lane, concurrently with slice 1): R2 on Hermes, the JS target and the
   native HTTP executors, and Rust `save_to` and `body_from`. The TS driver
   switches to them, and the Rust driver is added. D8's R2 tests.
3. **Memory** (Claude (Opus 5.5), the slices 2–3 lane, after slice 2): D5 (`Blob`, `Response.blob()`, object URLs,
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

- **Slice 1:** about 600 lines in the core, 350 in the TS driver, and 900
  of tests. No runtime change; about 20 lines in the web host (`appURL` and
  `storage.fs` removal).
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
- **For Charlie: the order of slices 2 and 3.** Answered 2026-10-08: slice
  1 first ("after"), then slices 2 and 3 ("let's do all of these"), the
  latter in a concurrent lane (§0).
