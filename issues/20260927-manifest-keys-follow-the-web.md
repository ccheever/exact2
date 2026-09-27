# Manifest: tsconfig `paths` replace `typescript.aliases`, outside-app resolution is `--locked`, and the dark launch colour is `background_color_dark`

**Status:** Fixed: both TypeScript producers read captured tsconfig paths/baseUrl, app resolution never rewrites locks, and web/iOS launch colours use background_color/background_color_dark; compiler regressions and five focused app/real-actool checks pass (full-suite baseline failures recorded below).
**Systems:** `scripts/app.mjs`, `scripts/app.schema.json`, `js/bake`, `host/apple/build.mjs`, apps' `app.json`
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1036.001 and LLP 1030 (the 2026-09-27 ruling), LLP 1069.008 (permissions)

**Do:**
- Read TypeScript import aliases from the app's `tsconfig.json` `compilerOptions.paths` (and `baseUrl`), and delete `typescript.aliases` from the schema and from every app that uses it (weird-castle and grnl are outside this repo: note what they need to change).
- Run `resolveApp`'s Cargo resolution with `--locked` (`scripts/app.mjs:174`, `:348`). A lockfile that is out of date is an error naming the update command, never a silent rewrite.
- Replace `launch.background`/`launch.backgroundDark` with the manifest's `background_color` and a new `background_color_dark`, in the web manifest's snake_case; update the schema, `host/apple/build.mjs` and the apps.
- Permissions are LLP 1069.008's (delete `host.*.permissions`); don't duplicate that work here.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

Implementation and verification (Astra, 2026-09-27):

- `scripts/app.mjs`: both resolution metadata calls use `--locked --offline`;
  missing/stale outside locks are refused with the explicit update command.
  The regression checks a missing lock stays missing and a stale lock stays
  byte-for-byte unchanged, then exercises deliberate lock creation.
- `scripts/app.schema.json`, `host/apple/build.mjs`, `host/web/build.mjs`:
  delete `launch` and `typescript.aliases`; accept `background_color_dark`;
  generate both web first-paint CSS appearances and iOS launch assets from the
  snake_case fields. No in-repo app declares a retired key.
- `js/bake/src/{lib,resident}.rs`: remove manifest alias translation. Both
  producers use one captured tsconfig resolution configuration. JSONC, local
  `extends`, exact/wildcard paths, ordered fallbacks, `baseUrl`, and paths into
  declared `typescript.sources` mounts work; uncaptured paths are refused.
  TypeScript 7's removed `baseUrl` is lowered into equivalent `paths` for both
  TypeScript and Rolldown. Checking policy remains producer-owned.
- Regressions in `scripts/app.test.mjs`, `js/bake/src/sources_tests.rs`,
  `js/bake/src/resident.rs`, and `js/bake/tests/producer.rs`. Before the fix:
  resolution wrote the missing lock, real `actool` emitted only one launch
  appearance, the schema rejected the dark key, and the compiler reported
  TS2307 for the tsconfig alias. After the fix, real TypeScript/Rolldown
  one-shot and resident compilation agree, including config invalidation,
  mounted sources and refusal/recovery. The real iOS catalog retains AppIcon
  and both ExactLaunch appearances.

External apps (not edited):

- **weird-castle:** in `app.json`, delete `typescript.aliases`. For each old
  entry `"A": "T"`, put `"A": ["./T"]` and `"A/*": ["./T/*"]` in the
  app's `tsconfig.json` `compilerOptions.paths` with `baseUrl: "."` (or the
  equivalent paths relative to its existing baseUrl). Keep existing matching
  editor paths instead of duplicating them. Keep `typescript.sources`;
  paths may also point to those mounts' original directories. Move
  `launch.background` to `background_color` (its previous override wins),
  `launch.backgroundDark` to `background_color_dark`, and delete `launch`.
  If the lock is stale, run `cargo metadata --offline --format-version 1`
  explicitly from its Cargo workspace, review and commit `Cargo.lock`.
- **grnl:** make those same exact key/entry transformations in its own
  `app.json` and `tsconfig.json`, preserving its existing colour values and
  source targets; explicitly update its Cargo workspace lock with the same
  command when needed. Permissions belong to LLP 1069.008's separate ticket.
- Neither outside app is present at `~/projects/weird-castle` or
  `~/projects/grnl` on this machine, so their current literal aliases and
  colours could not be inspected. The transformations above specify the
  changes without inventing their values. No Charlie design ruling remains.

Check results:

- Focused Bun regressions, including real Xcode `actool`/`assetutil`:
  `5 pass`, `0 fail`.
- `EXACT_JS_ENGINE=stub cargo test -p exact-js-bake --lib -- --nocapture`:
  `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
  These tests run actual TypeScript and Rolldown in both producer modes; they
  do not load Hermes. The full HBC bake could not run because this machine
  lacks the pinned macOS Hermes archives.
- Full `bun test scripts/app.test.mjs --timeout 60000`: `53 pass`, `2 skip`,
  `2 fail`. Both failures are unchanged `ordinary buildBake with split
  directories` tests: their empty Contract reports `lower-one-root` before
  the tests' expected `failed: exit 101`. They are outside this ticket.
- Required build, test and clippy commands were run with the requested
  all-targets/keep-going/no-fail-fast flags. All three stop on the same
  baseline `contract/cli/tests/it/time.rs:220,223` E0061 failures: calls to
  `set_place` lack its third argument. Reproduced before implementation;
  left for the time ticket's worktree.
- Targeted `EXACT_JS_ENGINE=stub cargo clippy -p exact-js-bake --all-targets
  --keep-going -- -D warnings`: `Finished dev profile`, exit 0.
- `cargo fmt --all -- --check`: exit 0, no output.
- `git add -A && bun scripts/caps.mjs`:
  `All budgets within cap. 6 categories inspected.`
- `bun scripts/boot.mjs`: exit 0;
  `boot — modules reachable before first pixel: 2 (host/web/glue.js, host/web/navigation.js); wasm references: 8`.
- `bun host/web/build.mjs caltrain-web`: exit 0;
  `host/web/dist: app.wasm 909 KiB (377 KiB gzip), app.plan 37 KiB; documents: 2 documents`.
  The installed wasm-split is 133 instead of pinned 132, so the successful
  build reports its existing unsplit fallback.
