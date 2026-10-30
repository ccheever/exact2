# Vendored Ibex split crates — Exact integration ledger

## Windows WebSocket readiness

`crates/ibex2/src/transport/websocket.rs` subscribes to writable readiness only
while output is pending. Windows rearms readiness after a blocked read; keeping
writable interest on an empty output queue makes an outstanding receive spin.
The idle-receive regression in `websocket_tests.rs` checks that the pump stays
parked and still wakes on cancellation.

## Mac widget Keychain (LLP 1126.001 WP16)

`crates/ibex2/src/secrets/darwin_keychain.mm` selects the restricted
`ExactKeychainAccessGroup` in the data-protection Keychain for a provisioned
Apple widget cohort. Ordinary Mac apps keep the login Keychain. The containing
app migrates its service's login items on its first provisioned launch;
copy failure keeps the old item, and an existing shared item wins. Migration
runs once per process, from `prepare` only. An unreadable item is skipped;
errors are logged without withholding the app's I/O. Retained items can be
retried on the next launch. The move is one-way. Extensions never migrate.
The Objective-C fixture in
`host/apple/tests/fixtures/keychain-migration.mm` exercises retries and failures
with substituted Security calls; signed Keychain access still needs profiles.

## Snapshot and update recipe

- Upstream: `https://github.com/expo/ibex.git` at
  `bbf47f380c138d1138581c299f298e3b1bd9cb9c` (2026-10-18).
- Source trees: `crates/{ibex2,ibex2-sqlite,hermes-lean-sys,hermes-lean-sys-installer}`.
- Data inputs: `scripts/icu74-filter-{root-en,en-intl}.json` and
  `third_party/wpt/urltestdata.json`.
- Hermes bundle is `hermes-vanilla-d412d3bd8512-v5`, source
  `d412d3bd…` / Hermes `260318099.0.4`. Repository engine-output directories
  remain absent; builds validate the paired compiler and VM in the bundle cache.
- The two standalone `Cargo.lock` files under `ibex2` and
  `hermes-lean-sys-installer` are consumer integration files. The installer
  shares pinned dependency versions with Exact's root lock.

The October 13 refresh compared the old `7f77c82a12f20a4cd3ebbe01fdaa7f22deb7c985`
upstream tree, Exact `3ad9ef9cb396fc1cc786abcec03cc1b6bc21cec5`'s vendor tree,
and the new upstream tree before replacing any source. The old local delta
included 40 changed/added files. Reusable changes now come from upstream;
Exact's explicit stub configuration and standalone locks are replayed below.

The follow-up refresh from `b20dded` to `bf74e54` changed only
`fs_image.rs` rustdoc within the vendored inputs: opaque native codec workspace
is a measured admission estimate, not an allocator ceiling. Both stub seams,
the three standalone workspace boundaries, both locks, and Exact's app-facing
storage declaration were unchanged. The subsequent `e60b3e0` refresh adopts
the authenticated v5 trust table: eleven target mappings for ten assets,
including Android aarch64. Its manifests, locks and integration seams are
unchanged. The published bundle was consumed by the qualification below.
The `e742c72` refresh adopts expo/ibex#1: off Apple, the portable WebSocket
transport tunnels through the environment's proxy as fetch does
(`transport/websocket.rs` and its tests, the only vendored inputs that changed
since `e60b3e0`). Manifests, locks and integration seams are unchanged.

The October 18 refresh compared that `e742c72` upstream tree, Exact
`a28fc4f18fda0da187f0a8e564f8407c57748dd8`'s vendor tree, and the new
`bbf47f3` upstream tree before replacement. Upstream now supplies the socket
queue count, scheme-wide grants, allocation-reduced descriptor reads and the
collection between the hardening guard and `harden.js`; their active local
patches are retired below. The refresh also adopts fail-closed portable fetch
for an unsupported environment SOCKS proxy, including a `NO_PROXY` bypass and
one proxy snapshot per transport. The eight changed upstream inputs are
`bindings/install.cc`, `grant.rs`, `stdlib/websocket.rs`, the two Darwin
WebSocket files, `rustls_http.rs`, `transport/websocket.rs`, and its tests.
The v5 engine trust table, manifests, data inputs and standalone locks did not
change. After replacement, the complete Exact delta apart from this ledger is
ten files: the two locks, three workspace boundaries, two stub build seams,
the app-facing storage declaration, Keychain group selection and read-only
directory snapshots.

For another refresh, export the four named crate trees and all three data inputs
from the selected commit into a temporary directory. Recover the current local
delta against the pin above before replacing them; classify each difference
against both snapshots. Replay the integration seams, regenerate the standalone
locks, verify installer/root overlap, and run both upstream and Exact consumers.
Do not copy local engine outputs or silently replace the bundle pin.

```sh
cargo generate-lockfile --manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml --offline
cargo metadata --manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml --locked --offline --format-version 1 --no-deps
```

After regeneration, align every installer's dependency also present in Exact's
root lock with that root version using `cargo update -p NAME --precise VERSION`.
The lock overlap check is:

```sh
bun -e 'const fs=require("node:fs"),read=p=>Bun.TOML.parse(fs.readFileSync(p,"utf8")).package,root=new Map; for(const p of read("Cargo.lock")){const v=root.get(p.name)||[];v.push(p.version);root.set(p.name,v)} const bad=read("vendor/ibex/crates/hermes-lean-sys-installer/Cargo.lock").filter(p=>root.has(p.name)&&!root.get(p.name).includes(p.version));if(bad.length)throw new Error(`installer/root lock mismatch: ${bad.map(p=>`${p.name}@${p.version}`).join(", ")}`)'
```

## Reusable patch adoption

Numbers preserve the former ledger so a patch never disappears without its
replacement and consumer evidence. Rows 3–7 and 9–13 retire the corresponding
October 13 source patches. The four named rows added on October 18 retire the
last reusable source patches. Platform-specific limits are named rather than
inferred from source replacement. The remaining consumer deltas are the seams
listed below.

| Former Exact patch | Upstream replacement | Exact consumer evidence |
|---|---|---|
| 3. Rustls on macOS | Explicit `portable-http` feature; native Apple default retained | Render tests pass with the feature; native HTTP deadline/drain tests pass in default and async modes |
| 4. Shared `exact-grants`, including `doc:/` | Ibex owns an independent grant grammar; no Exact dependency | Shared fixture passes both Rust grammars and the web matcher (111 browser assertions) |
| 5. Chosen documents | Typed provider and owned `DocumentLease`, with retained directory/file capability and scope guard | Native storage (22 tests), Apple document/session tests and real lean JS tests pass, including replacement and result caps |
| 6. Denied redirect origin | `HostError::DeniedRedirect`, shared hop admission | Runner redirect/refusal cases and actual slow-redirect total-deadline test pass |
| 7. Per-fetch timeout | Request timeout plus asynchronous exchange lifecycle | Actual redirect chain and silent/trickling server cancellation pass in default and async modes |
| 9. Image compression | Admitted `ImageJob`, bounded `ImageService`, owner-scoped work handles and atomic commit arbitration | Native codec, winning/losing cancellation with actual output consumption, and real lean JS (108 tests) pass; private ImageIO workspace qualification below |
| 10. Capped app reads/open handle | Public `AppDirectories::open_file` and `read_bounded` on Unix/Windows | Opened-handle upload, native single-copy and portable double-copy admission/accounting cases pass |
| 11. Host work in flight | Context count plus owned registry/worker/completion accounting | Owned-handle retirement and cancellation witnesses pass; JS continuation semantics below |
| 12. Bounded native filesystem results | Explicit `ResultLimits`; bounded reads/enumeration and batched recursive removal | Exact 16 MiB native/app/document cap tests pass, preserving the typed result-limit error |
| 13. Async fetch | Fallible preparation, infallible accepted activation, terminal retirement, bounded portable workers | Admission refusal, physical retirement, all six base-start failure positions and bounded-worker lifetime tests pass |
| Windows chosen-document EISDIR | Same retained-handle native document executor | Source patch retired against upstream native Windows tests; native Windows Exact storage, document and ordinary TypeScript host requalification is recorded below |
| Socket queued-message count | `MessageSender::buffered_messages`, implemented by the portable sender and the Darwin sender under its lock; the Darwin source test observes a paused native send complete | Exact's native executor checks a batch whole before the transport's 256-message limit; `executor_socket_tests::a_batch_is_refused_whole_past_the_buffer_or_after_a_close` is the direct consumer |
| Scheme-wide network grants | Ibex `GrantSet` owns `http:`/`https:` fetch grants and `ws:`/`wss:` socket grants, with independent family admission tests | `exact-grants` compares Ibex and Exact against `host/web/tests/fixtures/grants.json`; the web matcher consumes that same corpus |
| Presence policy on `secret.keep` | Ibex `GrantSet` reads `secret.keep <name> <policy> [optional] [platform words]` (exact2 LLP 1133.010 D4) and refuses `exact.presence.` names; the grant stays the name's | `exact-grants` compares Ibex and Exact against `host/web/tests/fixtures/grants.json`'s presence cases |
| Allocation-reduced descriptor reads | Ibex captures `hasOwnProperty` and reuses property identifiers while still walking captured own-property descriptors | The linked-lean `exact-js` bootstrap tests refuse Proxy-hidden abort hooks and prove every descriptor-reachable payload is frozen |
| Collection after the hardening guard | Ibex collects only after a trusted bootstrap guard ran, before evaluating `harden.js` | The same linked-lean bootstrap tests exercise the guarded and unguarded paths with production bindings |

Upstream `f28b08d`'s portable-fetch tests run the unsupported SOCKS cases in
child processes, prove refusal before DNS or a direct connection, prove
`NO_PROXY` bypass, and prove a pooled transport retains one supported proxy
snapshot. Exact's render and non-Apple executors consume that portable path.

Patch 9's app interface stays Exact's and is replayed on a refresh:
`bindings/storage.d.ts` declares `compressImage(from, to, {maxDimension,
maxBytes, type})` resolving to `{path, type, size, width, height}`, as Exact's docs
give it, and Exact's prelude (`js/src/prelude.js`) adapts that to upstream's
positional binding (`compressImage(from, to, maxDimension, maxBytes)`, replying
`width\theight\tsize`). Exact adds a fifth positional argument, the media type
(exact2 LLP 1069.002 A1.11, 2026-10-20): `boundary_abi.rs` reads it into
`fs_image::ImageRequest::format` (`ImageFormat::Jpeg` when absent, `Png`; any
other value is an invalid argument), and the codec is Exact's. Replay both on a
refresh. The October 13 refresh replaced the declaration with
the positional form; `contract/cli/tests/it/typescript.rs` now pins it. Upstream
now preserves Exact's app error branches: a source over the read cap is
`compressImage: too-large:`, and a trial deadline before publication is
`compressImage: timeout:`. This retires the local substring-based image error
patch; exact internal markers and typed deadline errors are matched upstream.
The source pin also deallocates an upload's bytes before invoking its last-release
callback, so the executor cannot refund memory while the allocation is retained.

Patch 9a (exact2 LLP 1133.019 D14, D15, D17, 2026-10-24) is Exact's alone and is
replayed on a refresh. `bindings/install.cc` names a second fs method,
`transformImage`, on op 121. Its native arguments are `(from, to, optionsJson)`;
`compressImage` may add a sixth, its edit's JSON. `boundary_abi.rs` admits both
shapes and passes the JSON unread in `fs_image::ImageRequest::options`, with
`transform` set for the three-argument form. `ImageRequest` is `Clone`, not
`Copy`, and has a `Default`. A transform's `max_dimension` and `max_bytes` are
the service's own (`ImageJob::prepare`), and its failures are named
`transformImage:` (`failed_as`, `WorkControl::check`). The codec's
`CompressedImage` gains `report` (a JSON object, passed through) and `extension`.
A transform whose `to` leaf has no `.` is written at `to.<extension>`, after
`fs.write` is admitted for that path, in the parent already pinned
(`AtomicTarget::extended`). `ImageFile` gains `path` and `report`, and a
transform replies `width\theight\tsize\tpath\treport`. `bindings/storage.d.ts`
declares `transformImage`, `TransformImageOptions`, `TransformedImage`,
`ImageCrop`, and `rotate`, `flip` and `crop` on `CompressImageOptions`.
`fs_image.rs`'s `a_transform_passes_its_options_and_extends_an_extensionless_target`
and `contract/cli/tests/it/typescript.rs` pin it.
The standalone manifests, dependency graph and locks are unchanged by this refresh.

### Historical v4 Windows consumer qualification — 2026-10-13

The Windows lane started from Exact `dfa6a26344cf238f4b642805492806795e01ddfd`
and qualified its corrected codec-refusal assertions and native host at
`b76971320c5c43c505b29b0e3195bf66020c27be`, with Ibex source pin
`de11540`. That lane is integrated in Exact `4fd19fb4e2`. It ran on x86_64
Windows with the authenticated
`hermes-vanilla-d412d3bd8512-v4` bundle. Its receipt uses schema
`ibex/hermes-upstream-pinned-receipt/2` and has SHA-256
`38714ce6a52d8511b4a0f03128ec1a3f4b6338d9e0d6572afbb2fd4d29819bb0`.

`cargo test --locked --offline -j 4 -p exact-data-host -p exact-js
--no-fail-fast` passed 25 data-host tests, 2 JS library tests and 101 JS
integration tests; one manual startup diagnostic stayed ignored. This includes
native chosen-document reads and writes, queued document ownership, storage
result bounds, redirect/deadline/cancellation contracts, and the linked-lean
test that loads its bytecode prelude and refuses source text. A host without an
installed image codec refuses after grant admission and before source/provider
reads; Apple retains its ImageIO assertions.

The existing ignored ordinary TypeScript fixture then built through
`bun host/windows/build.mjs` with four Cargo jobs. The native agent launched
the packaged EXE at 640×360 and delivered two platform button presses: its
Hermes-backed resource changed from `ts count: 0` to `1` and `2`, and the
owned process exited. The EXE SHA-256 is
`e40dadb300adcf003e2e28f16951b6bc7f9372480db81e7fa39967d3ebc08516`.

This does not qualify an actual HTTP exchange through `exact-render` on
Windows. Its library still imports Unix-only `UnixStream`, `AsRawFd` and
`libc::poll`, so the focused loopback executor test does not compile there.
`issues/20261013-exact-render-windows-http.md` records that host boundary.

The already upstream pre-split Windows connect readiness, Windows filesystem
backend, and lazy Rustls trust-store changes remain upstream.

## 8. Android bundle pin — adopted and cross-link qualified

Upstream source now supplies the per-target `HERMES_LEAN_SYS_DIR_<target>`
override, Android link recipe and release target. The consumer now pins the
published v5 bundle, including `aarch64-linux-android`. The release's complete
checksum inventory, archive attestations and paired compiler/VM receipts were
authenticated before the trust table changed.

Exact Q `56f107fa44bf65c3df043efd3b784e7dd71484a5` ran
`bun scripts/agent-android.mjs build duo-lab` from the Exact checkout on the
Linux builder. That invoked `cargo build --profile host-dev -p duo-lab-linux
--target aarch64-linux-android` using Rust/Cargo 1.97 and the installed Android
target. Its v5 tag and all eleven pin mappings matched upstream exactly. The
legacy Exact Android cache stayed empty: this build consumed the published
bundle through the normal resolver.

The output `target/aarch64-linux-android/host-dev/duo-lab-linux` is an
AArch64 ELF PIE executable with interpreter `/system/bin/linker64`,
78,809,152 bytes, SHA-256
`dec5e208fe21cb32340f603aa0514409414918f9eafad0e623ba6580cc66d98e`.
It defines 3,276 Hermes symbols, has no unresolved Hermes symbols, and needs
no `libc++_shared`. Its dynamic dependencies are Android's `libandroid`,
`liblog`, `libjnigraphics`, `libdl`, `libm` and `libc`. The build exited zero
and the Q checkout stayed clean. The consumer-release run retains
`exact-android.identity`, `exact-android-binary.txt`, `exact-android-build.log`
and the toolchain/pin comparison under its v5 qualification receipts.

This retires the local pin patch. It qualifies the Android executable link,
not device execution. Existing `scripts/hermes-android.mjs` local receipt flow
remains available.

## Remaining consumer integration seams

`ibex2`, `ibex2-sqlite` and `hermes-lean-sys` declare empty workspaces. This stops
Cargo from discovering an enclosing checkout when `cargo fmt --all` visits these
excluded path dependencies in a nested worktree. Preserve these boundaries when
refreshing the vendored crates.

`ibex2/build.rs` emits empty binding bytecode and hardening metadata only when
`EXACT_JS_ENGINE=stub`. `hermes-lean-sys/build.rs` recognizes that same explicit
configuration and emits empty engine/compiler metadata with no native link.
Normal builds still validate and link the receipted lean VM. These remain
Exact-only; paired `HERMESC_PATH` export is now generic upstream support.

`secrets/darwin_keychain.mm` selects the bundle's
`ExactKeychainAccessGroup` only when `ExactAppIdentifier` equals the Keychain
service. Exact's app and WidgetKit extension can therefore read the app's
granted secrets through their shared group without changing another Ibex
consumer's service. `host/apple/bundle.mjs` publishes the two values, and
`host/apple/widget-keychain.test.mjs` is the opt-in simulator integration
witness. This is Exact application policy and remains local.

On Unix, `AppDirectories::snapshot` pins only roots that already exist and
leaves the others absent for that invocation. A read-only widget refresh can
therefore run first-launch application logic without creating the app's data,
cache or temporary directories. `exact-js` selects it only for a snapshot;
`missing_snapshot_roots_do_not_prevent_activation_or_create_app_directories`
is the direct consumer test. This is LLP 1126 D8 application policy and
remains local.

The standalone locks above remain local. Application policy stays in Exact:
request scheduling and byte budgets, whole-exchange deadlines, source grants,
document session identity, JSON result vocabulary, native image codec, and
storage continuation ownership. Optional HTTP metric-file logging moved from
the vendor tree into `host/apple/src/executor_metrics.rs`; its asynchronous
callback records remain consumer-owned, and blocking/native comparisons use
upstream's explicit diagnostic. Portable workers reserve an additional upload
copy in the lane's byte budget; native Apple upload buffers are charged once.
Each portable worker owns its process slot until its actual thread exits.
Partially started pools cannot retry against still-owned slots; failed base
startup releases every unstarted portable reservation.

Borrowed timer task support does not install ambient timers in Exact. Its
production group selection remains `PURE | CRYPTO | ABORT` (plus the existing
platform Intl selection), and Exact's trusted prelude retains its refusal policy.

The existing Snapback consumer proves partitioned storage, durable operation
queues, reconnect and account retirement. There is no Snapback asset-upload API
in this checkout: the deterministic losing-cancellation output witness is Exact's
native storage adapter. No new Snapback media feature or qualification is claimed.

## Local patch 14, Exact's app interface: document watches and expected writes

`bindings/storage.d.ts` declares `fs.watch(path, {recursive})` and the
`options` argument of `writeFile`, `atomicWriteFile` and `appendFile`
(`{expect: {modifiedMs, size}}`), as exact2 LLP 1069.010.000 D1 and D2a give
them. Declarations only: no Ibex source runs either. Exact's prelude
(`js/src/prelude.js`) records a watch as an LLP 1016.002 topic and does an
expected write as a `stat`, a comparison and the write, queued together; the
web glue and Exact's Rust storage do the same. Replay the declarations on a
refresh.

## Local patch 15, Exact's kept documents: a selected file named as chosen

`stdlib/documents.rs` adds `DocumentLease::selected_file_named(directory,
leaf, name, guard)`: a selected file the guest path names `name` while its
leaf in the retained parent is `leaf`. `selected_file` is that with the two
equal. Exact keeps a chosen file across launches under the name it was
chosen by (exact2 LLP 1069.010.000 D4), and a Mac bookmark follows the file
when it is renamed, so the two differ after a rename. The provider still
decides what the name admits; Ibex still opens only `leaf`, refuses every
path beneath it, and refuses `rm`, `mkdir` and `readdir` on it. `bindings/storage.d.ts` declares
`fs.recentDocuments()` and `fs.forgetDocument(path)`, which Exact's prelude
answers from the host's kept list; no Ibex source runs either. Replay both on
a refresh.

## Local patch 16, bindings' bytecode read where it is linked

`bindings.rs` links each binding's `.hbc` and `HARDEN_BYTECODE` at a 16-byte
boundary (`aligned_bytecode!`), and `bindings/install.cc`'s `CompiledBytes`
lends such a span to Hermes instead of copying it, since Hermes keeps a
module's buffer for the runtime's life and the statics live as long; a span
not so aligned is still copied. `include/ibex2_jsi.h` says so at
`CompiledScript`. In euxy on iOS the copies were 81 KB of malloc at idle.
Replay on a refresh.

## Local patch 17, a host-task worker gives its stack back when idle

`pool.rs`: on Apple platforms a worker that finishes a job with none queued
releases its stack pages below its frame (`MADV_FREE_REUSABLE`). The two
workers kept through a quiet spell otherwise keep every page a job reached
dirty for the process's life: 80 KB in euxy on iOS after its launch's reads.
Replay on a refresh.

## Local patch 18, `copyFile` from a chosen document into an app file

`stdlib/documents.rs` adds `copy_document`, which `fs::run_with_documents`
runs for `copyFile` from a `doc:` path to an `app:` path (exact2 LLP 1069.010
D1, ruled 2026-10-26); every other two-path document operation is still
refused. It admits `fs.read` on the document, then
`AppDirectories::copy_in` admits `fs.write` on the destination, and only then
is the provider asked. The source opens as `readFile` opens it
(`app_fs::document_file`, Unix and Windows) and the host copies its bytes, so
no result envelope bounds them. Replay on a refresh.

## Local patch 19, an app file staged in pieces and published whole

`stdlib/app_fs_unix.rs` and `app_fs_windows.rs` add `AppDirectories::stage(grants,
path)` and `Staged` (`write_all`, `publish`), for `fetch(url, {exactBodyTo})` (exact2
LLP 1108 D6 R2, ruled 2026-10-26): the host writes a reply's body into an app file as it
arrives. `stage` admits `fs.write` on the path, refuses a missing parent or a target that
is not a regular file, and creates `.exact-<hex>.partial` beside the target through the
pinned parent. `publish` syncs it and renames it over the target; a `Staged` dropped
unpublished removes it. The stub for other platforms refuses. Replay on a refresh.

## Local patch 20, the `local-network` grant source

`grant.rs` adds `fetch.local-network` and `websocket.local-network` (exact2 LLP
1133.036 D5, ruled 2026-10-26), written `net.fetch local-network` and
`net.websocket local-network`, with the word `development` for loopback too. It
admits a host the URL writes as an RFC 1918, link-local or unique-local
address (an IPv4-mapped IPv6 address by its IPv4), a link-local IPv6 address,
or a `.local` name; never a name that merely resolves to one. exact2's
`grants` crate holds the same test. Replay on a refresh.

## Local patch 21, a host's check at every fetch hop and WebSocket open

`boundary.rs` adds `HopGate` and `HostError::NotAllowed { reason }` (shown
`NotAllowedError: <reason>`); `host.rs` gives `Fetch` and `WebSocket` an
optional gate (`with_gate`); `stdlib/fetch.rs` (`fetch_stream_gated`,
`admit_hop`), `fetch_async.rs` (`start_gated`) and `websocket.rs`
(`open_gated`) ask it after the grants admit each hop, a redirect's included,
and before anything is sent (exact2 LLP 1133.036 D6): exact2's Apple executor
refuses a tool's, a wake's or a package event's reach to the local network
while the person has not decided. Without a gate nothing changes. Replay on a
refresh.

## Local patch 22, a composed signal's sources

`bindings/abort.js`'s view of a signal (`inspect`, the hooks' `own`) names the
signals `AbortSignal.any` composed it of (`sources`, a copy; null for one not
composed), so exact2's data runtime gives a fetch carrying a composed signal
the earliest `AbortSignal.timeout` among them as its deadline (exact2 LLP 1146
§2.1). Nothing else reads it. Replay on a refresh.

## Local patch 23, an upload's bytes sent

`stdlib/fetch.rs` gives `Request` an optional `sent` hook (`Sent`), told
`(loaded, total)` as the body goes out and `(total, total)` by the time the
headers have arrived (exact2 LLP 1016.000 D10, XMLHttpRequest's upload
progress), and an optional start hook beside it. `fetch_stream_gated` tells
the start hook once, when `admit_hop` has passed the first hop's grant check
and local-network gate, so a request they refuse never started (the
embedder's `exact:sending`). The hook answers whether the upload may go out:
an embedder that has let it go answers `false`, which stops it as an abort
does, and the loop checks the signal again after it. So an embedder that
aborts an upload, then reads which uploads started, is never wrong about one.
`transport/darwin_http.mm` records URLSession's `didSendBodyData` on the
exchange, and `ibex2_darwin_http_headers` takes the bytes the caller last saw,
returning 2 when they change before the headers, so `transport/darwin.rs`
tells the hook on the opening thread and waits again.
`transport/rustls_http.rs` sends such a body through a reader that tells the
hook each read ureq takes, with an explicit `content-length`. Without a hook
nothing changes. `fetch_stream_gated`, every request's hop loop, moves the
body into each hop's request and back instead of cloning it, so a request
holds one copy of its body while it goes out, the platform's aside.

Tests: `darwin_stream_tests.rs`
`an_uploads_bytes_sent_are_told_before_its_headers`, `stream_tests.rs`
`the_portable_transport_tells_an_uploads_bytes_sent` and `stdlib/fetch.rs`
`an_upload_let_go_as_it_starts_never_reaches_transport`,
which run with this crate's manifest and in no lane. exact2's workspace tests
the hooks through its executor: the start, and none for a request the grants
refuse, in `exact-apple`'s `an_upload_the_grants_refuse_never_starts`;
URLSession in `exact-apple`'s async-lane
`an_upload_through_urlsession_tells_its_progress_and_waits_out_a_slow_reply`;
and the portable transport in `exact-js`'s
`an_upload_maps_its_bytes_sent_as_they_go_then_its_reply`. Replay on a refresh.

## Local patch 24, Exact's desktop processes: the transport and file operations

For exact2 LLP 1133.007. `process/` starts a child in a process group of
its own with default signals and only its three pipes (`posix_spawn` on
macOS, `fork` and `execve` with `PR_SET_PDEATHSIG` on Linux), `argv[0]` as
the caller names it, the parent's pipe ends raising no `SIGPIPE` on Apple
platforms, and answers what the host's supervision needs: waiting,
readiness of a pipe, signalling a group and listing its members, a
process's start time and session, the boot's identity, the password
database's home directory and login shell, and a wait at the process's
exit (`wait_at_exit`). It moves the child's descriptors above 2 before
putting them in place, and `fork` runs with every signal blocked until the
child's handlers are defaults. A `PATH` lookup skips relative entries.
`stdlib/fs.rs` resolves `home:/` and `bundle:/` (the namespaces `grant.rs`
reads) against roots the host sets (`set_roots`), the bundle read only,
each path admitted as spelt and run from its root's retained descriptor
with every component opened `O_NOFOLLOW` (`app_fs_unix.rs` `run_below`,
which `app:/` paths share), so no link below a root is followed; a native
absolute path is run the same way, admitted as spelt and as realized, from
the realized directory of the deepest grant line that admits it
(`GrantSet::fs_root`); `access` checks its leaf with `AT_SYMLINK_NOFOLLOW`;
`open_read` opens a path to read it the same way, for a child's stdin; and `stat`
reads `mode` and `isSymbolicLink`; `fs_more.rs` adds `lstat`, `readlink`,
`access`, `chmod`, `symlink`, `digest` and `flock`/`unlock`, host opcodes
122 to 129 in `host_opcodes.rs` and `boundary_abi.rs`, the methods in
`bindings/install.cc` and `storage.d.ts`; a lock moves to the
`RuntimeState` that asked, whose `unlock` alone releases it and whose
shutdown releases them all. The process lines are exact2's own and never
reach `grant.rs`. Replay on a refresh.

## Qualification boundaries

Exact's continuation snapshots the image work visible to its runtime owner turn.
It can cancel those captured jobs, including other jobs already visible in that
runtime; it cannot discover or cancel jobs admitted afterward. This is runtime-work
ownership, not cancellation correlated to one JavaScript Promise. A losing
cancellation waits outside shared locks for publication's actual result.

The ImageIO codec bounds source pixels and retained encoded output, and admits
decoded/intermediate surfaces using component depth, aligned rows, decoder
workspace and 25% native-workspace headroom within its 512 MiB scratch estimate.
Large RGBA16 cases that previously exceeded the job reservation are now refused
before pixel decode. LLP 1069.002's **Native image memory qualification,
2026-10-13** records the retained adversarial corpus, baseline/final RSS and
four-job cancellation/teardown evidence at Exact
`2dd657fd7d93176b6d8e924849ed85170beda67c`, from baseline `dfa6a26344cf`.
That lane is integrated as `1b797a773e80b579e55a33e19dade886bcc31c0a` and is
included in the `4fd19fb4e2` checks below. The decision keeps in-process ImageIO
for this app-local API. The estimate is not a proven cap on ImageIO's allocator,
peak process RSS or native-call duration; hard process containment would require
a separate codec service. The configured worker limit belongs to each work owner.

### Previous landing evidence

The adoption delivered as Exact `6dd91519c4292a10c7b6450abe9e082c7fcffdff`
and Ibex `595224eb9e6e2419a82b0622a7c710a9eceb09f6` used vendored source
`de11540` and the v4 bundle. Its focused qualification passed 108 real lean JS
integration tests, 22 native storage tests, 15 Fieldnotes mixed TypeScript/Rust
persistence tests, and the render suite. Real native HTTP
drain/redirect/deadline cases passed in both modes. The separate Snapback
workspace passed 15 real-server client tests, six durable-device tests and one
client unit test against its pinned server. The browser grant corpus passed
111 assertions. These receipts belong to that landing, not the newer source pin.

The macOS Fieldnotes app was built and driven with the named scratch store
`ibex-adoption-20261013`: save a note, write/read its Rust backup, restart,
delete the note, restore that file through TypeScript, then restart again.
The observed count changed 1 → 0 → 1, and the final title/body matched the
saved note with no pending work. This uses isolated test storage, not an
existing notebook. All six required Exact checks passed on `6dd91519c`;
the focused checks, app drive, final checks and production lean phase diagnostic
retain their individual revisions in the local consumer-adoption run ledger.

### Historical v4 follow-up evidence

Exact `4fd19fb4e2d5bdad1311a18a694feff9b6f1efc3`, with vendored source
`b20dded` and the v4 bundle, passed all six required checks: build, tests,
clippy plus fmt, caps, boot, and the full workspace check. Its additional
`exact-data-host` run passed 22/22 tests. The signed macOS Conduit fixture,
using isolated identity `com.ccheever.ibexasyncqualification` and scratch
storage, was built with `EXACT_HTTP_ASYNC=1` and driven through
home → article → home → login. Four articles loaded, the article body was
present, and every pending queue was empty. The app remained signed out and
issued no public writes. Four recorded HTTP transactions used h2 with one
observed endpoint pair. This is functional reuse evidence, not a timing result.
The consumer-release run retains `exact-4fd-six-gates.json`,
`exact-4fd-conduit-macos-receipt.json` and their command logs.

The rustdoc-only refresh to `bf74e54` passed formatting, caps, both standalone
locked/offline metadata checks, installer/root lock overlap and the vendor
comparison.

### Published v5 qualification

Exact `56f107fa44bf65c3df043efd3b784e7dd71484a5` (Q) consumes upstream
`e60b3e0` and the published v5 bundle. Q includes the accepted mainline through
`e8b90ef40`; all results in this section used that clean source revision.
The explicit installer acquired the authenticated bundle, and its offline
`--check` passed before compilation. The macOS archive digest is
`bc71c7b398f1d41a5390761b75ce72e7be234aa48cb1ddec947224188ed6a4f2`.

All six required Exact checks passed on Q, with 22/22 additional native
storage tests. The semantic comparison selected 298 corpus cases, 40 generated
cases and eight lowering cases: no divergence. The eight lowering cases covered
1,203 agreeing runs; seven bodies were outside the modeled fragment.

The signed Conduit fixture was rebuilt with v5 and `EXACT_HTTP_ASYNC=1`.
Its native Mac drive again loaded four articles, opened a nonempty article,
returned home and reached login. It stayed signed out with no pending work or
public writes. Four lane transactions used h2 on one observed endpoint pair;
no timing or LTE qualification is inferred. The executable SHA-256 is
`f7c169d57dd6a35c8ae25cc2d87987aba81d300e33790a1b948cb1be3c645ca6`.
The native resolver outputs name the v5 cache, and the owned app process exited.

Native x86_64 Windows Q also installed and verified v5 offline. Its archive
digest is `db9233d08c25a33d7b1cdd2d0eb9428e98020fa6de7ab396b0adfb241b61d1e0`;
receipt schema `ibex/hermes-upstream-pinned-receipt/2` has SHA-256
`32c11e970e9783fc8f4b5171700e146c0146a17ad7593a8237c70a3f9b34903e`.
The locked/offline run passed 25 data-host tests, two JS library tests and
101 JS integration tests, with one manual startup diagnostic ignored. The
ordinary TypeScript app rebuilt and its platform-delivered button presses
changed the Hermes-backed resource 0 → 1 → 2. Its EXE SHA-256 is
`81d1932eb51ee3e36410370cddf6ece5e272f3837788f54f349b85d0a3d3ac61`.
No app or build process remained. The Windows render-host HTTP compilation
boundary recorded above remains open; these results do not qualify that host.

The consumer-release run retains `exact-56f107fa4-six-gates.json`,
`exact-56f107fa4-conduit-macos-receipt.json`, `windows-v5-ledger.json`, the
Android receipt named in row 8, and their logs. Codex and Grok independently
cleared Q's code and pin adoption. Later evidence-only commits preserve Q's
runtime and native proof attribution; their required gate receipts name their
own clean revisions.

### Windows render-host HTTP follow-up — 2026-10-14

The completed follow-up at Exact
`b195e96dc953733589e54167286da92f59725389`, with current main
`0d2d2e28e972ae5bf603105c794b45fecc6a2b08` merged and the vendored Ibex pin
`e60b3e0570315f31710744c1187f3cec5574fedf`, closes the current Windows
render-host boundary without changing Q's historical receipts. Windows now
uses a private loopback TCP wake pair with WinSock `WSAPoll`; Unix keeps its
existing `UnixStream` and `poll` path. Pending wake state clears only after a
byte is consumed, and wake-channel failure releases queued and held sockets.
Both platforms retain LLP 1048.000 D10's 2-second per-socket hold, 64 KiB read
cap, 256 held-connection cap, bounded inbox, and immediate half-close. The
server's separate `Stopper` waits up to twice the render deadline plus
10 seconds for render workers, then drains the closing thread's remaining
2-second socket holds.

On native Windows, the focused real loopback executor test passed, followed by
118 unit tests, 37 integration tests, and one look test, with nine ignored and
no failures. All six focused linger tests, all-target clippy, and caps passed.
With `EXACT_HTTP_ASYNC` unset, the directly built Hermes-backed Weatherlight
renderer served its generated plan over loopback: health and `/` returned 200,
and `/` carried 100,584 bytes of rendered Weatherlight HTML. The standard web
build's existing colour-warning gate failure remains a retained failed attempt,
not a whole-app build pass. This run reused the authenticated v5 receipt and
archive hashes recorded above.

An independent native Linux run of the same corrected source passed 120 unit
tests, 37 integration tests, and one look test, with nine ignored and no
failures; all six linger tests and all-target, all-feature clippy passed. The
source-attributed commands, hashes, superseded first implementation, retained
failures, and app drive are recorded in the
[closed Windows HTTP issue](../../issues/closed/20261013-exact-render-windows-http.md).

### Physical-device evidence and remaining rollout boundaries

Six bounded ImageIO/ImageJob cases passed on a physical iPhone 17 Pro Max running
iOS 27.0.1 (24A446), using production codec and vendored job code from Exact
`a01c62241440300f3644ecde83592606262c58e7`. They cover four concurrent image
transforms, RGBA16 input, predecode refusal, cancellation, owner close, and
cancellation after commit starts. The largest observed process resident peak
was 195,837,952 bytes; sampled physical footprint peaked at 118,916,840 bytes.
[LLP 1069.002](../../llp/1069.002-media-picker.rfc.md) records the fixtures,
source hashes, retained receipts and limits. This finite workload does not prove
a whole-app memory limit or behavior under memory pressure.

The async-enabled iPhone app launched, but its control connection could not be
established. Rebuilding the scratch fixture with the documented local-network
permission setting reached a connection timeout. Network navigation and
same-store restart interaction remain unqualified pending device connectivity
and consent; the image results above do not substitute for them.

The isolated arm64 Android Shelf APK also built with the authenticated v5
bundle from Exact `a01c62241440300f3644ecde83592606262c58e7`. Its SHA-256 is
`2dc2aa1ed32f146d77e79962ec5b6b04611e30ad7affa68c4fe4a676c66f5588`;
it requires Android API 31 or newer. No physical Android device was
reachable for installation and interaction in this follow-up, so this remains
build evidence only.

Charlie assigned the historical bsky4 rollout campaign to the agent on Tuft;
LLP 1120 §5 records the original baseline and remaining measured gate. Async
remains off by default. Independent stage-1 code review, controlled TLS
setup-stall diagnostics, Mac smoke, and the bounded image tests do not replace
that campaign.
