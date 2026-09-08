# Exact

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
| `apps/caltrain/` | The v1 app: `app.contract`, its Rust data crate, and its wasm crate; the end-to-end fixture. | — |
| `gpu/` (`exact-gpu`) | The GPU canvas: a `Surface` trait against wgpu, a per-app module loaded on demand (a dylib on macOS, a second wasm on the web) after the first pixel; the same Rust renders on Metal and on the browser's WebGPU. `apps/caltrain/gpu` is the line map and the aurora. `gpu/reflect` (`exact-gpu-reflect`, naga only) reflects every `.wgsl` in a GPU crate's `build.rs`: bindings, struct layouts, vertex inputs, and entry points generated as Rust, the WGSL as the one declaration authority. | LLP 1009 |
| `host/apple/` (`exact-apple`) | The Apple host: runner + kernel as a static library with a C ABI, the kernel's layout with CoreText measurement through a callback, `exact-motion` as the executor, typed batches; `macos/` is the AppKit presenter and `ios/` the UIKit one (SwiftPM, sharing `swift/`). `node host/apple/build.mjs --run`; `node host/apple/build.mjs --ios --run` on a simulator. | LLP 1008 |
| `host/linux/` (`exact-linux`) | The Linux host, the first that paints: runner + kernel natively, cosmic-text measuring and painting from one cache, `exact-motion` as the executor, the kernel tree drawn by one walk over a backend — vello on the GPU (the main one), tiny-skia on the CPU (the fallback and the pixel oracle) — onto DRM/KMS dumb buffers with evdev input, or into a buffer with no display (the agent API, screenshots, the smoke; on macOS too). Pure Rust, no system library. `cargo build --release -p caltrain-linux`. | LLP 1015 |
| `host/web/` (`exact-web`) | The web host: runner + kernel in wasm over the real DOM, CSS computed once from the kernel's rows, springs lowered to frames the browser plays, a no-`unsafe` ABI, ~150 lines of glue, a headless-Chrome smoke, the motion parity harness, and the dev loop (`node host/web/dev.mjs`, edit → present ~20 ms). | LLP 1007 |
| `vendor/taffy/` | Taffy 0.9.2 plus two Exact patches. | `vendor/taffy/EXACT-PATCHES.md` |

All four surfaces run the app; `QUEUE.md` is the ordered list of what would
make sense to do next.

## Open the same development URL on Apple hosts

Start `node host/web/dev.mjs` and open a printed URL in your browser. Build
and launch the app's native client with that same address:

```sh
node host/apple/build.mjs --run --url http://127.0.0.1:8765/
node host/apple/build.mjs --ios --run --url http://127.0.0.1:8765/
node host/apple/build.mjs --device --run --url http://192.168.1.20:8765/
```

For a phone, replace the example with the server's reachable LAN or HTTPS
URL. Device builds require a connected, provisioned phone. `--url` overrides
`EXACT_DEV_PLAN` for this launch; it does not change the app's production origin.
An external app uses these commands with `EXACT_APP_DIR` set as usual.
Plans and assets reload through the native URL loader. Admitted TypeScript
module clients also reload logic on web/macOS/iOS; native Rust logic remains
binary-bound. Once built, an agent can drive
the same URL with `node scripts/agent.mjs macos --url http://127.0.0.1:8765/ tree state logs`
(also `web`, `ios`, and `linux`; Linux fetches once at launch).
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
`sources[source](args, store, storage)` without casts. `npm ci` installs the pinned `tsc`.
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

Native module clients can supply a `Module` factory to `exact_apple::host!`
(the sixth argument) and apply an `ExactGeneration` containing an `ExactModule`
through `ExactApp.applyGeneration`. All sessions prepare before any commit;
changed logic re-asks resources while preserving compatible slots and clock.
Initial module loading happens after first pixel. The binary's app identity
and grants must match; Rust-only clients refuse module replacement. Pairing
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

Run the ordinary build scripts and `node host/web/dev.mjs --app <name>` with
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
`node host/apple/build.mjs <app>-apple --bundle` and open the printed `.app` once.
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
node scripts/caps.mjs                                                   # caps
node scripts/boot.mjs                                                   # boot graph
```

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
