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
bundles, and the iOS device bundle where Xcode's iphoneos SDK is installed.
`exact setup --check` runs Ibex's own offline resolver validation over that
same set.

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

Snapback4 consumers use release **0.4.33**: the CLI is pinned in `bun.lock`;
Cargo pins the device and its client to the matching release source commit
`3653cae823145170df3e720f5e321bd1b5b4d79d`. `snapback4/` is one client for an
app's Rust and TypeScript on every host ([its README](../snapback4/README.md)):
the protocol in Rust without I/O, the native device, the web's wasm, and the
TypeScript driver an app mounts with `typescript.sources`. The client is its
own Cargo workspace, `snapback4/`; its lock carries the private source, so
root Cargo commands need no Snapback access. Direct Cargo commands use
`--manifest-path snapback4/Cargo.toml`; apps depend on `snapback4/` by path.

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

`font-family` accepts a CSS family-list string from state or data on web,
macOS, iOS and Linux. A change resolves the new installed or generic family
and remeasures affected text. Bundled faces are declared at compile time on
every host; a runtime string does not load a font file.

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

Contract carries `aria-disabled` on controls and elements with a role, including
groups and inline links. It is a boolean accessibility state: focusable elements
remain in the Tab order, and `press` still runs. The app guards an unavailable action.
`aria-disabled=true` also marks its focusable descendants, including inline
links, as unavailable to accessibility; an explicit false on a descendant
overrides the ancestor. HTML
`disabled` retains its separate focus and input behavior; see
[focus order](contract-grammar.md#focus-order-tabindex).
On Apple hosts, a projected tab or pressable node reports the same accessibility
state through its native segment, UIKit bar item or title, or AppKit menu item.
UIKit menu rows and alert actions remain selectable but read as enabled: their
public disabled states also stop selection.

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
mixed into JSON, including argument and output-write failures, and nothing is
written to stderr: advice the human form prints there (a resource fed by a live
clock, a game's stale surface arguments) is among the warnings. `bun exact.mjs
contract build app.contract --json` appends `app.ts`'s type-check errors to the
same array, one per TypeScript diagnostic (`id` `typescript-TS<n>`). Exit codes are
0 for success, 1 for compilation/I/O failure, and 2 for invalid arguments.
`-o <file.plan>` writes the same plan bytes in either output mode.

`build --map -o <file.plan>` also writes `<file.plan>.map.json`, keyed by
SHA-256 of the plan bytes. Each plan node has its original file and range,
component call-site chain, and one binding per style row: its base value's
origin (`own`, `class:<Name>`, or `tag`) and, where an author wrote it, `at`
and `attr`; and `states`, one `{state, origin, at}` per state part that
writes the row: `:active`'s (on the press paint row it lowers to), `:hover`'s
and `:focus-visible`'s (LLP 1121 D8, LLP 1122); a conditional class's
(`class=(c ? A : B)`) says no `at`, as its base row says none; a row a
`:disabled` or `:checked` part folds over its base says the base's attribute
as its base and each folded part in `states`. A row only a state part
writes, a press row among them, has no base fields. Each source is listed
with its digest, and a Contract source with its `home`, as its components'
identities name it (a param's full name is `<home>#<component>.<param>`). Slots, derives and actions retain their declarations,
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

## Run a terminal app

`cargo build --profile host-dev -p harness-terminal -p llp-terminal` builds
`target/host-dev/harness` (inline; `--fullscreen` for the alternate screen)
and `target/host-dev/llp [corpus-or-document]` (full screen). Both embed their
terminal entry as a compiled plan. Rebuild after editing the entry or an
import; launching the binary needs no Contract source or compiler work.
`exact-terminal <entry.contract>` remains the source-driven development
host. All three accept the headless operations and `--size COLSxROWS`.
Interactive terminal I/O runs on Unix only; Windows supports headless operations.

The harness's inline transcript is transferred to scrollback in bounded
batches, retiring each only after the terminal prints it. A large burst
still produces every entry immediately; fullscreen retains the full
transcript for scrolling.

Vertical `list virtualized=true` uses the shared collection window on the
terminal. Scroll and resize report the viewport and measured rows; Tab and
arrow navigation reach logical rows beyond the mounted window. A focused
row stays alive when scrolled away. Closed dialogs release their virtual
row views while retaining their list index and position. The LLP reader's
document tree uses this path, initializing its index on first open.

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

A standalone macOS app treats `kill -TERM <pid>` as an orderly quit: its
`beforeunload` handlers can cancel, pending storage gets the same five-second
hold, and each session's native module is destroyed before exit. Repeated SIGTERM
requests share that hold's original deadline. SIGKILL cannot run this cleanup.

A one-window Mac app ends when its window closes. An app with work the person
expects to go on without the window (a server, a sync), as Mail and Messages
have, sets `"keepRunning": true` in `app.json`'s `host.macos.window`. ⌘W,
File ▸ Close Window and the red button then hide the window without asking
`beforeunload`, and the app keeps running with its session and native module
alive. A Dock click or `open -a` shows the window again as it was, and a
document routed to the app shows it too. While the window is hidden,
`exactPage().visibilityState` is `hidden`, as for a minimized window, and it
is `visible` again when the window shows. The close and the reopen are also the
web's back/forward cache: `exactPage().persisted` is `true` from the close (a
`pagehide` with `persisted`) until the reopen, and `exactPage().restores`
counts the reopens (each a `pageshow` with `persisted`). A minimize, ⌘H, a
covering window or a locked screen changes neither. So an app that opens
something new on a reopen, as a Mac app makes a new window there, keys a task
on the count: `task fresh when page.restores > 0 key=page.restores`. ⌘Q quits as before, and the app's own
`close()` still closes the window and ends the app. A `navigate-new` app
(`launch_handler`) already outlives its last window, and the key changes
nothing there. The agent's `state.documents.windows[].visible` says whether
the window is showing.

A native module whose work must finish before the app exits (a child process
to stop, a socket to flush) overrides `willQuit(done:)`. On every orderly
quit (⌘Q, the app menu, an Apple Event, SIGTERM, the last window closing),
once no `beforeunload` has kept the app, the host calls it once, on the main
thread (a module first made during the hold, when it is made); start the work off the main thread and call `done` from any thread
when it has finished. The quit waits for every module's `done` and for
pending storage, five seconds at most for all of it together, and the window
stays responsive meanwhile; then `destroy()` runs, whether or not the work
finished. A repeated quit neither asks again nor extends the deadline. Letting
`done` go uncalled counts as calling it. iOS and tvOS have no quit and never
call it. `destroy()` runs synchronously on the main thread with no deadline,
so do not wait in it: force-end there whatever the deadline cut short, or a
child process outlives the app.

```swift
override func willQuit(done: @escaping () -> Void) {
    helper.terminationHandler = { _ in done() }
    guard helper.isRunning else { return done() } // a second `done` is ignored
    helper.terminate()                            // SIGTERM; it exits in its own time
}
override func destroy() {
    // After the deadline, finished or not: end a helper that ignored SIGTERM.
    if helper.isRunning { kill(helper.processIdentifier, SIGKILL) }
}
```

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

For URL schemes, `protocol_handlers` in `app.json` uses the Web App Manifest's
`{ "protocol": "web+notes", "url": "/open?u=%s" }` entries. A native Apple
bundle registers each declared scheme. `%s` is the complete incoming URL
encoded as a URI component, so the scheme, query, and fragment survive;
`searchParam(top(nav), "u")` recovers it in the `/open` route. A cold link is
the launch location; warm links arrive one at a time at the navigation root's
`navigate` handler. The app must declare that root and an accepting route.
The web manifest carries the same entries; browser registration is subject to
the browser's supported schemes and installation behavior. Linux's CLI URL
argument retains the ordinary location conversion because it does not
register OS protocol handlers.

For packaged-only Apple registration, add `"distribution_only": true` to an
entry, for example `{"protocol":"notes","url":"/open?u=%s","distribution_only":true}`.
Archives and `--distribution` builds include it; local builds, including
`--bundle` and `--release` alone, omit it from both the registered schemes and
native handler templates. The default is `false`. This option does not filter
the web manifest or explicit `host.<platform>.urlSchemes`.

`http` and `https` are the web's own schemes. An app declares both or
neither: each once, with the same `url` and the same `distribution_only`.
`readManifest` refuses any other form, so every build does, and
`host.ios.urlSchemes` and `host.macos.urlSchemes` refuse both. The pair makes
the app a web browser:

| host | what the pair does |
| --- | --- |
| macOS | a second URL type, `Web site URL`, role `Viewer`, and a `Web page` document type (`public.html`, `public.xhtml`; `Viewer`, rank `Alternate`), without which macOS lists no browser. The person chooses the app in System Settings › Desktop & Dock › Default web browser; the app never asks to become the default. A link arrives at the template. An HTML file that Finder opens in the app arrives there too, as its `file:` URL (`/open?u=file%3A%2F%2F%2F…`), unless the app's own `file_handlers` accept it. The web-page type is not offered by the app's pickers and does not widen what `drop` accepts. |
| iOS, tvOS | refused: the bundle registers neither, and the build says so. A default browser on iOS needs Apple's managed `com.apple.developer.web-browser` entitlement. |
| web | dropped from `manifest.json`: HTML's protocol handler safelist has neither. |
| Linux, Windows, Android | not registered, as no protocol handler is there. |

With `"distribution_only": true` on both entries, only an archive or a
`--distribution` build is a browser. A local Mac build then registers neither
scheme, writes no `Web page` type, and does not join this Mac's browser list.

A file chosen in the app's own picker (`showOpenFilePicker`,
`showSaveFilePicker`) joins File ▸ Open Recent — a save once it is written —
and becomes the window's document, as a routed one does. A path from the command line, from Finder, from ⌘O, or from a link inside
a document all arrive at the same place: the app's `open-file` node
(LLP 1033 D3). In a Mac browser, an HTML file from the command line or Finder
that `file_handlers` do not accept is a link instead, at the `https` template. One handed over at launch arrives before first pixel, before
app storage is ready and before a TypeScript data module has loaded: a send its
`change` makes waits for both and then runs, pending meanwhile, rather than being
refused (studio diary R14; notes diary). When nothing takes the path, the
host says why — no `open-file` field, or the `change` the app refused, and the
refusal — on stderr and in the journal. `exact uninstall <app>` takes both halves away.

Widget entries are top-level `app.json` `widgets` rows. For example:

```json
{"kind":"next-trains","families":["systemSmall","systemMedium","accessoryRectangular"],
 "component":"NextTrains","props":"NextTrainsProps","refresh":"nextTrains",
 "preview":{"station":"Mountain View","departures":[]}}
```

Optional `title` and `description` strings supply the widget gallery's labels;
each defaults to the humanised `kind` when omitted.

`component` and the `props` shape are declared or imported by `app.contract`;
every component prop must match a field of that shape. `preview` supplies all
fields, with their declared types. `refresh` names a data-source function taking
an epoch-millisecond number and returning a timeline: records with `date` (epoch
milliseconds) and `props` (the declared shape), in that field order. The plan's
`snapshot_refresh` header selects its typed source row; the render has no resource
for it. A data bake checks an immediate refresh answer against that signature at epoch
zero; pending answers are checked by the source declarations. TypeScript checks
the generated `Sources` type, including async refreshes. Compile entries once
with `contract::snapshot::entries(app)`, then pass that map to `source_plan`,
`check_refreshes` and receipt generation. `snapshot::bake_app` takes separate
data instances for the app and refresh, so refresh state cannot affect the app
first frame. The TypeScript and replaceable Rust producers use the same check.

On iOS, a TypeScript refresh runs under Hermes from the module bytes in the
app's pinned App Group generation (or the extension's embedded cohort before
the app first publishes it). The receipt admits both the entry plan and bytecode.
It can return immediately or await `fetch`
and `storage.fs` reads from `app:/`. File writes, SQLite, document handles and
native modules are unavailable in stage 1; `secret.keep` values are read from
the shared Keychain only during refresh. The extension never persists store
writes. The app reloads widget timelines after a successful write readable by a refresh.
Refresh has a 900 ms deadline starting when it begins, including boot. Each JS
turn gets the time remaining to that deadline, rather than the app's 100 ms
per-call budget. This leaves 100 ms below LLP 1126 D8's one-second
line for completion and teardown. Failure or timeout uses the last good timeline
cached in the App Group for this kind and generation, only while the app's store
has not changed since it was cached (a store write, such as an account switch,
expires it), or else an “Open app” placeholder,
and schedules a retry no sooner than 15 minutes later. Rendering and gallery
previews do not load Hermes or read the store.

Live Activity entries are top-level `live_activities` rows, each with a `kind`
(LLP 1126 D9). The packaging reads two flags: `push: true` for an activity the
app's own server advances, which signs `aps-environment` into the app (never
the extension, which reads no token), and `frequent: true` for one that needs
the frequent-update budget (`NSSupportsLiveActivitiesFrequentUpdates`). An
entry of either kind, a widget or an activity, gives the iOS build its WidgetKit
extension, the shared `ExactCore` framework, the App Group and the Keychain
group; an activity entry alone adds `NSSupportsLiveActivities` to the app's
`Info.plist` (a widget-only app has no such key). An app with activities alone
ships the extension with an empty widget bundle until its activity
configuration lands (LLP 1126.001 WP10). The entry's root, state shape and
preview are stage 2's bake (WP8).

A device build of an app with entries embeds two profiles, the app's and the
extension's (`<id>.widgets`), of one team, App ID prefix and distribution
method (development, Ad Hoc, enterprise or App Store), each granting the App
Group `group.<id>` and the Keychain group `<prefix>.<id>` (Keychain Sharing;
the team wildcard `<prefix>.*` grants it), the app's also `aps-environment`
(`development` on a development profile, `production` on the others) when an
activity takes pushes. A team wildcard profile grants no App Group, so each is
a profile for its exact App ID; `build.mjs --device` refuses before building
anything, naming for each App ID the profile that was close and the condition
it failed (expired, not this phone, another team or prefix, the other method),
and the capabilities to register and profiles to install. `EXACT_PROFILE` names
the app's, under the same conditions; the extension's is found among the
installed profiles by its id. An archive (`--archive`) matches the extension's
profile to the named one's method, and an Ad Hoc pair must cover the same
devices.

On macOS, `bun host/apple/build.mjs --run` packages the app's widgets in
`Contents/PlugIns`. The Mac keeps `systemSmall`, `systemMedium`, `systemLarge`
and `systemExtraLarge`; if no families remain it builds no extension. Live
Activities do not create a Mac extension. The extension uses the same runtime
framework and snapshot driver, in its own sandbox with outbound network access.
The app keeps its existing sandbox policy and loads the reload arm on demand.

Development Mac widgets need an Apple Development identity (`EXACT_IDENTITY`
can name its SHA-1 or full certificate name). Without one, the build prints a
notice and builds the ordinary app ad hoc, with no extension; agent drives,
smokes and metrics remain available. Selecting a Developer ID identity for a
development build takes the same ad hoc path. Distribution widgets use the
Developer ID Application identity selected for release.
Their container is `<TEAMID>.<app id>.widgets`, separate from iOS's group.
Shared secrets additionally need matching macOS profiles for the app and
`<app id>.widgets`, granting that container (or `<TEAMID>.*`) and the restricted
Keychain group `<prefix>.<app id>` (or `<prefix>.*`). `EXACT_PROFILE` can name
the app's profile; an iOS profile is ignored with a notice. Both are embedded
when available. Without them, entries marked `"requiresSecrets": true` show
the “Open app” placeholder;
set this on each refresh that reads account data, leaving public entries false.
Those entries neither read the app's login Keychain nor reuse cached account content.
The first provisioned app launch moves its login-Keychain items to the shared
data-protection Keychain, deleting each old item only after the copy succeeds.
The move is one-way: an unprovisioned build cannot read the moved items.
Unreadable items remain for the next launch; a migration error leaves I/O bound.
Caltrain and Shelf need no shared secrets. See LLP 1126.001 WP16 and §5 Q5.

The render's props slot is named `snapshot_props`; find it by name, since a
`path(...)` link also needs a router slot. Its initial value is the preview; reached bindings cannot use that reserved name.
The bake writes `plans/<kind>.plan` beside `app.plan`. The envelope's `plans`
map indexes these authenticated assets in the app's compatibility cohort.
Kinds, families and gallery labels are binary compatibility inputs; views and preview props
are plan contents. A running host refuses a snapshot plan at decode, and a
snapshot reader requires its exact kind. A widget also requires a refresh
signature; an activity has named placements and no refresh source.

Live Activities are top-level `app.json` `live_activities` rows:

```json
{"kind":"departure","component":"DepartureActivity","state":"DepartureState",
 "preview":{"station":"Mountain View","at":1787915700000},"push":true}
```

The root's props match every field of `state`. Optional `title` and `description`
follow the widget label rule; `push` and `frequent` are Booleans, defaulting to
false. Kinds are unique across both entry lists, ignoring case. Unknown keys fail
the bake with an edit. Capability flags and labels require a new binary; state
shapes, previews and views ride the bundle.

Declare the placements in that one component's view with `view slot="lockScreen"`,
`view slot="compactLeading"`, `view slot="compactTrailing"`, `view slot="minimal"`
and `view slot="expanded"`. Under `expanded`, declare four `view` children with
`slot="leading"`, `slot="trailing"`, `slot="center"` and `slot="bottom"`. Each slot
is required once. Put its layout attributes and content on that slot's view;
`expanded` only groups its regions. See `apps/caltrain/activities.contract`.

The activity plan has one `snapshot_state` slot and an `activity_slots` table
mapping each placement name to a node site under a synthetic container. One
session serves the activity. For each slot, the driver sets the session's viewport
to the region's proposed content size, lays out, takes that slot's subtree and
measures its `fit_content_height`. `snapshot_viewport` thus reads the current
region's size; no app-visible slot selector is injected. The other slots' frames
are not a shared surface layout. Rendering remains read-only, with the same
clock, request, input and handler refusals as widgets. `time` and `progress`
remain platform live views.

A development build with `--widget-gallery` also answers activity addresses:
`bun scripts/agent.mjs ios activity departure --placement expanded --state '<json>' tree "screenshot activity.png" state`.
The same command works on `macos`. Placements are `lockScreen`, `compactLeading`,
`compactTrailing`, `minimal` and `expanded` (all four expanded regions together).
Omit `--state` to use the entry's preview; `--scheme light|dark` selects its scheme.
`clock` advances the app's agent clock and the next read renders at that time.
These are gallery renders; the async simulator test checks the system's archive.

The extension renders every activity through one `ActivityConfiguration` for
the envelope type (`host/apple/widget/Activity.swift`, LLP 1126.001 WP10), a
thin adapter over `ExactSnapshot`'s `ActivityResolver`: the attributes' `kind`
names the plan, `plans/<kind>.plan` in the generation the attributes name,
which is the one published under that name in the installed cohort or the
bundle's embedded plans when their identity (`ExactWidgetGeneration`) makes
that generation; any other generation is refused, the activity shows "Open
<app>" until the app publishes again, and nothing is rendered in its place.
The state is the envelope's when its shape digest is the registry's and the
plan decodes it (then kept as the activity's last good state), else the last
decodable state kept for the activity id (shown dimmed, the driver's outdated
presentation; the system's `isStale` is the system's own), else the preview,
redacted as a placeholder. A render is keyed by the activity, its generation,
the state's bytes and every fact the session reads (the text size, Bold Text,
contrast, transparency, the scheme, the display scale, the locale and time
zone); the eight placements of one push share it. A placement's view answers
the system's size proposals from the kernel (`ActivityRender`): the bake
positions each placement's root absolutely at its region's top left, so in
the region's viewport CSS 2.1 §10.3.7 gives a `width: auto` root the
shrink-to-fit width (an authored `width` or `min-width` is kept, past the
region too; a percentage resolves against the region; `margin: auto` is
nothing; `flex-basis` does not apply, the root being no flex item), and the
Lock Screen and expanded bottom roots, which the system sizes, take the
region through `right: 0`. That one layout is measured and drawn: the region
reports the root's margin box, a paragraph the region wrapped settles to its
widest ink line, a content the region cannot hold is cut at the region, and
each view draws the layout its size was measured from, whatever region's
proposal last laid the session out. The Lock Screen placement sits inside the
system's content margins or, where the system gives none, the standard
layout margin for Live Activities on the Lock Screen, 14 points (Human
Interface Guidelines, Live Activities: "The standard layout margin for Live
Activities on the Lock Screen is 14 points"); its root's `background-color`
is the `activityBackgroundTint`, its `background-image`'s gradient layers
paint full bleed behind the margins over that tint, and both unsaid leave the
system's material. A Live Activity's content is account content as a
widget's is (D8): private on a locked phone but for a node that says
`data-privacy="public"`, which Caltrain's placements do, a train's departure
being nobody's account. A link in the Lock Screen or an expanded region is a
`Link`; the first link of any placement is its URL. The development gallery
shows the eight placements with the Island's geometry of one device (the
iPhone 17 Pro's, `IslandGeometry`), its expanded regions allocated together
as ActivityKit arranges them.
The sends that start, update and end an activity are LLP 1126.001 WP9.

The bake writes `plans/activities.json` and `activities/<kind>.schema.json` plus
`activities/<kind>.d.ts` (exporting `State`, `Attributes` and `ContentState`). The
state uses JSON Schema 2020-12 with required fields and no additional properties.
The shape digest hashes its canonical schema, including each record's field
order. The universal envelope's attributes are `{kind, generation, shape}`;
its content state is `{shape, state}`, where `state` is a JSON value, not an
encoded JSON string. `generation` names the immutable publication digest. The
preview and sends share a conservative 4096-byte JSON budget, including both
shape digests. Numbers reserve the longer of serde_json's and Foundation's
shortest Double spellings, capped at 24 bytes; the bake also covers a preview's
original integer spelling before Double decoding. Both reserve two bytes for
`\b`, `\t`, `\n`, `\f`, `\r` and slashes, and six for the other U+0000–U+001F
characters and U+2028/U+2029. Other characters and JSON punctuation use their
compact UTF-8 size. A refusal reports the budgeted bytes and asks for shorter
state. Shared Rust/Swift fixtures pin encoder spellings, escape counts, admission
of a 60-point route and 150 small integers, and refusal at 4097 bytes.

An activity pins its named generation within the installed compatibility cohort,
even after `current` moves. Publications currently keep every generation. The
cache has one file per activity id. The caller supplies the pinned plan's decoder:
a state it cannot decode falls back to its last decodable state with the driver's
outdated flag. Without a decodable kept state the placeholder has no outdated
flag. A failed write returns an error to the caller. ActivityKit's `isStale` is a separate system
fact. Rendering across a compatibility-changing binary update remains subject
to LLP 1126.001 §5 Q6's pending ruling.

`bun host/apple/build.mjs --watchos --no-install` builds a watch simulator
companion inside the iOS bundle. `--watchos --run --sim <iPhone udid>
--watch-sim <watch udid>` installs it on an existing, booted pair and opens its
placeholder gallery. Add a complication through the watch face's editor.
The watch extension keeps the accessory families (`accessoryCircular`,
`accessoryRectangular`, `accessoryInline`, `accessoryCorner`); iOS drops
`accessoryCorner`. An optional `watchFamilies` on an entry overrides its watchOS
families; `families` still names its iOS families. An app with `app.ts` has all
watch entries omitted with a message, including its Rust refreshes, until the
watch Hermes gate passes (LLP 1126.001 §5 Q3). Rust-only apps' entries
refresh against the watch's own App Group and store. No account transfer is
performed. The watch app is a placeholder gallery until the live driver lands.
For `accessoryInline`, emit exactly one `text` or `time` leaf plus at most one
optional image. Only that leaf and image survive the inline archive; a
container's background and layout do not. The bake evaluates the preview at
225 × 26, 168 × 11.5 and 13.5 × 13.5 pt; at runtime the driver checks the actual
size and props.
A violation refuses the bake with an edit, or logs the entry name and renders
“Open <app>” at runtime, including in the gallery (LLP 1126 D4). See
Apple's [inline family](https://developer.apple.com/documentation/widgetkit/widgetfamily/accessoryinline).

The `snapshot` columns on the style and prop rows declare admission. Unsupported
rows, scrolling, sticky positioning, timers, requests, native modules and handlers
fail with an edit. The author declares no resource; the bake synthesizes the
`exactViewport` resource for the size inject. The one fact a render reads
is the family's size, through the reserved inject `snapshot_viewport`: write
`inject` with `snapshot_viewport: <shape>` on the entry's component (or one it
uses; every injecting component names one shape), and the entry's plan
declares `exactViewport()` for it and provides it. `width` and `height` are the
family's content size in points, inside the system margins, given at boot and
again when the family's size changes; the preference fields are a still page's.
Choose a layout by them as `docs/recipes/responsive-columns.md` does for a page
(a Lock Screen row is under 100 pt tall; a medium widget is over 250 wide), so
one entry serves every family it declares. The app's own plan is untouched: an
unused component's inject is not demanded by the app's view. A `performanceNow()` call is refused everywhere, including
inside `time` and `progress`; pass instants from refresh for platform time
presentation. A variable's name imposes no restriction. A class is a style name
or `class=(condition ? A : B)`. Only literal font families reached through the
entry are bundled; a dynamic font string selects installed or generic families.
Images use a bundle-relative path, an `app:/` path written by the app, or a
`symbol:` source. Computed paths need a fixed local prefix; refresh downloads
remote images. A live `time` (`relative`, `timer`, `countdown`) must own its
text paragraph, so the widget can reserve its width; calendar formats may be
inline. Links must be their own elements, not runs among other paragraph text.
Snapshot handlers are refused; use a link to open the app.

Snapshot accessibility admits labels, roles, hints, heading levels and hidden
content. Live announcements and control-state accessibility rows are refused.

Snapshot paint uses document order and the system colour scheme. Use a condition
or `display: none` to hide content; `visibility` and `z-index` are refused.
Borders have one width, colour and solid, dashed, dotted or `none` style on all sides.
Radii are px or percentages; percentages on non-square boxes produce ellipses.
Radius calculations are refused. Overflow is the same on both axes: visible,
hidden or clip. Raster images author both width and height, and `object-fit` is
fill, contain, cover or scale-down. An entry declaring
any small or accessory family has at most one link; entries declaring only
medium, large and extra large families may have several.

Text aligns start, center or end; use `line-clamp` for truncation. Give an inline
highlight its own box. Colours are sRGB, system roles or named assets; wide-gamut
and profiled colours are refused. Gradients are nonrepeating linear or radial,
interpolated in sRGB or srgb-linear, with strictly increasing stops. Stop order
must hold at every box size; use a single unit when mixed units could cross.
Restricted paint values are literals or choices of literals, so their admission
is checked at bake.

App icons come from `app.json`'s Web App Manifest `icons` array: Apple uses
the first square raster of at least 512 px. To select an
[Icon Composer project](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer),
set `host.ios.icon` or `host.macos.icon` to its app-relative `.icon` directory,
for example `"host": { "ios": { "icon": "art/Primary.icon" }, "macos": { "icon": "art/Primary.icon" } }`.
Each platform selects independently; tvOS keeps its asset catalogs and does
not inherit the iOS selection. There is no `.icon` discovery. The selected
project must be inside the app and cannot coexist with a primary
`AppIcon.appiconset`, or a set with the project's name, in that platform's
catalogs; other sets stay alternate icons. Missing paths
and paths that are not `.icon` directories fail by name. The project and its
layers compile in the same `actool` pass as the app's catalogs; changing them
requires a new native build. `icons` still supplies the web and the native
raster fallback when the platform has no `icon` selection.

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
module. This field applies only to macOS bundles. A file written before the
build (a build stamp, a channel name) goes in such a tree under `Resources/`:
it is copied before the bundle is sealed, so it is inside the seal. Only code
goes under `Helpers/`, where `codesign` refuses a file that is not.

A helper that needs a hardened-runtime exception (a JavaScript runtime's JIT,
say) declares it as a grant beside the app's others, naming the exception and
the one executable it is for, by its path under `Contents` (LLP 1069.008 D9):

```ts
export const grants = `
runtime.allow-jit Helpers/node/bin/node
runtime.disable-library-validation Helpers/node/bin/node
`;
```

The exceptions are `allow-jit`, `allow-unsigned-executable-memory` and
`disable-library-validation`; `exact release` signs that executable with
`com.apple.security.cs.<exception>` and nothing else in the bundle with it.
Only helpers take an exception: a line naming `app`, the app's own executable,
is refused (deferred until it has a consumer). A target must be a Mach-O
executable inside a `host.macos.resources` destination, not reached through a
link; a nested `.app`'s executable gives its grant to that
bundle. `mac --bundle` refuses one that is not, and other hosts ignore the
lines. Adding a line changes the grant ceiling, so the deploy classifier calls
for a binary release.

Mach-O code in content-addressed `assets/`, `deck/` or `shaders/` is refused
when assembling a Mac bundle: place helpers in `host.macos.resources` instead.
The release signs GPU modules before baking their digests and preserves those
signatures through packaging. iOS archives package primary and named GPU modules
as signed frameworks; development builds keep their loose dylibs.
Combining `--device --archive out.ipa --embed` produces both the IPA and the
embed directory without a connected phone. The embed directory contains the
static library, C header, assets, shaders, compatibility and capability metadata,
receipt, and signed GPU frameworks (primary and named modules). Copy those
frameworks intact into the embedding app's `Frameworks/`; their binaries must
retain the baked digests.

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
(`--format-locale en-KR` gives `formatLocale` a Region of its own; unsaid, it is the locale)
or `open({host, seed: 42, locale: "fr-CA", timeZone: "America/Toronto", epoch: "2026-09-21T14:13:20Z"})`;
a test file writes them as launch lines (`epoch "2026-09-21T14:13:20Z"`, `time-zone "America/Toronto"`,
[authored tests](contract-grammar.md#authored-tests)), which override the flags.
A drive can fail fetches by URL prefix (LLP 1103): `--fail-fetch <prefix>` (repeatable) arms one before the
first data load, `fail fetch <prefix> [times <n>]` and `pass fetch <prefix>` arm and clear one mid-drive (a form of `prefer`:
`{"op":"prefer","faults":{"fail":…,"times":…}}` or `{…{"pass":…}}`), and `state.faults` lists each prefix's `times`, `left`, `hits` and
`armed`. Native carriers pass the launch table as `EXACT_AGENT_FAIL_FETCH`, web pages as `?failFetch=`, one
`<prefix>[\t<times>]` line a fault, read only in agent mode; a production build ignores both. `--fail-fetch` with `--test` arms every test.
A Mac agent drive never takes the focus: the app is an accessory, never activated (a `type` makes its window key
without activating it), so AppKit draws its controls as in an inactive app. `EXACT_AGENT_ACTIVATE=1`, which only
`scripts/measure-controls.mjs` sets (never a default), activates that one run, and the app that was in front is put
back when it exits, however it exits (LLP 1121 stage 9 F2).
Seeds are whole numbers from 0 through 2^32 − 1, the range every host draws (LLP 1116.005 D5); an epoch is an ISO date, Unix milliseconds, or `now`: the machine's clock,
read once by the driver at launch, for a drive against a live backend on real time (`snapback4 dev`). Native carriers pass
`EXACT_AGENT_SEED`, `EXACT_AGENT_LOCALE`, `EXACT_AGENT_TIME_ZONE` and `EXACT_AGENT_EPOCH`
(milliseconds), and `EXACT_AGENT_FORMAT_LOCALE` when given; direct agent launches can set these too. Web agent pages accept
`?agent=1&seed=42&locale=fr-CA&timeZone=America/Toronto&epoch=1790000000000` (and `&formatLocale=`).
The driver supplies its own defaults unless an option (or `open({env: ...})`)
overrides them. `clock +N` moves the date (`epochAtZero + performanceNow()`); `state.time`
reports all five facts. Before the first host report, the runner
also supplies usable `en-US`/`UTC` and seed 0. Ordinary launches draw their seed
from secure platform entropy. A development reload retains that launch's seed.
Linux takes its locale from the first nonempty `LC_ALL`, `LC_MESSAGES`, or `LANG`, and
`formatLocale` from `LC_TIME` by POSIX's precedence (`LC_ALL`, then `LC_TIME`, then `LANG`; with
none set, `formatLocale` is the locale),
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
``` `ledger` answered outside its shape: field `days`: field `transactions`: field `cents` is not in the shape ```,
which the agent's `state` shows under `failed` while the resource keeps its last value.
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
| `fetch`, `Headers`, `Response` | Grant-checked; `signal` aborts. A `body` is a string or an `ArrayBuffer` or view, sent as its bytes (an upload of `storage.fs.readFile`'s bytes); as Fetch does, a body on a GET or HEAD, or a view on a `SharedArrayBuffer` or resizable buffer, rejects, and a detached buffer sends no bytes. `exactBodyFrom: "app:/tmp/…"` sends an app file instead: the host reads it when it sends the request, under `fs.read` as `readFile` is, at most 64 MiB, and the bytes never enter JavaScript, so a 2 MB upload costs the answer's step nothing; with `body`, or on a GET or HEAD, it is a `TypeError`, and a missing file, a denied path or one over 64 MiB rejects the fetch (`FetchError`, kind `Refused`, naming why) before anything is sent, as does a host with no app files (a drive with no scratch store) or none built yet (Windows), with kind `Unsupported`; no Content-Type is added (LLP 1108 D6 R2). An older binary's prelude would send it as an empty body, so a native app with a TypeScript data module carries `typescriptRuntime` in its compatibility id: the first build after this change gives every such app a new id, and each needs a new binary once. `redirect` is Fetch's: `follow` (the default), `error` (a redirect rejects the fetch), or `manual`, which answers with the 3xx itself; natively its `Location` header is readable (an OAuth step whose redirect names an app scheme), while a browser gives its opaque redirect (status 0, no headers); any other word is a `TypeError`. An older prelude would ignore it and follow, so it moves `typescriptRuntime` to 2: such an app needs a new binary once more, and a cohort on an older binary gets no bundle that relies on it. A `Response` has `status`, `ok`, `headers`, `text()`, `json()`, `arrayBuffer()`; no `Request`, `Blob` or `FormData` |
| `structuredClone` | No transfer list |
| `TextEncoder`, `TextDecoder` | `TextEncoder` emits UTF-8. Hermes 0.4's built-in WHATWG decoder keeps the browser-style encoding labels, including UTF-8 and UTF-16LE/BE, plus `fatal`, streaming and `ignoreBOM` behavior |
| `URL`, `URLSearchParams`, `atob`, `btoa` | |
| `crypto.getRandomValues`, `crypto.randomUUID`, `crypto.subtle` | Inside an answer; `subtle` digests (SHA-256/384/512) and ECDSA P-256 keys (LLP 1069.005), and refuses the rest by name |
| `AbortController`, `AbortSignal` | `AbortSignal.timeout()` refuses: no timers |
| `queueMicrotask`, `Promise` | |
| `Intl.NumberFormat`, `Intl.DateTimeFormat`, `Intl.Collator`, `localeCompare`, `toLocaleString` | Date formatting needs an explicit timestamp. No `Intl.PluralRules`, `RelativeTimeFormat`, `ListFormat`, `Segmenter` or `DisplayNames` (Apple's engine; Linux's is built `--intl`). `Intl.Locale` is the prelude's on every Hermes host (the engine has none): a tag parsed and canonicalized as Chrome does, its options and getters, and `getWeekInfo()` with Chrome's `{firstDay, weekend}` from CLDR's week data, by the tag's region, its `-u-rg-`, or its language's likely region (two-letter languages and a few others; another reads Monday and a Saturday-Sunday weekend), and `-u-fw-`. It has no `maximize`, `minimize` or other `get…()` list, does not canonicalize aliases (`iw` stays `iw`, `en-840` keeps `840`, and its week is then the default, Monday, where Chrome's is `en-US`'s), a formatter given a `Locale` object rather than its string uses the default locale, and `structuredClone` copies a `Locale` as `{}` where Chrome refuses it. Apple's `ja-JP` long date puts a space before the weekday (`10月6日 火曜日`, Chrome `10月6日火曜日`). Apple's engine has no `notation: "compact"`: the prelude formats its short display as Chrome does for a decimal in en, en-GB, en-IN, de, fr, fr-CA, es, es-MX, it, pt, pt-PT, nl, sv, da, nb, fi, pl, ru, uk, cs, tr, ja, zh, zh-TW, ko, hi, he, id, th and vi (and their regions); `compactDisplay: "long"`, another locale or a compact currency is printed in full and said once in the logs. Apple's engine rounds a tie to even and ignores `roundingMode` (`$0.82` for 0.825 dollars, Chrome `$0.83`); the prelude rounds the number first, as Chrome does: from its shortest digits, by the formatter's `roundingMode` (half away from zero unless it names another), which `resolvedOptions()` then reports; a compact it prints in full rounds so too, to its style's digits (`$0.83` for a compact 0.825 dollars). A `unit` keeps the engine's rounding, since that engine converts it to the locale's unit (`0.622mi` for 1 kilometer). Apple's engine has no `formatToParts` |
| `console` | To the runner's logs after each answer and reply, including refused calls; available through the agent's `logs` on native hosts |

A `net.fetch` line names one origin (`https://api.example.com`), every host
under one domain (`https://*.example.com`), or a whole scheme, written as CSP
writes a scheme source: `net.fetch https:` or `net.fetch http:` admits any host
at any port of that scheme, for a server the person names at run time (a URL
they type, a local server on a port chosen at launch). `net.websocket` takes an
origin or `wss:`/`ws:` the same way. A scheme no line names is refused (`wss:`
does not admit `ws:`). The platform's own refusals still apply: App Transport
Security refuses cleartext beyond the local network, and a browser refuses
`ws:` from an `https:` page except to loopback. A render-server page's
`connect-src` carries the scheme source as written. The render server itself
doesn't hold a scheme-wide `net.fetch` line: a source that needs one keeps its
placeholder in the rendered page, and the client fetches it after adoption.

For `fetch` with the default `redirect: "follow"`, native hosts check each
redirect target against the source's `net.fetch` grants before contacting it.
The web lets the browser follow and checks only the final response URL. On
render-server pages, CSP `connect-src` allows the page's own origin and the
app's granted origins, while request admission uses the source's grants. A hop
outside the page origin and app grants is blocked before contact (`Network` on
web, `Refused` on native). An app-granted but source-ungranted hop can still
receive a 307 POST body and return to a source-granted final URL; web succeeds
while native refuses before that hop. Static and dev pages have no CSP: an
ungranted intermediate origin can receive the request (including a POST body
on a 307); if the chain ends at a granted origin, the
web succeeds while native refuses at the intermediate hop. Without CSP, an
ungranted final URL is refused after contact. CORS or another browser failure
can also hide the response and yield `Network`. Exact API grants do not
sandbox page code: it can call browser `fetch` directly, subject to the
rendered page's CSP where one is served.

On native hosts, ordinary ordered `GET`/`HEAD` reads wait in the same FIFO as
writes: up to 128 queued, running and unconsumed requests, with at most 64 MiB
of waiting request buffers. Other ordered work retains a 16-request admission
bound. A full queue rejects explicitly; the runtime never restarts the whole
source on that refusal, since earlier turns may have written already. For
example, 20 resources fetching once beside one source fetching 20 times can
all wait and answer. A failed resource keeps its last value; `refresh` asks
again. See LLP 1041 §8.4 for the byte reservations and cancellation policy.

That queue runs one request at a time, so a screen that reads twenty feeds
waits for each in turn. A read whose request and reply may both be handled
out of order can leave it: `fetch(url, { exactIndependentHttp:
{ maxResponseBytes: 4194304 } })` in TypeScript, or
`Request::get(url).independent_http(4 << 20)` from a Rust source. A native app
then runs up to six such reads at once, within their own budget of 128
requests and 64 MiB. A running read counts twice its ceiling, so a ceiling
above about 32 MiB is refused when the read is made: keep it near the largest
reply you expect. A browser has no lanes and schedules every fetch itself, so
there only the ceiling applies. On every host a reply larger than
`maxResponseBytes` fails as `refused`. The ceiling is 1 byte to 64 MiB, and
anything else is a `TypeError`. A `GET` is
not independent by default, because a source may depend on its replies
arriving in order. Leave writes, and any read whose parse depends on an
earlier reply, in the queue. Storage steps and native continuations cannot
opt out of it.

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
Aliases and dynamic property keys still reach the runtime guard. In a running
session, a refused TypeScript answer fails its resource and keeps its last value
or placeholder; the typed input or action still commits. `failure(x).code` is
`error` for the module's own error, unless its nonempty `reason` supplies a domain code. A failed send ends unsent, retains its previous
result and runs no `then`, including when queued. The journal names the failure.
Module initialization, bake and replacement validation still refuse failures.
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
if no storage grants are declared. Once storage is available, ungranted
operations reject with `Unavailable` and code `denied`, without loading the
browser storage adapters. A drive without a scratch store still gets `agent`
for app-storage writes and SQLite, and `ENOENT` for a file read, as an empty store
answers; `doc:/` handles do not need that store.
Generated declarations export Ibex2's `Storage` and related types; Rust sources
can use the same implementations through `ibex2::host`.

Hosts configure app-specific directories after first pixel. Files and databases
survive module reload; temporary storage is a directory under the app cache,
without an automatic cleanup guarantee. Agent mode does not open disk storage.
Bake rejects storage calls with `Unavailable` (`code: 'bake'`), and `native.call`
and `native.later` the same way: the build links no native module. A resource whose
answer fails for it, caught or not, compiles no value: it shows its placeholder
(its `else` row, or the type's zero) and is asked when the app runs, on every build.
So a source that asks the app's module needs no bake code of its own
(`native!.watch('state'); return native!.call({op: 'state'});`). Catch it only to answer
something better than the placeholder. Browser storage uses app-scoped IndexedDB files
and SQLite WASM in a dedicated worker, loaded on the first database operation.
Use HTTPS or localhost for Web Locks. Data persists across reloads within the
same browser origin, subject to browser storage retention and quota policies.
An open database exclusively locks its file; conflicting opens or filesystem
mutations return `Unavailable` with a busy message. Agent mode skips storage.

In `app.ts`, a storage refusal is an `Error` with `kind: 'Unavailable'`, a `code` and a
`message`. The code is the same on every host; the message says more and may
differ. Storage itself is unavailable: `'bake'` (the build compiles no answer
that reads storage or calls the native module; show a placeholder), `'agent'` (a scripted drive that names
no scratch store, `--storage <name>`), `'unsupported'` (a host with no app
storage). The operation was refused: `'denied'` (the grants do not cover it),
the filesystem's POSIX name (`'ENOENT'`, `'EEXIST'`, `'ENOTDIR'`, `'EISDIR'`,
`'ENOTEMPTY'`, `'EBUSY'`), `'full'` (256 operations already wait behind the
one in flight: the module's storage queue is full, LLP 1097), else `'failed'`. Branch on the code, never the
message: `catch (e) { if (e.code === 'ENOENT') return empty; throw e; }`.

A drive's app storage is a scratch store it names (`--storage <name>`), on a
native host `default` when it names none, or none (a web drive naming none).
A native one is kept between drives until a drive passes `--reset-storage`, which
empties all of it (`app:/` files, SQLite, kept state, secrets) at the app's boot
and refuses a store reached through a symbolic link; one drive holds a native store
at a time. On the web it is Chrome's profile for the name and its page's origin,
empty each drive unless `--keep-storage` (a Firefox or WebKit drive's is its own). A drive that compares with a
reference starts empty on every host unless `--keep-storage`. An authored test gets a fresh
one of its own, removed after it. A data module's `secret.keep` rides that same
store: files under the named tree on Apple and Linux, `localStorage` in the named
web profile. Apple native modules receive the same named data, cache and temporary
roots across launches. A fresh drive clears them; the picker's temporary documents
alone are cleared on each launch. A web drive with no `--storage` refuses app storage (`storage is unavailable
in agent mode unless the drive names a scratch store (--storage <name>)`; a file
read rejects `ENOENT` instead, as an empty store's) and keeps
no secrets. The driver's `state.storage` says
which (`{available: false, code: 'agent', message}` or `{available: true,
store}`), and the journal says `storage refused (agent): …` the first time
a read or write is refused (a refused read finds what an empty store would: nothing,
code `ENOENT`, so a source that catches it falls back silently), which the driver repeats once as a
note, after the first op that met it. A Rust module's storage request in such a drive is answered
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
replaceable. An answer the runner lets go between storage steps (one asked in a
commit that is refused, a read whose arguments changed or that a `refresh`
replaced) still runs the steps it began, and the chain behind them, to
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
WebSocket under `net.websocket` alone. Except for TypeScript sources on the
web JS target and native hosts, described below, sockets are receive-only. An event is
`{type, data, lastEventId, coalesced}`; messages that arrive faster than they
commit coalesce to the newest, counted in `coalesced`. The end is
`{type: 'error', kind, message, status}`: `kind` is `Network` (the far side
closed: `the socket closed (1000)`), `Refused` (outside the grants), `Aborted`,
or `Response` with the status and body of a reply that was not an event stream.

A TypeScript source's `http:`/`https:` stream whose 2xx body is not
`text/event-stream` is a **byte stream** (LLP 1016.000, the amendment of
2026-10-23): each `message` has an ArrayBuffer `data`, the bytes read since the
mapper's last turn, at arbitrary boundaries (a framing is the mapper's to
parse), never dropped or coalesced. At most 1 MiB, or the stream's
`exactIndependentHttp.maxResponseBytes` if smaller, waits for the mapper; a read
past it ends the stream with `Refused` (`byte stream receive queue exceeds 1
MiB`) after the bytes already waiting. The body's end is `Network`, `the byte
stream ended`. A 204, a 205 and a `HEAD` have no body and stay the one answer.
The request carries `Accept: text/event-stream`, as every stream's does, unless
the source names an `accept` header; name one where the server negotiates by
it. The web JS target, macOS, iOS and Linux read it; the web's wasm host, Rust
sources and worker-placed sources still take such a body as the one answer.
A caller's abort signal maps an `error` with kind `Aborted` while the resource
is still held. New arguments, `refresh` or the resource leaving the view close
the stream without further mapper calls;
`state.pending` lists it until its first message, `state.streams` while it is
open, with `grant`, the grant line that admitted it. The same holds on Hermes, the web's wasm host and the web build (the JS
target), with one difference: on the web build the stream's `fetch` must be
made while the answer is asked, before its first `await` (a later one is
refused, saying so); on Hermes it may follow an `await`.

On the **web JS target and on macOS, iOS and Linux, TypeScript sources**
receive `(event, socket)` for a WebSocket (LLP 1016.000 D7). Keep the handle by your resource's identity in
module state. Its events are `open`, ordered `message` (string or ArrayBuffer
`data`), and `close` (`code`, `reason`, `wasClean`); a refusal is still `error`.
Every protocol frame reaches the mapper; `coalesced` is zero. The mapper must
return a shaped snapshot synchronously. A mutation sends on a kept handle, and
the mapper may answer its own socket's frames (below):

```ts
import type { StreamSocket } from './app.contract.d.ts';
const feeds = new Map<string, StreamSocket>();
export function answer(source: string, [id, url, text]: [string, string, string]) {
  if (source === "feed") return fetch(url, {
    exactStream(event, socket) {
      if (socket) feeds.set(id, socket);
      return { status: event.type, text: typeof event.data === "string" ? event.data : "" };
    },
  });
  if (source === "sendFrame") {
    const socket = feeds.get(id);
    if (!socket) throw new Error("feed is not open");
    socket.send(text);
    return { accepted: true };
  }
}
```

Call `sendFrame` through a Contract `mutation`, after the feed reports `open`.
`send` accepts a string, ArrayBuffer or ArrayBufferView; bytes are copied at
the call. `close(code = 1000, reason = "")` accepts 1000 or 3000–4999 and a
reason of at most 123 UTF-8 bytes. In a mutation, both methods are allowed
during its source's synchronous call, and after it only when the source
answered with a promise (below). A source that stages socket operations
answers now with a value or with a promise (then it is an answer in flight,
below), not a stream answer, and makes no `fetch` in the same source. Whichever of a socket operation and a
`fetch` comes second throws `a mutation cannot both send on a socket and
fetch`, and the mutation is refused before either takes effect: no frame is
sent and no request starts. A thrown source, wrong-shaped answer or refused
commit sends none of them. The handle exposes read-only `readyState` (0–3)
and `bufferedAmount`: the transport's buffered bytes as this turn of the
module began, plus what this mutation has staged on the socket, so a turn
that sends nothing still sees a buffer that has drained. It cannot extend
the resource's lifetime.

The mapper may also `send` and `close` on the socket it was handed while it
answers `open` or a `message`: a subscribe on `open`, an `Ack` or `Pong` for
each frame, as a browser client answers in its handlers (LLP 1016.000 D7,
amended 2026-10-16):

```ts
exactStream(event, socket) {
  if (event.type === "open") socket.send('{"_tag":"Subscribe"}');
  if (event.type === "message") socket.send(JSON.stringify({ _tag: "Ack", id: JSON.parse(event.data).id }));
  return { status: event.type, text: typeof event.data === "string" ? event.data : "" };
},
```

What a mapper call stages is sent once the commit that settles that frame's
snapshot stands, in arrival order and ahead of later mutations' frames. A
throw, a wrong-shaped snapshot or a refused commit sends none of it, and
`logs` say `mapper <resource> failed: …; socket frames were refused`. Each
call has a mutation's bounds below; overload throws `QuotaExceededError` and
fails that frame's snapshot even if the mapper catches it. On `close` and
`error` the handle is already closed and refuses; so do another socket's
handle and a `fetch` in the call. A continuation the mapper schedules is not
the call's: it runs in a later turn (below).

A resource's answer may also `send` and `close` on a socket a stream
resource of the same module owns, so it sends its own request and awaits
the reply, with no timer, and so may a mutation whose source answers with
a promise (LLP 1016.000 D7, amended 2026-10-18):

```ts
const waiting = new Map<string, (reply: string) => void>();
// In `feed`'s mapper, for a reply: waiting.get(reply.id)?.(reply.text)
if (source === "request") {
  const socket = feeds.get(id);
  if (!socket) throw new Error("feed is not open");
  socket.send(JSON.stringify({ _tag: "Request", id: n }));
  return new Promise<string>(resolve => waiting.set(n, resolve));
}
```

What each turn of an answer sends goes out in the order it sent it once
the commit that ran the turn stands, as its fetches do; a refused commit
takes it back and says so, and nothing the answer does afterwards (a
value, a failure) takes it back. Natively its turns are its source's call
and each delivery of its own `fetch` or storage step. Code that resumes
from an `await` runs in the next turn that drains microtasks (its own
delivery, or the answer's that settled what it awaited) and sends as that
turn's answer; a mapper's call drains none, so code a mapper resumes runs
in the next answer asked, or in none (and its send refuses) when that is
the waiting answer's own re-ask. Send before the first `await`, or settle
waiters from an answer. An answer asked again sends again, so key a
request that must not repeat. On the web build only the source's
synchronous call may send; a continuation refuses, since the page cannot
tell whose it is. An answer
the runner lets go (new arguments, `refresh`, a newer send, leaving the
view) sends nothing more as its own; what it sent stays sent. It may also
`fetch`, though a mutation's synchronous source still may not do both. A
refused commit's batches are taken back without telling the code, so a
protocol that awaits a reply times out. Its bounds are a mutation's, over all its turns (`socket answer
exceeds 256 operations`, `… 1 MiB`); overload throws without failing the
answer. A handle that is not open, or whose socket a mutation's stream
owns, throws `InvalidStateError`, and so does an operation after the
answer's own `close` of that socket. A batch the host refuses is said in
`logs` as `answer <resource or mutation> failed: …; socket frames were refused`. Its
`bufferedAmount` counts the transport's bytes as the turn began and what
this turn has sent, and natively also what earlier turns sent that the host
has not yet taken.

A mutation or mapper call stages at most 256 operations and 1 MiB, including
bytes already buffered by the browser. Overflow throws `QuotaExceededError`; a stale or
not-yet-open handle throws `InvalidStateError` (also after close, a deliberate
difference from the browser's silent discard). A change before publication
can still refuse transport admission, reported in `logs`: acceptance is not
server acknowledgement. The receive queue is bounded by 256 frames and
1 MiB plus the per-message response ceiling; overflow maps accepted queued
frames in order before ending with a `Refused` error.
`state.streams` adds `readyState`, `bufferedAmount` and `sent` for these sockets.

Natively `readyState` follows the events the mapper has seen, and
publication also bounds the transport's buffer at 256 frames; LLP 1016.000
D7 lists the differences. **Remaining host gap:** the web wasm target, Rust
sources and worker-placed TypeScript do not provide the send path (on wasm
the handle never opens). No ambient `WebSocket` constructor is admitted. SSE
remains unchanged.

This first browser implementation targets modest app stores: filesystem
operations read the app's file records, and each SQLite mutation atomically
saves the whole database file. Database files share the filesystem namespace,
so closed databases can be copied or exported through `storage.fs`. SQLite integer results
are `bigint`: convert them to a Contract-compatible value before returning.

### Notifications

`showNotification(title=…, body=…, tag=…, showTrigger=…, navigate=…)` posts a local
notification; `closeNotification(tag)` takes one away, shown or still
waiting. The names are the Notification API's (`showTrigger` is the
Notification Triggers draft's member, given as the time in epoch
milliseconds: the date now is `exactTime().epochAtZero + performanceNow()`). A newer notification
with the same `tag` replaces the older. The app's grants must name
`device.notifications purpose.notifications` (a strings key, LLP 1069.008;
iOS shows its own fixed prompt text), or the command is refused. Permission
is asked the first time; the outcome is a journal line
(`showNotification: shown`, `scheduled`, `refused: denied`, …).
`navigate=` is a route location, such as `"/thread/42"`; relative paths and
custom-scheme URLs are reduced by the router's `location_of` on every host
(`app://thread/42` becomes `/thread/42`; `app:/thread/42` and
`app:///thread/42` become `//thread/42`). A plain click on the
notification delivers one `navigate` event to the navigation root while the
app runs, or makes that location the launch location when it starts the app.
Notifications posted before the app last quit still navigate after it reopens.
An embedded Mac host must receive launch callbacks before sizing or attaching
its Exact views: either can boot a session. Boot once with the launch location,
then attach the view.
Without `navigate=`, a click just brings the app forward. Action buttons,
replies, and badges are not available.

| host | now | at `showTrigger` |
| --- | --- | --- |
| web | `new Notification(title, {body, tag})`; a click navigates and focuses the open page | while the page is open: the web has no trigger that outlives the page |
| macOS, iOS | `UNUserNotificationCenter`, shown with the app in front too | the system's, delivered with the app closed |
| Linux | refused: `unavailable` | the same |

On macOS, system notifications need an app bundle (`bun host/apple/build.mjs
--bundle`); the bare development executable refuses them because it has no
notification centre.

Under the agent nothing reaches the system on any host: `state.notifications`
lists what the app posted (`{title, body, tag, showTrigger, navigate}`, a tag replacing
its older one, `closeNotification` removing it), so a drive reads a reminder
without a permission prompt. A drive can then `type` the listed location into
the navigation root to check its screen. Scheduling is one time per call: a daily
reminder posts the next one when the app runs.

### Local network

The grant `device.local-network purpose.<key>` writes the strings key's text
as `NSLocalNetworkUsageDescription` on macOS, iOS and tvOS (with each table's
translation in `InfoPlist.strings`). That text is the prompt the platform
shows before the app first reaches an address on the local network (macOS 15
and later, iOS 14 and later; Apple TN3179). Exact adds no gate of its own:
the platform asks and refuses. On iOS the grant's text replaces the
development client's default prompt from `host.ios.localNetworking`, which
still adds `NSAllowsLocalNetworking`. The web, Linux, Windows and Android
get nothing: a browser asks with its own Local Network Access prompt.

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
gain=, group=, playbackRate=, pan=)`, `playSounds(hits)` and ended by
`stopSounds(group=)` (LLP 1096; the rate and pan, LLP 1125 D10). The runner keeps the voice table, so what was scheduled, when, and how
each voice ended is the same on every host and under the driver's clock
(`state.sounds`, `expect sound`).

| host | output |
| --- | --- |
| web (both targets) | Web Audio: a one-shot `AudioBufferSourceNode` per voice at `start(when)` with its `playbackRate`, through a `StereoPannerNode` when its `pan` is not 0, aimed at the speaker through the context's output timestamp; the first sound after the page's first tap or key (one before it is dropped and journaled `sound blocked`) |
| macOS, iOS, tvOS | one `AVAudioEngine` with a C mixer behind an `AVAudioSourceNode`, sample-accurate (a playback rate reads between frames by linear interpolation; a pan follows the `StereoPannerNode`'s equal-power algorithm), in an arm loaded on demand (`libexact_sound.dylib`; `ExactSound.framework` in an `.ipa`) |
| Linux, Windows | none: the record only (`state.sounds.output` is `"none"` outside the driver) |

Under the driver nothing plays on any host (`output: "agent"`). `app.json`'s
root `audio_session` is the Apple audio session every sound, video and canvas
shares: `"ambient"` (the default: the ring/silent switch mutes it, other apps'
audio keeps playing) or `"playback"`. Under `"playback"` the session is held
only while something has sound: the sound arm and a canvas's audio once they
start, a `video` or `audio` while it has a source and is not muted, and a
media-session claimant (`metadata=`, which Now Playing needs). When the last
of those is muted or gone, the session goes back to ambient, which mixes with
other apps' audio, as Bluesky's player does on re-mute; it is given up, so a
paused podcast resumes, once no player is left. WebKit's Audio Session API takes it too;
the Mac has no session. A development run with `EXACT_SOUND_CHECK=1` taps the
engine's main mixer and journals (and prints) how far each onset after a
silence reached the speaker from its time.

**When the Apple output opens.** At the app's first voice, never at boot, so
declaring a `sound` costs a launch nothing. As a hand-built app starts its
engine on first use, that voice's commit activates the audio session (iOS and
tvOS), opens the output, reads every declared sound and starts the engine on
the main thread: the voice starts as late as that took (from its first frame,
never cut), about 100 to 200 ms on a Mac. An app that never plays never
touches the audio session.

**When it cannot open** (no output device, a session that does not activate,
an engine that does not start or raises), the app keeps running silent: the
line `sound: the output did not open: <why>` is journaled (and printed),
`state.sounds.output` is `{state: "failed", error, skipped}`, the voices
are skipped, and the first voice a second or more later tries again. A
failed output gives the audio session back, so a `"playback"` app does not
leave other apps' audio paused. An interruption that ends with
`shouldResume` restarts the engine (a failure fails the same way); one
without it (the person started other audio) is left to the next voice. An
engine the system stopped (a route or format change, a media services reset)
is opened again by the next voice; one that stops again within a second of
opening is a failure. Outside the driver `state.sounds.output.state` is
`closed` before the first open, then `running` or `stopped`. One failure is
beyond any app's catch: when CoreAudio's own RPC to the audio server times
out (`_ReportRPCTimeout`, seen on iOS simulators whose audio is wedged), it
aborts the process inside `AVAudioEngine`; opening at the first voice keeps
that from launch and from an app that plays nothing.

**A sequence** (`sequence NAME when cond` holding `play(tracks, bpm=…)`, LLP
1125 D2–D5) is expanded by the runner, never by a host: its steps become
voices in the same table a second ahead of the runner's clock, refilled when
half a second is left on a wake of its own that runs no action and that
`clock settle` does not wait for, so a main-thread stall of up to 500 ms
loses no step. The bound is the same for a sequence's voices and a tap's: at
most 32 voices sound at any voice's start (another is dropped and journaled),
and at most 256 wait to start (another tap is dropped; a sequence stops
expanding early and wakes when the first starts, dropping nothing). The
Apple mixer has 288 slots and a ring of 1024 ops for it (a Play that finds no free slot takes one whose voice ends inside the buffer, mixing that voice's last frames first), and frees a voice
retracted before it starts at once; the web lets its node go. A step's haptic
plays on the main thread at its time (Apple's feedback generators;
`navigator.vibrate` on the web, where it exists): a blocked main thread makes
it late, and `state.sounds.output.hapticsLate` counts those more than 10 ms
late. Linux and Windows keep the record. On iOS and tvOS a sequence stops
when the app goes to the background (`exactTransport().reason` is
`background`), unless `audio_session` is `"playback"` and
`host.ios.backgroundModes` has `"audio"`; a Mac app and a web page play on.
`state.transport` is the record: what `exactTransport()` answers, the sequence
that last played, how far its hits are issued (`issuedTo`), its last 16 tempo
segments, and its last 64 haptics with their states.

**Clock in** (`play(…, clock="<input id>")`, LLP 1125 D9) follows a MIDI
input's clock. The input's ticks, Start, Continue, Stop and Song Position
reach the runner with their arm timestamps in its ordinary input deliveries:
ticks alone at most 50 ms after the oldest, anything else at once with the
ticks ahead of it. The runner takes the tempo from the last 48 tick stamps,
counts the position in ticks, and issues hits only to 100 ms past each
delivery (the batch plus the 50 ms hold), so a hit is timed from the newest
estimate; a new tempo or phase that moves a hit not yet within the hold by
more than 2 ms takes the hits back (`by clock`) and issues them again. The
record is a function of the deliveries, so a replay of them is the same take.
Only the followed input's ticks are passed on (Apple's hub and midi-glue.js
drop the rest, and Active Sensing always); nothing ends a lost clock but the
hits running out. `state.transport.follow` shows the input, the ticks heard,
the window, the estimate, the ticks since Start and where Stop held it.

**MIDI** (`exactMIDI()`, `requestMIDIAccess()`, `sendMIDI`, a step's note and
`clockOut=`, `midimessage`; LLP 1125 D6–D8) needs the grant `device.midi purpose.midi`
on every host, `purpose.midi` a key of `strings/en.json` (the purpose the person is shown). The runner keeps one queue for every message, a sequence's notes and
clock ticks included, and a host hands each to its output 50 ms before it is due
and holds it until then: CoreMIDI could take it early and flush, but
`MIDIFlushOutput` empties the whole destination and Chromium has no
`MIDIOutput.clear()`. So an edit or a stop takes back what is held, and a note
already handed gets its note-off (a stop moves it to the stop). At most 1,024
messages wait; note-offs, Stop and ticks are never refused. Per host:

- **Apple** (macOS 14, iOS 17): a loaded arm, `libexact_midi.dylib` (the
  `ExactMIDI` framework on iOS), built only for a plan that uses MIDI. CoreMIDI
  runs on the arm's own thread and run loop, made at the first
  `requestMIDIAccess()` (no prompt: the state is `prompt` until then), so its
  notifications arrive and a cold MIDIServer never blocks the main thread. Port
  ids are `kMIDIPropertyUniqueID`, so a replugged device keeps its id; sends
  hold on the arm's thread (a main-thread stall does not delay them) and go out
  with `MIDISendEventList` stamped in host time; inputs are stamped from the
  packet's own time. It enumerates again when the app comes to the foreground
  and on a 3 s watchdog while a port is in use. tvOS: `unavailable`.
- **Web**: Web MIDI through midi-glue.js, loaded after the first frame for a
  plan that uses MIDI; `granted` without asking when the permission already
  holds; held sends on the page's timer, so a main-thread stall over 50 ms makes
  those messages late (a declared limit, counted in `state.midi.output.late`).
  WebKit, and so every iOS browser, is `unavailable`. SysEx is never asked for.
- **Linux, Windows**: the record; a request finds `unavailable`.

Under the driver nothing is sent: `prefer midi.*` names the ports a request
finds, `tap midi` delivers a message (`tap midi "<input>" clock <bpm> <ms>`
its clock, a 50 ms slice at a time), and `state.midi` is the record — the ports,
the inputs the listeners open, how many messages wait, the last 64 queued with
their states (`waiting`, `handed`, `retracted` and why), the last 64 heard with
their beat, and clock out's tick count.

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

### Keep the screen awake

`wakelock="screen"` on any element keeps the display from dimming or locking
for idleness while that element is mounted and displayed: neither it nor an
ancestor is `display: none` (or laid out so by its host, as an unopened tab
is), and it is under no tab the navigation leaves unselected and no screen a
stack keeps under its top, so a screen kept mounted in a hidden tab panel
holds nothing; the screen a sheet covers is still shown, and holds. An exit
animation's ghost holds nothing ([LLP 1125](../llp/1125-a-transport-midi-and-the-small-pieces.rfc.md)
D11). Natively an element the host shows as its own control (a header in the
navigation bar, a confirmation as a native alert, a menu as a native menu)
holds nothing, because a whole host cover marks both what the host hides and
what it shows natively and the kernel cannot tell them apart; the web holds
there. `"none"` is the default. Bind it to what needs the screen:
`wakelock=(playing ? "screen" : "none")`. A literal other than the two words
is refused; a computed one is journaled (`view N: invalid wakelock value …;
unset`) and holds nothing. The runner decides which elements hold it
(`exact_runner::wakelock`), so the rule is one on every host.

| host | held through |
| --- | --- |
| web (both targets) | `navigator.wakeLock.request("screen")`, asked again on `visibilitychange` when the page is visible again (a hidden page loses it) and released when no element holds it; `wakelock: unavailable: …` is journaled once where the API is missing or a request is refused |
| iOS, tvOS | `UIApplication.isIdleTimerDisabled`, true while any session's element holds it; under the driver the timer stays disabled for the whole drive |
| macOS | an `IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep)` assertion, held while any session's element holds it |
| Android | `View.setKeepScreenOn` on the reader's view (request kind 8) |
| Linux, Windows | none: `wakelock: unavailable: this host keeps no display awake`, journaled once |

`state.wakelock` is `{held, by}`: whether any element holds it and their view
ids, on every host's driver.

### Pick a file of any type

An `input type="file"` with no `accept` (or `accept="*/*"`) offers any file;
`multiple` allows more than one. `showPicker(id)` opens it, and `change`
receives `Picked[]` with each file's name, type, size and
`app:/tmp/picked/` path. A specific `accept` list is limited to media types
and types in the app's `file_handlers`. The live Linux picker is unavailable;
its agent substitute accepts any file. Image pixel and Canvas readback remain
deferred.

### Shrink a picked image for upload (`storage.fs.compressImage`)

`storage.fs.compressImage(from, to, {maxDimension, maxBytes, type})` decodes the image
at `from`, scales it so its longer side is at most `maxDimension` pixels, and
writes a JPEG of at most `maxBytes` bytes at `to`, searching JPEG quality the way
Bluesky's composer does (from quality 51 down and up; when 51 and 26 both miss,
the size shrinks by 0.8, at most four sizes). Orientation is applied, and none
of the source's metadata is written (no EXIF, GPS or orientation); transparency
becomes white and the colour is sRGB. With `type: 'image/png'` it writes a PNG
instead, which keeps transparency: PNG has no quality, so it is encoded at the
fitted size and, while over `maxBytes`, the size shrinks by 0.8, at most four
times. It resolves `{path, type, size, width, height}`, `type` the one written
([LLP 1069.002](../llp/1069.002-media-picker.rfc.md) Amendment A1, A1.11):

```ts
export const grants = 'fs.read app:/tmp\nfs.write app:/tmp';
// files from `change` on an `input type="file"`; posts: 4000 px and 2 MB
const out = await storage.fs.compressImage(files[0].path, `app:/tmp/upload/${seed}.jpg`,
  { maxDimension: 4000, maxBytes: 2_000_000 });
const body = await storage.fs.readFile(out.path);
```

Both paths are `app:/` files (`fs.read` on `from`, `fs.write` on `to`); `to` is
replaced atomically, and left as it was on any failure. It always re-encodes, so
compare the pick's `size`, `width` and `height` first to skip it. It is one
storage operation in the module's queue; natively it runs off the JS thread. A bad option
(a `type` other than `'image/jpeg'`, the default, or `'image/png'` too)
rejects with a `TypeError`; otherwise a refusal's `code` is `denied`, a
filesystem code, `failed` (a path that is not `app:/`), `too-large` (over 64 MiB,
or a header over 64 Mi pixels, checked before decoding), `undecodable`, `unfit`
(nothing fits `maxBytes`), `timeout` (the search's 20 s, or a native wait that
ran out with nothing written) or `unsupported`. macOS and iOS encode with
ImageIO (an HDR photo is tone-mapped), the web with a canvas (in the page or the
module's worker; JPEG, PNG, GIF, WebP and BMP, never HEIF or AVIF, whose size it
cannot read before decoding);
Linux, Windows and Android have no image encoder and answer `unsupported`, so
upload the original when it already fits. Under the agent it runs for real with
`--storage <name>`.

### Documents the person chose (`doc:`)

A file or folder the person picks (`showOpenFilePicker`, `showDirectoryPicker`,
`showSaveFilePicker`), opens from the system at the app's `open-file` node,
drops on a node with `drop` or pastes (below), arrives as a `doc:/<n>/<name>`
path (LLP 1069.010 D1). `storage.fs` reaches it,
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
refused. Native folder handles retain the selected directory even if its
pathname moves. Descendant symlinks and Windows reparse points are refused,
including in intermediate folders; explicitly choosing a file through a
symlink still selects its target. A native selection whose filename is not
valid Unicode is refused rather than addressing a lossy spelling. A document needs no app storage: a drive without `--storage` reaches
it. On the web the paths are the `FileSystemHandle`s the page's picker
returned (Chromium; Safari and Firefox refuse the pickers), and a module placed
on a worker on the wasm web host cannot reach them (`'unsupported'`); on macOS
and iOS they are security-scoped URLs held for the session; Linux opens no
picker panel (a drive's held pickers still answer), so outside a drive only
`open-file` delivers one there. The handles end with the
session, unless the app keeps what the person chose (below).

**A change on disk asks the answer again** (LLP 1069.010.000 D1). An answer
that calls `storage.fs.watch(path)` is asked again when the file, a folder's
own entries, or with `{ recursive: true }` everything beneath it, is created,
written, removed or renamed by any process, this app included. The change
carries nothing: the answer reads again, as LLP 1016.002's `native.watch`.
Call it inside an answer, on a `doc:` path the app may read (`fs.read
doc:/`):

```ts
await storage.fs.watch(folder);                  // its own entries
return (await storage.fs.readdir(folder)).filter((n) => n.endsWith('.md'));
```

A Rust source records the same topic:
`store.observe_topic(&exact_data::documents::watch_topic(path, false))`.
macOS listens with FSEvents, Linux with inotify, the web with Chromium's
`FileSystemObserver`; iOS, the Linux host built for macOS and a browser
without the observer accept a watch that never fires. `state.watches` lists
the watched paths.

**A write can expect what it read** (D2a). `writeFile`, `atomicWriteFile`
and `appendFile` on a `doc:` path take `{ expect: stat }`, the `stat` the
app read: if the file's `modifiedMs` or `size` has changed since, the write
is refused with code `ECHANGED` and nothing is written (a file that is gone
refuses `ENOENT`). The comparison and the write are queued together but are
not one atomic step; what they close is the time an editor spends editing.
Read the new file and let the person choose.

**Documents kept across launches** (LLP 1069.010.000 stage 2; macOS). An
app granted `doc.keep` keeps exactly what the person chose — a file or folder
from a picker, Finder, ⌘O, Open Recent, the command line or a drop; never a
typed path, a followed link, or a route's folder — and its `doc:/<n>/<name>`
reads again after a relaunch, in any of the app's windows, with no panel.
The kept set is File ▸ Open Recent's: Clear Menu, or an entry falling off
the end of the list, ends its reopening (a window that has it open keeps it
until the app quits).

```ts
export const grants = ['fs.read doc:/', 'doc.keep'].join('\n');
storage.fs.watch('doc:/');                       // asked again when the list changes
const recent = await storage.fs.recentDocuments();
// [{ path: 'doc:/12/notes', name: 'notes', kind: 'directory', mode: 'readwrite', opened: 1760572800000 }, …]
await storage.fs.forgetDocument(recent[0].path); // off the list and Open Recent; ENOENT if not kept
```

A Rust source sends `fs.recentDocuments` and `fs.forgetDocument` storage
requests (`apps/markdown/data`). A kept item is the object chosen: a move
or rename is followed and keeps its path (`<name>` is the name it was
chosen by); a file another program replaced at its name is not the
document, and the app's own `atomicWriteFile` carries the entry to the file
it wrote (from Rust; not yet from TypeScript). A path whose entry is gone
refuses `ENOENT`: ask the person to choose again. Without `doc.keep`
both calls are refused. Under the agent the kept set is the drive's scratch
store (`--storage`), never the person's. Linux and iOS keep nothing yet and
list nothing; the web has neither call yet.

### Files dropped or pasted

A `drop` hands its action a `DragEvent` whose `files` are `doc:` handles of
any type, a folder's included; a `paste`'s `ClipboardEvent` has `files` too,
image data with no file (a screenshot) as one `image.png`. Read them as any
`doc:` path, under `fs.read doc:/`; `storage.fs.stat(path).isDirectory` tells
a folder. A file drag over a `textarea` or `input` whose node or an ancestor
declares `drop` lands there, not as path text in the field. `dragenter` and
`dragleave` bracket the drag over a node, for a drop overlay
([grammar](contract-grammar.md#pointer)):

```text
action took(e: DragEvent)
  paths = e.files
action pasted(e: ClipboardEvent)
  if e.files.length > 0
    paths = e.files
    preventDefault()
column dragenter=entered dragleave=left drop=took
  textarea paste=pasted
```

| Host | `drop`, `dragenter`, `dragleave` | `ClipboardEvent.files` |
| --- | --- | --- |
| Web (JS and wasm targets) | yes: Chromium's handles, else read-only copies; a folder's entry as a read-only folder | `clipboardData.files`, read-only copies |
| macOS | yes: every file URL Finder drags, a field forwarding the drag to its `drop` node | file URLs, else PNG or TIFF data written as `image.png` |
| iOS | none (declared; the driver's drag forms answer `unsupported`) | file URLs, else the pasteboard's image as `image.png` |
| tvOS | none | empty: tvOS has no pasteboard |
| Linux, Windows, Android, terminal | none (declared; Linux and Windows drivers refuse by name) | empty: these hosts paste text only |

The driver: `tap <target> drop <path…>` (files and folders), `tap <target>
dragenter <path…>` and `tap <target> dragleave`, `type <target> paste --file
<path…>` and `type <target> paste --image <png>`.

### Bake and deliver a TypeScript module

Build an app-local `app.ts` module and bake its Contract through the resulting
Hermes bytecode with the compiler from the verified install-once bundle:

```sh
cargo run -q -p exact-js-bake -- path/to/app --out path/to/new-generation
```

The app exports `appId`, `grants`, and an `Answer`-typed `answer`; `appId` and
`grants` are constants the build reads from the source without running it (a
string, a template or `+` of constants, an array of them `.join('\n')`, or a
top-level `const` holding one). The producer
captures local imports, type-checks, bundles with Rolldown, compiles HBC, and
bakes with an empty store. It writes `app.plan`, `app.js`, `app.hbc`, generated
types, and an `app.module.json` pairing receipt into a **new** directory; it
never overwrites an existing generation. An installed package's code is captured
file by file as the bundle reaches it; the receipt's `packages` names each
`name@version` with a digest of what was taken (LLP 1027 D5, "Package code").
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
`REVISION`) into `OUT_DIR`, and on Apple `host.rs`, the host entry its `deploy.store`
names, which includes `baked.rs`: `exact_plan()` and `exact_compat()`. An Apple build
(`host/apple/build.mjs`) bakes before Cargo and links the plan, `compat.json` and the
module's bytecode into the executable, so an edit to the Contract or the TypeScript alone
compiles no Rust; there `module.rs` has no `REVISION`. With a store (`"A"`), a signed update carries the module pair under
`typescript/` (`app.module.json`, `app.hbc`); it needs the binary's TypeScript runtime
and exactly its grants. An app with both a TypeScript and a Rust module still sets its
`deploy.store` entries to `"0"`: signed delivery of both is not implemented.

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
there); `app.ts` imports pure-JavaScript packages only (Node's modules, the DOM, code from a string and native addons are refused). That check keeps a module portable between Hermes and the page; it is no security boundary: on the web the module runs in the page's realm, and a package is code the author chose, trusted as their own; signed delivery of TypeScript and Rust
modules isn't implemented, so set `deploy.store` to `"0"`. The history of how this
was proved on each host is in [LLP 1027](../llp/1027-typescript-data-sources.rfc.md)
and git.

## Access hatches

An access hatch hands the app's native code the platform object Exact built for
a node, at defined moments, so the app can do what only that object can: add a
gesture recognizer, set a navigation bar's look, draw into a view. It configures
what Exact made; it never replaces a route's authored tree. (LLP 1075.003,
1075.003.000, 1075.003.000.001; its §13 is the as-built record.)

A route's `header` or navigation root's `tablist` with `appearance="none"`
keeps its authored bar on iOS, including layout and type; a header button with
it keeps that button and its contents authored in place; the rest of the header
still projects. An authored Back replaces UIKit’s Back. Search and a header
tablist can opt out on their own element too. Tab/tabpanel navigation and retained stacks
remain intact. An opted-out root tablist also stays authored on the Mac and in wide
web windows instead of becoming a sidebar, and either stays the page's on Android
(the shared rule, `kernel/src/bars.rs`). On the Mac a header button with it keeps
the whole header painted: the window's bar shows a header whole or not at all.
See [tabs](recipes/tabs.md).

```sh
bun scripts/exact.mjs hatch avatar <app>     # in an app `exact new` made: bun exact.mjs hatch avatar
bun scripts/exact.mjs hatch --app <app>      # the app scope; --window for the window's
```

The verb writes a stub for each target the app builds (Swift in
`modules/apple/`, the page module in `modules/web/`, Rust in
`modules/linux/`), adds the word to
`app.json` `hatches` with those platforms, and tells you the node to mark:

```
column hatch="avatar" data-tone=(busy ? "busy" : "idle")
```

`"hatches": ["avatar"]` means every platform the app builds; `"hatches":
{"avatar": ["ios", "web"]}` names the platforms whose module handles the word.
Elsewhere the node is shown and no hatch is called for it. A hatch reads state
only through the node's `data-*` words (declared in `app.json` `data`), and is
called again when one changes.

An action can read those same declared words with
`querySelectorAll("[data-tone~=\"busy\"]")`: it returns the own ids of
mounted, displayed, non-inert matches in tree order; covered routes and
unselected tabs are hidden; a split list answers alongside its detail where
the host presents it (the web at desktop widths). Closed popovers
and dialogs are hidden, and open ones answer. For an Inspect overlay,
`elementsFromPoint(x, y)` returns the own ids of the boxes under a viewport
point, front to back. Both are action snapshots; `frame(id)` gives each
returned node's box for an outline. Point reads skip inert subtrees and use
the presenter's placement and order for open top-layer content.

| Scope | Swift (`ExactModule`) | Page module (`modules/web/index.js`) |
|---|---|---|
| A node marked `hatch="word"` | `element(_:)`, `elementEnded(_:)` | `element(e)`, `elementEnded(e)` |
| A route, a navigation stack, tabs (iOS; the web's elements) | `route`, `routeEnded`, `navigation`, `tabs`, `tabContainer` | `route`, `routeEnded`, `navigation`, `tabs` |
| The window toolbar (macOS) | `toolbar(_:)` | none |
| The app: its facts, by the web's names, and the root node's `data-*` words as `data` (each in `app.json` `data`, which `hatch --app` adds; Swift reads `data-booted` as `app.data[.booted]`) | `app(_:)`, `appEnded(_:)` | `app(a)`, `appEnded(a)` |
| The window the session presents into | `window(_:)`, `windowEnded(_:)` | `window(w)`, `windowEnded(w)` |

**Linux, Windows and Android paint their own pixels**, so there is no platform
object to hand over. A hatch there is Rust: one type that implements
`exact_linux::Hatches` in `modules/linux/*.rs` (or `modules/android/`,
`modules/windows/`), named once with `pub type ExactHatches = App;` and
included by the app's Linux crate (`contract::native::rust_hatch_entry` in its
`build.rs`, which the Rust-data apps here already call). Its handle carries the
node's box and words, the same acts, and two things in place of a view:
`element.overlay().draw(|c, w, h| …)`, a Canvas 2D recording that replaces the
last one whole, clipped to the node and painted over it; and
`context.observe(&element, |me, input, cx| …)`, the pointer and key input that
lands in the box, after Exact has handled it, to read only. It has the app and
window scopes, `context.frames` and `context.after`, and `diagnostics`. There
`changed` is also called when the node's size alone changes, and `ended` after
the commit that removed the node. An Android build is the same crate built for
Android; its words are the ones `app.json` gives `android`.

No hatch runs before first pixel. A node's end runs while its view is still
there, so take back there whatever the hatch added. On iOS a hatched node is a
view of its own and its list row is not reused unless the hatch sets
`element.reusable`; `logs` says what each word gives up.

**Into Contract only as a person could.** A handle acts on an authored node:
`click()`, `focus()`, `blur()`, and `input(text)`, which replaces a text
field's whole value without moving focus. Each is queued and runs after the
hatch returns. There is no dispatch and no state write.

**The frame clock.** `context.frames { frame in … }` (the web:
`exact.hatches.frames(frame => …)`) ticks once a frame after that frame's
tasks, and `context.after(ms) { … }` waits on the session clock. Under the
agent both run on the virtual display, so a drive repeats. They are for
behaviour, not for measuring the display. A view animated on the wall clock
(`UIView.animate`, a display link) is not on the agent's: `clock +N` leaves it
where it was, and only `clock +N real` lets it run, so a splash drawn from
`frames` or `after` is the one a drive can step past.

**The design's tokens** (LLP 1121 D12). A native view draws in the design's
colours by name: `ExactTamber(view).color(.surfaceCard)` on Apple (the view a
module made, or one inside it; in the process, a view under any node, or
`ExactTamber(session:)` for a screen of the host's own), a `UIColor` or
`NSColor` that resolves the scheme, contrast and elevation it is drawn in;
`ExactTamber(view).color("surface.card")` on Android, a `Color` int resolved
for the view's scheme. Each reads its place's app tuple: a view under an
`-exact-mode` pin answers that mode. `observe { … }` runs after each batch that
moved what the place reads (a `setMode`, a pin above it, a live edit, the
scheme, the root font size), so re-apply a colour kept in a layer there. A
name the design lacks answers the fallback (`color(_:fallback:)`) and is
logged once. `bun exact.mjs tamber gen swift .` writes the names
(`.exact/tamber/tamber.swift`, never committed), which a module's build
compiles in. `apps/tamber-host` is the sample.

**Whose window.** `window.window` is nil unless the embedder set
`session.hatchesOwnWindow`, and `app.application` unless it set
`session.hatchesOwnProcess`. The standalone iOS app sets both; the standalone
Mac app sets the first for each document's session.

**Make the hatch visible to the agent.**

| In hatch code | Where it shows |
|---|---|
| `diagnostics.log("…")` | `logs`, as `hatch element avatar: …` |
| `diagnostics.count("swipes")`, `publish("last", value)` | `state.hatches` |
| `diagnostics.measure("swipe.worst", ms:)`, `begin("swipe") … end()` | `perf hatches`, with Exact's own timing of every call |
| `element.owns(view: added, "what it is")`, `owns(recognizer:…)` | `tree`, under the node, beside what the host observes of it |
| `element.parts = [ExactPart(id: "seal", view: v, role: "button", label: "Verified")]` | `tree`; `tap avatar/seal` clicks it as a real pointer event |

Diagnostics are bounded and kept only in a development build. `EXACT_HATCHES=off`
(`?hatches=off` on a web page) runs a development build with no hatch
connected; the app must still work. If a run dies inside a hatch, the next
launch's `logs` say which one.

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
| `app_id()` | The app's identity, which names its storage: `com.example.my-app-3f9a2c` (`exact new` writes the name and six random hex digits), or for a game `com.exact.<Game::ID>` unless its app.json names one. A source with no id has no app storage on Apple and Linux. |
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
| `"fs.compressImage"` | `{"path", "destination", "maxDimension", "maxBytes"}`, and `"type"` (`"image/png"`; absent or `null` is JPEG) | `{"path", "type", "size", "width", "height"}`, as `storage.fs.compressImage` answers |

Grant what it touches: `sqlite.open app:/data/x.db`, `fs.read app:/data`,
`fs.write app:/data`. Storage is asynchronous and starts after first pixel. It
is absent at build, so `query` answers a placeholder, and in a scripted drive
that names no scratch store: there `response` is an error, and the source
answers as if nothing were kept. `--storage <name>` gives a drive a store kept
between native drives (`--reset-storage` empties it); on the web the next drive
keeps it with `--keep-storage`. An authored test gets an empty one, and its `reload` restarts
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
**secrets**: a session token, a key. Apple keeps them in the Keychain, Linux
in a file per secret only the user can read (`0600`, under
`$XDG_DATA_HOME/exact/<app id>/secrets`; not encrypted), and the web in
`localStorage`; Windows keeps them in memory for now. The host reads them into a snapshot before boot, so a
read is synchronous, and a scripted drive never keeps them. A best time is not
a secret: keep it in app storage.

## The six checks

```sh
cargo build --all-targets --keep-going                                  # build
cargo test --tests --no-fail-fast                                       # test
cargo clippy --all-targets --keep-going -- -D warnings                  # lint, and
cargo fmt --all -- --check                                              # lint (run both)
bun scripts/caps.mjs                                                   # caps
bun scripts/boot.mjs                                                   # boot graph
cargo check --workspace --all-targets --keep-going                      # workspace
```

The first three cover the root `default-members`: the deterministic, in-process
crates, the web host's and the Apple host's Rust among them (the Apple host's
real-socket, wall-clock and toolchain-launching tests are `async lane:`; a test
may still re-run its own binary to isolate its environment). The sixth compiles
every crate and target of the workspace and runs none of their tests: without it
a crate outside `default-members` can stop compiling on `main` while the other
five pass, as `exact-web-js` and `exact-linux`'s tests did on 2026-10-11. It
needs the pinned Bun first on `PATH` (or named by `BUN`): app crates' build
scripts run Bun, and under an older one each fails with `Bun <version> is older
than …`. The async lane runs the first three with
`--workspace` (the other hosts, GPU, Hermes, platform shells, stress fixtures),
which covers the sixth, and tests the separate game workspace with
`cargo test --manifest-path game/Cargo.toml --workspace --locked --lib --bins --tests`.
Changes to Apple host files, its Cargo path dependencies, or apps trigger the
iOS simulator tests, touch smoke and the tvOS compile
(`EXACT_JS_ENGINE=stub bun host/apple/build.mjs --tvos --no-install`: the Apple TV
bundle built and signed, installed on nothing, so it needs the tvOS SDK and
Rust's `aarch64-apple-tvos-sim` target and no simulator). Docs-only changes do not. The hourly
`--tier 2` lane installs that tvOS build on an Apple TV simulator and runs target-specific `cargo check` for the Linux
host and every Linux app (`-p '*-linux'`: each `apps/<app>/linux` crate is a
workspace member named `<app>-linux`) and for the Windows host and Windows Desk. Install
`x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` with `rustup target add`
and provide `zig` on the Mac: it compiles C, and C++ for the Linux target
(Hermes's JSI bindings). The check runs offline, so install the pinned Linux
Hermes bundle once per Hermes pin (again whenever the pin changes): `cargo run
--manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml --
--target x86_64-unknown-linux-gnu`. Until then the step fails, and the
`hermes-lean-sys` build error in its log names that command. These cross checks
cover compilation; native window presentation, KMS, and input devices remain
unqualified until run on their platforms.

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

A binary runs its own compiled data crate until an update replaces it, and only a
module that differs from that code is interpreted. The build records the digest of
the source the module compiles from (`nativeSource` in `scripts/rust.mjs`: the
module's local crates, the workspace's `Cargo.toml` and `Cargo.lock`, and the
compiler) and every Rust receipt carries it as `source`. A candidate whose
`source` is the binary's own becomes the native data source again
(`Swappable::with_native`). On iOS an update that changes the Rust runs
interpreted until a binary built from that source is installed; an update that
changes only the Contract or TypeScript leaves the Rust native.

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

### Intersection visibility

`reachstart` and `reachend` belong only to `list virtualized=true` with one
direct `each` (LLP 1010). Only a non-empty collection has edges: its geometric
window reaches the first or last supplied row. An empty collection fires neither
event. A plain list lays out all its children and has no collection window;
the Contract compiler refuses either handler there, on every target.

For a plain feed or masonry, use `intersect=` on a marker at the end of the
content, after the columns, inside the same scroller:

```text
view intersect=loadMore intersect-margin="0px 0px 800px 0px"
  height=1
```

`loadMore(entry: IntersectionObserverEntry)` should load only when
`entry.isIntersecting` is true, a next page exists, and no request is pending.
The bottom margin starts loading up to 800px before the marker enters the
scrollport. This observes the marker, not the last row of each column.
See [paging columns](recipes/responsive-columns.md#paging-columns) for a complete example.

`intersect=action` on a node offers a final `IntersectionObserverEntry` record
(`isIntersecting: bool`, `intersectionRatio: number`) after authored arguments.
The nearest ancestor scroll container is the root, otherwise the viewport;
threshold is fixed at 0. It delivers after mounting and on enter/leave, not
on every ratio change. Entries in one root/margin group follow observation order,
after resize callbacks and authored scrolling. `intersect-margin="200px 0px"`
is a literal rootMargin (one to four case-insensitive px/% lengths, default 0px).
Numbers follow CSS syntax: `.5px` and `1e2px` are valid; `1.px` is refused at compile time.
Percentages use the corresponding root dimension.
This can keep T3 sidebar subscriptions active near the viewport and track timeline turns.
Removal cancels observation without a leave callback. Both web targets use the
browser's IntersectionObserver after first paint. AppKit, UIKit and Linux use
the shared native sampler after resize settlement and presented scrolling.
Native inline runs, moving presentation geometry, rotated roots and SVG clip references remain unqualified;
percentage margins follow Chrome (LLP 1001). See LLP 1005's intersection
visibility deviation; other native hosts remain unqualified.

## Live time and progress

`time` is HTML's instant, in epoch milliseconds. `format` names the platform
presentation: `relative`, `timer`, `countdown`, `date-short`, `date-medium`
(the default), `date-long`, `date-full`, or `time-short` and `time-medium` (with seconds). Date and time styles use the viewer's locale and
time zone (Linux desktop currently uses English). Children are fallback text if the host cannot format the instant.

```contract
component Clock
  view
    column
      time datetime=1792209600000 format="relative"
        text "Departure"
      time datetime=1792209600000 format="countdown"
      progress start=1792209000000 end=1792209600000 aria-label="Trip"
```

Relative time changes at minute boundaries, timers and countdowns each second.
A timer counts elapsed or remaining time on either side of its instant; a
countdown stops at zero. The runner drives them without an app task, including
under the agent's `clock +60000`. A standalone live `time` reserves its box
at its font: two localized units for relative time, one extra hour digit
for a timer, and the first displayed shape for a countdown. Apple timers use
SwiftUI’s first field (`0:05`, `1:00:00`). A live `time` inside a `text` with other runs reflows as on the web;
make it its own text node when its width must stay fixed. Changing numeric
text uses tabular digits by default.

`progress start=… end=…` advances from zero to one over the interval and
clamps outside it. Supply both instants with `end` after `start`, without
`value` or `max`. The existing forms remain: `progress value=0.4 max=1` is
a determinate bar, and `progress` is the platform activity indicator. A
progress control takes no children; `aria-label` names it.

### Widget development gallery

A development Apple build with `widgets`, made with `--widget-gallery` or
`EXACT_WIDGET_GALLERY=1`, has **Widgets…** in its dev menu (Develop on macOS). It shows every declared family in full colour, accented and
vibrant modes, light and dark, at large and XXXL text sizes. It uses the entry's
headless plan and the extension's snapshot driver. In-process containers and
mode presentation are stand-ins; archive fidelity needs the real extension.

| Agent address | Reads |
|---|---|
| `bun scripts/agent.mjs ios widget next-trains --family systemMedium --mode accented --scheme dark --size xxxl tree "screenshot target/widget.png" state` | Snapshot tree, the entry alone, and its props |
| `bun scripts/agent.mjs macos widget next-trains tree state` | First declared family, full colour, light, large text |
| `bun scripts/agent.mjs ios widget next-trains --epoch 2026-08-28T09:05:00Z "clock +600000" tree state` | Refresh and render at the app’s advanced clock |

An immediate refresh answer supplies its first timeline entry. Pending, empty
or unavailable data keeps the preview; `state` reports which. `--props <json>`
replaces the preview props for this read and takes precedence over refresh. Widget
reads accept only `tree`, `screenshot <png>`, `state` and the app’s `clock`; film,
window, compare and `--scale` options are refused. Ordinary builds name the
`--widget-gallery` rebuild. Production builds include no gallery code.
