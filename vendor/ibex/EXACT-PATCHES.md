# Vendored Ibex split crates — Exact integration ledger

## Snapshot and update recipe

- Upstream: `https://github.com/expo/ibex.git` at
  `de115401e79843c0e6458718e9651ca3bcb755fd` (2026-10-13).
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
replacement and consumer evidence. Qualification is pending the full consumer
checks on this refresh; implementation replacement alone does not retire a row.

| Former Exact patch | Upstream replacement | Exact consumer evidence |
|---|---|---|
| 3. Rustls on macOS | Explicit `portable-http` feature; native Apple default retained | Render selects the feature; HTTP/executor proof pending |
| 4. Shared `exact-grants`, including `doc:/` | Ibex owns an independent grant grammar; no Exact dependency | Shared native/browser fixture corpus now tests both Rust grammars; native parity passed |
| 5. Chosen documents | Typed provider and owned `DocumentLease`, with retained directory/file capability and scope guard | Exact table supplies leases; chosen-file/folder/session, replacement and bound tests pending |
| 6. Denied redirect origin | `HostError::DeniedRedirect`, shared hop admission | Exact runner redirect/refusal tests pending |
| 7. Per-fetch timeout | Request timeout plus asynchronous exchange lifecycle | Exact retains its single total deadline; redirect/deadline proof pending |
| 9. Image compression | Admitted `ImageJob`, bounded `ImageService`, owner-scoped work handles and atomic commit arbitration | Native codec and winning/losing cancellation with actual output consumption passed; real lean JS proof pending |
| 10. Capped app reads/open handle | Public `AppDirectories::open_file` and `read_bounded` on Unix/Windows | Exact chunked upload reader uses the opened file; upload proof pending |
| 11. Host work in flight | Context count plus owned registry/worker/completion accounting | Exact uses captured work handles; retirement/cancellation proof pending |
| 12. Bounded native filesystem results | Explicit `ResultLimits`; bounded reads/enumeration and batched recursive removal | Exact supplies 16 MiB envelope; native/app bound passed, document cap follow-up pending |
| 13. Async fetch | Fallible preparation, infallible accepted activation, terminal retirement, bounded portable workers | Exact adapts reserve/refusal/deadline/accounting; runner proof pending |
| Windows chosen-document EISDIR | Same retained-handle native document executor | Upstream native Windows qualification; Exact Windows runtime not run here |

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

Borrowed timer task support does not install ambient timers in Exact. Its
production group selection remains `PURE | CRYPTO | ABORT` (plus the existing
platform Intl selection), and Exact's trusted prelude retains its refusal policy.

The existing Snapback consumer proves partitioned storage, durable operation
queues, reconnect and account retirement. There is no Snapback asset-upload API
in this checkout: the deterministic losing-cancellation output witness is Exact's
native storage adapter. No new Snapback media feature or qualification is claimed.
