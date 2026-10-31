# LLP 1154: exact2 and Snapback move independently

**Type:** RFC
**Status:** Draft r1, 2026-10-31. Written at Charlie's request ("we sort of ended up pinning a specific snapback4 version to exact2. i think that was a mistake … the two projects should be able to move independent of each other"); that request is the approval `rules/RULES.md` asks for the build changes of §3 and for the deletions. Nothing built.
**Systems:** the Snapback adapter (`snapback4/`: `client/`, `src/`, `web/`, `ts/`, `online/`, `web/build.mjs`, `web/device-build.mjs`, `tests/`); the pins (`snapback4/Cargo.toml`, `snapback4/Cargo.lock`, `package.json`, `bun.lock`, `snapback4/readme-tested.json`); the retest apparatus (`scripts/retest-snapback-readme.mjs`, `scripts/check-snapback-readme.mjs`, `scripts/check-snapback-readme.test.mjs`, `scripts/snapback-handshake.test.mjs`); scaffolding (`game/new.mjs`: `wireSnapback`, `buildSnapbackDevice`, `snapbackVersion`; `scripts/exact.mjs`: the `snapback` verb and `new --snapback`); the bake (`js/bake/src/lib.rs` `snapback_glue`, `js/bake/src/typescript.mjs` `snapbackMutationTypes`); the JS target's build (`host/web-js/build.mjs`); the test driver (`scripts/agent-test.mjs`: `devicePreflight`, `fresh`); the module seam (LLP 1133.001 as built: `contract/cli/src/native/packages.rs`, `exact_linux::package::Package`, `artifacts`); Harmony (`apps/harmony/app/`, `apps/harmony/package.json`); the async lane (`scripts/async.mjs`: `harmony`, `harmony-check`); the docs (`README.md`, `AGENTS.md`, `docs/reference.md`, `docs/start-here.md`, `docs/recipes/snapback*.md`)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-31
**Implementer:** a coding lane (codex GPT-6.1 Sol, or Opus 5.5) from 2026-11-01 for §3 and §5, reviewed blind by Astra and Grok before each push; the Snapback side (§4) is Snapback's own document to adopt.
**Related:** LLP 1036.001 D4 (the private Snapback pin leaves the root workspace, ruled 2026-09-30: it moved the pin, which this document removes); LLP 1086 D12 (`exact new --snapback` and the `snapback` verb, which §3 moves to Snapback's CLI); LLP 1153 D2.4 (Harmony's own workspace, kept out of the root lock because of the pin); LLP 1133.001 (modules anyone can write: D1 the package kinds, D4 the app opts in, D9 distribution and prebuilt libraries, as built for Linux and Windows on 2026-10-26); LLP 1133 D8 (every target it names, or a visible absence); LLP 1091 D9 (packages through `node_modules`); LLP 1108 and LLP 1110 (what the adapter does; they move with it); Snapback's LLP 3000 (2026-09-13: one interpreter hosted two ways, "the native module on hosts where SQLite is native and bytes are a library — Expo, and Exact2's Apple and Linux hosts") and LLP 3000.001.000 (the build stamp and the compatibility floor)

## Summary

exact2 compiles Snapback's private Rust source, so it must name one Snapback commit, and
it names the matching npm version in four more places. Every Snapback release therefore
needs an exact2 commit, an app cannot run a Snapback newer than exact2's pin, and Snapback
cannot change its Rust internals without breaking exact2. Six repins in two weeks still
left exact2 eight releases behind (§1).

The decision: **exact2 names no Snapback version, carries no Snapback source and installs
no Snapback binary** (D1). Snapback on Exact is a module package Snapback publishes, on
the seam LLP 1133.001 already defines for anyone's module: the TypeScript driver, the
web's wasm and a prebuilt native library per target, versioned with `snapback4` as
`snapback4-expo` is (D2). The native boundary is the one JSON call Snapback declares for
"Expo and Exact2 hosts", behind the package's own Rust shim; exact2 links a library and
never sees a C header (D3). The app chooses the version in its `package.json`, and the
server's floor and the device's stamp decide compatibility, as they do today (D4). What
exact2 builds is the two items the seam lacks: linking a package's prebuilt library on
Apple and Android, and composing a package's Rust shim into the Apple crate as Linux
already does (§3). What it deletes is `snapback4/`, the pins, the retest apparatus and the
Snapback verbs (§3). The move is three steps in two repositories, none leaving exact2
half-changed (§5). Harmony is the proof: it builds and passes on the web, iOS and Linux
from its own lock, and a Snapback upgrade touches nothing outside `apps/harmony` (§6).

## 1. Why

**What is pinned, and where.** `snapback4/Cargo.toml` names three crates from
`github.com/expo/snapback` at one revision with an exact version; the crates are
`publish = false`, so no registry carries them. The root `package.json` installs
`snapback4@0.4.33` so tests have a server binary, and Harmony's does the same. The README,
`AGENTS.md`, `docs/reference.md`, the recipes and two test scripts say the number;
`snapback4/readme-tested.json` records the version, the revision and the hash of twenty
files, and `check-snapback-readme.mjs` refuses a change to any of them until
`retest-snapback-readme.mjs` runs the README's four apps against a registry and rewrites
the pins.

**How it has gone.** The pin moved on 2026-10-07 (0.4.13), 10-08 (0.4.16), 10-13
(0.4.25), 10-15 (0.4.28), 10-16 (0.4.29) and 10-20 (0.4.33). Snapback publishes most days:
0.4.35, 0.4.37, 0.4.38, 0.4.39, 0.4.40, 0.4.43 and 0.4.49 followed by 2026-10-29, and its
main branch is at 0.4.50. Each repin changed tests and docs with it: 0.4.33 closed a
password-only app's development doors unless started with `--unsafe-dev-doors`, and
refused personas a schema does not declare, so `fresh`, the README's password app, the
retest and two fixture schemas changed in the same commit (`b5c9552473`).

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
Only `partition::bind` (the `meta` and `set_meta` operations, which the C ABI's call
already answers) and the request headers (`engine_headers`, which the published wasm
exports) reach deeper. The web wasm `exact-snapback4-web` re-implements what Snapback's
published `WebDevice` exposes: construct with the kept changes, `call`, `drain`,
`engine_info`, `engine_headers`.

**Skew is already designed in.** Every server answers `device_compatibility`
(`server_version`, `minimum_version`, `capabilities`); the floor at 0.4.33 is 0.4.24. The
device carries a `snapback4.engine` stamp, and both `snapback4 check`
(`exact_device.rs`: "mounted Exact apps carry their actual wasm engine stamp, not the npm
pin") and exact2's `devicePreflight` compare the two. A device and a server need not be the
same version; they need the device above the floor with the capabilities the schema
requires. Nothing in that contract needs exact2 to know a version.

**The direction of ownership.** The adapter's Rust client speaks Snapback's HTTP wire;
when the wire moves, the client moves. The party that changes a wire owns the clients
that speak it and ships them under one version: that is how Snapback ships its TypeScript
client and its Expo module, and how `snapback4 check` already knows an Exact app's
`app.json` and device stamp. Exact is the platform; Snapback is a module on it, as any
`@exact/*` package or anyone's module is (LLP 1133.001). The coupling runs one way:
Snapback tracks Exact's module seam, which exact2 keeps stable for every module.

## 2. Design

### D1 — exact2 names no Snapback version, carries no Snapback source, installs no Snapback binary

In the exact2 repository, outside `apps/`:

- no dependency on `github.com/expo/snapback`, in any `Cargo.toml` or lock;
- no `snapback4` or `snapback4-*` entry in `package.json` or `bun.lock`;
- no Snapback version number in prose, code, tests or fixtures;
- no Snapback server started by a check or by the async lane except through an app under
  `apps/` that installs its own (D5).

What exact2 may know of Snapback is its documented wire, as a consumer: the test
operation `fresh` empties a development server through `POST /dev/reset` and names the
flag that opens the doors (no version, no "install snapback4@…"), and nothing else in
`scripts/` or the hosts is Snapback's.

An app under `apps/` is an app: its `package.json` names the Snapback it uses, exactly,
as any app outside the repository does. Harmony's does today.

### D2 — Snapback on Exact is a module package Snapback publishes

`snapback4-exact`, published with each `snapback4` release as `snapback4-expo` is, an
implementation package in LLP 1133.001 D1's sense, holding:

- **the TypeScript driver** (today `snapback4/ts` and `snapback4/online`): what an app's
  `app.ts` imports as `import { Snapback } from 'snapback4-exact'`, baked as LLP 1027 D5
  bakes any npm package; the `typescript.sources` mount `snapback4` (a relative path into
  an exact2 checkout) goes away;
- **the web device**: the wasm the page loads (today `exact-snapback4-web`: the device
  over the memory store, the client, `drain` into Exact SQLite), served as an asset of the
  app from the package, so the page loads it as it loads `assets/snapback4.wasm` today
  and the bake captures it with the package's bytes;
- **the native module**: the Rust shim (today `exact-snapback4`'s `Module`: the app's
  grants and storage roots, one partition, the client's dispatch) as `sources.rust`, and
  the device and client compiled by Snapback as a prebuilt static library per target in
  `artifacts` (D9: `{target, url, sha256}`), behind Snapback's C ABI;
- `exact.json`: `kind: "implementation"`, its targets, its names and their substitutes,
  so an agent drives an app on any target without the library (LLP 1133.001 D2);
- the guide (today `snapback4/README.md`), the Exact scaffold (`snapback4 init --exact`,
  as `init --expo` and `init --web` exist) and what `exact snapback` does today: the
  `appId` check, the grants, the origin and port, the device's preflight against the
  server's floor.

The code moves from `snapback4/` to the Snapback repository with its history; LLP 1108
and LLP 1110, which specify the adapter's media and live reads, move with it as research
for Snapback's own documents. What stays in exact2 of those two is what the runtime
provides to any module (`Answer::message`, `exactSaveTo`, `Blob`), already landed.

### D3 — The native boundary is one JSON call behind the package's shim

The library the package ships exposes the module's call, as `exact_snapback4::Module::call`
is today (`open` with the partition path, origin, viewer and backend; `close`;
`engine_info`; `engine_headers`; the client's operations), over Snapback's C ABI
discipline: every answer is JSON the caller frees, one caller at a time, nothing unwinds
across the boundary. The shim in `sources.rust` is the only code that calls it; it
implements the host's package trait and answers Exact's data seam (`Answer::Later` hands
each HTTP exchange to the host, as today). exact2 links the library the manifest names
for the target and composes the shim; it reads no header and knows no symbol.

Rejected: a generic "C ABI JSON module" seam in exact2, with symbols exact2 names. It
would be a second module contract beside LLP 1133.001's, and the shim is a few dozen lines
the package owns.

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
stands: Harmony is an app, with an app's workspace. The async lane's `harmony` step
installs from Harmony's lock and runs its tests against the server that install provides;
`harmony-check` compiles its workspace as today. No step of the gate or the lane touches
Snapback otherwise.

### D6 — Targets Snapback has not published are visible absences

The package names the targets its `artifacts` cover. A target it does not cover is LLP
1133 D8's visible absence: the build refuses by name unless the app marks the module
optional. Nothing in exact2 builds a Snapback device from source for a missing target;
that is Snapback's to publish (§4).

## 3. What exact2 changes

**Builds** (approval: the request in Status):

1. **Link a package's prebuilt library** named by `artifacts` for the target being built:
   Apple (the static library into the app's Rust module, as the host's own modules link),
   Linux and Android (into the app's crate). D9 specifies the fetch and the hash check;
   nothing fetches it yet, so this is the first package that needs it, and it lands here.
2. **Compose a package's Rust shim into the Apple crate** as the Linux and Windows build
   does (`contract/cli/src/native/packages.rs`, 2026-10-26): the shim joins the app's
   Apple crate as a `#[path]` module and implements the same package trait, which the
   Apple host then serves as `exact_linux::Composed` does. A shim names only the crates
   the app's crate has, and `serde_json` through them.
3. **A package's web asset**: a file the package declares is served under the app's
   assets and captured by the bake with the package.
4. **The proof package**: a fixture under `packages/fixtures/` (beside `tally`), with a
   tiny prebuilt library per target built from Rust in the fixture, answering a call on
   iOS, macOS and Linux headless from an app `exact new` makes. It is not Snapback: it
   proves the seam before Snapback publishes on it, and it stays as the seam's test.

**Deletes**, once `snapback4-exact` is published for Harmony's targets (§5 step 3):

- `snapback4/` entire, and the root `Cargo.toml`'s `exclude` of it;
- `snapback4` from `package.json` and `bun.lock`;
- `scripts/retest-snapback-readme.mjs`, `scripts/check-snapback-readme.mjs`, its test,
  `scripts/snapback-handshake.test.mjs` and `snapback4/readme-tested.json`: Snapback's
  publish already proves each release from an installed consumer, and the package's own
  tests take the handshake's;
- `wireSnapback`, `buildSnapbackDevice`, `snapbackVersion` and `new --snapback` in
  `game/new.mjs` and `scripts/exact.mjs` (the `snapback` verb with them: LLP 1086 D12's
  flag and verb become `snapback4 init --exact` and the package's scripts);
- `snapback_glue` in the bake, `snapbackMutationTypes` and the driver-mount test in
  `js/bake/src/typescript.mjs`, the mount warnings in `host/web-js/build.mjs`,
  `devicePreflight` and `mountsSnapback` in `scripts/agent-test.mjs`;
- the Messages leftovers in `.gitignore` (`apps/messages/snapback/`,
  `assets/snapback4-device.wasm`, `snapback4-LICENSE.txt`, `.snapback/`);
- every Snapback version number in `README.md`, `AGENTS.md`, `docs/reference.md`,
  `docs/start-here.md`, the recipes and `scripts/agent-test.mjs`; the reference's
  Snapback paragraph becomes two sentences naming the package and D1.

**Keeps**: `fresh` (a documented wire, no version); the recipes `snapback`,
`snapback-tests` and `snapback-auth`, rewritten to point at the package's guide for
wiring and to keep only what is Exact's (the test file's `epoch now`, `fail fetch`,
`clock data`, `fresh`); Harmony.

## 4. Asks of Snapback

Snapback's own document decides these; they are what D2 needs of it.

1. **Publish `snapback4-exact`** with each release, from the code `snapback4/` holds, with
   `exact.json`, the driver, the web wasm, the Rust shim and the prebuilt library.
2. **Prebuilt libraries for Exact's targets**, as `build-xcframework.sh` builds the two
   iOS targets: first Harmony's (`aarch64-apple-ios`, `aarch64-apple-ios-sim`,
   `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
   `aarch64-linux-android`); tvOS, watchOS and Windows when an app needs them (D6).
3. **The module's call over the C ABI**: `engine_info` and `engine_headers` answered
   through the call (today Rust functions), and the client's dispatch compiled into the
   library beside the device.
4. **`snapback4 init --exact`**, taking `exact new --snapback`'s job, and `upgrade`
   moving `snapback4-exact` with `snapback4`.
5. **The guide** (`snapback4/README.md`) and the retest it describes, as the package's
   README and tests.

## 5. Transition

Three steps, in order; each repository is whole after each.

1. **exact2 builds the seam** (§3 Builds, items 1 to 4) and proves it with the fixture
   package. Nothing about Snapback changes; `snapback4/` and the pin stay as they are.
2. **Snapback publishes `snapback4-exact`** for Harmony's six targets (§4 items 1 to 3),
   from the moved code, against its current release.
3. **exact2 switches and deletes**: Harmony depends on the published package (D5); then
   every deletion of §3 in one change, with the docs. The Harmony lane passes on the web,
   iOS and Linux before the push.

No shim, no transitional mount, no "legacy" path: step 3 is one change (`rules/RULES.md`,
"Delete; don't deprecate").

## 6. What proves it

| | Measure | Passes when |
|---|---|---|
| M1 | `git grep -n 'expo/snapback\|snapback4@\|"snapback4"'` over the repository outside `apps/` | empty |
| M2 | Harmony's tests on the web, iOS and Linux, from Harmony's own lock | pass, with no Snapback source fetched |
| M3 | bump Harmony's `snapback4` and `snapback4-exact` to the latest; `git status` | changes only under `apps/harmony` |
| M4 | the six checks and the async lane on a machine with no Snapback installed and no access to its source | green |
| M5 | the fixture package's call on iOS, macOS and Linux headless (`packages/fixtures/<name>/test/`) | answers |

M3 is the sentence of the title made checkable: Snapback moved, exact2 did not.

## 7. Alternatives rejected

- **Repin on each release** (today). Six repins in two weeks and eight releases behind;
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
- **A generic C ABI module seam in exact2** (D3's rejection).

## 8. Open questions

1. **Which package trait the Apple shim implements** (§3 item 2): `exact_linux::package::
   Package` is Linux's; the Apple host's equivalent is what the Apple build composes.
   Decided by the implementer with the fixture package, and recorded here.
2. **Where a package's web asset is served** (§3 item 3): under `/assets/<package>/…` or
   a path the package declares. The implementer's call; the driver reads it from the
   package, not from a constant.
3. **Harmony on Android**: LLP 1107 builds Android from the Linux crate, so
   `aarch64-linux-android` is in §4's first six; if Harmony has no Android build at step 3,
   the target may follow.
