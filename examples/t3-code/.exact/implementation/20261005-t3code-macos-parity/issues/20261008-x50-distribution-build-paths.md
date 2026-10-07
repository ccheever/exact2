---
name: 20261008-x50-distribution-build-paths
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-portable-app-download]
upstream_url: null
reproduced_on: feat(example)/t3-code-portable-app-download on the feature branch's framework (main 1f19b2400)
---

# X50: a distributed macOS build carries the build machine's paths

## Summary

`host/apple/build.mjs <app> --bundle --distribution` (what `exact release` signs) leaves absolute
paths of the build checkout, Cargo's home, the Rust toolchain and Xcode in the binaries it
ships. A downloadable build then names the folder and user it was built by, and an audit that
refuses build-machine paths cannot pass without app-side work. Most of it can be removed by the
app's packaging step; two framework build scripts cannot be worked around from an app.

## Why this issue arose

### What the clone needs
The downloadable `.app` (20261005-portable-app-download, spec "Delivery") must contain no path of
the machine that built it (`audit-bundle.mjs`, rule `machine-path`).

### What exact2 does today (measured on the clone's packaged build, 2026-10-08)
A `--distribution` build of `t3-code-macos` in an export at `/tmp/t3-code-package/exact2`, before any
packaging fix:

| Where | What | Source |
| --- | --- | --- |
| the app executable | 13 `<checkout>/vendor/ibex/crates/ibex2/src/bindings/*.js` and `<checkout>/target/aarch64-apple-darwin/host-dev/build/exact-js-<hash>/out/prelude.js` | `vendor/ibex/crates/ibex2/build.rs` `compile_javascript` and `js/build.rs` run `hermesc` on absolute source paths; Hermes keeps them as each function's file name |
| the app executable | Rust panic locations under the checkout and `~/.cargo/registry` | the native Cargo build passes no `--remap-path-prefix` (the wasm build does: `scripts/app.mjs` `wasmRemapFlags`) |
| the app executable | `LC_RPATH` `/Applications/Xcode.app/…/usr/lib/swift-6.2/macosx` and `/var/run/com.apple.security.cryptexd/…/Metal.xctoolchain/usr/lib/swift-6.2/macosx` | SwiftPM's back-deployment rpaths; nothing in the app loads from them |
| `libexact_canvas_gpu.dylib`, `libexact_svg.dylib` | `~/.rustup/toolchains/…/lib/rustlib/…/lib*.rlib` (the linker's debug map) | `exact release`'s `stripForDistribution` strips only the main executable |
| every `libexact_*.dylib` | an install name that is a build path (`…/target/apple-swift/macos-14.0/arms/<hash>-libexact_web.dylib.<pid>.tmp`, `…/target/apple-modules/…/libexact_canvas_vello.dylib`) | the arms and the kept modules keep the name they were linked under |

## Why it must be resolved

Every app that is downloaded rather than built by its user ships its builder's home folder and
checkout path. That is a privacy leak (the user name) and makes the bytes differ by checkout,
so two people cannot build the same artifact.

## Requested support

For `--distribution` (and therefore `exact release`): remap Rust source paths for the app's own
target as the wasm build does (checkout to ``, Cargo's home to `cargo`, std to `/rustc/<commit>`);
give `hermesc` relative source names (run it from the crate's folder) or strip its debug file
names; `strip -x` every Mach-O file of the bundle; drop absolute rpaths outside `/usr/lib` and
`/System`; give each bundled dylib an `@rpath/<name>` install name.

## The clone's workaround (example-local, `package-app.mjs`)

- `CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS` with the three remaps (the native target only, so the
  wasm build's own flags stay).
- `strip -x` on every file in `Contents/MacOS`, `install_name_tool -delete_rpath` for the toolchain
  rpaths and `-id @rpath/<name>` for the dylibs, before the ad hoc signature.
- The export lives at the fixed `/tmp/t3-code-package/exact2`, which names no user or checkout; the
  13 hermesc file names under it are the only build paths left and `bundle-allowlist.json` lists
  them with this issue as the reason.

## How to reproduce

```sh
EXACT_APP_DIR=$PWD/examples/t3-code EXACT_IDENTITY=- bun host/apple/build.mjs t3-code-macos --bundle --distribution
B="target/clients/<key>/com.exact.t3code.macos/macos/T3 Code (Exact).app/Contents/MacOS"
grep -ac "$PWD" "$B/T3 Code (Exact)"          # > 0: the checkout path
ln -sf "$PWD/$B/T3 Code (Exact)" /tmp/exe && otool -l /tmp/exe | grep -A2 LC_RPATH   # toolchain rpaths
otool -D "$B/libexact_web.dylib"              # a build-path install name
```

`bun examples/t3-code/audit-bundle.mjs "<that>.app"` lists every hit by file.

## Acceptance for the fix

A `--distribution` bundle built in any checkout has no `machine-path` or `build-path` finding from
`audit-bundle.mjs` with an empty `allow` list (only `/usr/lib/swift` rpaths and system libraries).
