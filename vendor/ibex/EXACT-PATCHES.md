# Vendored Ibex split crates — Exact patches

## Snapshot and update recipe

- Upstream: `https://github.com/expo/ibex.git` at
  `7f77c82a12f20a4cd3ebbe01fdaa7f22deb7c985` (2026-10-06).
- Vendored: `crates/{ibex2,ibex2-sqlite,hermes-lean-sys,hermes-lean-sys-installer}`.
- Exact also commits `crates/hermes-lean-sys-installer/Cargo.lock`: the
  installer is a standalone runnable tool here, not an Ibex workspace member.
- Minimum external inputs: `scripts/icu74-filter-{root-en,en-intl}.json`,
  embedded by `hermes-lean-sys`'s receipt tests, and
  `third_party/wpt/urltestdata.json`,
  read by `ibex2/tests/wpt_url.rs`. Repository engine-output directories are
  deliberately absent; consumer builds use the verified bundle cache.
- Hermes pin: Ibex release `hermes-vanilla-d412d3bd8512-v4`, whose attested
  artifacts carry Hermes `260318099.0.4` / source `d412d3bd…`.

The `7f77c82` refresh replayed the complete Exact delta below from the former
`ec949fe` snapshot with the same stable patch id (`9d5dc416…`). Upstream now
also exposes the resolver's offline validation as the installer's `--check`,
makes its resolver fixture host-independent, and resolves the case-mapping
default locale lazily. The v4 pin's tvOS bundles, universal Apple Simulator
archives, Linux ICU tiers, Windows OS-ICU Intl, and native Headers lifetime
ownership remain unchanged. None replaces an Exact-only patch.

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

Replace only the four snapshot crate trees and two named data files and update
the hash above. Then regenerate the standalone installer's lock and align every
dependency version it shares with exact2's root lock. Update this reviewed pin
list when the root lock advances; the check refuses a missed overlap.

```sh
installer_manifest=vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml
cargo generate-lockfile --manifest-path "$installer_manifest" --offline
for pinned in bitflags:2.13.1 cc:1.4.4 cfg-if:1.0.4 crc32fast:1.5.1 \
  find-msvc-tools:0.1.11 libc:0.2.189 rustix:1.1.4 syn:3.0.4 \
  unicode-ident:1.0.24 zlib-rs:0.6.7; do
  cargo update --manifest-path "$installer_manifest" \
    -p "${pinned%%:*}" --precise "${pinned#*:}" --offline
done
bun -e 'const fs=require("node:fs"),read=p=>Bun.TOML.parse(fs.readFileSync(p,"utf8")).package,root=new Map; for(const p of read("Cargo.lock")){const v=root.get(p.name)||[];v.push(p.version);root.set(p.name,v)} const bad=read("vendor/ibex/crates/hermes-lean-sys-installer/Cargo.lock").filter(p=>root.has(p.name)&&!root.get(p.name).includes(p.version));if(bad.length)throw new Error(`installer/root lock mismatch: ${bad.map(p=>`${p.name}@${p.version}`).join(", ")}`)'
cargo metadata --manifest-path "$installer_manifest" --locked --offline \
  --format-version 1 --no-deps >/dev/null
```

Reapply every carried patch below and run both the Ibex crate tests and Exact's
consumers. Do not import repository build outputs.

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

### 8. an Android target for `hermes-lean-sys` (pending upstream)

From Exact's LLP 1107 lane (2026-10-07): `hermes-lean-sys/build_support.rs`
reads a per-target install override, `HERMES_LEAN_SYS_DIR_<target>` (the target
with `-` and `.` as `_`, as the `cc` crate names its per-target variables),
before `HERMES_LEAN_SYS_DIR`, so a cross build's host instance (exact-js's
build-dependency) keeps the pinned bundle while the target uses a local one;
`build.rs` reruns on it and links `aarch64-linux-android`'s bundle with the
NDK's `c++_static`, `c++abi`, `log`, `dl` and `m`. The bundle itself is built by
Ibex's release script with the same pending Android target (Unicode Lite, no
Intl, `ANDROID_STL=c++_static`); `scripts/hermes-android.mjs` runs it and
installs the result. The upstream patch is Ibex branch `android-hermes-bundle`
(local, not pushed). Drop this entry when an Ibex release pins the Android
bundle.

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
