# Code review: exactBodyFrom (59f55d1d0), grok

Reviewer: Grok 4.7 (xhigh, read-only), 2026-10-08, on the detached review worktree at 59f55d1d0.

I'll review the HEAD commit read-only: the `exactBodyFrom` change, the LLP notes, and how each host actually sends the file.The diff is in. Next I’ll trace grant checks, path handling, and every place a `body_from` can be dropped before the request is sent.The review is still open. I’ll confirm the line-level evidence for the compat-id gap, the Windows read, and the reservation accounting before ranking what blocks landing.## Findings

### 1. P1 — An older host of the same compatibility id silently POSTs an empty body

`bake/src/compat.rs:319`, `js/src/prelude.js:794`, `js/web/src/lib.rs:360`

The compatibility id is the digest of the object built at `bake/src/compat.rs:319`. That object hashes the kernel schema, plan format, ABI numbers, executor names, data crate, and the host capabilities a plan may select. It does not hash `js/src/prelude.js` or any fetch-feature bit. `RUST_ABI` stays 3. LLP 1030 D3a says a host change a bundle can observe has to move that id; a faster painter must not.

`exactBodyFrom` is read in exactly one place in the guest, `js/src/prelude.js:794`. That prelude is compiled into the host (`js/src/lib.rs` `include_bytes!` of `prelude.hbc`), on Hermes and in the web wasm guest. An older binary’s prelude never looks at the option. `JSON.stringify` then records a POST whose `body` is `""` and whose JSON has no `body_from`. The new parser (`js/src/wire.rs:57`, `js/web/src/lib.rs:361`) is not what runs. The request is sent, and a server can answer 200.

Failure: a shell installed from the parent of this commit is still in this cohort, because nothing in the digest changed. An app update does `fetch(url, {method: "POST", headers: {"content-type": "image/jpeg"}, exactBodyFrom: "app:/tmp/photo.jpg"})`. The old prelude uploads zero bytes and the photo never leaves the device. `exactTimeout` on that same old prelude fails open to “no deadline” with the real body still attached. This one fails open to the wrong body. The logic-module seam does the safe thing (`logic/abi/src/lib.rs:212`): it answers `Unavailable("exactBodyFrom cannot cross the Rust module seam")` on the existing error tag and never encodes the field.

The JS target is not this hole. `host/web-js/ts-fetch.js` is injected by the bundler, so a new bundle carries `fromFile` with it.

Fix: add a compat input this binary has and every older binary lacks, for example a fetch capability inside that inputs object, and cover it with a bake test that the parent commit’s id differs. Already-shipped preludes cannot be taught to reject an unknown `exact*` option.

Skipping the other bumps is right. The logic ABI must stay 3: the refusal is the existing tag-4 `Unavailable`, and `read_result` forces `body_from: None`. The turn envelope (`data/src/envelope.rs:11`, header `exact-turn: 1`) gains a JSON field, but `encode` / `apply` run in-process (`data/src/placed.rs:265`, `data/src/placed.rs:327`, `data/src/mixed.rs:385`, `data/src/mixed.rs:475`), so there is no second binary to skew.

### 2. P2 — The non-Unix read ignores the 64 MiB cap

`host/apple/src/executor_body.rs:77`, `vendor/ibex/crates/ibex2/src/stdlib/windows_directory.rs:272`

The comment at `executor_body.rs:77` says the read is capped, so a file that grew after `stat` is refused without being read whole. That is true only under `#[cfg(unix)]`, which calls `AppDirectories::read_capped(..., MAX_BODY_FROM_BYTES + 1)` (`executor_body.rs:91`). `#[cfg(not(unix))]` (`executor_body.rs:95`) calls `FsOp::ReadFile`, and Windows `Directory::read` is `read_to_end` with no limit (`windows_directory.rs:272`). The size check at `executor_body.rs:114` runs after that allocation. `EXACT-PATCHES.md:171` already says `read_capped` is Unix-only. This executor is the one Linux builds for Windows (`host/linux/src/executor.rs` includes `executor_core.rs`, which includes `executor_body.rs`), and the Windows host runs that crate.

Failure: `stat` sees a small file under `app:/tmp`. Another writer appends past 64 MiB before `ReadFile`. The worker, or one of 16 stream threads (`executor_stream.rs:68` uses the same `resolve`), allocates the whole file and only then returns `too-large`. Nothing is sent, but the 64 MiB bound is already broken. Unix `take(cap)` does not have this window. Symlinks are not the issue: `validate_name` rejects `\` and `..`, and reparse points are not followed.

Fix: read at most `MAX_BODY_FROM_BYTES + 1` off the opened handle on Windows, or refuse `exactBodyFrom` on non-Unix until that read exists. Add a test that a file which grows past the cap after `stat` does not allocate the tail.

### 3. P2 — `body_from` is omitted from the admission charge

`host/apple/src/executor_core.rs:677`, `host/apple/src/executor_core.rs:268`

`reservation` sums capacity so a caller cannot hide an oversized retained buffer (`executor_core.rs:677`). The new `body_from: Option<String>` is not in `sizes`. The 4 MiB `MAX_REQUEST` check (`executor_core.rs:697`) and the ordered queue’s 64 MiB waiting budget (`executor_core.rs:268`, which sums `job.charge`) therefore ignore it. The file bytes themselves are intentionally outside this budget: the `Core` comment at `executor_core.rs:96` says native-work allocations are bounded by worker and stream counts. The path string is a retained request buffer, and it sits in the queue until a worker runs.

Failure: the prelude checks the `app:/` prefix and rejects `.` / `..` (`js/src/prelude.js:796`) and does not cap length. A source enqueues `exactBodyFrom: "app:/" + "a".repeat(8 << 20)`. Admission succeeds. Up to 128 ordered waits (`ORDERED_READS`) can hold those strings on top of the 64 MiB the queue thinks it charged.

Fix: add `request.body_from.as_ref().map_or(0, String::capacity)` to `sizes`, and reject an overlong path in the prelude and in `body_from_refusal` before enqueue.

### 4. P3 — Web byte entries are copied, then rejected

`host/web/storage-fs.js:408`, `host/web/storage-fs.js:221`

`requestBody` calls `store.blob(normalized, '')` and only then checks `blob.size` against 64 MiB (`storage-fs.js:418`). `blob`’s `maxBytes` defaults to `Infinity` (`storage-fs.js:221`), and its oversize text is hardcoded `compressImage:`. For a byte record, `run` has already structured-cloned the IndexedDB value, and `new Blob([contents])` copies it again, before the reject. A picked `File` is rejected on `.size` and sliced with an empty type, which is what the tests cover. `writeFile` in this store has no 64 MiB cap. Native `stat`s first and never reads past the cap.

Failure: `exactBodyFrom` of a 200 MB byte entry on the wasm host or the JS target allocates the record plus a second `Blob`, then rejects. The upload is not sent.

Fix: pass `MAX_BODY_FROM_BYTES` into `store.blob` and give that path an `exactBodyFrom` message. The IndexedDB clone of a byte record remains; that is the same constraint `compressImage` documents just above.

### 5. P3 — Diagnostics and the JS-target `Request` path disagree

`host/web-js/ts-fetch.js:14`, `host/web-js/ts-stream.js:23`, `runner/src/request.rs:333`, `docs/reference.md:381`

`bodyFromRefusal` takes the method from a `Request`, then treats only `init.body` as “a second body” (`ts-fetch.js:19`). `fetch(new Request(url, {method: "POST", body: secret}), {exactBodyFrom})` therefore does not throw. `fromFile` passes the file as `init.body` and the secret is dropped. The same call with `exactStream` is worse: `ts-stream.js:23` sets the method from `init.method ?? "GET"`, so `http-body.js:180` refuses it as a GET with a body and the upload never starts. Hermes has no `Request`; the JS target runs in a browser and this function already special-cases `Request` for the method.

The strings for one condition differ by caller. The prelude says `exactBodyFrom must be an app:/ path` and `fetch: a request has one body: body or exactBodyFrom` (`js/src/prelude.js:797`, `js/src/prelude.js:803`). `Request::body_from_refusal` says `needs` and uses a comma (`runner/src/request.rs:333`, `runner/src/request.rs:335`). A `.` or `..` segment is a prelude `TypeError`, a native `filesystem: app path contains traversal or NUL` (`vendor/ibex/crates/ibex2/src/stdlib/app_fs_unix.rs:199`), and on the wasm page a grant denial, because `coversPath` returns false when `grantPathParts` rejects the segment (`host/web/navigation.js:1382`, `host/web/http-body.js:182`). A directory is `not a file` natively (`executor_body.rs:84`) and `operation needs a regular file` on the web (`storage-fs.js:116`). All of these refuse before send.

`docs/reference.md:381` says a missing file, a denied path, or a file over 64 MiB is `FetchError` kind `Refused`. A host with no roots returns kind `Unsupported` and `this host has no app files` (`executor_body.rs:66`). The render host sets no roots unless the caller does.

Fix: in `bodyFromRefusal`, reject when `input instanceof Request && input.body != null`, and in `ts-stream.js` `request` use that same method. Point `reference.md` at the `Unsupported` no-roots case. Unifying the `must` / `needs` strings is worth doing only where a test or a caller matches them.

## What held up

Unix path checks match `readFile`. `parse` rejects `.`, `..`, and NUL before admission (`app_fs_unix.rs:183`). `openat` uses `O_NOFOLLOW` (`app_fs_unix.rs:30`). A symlink swap between `stat` and `read_capped` fails the open or stops at `MAX+1`. Roots come from `picker::app_dirs(app_id)` on Apple (`host/apple/src/abi.rs`) and Linux (`host/linux/src/host.rs`); `app_roots()` turns an error into no roots, and `resolve` then refuses. The render host never sets them. Scope narrowing goes through `exact_data::storage::scope` (`data/src/storage.rs:28`) on the worker and on streams, and through `scopedGrantSet` on the web (`host/web/http-body.js:149`). An empty scope has no `fs.read`. `fs.read app:/data` does not cover `app:/database` or `app:/tmp`. Grant text is checked again inside `read_capped` and `requestBody`. No `Content-Type` is added for a byte body (`prepare_body` adds one only for form data; a picked `File` is sliced with an empty type). A zero-byte file sends no bytes, same as an empty byte body. WebSocket streams skip `resolve` the way they already skip any body (`executor_stream.rs:66`). `exactSaveTo` is still documented as unbuilt.

## Landing

**Yes, finding 1 blocks landing.** An app update that uses `exactBodyFrom` can be delivered to an already-installed shell and upload nothing. Findings 2–5 should be fixed, and 2 before any Windows host runs this executor; they do not block Apple, Linux, or the web wasm/JS targets.
