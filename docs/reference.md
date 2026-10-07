# Exact developer reference

Operational detail for working in this repository: toolchains, the Contract
CLI, the native and web loops, TypeScript and Rust data modules, delivery,
and the stress examples. The overview and quick start are in the [README](../README.md).

Tooling runs on **Bun 1.4.2**, the version `package.json` pins.
Run `bun install --frozen-lockfile` to install the dependencies in `bun.lock`.
The web artifacts build with a pinned nightly Rust and its std sources (LLP 1047);
everything else uses `rust-toolchain.toml`'s stable. `host/web/build.mjs` prints the
install commands when they're missing:
`rustup toolchain install nightly-2026-08-21 --profile minimal --component rust-src`,
then `cargo +nightly-2026-08-21 fetch` on that toolchain's `library/Cargo.toml`.
Node and npm are not required. Rolldown remains the app bundler; the existing
build, serve, watch, and reload scripts run under Bun. Run tooling unit tests with
`bun test ./scripts/`; the explicit path keeps Bun from searching generated
fixture checkouts. This is the source
tooling installation; a standalone CLI distribution is not packaged yet.

An app with TypeScript sources (`app.ts`) is baked with the `hermesc` paired
with Exact's linked lean Hermes. Install Ibex's pinned, attested bundle once:

```sh
cargo run --manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml -- --target <triple>
```

The verified install lives under `~/.cargo/hermes-lean-sys/`. Exact sets
`HERMES_LEAN_SYS_OFFLINE=1` in `.cargo/config.toml`, so an ordinary build
neither downloads a bundle nor compiles Hermes; a missing install fails with
the one-time command. `HERMES_LEAN_SYS_DIR` remains a development override.
The v4 set supports macOS, Linux, Windows, iOS devices and the universal iOS
Simulator, plus tvOS devices and the arm64 tvOS Simulator. Exact selects
Ibex's English `intl` tier on Linux and Windows, not `intl-all-locales`; Apple
keeps Hermes's OS-backed Intl. A Rust-only app needs no engine at run time.
`exact setup` installs the host bundle and, on macOS, the iOS and tvOS Simulator
bundles. `exact setup --check` runs Ibex's own offline resolver validation over that
same set. A signed iOS device build names its separate one-time target command.

### Windows TypeScript

The former private `260318099.0.0` build is superseded. Native Windows
TypeScript uses Ibex's v4 debugger-off lean bundle. `exact-js` selects the
`intl` feature and installs `GROUP_INTL`; Ibex binds the Windows 10 2004+ OS
`icu.dll` from System32 through function pointers, with no ICU linker flags or
bundled locale data. The ordinary app path still requires the Windows
qualification in [LLP 1027.006](../llp/1027.006-windows-native-typescript.plan.md)
before release claims. Do not
resurrect Exact's deleted private source builder or use its old
`%LOCALAPPDATA%/Exact/hermes` cache for this snapshot. `EXACT_JS_ENGINE=stub` remains the Hermes-free,
refusing build for CI; it cannot bake a working TypeScript app.

Windows application storage uses the current user's LocalAppData known folder,
under `exact/<app-id>/{data,cache,temporary}` (`app:/tmp` uses `temporary`). It does
not require `HOME`. App and scratch
identities and `app:/` path components must be safe Windows leaves; drive, UNC,
backslash traversal, alternate-stream and reserved-device forms are refused.

Snapback4 consumers use release **0.2.30**: the CLI and browser device are pinned
in `bun.lock`; Cargo pins native devices and schema compilers to the matching
release source commit `a397218e2332964ebe29aa1d30918c436713cc8a`.
Run `bun install --frozen-lockfile` before baking Messages Legacy, and use the pinned CLI
with `bun run --bun snapback4` from an app directory.
Messages Legacy and the optional `exact-snapback4` adapter belong to the separate
`snapback4/` Cargo workspace. Its lock carries the private source; root Cargo
commands need no Snapback access. The `messages-legacy` build commands select
that workspace automatically; direct Cargo commands use
`--manifest-path snapback4/Cargo.toml`. External consumers keep their path
dependency on `snapback4/`.

The canonical [Messages](../apps/messages/README.md) app is the Exact port of Expo's
chat demo, with model conversations through a local OpenRouter service. It belongs
to the root workspace and does not use Snapback. Run `bun apps/messages/service.ts`
and `bun host/web/dev.mjs --app messages --port 8767`.


A cross-platform application runtime. The Rust kernel computes layout, each platform
renders natively, and Contract is the authoring model.

Surfaces: web, macOS, iOS, Linux.

Start here:

- **`rules/RULES.md`** — how work happens here. One page. Read it before your first PR.
- **`rules/DEFERRED.md`** — what v1 deliberately excludes, and why.
- **`llp/1000-exact2-root.explainer.md`** — the map: what exists, what is next, how the
  design corpus is laid out.

The predecessor repo is research, not authority: cite it for how something worked,
never to block. Its design documents are imported under `llp/research/`.

## What exists

| Crate | What it is | Spec |
|---|---|---|
| `kernel/` (`exact-kernel`) | Typed columnar arena, EXWF wire frames, validate-then-apply transactions, Taffy layout with changed-geometry receipts, EXNODE columnar export, injected text measurement. Builds for `wasm32-unknown-unknown`. | `llp/1001-kernel-v1.spec.md` |
| `motion/` (`exact-motion`) | CSS `transition` semantics over `translate`/`scale`/`rotate`/`opacity`, one spring, a seekable clock. The web executes it as CSS; everywhere else this crate does. | LLP 1002 (decision), LLP 1003 (spec) |
| `plan/` (`exact-plan`) | The plan format: tables, bytecode, and the validating decoder, generated from one JSON authority. Depends on nothing. | LLP 1005 |
| `runner/` (`exact-runner`) | The plan runner: VM, keyed instances, kernel ops, events, timers under a seekable clock, the data seam. | LLP 1005 |
| `contract/` | The Contract compiler in Rust: `syntax` → `types` → `analyze` → `lower`, the `contract` driver and CLI, and the corpus. | LLP 1004 (decision), LLP 1006 (spec) |
| `apps/fieldnotes/` | Offline notes: Contract UI, TypeScript sources, SQLite persistence, and JSON file backup/restore. See its [README](../apps/fieldnotes/README.md). | LLP 1027 |
| `apps/markdown/` | A Markdown reader: the parser and block model both readers share, and the general one-file app. `mdview`. See its [README](../apps/markdown/README.md). | LLP 1033 |
| `apps/llp/` | The same reader specialised for an LLP corpus: numbered index with sub-documents, search over every document's text, section outline, `LLP 1234` as a link. `llpview`. See its [README](../apps/llp/README.md). | LLP 1033 |
| `apps/caltrain/` | The v1 app: `app.contract`, its Rust data crate, and its wasm crate; the end-to-end fixture. | — |
| `apps/exact-live/` | A creative production workspace combining photo zoom, scene ordering, crew chat and a runbook. The [browser preview](https://exact-live.tuft.host/) passes 19 interaction checks; native delivery and connected jobs remain in progress. See its [README](../apps/exact-live/README.md). | LLP 1041 §8 |
| `gpu/` (`exact-gpu`) | The GPU canvas: a `Surface` trait against wgpu, a per-app module loaded on demand (a dylib on macOS, a second wasm on the web) after the first pixel; the same Rust renders on Metal and on the browser's WebGPU. `apps/caltrain/gpu` is the line map and the aurora. `gpu/reflect` (`exact-gpu-reflect`, naga only) reflects every `.wgsl` in a GPU crate's `build.rs`: bindings, struct layouts, vertex inputs, and entry points generated as Rust, the WGSL as the one declaration authority. | LLP 1009 |
| `host/apple/` (`exact-apple`) | The Apple host: runner + kernel as a static library with a C ABI, the kernel's layout with CoreText measurement through a callback, `exact-motion` as the executor, typed batches; `macos/` is the AppKit presenter and `ios/` the UIKit one (SwiftPM, sharing `swift/`). `bun host/apple/build.mjs --run`; `bun host/apple/build.mjs --ios --run` on a simulator. | LLP 1008 |
| `host/linux/` (`exact-linux`) | The Linux host, the first that paints: runner + kernel natively, cosmic-text measuring and painting from one cache, `exact-motion` as the executor, the kernel tree drawn by one walk over a backend — vello on the GPU (the main one), tiny-skia on the CPU (the fallback and the pixel oracle) — onto DRM/KMS dumb buffers with evdev input, or into a buffer with no display (the agent API, screenshots, the smoke; on macOS too). Pure Rust, no system library. `cargo build --profile host-dev -p caltrain-linux` for the one an agent drives (a touched line rebuilds in seconds); `--release` for the one that ships. | LLP 1015 |
| `host/web/` (`exact-web`) | The web host: runner + kernel in wasm over the real DOM, CSS computed once from the kernel's rows, springs lowered to frames the browser plays, a no-`unsafe` ABI, ~150 lines of glue, a headless-Chrome smoke, the motion parity harness, and the dev loop (`bun host/web/dev.mjs`, edit → present ~20 ms). | LLP 1007 |
| `vendor/taffy/` | Taffy 0.9.2 plus two Exact patches. | `vendor/taffy/EXACT-PATCHES.md` |

All four surfaces run the app; `QUEUE.md` is the ordered list of what would
make sense to do next.

## Serve RealWorld

RealWorld's public pages can be served by the native renderer:

```sh
EXACT_WEB_DIST=target/realworld-dist bun host/web/build.mjs realworld-web
cargo run --release -p realworld-web --bin realworld-render -- --serve target/realworld-dist --port 8080 --name Conduit
```

The home feed, articles and profiles arrive as anonymous HTML. Their public
responses are cached for 60 seconds by default (`--lifetime` changes this).
Links work without the runtime, which loads when the page is idle; a press
made before it is ready is replayed once it is. Login and editing routes
start the client normally. The renderer compresses what it sends (brotli, else
gzip, by `Accept-Encoding`), and SIGTERM drains it. Reading-page transfer and
the later runtime download are separate costs. With `--generations <dir>` it
keeps the builds of `app.wasm` it has served, and a browser holding an earlier
one gets the new build as a delta against it (LLP 1047.000 §9): after a
one-line app change, 9.5 KB instead of 238.6.

## Inspect and format Contract

```sh
cargo run -q -p contract -- build apps/messages/app.contract --json
cargo run -q -p contract -- build apps/messages/app.contract -o /tmp/messages.plan --map
cargo run -q -p contract -- symbols apps/messages/app.contract
cargo run -q -p contract -- symbols apps/messages/app.contract --name select
cargo run -q -p contract -- fmt --stdout apps/messages/app.contract
cargo run -q -p contract -- fmt --check apps/messages/app.contract
```

`symbols` prints JSON with `definitions` and `references`. Each reference's
`to` is an index into `definitions`; locations include the original file,
1-based line and byte column, and an exclusive `end_col`. Component interfaces,
local bindings, parameters, typed shape fields, font families and literal IDs are navigable.
Shared shape/function/style/font files can be queried directly. The query uses the
compiler's import and type rules and writes no files. Repeated literal IDs have
an edge to each matching declaration; dynamic IDs have no static target.
An action's definition carries `writes`: the states and mutations its body
assigns or sends, inferred (an action declares none), in slot order: router
state, states, then mutations, each as declared.

`--name <exact-name>` returns all matching definitions across scopes and their
references, with `to` indices into that response's smaller `definitions` array.
Names are exact and case-sensitive, not patterns; no match returns empty arrays.
The complete source graph is still checked. `symbols --help` shows the syntax.

`build --json` writes one diagnostics array to stdout: `[]` on success, or a
stable `id`, `message`, original `file`, `line`, `col`, `end_col`, and `related`
array per refusal. Compilation returns the first refusal. Action-interface
mismatches link the invocation, declaration and caller binding, each with its
own source file, including forwarded props and injected actions. Locations use the same byte columns as symbols; zero
means no source range, and a null file means no file is associated. No prose is
mixed into JSON, including argument and output-write failures. Exit codes are
0 for success, 1 for compilation/I/O failure, and 2 for invalid arguments.
`-o <file.plan>` writes the same plan bytes in either output mode.

`build --map -o <file.plan>` also writes `<file.plan>.map.json`, keyed by
SHA-256 of the plan bytes. Each plan node has its original file and range,
component call-site chain, and the winning style row's origin (`own`,
`class:<Name>`, or `tag`). Slots, derives and actions retain their declarations,
including state and actions lifted from child components. Maps are separate
files; ordinary compilation collects neither instantiation provenance nor lowering sites. The compiler API's
`compile_path_mapped` and `compile_path_source_mapped` return the map alongside
the plan; `SourceMap::bake_error` resolves measured layout refusals, and
`SourceMap::json` takes the final encoded bytes after baking. A consumer must
verify the map's digest against the plan actually accepted by its session.
The resident Contract, TypeScript and portable Rust producers emit maps after
baking. Temporary source captures retain the original app filenames. The dev
server keeps the matching map at the generation's `app.plan.map.json` URL,
declared under `dev.sourceMap`; it is never an asset or module payload. Static
builds and production publication omit it. `agent.mjs ... "layout <target>"`
reads the map beside `--plan` or from the development `--url`/`EXACT_DEV_PLAN`
envelope. It shows the declaration, component callers and winning authored style
origins only when the same node reply carries the matching plan digest. The
runner computes that digest lazily once per accepted plan; ordinary inspection
does no hashing. Missing, invalid or stale maps leave geometry available with a
source-unavailable explanation. The driver retains four recent map digests for
sessions that keep an older plan after a refused reload. A fresh driver may lack
that older map and refuses the join. “Compatible source map” means the compiled
plan matches: formatting-only edits can change source locations without changing
the plan, so this is not an original-source revision guarantee.

`fmt --stdout` previews source-preserving formatting; `--check` prints a diff
and exits nonzero when formatting differs. Plain `fmt <file>` writes the result
explicitly. Formatting never runs automatically on save.

## Run an app from a terminal on macOS

```sh
bun scripts/exact.mjs list                       # the apps here, and their commands
bun scripts/exact.mjs run markdown README.md     # build and launch, log on this terminal
bun scripts/exact.mjs install markdown           # ~/Applications/Markdown.app + `mdview`
mdview README.md                                  # from anywhere, reusing a running copy
```

`bun link` (or a symlink into a directory on `PATH`) makes it plain `exact`.
Both verbs launch the executable inside `<Name>.app`, so the process has the
app's bundle identity: its name in the menu bar, its Dock tile, and the
document types Finder's Open With reads. `install` also writes a shim named by
the manifest's `app.command` into the first of `~/.local/bin`, `/usr/local/bin`,
`~/bin` that is already on `PATH` (`EXACT_BIN_DIR` overrides), and prints the
line to add when none is.

An app says what it opens with `file_handlers` in `app.json` — the W3C Web App
Manifest's own key — and the macOS bake derives `CFBundleDocumentTypes` from
it. Each MIME type it accepts must be one the Apple hosts map to a system type
(`DOCUMENT_UTIS` in `scripts/app.mjs`; `application/octet-stream` is
`public.data`), which the app views (`Viewer`, rank `Alternate`), or the app's
own format: a vendor or unregistered type (`application/vnd.studio.board+json`,
`application/x-studio-board+json`) with its extensions (`[".board"]`), which the
bake exports as `<app id>.<subtype>` (`UTExportedTypeDeclarations`, conforming to
JSON for `+json`, XML for `+xml`, else data) with the app as its `Editor` and
`Owner` (studio diary R13). Every build refuses any other type when it reads the
manifest, the web's included. An extension another app already owns on a Mac
(Freeform has `.board`) can still resolve to that app's type there; the open
panels and drops take a declared extension whichever type the Mac gives it.
A file chosen in the app's own picker (`showOpenFilePicker`,
`showSaveFilePicker`) joins File ▸ Open Recent — a save once it is written —
and becomes the window's document, as a routed one does. A path from the command line, from Finder, from ⌘O, or from a link inside
a document all arrive at the same place: the app's `open-file` node
(LLP 1033 D3). One handed over at launch arrives before first pixel, before
app storage is ready and before a TypeScript data module has loaded: a send its
`change` makes waits for both and then runs, pending meanwhile, rather than being
refused (studio diary R14; notes diary). When nothing takes the path, the
host says why — no `open-file` field, or the `change` the app refused, and the
refusal — on stderr and in the journal. `exact uninstall <app>` takes both halves away.

Apple products live under the resolved app's target directory, scoped by
canonical source directory, manifest id, destination, composition and trust
policy. The Swift host is compiled once per destination for every app, in
`<target>/apple-swift/<destination>-<minimum OS>`; each app only links there,
one at a time, and its executable is copied to its own products before the next
app links. A development build compiles it file by file and incrementally, and
beside the app's Rust; the host's two Rust modules build in
`<target>/apple-modules`, and a checkout that has not built one takes it from
`~/.cache/exact/apple-modules` when another checkout of this machine compiled
it from the same bytes. A target directory that has compiled nothing starts
with the registry crates this machine has compiled
(`~/.cache/exact/apple-crates`; Cargo decides which it can use). Delete either
directory to compile everything here. What is
distributed (`--archive`, `exact release`) is the whole-module build, stripped,
with its dSYM and whole receipt beside it; a production build links its Rust
with fat LTO and, when its plan is fixed, leaves out the loaded modules the
plan cannot reach (LLP 1036.000 §5–§10). `--bundle` prints the stable Mac bundle at
`<target>/clients/<source-key>/<id>/macos/<Name>.app`; `scripts/exact.mjs`,
`agent --app` and metrics use that same resolver. `--host` leaves both standalone
and sample products; simulator and device bundles have separate destinations.
Two apps can build together. A second Apple build of the same source/app fails
with its owner's PID and lock path before baking; remove a stale lock only after
verifying that owner is no longer running. Failed packaging retains the previous
complete product. `EXACT_MAC_BIN` remains an explicit diagnostic override, checked
against the selected app's embedded identity before the driver launches it.

An app can ship macOS helper executables and resource trees separately from
its baked assets. Keep the tree in a dedicated directory beside `app.json`:

```json
{"host":{"macos":{"resources":[{"from":"server","to":"Resources/server"}]}}}
```

`mac --bundle` copies it to `Contents/Resources/server`, preserving file modes,
names such as `node_modules/@scope`, and relative symlinks within the tree.
These files have no bake size cap and are excluded from TypeScript capture and
web assets. `from` cannot overlap source, asset or output roots; `to` must name
a private subtree of `Resources/`, `Helpers/` or `Frameworks/`. Mach-O helpers
and libraries are signed before the outer bundle; `exact release` signs them
with its release identity. Resource changes require a new binary, not an asset
update. Find `Resources/server` through `Bundle.main.resourceURL` from a native
module. This field applies only to macOS bundles.

## Open the same development URL on Apple hosts

Start `bun host/web/dev.mjs` and open a printed URL in your browser. Build
and launch the app's native client with that same address:

```sh
bun host/apple/build.mjs --run --url http://127.0.0.1:8765/
bun host/apple/build.mjs --ios --run --url http://127.0.0.1:8765/
bun host/apple/build.mjs --device --run --url http://192.168.1.20:8765/
```

For a phone, replace the example with the server's reachable LAN or HTTPS
URL. Device builds require a connected, provisioned phone. `--url` overrides
`EXACT_DEV_PLAN` for this launch; it does not change the app's production origin.
An external app uses these commands with `EXACT_APP_DIR` set as usual.
Plans and assets reload through the native URL loader. Admitted TypeScript
module clients reload logic on web/macOS/iOS; declared Rust modules reload on
web/macOS/iOS/Linux using the executor selected below. Once built, an agent can drive
the same URL with `bun scripts/agent.mjs macos --url http://127.0.0.1:8765/ tree state logs`
(also `web`, `ios`, and `linux`; Linux polls the same URL after first pixel).
The Go/custom-client sequence
is in [LLP 1030.000 §7](../llp/1030.000-dev-server-as-deployer.rfc.md#7-exact2-go-and-custom-development-clients--implementation-direction).

Agent sessions use `exactTime()` launch facts `seed: 1` (LLP 1069.007), `locale: "en-US"`,
`timeZone: "UTC"` and `epochAtZero` 2026-01-01T00:00:00Z (LLP 1027.000.000 D3, with the
zone's `utcOffset` at that instant, answered again when the virtual date crosses a DST change) on every host. Override them at session setup with
`bun scripts/agent.mjs web --seed 42 --locale fr-CA --time-zone America/Toronto --epoch 2026-09-21T14:13:20Z tree`
or `open({host, seed: 42, locale: "fr-CA", timeZone: "America/Toronto", epoch: "2026-09-21T14:13:20Z"})`;
a test file writes them as launch lines (`epoch "2026-09-21T14:13:20Z"`, `time-zone "America/Toronto"`,
[authored tests](contract-grammar.md#authored-tests)), which override the flags.
A drive can fail fetches by URL prefix (LLP 1103): `--fail-fetch <prefix>` (repeatable) arms one before the
first data load, `fail fetch <prefix> [times <n>]` and `pass fetch <prefix>` arm and clear one mid-drive (a form of `prefer`:
`{"op":"prefer","faults":{"fail":…,"times":…}}` or `{…{"pass":…}}`), and `state.faults` lists each prefix's `times`, `left`, `hits` and
`armed`. Native carriers pass the launch table as `EXACT_AGENT_FAIL_FETCH`, web pages as `?failFetch=`, one
`<prefix>[\t<times>]` line a fault, read only in agent mode; a production build ignores both. `--fail-fetch` with `--test` arms every test.
Seeds are integers from 0 through 2^53 − 1; an epoch is an ISO date or Unix milliseconds. Native carriers pass
`EXACT_AGENT_SEED`, `EXACT_AGENT_LOCALE`, `EXACT_AGENT_TIME_ZONE` and `EXACT_AGENT_EPOCH`
(milliseconds); direct agent launches can set these too. Web agent pages accept
`?agent=1&seed=42&locale=fr-CA&timeZone=America/Toronto&epoch=1790000000000`.
The driver supplies its own defaults unless an option (or `open({env: ...})`)
overrides them. `clock +N` moves the date (`epochAtZero + now()`); `state.time`
reports all five facts. Before the first host report, the runner
also supplies usable `en-US`/`UTC` and seed 0. Ordinary launches draw their seed
from secure platform entropy. A development reload retains that launch's seed.
Linux takes its locale from the first nonempty `LC_ALL`, `LC_MESSAGES`, or `LANG`,
normalizes POSIX names to BCP 47 (`C`/`POSIX` use `en-US`), and reads the zone from
`TZ` when it names a zoneinfo entry, otherwise the system's IANA zone (UTC fallback).

## Generate TypeScript data-source types

The compiler can derive the logic interface from a Contract's source signatures:

```sh
cargo run -q -p contract -- types path/to/app.contract -o path/to/app.contract.d.ts
```

In `app.ts`, use `import type { Sources, Answer } from './app.contract.d.ts'`.
Annotate the provider map as `Sources`; each function takes `(args, store, storage)` and
returns its declared result or a Promise of it. An `Answer` dispatcher can call
`sources[source](args, store, storage)` without casts. `bun install --frozen-lockfile` installs the pinned `tsc`.
An answer is held to its shape exactly, on every executor: each declared field
present (an `undefined` one is missing, as `JSON.stringify` leaves it out), none
undeclared at any depth, each value of its declared kind. TypeScript's excess
property check misses a spread (`{ ...row, amount }` keeps `row`'s other fields),
so the refusal is at run time, the same on the web as on a device:
``` `ledger` answered outside its shape: field `days`: field `transactions`: field `cents` is not in the shape ```.
Use a distinct filename: adjacent `app.ts` shadows an `app.d.ts` import.
Generated declarations are build artifacts, not files to commit. A development
build writes them beside `app.ts` for an editor: the web build and the native
development bake both do.

### What a data module can use

Every build type-checks `app.ts` and what it imports with one configuration,
`js/bake/src/typescript.mjs`: the web build (alongside bundling, ~40 ms for a
small app, about nothing on the build's wall time) and the native bake, with
the same capture, entry and diagnostics, so an `app.ts` that builds on one
host builds on all of them and a type error stops every build the same way.
`strict`; target and library ES2023, plus ES2024's `Object.groupBy`,
`Map.groupBy`, `Promise.withResolvers` and well-formed strings; `WebWorker`'s
web APIs, never the DOM's UI types (`Document`, `HTMLElement`, `Window`).
Imports may name `.ts` files (`import { day } from './dates.ts'`) or leave the
extension off; `tsconfig.json` contributes only `paths` and `baseUrl`.

The language is the same everywhere. The globals beyond it are the browser's
on the web and these on Hermes (macOS, iOS, Linux):

| Available on every executor | Notes on Hermes |
| --- | --- |
| `fetch`, `Headers`, `Response` | Grant-checked; `signal` aborts. A `Response` has `status`, `ok`, `headers`, `text()`, `json()`, `arrayBuffer()`; no `Request`, `Blob` or `FormData` |
| `structuredClone` | No transfer list |
| `TextEncoder`, `TextDecoder` | `TextEncoder` emits UTF-8. Hermes 0.4's built-in WHATWG decoder keeps the browser-style encoding labels, including UTF-8 and UTF-16LE/BE, plus `fatal`, streaming and `ignoreBOM` behavior |
| `URL`, `URLSearchParams`, `atob`, `btoa` | |
| `crypto.getRandomValues`, `crypto.randomUUID`, `crypto.subtle` | Inside an answer; `subtle` digests (SHA-256/384/512) and ECDSA P-256 keys (LLP 1069.005), and refuses the rest by name |
| `AbortController`, `AbortSignal` | `AbortSignal.timeout()` refuses: no timers |
| `queueMicrotask`, `Promise` | |
| `Intl.NumberFormat`, `Intl.DateTimeFormat`, `Intl.Collator`, `localeCompare`, `toLocaleString` | Date formatting needs an explicit timestamp. No `Intl.PluralRules`, `RelativeTimeFormat`, `ListFormat`, `Segmenter` or `DisplayNames` (Apple's engine; Linux's is built `--intl`). `Intl.Locale` is the prelude's on every Hermes host (the engine has none): a tag parsed and canonicalized as Chrome does, its options and getters, and `getWeekInfo()` with Chrome's `{firstDay, weekend}` from CLDR's week data, by the tag's region, its `-u-rg-`, or its language's likely region (two-letter languages and a few others; another reads Monday and a Saturday-Sunday weekend), and `-u-fw-`. It has no `maximize`, `minimize` or other `get…()` list, does not canonicalize aliases (`iw` stays `iw`, `en-840` keeps `840`, and its week is then the default, Monday, where Chrome's is `en-US`'s), a formatter given a `Locale` object rather than its string uses the default locale, and `structuredClone` copies a `Locale` as `{}` where Chrome refuses it. Apple's `ja-JP` long date puts a space before the weekday (`10月6日 火曜日`, Chrome `10月6日火曜日`). Apple's engine has no `notation: "compact"`: the prelude formats its short display as Chrome does for a decimal in en, en-GB, en-IN, de, fr, fr-CA, es, es-MX, it, pt, pt-PT, nl, sv, da, nb, fi, pl, ru, uk, cs, tr, ja, zh, zh-TW, ko, hi, he, id, th and vi (and their regions); `compactDisplay: "long"`, another locale or a compact currency is printed in full and said once in the logs. It rounds a tie to even where Chrome rounds it away (`¥1,234` for 1234.5 yen), and has no `formatToParts` |
| `console` | To the runner's logs after each answer and reply, including refused calls; available through the agent's `logs` on native hosts |

Not in a data module, by design (LLP 1027.000): timers (`setTimeout`,
`setInterval`), `performance.now()`, `Date.now()`, `new Date()` without a value
and `Math.random()`: time and seeds are source arguments. Every executor
refuses them by name, with the same message, on first use: Hermes, the web's
module realm, and the web build, whose bundler gives the app's own modules
guarded `Date`, `Math`, `Intl`, timers and `performance` in place of the
page's (LLP 1027.000 D3), so an app that reads the clock fails in the web loop
as it would on a device. The type check cannot see the difference, but every
build refuses a direct use in a module `app.ts` reaches, by file and line
(`logic.ts:2:28: Date.now() is unavailable in data sources; …`), so a test that
runs the module under Bun, which has no such guard, cannot hide it.
Literal bracket access such as `Date['now']()` and `globalThis['setTimeout']()`
gets the same diagnostic as dot access, including in the web build's summary.
Aliases and dynamic property keys still reach the runtime guard.
Development JS builds name a derive
whose value fails its type check and report failed resource/source dependencies
that it read.
ES2024's resizable `ArrayBuffer`, shared memory and the RegExp `v` flag are not
in Hermes, so they are not in the library.

The dispatcher receives storage as its fourth argument:
`answer(source, args, store, storage)`. `store` remains the grant-checked secrets
interface. Native `storage.fs` provides byte-oriented files under `app:/data`,
`app:/cache`, and `app:/tmp`; `storage.sqlite` provides databases, prepared
statements, and batch transactions. Declare grants such as `fs.read app:/data`,
`fs.write app:/data`, and `sqlite.open app:/data/notes.db` in `app.ts`’s exported
`grants` string. The TypeScript module always receives `storage`, including
when no storage grants are declared. Once storage is available, ungranted
operations reject with `Unavailable` and code `denied`, without loading the
browser storage adapters. A drive without a scratch store still gets `agent`
for app-storage operations; `doc:/` handles do not need that store.
Generated declarations export Ibex2's `Storage` and related types; Rust sources
can use the same implementations through `ibex2::host`.

Hosts configure app-specific directories after first pixel. Files and databases
survive module reload; temporary storage is a directory under the app cache,
without an automatic cleanup guarantee. Agent mode does not open disk storage.
Bake rejects storage calls with `Unavailable` (`code: 'bake'`). A resource whose
answer fails for it, caught or not, compiles no value: it shows its placeholder
and is asked when the app runs, on every build. Catch it only to answer
something better than the placeholder. Browser storage uses app-scoped IndexedDB files
and SQLite WASM in a dedicated worker, loaded on the first database operation.
Use HTTPS or localhost for Web Locks. Data persists across reloads within the
same browser origin, subject to browser storage retention and quota policies.
An open database exclusively locks its file; conflicting opens or filesystem
mutations return `Unavailable` with a busy message. Agent mode skips storage.

In `app.ts`, a storage refusal is an `Error` with `kind: 'Unavailable'`, a `code` and a
`message`. The code is the same on every host; the message says more and may
differ. Storage itself is unavailable: `'bake'` (the build compiles no answer
that reads storage; show a placeholder), `'agent'` (a scripted drive that names
no scratch store, `--storage <name>`), `'unsupported'` (a host with no app
storage). The operation was refused: `'denied'` (the grants do not cover it),
the filesystem's POSIX name (`'ENOENT'`, `'EEXIST'`, `'ENOTDIR'`, `'EISDIR'`,
`'ENOTEMPTY'`, `'EBUSY'`), `'full'` (256 operations already wait behind the
one in flight: the module's storage queue is full, LLP 1097), else `'failed'`. Branch on the code, never the
message: `catch (e) { if (e.code === 'ENOENT') return empty; throw e; }`.

A drive's app storage is a scratch store it names (`--storage <name>`) or none,
kept between drives (on the web, Chrome's profile for the name and its page's
origin; a Firefox or WebKit drive's is its own); an authored test gets a fresh
one of its own, removed after it. A data module's `secret.keep` rides that same
store: files under the named tree on Apple and Linux, `localStorage` in the named
web profile. A drive with no `--storage` refuses app storage (`storage is unavailable
in agent mode unless the drive names a scratch store (--storage <name>)`) and keeps
no secrets. The driver's `state.storage` says
which (`{available: false, code: 'agent', message}` or `{available: true,
store}`), and the web's journal says `storage refused (agent): …` the first time
a refusal lands. A Rust module's storage request in such a drive is answered
with the same message, never refused outright (trivia F7).

An answer waits for the storage it awaits, and only that (LLP 1097). Storage
it starts and does not await finishes after it, as a page's does, on every
host: the answer is given when its value is ready, and the write lands behind
it as the module's background work. A save that has started lands on every host
(kanban F22, drums R10). Hermes drains the microtask checkpoint before
replying, including when the answer is a synchronous value, so a save chained
behind a promise that is already resolved is issued as that answer's work and
is not left with nobody to run it. So an editor answers from memory and saves
in the answer, unawaited:

```ts
edit(store, args) {
  song = apply(song, args);
  storage.fs.atomicWriteFile(PATH, new TextEncoder().encode(JSON.stringify(song))).catch(note);  // started now, not awaited
  return song;
}
```

Every storage operation of a module runs in one queue, in the order it was
issued, one at a time, so a read issued after a write sees it. Call storage in
the answer and let the queue order it, rather than chaining it on a promise: a
`.then` issues its write only when the promise before it lands, so a read
issued meanwhile overtakes it. Two answers interleave at their awaits, as two
async calls do on the web: writes that must stay together go in one
`transaction` (SQLite) or one operation. `clock settle` and a test's `reload`
wait for the module's storage; `clock +N` names what is left (`background`,
beside `inflight`), and `state.background` counts what landed and failed.
Every failed storage operation, every unhandled rejection and every `console`
line of the module reaches `logs` (`storage failed: …`, `data: unhandled
rejection: …`, `console: …`). Storage is refused, with its line, during module
evaluation (no answer has begun) and at bake; `fetch` and `native.call` are
refused in background work, which no answer waits for (a `fetch` belongs in an
answer). A storage or `fetch` call made when no answer is in flight is refused
and logged, never silently dropped. A worker-placed source still answers once
its storage has landed. A quit waits for the storage to land (macOS, five
seconds; a Linux exit, five; iOS holds a background task while it lands; a
native dev restart, one second); a web page being unloaded may lose what is in
flight, as any page does. A native answer that saves is a reply on real time:
under the driver it lands at the next `clock` step, not with the input (`tap`
then `expect` reads the state before it; on the web build an answer given at
once is there already), and on a device it lands a few milliseconds after the
input.

A newer send may replace a mutation's reply. An operation already issued still
runs, in the order it was issued; the replaced reply is dropped, and that
answer's Store writes are not the live answer's. A send dropped before it has
issued storage does not run. A replaced answer waiting on a `fetch` is not
stranded: on the web build (the JS target) its fetch completes and the code
after the `await` runs; natively and in the web's wasm module realm the fetch
rejects with a `FetchError` of kind `Aborted` (the request may already have
been sent), so a `catch` or `finally` runs. A stream's fetch never settles. Its reply is
dropped either way; a mutation that needs every reply is declared `queue`. Unloading finishes storage the module already
started, within a second, and drops what has not begun. Reads remain
replaceable. An answer the runner lets go between storage steps (a refresh it
discards before a mutation lands, a read whose arguments changed or that a
`refresh` replaced) still runs the steps it began, and the chain behind them, to
their end; only its answer is dropped, so
serializing storage through one promise chain composes with `refreshes` and
fast-changing arguments (ledger F12, minesweeper F10).

A `fetch` waits as long as the platform lets it (URLSession's 60 seconds
without data on Apple), holding an ordered source's lane meanwhile. Give it a
deadline with `exactTimeout`, in milliseconds (1 to 3600000), for the whole
exchange, headers and body: `fetch(url, { exactTimeout: 10000 })`. When it
passes the request is cancelled and the fetch rejects with a `FetchError`
whose `kind` is `Timeout` (`the request timed out after 10000 ms`). The same
holds on Apple, Linux, the web's wasm host and the web build; a stream
(`exactStream`) takes none.

An answer that keeps coming (LLP 1016.000) is a `fetch` with `exactStream`,
returned as the answer: `return fetch(url, { exactStream: (event) => value })`.
The promise never settles; each message, and the end, is mapped now (the
mapper cannot await) and commits as the resource's answer. An `http:`/`https:`
URL is read as server-sent events under `net.fetch`; a `ws:`/`wss:` URL is a
receive-only WebSocket under `net.websocket` alone (no frame is ever sent, so a
feed that waits for a subscribe frame cannot be read). An event is
`{type, data, lastEventId, coalesced}`; messages that arrive faster than they
commit coalesce to the newest, counted in `coalesced`. The end is
`{type: 'error', kind, message, status}`: `kind` is `Network` (the far side
closed: `the socket closed (1000)`), `Refused` (outside the grants), `Aborted`,
or `Response` with the status and body of a reply that was not an event stream.
New arguments, `refresh` or the resource leaving the view close the stream;
`state.pending` lists it until its first message, `state.streams` while it is
open. The same holds on Hermes, the web's wasm host and the web build (the JS
target), with one difference: on the web build the stream's `fetch` must be
made while the answer is asked, before its first `await` (a later one is
refused, saying so); on Hermes it may follow an `await`.

This first browser implementation targets modest app stores: filesystem
operations read the app's file records, and each SQLite mutation atomically
saves the whole database file. Database files share the filesystem namespace,
so closed databases can be copied or exported through `storage.fs`. SQLite integer results
are `bigint`: convert them to a Contract-compatible value before returning.

### Notifications

`showNotification(title=…, body=…, tag=…, showTrigger=…)` posts a local
notification; `closeNotification(tag)` takes one away, shown or still
waiting. The names are the Notification API's (`showTrigger` is the
Notification Triggers draft's member, given as the time in epoch
milliseconds: the date now is `exactTime().epochAtZero + now()`). A newer notification
with the same `tag` replaces the older. The app's grants must name
`device.notifications purpose.notifications` (a strings key, LLP 1069.008;
iOS shows its own fixed prompt text), or the command is refused. Permission
is asked the first time; the outcome is a journal line
(`showNotification: shown`, `scheduled`, `refused: denied`, …).

| host | now | at `showTrigger` |
| --- | --- | --- |
| web | `new Notification(title, {body, tag})` | while the page is open: the web has no trigger that outlives the page |
| macOS, iOS | `UNUserNotificationCenter`, shown with the app in front too | the system's, delivered with the app closed |
| Linux | refused: `unavailable` | the same |

Under the agent nothing reaches the system on any host: `state.notifications`
lists what the app posted (`{title, body, tag, showTrigger}`, a tag replacing
its older one, `closeNotification` removing it), so a drive reads a reminder
without a permission prompt. Scheduling is one time per call: a daily
reminder posts the next one when the app runs.

### Apple Health

Exact has no Health API: the app's own Swift module (LLP 1067) calls
HealthKit. What Exact does is let the binary ask. The app's grants name
`device.health-read purpose.<key>`, `device.health-write purpose.<key>`,
or both, each with its own strings key (LLP 1069.008.000):

- iOS gets `NSHealthShareUsageDescription` and
  `NSHealthUpdateUsageDescription` (one direction granted writes both, the
  other borrowing its text, which iOS never shows), and the
  `com.apple.developer.healthkit` entitlement in the signature, on a
  simulator too.
- macOS gets the two keys and no entitlement (it is restricted there, and a
  development build carrying it does not launch), so a module should treat
  a Mac's request as unavailable. tvOS, the web, Linux and Windows get
  nothing.
- A phone build needs a development profile for the app's own id with
  HealthKit turned on. A team wildcard never allows it, and the build
  refuses (`grant-device-profile`) rather than sign one that fails at its
  first request.
- An app with a Health grant keeps no answers across launches: its first
  frame never shows last launch's data from the store, and any kept answer
  on disk is forgotten at boot.

### Sounds

A declared WAV (`sound "assets/…wav"`) is played by `playSound(src, at=,
gain=, group=)`, `playSounds(hits)` and ended by `stopSounds(group=)` (LLP
1096). The runner keeps the voice table, so what was scheduled, when, and how
each voice ended is the same on every host and under the driver's clock
(`state.sounds`, `expect sound`).

| host | output |
| --- | --- |
| web (both targets) | Web Audio: a one-shot `AudioBufferSourceNode` per voice at `start(when)`, aimed at the speaker through the context's output timestamp; the first sound after the page's first tap or key (one before it is dropped and journaled `sound blocked`) |
| macOS, iOS, tvOS | one `AVAudioEngine` with a C mixer behind an `AVAudioSourceNode`, sample-accurate, in an arm loaded on demand (`libexact_sound.dylib`; `ExactSound.framework` in an `.ipa`) |
| Linux, Windows | none: the record only (`state.sounds.output` is `"none"` outside the driver) |

Under the driver nothing plays on any host (`output: "agent"`). `app.json`'s
root `audio_session` is the Apple audio session every sound, video and canvas
shares: `"ambient"` (the default: the ring/silent switch mutes it, other apps'
audio keeps playing) or `"playback"`. WebKit's Audio Session API takes it too;
the Mac has no session. A development run with `EXACT_SOUND_CHECK=1` taps the
engine's main mixer and journals (and prints) how far each onset after a
silence reached the speaker from its time.

### Media session

An `audio` or `video` with `metadata=MediaMetadata(…)` claims the platform's
media session, and the six actions it binds (`seekbackward`, `seekforward`,
`seekto`, `previoustrack`, `nexttrack`, `stop`) are the controls offered (LLP
1098). Play and pause are always offered and act on the element; the position
and playback state are the player's. The owner is the claimant that most
recently started playing, across every session of a process on Apple.

| host | published through |
| --- | --- |
| web (both targets) | `navigator.mediaSession`: the metadata (the artwork resolved as the page resolves an asset), the handlers, the declared `playbackState` and `setPositionState`; the browser decides where it shows (Chrome's media hub, the system's Now Playing) and routes the media keys; under the driver too, in the driver's own browser |
| macOS, iOS, tvOS | `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`, from the video arm (AVKit's own publication is off while a claimant owns the session); under the driver nothing is assigned and `state.mediaSession.published` is `"agent"` |
| Linux, Windows | none: the record only (`published: "none"`, no `play` or `pause`) |

On iOS the lock screen and Control Center show only a non-mixable playback
session, and audio stops at the lock without the `audio` background mode, so
an app whose plan claims the media session states both in `app.json`, or
`build.mjs --ios` refuses it:

```json
{
  "audio_session": "playback",
  "host": { "ios": { "backgroundModes": ["audio"] } }
}
```

`state.mediaSession` reads `owner` (a view id), `testId`, `claimants`,
`metadata` (as authored), `actions`, `seekOffsets`, `playbackState`, `position`,
`published` and `readback` (what the platform holds: on the web the page's
declaration, `playbackStateDeclared`); `artworkError` says why an artwork was
not published. `tap <element> mediasession <action> [seconds]` calls the handler
the platform would call (`delivery: "substituted"`).

### Documents the person chose (`doc:`)

A file or folder the person picks (`showOpenFilePicker`, `showDirectoryPicker`,
`showSaveFilePicker`), or opens from the system at the app's `open-file` node,
arrives as a `doc:/<n>/<name>` path (LLP 1069.010 D1). `storage.fs` reaches it,
and paths beneath a chosen folder, under the grants `fs.read doc:/` and
`fs.write doc:/` — the same grants, operations, refusals and codes whether the
data module is TypeScript or Rust, on every host:

```ts
export const grants = 'fs.read doc:/';
// listing([folder]) with folder = 'doc:/1/notes', from `change` on the picker's node
for (const name of await storage.fs.readdir(folder)) {
  const stat = await storage.fs.stat(`${folder}/${name}`);       // a folder's size is 0
  if (stat.isFile) bytes = await storage.fs.readFile(`${folder}/${name}`);
}
```

| Operation on a `doc:` path | Grant | What it does |
| --- | --- | --- |
| `readFile`, `stat`, `readdir` | `fs.read doc:/` | `doc:/<n>` itself lists only `<name>` |
| `writeFile`, `atomicWriteFile`, `appendFile` | `fs.write doc:/` | Creates a file beneath a chosen folder |
| `mkdir` | `fs.write doc:/` | Makes the folders above it too |
| `rm` | `fs.write doc:/` | One file or one empty folder beneath the chosen document; never the document itself, never a tree |
| `rename`, `copyFile`, `realpath` | — | Refused (`'failed'`): read the bytes and write them |

A path never minted, `.`/`..`, and a closed window's or page's handle are
refused. A document needs no app storage: a drive without `--storage` reaches
it. On the web the paths are the `FileSystemHandle`s the page's picker
returned (Chromium; Safari and Firefox refuse the pickers), and a module placed
on a worker on the wasm web host cannot reach them (`'unsupported'`); on macOS
and iOS they are security-scoped URLs held for the session; Linux opens no
picker panel (a drive's held pickers still answer), so outside a drive only
`open-file` delivers one there. The handles end with the
session: nothing about a document is remembered across launches.

### Bake and deliver a TypeScript module

Build an app-local `app.ts` module and bake its Contract through the resulting
Hermes bytecode with the compiler from the verified install-once bundle:

```sh
cargo run -q -p exact-js-bake -- path/to/app --out path/to/new-generation
```

The app exports `appId`, `grants`, and an `Answer`-typed `answer`. The producer
captures local imports, type-checks, bundles with Rolldown, compiles HBC, and
bakes with an empty store. It writes `app.plan`, `app.js`, `app.hbc`, generated
types, and an `app.module.json` pairing receipt into a **new** directory; it
never overwrites an existing generation. npm dependencies are not captured yet.
`EXACT_TSC` and `EXACT_ROLLDOWN` override producer tools. `hermesc` is always
the compiler paired with the selected `hermes-lean-sys` bundle.
A module's placement (LLP 1027.002) is the manifest's: `typescript.placement`
and `rust.placement` are `main` (the default) or `worker`, overridable per
platform under `platforms.<platform>.placement`; `EXACT_TYPESCRIPT_PLACEMENT`
and `EXACT_RUST_PLACEMENT` override a bake for a measurement. A change is a new
compatibility id, never an update. `build_mixed_with` bakes a mixed app through
its own composer, so a resource only Rust owns needs no TypeScript placeholder;
the development producer keeps such a resource's last Cargo-baked value.

Native module clients can supply a `Module` factory to `exact_apple::host!`
(the sixth argument) and apply an `ExactGeneration` containing an `ExactModule`
through `ExactApp.applyGeneration`. All sessions prepare before any commit;
changed logic re-asks resources while preserving compatible slots and clock.
Initial module loading happens after first pixel. The binary's app identity
and grants must match; a client must include the candidate's executor. Pairing
hashes are not authentication: this API requires an admitted development origin,
and does not accept signed-delivery generation tokens.

For a module client's `build.rs`, depend on `exact-js-bake` and call
`exact_js_bake::build(Path::new(".."), "web")` (or `"macos"` / `"ios"`). This writes the
paired artifacts, `compat.json`, and `module.rs` constants (`APP`, `GRANTS`,
`REVISION`) into `OUT_DIR`. Set the participating platforms' `deploy.store` entries to
`"0"` in `app.json`: signed module delivery is not implemented.

The web crate links `exact-js-web`, not Hermes. Include the generated constants
and artifacts, then use the host macro's factory and paired-artifact arguments:

```rust
include!(concat!(env!("OUT_DIR"), "/module.rs"));
exact_web::host!(exact_js_web::Module,
    include_bytes!(concat!(env!("OUT_DIR"), "/app.plan")),
    include_str!(concat!(env!("OUT_DIR"), "/compat.json")),
    || exact_js_web::Module::new(APP, GRANTS, REVISION), [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")),
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")),
        include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc")),
    ]);
```

Run the ordinary build scripts and `bun host/web/dev.mjs --app <name>` with
`EXACT_APP_DIR` set for an external app. The dev server watches local TypeScript
imports and Contract, publishes complete immutable generations, and the same URL
delivers plan/logic/assets to the browser and an admitted macOS/iOS client without
rebuilding either binary. Browser code runs after first paint in a disposable
private realm; page and guest globals are untouched. This is trusted app code,
not a security sandbox. Corrupt, incompatible, or failing candidates preserve
the running app.

Browser providers support async answers and sequential/parallel `fetch` through
the existing grant-checked host transport. Executor-local continuation tickets
drain microtasks without re-entering wasm; stale incarnations cannot fulfill the
replacement app. 

Linux uses the same installer command at the top of this reference with its
Rust target triple. Exact selects Ibex's `intl` feature and `INTL` group there:
`en`/`en-US` and complete currency data are present, and unsupported locales
fall back to `en-US`; it does not select `intl-all-locales`. The verified
bundle supplies both VM archives, all three ICU data tiers and the matching
compiler, while the feature links the English tier. Apple does not install
Ibex's `INTL` group because Hermes retains OS-backed Intl there. iOS, tvOS and
Windows use their pinned v4 bundles and must not fall back to the former
sibling-Ibex/private-cache recipes. tvOS builds set a 17.0 deployment target,
above the bundle's 15.0 minimum. The normal native
build captures bundle inputs under the `hermes-lean-sys/` bake receipt root.
`smoke.mjs --app-only` runs the selected app and its tests without unrelated
bare-plan host fixtures. The driver supports `ios --device [--phone <name|udid>]`:
the phone connects outward to a temporary Mac-side port with a per-launch token.
Use a trusted LAN, allow local networking, and keep the app visible;
`EXACT_AGENT_HOST` overrides the Mac IPv4 address. The carrier is not encrypted.

The dev page's **Open in native…** link offers an installed-client action and
local setup instructions at `/__dev/open`. Development Apple builds register an
app-specific opening scheme and pass its HTTP(S) locator to the existing loader;
production builds do not register that development handler. iOS handles cold and
warm URL delivery. For a local macOS bundle, use
`bun host/apple/build.mjs <app>-apple --bundle` and open the printed `.app` once.
The bundle includes its assets and native modules; it is not a notarized download.
Browser navigation, both Apple cold/warm handlers and malformed-link refusals are
tested. The page cannot detect installation, and does not trigger signing/builds.

**Limits.** Linux native hosts don't run TypeScript yet (use a Rust data crate
there); `app.ts` can't import npm packages; signed delivery of TypeScript and Rust
modules isn't implemented, so set `deploy.store` to `"0"`. The history of how this
was proved on each host is in [LLP 1027](../llp/1027-typescript-data-sources.rfc.md)
and git.

## Rust data sources

A data crate answers the view's sources in Rust instead of `app.ts`: an app's
`data/` crate ([Caltrain](../apps/caltrain/data/src/lib.rs),
[Fieldnotes](../apps/fieldnotes/data/src/lib.rs)) or a game's `game.data`
(`game/README.md`). Its type implements `exact_runner::DataSource`:

| Method | What it does |
|---|---|
| `query(source, args)` | Answer now, with no I/O: the build bakes this into the plan for the first frame. `Err(DataError::Unavailable(…))` leaves the resource unbaked; every host asks again at launch. |
| `answer(store, source, args)` | At run time: `Answer::Now(value)`, or `Answer::Later(request)` for the host to run (storage, `fetch`). The default is `query`. |
| `parse(store, source, args, outcome)` | What the host brought back for that request: answer, or hand back one more request. |
| `app_id()` | The app's identity, which names its storage: `com.example.my-app`, or for a game `com.exact.<Game::ID>` unless its app.json names one. A source with no id has no app storage on Apple and Linux. |
| `grants()` | One grant per line, the lines `app.ts`'s `grants` takes (`net.fetch …`, `sqlite.open …`, `fs.read …`). |

A record is positional: `Value::record(vec![…])` takes its fields in the order
the shape declares them. `bun exact.mjs contract rust app.contract -o
/tmp/shapes.rs` writes each shape as a struct with its fields in that order;
`Value::Number`, `Value::str`, `Value::Bool` and `Value::list` are the rest.

### Where data lives: app storage, not secrets

Keep app and game data (best times, settings, saves, notes) in **app
storage**: SQLite databases and files under `app:/data`, the storage `app.ts`
reaches as `storage.sqlite` and `storage.fs`. A Rust source asks for it with
`exact_data::storage::request(op, args)` as an `Answer::Later`, and reads the
reply in `parse` with `exact_data::storage::response(outcome)`, which is the
JSON result or the host's message. The operations:

| `op` | `args` | Reply |
|---|---|---|
| `"sqlite"` | `{"path": "app:/data/x.db", "commands": [{"kind": "execute" or "query", "sql", "params"}]}` | One result per command, in order: a query's `{"columns", "rows"}` (an integer is `{"integer": "7"}`, text a string), an execute's `{"changes", "lastInsertRowid"}` |
| `"sqlite.transaction"` | the same, `execute` commands only | the executes' results, all or none |
| `"fs.readFile"`, `"fs.atomicWriteFile"`, `"fs.writeFile"`, `"fs.appendFile"` | `{"path"}`, and `"text"` (or `"bytes"`) to write | read: `{"base64"}`; write: `null` |
| `"fs.mkdir"`, `"fs.rm"`, `"fs.stat"`, `"fs.readdir"`, `"fs.rename"`, `"fs.copyFile"` | `{"path"}`, and `"destination"` to move or copy | as `storage.fs` answers |

Grant what it touches: `sqlite.open app:/data/x.db`, `fs.read app:/data`,
`fs.write app:/data`. Storage is asynchronous and starts after first pixel. It
is absent at build, so `query` answers a placeholder, and in a scripted drive
that names no scratch store: there `response` is an error, and the source
answers as if nothing were kept. `--storage <name>` gives a drive a store kept
between drives; an authored test gets an empty one, and its `reload` restarts
on it. A game's data crate takes these crates from the SDK
(`exact-data.workspace = true`, `serde_json.workspace = true`).

Best times, one row a level, decided in SQL. This is the platformer's source
(the diary's R6). One request reads the previous best and writes the run where
it beats it:

```rust
use exact_data::storage;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store, Value};
use serde_json::json;

const DB: &str = "app:/data/scores.db";
const TABLE: &str = "CREATE TABLE IF NOT EXISTS best (level INTEGER PRIMARY KEY, ms INTEGER NOT NULL)";

#[derive(Default)]
pub struct Scores;

impl DataSource for Scores {
    fn app_id(&self) -> &str { "com.exact.hopper" }
    fn grants(&self) -> &str { "sqlite.open app:/data/scores.db" }
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "best" => Ok(Value::record(vec![Value::list(vec![])])), // the build's first frame: nothing kept
            _ => Err(DataError::Unavailable("asked at run time".into())),
        }
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let commands = match (source, args) {
            ("best", _) => json!([{"kind": "execute", "sql": TABLE, "params": []},
                {"kind": "query", "sql": "SELECT level, ms FROM best ORDER BY level", "params": []}]),
            ("finish", [Value::Number(level), Value::Number(ms)]) => json!([{"kind": "execute", "sql": TABLE, "params": []},
                {"kind": "execute", "sql": "INSERT INTO best VALUES (?, ?) ON CONFLICT(level) DO UPDATE SET ms = excluded.ms WHERE excluded.ms < best.ms", "params": [level, ms]}]),
            _ => return Err(DataError::UnknownSource(source.into())),
        };
        Ok(Answer::Later(storage::request("sqlite", json!({"path": DB, "commands": commands}))))
    }
    fn parse(&mut self, _: &mut Store, source: &str, _: &[Value], outcome: Outcome) -> Result<Answer, DataError> {
        let results = storage::response(outcome);
        let int = |cell: &serde_json::Value| cell["integer"].as_str().and_then(|n| n.parse::<f64>().ok());
        Ok(Answer::Now(match source {
            // No storage here (a drive with no scratch store): nothing kept.
            "best" => Value::record(vec![Value::list(results.ok().and_then(|r| r[1]["rows"].as_array().cloned()).unwrap_or_default()
                .iter().filter_map(|row| Some(Value::record(vec![Value::Number(int(&row[0])?), Value::Number(int(&row[1])?)]))).collect())]),
            _ => Value::record(vec![Value::Bool(results.map_err(DataError::Unavailable)?[1]["changes"].as_str() == Some("1"))]), // a new best?
        }))
    }
}
```

`Store` (`store.get`, `store.set`, under `secret.keep <name>`) is for
**secrets**: a session token, a key. Apple keeps them in the Keychain and the
web in `localStorage`; the host reads them into a snapshot before boot, so a
read is synchronous, and a scripted drive never keeps them. A best time is not
a secret: keep it in app storage.

## The five checks

```sh
cargo build --all-targets --keep-going                                  # build
cargo test --lib --bins --tests --no-fail-fast                          # test
cargo clippy --all-targets --keep-going -- -D warnings                  # lint, and
cargo fmt --all -- --check                                              # lint (run both)
bun scripts/caps.mjs                                                   # caps
bun scripts/boot.mjs                                                   # boot graph
```

Cargo's checks cover the root `default-members`: the deterministic, in-process
crates, the web host's and the Apple host's Rust among them (the Apple host's
real-socket, wall-clock and toolchain-launching tests are `async lane:`; a test
may still re-run its own binary to isolate its environment). The async lane runs the same commands with
`--workspace` (the other hosts, GPU, Hermes, platform shells, stress fixtures).

Development and test builds optimize the third-party CPU rasterizer `tiny-skia`.
Debug assertions and overflow checks remain enabled; the normal development
profile still leaves app and engine code unoptimized. Tests keep their full frame
counts. Use release builds when comparing application frame costs.

An unset `EXACT_UPDATE_TRUST` bakes development trust. A development binary
admits unsigned heads, so it checks only an origin named by
`EXACT_UPDATE_ORIGIN`, never the manifest's. `EXACT_UPDATE_TRUST=production`
bakes a release: an updating native artifact requires
`EXACT_UPDATE_RECEIPT` pointing to the authenticated publisher receipt for its
exact plan and complete asset roster. A new production stream instead requires
`EXACT_UPDATE_GENESIS=1`; it starts at sequence zero. Existing streams retain the
receipt's sequence and verification keys. Updater-free Level 0 artifacts require
neither input.

`kernel/tables/schema.json` is the one declaration authority for node types, props,
style rows, enums, and opcodes; `kernel/build.rs` generates the Rust from it at build
time. Edit the table, never the generated code.

## Installation page

Every web build includes `/.exact/install/`, with explicit `web/`, `ios/` and
`macos/` pages. Each shows stacked Web, iOS and macOS sections; unavailable
platforms are gray. Web works by default and shows its destination URL. Native
methods appear only when configured. The header shows build/source/timestamp
and serving context. Optional `brand.logo` and `brand.wordmark` reuse app images
or a text wordmark with an asset font; the footer uses the gray Exact mark.
Beside the pages, `/.exact/install.json` carries the same content as data (the
app, its build, each platform's methods in order with labels resolved, and what
it can reach), for a native client such as Exact2 Go to draw its own install
sheet. Both web targets write them; a deploy's JS root carries its bake's.

Configure `install.<platform>.methods` in the app’s `app.json`; use
`recommended` to name a configured method. See [LLP 1030.003 D6a](../llp/1030.003-continuous-release-loop.rfc.md#d6a--the-standard-install-page)
for the schema and examples. A method links to an already available installation
flow; configuring it does not build, sign, upload or verify a native app.
`terminal` displays a command and offers Copy, never executes it.

On a Mac, the development server adds a dev-only iOS method to the page. It lists
available iOS Simulators and reachable paired devices, then **Build and install**
runs the existing `--ios --sim` or development-signed `--device` build, installs
the app, and opens it on the same development URL. Simulator builds work from a
loopback server. A physical phone must be unlocked, paired, in Developer Mode and
able to reach the server, so start the dev server with `--lan`; the build opens the
app on a LAN address the server printed. The action appears only on a page loaded
over loopback on the Mac itself, and its request is protected by a random token that
exists only for that server process. Static and hosted pages never expose it.

For additional web destinations, set `install.web.urls` to entries such as
`{"label":"Public", "url":"https://interview.example/"}` alongside the web
method. These are explicit HTTPS destinations; the build does not guess a public
hostname. Development install pages also list localhost and interface addresses
for the actual listener, labeling LAN and Tailscale/VPN addresses. Loopback-only
servers omit other interfaces. Published pages do not expose the host's private
network addresses. Address discovery uses the OS interface list, with no external
commands on the request path.

## Rust live replacement

Rust replacement is enabled by default in development and production: native
shared libraries on macOS/Linux, the browser's Wasm executor on web, and the
`wasmi` interpreter on iOS devices and simulators. Native hosts start with their
linked Rust; interpreted execution applies to replaced logic. Android/Windows
have policy defaults but their hosts remain future work. See
[LLP 1029.000](../llp/1029.000-rust-development-reload.rfc.md) for the boundary and
verification status.

Control inclusion in `app.json`. `"rust": false` disables everywhere;
`"rust": {"prod": false}` keeps it in development only. A platform can override
both or either environment:

```json
{
  "rust": {
    "module": { "package": "caltrain-logic" },
    "platforms": {
      "ios": { "prod": false },
      "macos": { "prod": "wasm" }
    }
  },
  "dev": { "rebuild": { "rust": "manual", "typescript": "save" } }
}
```

Modes are `auto`, `native`, `tiered`, `wasm`, and `off`; `true` means `auto` and `false`
means `off`. Resolution is global → environment → platform → platform environment.
`off` removes the replacement path from the native bake; changing that requires
a new binary. TypeScript and delivery cadence remain separate choices. Save is
the default trigger for both languages while the development watcher runs;
manual is useful when an agent wants to finish a set of edits before rebuilding.
After the app's initial web bake, run `bun scripts/rust.mjs caltrain` to build
the independent Rust module variants; the running dev server consumes the
completed output. At the dev
server’s terminal, `r` + Enter rebuilds Rust and `t` + Enter rebuilds TypeScript.

For portable modules without essential private state,
`"rust": {"platforms": {"macos": {"dev": "tiered"}}}` runs new Wasm immediately
after publication and promotes to native when the library finishes loading.
Both variants must declare `exact_logic_abi::export!(Data, constructor, stateless)`:
essential state lives in host inputs, Contract or host-owned storage; initialization
and destruction have no external effects. Promotion preserves the app generation
and does not restart TypeScript or replay business calls. Native loading failures
leave Wasm running. This mode adds the interpreter and both artifact variants;
explicit `native` avoids that interpreter. The usual dev/prod overrides apply,
but `tiered` is unavailable on iOS/web. Update Lab opts in for macOS development;
`auto` defaults are unchanged. Both variants still compile before publication.

The repository's app entries use `contract::rust_entry` to select a concrete
factory at bake time. A custom host adapter must do the same (and depend on
`exact-logic`), or compose `Swappable::native`, `Swappable::tiered`, `Swappable::wasm` or
`Swappable::browser` explicitly. Merely declaring a module does not retrofit
a custom, manually implemented `DataSource` host.

[Update Lab](../apps/update-lab/README.md) composes one Rust probe with one
TypeScript executor. Its complete development candidate carries both modules,
so a TypeScript or Contract edit retains the last accepted Rust version.

The replacement unit is the declared portable logic-wrapper `cdylib`, not an individual
Rust function or file. Keep business logic in small Cargo crates and native I/O
in host adapters; Cargo reuses unchanged helpers, but their dependent loadable
module still relinks. Independent replacement of several domains requires
separate loadable artifacts and explicit host composition; a multi-module
manifest registry is not implemented.

Production capability does not grant store permission to deliver arbitrary
code updates. Apple's [review guidelines](https://developer.apple.com/app-store/review/guidelines/)
restrict downloaded feature-changing code, including interpreted code; Google
Play's [policy](https://support.google.com/googleplay/android-developer/answer/16559646?hl=en)
restricts downloaded native `.so` code and describes a VM/interpreter exception.
Use the production opt-out where the app's distribution requires it. Native
macOS libraries also need platform-appropriate code signing: the production
producer requires `EXACT_RUST_SIGN_IDENTITY` for the host's signing team, or
select `wasm` for macOS production.

## Interactive stress examples

[Messages stress](../apps/messages-stress/README.md) compares a bounded synthetic
history page with deliberately eager construction while typing and streaming
updates. [Completion Storm](../apps/completion-storm/README.md) holds and releases
real local HTTP requests, including failures and replies to a departed screen.
[Markdown stress](../apps/markdown-stress/README.md) exercises the shipped parser
and reader components with large documents, huge individual blocks and reflow,
and the Markdown editor with a toolbar and a link sheet
([`markup`, `format`, `select`](contract-grammar.md#markdown-markup-format-select)).
These are opt-in developer workloads; none claims automatic virtualization or
120 Hz performance. [LLP 1041](../llp/1041-graceful-overload.rfc.md) specifies the
graceful-overload direction and records what the first examples actually prove.

With a fixture running, `bun scripts/metrics.mjs --stress-url
http://127.0.0.1:4318 --seconds 10 --target-hz 120` samples typing-to-echo and
frame-callback gaps (`CHROME` selects the browser). Repeat `--tap <testId>` for
workload controls. This is a headless diagnostic, not a physical-display FPS test.

After building a native app, `bun scripts/native-resize-metrics.mjs macos
--app messages-stress` interleaves window resizing, typing and scrolling, then
checks geometry, echo and recovery. Substitute `linux` on an actual Linux host
or select `--app markdown-stress` / `completion-storm` (with its fixture running).
It records raw command acknowledgments and executable identity; AppKit window
resizing and Linux headless presenter resizing do not measure physical refresh.
