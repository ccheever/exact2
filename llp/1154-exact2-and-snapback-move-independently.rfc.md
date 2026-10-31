# LLP 1154: exact2 and Snapback move independently

**Type:** RFC
**Status:** Review r2, 2026-10-31. r1 was reviewed blind by GPT-6.1 Sol (ultra) and Grok 4.7 (xhigh) (`llp/reviews/1154-exact2-and-snapback-move-independently.{sol,grok}.md`): both NOT READY, both finding the direction right and the native shim on the wrong seam. r2 takes what §R lists. Written at Charlie's request ("we sort of ended up pinning a specific snapback4 version to exact2. i think that was a mistake … the two projects should be able to move independent of each other"), and his request the same day to have it implemented after review is the approval `rules/RULES.md` asks for the build items and the fixture of §3, which are apparatus. Nothing built.
**Systems:** the Snapback adapter (`snapback4/`: `client/`, `src/`, `web/`, `ts/`, `online/`, `web/build.mjs`, `web/device-build.mjs`, `tests/`); the pins (`snapback4/Cargo.toml`, `snapback4/Cargo.lock`, `package.json`, `bun.lock`, `snapback4/readme-tested.json`); the retest apparatus (`scripts/retest-snapback-readme.mjs`, `scripts/check-snapback-readme.mjs`, `scripts/check-snapback-readme.test.mjs`, `scripts/snapback-handshake.test.mjs`); scaffolding (`game/new.mjs`: `wireSnapback`, `buildSnapbackDevice`, `snapbackVersion`; `scripts/exact.mjs`: the `snapback` and `backend` verbs, `new --snapback`; `scripts/app-cli.mjs`); the bake (`js/bake/src/lib.rs`: `snapback_glue`, `module_constants`; `js/bake/src/typescript.mjs`: `snapbackMutationTypes`, `moduleInterfaces`); the data module's native slot (`js/src/native.rs` `NativeModule`, `js/src/lib.rs` `with_native`); the JS target's build (`host/web-js/build.mjs`); the test driver (`scripts/agent-test.mjs`: `devicePreflight`, `fresh`, `backendHead`; `scripts/agent-launch.test.mjs`); the Apple build's modules (`host/apple/build.mjs`, `host/apple/modules.mjs`); the module seam (LLP 1133.001 as built: `contract/cli/src/native/packages.rs`, `artifacts`); Harmony (`apps/harmony/`, `scripts/memory-backend-test.mjs`, `packages/background/test/harmony.mjs`); the async lane (`scripts/async.mjs`: `harmony`, `harmony-check`); the docs (`README.md`, `AGENTS.md`, `CLAUDE.md`, `docs/reference.md`, `docs/start-here.md`, `docs/recipes/snapback*.md`)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-31
**Revised:** r2, 2026-10-31
**Implementer:** a coding lane (codex GPT-6.1 Sol, or Opus 5.5) from 2026-11-01 for §3 and §5, reviewed blind by Astra and Grok before each push; the Snapback side (§4) is Snapback's own document to adopt.
**Related:** LLP 1036.001 D4 (the private Snapback pin leaves the root workspace, ruled 2026-09-30: it moved the pin, which this document removes); LLP 1086 D12 (`exact new --snapback` and the `snapback` verb, which §3 moves to Snapback's CLI); LLP 1153 D2.4 (Harmony's own workspace, kept out of the root lock because of the pin); LLP 1027 D5 (the bake captures npm packages), D8 and D10 (the data module's native slot: `native.call`, host grants); LLP 1133.001 (modules anyone can write: D1 the package kinds, D9 distribution and prebuilt libraries, as built for Linux and Windows on 2026-10-26, and its refusal of a module naming `exact-*` crates); LLP 1133 D8 (every target it names, or a visible absence); LLP 1009 D6 (a GPU module's dylib beside the app, which D3 follows); LLP 1091 D9 (packages through `node_modules`); LLP 1107 (an app on Android is its Rust crate for `aarch64-linux-android`); LLP 1108 and LLP 1110 (what the adapter does; they move with it); Snapback's LLP 3000 (2026-09-13: one interpreter hosted two ways, "the native module on hosts where SQLite is native and bytes are a library — Expo, and Exact2's Apple and Linux hosts") and LLP 3000.001.000 (the build stamp and the compatibility floor)

## R. Round 1, and what r2 changed

Two reviewers, blind, asked for direction and major oversights. Both found the ownership
decision right and the seam wrong: r1 put the native shim on LLP 1133.001's `Package`
trait, which has no storage roots, no grants and no synchronous call, while the driver
already uses the data module's native slot. r2 takes every material concern and the minor
ones the repository bears out.

| Change | Raised by | Where |
|---|---|---|
| The shim implements the data module's native slot (`exact_js::NativeModule`), not the `Package` trait; the bake generates the `with_native` wiring; open question 1 closes | Sol, Grok | D2, D3, §3 |
| Grants stay in Exact: the host hands the native module admitted storage, so the shim names no `ibex2` or `exact-runner` | Sol, Grok | D3, §3 item 2 |
| The library is a dynamic library per target, placed as Exact's own Rust modules are, so two Rust runtimes and two SQLites never share one link | Sol (minor 1), Grok (minor 7) | D3, §7 |
| The package's declaration: one field for the native slot, with the library per target as `{url, sha256}`; the fetch and the hash check are §3's; M5 fetches and refuses a wrong hash | Sol, Grok | D2, §3 item 1, §6 |
| Android is deferred: Harmony has no Android crate; a target the package does not cover is a visible absence; §4's first list is five targets | Sol, Grok | D6, §4, §8 |
| Transport is the driver's `fetch`; the adapter's Rust `DataSource` shape has no consumer in exact2 and goes with the code | Sol | D3 |
| Every Snapback-specific caller joins the deletion: `CLAUDE.md`, the `backend` verb, `app-cli` registrations, `memory-backend-test.mjs`, `packages/background/test/harmony.mjs`, the preflight test, `agent-test`'s mount detection | Sol, Grok | §3 |
| Harmony's lane installs from Harmony's lock on every run and builds no device | Sol, Grok | D5, §3 |
| Step 2 copies the code and publishes; exact2's tree is untouched until step 3; step 3 waits for every ask of §4, not three of five | Sol, Grok | §5 |
| M1 measures D1 (three greps, `llp/` and `apps/` excluded); M3 reinstalls, rebuilds and runs; M6 reopens a retained partition | Sol, Grok | §6 |
| The wasm's glue ships with it and takes its URL from the driver | Sol (minor 2), Grok (minor 6) | D2 |
| Errors: nineteen hashed files and five examples; the device is pinned, the server is free; `publish = false` is upstream's; `engine_info` is the client's and `engine_headers` the hosts'; the floor check is `devicePreflight`'s, not the verb's; seven published releases; the dated list was diary | Sol, Grok | Summary, §1 |

Not taken: Sol's Swift and Kotlin bridges (§7), Grok's `DataSource` in the package
through a generated manifest naming `exact-runner` and `ibex2` (§7).

## Summary

exact2 compiles Snapback's private Rust source, so it must name one Snapback commit, and
it names the matching npm version in the root `package.json`, the README, `AGENTS.md`,
`CLAUDE.md`, the reference, the recipes and two tests. Every Snapback release therefore
needs an exact2 commit; an app's device is the version exact2 compiled, whatever server
the app installs; and Snapback cannot change its Rust internals without breaking exact2.
Six repins in two weeks still left exact2 seven published releases behind (§1).

The decision: **exact2 names no Snapback version, carries no Snapback source and installs
no Snapback binary** (D1). Snapback on Exact is a package Snapback publishes, versioned
with `snapback4` as `snapback4-expo` is: the TypeScript driver, the web's wasm, a Rust
shim for the data module's native slot, and a prebuilt dynamic library per target (D2).
The native boundary is the one JSON call Snapback declares for "Expo and Exact2 hosts",
behind the package's own shim; the host hands the shim admitted storage, so grants never
leave Exact and exact2 never sees a C header (D3). The app chooses the version in its
`package.json`; the server's floor and the device's stamp decide compatibility, as they do
today (D4). exact2 builds four things the seam lacks: fetching and linking a package's
library per target, generating the native slot's wiring from the package, serving a
package's web asset, and a fixture package that proves all three (§3). It deletes
`snapback4/`, the pins, the retest apparatus and every Snapback verb (§3). The move is
three steps in two repositories, and exact2's tree changes only in the first and the last
(§5). Harmony is the proof: it builds and passes on the web, iOS and Linux from its own
lock, and a Snapback upgrade touches nothing outside `apps/harmony` (§6).

## 1. Why

**What is pinned, and where.** `snapback4/Cargo.toml` names three crates from
`github.com/expo/snapback` at one revision with an exact version; in Snapback's repository
the crates are `publish = false`, so no registry carries them. The root `package.json`
installs `snapback4@0.4.33` so tests have a server binary; Harmony's does the same. The
README, `AGENTS.md`, `CLAUDE.md`, `docs/reference.md`, the recipes and two test scripts say
the number; `snapback4/readme-tested.json` records the version, the revision and the hash
of nineteen files, and `check-snapback-readme.mjs` refuses a change to any of them until
`retest-snapback-readme.mjs` runs the README's four apps and the auth recipe against a
registry and rewrites the pins.

**How it has gone.** The pin moved six times between 2026-10-07 and 2026-10-20, from
0.4.13 to 0.4.33; Snapback published seven more releases by 2026-10-29, the latest 0.4.49.
Each repin changed tests and docs with it: 0.4.33 closed a password-only app's development
doors unless started with `--unsafe-dev-doors`, and refused personas a schema does not
declare, so `fresh`, the README's password app, the retest and two fixture schemas changed
with the pin.

**Why the pin exists.** The adapter reaches past the boundary Snapback declares. Snapback's
README defines `snapback4-device` as "the interpreter over its own SQLite behind one JSON
call and a C ABI (the native module for Expo and Exact2 hosts)", and its Expo module links
exactly that: a prebuilt `Snapback4Device.xcframework` whose header has four functions
(`sb4_open`, `sb4_call`, `sb4_free`, `sb4_close`). exact2 instead uses Snapback's Rust
types: `native::Device`, `memory::MemoryStore` and its `load`/`drain`, `opening::Opening`,
`engine::headers()`, `DeviceEngine::current()`, `call::call`. Code compiled against a
private crate's internals must name a commit.

**How close the adapter already is to the boundary.** `exact-snapback4-client` (the sync
rounds, receipts, generation adoption and the change poll, 2,500 lines ported from
Snapback's `local.ts`) drives the device through one trait, `Core::call(Json) -> Json`.
What reaches deeper: `partition::bind` (the device's `meta` and `set_meta`, which the C
ABI's call also answers), `engine_info` in the client (`DeviceEngine::current()`) and
`engine_headers` on each host (`engine::headers()`). The web wasm `exact-snapback4-web`
re-implements what Snapback's published `WebDevice` exposes: construct with the kept
changes, `call`, `drain`, `engine_info`.

**The seam the driver uses.** On Apple and Linux the driver calls the app's one native
module, `native.call` (LLP 1027 D8): the app's crate attaches `exact_snapback4::Module`
with `exact_js::Module::with_native`, and the host calls the module's `configure_storage`
with the data, cache and temporary roots before its first `call`
(`apps/harmony/app/apple/src/lib.rs`, `apps/harmony/app/linux/src/main.rs`,
`js/src/native.rs`). The module admits `sqlite.open app:/data/<name>.sqlite` against the
app's grants itself, through `ibex2`. This is a different slot from LLP 1133.001's
`Package` (named sources and sends, `later` with a reply, no roots, no grants).

**Skew is already designed in.** Every server answers `device_compatibility`
(`server_version`, `minimum_version`, `capabilities`); the floor at 0.4.33 is 0.4.24. The
device carries a `snapback4.engine` stamp, and both `snapback4 check`
(`exact_device.rs`: "mounted Exact apps carry their actual wasm engine stamp, not the npm
pin") and exact2's `devicePreflight` compare the two. A device and a server need not be the
same version; the device must sit above the floor with the capabilities the schema
requires. Nothing in that contract needs exact2 to know a version.

**The direction of ownership.** The adapter's Rust client speaks Snapback's HTTP wire;
when the wire moves, the client moves. The party that changes a wire owns the clients
that speak it and ships them under one version: that is how Snapback ships its TypeScript
client and its Expo module, and how `snapback4 check` already knows an Exact app's
`app.json` and device stamp. Exact is the platform; Snapback is a module on it, as any
`@exact/*` package or anyone's module is. The coupling runs one way: Snapback tracks
Exact's seams, which exact2 keeps stable for every module.

## 2. Design

### D1 — exact2 names no Snapback version, carries no Snapback source, installs no Snapback binary

In the exact2 repository, outside `apps/` and `llp/`:

- no dependency on `github.com/expo/snapback`, in any `Cargo.toml` or lock;
- no `snapback4` or `snapback4-*` package in `package.json`, `bun.lock`, any `Cargo.toml`
  or `Cargo.lock`;
- no Snapback version in prose, code, tests or fixtures;
- no Snapback server started by a check or by the async lane except by an app under
  `apps/` from its own install (D5).

The package's name may appear where a consumer is described (a recipe pointing at its
guide); a version and a dependency may not. `llp/` is history and keeps what it says.

What exact2 keeps of Snapback's documented wire, as a consumer: the test operation `fresh`
empties a development server through `POST /dev/reset` and names the flag that opens the
doors; `backendHead` reads a server's commit head before a run. Both detect a server by
what it answers at a loopback origin the app fetches, never by a mount path.

An app under `apps/` is an app: its `package.json` names the Snapback it uses, exactly,
as any app outside the repository does. Harmony's does today.

### D2 — Snapback on Exact is a package Snapback publishes

`snapback4-exact`, published with each `snapback4` release as `snapback4-expo` is, an npm
package the backend's directory installs beside `snapback4`, holding:

- **the TypeScript driver** (today `snapback4/ts` and `snapback4/online`): what an app's
  `app.ts` imports as `import { Snapback } from 'snapback4-exact'`, baked as LLP 1027 D5
  bakes any npm package; the `typescript.sources` mount `snapback4` (a relative path into
  an exact2 checkout) goes away;
- **the web device**: the wasm the page loads (today `exact-snapback4-web`: the device
  over the memory store, the client, `drain` into Exact SQLite) with its wasm-bindgen
  glue beside it, both built and committed by Snapback; the package declares the wasm as
  a web asset, the build serves it under the app's assets and captures it with the
  package's bytes, and the glue takes the asset's URL from the driver, never from a
  constant;
- **the native module**: the Rust shim (today `exact-snapback4`'s `Module`: one partition,
  the client's dispatch) and the device and client compiled by Snapback as a prebuilt
  dynamic library per target (D3), behind Snapback's C ABI;
- **`exact.json`**: the package's declaration, with one field this document adds for the
  data module's native slot:

  ```json
  "native": {
    "rust": "native/lib.rs",
    "class": "Module",
    "library": {
      "aarch64-apple-ios": { "url": "…", "sha256": "…" },
      "aarch64-apple-ios-sim": { "url": "…", "sha256": "…" },
      "aarch64-apple-darwin": { "url": "…", "sha256": "…" },
      "x86_64-unknown-linux-gnu": { "url": "…", "sha256": "…" },
      "aarch64-unknown-linux-gnu": { "url": "…", "sha256": "…" }
    }
  }
  ```

  `rust` is the shim's root file, `class` the type in it that implements
  `exact_js::NativeModule` and has `fn new(app: &str, grants: &str) -> Self`; `library` is
  LLP 1133.001 D9's `{url, sha256}` per target. The package is not a module in LLP 1133
  D6's sense (it declares no named sources or sends; the driver's API is TypeScript), so
  `kind`, `names` and `implements` do not apply to it;
- the guide (today `snapback4/README.md`), the Exact scaffold (`snapback4 init --exact`,
  as `init --expo` and `init --web` exist), and what `exact snapback` and `exact backend`
  do today: the `appId` check, the grants, the origin and port, the development server,
  and the device's preflight against the server's floor (today `devicePreflight`), run
  before the package's own tests.

The code is copied from `snapback4/` to the Snapback repository (§5 step 2); LLP 1108 and
LLP 1110, which specify the adapter's media and live reads, go with it as research for
Snapback's own documents. What stays in exact2 of those two is what the runtime provides
to any module (`Answer::message`, `exactSaveTo`, `Blob`), already landed.

### D3 — The native boundary is one JSON call behind the package's shim

**The slot.** The shim implements `exact_js::NativeModule`, the data module's native
module (LLP 1027 D8): `call(request) -> Result<Json, String>`, answered synchronously on
the source's thread. The bake generates the `with_native` wiring from `exact.json` into
the app's `module.rs`, beside `APP` and `GRANTS`, on Apple and Linux (and so on Android,
whose crate is the same Rust: LLP 1107), so an app's crate writes nothing: Harmony's
`struct Snapback` and `fn snapback` go.

**Storage.** The host hands the module admitted storage, and the shim never reads grants:
`configure_storage` gives an `exact_js::NativeStorage` whose `sqlite(app_path)` admits
`sqlite.open <app_path>` under the app's grants (`app:/data/<name>.sqlite`, as today's
`Module::open` admits it through `ibex2`) and answers the real path, and whose roots are
the data, cache and temporary directories. What `Module::open` does today with `ibex2`
and `exact_runner` moves into the host behind that call; the shim names only `exact_js`,
`serde_json` and the standard library, which every app's crate has.

**The call.** The library exposes the module's call, as `exact_snapback4::Module::call` is
today (`open` with the partition's real path, origin, viewer and backend; `close`;
`engine_info`; `engine_headers`; the client's operations), over Snapback's C ABI
discipline: every answer is JSON the caller frees, one caller at a time, nothing unwinds
across the boundary. The shim is the only code that calls it; exact2 reads no header and
knows no symbol.

**Transport.** HTTP stays the driver's: each exchange the client needs is handed back to
TypeScript, which runs `fetch` under the app's grant, as today. The adapter's Rust
`DataSource` shape (`host_request`, `host_reply`, for a Rust-data app) has no consumer in
exact2 but the adapter's own test; it goes with the code, and whether Snapback keeps it is
Snapback's.

**The library.** A dynamic library per target, built by Snapback: an xcframework of
frameworks for Apple, a `.so` for Linux and Android. It carries its own Rust runtime and
SQLite privately and exports only its C symbols, so it never shares a link with Exact's
Rust: a static archive would put two Rust runtimes and two SQLites (Exact's `ibex2-sqlite`
bundles one; Snapback's `rusqlite` bundles one) into one binary. The build links it
through the shim's `#[link]` and places it as Exact's own Rust modules are placed: in the
app's frameworks on Apple, signed with them (`host/apple/build.mjs`: `ExactCore.framework`,
the GPU module's dylib, LLP 1009 D6), beside the binary on Linux, in the APK's libraries on
Android.

### D4 — The app chooses the version; the floor and the stamp decide compatibility

`package.json` in the backend's directory pins `snapback4` and `snapback4-exact` exactly
(a package with native code pins exactly: LLP 1133.001 D9), and `snapback4 upgrade`
moves both, as it moves `snapback4-expo` today. Compatibility is Snapback's contract
alone: the device above the server's `minimum_version` with the capabilities the schema
requires, checked by `snapback4 check` and by the package's own preflight before a test
run. exact2's build refuses nothing about versions.

### D5 — Harmony installs the package; the lane tests what Harmony installs

Harmony's `package.json` names `snapback4` and `snapback4-exact`; its crates depend on no
path into exact2 for Snapback, so its Cargo workspace needs no private source. Whether its
crates then rejoin the root workspace is LLP 1153 D2.4's question again, and its answer
stands: Harmony is an app, with an app's workspace. The lane's `harmony` step runs a
script under `apps/harmony/` that installs with `--frozen-lockfile` on every run, starts
the server that install provides in memory on a free port, and runs the tests; it builds
no device. `harmony-check` compiles Harmony's workspace as today. No step of the gate or
the lane touches Snapback otherwise.

### D6 — Targets the package does not cover are visible absences

`library` names the targets the package covers. A target it does not cover is LLP 1133
D8's visible absence: the build refuses by name unless the app marks the module optional.
Nothing in exact2 builds a Snapback device from source for a missing target; that is
Snapback's to publish (§4). Android is one: Harmony has no `android/` crate, so the first
list of §4 has five targets, and `aarch64-linux-android` follows when an app with an
Android crate needs it, through the same generated wiring.

## 3. What exact2 changes

**Builds** (approval: the request in Status; each is apparatus under `rules/RULES.md`):

1. **Fetch and link a package's library** named by `exact.json` `native.library` for the
   target being built: fetched by the step that fetches locked Cargo sources, into a cache
   keyed by the hash, refused on a mismatch (LLP 1133.001 D9); linked through the shim and
   placed in the app's bundle as D3 says, on Apple, Linux and Android.
2. **Generate the native slot's wiring and hand it admitted storage**: the bake writes the
   `with_native` factory from `native.rust` and `native.class` into `module.rs`
   (`js/bake/src/lib.rs` `module_constants`), and `exact_js::NativeModule::configure_storage`
   takes an `exact_js::NativeStorage` whose `sqlite(app_path)` admits under the app's grants
   (the admission `snapback4/src/lib.rs` does today, moved into `exact_js`). The trait's
   one consumer is the adapter, so this is a change, not a migration.
3. **A package's web asset**: a file the package declares is served under the app's
   assets and captured by the bake with the package (`host/web-js/build.mjs` copies the
   app's own assets today; package JavaScript capture does not carry arbitrary files).
4. **The proof package**: a fixture under `packages/fixtures/` (beside `tally`, which proves
   source composition, not a library), with a prebuilt dynamic library per target built
   from Rust in the fixture by its own script, published at a `file:` URL with its hash,
   and a shim on the native slot that opens a SQLite file through `NativeStorage`. Its test
   fetches the library by hash, refuses a wrong hash, refuses an ungranted path, and
   asserts a value only the library answers, under native binding and not the
   substitute, on iOS, macOS and Linux headless, from an app `exact new` makes. It is not
   Snapback: it proves the seam before Snapback publishes on it, and it stays as the
   seam's test.

**Deletes**, once `snapback4-exact` is published and the asks of §4 are met (§5 step 3),
in one change:

- `snapback4/` entire, and the root `Cargo.toml`'s `exclude` of it;
- `snapback4` from `package.json` and `bun.lock`;
- `scripts/retest-snapback-readme.mjs`, `scripts/check-snapback-readme.mjs`, its test,
  `scripts/snapback-handshake.test.mjs` and `snapback4/readme-tested.json`: Snapback's
  publish already proves each release from an installed consumer, and the package's own
  tests take the handshake's;
- `wireSnapback`, `buildSnapbackDevice`, `snapbackVersion` and `new --snapback` in
  `game/new.mjs` and `scripts/exact.mjs`; the `snapback` and `backend` verbs and their
  `scripts/app-cli.mjs` registrations and the `contract build` preflight hook there (LLP
  1086 D12's flag and verbs become `snapback4 init --exact` and the package's scripts);
- `snapback_glue` in the bake; `snapbackMutationTypes` and the driver-mount test in
  `js/bake/src/typescript.mjs`; the mount warnings in `host/web-js/build.mjs`;
  `devicePreflight`, `mountsSnapback` and the mount detection in `scripts/agent-test.mjs`
  (`backendHead` and `fresh` stay, detecting a server by its answers); the preflight test
  in `scripts/agent-launch.test.mjs`;
- `scripts/memory-backend-test.mjs` and `packages/background/test/harmony.mjs`'s device
  build: the lane's script becomes Harmony's (D5), and the background package's Harmony
  test uses it;
- Harmony's `struct Snapback` and `fn snapback` in its Apple and Linux crates, its
  `exact-snapback4` path dependencies and its `snapback4` mount;
- the Messages leftovers in `.gitignore` (`apps/messages/snapback/`,
  `assets/snapback4-device.wasm`, `snapback4-LICENSE.txt`, `.snapback/`);
- every Snapback version in `README.md`, `AGENTS.md`, `CLAUDE.md`, `docs/reference.md`,
  `docs/start-here.md`, the recipes and `scripts/agent-test.mjs`; the reference's
  Snapback paragraph becomes two sentences naming the package and D1.

**Keeps**: `fresh` and `backendHead`; the recipes `snapback`, `snapback-tests` and
`snapback-auth`, rewritten to point at the package's guide for wiring and to keep only
what is Exact's (the test file's `epoch now`, `fail fetch`, `clock data`, `fresh`);
Harmony.

## 4. Asks of Snapback

Snapback's own document decides these; they are what D2 needs of it, and step 3 of §5
waits for all five.

1. **Publish `snapback4-exact`** with each release, from the code `snapback4/` holds, with
   `exact.json`, the driver, the web wasm and its glue, the Rust shim and the library.
2. **Prebuilt dynamic libraries for Exact's targets**, as `build-xcframework.sh` builds the
   two iOS targets as static ones: first Harmony's five (`aarch64-apple-ios`,
   `aarch64-apple-ios-sim`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`,
   `aarch64-unknown-linux-gnu`), each carrying its Rust runtime and SQLite privately and
   exporting its C symbols only; Android, tvOS, watchOS and Windows when an app needs them
   (D6).
3. **The module's call over the C ABI**: `open` with a real path, origin, viewer and
   backend; `engine_info` and `engine_headers` answered through the call (today Rust
   functions); the client's dispatch compiled into the library beside the device. This is
   a wider ABI than the Expo module's four device functions, not a reuse of it.
4. **`snapback4 init --exact`**, taking `exact new --snapback`'s job, and `upgrade`
   moving `snapback4-exact` with `snapback4`.
5. **The guide** (`snapback4/README.md`), its retest and the device preflight, as the
   package's README, tests and scripts.

## 5. Transition

Three steps, in order; each repository is whole after each.

1. **exact2 builds the seam** (§3 Builds, items 1 to 4) and proves it with the fixture
   package. Nothing about Snapback changes: `snapback4/`, the pin and Harmony's paths stay
   as they are.
2. **Snapback copies the code and publishes `snapback4-exact`** for Harmony's five targets,
   meeting every ask of §4, against its current release. exact2's tree does not change.
3. **exact2 switches and deletes**: Harmony depends on the published package (D5); then
   every deletion of §3 in one change, with the docs. The Harmony lane passes on the web,
   iOS and Linux before the push.

No transitional shim, no second mount, no "legacy" path: step 3 is one change
(`rules/RULES.md`, "Delete; don't deprecate").

## 6. What proves it

| | Measure | Passes when |
|---|---|---|
| M1 | three greps outside `llp/` and `apps/`: `git grep -n 'expo/snapback' -- ':!llp' ':!apps'`; `git grep -n 'snapback4' -- '*/package.json' package.json bun.lock '*/Cargo.toml' Cargo.toml '*/Cargo.lock' Cargo.lock ':!apps'`; `git grep -n -E 'snapback4(-[a-z]+)?@|0\.4\.[0-9]+' -- ':!llp' ':!apps'` | all three empty |
| M2 | Harmony's tests on the web, iOS and Linux, from Harmony's own lock, on a machine with no access to Snapback's source | pass |
| M3 | bump Harmony's `snapback4` and `snapback4-exact` to the latest, `bun install --frozen-lockfile`, rebuild, run M2; `state` shows the new engine version; `git status` | M2 passes; the version moved; changes only under `apps/harmony` |
| M4 | the six checks and the async lane on a machine with no Snapback installed and no access to its source | green |
| M5 | the fixture package's test (§3 item 4) on iOS, macOS and Linux headless | the hash check refuses, the grant check refuses, the native answer is the library's |
| M6 | a partition Harmony's current adapter wrote, with a queued write, reopened under the package | the identity is kept and the write reaches the server |

M3 is the sentence of the title made checkable: Snapback moved, exact2 did not.

## 7. Alternatives rejected

- **Repin on each release** (today). Six repins in two weeks and seven releases behind;
  each repin edits tests and docs; apps wait for exact2.
- **Track Snapback's main branch.** The lock still names one commit, and exact2 stays
  compiled against Snapback's Rust internals, so Snapback cannot refactor without
  breaking exact2. Independence runs both ways or not at all.
- **A path dependency on a sibling Snapback checkout.** The "mutable sibling checkout"
  `snapback4/Cargo.toml` refused: builds then differ by machine.
- **Snapback's crates on a registry.** crates.io exposes private source; a private
  registry is the second package system LLP 1091 §6 rejected, and exact2 would still
  compile Snapback internals.
- **Keep the adapter in exact2 as `@exact/snapback4` over Snapback's published device.**
  The coupling moves from the Rust API to the HTTP wire: every wire change is still an
  exact2 commit, and exact2 still needs a Snapback server to test it. The client belongs
  with the wire (§1, "the direction of ownership").
- **A generic C ABI module seam with symbols exact2 names.** A second module contract
  beside the native slot; the shim is a few dozen lines the package owns.
- **The shim on LLP 1133.001's `Package` trait** (r1). No roots, no grants, no synchronous
  call; the driver uses the native slot.
- **Swift and Kotlin bridges per host** (Sol). The driver's seam is the data module's
  native slot, which is Rust on every host; a Swift bridge would need a new seam from
  Swift into the data module on each host, for one package.
- **The shim as a `DataSource` through a generated Cargo manifest naming `exact-runner`
  and `ibex2`** (Grok). LLP 1133.001 D9 refuses a module naming `exact-*` crates, and
  grants would then be parsed outside Exact. The admitted-storage handle keeps them in.
- **A static archive.** Two Rust runtimes and two SQLites in one link (D3).

## 8. Open questions

1. **Where a package's web asset is served** (§3 item 3): under `/assets/<package>/…` or
   a path the package declares. The implementer's call; the driver reads it from the
   package, never from a constant.
2. **One native slot per app.** The data module has one native module; an app with
   Snapback and another `native.call` module has the same limit today. Not changed here;
   LLP 1133.001's modules are separate from the slot.
3. **The glue's build.** The wasm-bindgen glue is committed in the package by Snapback at
   the version its wasm was built with; whether the bake re-checks the pair is the
   implementer's call with §3 item 3.
