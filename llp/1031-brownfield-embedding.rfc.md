# LLP 1031: Brownfield embedding — an Exact surface inside an app that already exists

**Type:** RFC
**Status:** Draft
**Systems:** Apple host (instance-scoped C ABI, UIKit/AppKit container), Runner (instance ownership and lifecycle), Data seam (host services), Native modules (native content inside Exact), Build and packaging (prebuilt core plus optional artifacts), Delivery (the host app as carrier), Agent API (instance-scoped access), future Android host (return trigger only)
**Author:** Charlie Cheever / Codex
**Date:** 2026-09-03
**Related:** LLP 1000 (the layered crate graph and optional-artifact rule), LLP 1005 (runner and `DataSource`), LLP 1008 (the Apple host as built), LLP 1009 D2/D4 (GPU as a separate on-demand artifact), LLP 1012 (the eight-operation agent API), LLP 1016 D1/D2/D6 (requests, replies, and grants), LLP 1018 (the store), LLP 1024 (native modules — native content inside an Exact tree), LLP 1027 D3/D4 (Hermes as an optional executor after first pixel), LLP 1029 D2/D6 (an app names the executors it carries), LLP 1030 D1/D3a and its brownfield-host row (artifacts, compatibility, and the host app as carrier), `rules/RULES.md` §Scope and §Budgets, `rules/NOT-DOING.md` §Surfaces and §Process

## Summary

Exact's first adoption unit in an app that already exists is an **embedded
surface**, not a second application. The native app owns its process, window,
scene, navigation, authentication, services, rollout, and surrounding view
tree. An `ExactSession` owns one runner, plan, store namespace, executor
composition, and clock; an `ExactView` presents that session as an ordinary
`UIView` or `NSView`. More than one session may live in a process, and destroying
one releases everything attributable to it.

The standalone Exact apps become thin compositions over the same surface. They
create a window, put one `ExactView` in it, and provide the default services.
There is no second presenter, runner mode, or brownfield-only semantic path.

The developer-facing promise is deliberately small:

```swift
let session = try ExactSession(
    plan: plan,
    services: appServices,
    store: appStore
)
let exact = ExactViewController(session: session)
navigationController.pushViewController(exact, animated: true)
```

Integrating a prebuilt surface requires Swift Package Manager and no Rust
toolchain. Editing native Rust logic still requires the Exact build, as it must:
the result is an app artifact linked into the host. Hermes, wasmtime, GPU, web
content, and native modules remain separate artifacts selected by the app; none
becomes a feature on the core host.

The binary number is promising but not the product. Measured on this checkout
2026-09-03, the complete Rust-only Apple executable is 2.48 MB on macOS and
2.50 MB on an iOS device, about 1.0 MB under gzip. Those are useful conservative
proxies, not yet an incremental brownfield link measurement: they include the
standalone presenter, Caltrain logic, and executable framing. The current Apple
GPU module is another 3.11 MB; the WebKit arm 0.16 MB; LLP 1027 measured lean
Hermes at +1.81 MB; LLP 1028 measured wasmtime's runtime at +0.49 MB on desktop
and +0.58 MB with Pulley. A Rust-only app without dynamic Rust delivery should
therefore expect **about 3 MB uncompressed at first estimate**, while an app that
selects optional capabilities pays each separately.

What decides adoption is whether Exact feels like adding a view library. If it
asks the native app to reorganize around Exact's lifecycle, networking, state,
or navigation, even a smaller binary is too expensive.

## 1. Motivation

The standalone hosts prove that one Contract source can run on the four v1
surfaces. A brownfield host asks a different question: can one Exact-owned
feature coexist with years of native code and leave the rest of the application
alone?

That is a useful adoption wedge. A team can use Exact for one frequently changed
surface — onboarding, settings, account management, commerce, an operational
dashboard, or a content feature — while keeping the native shell, login,
navigation, analytics, purchases, and platform-specific work. The team gets the
Contract edit loop, the same rendering semantics on each Exact surface, and the
agent's deterministic tree/state/layout/clock without first rewriting its app.

The current Apple shape is not yet that product:

- `exact_in`, `exact_out`, `exact_boot`, and every subsequent operation have no
  instance parameter (`host/apple/include/exact.h:85–119`). The generated host
  holds one process-global bridge.
- `host/apple/swift/Bridge.swift` wraps that global state in a static `Exact`
  enum, including one static wake callback.
- the macOS and iOS entry points own the application, window, global clock,
  presenter, timers, GPU manager, and dev plumbing. They compose a whole app,
  not a child view.
- `Presenter.viewport` is always the page scroll container. That is correct for
  a standalone page but does not state how it participates in an existing
  native scroll hierarchy.

This RFC changes ownership, not Contract semantics. The kernel, plan, runner,
CSS vocabulary, events, store, delivery artifacts, and eight agent operations
remain the same.

## 2. Decisions

### D1 — The embedded surface is the host primitive; standalone is an adapter

`ExactSession` is the unit of execution. It owns:

- one decoded/running plan and its carried state;
- one `DataSource`/executor composition;
- one store namespace and request queue;
- one logical clock, timer set, and motion evaluator;
- the input/output buffers used by its ABI;
- the node ids, logs, and agent journal for that plan;
- references to optional modules selected for it.

`ExactView` is the unit of presentation. It owns the platform node mirror for a
session and is mounted like an ordinary native view. `ExactViewController` is a
convenience container, not a second runtime. The native app creates and places
either; Exact never creates an application, scene, window, navigation
controller, or process-global menu in the embedding package.

The existing `ExactIOS` and `ExactMac` executables become samples/adapters:
create the platform application and window, create one session and one surface,
install the default service implementations, and run. The same adapter remains
the v1 app and smoke target, so embedding cannot become a divergent presenter.

One session has one mounted view in v1. Moving it between containers is
unmount/mount on the main thread. Mirroring one live session into two views is
not supported: input focus, scroll offsets, and native control identity make
that a different feature.

### D2 — Every ABI operation is instance-scoped, and destruction is complete

The Apple C ABI gains an opaque `ExactRuntime *`. In shape:

```c
ExactRuntime *exact_create(const ExactRuntimeConfig *config);
void exact_destroy(ExactRuntime *runtime);
uint8_t *exact_in(ExactRuntime *runtime, size_t len);
const uint8_t *exact_out(ExactRuntime *runtime);
uint32_t exact_boot_plan(ExactRuntime *runtime, size_t len,
                         float width, float height);
uint32_t exact_dispatch(ExactRuntime *runtime, uint32_t view,
                        uint32_t kind, size_t len, double now_ms);
```

Every existing call takes the handle: boot, pump, dispatch, advance, resize,
insets, tick, intrinsic size, fonts, and agent. Callbacks receive the runtime or
their instance context. No runner, bridge buffer, font catalog, wake callback,
clock, store, or journal remains process-global.

Calls for one runtime stay on one owning thread, as LLP 1008 requires. Different
runtimes may have different owning threads, but the UIKit/AppKit wrapper uses
the main thread. A callback may enqueue a reply from another thread; the wake
returns to that runtime's owner.

`exact_destroy` is idempotent at the Swift ownership layer and one-shot at the C
boundary. It:

1. marks the runtime dead so late callbacks are dropped;
2. cancels or detaches its outstanding requests;
3. destroys its GPU/web/native surface instances;
4. invalidates timers and frame requests;
5. clears its view ids, store handles, buffers, and journal;
6. frees the runner.

Callbacks carry a generation/token so a reply from a destroyed or rebooted
session cannot enter a replacement. The current rule that a destroyed node does
not hear a late event extends to the runtime itself.

The ABI is size-versioned when exposed as a distributable SDK. Before 1.0 it has
no compatibility promise (`rules/RULES.md`); there are no shims or legacy entry
points. The handle shape is chosen now so reaching 1.0 does not require turning
a singleton into an instance under adopters.

### D3 — The containing app owns geometry and lifecycle

The native parent sets the `ExactView` frame through ordinary Auto Layout or
frame layout. Its bounds are the Exact viewport; a bounds change calls the
existing resize path. Safe-area and keyboard-derived insets are computed in the
view's actual container, not from the process window, and sent through the
existing CSS `env(safe-area-inset-*)` path.

An embedded surface does not assume it is the root scroll view. Internal nodes
with CSS overflow keep their Exact/native scroll behavior. For the outer edge,
the container chooses one of two constraint shapes rather than an Exact-specific
behavior vocabulary:

- **bounded:** width and height are finite; the surface is a viewport and its
  page may scroll, like the standalone host;
- **content-height:** width is finite and height is offered as max-content; the
  kernel reports the resulting content height as the native view's intrinsic
  height, and the containing native scroll view owns outer scrolling.

The second shape must be proven against real UIKit/AppKit layout before its
syntax is fixed. It may use an SDK configuration value because it describes the
native containment offer, not a Contract semantic. Nested scrolling must chain
at edges exactly as LLP 1010 requires; the parent receives motion the child
cannot consume.

The surface observes and forwards, per instance:

- mount, unmount, foreground, background, and memory pressure;
- bounds, safe-area, keyboard, display scale, and appearance changes;
- locale, accessibility text size, reduced motion, and increased contrast once
  those values have Contract/CSS representations.

Unmount pauses platform frame production and visibility-dependent GPU work but
does not destroy session state. Destruction is explicit. Backgrounding stops
display links and applies the store's existing persistence rule; it does not
invent a second snapshot format.

### D4 — Existing native services cross the existing seams

A brownfield app must not rebuild its authentication, URL loading, analytics,
purchases, or storage merely to render an Exact screen. `ExactServices` supplies
host implementations of the seams Exact already has:

- **transport/request executor:** the runner's request value is performed by
  the app's existing client and replied to through LLP 1016's tokened queue;
- **store:** a namespaced implementation of LLP 1018, backed by the app's
  chosen storage and protection class;
- **commands:** post-commit intentions from the plan to the native coordinator;
- **logging:** structured Exact records into the app's existing log/crash sink;
- **assets:** resolution through the host's bundle/cache policy where needed.

Values crossing the boundary use the plan's canonical `Value` bytes and shape
checks, not an open object bridge. Each service has a declared name and grant;
unknown or ungranted work is refused loudly. Replies are asynchronous and
generation-scoped. A service implementation cannot receive a kernel pointer or
mutate the tree behind a commit.

The default standalone adapter supplies the current ibex2 request executor and
store. An embedder may replace either without replacing the runner. TypeScript
and Rust logic continue to see the same data seam; they do not learn whether a
request was performed by Exact's default executor or the containing app.

This is dependency injection, not cross-runtime shared data. The native app owns
a service; Exact calls it with values. `rules/NOT-DOING.md`'s refusal of a shared
heap remains intact.

### D5 — Navigation and native interoperability are intentions, not ownership

Exact never reaches into a `UINavigationController`, window, scene, or AppKit
controller hierarchy. A Contract action may emit a command through the existing
post-commit command path. `ExactSessionDelegate` receives the command's name and
typed arguments; the containing app decides whether it means push, present,
dismiss, open a URL, update native chrome, or refuse.

The SDK standardizes only meanings that already have web names and semantics,
such as navigating to a URL. Product route names remain the app's. There is no
Exact route registry for a brownfield app and no platform-suffixed Contract
override.

For content in the other direction, LLP 1024 remains the one mechanism: a
hyphenated Contract tag resolves to a factory in the app's native-module
artifact and produces an ordinary platform view inside the Exact box. Embedding
does not add a second plugin system. Native module callbacks are scoped to the
owning session and invalid after its generation changes.

### D6 — Installation uses prebuilt packages; authoring tools stay out of the app

The Apple distribution shape is a Swift package with a small source wrapper and
separate binary artifacts:

- **ExactCore** — runner, kernel, plan decoder, motion, C ABI;
- **ExactAppleUI** — the UIKit/AppKit presenter and `ExactSession` wrappers;
- **ExactHermes** — only for a composition naming `exact-js`;
- **ExactWasm** — only for a composition naming `Swappable`;
- **ExactGPU** — the app's GPU module, only when the app has a canvas;
- **ExactWeb / app native modules** — separate on-demand artifacts as today.

These are packages/artifacts, never cargo features on a core crate. An app links
only the executors and modules named by its baked host descriptor (LLP 1029 D2,
LLP 1030 D3a). Linkage and the descriptor must agree; a plan requiring an absent
executor, module roster, or grant is refused before boot.

A developer embedding an already-built Exact bundle installs the package and
does not install Rust, Hermes tools, wasm tools, or wgpu. `exact bake` is the
producer of the plan, host descriptor, and any app-specific native artifact.
Someone editing a Rust data crate still invokes that producer; hiding a native
compiler behind Xcode would not remove the toolchain, only make its failures
opaque.

The SDK ships headers/module maps, privacy-manifest inputs derived from the
selected capabilities, symbols for crash attribution, and one installation
receipt. It does not create another manifest authority: LLP 1030's app manifest,
host descriptor, compatibility id, and build receipt remain the declarations.

### D7 — Optional weight is visible and separately payable

The base measurement is the final linked delta against a minimal host app, per
architecture, not the size of a static archive. Each optional artifact reports
the same delta. App Store/download compression is reported separately from
installed Mach-O bytes; universal/fat artifacts are never presented as a device
cost.

The target table for an embedding release has at least:

| Selection | Incremental installed bytes | Compressed estimate | Loaded before first Exact surface | Loaded before the containing app's first pixel |
|---|---:|---:|---|---|
| Core + Apple UI + Rust logic | measured link delta | measured | yes | no |
| lean Hermes | +delta | +delta | only for TS session readiness (LLP 1027 D4) | no |
| wasmtime runtime | +delta | +delta | only when selected | no |
| GPU module | +delta | +delta | after a canvas box paints | no |
| WebKit/native module arms | +delta each | +delta | first use | no |

The containing app's first pixel is outside Exact and always precedes Exact's
optional initialization. An embedder may call `prepare()` after its own first
pixel to verify a plan and prepare selected executors; `prepare()` does not
create platform views, run app logic before policy permits it, or compile code.

Memory is a first-class measurement beside binary bytes: empty session, first
surface, second simultaneous session, Hermes heap, and first GPU device. Engines
may share immutable code pages naturally; runtime heaps, stores, clocks, and
journals do not become global to manufacture a smaller number. A GPU module may
share one device where wgpu/platform rules permit, while every surface instance
and its generation remain owned.

### D8 — Failure is contained to the surface as far as an in-process library can contain it

Plan decode, compatibility, shape, service, module-load, and update failures
leave either the prior good generation or a developer-supplied native fallback
visible. A candidate plan swaps transactionally as LLP 1023/1030 require. One
session's refusal does not reset another session.

`ExactView` exposes a small state callback: preparing, ready, failed with a
structured refusal, and destroyed. It does not put a proprietary error screen
into a release app unless the embedder supplies one. The host may disable a
surface or select the embedded fallback through LLP 1030's delivery policy.

The boundary cannot promise process isolation: a native memory-safety defect,
platform abort, or `panic=abort` bug can terminate the containing process. The
honest promise is that all expected bad input and external failures are bounded,
validated, and returned rather than trapped; optional modules fail their node
or session as their governing LLP says. A team requiring crash isolation needs
another process, which is not this embedding model.

### D9 — The eight agent operations become session-scoped; no ninth is added

`ExactSession.agent(request)` invokes LLP 1012 against that session. `tree`,
`state`, `layout`, `logs`, and `clock` therefore remain useful inside a larger
native test without pretending to describe the host's whole view hierarchy.
Coordinates returned for an Exact node are convertible through `ExactView` to
window/screen coordinates, so the existing native carrier can synthesize the
same tap or type at the correct place.

The standalone agent carrier continues to target its only session. A brownfield
test adapter chooses a session by a host-owned label before handing the request
to the unchanged eight-operation API; session selection is carrier routing, not
a ninth Exact operation.

Logs include a stable per-session label supplied by the host and the generation,
then flow to both LLP 1012's journal and the host log sink. Secrets and store
values keep their existing redaction rules. Crash reports ship ordinary native
symbols; there is no new devtools UI.

### D10 — The proof is a hostile host, not another Exact demo

The implementation is not demonstrated by launching Caltrain in its own window.
It is demonstrated by an existing UIKit/AppKit application that:

1. creates two independent Exact sessions and shows both in a native hierarchy;
2. places one bounded surface inside a native screen and one content-height
   surface inside a native scroll container;
3. uses the host's authenticated request client and store adapter;
4. handles an Exact command by pushing a native screen, then returns with both
   sessions intact;
5. rotates/resizes, changes appearance and accessibility size, backgrounds,
   receives memory pressure, and remounts;
6. destroys one session while a request and image are outstanding and observes
   no late callback;
7. refuses a bad replacement plan while leaving the prior generation visible;
8. drives both sessions through the existing agent operations;
9. builds once with core only and once with each optional artifact, recording
   incremental link and memory deltas;
10. removes Exact without changing the native app's service or navigation
    architecture.

The first real external consumer should be a brownfield app outside this
repository, driven through `scripts/app.mjs`/`EXACT_APP_DIR` as the repository
already requires for Weird Castle. A tiny in-repo showcase would prove the
opposite of the claim.

## 3. What this deliberately does not add

- No new Contract tag, style row, event, state plane, or agent operation.
- No React tier, Rust-owned native application root, or platform-suffixed route.
- No shared heap between Swift, Rust, Hermes, or wasm.
- No requirement to ship Hermes, wasmtime, GPU, WebKit, or native modules.
- No dynamic delivery requirement. A baked, Rust-only surface is the smallest
  and first embedding shape.
- No Android implementation in v1. `rules/NOT-DOING.md` still binds. A proven
  Apple `ExactSession`/`ExactView` contract plus a real adopter requiring mobile
  parity is the return trigger; moving Android into scope requires the named
  trade at that time.
- No public compatibility promise before 1.0 and no compatibility shims. The
  minimal handle-based ABI is designed so the promise can later cover a small
  boundary rather than the implementation behind it.
- No new packaging manifest, registry, check, devtools UI, or integration
  harness merely because this RFC names desired evidence. Implementation must
  use or replace existing apparatus under `rules/RULES.md`.

This RFC requires no `NOT-DOING` trade by itself: it changes the ownership and
packaging of systems already in v1 and adds no platform or runtime feature.

## 4. Sequence if accepted

This is ordering, not an implementation spec; an implementer and date are
required before the work is transcribed into a child LLP.

1. **Instance the Rust/ABI boundary.** Replace the global bridge with an opaque
   handle, prove two runtimes in host tests, and keep the standalone C surface
   working through the new handle only.
2. **Extract the Apple component.** Move clock, frames, presenter callbacks,
   lifecycle, and reset/destroy ownership into `ExactSession` + `ExactView`;
   reduce both platform `main.swift` files to adapters.
3. **Prove containment.** Bounded surface first; content-height and native scroll
   ownership only after the platform/kernel constraint exchange is measured.
4. **Inject host services.** Request transport, store, command delegate, log sink,
   and asset resolver over canonical values and existing grants.
5. **Package the smallest Rust-only artifact.** Measure a real host link and make
   installation independent of a Rust toolchain for consumers of built output.
6. **Reattach optional artifacts one at a time.** GPU, WebKit/native modules,
   Hermes, then wasmtime; each retains its existing load gate and records bytes
   and memory.
7. **Prove the external hostile host.** Only then call the embedding API the
   product rather than an internal refactor.

## 5. Open questions and leanings

1. **Is one session allowed to survive with no mounted view?** Lean yes: it is
   how native navigation can pop/push without destroying app state, but it must
   stop frames and visibility-dependent work.
2. **Is content-height containment in the first implementation?** Lean yes for
   adoption, conditional on one measured constraint exchange with no relayout
   loop. If that fails, bounded surfaces ship first and the limitation is loud.
3. **Which host services are required for the first public package?** Lean:
   request transport, store, command delegate, and log sink. Asset resolution
   may use the existing bundle/network behavior until a consumer demonstrates a
   custom cache.
4. **Does `ExactSession` own a serial queue or require caller serialization?**
   Lean caller/main-thread serialization for Apple v1, matching LLP 1008; async
   service replies already have the wake/pump hop.
5. **Can multiple sessions share one Hermes runtime?** Lean no initially. Share
   linked code pages, not heaps or globals; isolation and destruction are worth
   more than a speculative memory saving. Measure before changing it.
6. **Should core and Apple UI be one distributable binary?** Lean separate Swift
   wrapper plus one core XCFramework, while optional executors/modules remain
   distinct. The final answer follows the measured link/install loop, not package
   aesthetics.
7. **What is the first real brownfield consumer?** Unnamed. Without one, this RFC
   may guide the singleton refactor but must not cause a general SDK apparatus to
   accrete. Naming the consumer, implementer, and date is the gate for a child
   spec.

## 6. Acceptance questions for Charlie

1. Is **embedded surface as primitive; standalone app as adapter** the governing
   direction?
2. Is **multiple isolated sessions in one process** required from the first
   embedding release?
3. Should **native-owned outer scrolling/content height** be required for the
   first consumer, or may the first surface always be a bounded viewport?
4. May the first implementation extract and prove the Rust-only Apple core before
   packaging Hermes, wasmtime, and GPU?
5. Which existing native application is the first consumer that makes the SDK
   shape concrete?
