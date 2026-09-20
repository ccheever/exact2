# Exact

Tooling runs on **Bun 1.3.12 or newer** (release toolchain pinned in `package.json`).
Run `bun install --frozen-lockfile` to install the dependencies in `bun.lock`.
Node and npm are not required. Rolldown remains the app bundler; the existing
build, serve, watch, and reload scripts run under Bun. Run tooling unit tests with
`bun test ./scripts/install-page.test.mjs ./scripts/rust.test.mjs`; the explicit
paths keep Bun from searching generated fixture checkouts. This is the source
tooling installation; a standalone CLI distribution is not packaged yet.

Snapback4 consumers use release **0.2.30**: the CLI and browser device are pinned
in `bun.lock`; Cargo pins native devices and schema compilers to the matching
release source commit `a397218e2332964ebe29aa1d30918c436713cc8a`.
Run `bun install --frozen-lockfile` before baking Messages, and use the pinned CLI
with `bun run --bun snapback4` from an app directory.


A cross-platform application runtime. The Rust kernel computes layout, each platform
renders natively, and Contract is the authoring model.

Surfaces: web, macOS, iOS, Linux.

Start here:

- **`rules/RULES.md`** — how work happens here. One page. Read it before your first PR.
- **`rules/NOT-DOING.md`** — what v1 deliberately excludes, and why.
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
| `apps/fieldnotes/` | Offline notes: Contract UI, TypeScript sources, SQLite persistence, and JSON file backup/restore. See its [README](apps/fieldnotes/README.md). | LLP 1027 |
| `apps/markdown/` | A Markdown reader: the parser and block model both readers share, and the general one-file app. `mdview`. See its [README](apps/markdown/README.md). | LLP 1033 |
| `apps/llp/` | The same reader specialised for an LLP corpus: numbered index with sub-documents, search over every document's text, section outline, `LLP 1234` as a link. `llpview`. See its [README](apps/llp/README.md). | LLP 1033 |
| `apps/caltrain/` | The v1 app: `app.contract`, its Rust data crate, and its wasm crate; the end-to-end fixture. | — |
| `apps/exact-live/` | A creative production workspace combining photo zoom, scene ordering, crew chat and a runbook. The [browser preview](https://exact-live.tuft.host/) passes 19 interaction checks; native delivery and connected jobs remain in progress. See its [README](apps/exact-live/README.md). | LLP 1041 §8 |
| `gpu/` (`exact-gpu`) | The GPU canvas: a `Surface` trait against wgpu, a per-app module loaded on demand (a dylib on macOS, a second wasm on the web) after the first pixel; the same Rust renders on Metal and on the browser's WebGPU. `apps/caltrain/gpu` is the line map and the aurora. `gpu/reflect` (`exact-gpu-reflect`, naga only) reflects every `.wgsl` in a GPU crate's `build.rs`: bindings, struct layouts, vertex inputs, and entry points generated as Rust, the WGSL as the one declaration authority. | LLP 1009 |
| `host/apple/` (`exact-apple`) | The Apple host: runner + kernel as a static library with a C ABI, the kernel's layout with CoreText measurement through a callback, `exact-motion` as the executor, typed batches; `macos/` is the AppKit presenter and `ios/` the UIKit one (SwiftPM, sharing `swift/`). `bun host/apple/build.mjs --run`; `bun host/apple/build.mjs --ios --run` on a simulator. | LLP 1008 |
| `host/linux/` (`exact-linux`) | The Linux host, the first that paints: runner + kernel natively, cosmic-text measuring and painting from one cache, `exact-motion` as the executor, the kernel tree drawn by one walk over a backend — vello on the GPU (the main one), tiny-skia on the CPU (the fallback and the pixel oracle) — onto DRM/KMS dumb buffers with evdev input, or into a buffer with no display (the agent API, screenshots, the smoke; on macOS too). Pure Rust, no system library. `cargo build --release -p caltrain-linux`. | LLP 1015 |
| `host/web/` (`exact-web`) | The web host: runner + kernel in wasm over the real DOM, CSS computed once from the kernel's rows, springs lowered to frames the browser plays, a no-`unsafe` ABI, ~150 lines of glue, a headless-Chrome smoke, the motion parity harness, and the dev loop (`bun host/web/dev.mjs`, edit → present ~20 ms). | LLP 1007 |
| `vendor/taffy/` | Taffy 0.9.2 plus two Exact patches. | `vendor/taffy/EXACT-PATCHES.md` |

All four surfaces run the app; `QUEUE.md` is the ordered list of what would
make sense to do next.

## Inspect and format Contract

```sh
cargo run -q -p contract -- build apps/messages/app.contract --json
cargo run -q -p contract -- build apps/messages/app.contract -o /tmp/messages.plan --map
cargo run -q -p contract -- symbols apps/messages/app.contract
cargo run -q -p contract -- symbols apps/messages/app.contract --name selectRecipient
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
it. A path from the command line, from Finder, from ⌘O, or from a link inside
a document all arrive at the same place: the app's `open-file` node
(LLP 1033 D3). `exact uninstall <app>` takes both halves away.

Apple products and Swift caches live under the resolved app's target directory,
scoped by canonical source directory, manifest id, destination, composition and
trust policy. `--bundle` prints the stable Mac bundle at
`<target>/clients/<source-key>/<id>/macos/<Name>.app`; `scripts/exact.mjs`,
`agent --app` and metrics use that same resolver. `--host` leaves both standalone
and sample products; simulator and device bundles have separate destinations.
Two apps can build together. A second Apple build of the same source/app fails
with its owner's PID and lock path before baking; remove a stale lock only after
verifying that owner is no longer running. Failed packaging retains the previous
complete product. `EXACT_MAC_BIN` remains an explicit diagnostic override, checked
against the selected app's embedded identity before the driver launches it.

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
is in [LLP 1030.000 §7](llp/1030.000-dev-server-as-deployer.rfc.md#7-exact2-go-and-custom-development-clients--implementation-direction).

## Generate TypeScript data-source types

The compiler can derive the logic interface from a Contract's source signatures:

```sh
cargo run -q -p contract -- types path/to/app.contract -o path/to/app.contract.d.ts
```

In `app.ts`, use `import type { Sources, Answer } from './app.contract.d.ts'`.
Annotate the provider map as `Sources`; each function takes `(args, store, storage)` and
returns its declared result or a Promise of it. An `Answer` dispatcher can call
`sources[source](args, store, storage)` without casts. `bun install --frozen-lockfile` installs the pinned `tsc`.
Use a distinct filename: adjacent `app.ts` shadows an `app.d.ts` import.
Generated declarations are build artifacts, not files to commit.

The dispatcher receives storage as its fourth argument:
`answer(source, args, store, storage)`. `store` remains the grant-checked secrets
interface. Native `storage.fs` provides byte-oriented files under `app:/data`,
`app:/cache`, and `app:/tmp`; `storage.sqlite` provides databases, prepared
statements, and batch transactions. Declare grants such as `fs.read app:/data`,
`fs.write app:/data`, and `sqlite.open app:/data/notes.db` in `app.ts`’s exported
`grants` string.
Generated declarations export Ibex2's `Storage` and related types; Rust sources
can use the same implementations through `ibex2::host`.

Hosts configure app-specific directories after first pixel. Files and databases
survive module reload; temporary storage is a directory under the app cache,
without an automatic cleanup guarantee. Agent mode does not open disk storage.
Bake rejects storage calls with `Unavailable`; catch it when a resource needs an
empty-store bake placeholder. Browser storage uses app-scoped IndexedDB files
and SQLite WASM in a dedicated worker, loaded on the first database operation.
Use HTTPS or localhost for Web Locks. Data persists across reloads within the
same browser origin, subject to browser storage retention and quota policies.
An open database exclusively locks its file; conflicting opens or filesystem
mutations return `Unavailable` with a busy message. Agent mode skips storage.

This first browser implementation targets modest app stores: filesystem
operations read the app's file records, and each SQLite mutation atomically
saves the whole database file. Database files share the filesystem namespace,
so closed databases can be copied or exported through `storage.fs`. SQLite integer results
are `bigint`: convert them to a Contract-compatible value before returning.

Build an app-local `app.ts` module and bake its Contract through the resulting
Hermes bytecode (currently a macOS producer with the sibling ibex toolchain):

```sh
cargo run -q -p exact-js-bake -- path/to/app --out path/to/new-generation
```

The app exports `appId`, `grants`, and an `Answer`-typed `answer`. The producer
captures local imports, type-checks, bundles with Rolldown, compiles HBC, and
bakes with an empty store. It writes `app.plan`, `app.js`, `app.hbc`, generated
types, and an `app.module.json` pairing receipt into a **new** directory; it
never overwrites an existing generation. npm dependencies are not captured yet.
`EXACT_TSC`, `EXACT_ROLLDOWN`, and `EXACT_HERMESC` override producer tools.
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
replacement app. Real Chrome tests run all 20 Caltrain data cases and the same
25 ambient-read probes at initialization, in answers, and after fetch as Hermes,
plus store, errors, binary responses, interleaving and disposal cases.

Linux provisions the same vanilla pin with `./scripts/build-hermes-linux.sh
--vanilla --release --intl` in the sibling Ibex checkout. Exact links its lean
archive from `ibex/linux-vanilla` and compiles with the matching
`ibex/tools/hermes-vanilla/hermesc-linux-<arch>`. After replacing an engine or
compiler, run `cargo clean -p exact-js` before rebuilding native apps so a warm
build cannot reuse captured archives or bytecode from the previous installation.

iOS uses lean bytecode-only Hermes archives, not the compiler-containing
framework. Provision matching device/simulator builds under `target/hermes-ios`
(override with `EXACT_HERMES_IOS_DIR`); the recipe and archive layout are in
[LLP 1027 D6](llp/1027-typescript-data-sources.rfc.md#d6--the-web-the-browser-is-the-executor-one-wasm-import-the-same-module-under-two-loaders).
The normal Apple build captures the linked archives in its receipt. The iOS
simulator executed an async module, fetched twice and followed a URL logic edit
while retaining count 1 alongside the browser. The device-target archive also
builds. The simulator guard app passed all 25 forms at initialization, in direct
calls and after fetch, explicit UTC/Intl inputs, interleaved async calls and an
uncaught-initialization refusal (27 HTTP requests, no pending work).
The physical iPhone 17 Pro Max / iOS 26.6.1 now passes the same guard sweep,
including all 75 refusals, 27 HTTP requests, no pending work, and a copied,
inspected screenshot. A repeat assertion run passed in 5.7 s (83.5 ms first
frame, one sample rather than a startup budget result).

The TypeScript Caltrain twin passed the complete app drive and all three
Contract tests on web, macOS, iOS simulator and physical iPhone, with its real assets, deck and
GPU module. Production Caltrain remains Rust. `smoke.mjs --app-only` runs the
selected app and its tests without unrelated bare-plan host fixtures, which a
paired module client correctly refuses. The driver now supports
`ios --device [--phone <name|udid>]`: the phone connects outward to a temporary
Mac-side port with a per-launch token, because developer-console stdin closes
immediately. Use a trusted LAN, allow local networking, and keep the app visible;
`EXACT_AGENT_HOST` overrides the Mac IPv4 address. The carrier is not encrypted.
Physical URL replacement is now driven alongside the browser: TypeScript edits
change the answer with counter 1 and clock 12345 retained, unchanged plan and
native binary, and the same phone PID. A candidate that throws only at the carried
counter preserves both clients; the next valid edit recovers. Each valid revision
passes the async guard sweep. Earlier apparent stalls included a UIKit delayed-touch
crash; the dev-menu recognizers no longer delay touch endings. The complete proof
passes with the menu enabled and tracing removed. The full Caltrain URL proof
also passes: live edit, broken-candidate refusal and recovery preserve the selected
station, clock and train boards, with unchanged phone PID/native binary. The
initial menu-only mitigation was incomplete: Caltrain's hover recognizers still
delayed touch endings. Hover now neither delays nor cancels finger events, and
the four-finger shortcuts accept only direct touch events. Two physical Caltrain
replacement/refusal/recovery runs pass with the menu enabled (the final one with
tracing removed). Those gesture mitigations did not fix real finger scrolling:
the same-binary diagnostic isolated session creation before UIApplicationMain.
Both iOS adapters now create sessions after UIKit starts; Charlie confirmed
scrolling in regular Caltrain and opening it natively from Safari. Normal URL
module replacement also configures storage before activation, exactly once.
Manual four-finger single/double-tap verification remains owed.
Agent deadlines include native
diagnostics; a closed carrier rejects later requests immediately. Systematic
size/startup/per-call measurements remain to be proved.

The dev page's **Open in native…** link offers an installed-client action and
local setup instructions at `/__dev/open`. Development Apple builds register an
app-specific opening scheme and pass its HTTP(S) locator to the existing loader;
production builds do not register that development handler. iOS handles cold and
warm URL delivery. For a local macOS bundle, use
`bun host/apple/build.mjs <app>-apple --bundle` and open the printed `.app` once.
The bundle includes its assets and native modules; it is not a notarized download.
Browser navigation, both Apple cold/warm handlers and malformed-link refusals are
tested. The page cannot detect installation, and does not trigger signing/builds.
Safari's reported 5–10-second initial scroll delay remains unresolved: the web
root is inert until the module loads. A held-loader Chrome probe confirmed that
this blocks scrolling despite the complete list already being present; physical
Safari timing still needs a working remote automation connection. Web-only program
rebuilds also currently invalidate connected native clients unnecessarily.

Remaining: Linux native TypeScript execution, npm dependency capture, signed
module updates, downloadable custom clients, and the generic Go launcher.
One async web/iOS edit measured 410 ms save-to-DOM / 430 ms to a rendering
opportunity; the 100 ms save-to-present p50 target is not demonstrated.

## The five checks

```sh
export EXACT_UPDATE_TRUST=development                                   # local development artifacts
cargo build --workspace                                                 # build
cargo test --workspace                                                  # test
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check   # lint
bun scripts/caps.mjs                                                   # caps
bun scripts/boot.mjs                                                   # boot graph
```

Development and test builds optimize the third-party CPU rasterizer `tiny-skia`.
Debug assertions and overflow checks remain enabled; the normal development
profile still leaves app and engine code unoptimized. Tests keep their full frame
counts. Use release builds when comparing application frame costs.

The shared build scripts select development trust explicitly too. Direct Cargo
builds otherwise use production trust: an updating native artifact requires
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

Configure `install.<platform>.methods` in the app’s `app.json`; use
`recommended` to name a configured method. See [LLP 1030.003 D6a](llp/1030.003-continuous-release-loop.rfc.md#d6a--the-standard-install-page)
for the schema and examples. A method links to an already available installation
flow; configuring it does not build, sign, upload or verify a native app.
`terminal` displays a command and offers Copy, never executes it.

On a Mac, the development server adds a dev-only iOS method to the page. It lists
available iOS Simulators and reachable paired devices, then **Build and install**
runs the existing `--ios --sim` or development-signed `--device` build, installs
the app, and opens it on the same development URL. Simulator builds work from a
loopback server. A physical phone must be unlocked, paired, in Developer Mode and
able to reach the server, so start without `--loopback` and use a printed LAN URL
for that case. The request is same-origin and protected by a random token that
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
[LLP 1029.000](llp/1029.000-rust-development-reload.rfc.md) for the boundary and
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

[Update Lab](apps/update-lab/README.md) composes one Rust probe with one
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

[Messages stress](apps/messages-stress/README.md) compares a bounded synthetic
history page with deliberately eager construction while typing and streaming
updates. [Completion Storm](apps/completion-storm/README.md) holds and releases
real local HTTP requests, including failures and replies to a departed screen.
[Markdown stress](apps/markdown-stress/README.md) exercises the shipped parser
and reader components with large documents, huge individual blocks and reflow.
These are opt-in developer workloads; none claims automatic virtualization or
120 Hz performance. [LLP 1041](llp/1041-graceful-overload.rfc.md) specifies the
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
