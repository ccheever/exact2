# Vendored Ibex split crates — Exact patches

## Snapshot and update recipe

- Upstream: `https://github.com/expo/ibex.git` at
  `ec949fef1cf4f92a8ce09bbc357ba8b150ec25a0` (2026-10-06).
- Vendored: `crates/{ibex2,ibex2-sqlite,hermes-lean-sys,hermes-lean-sys-installer}`.
- Minimum external inputs: `scripts/icu74-filter-{root-en,en-intl}.json`,
  embedded by `hermes-lean-sys`'s receipt tests, and
  `third_party/wpt/urltestdata.json`,
  read by `ibex2/tests/wpt_url.rs`. Repository engine-output directories are
  deliberately absent; consumer builds use the verified bundle cache.
- Hermes pin: Ibex release `hermes-vanilla-d412d3bd8512-v4`, whose attested
  artifacts carry Hermes `260318099.0.4` / source `d412d3bd…`.

The `ec949fe` refresh replayed the complete Exact delta below from the former
`0cd42c8` snapshot with the same stable patch id (`9d5dc416…`). Upstream's v4
pin adds tvOS device and arm64 Simulator bundles, thin-at-build universal Apple
Simulator archives, three Linux ICU data tiers, Windows OS-ICU Intl, and native
Headers lifetime ownership. None replaces an Exact-only patch.

Stage an update without touching a live Ibex checkout, then review it as a
three-way refresh against this ledger:

```sh
snapshot=<new-ibex-commit>
stage=$(mktemp -d)
git -C ../ibex2-i45 archive "$snapshot" \
  crates/ibex2 crates/ibex2-sqlite crates/hermes-lean-sys \
  crates/hermes-lean-sys-installer scripts/icu74-filter-root-en.json \
  third_party/wpt/urltestdata.json | tar -x -C "$stage"
diff -ru vendor/ibex "$stage"             # classify upstream movement first
base=$(mktemp -d); exact=$(mktemp -d)
git -C ../ibex archive e3e00690 crates/ibex2 | tar -x -C "$base"
old_crate=ibex2; old_vendor=vendor/$old_crate
git archive origin/main "$old_vendor" | tar -x -C "$exact"
diff -ru "$base/crates/ibex2" "$exact/$old_vendor" # recover the old Exact delta
```

Replace only the four snapshot crate trees and two named data files, update
the hash above, then reapply every carried patch below and run both the Ibex
crate tests and Exact's consumers. Do not import repository build outputs.

## Delta classification from the pre-split snapshot

The migration compared `e3e00690:crates/ibex2` from the read-only pre-split
Ibex repository with exact2 `origin/main`'s former vendor tree, hunk by hunk.

| Old local delta | Classification at this snapshot |
|---|---|
| Windows nonblocking connect readiness / WinSock features | Dropped: upstream (`107b5f8`) |
| Windows native filesystem backend, grants, and tests | Dropped: upstream (`6a23ec8`) |
| Lazy process-wide rustls trust-store cache | Dropped: upstream |
| macOS rustls target and writable-connect polling | Carried: Exact patch 3 |
| shared `exact-grants` grammar, including `doc:/` | Carried: Exact patch 4 |
| chosen-document Context, dispatch, and executor | Carried: Exact patch 5 |
| refused redirect reports its destination origin | Carried: Exact patch 6 |
| per-fetch deadline (`exactTimeout`, `Request::timeout`, `Timeout`) | Carried: Exact patch 7 |
| Windows chosen-document same-handle EISDIR read | Carried: Exact-only |
| `587ddf4f4` / `eb7cbfb99` vendor hunks | Carried below: document tests / grammar adaptation |

## Carried Exact-only patches

### 3. rustls transport on macOS

Exact's render/server role needs the rustls transport on macOS even though the
default Apple app transport remains NSURLSession. `Cargo.toml` and
`src/transport/{mod.rs,rustls_http.rs}` enable it and wait for writable socket
readiness rather than sleeping. The trust-store cache is no longer local.

### 4. one grant grammar, including `doc:/`

`Cargo.toml`, `src/grant.rs`, `src/stdlib/{fs.rs,app_fs_unix.rs,windows_path.rs,
windows_fs_tests.rs}`, and `src/secrets/mod.rs` use Exact's pure
`exact-grants` crate. This keeps the native bindings, Rust executor, and web
runner on one quoted-target grammar while native path realization stays in
Ibex. `doc:/` is the capability namespace for a person-chosen document.

### 5. chosen documents

`src/{bindings.rs,boundary_abi.rs,task.rs}` and `src/stdlib/fs.rs` add the
embedder's document table and dispatch `doc:/` operations through it. Real
paths never enter guest errors; rename/copy/realpath remain refused.

### 6. denied redirect diagnostics

`src/boundary.rs` adds `HostError::DeniedRedirect`, and
`src/stdlib/fetch.rs` returns it before sending an ungranted redirected hop.
The origin is diagnostic only and the tests pin the one-hop refusal.

### 7. caller deadline across a fetch

From Exact `f0f7bc865`, `366a8e636`, and `d6019439c`:
`src/stdlib/fetch.rs` carries `Request::timeout`; `src/transport/darwin.rs` and
`darwin_http.mm` carry the Objective-C bridge's `exactTimeout`; and
`src/transport/rustls_http.rs` applies that timeout to each transport attempt;
redirect following reuses it per hop, so `Request::timeout` alone is not a
whole-exchange deadline. Exact's executor arms one deadline and passes one
caller AbortSignal across every redirect hop, bounding connect, TLS, headers,
body, and the redirect chain as a whole. The executor redirect-chain test pins
that distinction.

### Windows chosen-document EISDIR

`src/stdlib/fs.rs` opens one handle with backup semantics, checks that same
handle's metadata, and reads through it. `windows_document_tests.rs` covers
bytes, directory refusal, grant-before-resolution, and ACL denial. This avoids
a racy precheck and does not reinterpret ordinary access-denied errors.

## Split-crate integration adaptations

`hermes-lean-sys/{build.rs,src/lib.rs}` exports the paired `HERMESC_PATH` and
lets explicit `EXACT_JS_ENGINE=stub` builds remain Hermes-free.
`ibex2/build.rs` emits empty binding artifacts for that stub configuration.
These are Exact integration seams, not changes to the pinned bundle contents;
normal builds still validate and link `link-lean`, and offline misses refuse.
