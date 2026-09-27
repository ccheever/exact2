# LLP 1031: Brownfield embedding — an Exact surface inside an app that already exists

**Type:** RFC
**Status:** Draft r2 (r1 written 2026-09-03 by Charlie Cheever / Codex; r2 the same day, folding a two-round three-family panel Charlie convened — grok-4.6 at xhigh, codex gpt-5.6-sol at ultra, and Claude Fable 5.1 reviewing independently before reading either; round 1 blind, round 2 mutually visible; artifacts `llp/reviews/1031-brownfield-embedding.{grok,sol,claude}.md`; dispositions in §8. Implementation began the same day under this document, in the order §4 gives; what has landed is marked **landed**)
**Systems:** Apple host (an instance-scoped C ABI; the `ExactSession`/`ExactView` Swift surface; the standalone apps as adapters), Runner (ownership per session; the request and store crossings), Data seam (host-supplied transport and storage over the existing grants), Native modules (native content inside Exact, unchanged), Build and packaging (the app artifact; the Swift package; optional artifacts as today), Delivery (the host app as carrier; the app-scoped owner of the update store and the dev connection), Agent API (session-scoped, no ninth operation), future Android host (a return trigger only)
**Author:** Charlie Cheever / Codex (r1); Claude (Fable 5.1) for Charlie Cheever (r2, the fold)
**Implementer:** Claude (Fable 5.1), from 2026-09-03 — this is the document the landing transcribes into, stage by stage (§4)
**Date:** 2026-09-03
**Related:** LLP 1000 (the layered crate graph and the optional-artifact rule), LLP 1005 (runner and `DataSource`), LLP 1008 (the Apple host as built; §4 the C ABI this instances), LLP 1009 D2/D4 (the GPU module as a separate on-demand artifact, loaded once), LLP 1010 (scrolling; nested chaining), LLP 1012 (the eight-operation agent API), LLP 1016 D1/D2/D5/D6 (requests as values, the ticketed queue, forgotten tickets, grants), LLP 1018 D1/D4/D6/D7 (the store: snapshot in, writes out; the Keychain's own scope), LLP 1019 (declared fonts; the catalog per plan), LLP 1023 D1–D3/D5/D10 (the envelope, the transactional swap, the identity gate, one plan), LLP 1024 (native content inside an Exact tree — the one mechanism), LLP 1026 D9–D12 (the update store, two policies), LLP 1027 D3/D4 (Hermes as an optional executor after first pixel), LLP 1029 D2/D6 (the composition names the executors), LLP 1030 D1/D3a/D4 and its brownfield-host row, LLP 1030.000 §6 (the host descriptor, pulled forward here), `rules/RULES.md` §Scope, §Budgets, §Agents, `rules/DEFERRED.md` §Surfaces and §Process

## Summary

Exact's first adoption unit in an app that already exists is an **embedded
surface**, not a second application. The native app owns its process,
window, scene, navigation, authentication, services, rollout, and
surrounding view tree. An `ExactSession` owns one runner, one plan, one
clock, its buffers, its journal, and its request queue; an `ExactView`
presents that session as an ordinary `NSView` or `UIView`; an `ExactApp`
— one per process, for the one Exact app the process links — owns what is
the app's and not a session's: its identity and compatibility id, its
asset resolver, its durable store, and its one dev or production
connection, from which sessions are made. More than one session may
live in a process, and destroying one releases everything attributable
to it and nothing that is not.

The standalone Exact apps become thin adapters over the same surface:
create a window, make one session, put its view in the window, keep the
dev menu. There is no second presenter, no runner mode, and no
brownfield-only semantic path — the standalone smoke drives the same
handle an embedder gets.

What a brownfield app links, said plainly (D6): **the app artifact**
`exact bake` / `build.mjs` produce — a static archive holding the runner,
the kernel, motion, the app's data crate, the baked plan, and the
compatibility id, exporting the C ABI under fixed names — plus the
`ExactKit` Swift package over that ABI, plus whichever optional artifacts
the app's composition names (the GPU module, the WebKit arm, native
modules, Hermes, wasmtime), each a separate file loaded on demand as
today. One Exact app per host process. A prebuilt, app-agnostic core is
not v1; its trigger is a TypeScript-only consumer.

The developer-facing promise, as it will actually read:

```swift
// Link lib<app>_apple.a (from `node host/apple/build.mjs --embed`) and
// the ExactKit package; no Rust toolchain in the consuming project.
// On iOS, run this from the app/scene/controller lifecycle after UIKit starts,
// never top-level before UIApplicationMain: a session already creates UIKit views.
let app = ExactApp.shared                        // the archive's app: identity, assets, the dev connection
let session = app.makeSession(delegate: self)    // one runner, one plan, one clock; boots on first layout
let exact = ExactView(session: session)          // an ordinary UIView / NSView
navigationController.pushViewController(ExactViewController(view: exact), animated: true)
// Iterate: `app.connect(devURL)` hot-reloads every session from dev.mjs;
// `session.apply(planBytes)` is the same restart-with-carry, by hand.
// Requests run on the library's own executor under the app's grants; the
// Keychain keeps the app's store. Replacing either is the contract D4
// states and a later landing.
```

Two facts the panel corrected and this revision states: a request never
reaches the presenter today and a host-performed request is therefore a
new ABI crossing with the grant check kept in Rust (D4); and CoreText
registration, a `dlopen`ed module, and a Metal device are process-wide
by platform, so D2's rule is "nothing *semantic* is shared," not
"nothing is shared" (D12).

The binary number is promising but not the product. Measured on this
checkout 2026-09-03, the complete Rust-only Apple executable is 2.48 MB
on macOS and 2.50 MB on an iOS device, about 1.0 MB under gzip — a
conservative proxy that includes the standalone presenter and Caltrain's
logic. The number that matters is the link delta against
`host/apple/macos/floor.swift`, measured by `metrics.mjs --long` once the
sample host exists (D7). What decides adoption is whether Exact feels
like adding a view library; a smaller binary that asks the native app to
reorganize around Exact's lifecycle is too expensive.

## 1. Motivation

The standalone hosts prove that one Contract source runs on the four v1
surfaces. A brownfield host asks a different question: can one
Exact-owned feature coexist with years of native code and leave the rest
alone? A team can then use Exact for one frequently changed surface —
onboarding, settings, a dashboard — while keeping the native shell,
login, navigation, analytics, purchases, and platform work, and gets the
Contract edit loop, the same rendering on every surface, and the agent's
deterministic tree, state, layout, and clock without rewriting its app.

The Apple host as built (LLP 1008) is not that product, and the panel
found each reason in the code:

- Every export was a process-wide singleton: `host!` put one
  `thread_local!` `RefCell<Bridge<D>>` behind fixed `#[no_mangle]` names
  (`host/apple/src/abi.rs`, before r2), and `exact_in`/`exact_out`/
  `exact_boot` took no instance (`exact.h`, v1). **Landed 2026-09-03:**
  every export takes a runtime handle (D2).
- `Bridge.swift` was a static `enum Exact` with one static wake; both
  `main.swift` files own the application, window, presenter, canvases,
  frames, clock, timers, dev plumbing, and the agent — a whole app, not a
  child view.
- `Text` is one static catalog and cache that a plan install wipes
  (`Text.swift` `install`), `webviews` and `canvases` are globals keyed by
  runner-local node ids, and `Presenter.viewport` is always the page's
  scroll container.
- A request never reaches Swift: `executor.rs` runs it on `ibex2::host`
  on the library's thread, where ibex2's `Bindings` enforce the grants.
- The dev loop — `EXACT_DEV_PLAN`, `PlanURL`, the dev menu — is wired in
  the mains around a process-global `PlanURL.boot` closure.

This RFC changes ownership, not Contract semantics. The kernel, plan,
runner, CSS vocabulary, events, store rules, delivery artifacts, and the
eight agent operations remain what they are.

## 2. Decisions

### D1 — Three owners: the app, the session, the view; one plan, N sessions

**`ExactApp`** — one per process, made from the archive the process
links, thin on the day it lands and growing only as its owned things
land (round 2's split, §8) — owns what belongs to the app and not to any
session:

- the identity (`app_id`), the compatibility id, and the host descriptor
  (D6), read from the archive;
- the asset resolver (the bundle, the app directory in dev, the update
  store's digest-addressed files once it exists) — every session
  resolves images, fonts, shaders, and deck pages through it;
- the durable store adapter (D4): one Keychain namespace per `app_id`,
  shared by every session of the app;
- one dev or production connection (D11): the `PlanURL` session in dev,
  the update store's selection and check in production; a verified
  candidate is staged once; every live session prepares it before the
  app commits its store, complete asset resolver, and sessions together;
- the loaded optional modules (D12), shared by nature.

**App-wide acceptance (Charlie, 2026-09-04).** One generation owns the
plan and the complete asset namespace. Preparation decodes and lays out a
candidate beside every live runner, including unmounted and not-yet-booted
sessions, with its own font catalog, carried state, and candidate delivery
facts. It releases no requests, secret writes, font registration, presenter
batches, or delegate callbacks. An app with no sessions validates with a
disposable runtime and starts no executor. Any refusal discards all candidates
and leaves the live store, runners, and resolver unchanged. Once every session
accepts, the store commits the same pinned identity; prepared runners swap
without another fallible layout, then the app presents batches and releases
callbacks. Newly created sessions take the app's committed generation.

The selected plan is verified during its mandatory read; assets verify lazily
once and retain immutable bytes. A selected roster replaces the namespace:
absent names hide embedded and older entries. Images, fonts, shaders, and local
decks consume those bytes; path-only consumers receive private materializations.
A draw receipt captures its session generation and bundle token when its node
is created. An old draw cannot bless a newer token. A corrupt initial candidate
falls back app-wide before any session runs; a later per-session refusal cannot
switch the resolver beneath sessions already running.

**`ExactSession`** — any number — is the unit of execution. It owns one
runtime handle (D2) and with it one runner, one decoded plan, its store
snapshot, its executor thread and request queue, its clock and timers,
its motion engine, its input/output buffers, its node ids, its journal,
and its agent (D9); on the Swift side its presenter (the batch applier
and node map), its text engine (the catalog and caches for its plan, D12),
its canvases, its web views, and its display link demand.

**`ExactView`** is the unit of presentation: an ordinary `NSView`/`UIView`
holding the session's viewport, mounted like any other view. A session
has at most one mounted view; moving it is unmount then mount on the main
thread; mirroring one session into two views is not supported. A session
may live unmounted (a native pop that keeps state): frames and
visibility-dependent GPU work stop, the runner stands, destruction is
explicit.

**Landed 2026-09-04, composition port:** `ExactKit` owns the generic
`ExactGeneration`, complete `AssetResolver`, and all-session prepare/commit
operation. `ExactUpdates` supplies immutable reads through the optional C
function table and owns store selection, commit, check, and generation-specific
boot marks. ABI v4 adds the prepare/commit/discard calls and token-aware table;
an embedded-only composition links neither this target nor its updater calls.

**One plan, N sessions.** In v1 a process links one Exact app, which has
one plan, one `app_id`, one compatibility id, one envelope, one stream,
and one store — everything LLP 1023 D10 and 1030 D3a already say — and any
number of sessions boot *that plan*. A host that wants three different
screens writes them as routes of one Contract and mounts one session in
three native containers, or waits: N entry `.contract` files sharing
`use`d components, baked against one data crate and listed in one
envelope under a `plans` map with the default under `plan`, one stream
carrying all of them, is the multi-surface shape, deferred with the
trigger *a named consumer whose second surface cannot be a route of the
first*. Nothing in v1 forecloses it: the store, the compatibility id, and
the identity gate are per app already. Two different Exact apps in one
process is not v1 either — their archives export the same C names (D6).

The standalone `ExactMac` and `ExactIOS` are adapters: the platform
application, one window, `ExactApp.shared`, one session, one view, the
default services, the dev menu. They remain the v1 apps and smoke
targets, so embedding cannot become a divergent presenter.

### D2 — Every ABI call takes a runtime handle; destruction is complete; re-entry is refused — **landed 2026-09-03**

The C ABI (`host/apple/include/exact.h`, v4) is instance-scoped:

```c
typedef uint32_t ExactRuntime;                     /* never 0, never reused */
ExactRuntime exact_create(void);
void exact_destroy(ExactRuntime rt);               /* idempotent */
void exact_set_measure(ExactRuntime rt, ExactMeasureFn measure, void *ctx);
void exact_set_wake(ExactRuntime rt, ExactWakeFn wake, void *ctx);
void exact_set_fonts(ExactRuntime rt, ExactFontsFn fonts);
uint8_t *exact_in(ExactRuntime rt, size_t len);    /* NULL: no such runtime */
const uint8_t *exact_out(ExactRuntime rt);
uint32_t exact_boot(ExactRuntime rt, float w, float h);
uint32_t exact_boot_plan(ExactRuntime rt, size_t len, float w, float h);
uint32_t exact_pump(ExactRuntime rt, double now_ms);
uint32_t exact_dispatch(ExactRuntime rt, uint32_t view, uint32_t kind, size_t len, double now_ms);
uint32_t exact_advance / exact_resize / exact_insets / exact_tick / exact_intrinsic / exact_agent (ExactRuntime rt, …);
```

- **The handle is an integer into a thread-local registry inside
  `exact-apple`**, not a pointer: the crate stays `#![deny(unsafe_code)]`,
  a destroyed or invented handle is refused *by name* — the returned
  batch's `error` reads `runtime N: no such runtime (destroyed, or never
  created)` — and handles come from one process-wide counter, never
  reused and unique across threads (a handle made on another thread is a
  stranger here too, which round 2 caught as the one invariant a
  per-thread registry needs), so a late call can never reach a
  successor. The registry holds nothing semantic: one `Bridge`
  per runtime, which the session owns through its handle; it is the shape
  the GPU module's ABI already uses for canvas ids. Setters replace a
  config struct so the library reads no pointer it did not hand out.
- **Re-entry is refused, never a trap.** A call on a runtime while a call
  on that runtime is in progress on the same thread — from the measure
  callback, from a delegate — gets a batch whose `error` is `runtime N:
  busy: a call is already in progress on this runtime`, written to a
  buffer no runtime owns, and the outer call is untouched. Under
  `panic = "abort"` (`Cargo.toml`) the old `RefCell` would have killed
  the host process. Cross-runtime calls on one thread are allowed.
- **Commands are delivered after the batch is applied**, FIFO, the way
  the presenter already queues view events during `apply` — so a
  delegate that pushes a screen, makes a second session, or calls back
  into this one runs against a settled tree. (Owed with the Swift
  extraction, §4 step 2.)
- **Threads.** All calls for one runtime are on its owning thread — the
  main thread under `ExactKit`, as LLP 1008 requires. The wake may arrive
  on the executor's thread after `exact_destroy` (a job already running):
  its `ctx` is the handle, the Swift side looks it up among its live
  sessions, and a stranger's wake is dropped; a pump on a dead handle is
  refused as above.
- **`exact_destroy`** drops the runner, the executor's sender (a job in
  flight finishes into a receiver that is gone), the buffers, the
  journal, and the store snapshot. The Swift side, before it: cancels
  its `PlanURL` subscription if it owns one, invalidates its display link
  and clock timer, destroys its GPU surface instances (never the device,
  D12) and web views, removes its views, and marks its generation dead so
  a late image decode or a late reply drops.
- **The ABI is not size-versioned before 1.0** (`rules/RULES.md`: no
  shims). The header carries `EXACT_ABI_VERSION`, which the compatibility
  id hashes (1030 D3a); a change is a new binary.

Verified: `host/apple/tests/host.rs` boots two runtimes in one thread with
different plans, viewports, and clocks, destroys one and refuses it by
name while the other answers, refuses an invented handle, proves handles
are never reused, and proves a setter takes effect at boot; the macOS
and iOS presenters boot and answer the agent through the handle.

### D3 — The containing app owns geometry and lifecycle; bounded first

The native parent sets the `ExactView` frame through Auto Layout or
frames. Its bounds are the Exact viewport; a bounds change is the
existing resize path. Safe-area and keyboard insets are computed in the
view's own container (`safeAreaInsets` of the view, the keyboard's
overlap with the view's window rect) and sent through the existing CSS
`env(safe-area-inset-*)` path, never from the process window.

**Bounded is the first and, in this landing, the only containment
shape:** width and height are finite, the surface is a viewport, its page
may scroll, exactly the standalone host's shape. Internal nodes with CSS
overflow keep their behavior (LLP 1010). **Content-height** — width
finite, height offered as max-content, the kernel's resulting root
height reported as the view's intrinsic height, the native scroll view
owning outer scrolling — is a named follow-up, not v1, because the code
has none of its pieces: `Host::layout` offers `Offer::definite(w, h)`
only, the page extent floors at the viewport, and a real UIKit pan does
not chain out of a nested scroll view (only the agent's wheel does). When
it is built its loop guard is: the assigned height is never a kernel
input in max-content mode; layout runs on a width, environment, or
commit change, never on the height the view itself reported; the
intrinsic height is invalidated only after a batch applies and only when
the point-rounded value changed; a second Auto Layout pass at the same
width returns the cached height without running the kernel; proven with
shrinking content, image arrival, a text-size change, a width change,
and a real nested pan on both platforms, in no more than two passes.

The view observes and forwards, per session: mount, unmount, foreground,
background, memory pressure; bounds, safe-area, keyboard, display scale,
appearance. **Mounted and visible participate in GPU frame demand** —
today both presenters judge visibility by the application's state alone,
which the extraction corrects: an unmounted view wants no frames.
Backgrounding stops display links and applies the store's existing
persistence; it invents no snapshot format.

**iOS native-owner teardown (2026-09-10, LLP 1035.001 D2/D3):** unmount
retires the session's sheets and navigation controllers without dispatching Back.
The runner and surviving route nodes remain; offscreen updates mount no native
controllers. Returning to a window projects current route intent and delivers
focus commands issued offscreen only after controller installation. Physical
unmount/destruction during a sheet drag and offscreen route replacement pass in
the two-session host (`/tmp/messages-unmounted-owner/`). **Follow-up, 2026-09-11:**
a different UIKit controller in the same window now acquires the current native
owners after removal from the old parent. Messages sheet transfer and return,
transfer during a physical sheet drag, and an offscreen replacement's queued
focus pass (`/tmp/messages-reparent-owner/`, LLP 1035.001 D2). The other session
remains usable. Existing focus ends at unmount; explicit pending focus survives.
This does not establish continuous editing or moving between windows.

### D4 — Existing native services cross the existing seams; the transport and store crossings are the contract, not this landing's code

A brownfield app must not rebuild its authentication, URL loading,
logging, or storage to render an Exact screen. The seams exist. What is
built now, and what is only specified, was the panel's round-2 verdict
(§8): **command, log, and plan application ship; ibex2 and the Keychain
stay the only performers; the request and store crossings are the
contract an adopter triggers**, because a security ABI with no assigned
consumer is what `rules/RULES.md` §Scope forbids, and the default
performer already honors every grant the day the handle is instanced.

**Built with the extraction:**

- **Commands** — post-commit intentions delivered after the batch is
  applied (D2), typed by the plan's `Value` JSON (records positional,
  unit and none `null`); the session's delegate receives name and
  arguments (D5).
- **Logging** — the session's journal lines and the host's stderr lines,
  as `logs` already merges them, into a sink the app supplies or stderr.
- **Plan application** — `session.apply(bytes)` and the app's connection
  (D11).
- **Assets** — resolved by the app's resolver (D1).
- **The defaults** — every request runs on the library's executor thread
  (`executor.rs`, `ibex2::host`) under the data crate's grants, exactly
  as today; the store is the Keychain (`store.rs`), one namespace per
  `app_id`, with **snapshot isolation**: a session sees the durable state
  at its boot, its own writes after, and a peer's only at its next boot
  or `data_ready`; the last persisted write per name wins; there is no
  cross-session notification (a reactive store would be the shared heap
  `rules/DEFERRED.md` refuses). Said loudly: a login in one session is
  invisible to a running peer. The fixture (D10) drives an allowed and a
  Rust-refused request through this path.

**The contract, when an adopter whose client or storage policy cannot be
wrapped triggers it** (the whole crossing lands then, never a partial
callback):

- The bridge checks each `RequestOut` against the data crate's
  `grants()` (ibex2's `GrantSet` is already linked) before anything
  leaves; a refused request is fulfilled as `Refused` inside the
  library. A granted one crosses as the batch op `{"op":"request",
  "ticket","generation","method","url","headers","body"}` — the web
  host's shape, LLP 1016 D2 — and the outcome returns through
  `exact_fulfill(rt, ticket, generation, status, len)` with the body in
  the input buffer, or `exact_fail(rt, ticket, generation, kind, len)`. A
  forgotten ticket drops (1016 D5); a stale generation, a duplicate, or
  a malformed completion drops before `Runner::fulfill`. **Redirects:**
  the host performer must not auto-follow; a redirect is a `fail` of
  kind `Redirect` carrying the target, which the library re-checks
  against the grants and re-issues under a new ticket, so every hop is
  decided in Rust. Completions may arrive on any thread; the Swift side
  copies canonical bytes into the session's queue and hops to the owner
  thread, retaining a reply port keyed by (handle, generation), never the
  runtime.
- The store keeps LLP 1018 literally: the granted names' snapshot is
  read outside any runtime call and passed at boot; the writes ride out
  after each commit as `{"op":"store","name","value"|null}`; ungranted
  names are refused in Rust (`Store::set`) before the host is asked.

This is dependency injection over values, not cross-runtime shared
data. Each service is named, granted, refused loudly, asynchronous where
it can be, and generation-scoped. A service implementation never receives
a kernel pointer and never mutates the tree behind a commit.

### D5 — Navigation and native interoperability are intentions, not ownership

The containing app owns its windows, scenes and existing controller hierarchy.
Navigation outside the Exact surface remains a command through the existing
post-commit path: the session's delegate receives its name and arguments and
decides. The SDK standardizes meanings the web already has (navigating to a URL);
product route names remain the app's, without a route registry or platform-suffixed
Contract.

**Clarified against the implemented native routes (2026-09-10, LLP 1035.001
D1/D4):** the Apple presenter creates child navigation controllers for routes
inside its surface, contained by the embedding controller. Exact manages those
controllers and their route views; it does not rewrite the embedder's existing
navigation stack. A sheet receives a separate Exact-owned navigation controller,
while the presenting owner keeps its parent, stack and scroll relationship. The
original blanket statement that Exact never reaches into a controller hierarchy
predated this native projection; the boundary is which controllers Exact owns.
For content the other way, LLP 1024 stays the mechanism; a native-module callback
is scoped to its session and generation.

### D6 — What a brownfield app links: the app artifact, the Swift package, the optional artifacts

The `host!` macro monomorphizes the C exports over the app's `DataSource`
type and bakes the plan bytes into the archive
(`apps/caltrain/apple/src/lib.rs`; `abi.rs`); LLP 1029 D2 makes that one
line the place an app names its executors. There is therefore no
app-agnostic "ExactCore" binary that contains the C ABI, and r1's D6
described one. The honest matrix, per composition (1029 D2):

| Composition | The app artifact (from `exact bake` / `build.mjs --embed`) | Reusable, unchanged per app | On demand, separate files |
|---|---|---|---|
| Rust-only (`data::App`, Caltrain) | `lib<app>_apple.a`: runner + kernel + motion + the data crate + the plan + the compatibility id | `ExactKit` (Swift, over `exact.h`) | GPU module, WebKit arm, native modules |
| TypeScript-only (`exact_js::Module`) | the same archive with an empty data crate; the plan and `.hbc` could be files | `ExactKit`; a prebuilt core is *possible* here and is the trigger for one | + Hermes (1027 D3) |
| Mixed (`Either<App, Module>`) | archive + bytecode | `ExactKit` | + Hermes |
| + `Swappable` | + the wasm module as a bundle file | `ExactKit` | + wasmtime (1029 D3) |

"Installs with Swift Package Manager and no Rust toolchain" is true for
the **consumer of a built artifact** — the archive and the package —
and false for anyone editing native Rust, which is `exact bake`'s job as
r1 said. **One Exact app per host process:** two archives export the same
names. A type-erased core (the exports written once in `exact-apple`, the
app registering a `Box<dyn DataSource>` and its bytes at `exact_create`)
is the door to a prebuilt core and to two apps in a process; it is not
opened here — nothing in v1 needs it, and the trigger is a
TypeScript-only consumer that wants a core it did not build.

The Swift package is **one package**, `host/apple/Package.swift`, with a
library target `ExactKit` (the session, the view, the presenter,
text, canvases, web views, the agent, `PlanURL`, the dev menu helpers —
platform halves under `#if os(macOS)`/`#if os(iOS)`) and executable
targets `ExactMac`, `ExactIOS` (the adapters) and `ExactHostMac`,
`ExactHostIOS` (the sample hosts, D10). It links the archive `build.mjs`
built; a brownfield consumer adds the package and the archive to its
project. `ExactKit` has no update owner or update commands. An L=A embedder
also links `ExactUpdates` and installs it on the app before creating sessions;
an L=0 embedder links only `ExactKit`. The standalone and sample executables
share two thin compositions, selected by `build.mjs` from Cargo's baked
actual-target compatibility output. `EXACT_APP_COMPOSITION` is the SwiftPM
build input it supplies (default `embedded` for direct package use), never a
runtime switch; the two compositions keep independent Swift scratch directories.
Those directories also include canonical app source, manifest id, destination and
trust policy (LLP 1036.000). A `--host` build publishes both standalone and sample
products; every driver resolves the selected app before opening either.
`--embed` carries that same baked compatibility and records the composition.
The lifecycle callback and complete asset provider in the core are generic;
normal data networking and the debug dev connection remain usable at L=0.
Optional artifacts stay what they are — `libexact_gpu.dylib`,
`libexact_web.dylib`, the native-module dylib, the Hermes and wasmtime
runtimes when 1027/1029 land — loaded once per process (D12), never cargo
features on a core crate.

**The host descriptor** (1030.000 §6, pulled forward): bake's output, not
the host's guess — the compatibility id plus what the archive links
(`L`, `E`, the roster, the grant ceiling) and where the store lives —
written into the archive beside the plan (`COMPAT`, 1030.000 stage 3) and
readable through `state.delivery`. A brownfield build does not generate
the containing app's `Info.plist` from Exact's `app.json`; that manifest
is the standalone adapter's (1030 D2). Privacy-manifest inputs and crash
symbols are the archive's build receipt (1030 D2).

### D7 — Optional weight is visible and separately payable

**Measured 2026-09-04** (`node scripts/metrics.mjs --long`, this Mac,
arm64): the sample host `ExactHostMac` — the archive and `ExactKit`,
nothing optional — is 2.51 MB against `floor.swift`'s 0.08 MB, a **linked
delta of 2.44 MB, 1.04 MB gzip**; beside it the optional GPU module
(`libexact_gpu.dylib`, dlopened at the first canvas) is 3.02 MB, 1.43 MB
gzip, and the web arm 0.16 MB, 0.04 MB gzip — each reported apart, never
folded in. `node host/apple/build.mjs --embed` (D10's promise) writes
what a consumer without a Rust toolchain links under
the app-owned Apple namespace returned by `appleArtifacts(app).embed`
(LLP 1036.000): the archive (`libcaltrain_apple.a`,
30.6 MB unstripped — the linked delta above is what a binary pays), the
C header, the GPU module, the shaders and assets, the cohort's
`compat.json`, and a receipt; `ExactKit` is the package at `host/apple`.

The base measurement is the **final linked delta against
`host/apple/macos/floor.swift`** — the empty AppKit app `metrics.mjs
--long` already builds — per architecture, from the sample host (D10)
linking the archive and `ExactKit` and nothing else; each optional
artifact reports the same delta; download compression is reported apart
from installed bytes; a fat artifact is never presented as a device cost.
The table lands with the measurement, not before it.

**First pixel, stated correctly.** Optional initialization — the GPU
module, Hermes, the WebKit arm — never blocks the applicable first-paint
gate: a brownfield host's own first paint precedes `prepare()`, which the
host calls when it likes; the standalone adapter's first pixel *is* an
Exact surface and its gate is the presenter's first draw, as today.
`prepare()` is that slot and nothing more — it creates no platform views,
runs no app logic before policy permits it, compiles nothing, and moves
nothing onto the boot path `scripts/boot.mjs` counts.

Memory is measured beside bytes: empty session, first surface, a second
session, the Hermes heap, the first GPU device. Two sessions pay two
Hermes runtimes; code pages are shared by the loader, heaps are not
(§5).

### D8 — Failure is contained to the surface as far as an in-process library can contain it

Unchanged in substance. Plan decode, compatibility, shape, service,
module-load, and update failures leave the prior good generation or a
developer-supplied fallback visible; a candidate swaps transactionally
(LLP 1023 D3); one session's refusal never resets another. `ExactView`
exposes preparing / ready / failed-with-a-refusal / destroyed and puts no
proprietary error screen in a release app. The boundary cannot promise
process isolation (`panic = "abort"`); what it promises is that expected
bad input and external failures are bounded, validated, and returned. Two
expects in the library's own path become structured failures with the
extraction: the executor thread's creation (`executor.rs`) and the
registry's.

### D9 — The eight agent operations become session-scoped; no ninth is added

`session.agent(request)` answers `tree`, `state`, `logs`, `settle` from
that session's runner; `layout`, `tap`, `type`, `screenshot`, `clock` are
its view's. Coordinates convert through `ExactView` to window and screen
space, so a native carrier synthesizes the tap at the right place. The
standalone carrier targets its one session; a sample-host carrier routes
by a host-owned label in the request — carrier routing, not an
operation (LLP 1012; `rules/DEFERRED.md`). Logs carry the session's
label and generation. Agent mode's memory store is per process, as
today: a drive starts from nothing.

### D10 — Two proofs: an in-repo sample host holds the contract; a named external app proves adoption

r1 said an in-repo showcase "would prove the opposite of the claim." It
would, if it were Caltrain in a second window. A **hostile sample host**
in-repo proves something else and is required by `rules/RULES.md`: only
a fixture the five checks and `scripts/smoke.mjs` can drive holds the
embedding contract when the next lane edits `Presenter.swift`.
`floor.swift` is the precedent. So, two proofs:

1. **The fixture** — `ExactHostMac` / `ExactHostIOS` (D6): a native
   application that is not Exact's — an AppKit window with a split view
   and a native table, a UIKit root with a navigation controller per pane —
   that links the archive and `ExactKit` and, driven by the eight
   operations through the existing carriers: creates two sessions of one
   plan with overlapping node ids and interleaves operations on them;
   mounts one bounded surface inside a native screen, unmounts and
   remounts it; handles an Exact command by pushing a native screen and
   returns with both sessions intact; issues an allowed and a denied
   request through a host performer and a store write through a host
   adapter; refuses a bad replacement plan and keeps the prior
   generation; exercises cross-session creation from a delegate and
   same-session re-entry refusal; destroys one session with a request
   and an image in flight and observes no late callback; and, once
   content-height exists, mounts one inside a native scroll container.
   Not a sixth check: a smoke drive.
2. **The proof** — an existing UIKit/AppKit application outside this
   repository, named by Charlie (§6), integrated by linking the artifact
   and adding the package with no Cargo in its build, and removed again
   without changing its service or navigation architecture. Until it is
   named, this document guides the refactor and packaging accretes no
   SDK machinery beyond what the fixture needs (§5).

### D11 — The dev loop is the session's and the app's, and the menu is the adapter's

An embedded surface hot-reloads from `dev.mjs` exactly as the standalone
app does, or it is not Exact. The primitives:

- **`session.apply(planBytes)`** — prepare/commit with `Runner::carry`:
  the transactional restart with carry, also available without a loader.
  Public, so a test or an embedder's own debug UI can drive it.
- **`app.connect(url)` / `app.disconnect()`** — the app's one dev
  connection: `PlanURL` per app, never per session (two sessions would
  otherwise open two streams and fight over one `current`). Each revision
  identifies the plan and complete asset manifest, with epoch and sequence
  ordering (LLP 1023 D3). Initial connection and reconnect fetch and verify
  the whole generation, prepare every live or unmounted session, and commit
  only after all accept under D1. Missing asset names are absent, including
  embedded names; image, font, deck, and shader consumers take the new resolver.
  The app remembers the committed content identity across connections, so
  an unchanged reconnect does not reapply; an explicit session or app apply
  invalidates it. Older fetch completions cannot commit after newer ones.
  A changed server program digest or `{rebuilt}` is terminal until the native
  binary is rebuilt; a process restart with the same program resets only the
  revision ordering. The app owns the program pin for its lifetime; public
  disconnect/connect cannot clear it. The optional production updater uses the same core
  acceptance primitive and owns its separate store policy.
- The adapters keep `EXACT_DEV_PLAN`, `EXACT_PLAN`, Open Project…, ⌘R,
  the four-finger tap, and the dev menu. The SDK installs none of them;
  an embedder that wants a debug affordance calls `connect` from its own.
- An `L = 0` embedder never links the store and still has `apply` and
  `connect` (a dev build) — the update economy is a policy, not a
  precondition of iterating.

### D12 — What is session-owned and what is deliberately process-shared

D2 in r1 said nothing remains process-global. The platform disagrees,
and the code wins:

| Thing | Owner | On session destroy |
|---|---|---|
| Runtime handle, runner, clock, timers, buffers, journal, executor thread, request queue, wake ctx | session | freed |
| Presenter, node views, canvases' surface instances, web views, display-link demand, agent | session | destroyed; instances `gpu_destroy`ed |
| Text catalog (stack → faces), font cache, paragraph cache | **session** (today's static is a bug under D1: a second plan's install wipes the first's) | dropped |
| CoreText file registration (`CTFontManagerRegisterFontsForURL`) | **process**, by platform; registered once per URL, never unregistered in v1 | nothing |
| `dlopen`ed GPU / WebKit / native-module libraries and their function tables; `gpu_load` | **process**, loaded once — a second `gpu_load` would replace the module state under the first session's surfaces | nothing (`dlclose` never, LLP 1024) |
| GPU device and queue | process (D7 permitted it) | nothing |
| Durable store namespace (Keychain items per `app_id`) | app (D4) | nothing |
| Asset resolver, update store, dev connection, compatibility id | app | nothing |
| `NSApplication` / `UIApplication`, scenes, windows, the run loop | the host app's | Exact never owned them |
| Agent mode's memory store, `EXACT_*` environment | process | nothing |

Every callback is keyed by handle and generation; shared objects hold no
session-local routing state.

## 3. What this deliberately does not add

- No new Contract tag, style row, event, state plane, or agent operation.
- No React tier, Rust-owned native application root, or platform-suffixed route.
- No shared heap between Swift, Rust, Hermes, or wasm; no reactive
  cross-session store.
- No requirement to ship Hermes, wasmtime, GPU, WebKit, or native modules.
- No dynamic delivery requirement. A baked, Rust-only surface with
  `L = 0` is the smallest and first embedding shape.
- No type-erased core, no prebuilt app-agnostic binary, no two apps in a
  process in v1 (D6, with the trigger).
- No content-height containment in this landing (D3, with the guard).
- No Android implementation in v1. `rules/DEFERRED.md` binds; a proven
  Apple `ExactSession`/`ExactView` plus a real adopter requiring mobile
  parity is the return trigger, with the named trade.
- No public compatibility promise before 1.0, no size-versioned ABI, no
  shims.
- No new packaging manifest, registry, check, devtools UI, or harness
  beyond the sample host the smoke drives.

This RFC requires no `DEFERRED` trade: it changes ownership and
packaging of systems already in v1 and adds no platform or runtime
feature.

## 4. Landing order

The implementer is named above; each step is verified by running and
transcribed here when it lands. Interleaved with LLP 1030.000's stages,
in the order round 2 converged on (§8):

1. **The handle** (D2) — **landed 2026-09-03**: `exact.h` v2, the
   registry with a process-wide counter, the refusals, two runtimes under
   test, both presenters over the handle. Before the update store, so
   the store is never born on a global.
2. **`ExactKit`: the thin app owner, the session, the view; the
   adapters** (D1, D11, D12) — **landed 2026-09-03**: one Swift package
   (`host/apple/Package.swift`: the `ExactKit` library, `ExactMac`,
   `ExactIOS`, `ExactHostMac`, `ExactHostIOS`); `TextEngine` per session
   with `FontRegistry` process-wide; `Canvases` and `WebViews` per
   session over modules loaded once (`GpuModule.loadShared`); the wake
   keyed by handle through a live-session table; commands queued and
   delivered after apply; `session.apply`, `app.connect`; both
   `main.swift` files reduced to adapters over `ExactApp.shared`, one
   session, one `ExactView`, the dev menu; `Presenter.swift` split into
   a node view and a presenter per platform under the cap; the macOS,
   iOS, and web smokes green through the new handle only (macOS 12.2 s,
   iOS 29.7 s, web 10.2 s).
3. **The sample host** (D10) — **landed 2026-09-03 on macOS**:
   `ExactHostMac`, driven by `scripts/smoke.mjs host` in 7.4 s — two
   sessions of one plan with overlapping node ids answering apart; a tap
   on one leaves the other untouched; each session's own clock; a
   command from one pushing a native screen over the other (unmounted,
   its tree still answering) and popping it (remounted, laid out again,
   state kept); a bad candidate plan refused with both apps kept; one
   session destroyed under the other, its handle refused by name (`session
   a: destroyed (runtime 1: no such runtime)`) while the other answers.
   Each request names its session by label (`--session`), routing in the
   carrier. Not exercised, honestly: a request in flight (Caltrain's data
   crate makes none — a fetching app is the fixture for that), a busy
   handle from a delegate (the Rust tests hold the refusal), and the iOS
   sample host (a placeholder target that builds; the fixture is owed).
   **And on iOS, 2026-09-04:** `ExactHostIOS` — a UIKit app whose root
   stacks a native header over two panes, each a session inside its own
   navigation controller (the phone's shape of the split view: a native
   screen pushed over session a covers a's pane and no other, which is
   what lets the smoke keep tapping b under it — one stack for both
   sessions refused the tap as off screen, a faithful first attempt).
   `build.mjs --ios --host` assembles its own bundle, `<app id>.host`,
   beside the app's on the simulator; `scripts/agent.mjs host-ios` is
   the socket carrier with the stdio carrier's session routing;
   `scripts/smoke.mjs host-ios` runs the same five steps unchanged,
   green in 3.5 s at a phone's width (402).
4. **1030.000 stage 1, the asset row** — against the app's asset
   resolver, per app, never `EXACT_ASSETS` alone; the shader packaging
   (1030 D8) lands from its own lane and joins it.
5. **The descriptor** (D6) — bake emits the archive's descriptor beside
   the plan once the compatibility-id lane joins; stage 2's remainder.
6. **1030.000 stage 3** — the classifier, `exact deploy --dry-run`, the
   `delivery` resource and commands, `state.delivery` per session — the
   honest `L = 0` answer first, with no updater.
7. **1030.000 stage 4** — the update store as `ExactApp`'s connection
   under the production policy; `PlanURL` and `EXACT_DEV_PLAN` collapse
   into it (1026 D12); then publishing: the origin's conditional put,
   per-stream heads, `--watch`.
8. **Packaging and the measurement** (D6, D7) — `build.mjs --embed`
   producing the archive, the header, and the package a consumer adds;
   the link delta against `floor.swift` in `metrics.mjs --long` for
   `L = 0`, `L = A`, and each optional artifact.
9. **Deferred with a trigger:** the request and store crossings (D4),
   content-height (D3), the external proof (D10) when Charlie names it.

Real dependencies: 1 before 7; 2 before 3, 4, and 8; the compatibility id
and shader lanes before 5 and 6; 6 before 7. 1030.000 stage 2 (the
manifest, the generated host files, the receipt — **landed 2026-09-03**
for the standalone adapter) depended on none of these.

**As built, the shape an embedder sees** (`host/apple/Sources/ExactKit`):
`ExactApp.shared` (the asset root, `makeSession(delegate:label:)`,
`connect(_:)`/`disconnect()`, `apply(_:label:)` to every session);
`ExactSession` (`boot(size:)`, `boot(plan:size:)`, `apply(_:label:)`,
`resize`, `insets`, `agent(_:)`, `destroy()`, `state`, `generation`,
`clock`, the boot numbers); `ExactSessionDelegate` (`command`,
`didChange state`); `ExactView(session:)` on both platforms (boots at
its first real size when the adapter has not, resizes on frame changes,
sends its own safe-area insets, stops frames when unmounted); `Agent`
(the carrier's `startStdio(sessions:)` / `startSocket(ready:sessions:)`
routing by label); `DevMenu` (the adapters' only). `node
host/apple/build.mjs [--ios] [--host]` builds the products from the one
package; `scripts/agent.mjs host --session <label> …` drives the sample
host.

## 5. Open questions and leanings

1. **Is one session allowed to survive with no mounted view?** Yes (D1):
   it stops frames and visibility-dependent work; destruction is explicit.
2. **Content-height in the first implementation?** No (D3); the guard is
   written; the probes decide when.
3. **Which host services are in the first package?** Command, log,
   `apply`/`connect`; ibex2 and the Keychain as the only performers; the
   transport and store crossings when an adopter triggers them (D4).
4. **Does `ExactSession` own a serial queue?** No: caller/main-thread
   serialization, matching LLP 1008; replies already hop.
5. **Can multiple sessions share one Hermes runtime?** No in v1: shared
   code pages, separate heaps; measure before changing it.
6. **One distributable binary for core and Swift UI?** The archive plus a
   Swift package; XCFramework packaging follows the measured link loop
   (§4 step 8), not aesthetics.
7. **The first real brownfield consumer?** Unnamed; Charlie's (§6).

## 6. Acceptance questions for Charlie

1. Is **embedded surface as primitive; standalone app as adapter** the
   governing direction? (The panel: yes, unanimously.)
2. **Multiple isolated sessions in one process from the first release?**
   The panel: yes for isolation — the handle and the fixture prove two —
   and no for N *products* (D1: one plan, N sessions). Confirm.
3. **Bounded-only in this landing**, content-height a named follow-up
   with its guard (D3)? The panel: yes.
4. **The app artifact as the unit**, no type-erased core in v1 (D6)?
   The panel, unanimously after round 2.
5. **Which existing native application is the first consumer?**

## 7. What this amends, at acceptance

- **LLP 1008 §4** — the C ABI is v2, instance-scoped (D2); the header's
  own comment carries the rule.
- **LLP 1030.000 §6** — the host descriptor and the embedding SPI leave
  the deferred list: the descriptor is bake's output beside the plan
  (D6, §4 step 6); `brownfield: true` publishes against it.
- **LLP 1026 D12 / 1030.000 stage 4** — the update store is owned by
  `ExactApp`, per app, and `PlanURL`'s collapse into it happens through
  `session.apply` (D11).
- **LLP 1018 D6** — "nothing crosses the ABI" gains the exception D4
  names: an embedder's store adapter, snapshot in and `store` ops out,
  with the grant check still in Rust.

## 8. Panel fold (r2, 2026-09-03)

Three families, two rounds; round 1 blind, round 2 with all three
answers visible. The artifacts carry both rounds verbatim. What each
catch did here:

| Catch | Who | Held? | Where it landed |
|---|---|---|---|
| ExactCore-as-`host!` cannot be app-agnostic; the archive is the unit; two archives collide on the C names | grok B1, sol Q1, claude | yes | D6, Summary |
| Several surfaces undecided; one plan, N sessions in v1; N plans deferred with a trigger | grok M1, sol Q2, claude | yes | D1 |
| A request never reaches Swift; the grant check must stay in Rust; the crossing is a batch op + `fulfill` | grok B2, sol Q3, claude | yes | D4 |
| Redirects must not be auto-followed by a host performer | sol Q3 | yes | D4 |
| Re-entry on a `RefCell` under `panic=abort` kills the host; refuse, never trap; commands after apply | grok B3, sol Q9, claude | yes | D2 (landed) |
| "Nothing process-global" is false; a table of what is shared | grok B4, sol Q8, claude | yes | D12 |
| A second `gpu_load` replaces the first session's module state | sol Q8 | yes | D12 |
| Unmount's promise is false today: visibility is app-level | sol Q8 | yes | D3 |
| The dev loop is absent; `apply` + `connect`; menus stay in the adapter | grok M2, sol Q4, claude | yes | D11 |
| The store across sessions: per app, snapshot-isolated, last write wins | grok M3, sol Q5, claude | yes | D4 |
| An app-scoped owner above the session for the store, assets, and the connection | sol Q4 | yes — `ExactApp` | D1, D11 |
| Content-height unsafe as specified; bounded-only; the loop guard | grok M4, sol Q6, claude | yes | D3 |
| An in-repo fixture is required; the external app is the proof | grok M5, sol Q7, claude | yes | D10 |
| D7's first-pixel claim is false for the standalone adapter; `.expect` in the executor | sol Q11 | yes | D7, D8 |
| The descriptor is on two calendars (1031 D6 vs 1030.000 §6) | grok M6, sol Q4 | yes — pulled forward | D6, §7 |
| Font catalog per session; registration process-wide, by URL | grok B4, claude | yes | D12 |
| The integer handle into a registry, no `unsafe` | claude (built); sol preferred a pointer | see round 2 | D2 |
| Do not size-version the ABI before 1.0 | grok s7 | yes | D2 |
| Measure against `floor.swift` in `metrics.mjs --long` | grok s5, claude | yes | D7 |
| `prepare()` must not warm engines on the boot path | grok m2 | yes | D7 |
| `Presenter.swift` must split under the cap | grok m3 | yes | §4 step 2 |
| Type-erase now to open the prebuilt-core door | grok B1/s1 | not in v1; the trigger is named | D6, §3 |

### Round 2

All three answers visible; each wrote only where it differed, then took
a final position on six forks. Where they landed, and what this document
does:

| Fork | Final position (both external families) | Here |
|---|---|---|
| F1 the unit | the app-specific archive from `host!`, the Swift package, optional artifacts; grok retracted "type-erase now" — the trigger is a TypeScript-only consumer, a second app in one process, or an erased-`Host` prototype in hand | D6, §3 |
| F2 the handle | the integer handle as built, not a pointer — sol withdrew "contrary to ownership"; sol's invariant taken: ids process-unique from one atomic counter, never per thread, exhaustion refused | D2 (landed, with the counter and a cross-thread test) |
| F3 the app owner | split: grok "not this push, `applyPlan` only, the owner when the store lands"; sol "a thin owner now, before the asset row"; both agree loaders and menus stay in the adapter and that a per-session `connect` is wrong | a **thin** `ExactApp` with the extraction — identity, the asset root, the sessions, the one `PlanURL` connection that exists today — growing when the compatibility id and the store land; `session.apply` public (grok: an `L = 0` embedder must not be forced through a store) | D1, D11 |
| F4 the crossings | do not build them this push: command, log, plan application ship; ibex2 and the Keychain stay the performers; the fixture drives an allowed and a Rust-refused request through the default executor; the full crossing (redirects re-decided in Rust, reply ports, `(generation, ticket)` queues, snapshot before boot) is the contract when a named adopter's client cannot be wrapped — whole, never a partial callback | D4 rewritten; D10 narrowed |
| F5 several surfaces | one plan, one `app_id`, N sessions; grok's correction taken — the plan's own state can select a screen and one session can be remounted in three containers, so "it cannot" was too loud | D1 |
| F6 the order | extraction → fixture → asset row (with the shader lane) → descriptor → classifier and dry-run → `delivery` → update store and publishing → packaging and measurement; crossings deferred; the two real chains: ownership before the fixture, resolver, and store; artifact truth (shaders, compatibility id, descriptor) before classification, classification before publishing | §4 |

## Ratification note

Draft r2, 2026-09-03, r1 and r2 the same day; the panel's two rounds
are folded and its three artifacts carry them verbatim. Implementation
began the same day under §4 with the implementer named above; steps
marked **landed** are transcription, the rest are decisions this
document will transcribe as they land. Nothing measured beyond the
proxy in the Summary; D7's table waits for the sample host.
