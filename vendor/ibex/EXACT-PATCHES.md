# Vendored Ibex split crates — Exact integration ledger

## Snapshot and update recipe

- Upstream: `https://github.com/expo/ibex.git` at
  `b20dded821cf284f35930ff8f68933fe4ac6e2b8` (2026-10-13).
- Source trees: `crates/{ibex2,ibex2-sqlite,hermes-lean-sys,hermes-lean-sys-installer}`.
- Data inputs: `scripts/icu74-filter-{root-en,en-intl}.json` and
  `third_party/wpt/urltestdata.json`.
- Hermes bundle remains `hermes-vanilla-d412d3bd8512-v4`, source
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
local source patches. The evidence below qualifies the macOS consumers and
portable adapter; platform-specific limits are named rather than inferred from
source replacement. The consumer deltas are two explicit stub build scripts, two standalone locks,
and the workspace boundaries below.

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
| Windows chosen-document EISDIR | Same retained-handle native document executor | Source patch retired against upstream native Windows tests; Exact Windows runtime remains unqualified here |

Patch 9's app interface stays Exact's and is replayed on a refresh:
`bindings/storage.d.ts` declares `compressImage(from, to, {maxDimension,
maxBytes})` resolving to `{path, type, size, width, height}`, as Exact's docs
give it, and Exact's prelude (`js/src/prelude.js`) adapts that to upstream's
positional binding (`compressImage(from, to, maxDimension, maxBytes)`, replying
`width\theight\tsize`). The October 13 refresh replaced the declaration with
the positional form; `contract/cli/tests/it/typescript.rs` now pins it. Upstream
now preserves Exact's app error branches: a source over the read cap is
`compressImage: too-large:`, and a trial deadline before publication is
`compressImage: timeout:`. This retires the local substring-based image error
patch; exact internal markers and typed deadline errors are matched upstream.
The source pin also deallocates an upload's bytes before invoking its last-release
callback, so the executor cannot refund memory while the allocation is retained.
The standalone manifests, dependency graph and locks are unchanged by this refresh.

The already upstream pre-split Windows connect readiness, Windows filesystem
backend, and lazy Rustls trust-store changes remain upstream.

## 8. Android bundle pin — still pending

Upstream source now supplies the per-target `HERMES_LEAN_SYS_DIR_<target>`
override, Android link recipe and release target. The consumer stays on the
published v4 bundle. An Android-capable v5 publication and reviewed pin update
have not happened in this local delivery, so row 8 is not retired. Existing
`scripts/hermes-android.mjs` local receipt flow remains available. Source support
is not evidence that a released Android artifact was consumed.

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

## Qualification boundaries

Exact's continuation snapshots the image work visible to its runtime owner turn.
It can cancel those captured jobs, including other jobs already visible in that
runtime; it cannot discover or cancel jobs admitted afterward. This is runtime-work
ownership, not cancellation correlated to one JavaScript Promise. A losing
cancellation waits outside shared locks for publication's actual result.

The ImageIO codec bounds source pixels, the explicitly retained decoded and trial
buffers, and encoded output. Its 512 MiB scratch estimate is not a proven bound
on ImageIO's opaque private allocator or peak process RSS. The consumer does not
claim a platform allocator guarantee from the configured reservation.

The final-pin focused run passes 108 real lean JS integration tests, 22 native
storage tests, 15 Fieldnotes mixed TypeScript/Rust persistence tests, and the
render suite. Real native HTTP drain/redirect/deadline cases pass in both modes;
async production selection remains opt-in. The separate Snapback workspace
passes 15 real-server client tests, six durable-device tests and one client unit
test against its pinned server. The browser grant corpus passes 111 assertions.

The macOS Fieldnotes app was built and driven with the named scratch store
`ibex-adoption-20261013`: save a note, write/read its Rust backup, restart,
delete the note, restore that file through TypeScript, then restart again.
The observed count changed 1 → 0 → 1, and the final title/body matched the
saved note with no pending work. This uses isolated test storage, not an
existing notebook. All six required Exact checks are run on the final commit;
full command receipts and the production lean phase diagnostic are in the
local consumer-adoption run ledger.
