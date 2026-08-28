# LLP 0292 — Host Input Capabilities and Declared Reactive Host State

**Type:** Decision
**Status:** Active
**Systems:** contract, web-host, native-host, renderer
**Author:** Claude (Fable 5), directed by Charlie Cheever
**Date:** 2026-07-02
**Revised:** 2026-07-11 (ENG-24199 — declared reactive host bindings;
ENG-24188 review hardening — native safe-area bridge, deep appGlue validation,
and shared appGlue node/depth admission bounds)
**Revised:** 2026-07-17 (macOS first-responder ownership + key-event target
resolution, PR #25). The heuristic-delivery caveats below assumed nothing owns
the responder lifecycle; that hole is now closed on macOS: the AppKit
presenter root heals an orphaned (window-held) first responder after snapshot
applies with a settle delay so real focus handoffs win, and presenter node
views hand first responder back to their root on teardown. Target resolution
is shared by ExactEngine and ExactView (`exactResolveKeyEventTargetNode`,
unit-tested): a targeted node with zero key handlers no longer swallows the
event and falls back per phase to the deepest mounted consumer (the same
semantics untargeted events always had), while a target binding only the
other key phase keeps the strict drop so a keyDown/keyUp pair is never split
between the target and an unrelated node.
**Related:** LLP 0293 (slides diary), LLP 0330 (Contract semantics), LLP 0328 (Contract Native tiers), Snapback LLP 0052 (update economy), LLP 0234 (authoring QoL), LLP 0093 (capability plane), LLP 0157 (capability protocol), LLP 0251 (friction ledger)

## Context

The LLP 0293 slides build produced an eight-row pain ledger. Five rows were
platform gaps an app had to work around in userland: no keyboard events
(row 1), no disposal channel for task-spawned resources (row 2), no grid
container (row 5), no reactive viewport (row 7), and a check-script sidecar
resolution gap (row 4). This decision lands the framework-owned versions and
deletes the slides app's workarounds as proof.

## Decisions

1. **`keydown` event attr** (grammar + web host). Element-scoped, DOM
   semantics: lowers to `onKeyDown`; the action receives the key string
   (`action onKey(key)`), matching how `change` delivers the input value.
   `submit`, `keydown`, and `dismiss` all lower to `onKeyDown`, so onKeyDown
   consumers now **compose in authoring order instead of clobbering**. The
   dom-mirror grants keydown-consuming non-focusable elements programmatic
   focusability (`tabindex="-1"`, applied after attribute reconciliation) —
   a keydown consumer on an unfocusable div is otherwise dead code.

   **Per-node keydown on native (ENG-22597 decision).** Web is the
   reference semantics: delivery is focus-scoped to the element. Native
   has the mechanics but not the model — `keydown=` attrs do bind kernel
   `KeyDown` handlers through the shared host-ops adapter, and each
   platform's presenter forwards hardware keydowns app-wide to
   `ExactEngine.handleKeyEvent`, which targets the **deepest mounted node
   with a KeyDown handler** (`topmostNodeWithEventHandler`) because
   nothing defines which non-text node is "focused." Delivery further
   requires the presenter's own key-capture view to be first responder,
   which other responders routinely preempt (the dev-overlay interaction
   controller holds first-responder status almost constantly on iOS —
   verified live during ENG-22596; the app-level module needed an
   app-delegate responder-chain fallback for exactly this reason). Native
   text inputs forward only Enter (the submit path), and the iOS per-node
   path has no autorepeat. Net: an element-scoped keydown consumer that
   works on web is unreliable-to-dead on native.

   The decision is to **not** paper over this with more heuristics.
   Honest per-node native keydown needs, together: (a) a native focus
   model for non-text nodes — who is focused, how focus moves; (b)
   nodeId-tagged key events from a first-responder-independent source
   (the monitor/funnel pattern that now backs the app-level capability);
   and (c) parity guards (autorepeat, typing-wins). Until that lands, the
   Contract runtime **warns once per session** when a `keydown=` attr is
   lowered on a documentless host (`warnNativeKeydownAttrOnce`,
   `runtime/view.ts`), naming the heuristic, the Enter-only input
   behavior, and the app-level `keyboard` capability as the reliable
   native path. `submit`/`dismiss` lower to onKeyDown too but have
   working native paths, so only the explicit `keydown` attr warns.

2. **`keyboard` capability** (`use keyboard from "@exact/contract/runtime"`).
   App-level key bindings: the capability value is *callable*
   (`keyboard("ArrowRight", next)` from a `task ... mount` body) so task
   statements stay in the bare-call grammar. One document listener per root,
   installed lazily; action calls wrapped in `batch(allowStateWrites(...))`
   exactly like the runtime's own `every()`; disposal registered on the root
   owner, so reset-based HMR and unmount stay leak-free. Modifier chords and
   editable-element targets are skipped. Two backings share the binder
   surface (ENG-22579): web installs a `document` keydown listener
   (`document` is the gate — native defines a window shim); native syncs
   the union of bound keys to the host keyboard module
   (`com.exact.host.keyboard`) over the module-action channel, and the
   host consumes exactly those plain keydowns, emitting them back as
   `keydown` module events with web `KeyboardEvent.key` names.
   Unregistered keys, modifier chords, and keys headed for an editable
   first responder pass through the responder chain untouched. Hosts
   without the module (prerender, older native builds) leave the binder a
   silent no-op.

   Per-platform key sources behind the same module contract:

   - **macOS** (`HostKeyboardModule.swift`, ENG-22579): an app-global
     `NSEvent` local keydown monitor; returning `nil` consumes the event.
     Autorepeat comes free — the system delivers repeated `NSEvent`s at
     the user's key-repeat rate.
   - **iOS** (`HostKeyboardModuleIOS.swift`, ENG-22596): iOS has no
     app-global key monitor, so hardware `UIPress`es are *offered* to the
     module from two places — the presenter key-capture views
     (`ExactUIKitPresenterRootView`, `ExactKeyboardCaptureView`) before
     per-node key dispatch, and `ExactIOSAppDelegate.pressesBegan` at the
     end of the responder chain (needed because another responder — the
     dev-overlay interaction controller, a focused text input — routinely
     holds first-responder status and the capture views never see the
     press). Claims are deduped by `UIPress` identity; the identity set
     is dropped on app-resign along with the repeat timers, since a
     stale heap-address identity could swallow a future press allocated
     at the same address. UIKit delivers `pressesBegan` once per
     physical press, so the module synthesizes the autorepeat stream
     itself (0.4s initial delay, 0.1s interval; canceled on
     keyup/press-cancel/app-resign, when `setKeys` drops the held key,
     and when a text input becomes first responder mid-hold — each
     synthesized repeat re-runs the typing-wins guard the way every
     fresh macOS repeat `NSEvent` does; mid-hold modifier changes are
     not observable without a new press, so the chord guard cannot
     re-run). `UIKey.charactersIgnoringModifiers` drops Shift (NSEvent
     keeps it), so the module's normalizer re-uppercases shifted
     letters, and shifted non-letters **fail closed**: the shifted form
     of "1" or "/" can't be derived without a layout map, and forwarding
     the base character would false-fire a registered base binding on a
     shifted press that web (`event.key === "!"`) and macOS would not
     fire. The iOS shifted-symbol gap is therefore under-delivery only
     ("?" bindings don't fire from Shift+/), never a wrong-key claim.

3. **`viewport` capability** (same import surface). Signal-backed
   `width`/`height` that re-evaluate bindings on window resize — the
   reactivity the `window.innerHeight`-at-mount hack never had. Without a
   DOM the signals start at 0. Apple presentation updates now feed viewport
   atomically with declared safe-area host state; standalone native viewport
   wiring for apps that do not select that host extension is the tracked
   follow-on.

4. **`onCleanup` task builtin.** `onCleanup(installThing(...))` in a task
   body registers the returned disposal with the same owner `every()`/
   `after()` use. This is the disposal channel for `use`-imported helpers.

5. **`grid` container tag.** `grid cols=3 gap=12` renders real CSS grid on
   the web hosts; a bare number expands to
   `repeat(n, minmax(0, 1fr))` (strings pass through for full control).
   Native currently degrades to block flow (children stack) — documented,
   not hidden; Taffy grid plumbing is the tracked follow-on.

6. **Declared reactive host state (`host`, ENG-24199).** The component-only
   forms are `host scheme = colorScheme()`, `host insets =
   safeAreaInsets()`, `host location = geolocation({ mode:
   "whenAvailable" })`, and `host backend = appGlue("backend", { default:
   "connecting" })`. The compiler lowers a closed discriminated IR:
   arbitrary initializers, dynamic app-glue names, non-static defaults,
   collisions, writes, and behavior-scoped acquisition are errors.

   Values are stable per-root signal facades. Scheme is `light | dark`;
   insets has scalar `top/right/bottom/left`; geolocation is tagged
   `pending | available | denied | unavailable` with coordinates/source;
   appGlue's JSON-like default defines its publication shape. Missing sync
   ambient inputs use explicit light/zero fallbacks; a missing async sensor
   is `unavailable`, never silently stale. Scalar fields and bounded
   appGlue structural equality retain canonical identity, so N fresh
   value-equal deliveries cause zero propagation after the first.
   A host/test `globalThis.exact.colorScheme` override remains authoritative
   across later OS `matchMedia` events; the shared listener re-reads the bridge
   instead of overwriting the override with the physical preference.

   Per-root instances fan out from one lazy/refcounted physical listener per
   capability (`matchMedia`, safe-area adapter, `watchPosition`); the first
   root starts it and the last removes it. The browser/native source is
   injectable through `ContractHostStateAdapter`. Hydration records a
   `hostState` bag, replays it through adoption, buffers live arrivals, and
   releases to the latest live value before deferred mount tasks.

   `publishHostTransaction` applies correlated viewport/scheme/insets/
   geolocation/app-glue values in one batch. Distinct host events are not
   frame-coalesced. `publishHostState` uses the same canonicalization path
   and rejects undeclared names or wrong shapes. Publications arriving
   inside an action/task batch stage FIFO and drain while that outer batch
   remains open: **they join the action's exit flush** (resolving Snapback
   LLP 0052's drain-point question).

   `appGlue` is limited to low-frequency host-adjacent state and read-only
   provider-lifecycle projections. Persisted preferences remain app state
   with persistence; async results remain query/resource state.

   On native Apple hosts, safe-area changes enter through the existing
   `__exactSafeAreaInsetsChanged` window-management bridge. That bridge fans
   out to the conditional host extension and carries the same presentation
   snapshot's viewport, so rotation geometry and insets publish through one
   `publishHostTransaction`. It adds no second native observer. `appGlue`
   publications are recursively validated as acyclic JSON-like data before
   shape matching and cloning; an empty-array default relaxes element *shape*,
   never the JSON domain. That domain is explicitly bounded to 2,048 visited
   nodes and depth 64. Provider publications, hydration captures, and static
   defaults pass an iterative descriptor-based gate before recursive
   shape/clone work; accessors, cycles, or either over-budget condition are
   precise runtime errors. The node limit is also the structural-equality
   budget, so every accepted fresh value-equal delivery is fully comparable
   and retains canonical identity rather than propagating at the cutoff.

   The substantial manager is a conditional runtime extension:
   `compileContractModule` imports `runtime/host-bindings` only for files
   containing host declarations. No-host modules retain their static graph.
   Until Rust implements this ABI, compiler eligibility, raw product
   admission, and direct Rust IR parsing all reject non-empty `hosts` rows;
   silent TS-only semantics are forbidden by LLP 0328.

## Proof (deletion budget)

The slides engine deleted: `installSlideKeys` (~55 lines incl. the
batch/allowStateWrites wrapping and the idempotent-install hack),
`stageStyle`/`pageStyle` (viewport probes), and `chunkPad` + ghost-spacer
grid emulation (~40 lines + per-slide row data). `js/src/slides/lib.ts` is
~110 lines lighter and the engine reads as grammar.

## Corrections recorded elsewhere

LLP 0293's R7 ("viewport units silently dropped") was a misdiagnosis — an
erratum in that diary corrects it: `'100vh'` passes through to CSS; the
observed bug was `flex=1`'s `flex-basis: 0%` overriding the explicit height.
No "dropped style value" warning is needed on this path; the viewport
capability stands on reactivity grounds alone.

## Coverage

`packages/exact-contract/src/__tests__/host-input-capabilities.test.ts` —
seven end-to-end tests through the production web host: key delivery +
focusability, submit/keydown composition, document-level binding +
modifier-skip + reset disposal, reactive resize, onCleanup-on-reset, grid
tracks (numeric and string templates).

`packages/exact-contract/src/__tests__/host-keyboard-native-channel.test.ts`
(ENG-22579) — the native backing: bound keys sync to the host module sorted
on action id 0, emitted `keydown` module events dispatch to actions (with
the `"Space"`/`" "` alias), unbound keys are ignored, reset clears the host
key set, and a host without the module stays a silent no-op.

`packages/exact-contract/src/__tests__/native-keydown-attr-warning.test.ts`
(ENG-22597) — the per-node gap warning: a `keydown=` attr lowered on a
documentless host warns exactly once (pointing at this LLP and the keyboard
capability); `submit=` alone never triggers it.

iOS hardware-keyboard backing (ENG-22596) was verified live on an iPhone 17
Pro simulator through the agent relay, with HID key events injected via
`idb ui key`: arrows navigate `/slides` decks both directions, Space
advances, `g` opens contents, unbound keys emit nothing, and a 1.2s held
arrow emitted an initial keydown plus synthesized repeats that stopped on
release.

`packages/exact-contract/src/__tests__/contract-host-bindings.test.ts`
(ENG-24199) covers grammar/format/diff/graph, diagnostics, conditional
codegen and HMR no-host↔host reset, interpreted+compiled delivery,
value-equality suppression, two roots/one physical subscription/cleanup,
all geolocation arms, typed/deeply validated appGlue, FIFO action staging,
adapter-level correlated viewport/insets transactions, hydration, and
compiler-side Native rejection. `js/src/window-management-lifecycle.test.ts`
drives the actual `__exactSafeAreaInsetsChanged` callback and pins its
correlated subscription payload. Rust unit tests pin raw-admission and
direct-IR rejection independently.
