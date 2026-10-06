# Adopt the split ibex2 (bindings door on the lean VM, pinned Hermes bundles) in exact-js

**Status:** Open
**Systems:** js (exact-js), vendor/ibex/crates/ibex2, Hermes engine pin, build
**Severity:** P2 (no breakage today; exact2 runs on its ibex1-era vendor)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Related:** expo/ibex LLP 0057.000 §5.0 (three doors), §6 L1 (L1b–L1h notes); exact2 LLP 1027.006 (Windows native TypeScript); vendor/ibex/crates/ibex2/EXACT-PATCHES.md

## 2026-10-06 E2 checkpoint

Exact now vendors Ibex `ec949fef` and its v4 bundles. All Exact patches replay
with the same stable patch ID. `exact-js` selects the English `intl` tier and
installs `INTL` on Linux/Windows only; Apple retains engine-owned OS Intl.
Fieldnotes launches with TypeScript on both iOS and tvOS Simulators, including
the universal-iOS thinning path, and tvOS targets 17.0. `setup --check` now
authenticates the canonical receipt and every compiler/header/VM/ICU member.
Ibex's installer at this snapshot has no read-only `--check`; upstream should
expose its resolver validation so Exact can call that instead of maintaining a
schema-parallel check. No vendor-only mode was added here.
This issue stays open only for the actual Linux and Windows qualification runs;
the Windows build path is implemented but must not be described as qualified
from macOS inspection.

## Why this is open

Ibex 2 now lives in its own repository, github.com/expo/ibex, and is split into three
crates: `ibex2` (the engine-free library, plus an off-by-default `bindings` feature
that installs ibex2's web APIs into a caller-owned JSI runtime), `ibex2-runtime`, and
`hermes-lean-sys` (which resolves, verifies, and links Hermes). exact2 still vendors
the pre-split single crate from expo/ibex1 (`e3e00690` plus Exact patches 3–5) and
builds Hermes itself. Ibex's plan (LLP 0057.000 decision C) is for exact-js to install
ibex2's `PURE` group through the bindings door and run ibex2's hardening before app
code. That was paused on 2026-10-05 because it conflicted with LLP 1027.006 (keep the
Hermes pin; no download or compiler work in build.rs; Windows lean with Intl). Charlie
chose to make ibex2 fit exact2's rules first. This issue records what is now in place
and what adoption still needs.

## What ibex2 now provides (all landed on expo/ibex main)

- **Lean VM as a supported option** (L1f). Every release bundle carries both the full
  (`hermesvm_a`) and the lean (`hermesvmlean_a`, no compiler) VM. `hermes-lean-sys`
  has `link-lean`, exclusive with `link`. The linked identity always names the archive
  actually linked, and lean is offered only when a receipt authenticates it.
  Measured on arm64, stripped: a minimal embedding is 1.63 MB on lean and 2.63 MB on
  full.
- **Pinned, attested bundles.** The current set is `hermes-vanilla-d412d3bd8512-v4`
  (2026-10-06): every bundle has both VMs, Linux carries base, English and full
  ICU 74.2 tiers, and Windows and tvOS are included. Immutable GitHub prereleases on expo/ibex, built from
  `main` by an unprivileged builder and published by a default-branch-only publisher.
  Each archive has a Sigstore attestation bound to the publisher on `refs/heads/main`.
  `hermes-lean-sys` pins each bundle's SHA-256 in source and verifies it before
  extraction, and again on every cache admission.
- **Install once, build offline** (L1h), for LLP 1027.006's rule. `cargo run
  --manifest-path <ibex>/crates/hermes-lean-sys-installer/Cargo.toml -- --target <triple>`
  installs verified bundles into the cache. With `[env] HERMES_LEAN_SYS_OFFLINE = {
  value = "1", force = true }` in `.cargo/config.toml`, build.rs never touches the
  network. A vendored copy must include `crates/hermes-lean-sys-installer` beside
  `hermes-lean-sys`. Build scripts still run the pinned `hermesc` (to compile ibex2's
  binding bytecode), as exact2's own `js/build.rs` already does; Hermes itself is never
  built in build.rs.
- **Intl and ICU are opt-in** (L1g; Charlie: "things that might not be used should be
  optional"). On Linux, Hermes is built with its own Intl off, and ICU 74.2 is trimmed
  to root+en data by default. The full locale data and ibex2's Intl implementation come
  only with `ibex2/intl`. Stripped Linux `ibex2-runtime`: 10.95 MB (arm64) and 11.7 MB
  (x86_64) by default, versus 42.3 MB with full ICU. Basic JavaScript stays correct (`é→É`, `ß→SS`,
  normalize, collation, dates). Apple uses the OS frameworks (Intl costs about 100 KB).
  Windows uses the OS `icu.dll` for basic Unicode, and Hermes Intl is off there.

## What adoption needs in exact2

1. **The Hermes pin.** exact2 pins `6badada7` (`260318099.0.0`); ibex2's bundles are
   `d412d3bd` (`260318099.0.4`). Adopting the bundles means bumping, and LLP 1027.006
   currently says "no engine upgrade". The alternative is to keep exact2's own lean
   builds and point `HERMES_LEAN_SYS_DIR` at a receipted install. That needs a receipt
   v2 for exact2's builds, and ibex2's binding bytecode must match exact2's HBC
   version.
2. **Windows Intl.** exact2's Windows apps need `Intl.DateTimeFormat` (six existing
   tests). Hermes's Windows Intl requires the ICU C++ API and ICU 65+, which Windows'
   `icu.dll` doesn't export. ibex2's own Intl shims use only the ICU **C** API, which
   `icu.dll` does export. A possible ibex2 lane: build the `intl` shims on Windows
   against the OS `icu.dll`, giving Windows Intl with no bundled data. Not started;
   needs a spike.
3. **Re-vendor onto the split.** Take `crates/ibex2`, `crates/ibex2-sqlite`,
   `crates/hermes-lean-sys`, and `crates/hermes-lean-sys-installer`. Re-apply Exact
   patch 3 (rustls on macOS), patch 4 (grant grammar from exact-grants), and patch 5
   (`doc:` documents; it rebases onto the heavily changed `task.rs`/`boundary_abi.rs`).
   The Windows backports are already upstream. Rewrite the update recipe in
   EXACT-PATCHES.md. Host crates using `stdlib::websocket` need `features =
   ["websocket"]`, or listening WebSockets become `Unavailable`.
4. **exact-js changes** (scoped 2026-10-05; about 11 files and 250–350 lines):
   - `js/Cargo.toml`: depend on `ibex2[bindings]` and `hermes-lean-sys[link-lean]`.
   - `js/build.rs`: stop compiling the vendored `ibex2_jsi.cc` (ibex2 compiles
     `bindings/install.cc` itself); take headers and `hermesc` from `DEP_HERMES_LEAN_*`;
     drop exact's own engine resolution and link lines (ICU included); take
     `HARDEN_BYTECODE` from ibex2.
   - `js/src/shim.cc`: construct the `Adapter` at create time, `install(GROUP_PURE, …)`
     with `ibex2::bindings::compiled_scripts(Groups::PURE)`, reuse the adapter for
     storage, and call `Adapter::harden()` on every load before app bytecode.
     Hardening becomes universal (today only storage modules are hardened).
   - `js/src/engine.rs`, `lib.rs`, and `storage.rs`: create the `Context` before the
     engine for every module, keep drop order, and stop passing `bindings::Context` in
     stub builds.
   - `js/src/pure.js`, `pure.rs`, and `prelude.js`: drop the URL and search-params
     natives and exact's own `Headers` in favor of ibex2's.
   - Stub builds (tvOS, Hermes-less CI): gate `bindings` so `EXACT_JS_ENGINE=stub`
     still builds without an engine.
   - The bake (`js/bake/src/lib.rs`) resolves `hermesc` the same way.
   - There is no C `ibex2_install` function. The door is the C++ `Adapter`, as ibex2's
     own embedding test uses it.
5. **Measure** exact-js before and after (size and startup), per platform, against
   LLP 1027's budgets.

## Recommendation

Decide item 1 first. If exact2 keeps `0.0`, adoption is items 3–5 with exact2's own
receipted lean installs and no bundles. If it bumps to `.0.4`, it gets the attested
bundles, install-once, and the trimmed Linux ICU for free. Item 2 is an ibex2 lane
either way.
