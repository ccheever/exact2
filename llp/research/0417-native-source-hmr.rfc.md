# LLP 0417: Native Source HMR

**Type:** RFC
**Status:** Accepted
**Systems:** Dev Server, Vite, HMR, Module Graph, Module Runner, Ibex, Hermes, Apple Hosts, Capability Security, Tooling, Verification
**Author:** Charlie Cheever / Claude
**Date:** 2026-07-30
**Revised:** 2026-08-25 (r6 — the three ibex LLP 0055 §12 owner asks ruled by Charlie
Cheever (2026-08-25, decisions relayed via orchestration session exact-9e; the
conservative option on all three, the orchestrator's lean accepted; the ticket
`issues/closed/20260824-llp0417-owner-asks-from-h1.md` records the rulings). Ask 1
(server's class-driven response becomes advisory) — DECLINED for v1: §4.3 stands; the
server's full-reload-class `reload` answer remains, riding as a redundant coherent
reload beside the consumer-executed recovery of ibex 0055 §9.1 (sound-but-wasteful,
defense in depth; revisit with receipt signing under device enrollment, ibex 0055 §13
OQ4). Ask 2 (revision-race row) — TAKEN: §4.8's "revision race lost" moved from
`full-reload-current-authority` to `keep-last-good` — the only normative text change in
this revision; the H1 surface's single-flight + base-currency-at-begin refusal is pure
(before any dispose/evaluation effect), so the producer restages from live coordinates
with no reload. Ask 3 (narrow restart-row "edge widening" to authority-relevant
widening) — DECLINED: the broader restart classification stands (defense in depth; the
server pre-classifies everyday same-authority shape edits into the reload class before
staging, so behavior is unambiguous even though the taxonomy reads broadly).)
**Revised:** 2026-07-30 (r5 — round-4 super-refine revision; fail-closed pass over rev 4 (r1–r4 lineage in `llp/reviews/`). Five codex round-4 findings all adopted by REMOVING risky flexibility, not adding mechanism. (F1) generation.rs supplies the transaction/refusal *algebra* (kept) but keys records by `BTreeMap<String,SourceId>` with empty-binding-map tests, whereas the live runner needs `GraphEdgeKey{specifier,resolution_kind}` (graph.rs:123) + candidate/deferred tables — so H1 MUST extend its digest, immutable ceiling, AND adversarial tests to the typed runner graph, with an edge-ceiling acceptance fixture (resolution_kind-distinct same-spelling edges); the "graph representation unchanged" claim is deleted. (F2) `effectful-unknown` NO LONGER hot-applies — it refuses into coherent reload unless the author opts into unmanaged-effect semantics (`hot.acceptUnmanagedEffects()`, default off); a throwing dispose initiates coherent recovery, not silent success; candidate-effect lease named as the post-v1 H1 path to make effects revision-owned. (F3) v1 MANDATES runtime recreation for coherent reload; surviving-runtime generation-advance is a post-v1 optimization gated on ambient-effect (timer/next-tick/global/host-registration) retirement fixtures proving fresh-boot equivalence. (F4) namespace export-shape rule: a HotRevision is slot-eligible only if replacement export names/descriptors/interop-shape are identical; any change is the new `export-shape-changed` refusal → full reload (ESM namespaces are non-extensible, getters capture generation+record, hermes_module_runner.cc:3656). (F5) loopback is ENFORCED, not inferred: normative H2 gate REFUSES non-loopback peer addresses (socket captures them, vite-plugin.ts:5557) or disables HMR when LAN-bound; "loopback by construction" deleted. Impl fold-ins: H0 stop uses a stated noise band; Rust layer named as immutable per-generation link plan (graph.rs:139) distinct from retained C++ JSI objects; 0413 §11 blog-lane cross-ref at H2 budget; trivial cite drifts. Net line count held flat/negative by the fail-closed deletions.) Accepted 2026-07-30 by Charlie Cheever after a 5-round dual-family super-refine (fable READY r3–r5; codex NOT READY throughout, all findings adopted); accepted with the H1 entry-obligations checklist folding codex's residual round-4/5 findings as binding pre-implementation work. Growth +60% over rev 1 recorded per LLP 0151.
**Related:** LLP 0413 (§5.5, §9.3, §9.4, §10 Phase 4, §11), LLP 0416 (D1–D3, R5), LLP 0127, LLP 0128, LLP 0160, LLP 0297, LLP 0307, LLP 0322; Ibex LLP 0002 (dev-served seam), 0023 §2.3, 0024, 0026 §8, 0027, 0038, 0042; `issues/20260728-llp0413-phase4-hmr-generation-join.md`; ibex `issues/closed/20260728-prepared-graph-independent-commitment.md`; ENG-23118 (loopback auth precedent, superseded for v1 by loopback scoping — §5)

## Summary

Native Exact has **no module-level hot updates** — not an implementation
gap in an existing HMR system; the system does not exist. LLP 0413 Phase 4
M2 established this as current truth with receipts and recorded work item 3
("apply compatible edits through source ModuleRunner/HMR") as an
**architectural stop**, not a code slice
(`issues/20260728-llp0413-phase4-hmr-generation-join.md`).

This RFC is the program that ends the stop. Its execution model is
two-level: an **ExecutionGeneration** is a complete authenticated
graph/runtime lifetime — exactly Ibex's landed contract, across which no
live record ever crosses — and a **HotRevision** is a new, ibex-amended
transaction *within* one execution generation that swaps replacement
records for an accepted invalidation closure behind stable logical slots.
Full reload advances the generation; a compatible edit commits a hot
revision; a refused revision leaves the generation untouched and becomes
exactly one Phase 4 coherent full reload — never a half-applied graph.
Ibex's complete-but-unwired generation kernel (`generation.rs`) is the
admission gate for both levels. The RFC exists to make LLP 0413 §11's
"leaf-edit HMR ≤250 ms median to correct pixels" row measurable, then met.

## 1. Problem Statement (with the Phase 4 receipts)

Current truth, established from code and confirmed live during Phase 4 M2:

1. **The only native edit path is a full reload.** The dev server sends
   `{type:"reload"}` on `/__exact/ws`; the device re-fetches the whole
   startup envelope and re-evaluates the whole graph in the surviving
   Hermes runtime (`DevServerHotReloadClient.swift` `dispatchReload` →
   `loadBundleFromCurrentURL`). No per-module update message exists in
   either direction of the socket vocabulary.
2. **`import.meta.hot` is stripped from every native module.**
   `sanitizeNativeModuleSource`
   (`packages/exact-devtools/src/vite-plugin.ts:16896`) removes the
   `/@vite/client` imports and hot-context installation and rewrites every
   remaining `import.meta.hot` to the literal `undefined`; Contract's
   accept prologue (`packages/exact-contract/src/vite-plugin.ts:639-668`)
   therefore compiles to `if (undefined) { … }` on native. Contract HMR —
   the only source HMR on the main `js/` surface — is web-only. (The
   Tier-2 sentinel `examples/facet-gallery` separately runs Fast Refresh
   on web; nothing native consumes it.)
3. **Ibex's generation model is unreferenced scaffolding.**
   `vendor/ibex/src/module_loader/generation.rs` has zero call sites
   outside its own unit tests (§4.1).
4. **Full reload is the cost of every edit, even after Phase 4.** With
   M1–M3 landed, measured on-device edit → re-evaluated-generation: hello
   (prepared-HBC full reload) 796 ms median (run 1) to 2.1 s (run 2); blog
   (§5.4 legacy fallback, blocked from the prepared lane) 2.8–3.4 s. The
   §11 "leaf-edit HMR ≤250 ms" row was recorded **NOT MET and NOT
   MEASURABLE AS SPECIFIED** — the row's mechanism does not exist.

Speculation made the *server* cheap (post-speculation reload fetch 85–105 ms
warm) but the device still tears down and re-evaluates the entire
application graph per edit. No amount of full-reload optimization reaches
250 ms to correct pixels with state where it matters; that is why LLP 0413
§7.H's verdict adopted source HMR as "one half of the development model"
and §9.3 reserved the `module-runner-hmr` lane. This RFC fills that lane.

## 2. Goals

1. **Module-level updates ≤250 ms median (≤500 ms p95) to correct pixels**
   for leaf edits on the reference lanes, on-device — the LLP 0413 §11 row,
   made measurable and then met (budget binds at H2's hello-lane gate; row
   discharge is H3).
2. **HMR is source/record based** (LLP 0413 §5.5; LLP 0127 Decisions 7,
   "HBC is not the module-level HMR format"). An update ships transformed
   source module records as verified artifacts into a staged hot revision;
   HBC carriers are never mutated, patched, or partially replaced in a
   running generation.
3. **Full reload remains the §5.5 fallback through one coherent prepared
   generation** — the landed Phase 4 *server* machinery is the refusal
   target for the full-reload class, unchanged; the device/runtime
   generation transition it feeds (advance-and-retire or runtime recreate)
   is new work (§4.1, §4.8).
4. **Both authoring tiers are served.** Contract's reset-based HMR rides
   the same transport it uses on web; the React tier gets an honest
   disposition (§4.6).
5. **Threading contract compliance** (LLP 0297): updates apply on the
   runtime thread through the async executor; no new sync waits; every new
   callback lands in `docs/callback-affinity.md` before merge.
6. **Production untouched.** Production has exactly one execution
   generation (Ibex LLP 0026 §8; `GenerationMode::Production` refuses
   updates); nothing changes armed admission, packaging, or release bytes.

## 3. Non-Goals

1. **React Fast Refresh in v1.** The main `js/` surface has no integrated
   refresh path and no native refresh runtime exists anywhere in the tree
   (web Fast Refresh exists only in the Tier-2 sentinel). Native v1 gives
   the React tier a fast, coherent full reload, not component-state
   preservation (§4.6).
2. **Worklet-runtime module updates.** The shipped worklet model already
   gives what HMR needs: installs are mount-scoped from source and stamped
   with the app-runtime generation; the generation bump + reset ordering
   are shipped (LLP 0297 §4.8 — the unshipped slot/tombstone semantics are
   0099 M1 contract language this design does not depend on). HMR reuses
   the shipped part (§4.7); a dev worklet-eval surface stays future work.
3. **Hot updates to dependency principals.** Integrity-pinned package bytes
   cannot hot-reload (Ibex LLP 0026 §8 authority asymmetry); a package edit
   is a refusal in the *restart* class (§4.8), by design.
4. **HBC-encoded HMR payloads.** Rejected by LLP 0127 and 0413 §5.5; also
   by the worklet precedent (source artifacts only).
5. **A second invalidation authority.** Vite owns the module graph, file
   watching, and invalidation events (LLP 0127 Decisions 6). This RFC maps
   Vite's boundary computation onto Ibex hot revisions; it builds no
   parallel watcher or dependency graph.

## 4. Design

### 4.1 Two-level execution model: ExecutionGeneration and HotRevision

**Level 1 — ExecutionGeneration (landed, adopted as-is).** An execution
generation is the lifetime of one complete authenticated graph in one
runtime, and Ibex's landed contract is strict: generations may reuse
**immutable artifacts only** — never live records, cells, namespaces,
promises, errors, or CommonJS exports (`generation.rs:1-6`; ibex 0026 §8).
This is not pending spec work — the 0023 §2.3 / 0024 amendments 0026 §8
called for have **landed** and plainly prohibit cross-generation record
sharing ("Live cells… never cross generations"). A **full reload must become a fresh execution
generation** — Phase 4's *server* publication already provides that, but the
*device/runtime* path does not yet: `dispatchReload` re-evaluates in the
**surviving** JSEngine (`ExactRuntimeShell.swift` `beginBundleEval` reuses the
same engine and producer ingress; the engine is recreated only for the
embedded-release→dev takeover, ~:2597), and ibex 0027 pins the graph generation
until owner-thread runtime teardown, so today the counter never advances, the
old graph stays pinned, and none of the state owners are released — the JS
loader cache, the retained Rust record layers (`SourceModuleGraphV1` plus
`NativePublishedGraphIndex`, `hermes.rs:1535-1542`; `module_runner.rs:2414-2445`,
released whole by a new generation rather than mutated — note
`SynchronousGraphPlan` is *dropped after linking* (`hermes.rs:2620` `drop(plan)`)
and is therefore **not** a retained owner), and the retained C++
prepared-carrier JSI objects. Making the fallback a genuine generation transition is a named H1/H2
exit (§4.8, §6): **v1 fails closed by recreating the runtime; a
surviving-runtime generation-advance is a post-v1 optimization** — not a done
property the design may assume.

**Level 2 — HotRevision (new contract work).** A hot revision is a distinct
transaction **within** one execution generation. It stages **replacement
verified artifacts and shadow records for exactly the accepted invalidation
closure**; nothing outside the closure is touched, so unchanged modules
keep their live records trivially — no generation boundary is crossed.
Every export the closure exposes to outside importers is reached through a
**stable logical slot** (the boundary's export identity); commit switches
the slot targets to the new incarnation records in one publication step,
and outside importers keep their namespace identities while seeing the new
bindings through the slots. A **refused hot revision drops its shadow
records whole** and leaves the generation untouched — LLP 0027's "no
refused drive may leave partial new records," applied at the revision
level. The incarnation key gains a revision dimension —
`(SourceId, ExecutionGeneration, HotRevision)` or ibex's equivalent — and
the publication-token fencing algebra applies per revision: a stale
completion (TLA, dynamic import, error, CJS cache, artifact cache) cannot
publish into a newer revision.

**Slot eligibility is export-shape-bounded (normative).** An ESM namespace
is a non-extensible object whose per-export getters capture a specific
generation and record id (`hermes_module_runner.cc:3656`), so a replacement
cannot both keep the outside importer's namespace identity and expose a
changed export set. A HotRevision is slot-eligible **only if** the
replacement's export names, property descriptors, and interop shape are
identical to the prior incarnation's; any add, remove, rename, or
interop-shape change is the `export-shape-changed` refusal (full-reload
class, §4.8), not a slot retarget. OQ4 stays open only for the
export-shape-compatible case.

**Landed vs new, stated plainly:** the ExecutionGeneration contract and
its cross-generation prohibition are landed ibex text. The HotRevision
mechanism — stable logical slots, shadow-record staging, revision-scoped
fencing, mixed record provenance within a live generation — **does not
exist and is the new contract work**, and it touches landed text: 0023 §2.3
keys a live incarnation as `(runtime/session, SourceId, execution graph
generation)` and 0024 §7.9 gives one `SourceId` one incarnation per
generation, while HotRevision creates successive incarnations *within* one
generation. H1 therefore amends **ibex 0023 §2.3, 0024 §7.9, 0026 §8, and
0027 as one coherent set**, and draws the slot-vs-namespace line: a stable
logical slot is a generation-owned forwarding *binding target* (importers
bind the slot; it targets exactly one incarnation's records at a time), not
a live namespace shared between incarnations, which 0023 prohibits. In 0027
the amendment defines the sanctioned post-boot path by which replacement
source-artifact records join a live generation whose other records carry
prepared-carrier provenance; boot admission is not relaxed (prepared-HBC
boots stay uniform, Phase 4's per-carrier mixed-encoding disposition
unchanged for full reloads).

**`generation.rs` supplies the transaction/refusal *algebra* — adopted; its
graph *data model* is extended in H1, not reused unchanged.** Inspection
(790 lines, feature-gated, zero call sites outside its own tests) shows a
complete, tested policy kernel: monotonic overflow-refusing
`ExecutionGeneration`; publication tokens/receipts over six publication
points; staged whole-graph transactions with race and converse-invalidation
refusal; `ImmutableGenerationAdmissionV1` ceilings; `GenerationMode::Production`
refusing `begin_update`; four unit tests (adversarial composite, monotonic,
production-refusal, owner-slot-swap). That algebra is the valuable,
hard-to-re-derive part, and every HotRevision commit runs it — authority
re-validation, full post-revision validation against the immutable ceiling,
the converse check. But its authenticated graph is **not the live runner's
graph**: it
keys records by `BTreeMap<String, SourceId>` and its digest and ceiling
bind only `(source, specifier text, target)` (`generation.rs:83-100,
148-175`), while every adversarial test builds **empty binding maps**
(`generation.rs:627-634`), so the ceiling is never exercised against real
edges. The live runner requires `GraphEdgeKey { specifier, resolution_kind
}` (`graph.rs:123`) — identical spellings resolve differently under static
ESM, dynamic import, and CJS — plus candidate/deferred-dynamic/deferred-CJS
tables (`runner_pipeline.rs:252-270`). So `generation.rs` **cannot guard
armed admission as-is: H1 MUST extend its digest, immutable ceiling, and
its adversarial tests to the full typed runner graph before it gates
anything** — which keeps the tested algebra, where a rewrite would re-derive
it without its tests.

What the kernel does **not** provide — the H1 bridge scope: (1) call
sites — nothing constructs an `AuthenticatedGenerationGraphV1` or holds a
`ModuleExecutionGenerationsV1`; (2) the right counter — today
`graph_generation` derives from the armed CapSec snapshot's dynamic grant
generation (`runner_pipeline.rs:1959`), an *authority* counter constant for
the runtime's life that 0026 §8 says HMR must never touch, so the execution
counter must be decoupled onto `ExecutionGeneration` with the HotRevision
counter defined against it (OQ1); (3) mechanism — the record surgery is
new work (a loader-cache invalidation entry point: `module-loader.js:729`
has none, every `delete cache[...]` is a failure path; slot relink;
eviction of the C++ `prepared_carrier_tables` memo, which has none
today); (4) a staging seam — `captureDevServedModuleTable` is a frozen
one-shot boot hook; hot revisions need a revision-scoped seam (shape is
ibex's call — OQ2); (5) **the typed graph representation** (above) — plus a
**named acceptance fixture: an edge-ceiling test carrying two same-spelling
edges of distinct `resolution_kind` over a non-empty binding map**.

### 4.2 Admission shape: verified artifacts under a session signing key

How fetched update records become trusted is answered here, not left open:
**updated records enter the runtime only as verified artifacts, never as
self-authenticating source.** Each update payload is signed under an
**ephemeral dev-session signing keypair**: the private key is held only by
the producing session (LLP 0042's custody rule, preserved — a symmetric
session HMAC was rejected because 0042 keeps that secret producer-only, so
a device could never verify it); the **public verifier** rides the startup
session envelope alongside the §5 pull credential and rotates with the
session. In v1 that envelope is loopback-scoped (§5), so an unauthenticated
serve is acceptable; the LAN/device enrollment follow-up (§5) instead binds
the verifier to the enrolled boot identity rather than merely serving it. The
signature binds `runId`, authority generation, execution generation, hot
revision, the **normalized target descriptor, entry/profile, boot/consumer
identity, and the committed base-graph digest**, and the payload digest over
the canonical update body — update identity, base generation + revision, the
invalidation closure, declared effect classes (§4.8), and per-record semantic
digests + source integrity. Binding `runId`/authority-gen/exec-gen/revision/
closure/effects/record-digests alone is **insufficient**: Exact's server is
multi-target (startup identity carries target/platform/entry; sockets are
partitioned by target environment; artifacts are selected by the normalized
target descriptor + entry — `vite-plugin.ts:1198-1233,1355-1359,6240-6283`),
so a valid server-signed payload for one platform/profile could otherwise
verify in another consumer whose session and revision coordinates coincide,
and the immutable ceiling need not catch it when root-source identities and
permitted edges match. **HTTP payload selection and WS routing MUST be checked
against the same target/entry/boot/base-graph fields** — 0042's commitment
precedent binds target, entry, and deployment-graph digest
(`vendor/ibex/llp/0042-prepared-graph-independent-commitment.rfc.md:94-120`).
The device verifies before admitting any record through the
`VerifiedModuleArtifactV1`-class path; per-record digests alone are
self-consistency, not authentication (ibex 0042's adversarial contract:
self-consistency is not authentication). The signature defeats cross-session,
stale-generation, and cache-substitution payloads — it is not a
network-attacker or compromised-dev-server defense (0042's own honesty). This
requires a 0042 amendment (H1, jointly with ibex); it interlocks with ibex
0042 Q1 — commitment transport to a *relaunching* consumer — but does not
wait on that answer (OQ5). H1 pins the remaining wire details: canonical
signature encoding, payload and module-count limits, replay-table bounds, and
whether stage timing crosses server/device clock domains. Posture is dev-only,
structurally: the dev commitment schema is a different kind refused by
`ArmedSnapshot::load` before field comparison, and
`GenerationMode::Production` refuses `begin_update` before any of it is
reachable.

### 4.3 Transport: extend `/__exact/ws`, fetch the payload

The update channel extends the existing dev socket vocabulary rather than
inventing a second socket (`/__exact/ws` is the native control plane and
already carries `reload`):

- **Server → device `hmr-update` (notification only):** a small envelope —
  `{ updateId, baseGeneration, baseRevision, boundaryKind: "contract" |
  "js" | …, moduleCount }`. No executable source rides the notification.
- **Device → server payload fetch:** `GET /__exact/hmr-payload?updateId=…`,
  gated by the §5 dev-session HMR credential, returns the staged record
  set: per invalidated module, the dev-served-shaped record (`id`, `kind`,
  `path`, `source` — transformed, *unsanitized-for-hot*), the typed
  boundary metadata (accepted boundary, invalidation closure, declared
  per-boundary effect class — §4.8 — and per-record semantic digests), the
  target revision, and the §4.2 session signature over the whole body.
  Pull keeps executable payloads off the control socket, fetched from the
  server the device already trusts for its bundle URL.
- **Device → server receipts, self-measuring from day one:**
  `hmr-applied { updateId, generation, revision, stages,
  observedEffectClass }` — `stages` is the per-stage breakdown (notice
  latency, fetch, verify, eval, commit, remount, dispatch→frame), so the H0
  harness, the H2 integrated gate, and the H3 Acto receipts are one instrument
  and the §11 row is continuously self-measuring; `observedEffectClass`
  (dispose seen / not seen even on success) gives the server a free drift
  signal for §4.8's manifest derivation before mismatches become refusals.
  And `hmr-refused { updateId, reasonClass, reason,
  committedGeneration, committedRevision }` — a §4.8 disposition class
  plus the device's committed coordinates, so the server never guesses
  the device's revision; the response is class-driven — only the
  full-reload class is answered with one `reload` (speculation-warmed,
  exactly like a watcher invalidation).

Boundary computation stays server-side and Vite-owned: the devtools plugin
consumes Vite's hot-update events for the native environment, computes the
accepted-boundary/invalidation closure against the native module graph, and
decides `hmr-update` vs `reload` (no-boundary edits, config/env/optimizer
invalidations, and dependency-principal edits go straight to the
appropriate class, per LLP 0127's conservative rules). The device never
re-derives graph structure.

### 4.4 `import.meta.hot` on native

`sanitizeNativeModuleSource` stops rewriting `import.meta.hot` to
`undefined` and instead binds it to a native hot context installed by the
module runner, keyed by `(SourceId, ExecutionGeneration, HotRevision)`. The
v1 surface is the subset the tree actually uses:

- `hot.accept(cb)` / `hot.accept(deps, cb)` — registers the accepting
  boundary; the server's boundary computation must agree (mismatch =
  refusal, full-reload class).
- `hot.dispose(cb)` — §4.8 owns ordering and failure semantics.
- `hot.invalidate()` — a full-reload-class refusal.
- `hot.data` — carried across *committed* hot revisions of one `SourceId`
  within the generation; data written during an apply whose incarnation
  never published drops with the refused revision (the next update's
  dispose runs against the last committed incarnation).
- `hot.on` / `hot.send` — required by Contract's first-paint self-heal
  (`exact-contract:sync` / `sync-request`); bridged over `/__exact/ws` as
  namespaced events. If this slips, v1 ships without the sync channel
  (OQ6) — but accept/dispose/invalidate/data are non-negotiable.

Callbacks registered under a stale generation or revision are defined
no-ops (the LLP 0297 §4.8 worklet-install fencing shape).

### 4.5 Contract tier: reset-based HMR rides the same transport, single-root in v1

Contract's runtime *decision* pipeline (`hot-update.ts`: eval-failure
keep-last-good → slot patch → same-version noop → remount in place → adopt
records) is platform-neutral and rides the same emitted prologue
(`globalThis.__exactContractHotUpdate(emitId, newModule)`) that it uses on
web, operating on platform-neutral host-ops mounts. But the landed pipeline
is **not** a prepare/commit transaction and therefore **cannot participate
unchanged in the §4.8 atomic-publication invariant**: today it *applies
immediately* — slot patching mutates live host targets and advances the
IR-of-record (`hot-patch.ts:258-322`); remount installs the candidate tree
right after disposing the old one (`mount-core.ts:690-717`); and candidate
module evaluation updates framework-global source and Design Mode registries
*before* the accept callback (`define.ts:125-143`, `design-meta.ts:603-650`).
It is an applying pipeline, not a candidate builder, so a later HotRevision
refusal or host-activation failure could leave Contract host state, IR, or
framework registries ahead of the committed module graph. **H1 must therefore
either (a) introduce a two-phase Contract adapter — prepare diff/remount/
registry overlays without live mutation, then activate them with the
HotRevision — or (b) disable live slot patching in v1 and use only a versioned
shadow-root activation path** (H1 entry obligation, §6). Once that adapter
lands and the native runner can re-evaluate a `.contract` module with a live
`import.meta.hot`, remount re-runs mounts through the protocol host exactly as
a Design Mode override commit does today.

**v1 scope: single root.** The honest reading of the landed pipeline
(`hot-update.ts:179-206`) is that multi-root application is best-effort
today: targets remount *sequentially*; on mixed success the records follow
the newest graph while failed roots remain stale "stragglers" healed by
the next successful update — contradicting any multi-root atomicity
claim. Rather than claim an atomic batch v1 does not build, **v1 refuses
hot revisions whose remount targets span more than one root** (full-reload
class); the transactional shape — shadow-mount all affected roots,
validate, publish slot switches + root swaps as one runtime-thread batch —
is Phase H3 work (OQ9). Contract is the v1 centerpiece deliberately (default
authoring model, LLP 0160): `hot-update.ts` already downgrades what it cannot
patch into remount-or-invalidate, mapping cleanly onto §4.8.

### 4.6 React tier: honest deferral

Scoped correctly: the main `js/` surface has no integrated refresh path
(no `@vitejs/plugin-react` in any `js/` Vite config; web React/TSX edits
there take stock full-page reload), while the Tier-2 sentinel
`examples/facet-gallery` runs Fast Refresh on web today. What is missing
is main-app-surface adoption and — everywhere — a **native refresh
runtime**; both are a follow-up RFC extending §4.4's hot context (refresh
boundaries are `accept`-shaped, so this transport and revision model
suffice), with the sentinel as the web-first precedent. Disposition:
**React-tier edits refuse to HMR and take the warm coherent full reload** —
a statement about sequencing, not importance. The cheapening comes from
Phase 4's *server* speculation (warm reload-fetch 85–105 ms), not from the
device: v1's fail-closed runtime-recreate (§4.8) makes each device-side full
reload strictly *more* expensive than today's same-engine reload, so the
warm-full-reload-recreate cost is added to the H2/H3 per-stage measurement
(the same receipt instrument) and the 0413 §11 warm-reload row is measured,
not silently regressed.

### 4.7 Threading and quiesce points (LLP 0297)

- **Arrival:** `/__exact/ws` messages *dispatch to main first* (the
  `receiveWebSocketMessage` hop), then `handleWebSocketMessage` →
  `controlMessage(from:)` *parses on main* — the existing control-plane
  ordering (ENG-23250), reflected as-built; `hmr-update` follows that shape
  (unchanged rows for `open-url`/`host-automation`).
- **Apply:** the payload fetch completes off-main; the apply task is
  enqueued onto the **runtime executor** (Context 3, app runtime on the
  dedicated runtime thread — LLP 0297 §4.1/§4.2 A1). The executor is
  serial, so "quiesce point" is structural: the apply task runs between
  app-JS entries, never reentrantly inside one; verification, staging,
  evaluation, dispose, commit, and remount dispatch all execute inside
  that one task (plus revision-stamped TLA continuations).
- **No sync waits.** The LLP 0297 §4.4 live-resize carve-out is not
  available to this path; every hop is async end-to-end.
- **Affinity rows before merge** (LLP 0297 W0(b)): new rows for hmr-update
  arrival (delegate thread → main; parse and handle on main), payload-fetch
  completion (background → runtime executor), apply task (runtime thread),
  and receipt send. Unclassified callbacks block merge.
- **Worklets:** installs stay mount-scoped and generation-stamped through
  the shipped path (LLP 0297 §4.3/§4.8); a remounted boundary reinstalls
  its worklets via `installWorklet`. An update touching a worklet-authoring
  module *not* covered by a remounting boundary refuses (full-reload
  class); HMR never targets the worklet runtime directly.
- **EXFF:** an HMR apply does not touch the precomputed-first-frame store;
  the full-reload class flows through LLP 0307's discard-and-replace
  hydration. Divergence vs captured frames is OQ10.

### 4.8 State, atomicity, and refusal dispositions

**What survives an update:** everything outside the invalidation closure —
a hot revision crosses no generation boundary. Inside it, the accepted
boundary and its invalidated importer chain re-evaluate as new
incarnations; module-scope state there is lost except what crosses via
`hot.data`. Contract component state follows `hot-update.ts`: preserved on
slot-patch, reset on remount. Outside importers are not re-run — they
reach the boundary's exports through the §4.1 stable slots, which is why
the boundary must be an accepting one. Sharp edge from LLP 0027: CJS
detected named exports are snapshots that do not update after
`module.exports` mutation, so cross-boundary CJS named imports refuse
(full-reload class). No module-cache surgery API for app code; no
in-place namespace mutation for arbitrary ESM.

**The atomicity invariant, narrowed to what the mechanism guarantees
(normative):** the runtime guarantees **atomic publication** — records,
bindings, slot targets, cache ownership, and (v1 single-root) the root
swap switch in one runtime-thread publication step; no importer can
observe a mixed graph, and a refused revision leaves no partial records.
It does **not** roll back JavaScript effects — Ibex treats module
initialization as effectful execution under module authority, and nothing
here pretends otherwise. Effects are bounded by an ordering contract, not
transactions:

1. **Preflight before any effect.** Compile/link the staged records (and
   extract Contract IR) before running any app-visible code; failures
   here refuse with zero app-visible effects — no app code has run.
2. **Shadow-evaluate first (pure boundaries only).** A boundary registering
   no `hot.dispose` runs its new incarnation inside the staged revision
   before the live graph changes, so a throwing evaluation refuses
   keep-last-good with the live graph untouched. This inverts stock Vite
   ordering safely only for a genuinely pure boundary: **unpublished records
   ≠ rolled-back effects**, so any timer/subscription/singleton/host effect
   run before a throw still leaks. The rule-4 fail-closed class assignment
   is what keeps effectful modules off this path.
3. **Dispose-registered boundaries keep stock Vite
   dispose-before-evaluate,** with the degraded failure semantics
   documented: if the new incarnation then fails to evaluate, the module
   stays torn down until the next successful update or reload, whose
   dispose runs against the last *committed* incarnation — guarded so no
   committed incarnation is disposed twice in one apply. The repo's entry
   dispose patterns (`examples/hello-world/main.tsx:19-22`,
   `js/src/home/main.tsx:137-141`) clean up old-incarnation state and
   must run before the new incarnation claims it.
4. **Declared effect classes (v1 manifest, fail-closed).** The Vite-emitted
   update manifest declares one class per boundary — `contract-staged-pure`
   | `dispose-registered` | `effectful-unknown` — inside the §4.2 signed
   body; the runtime selects ordering from the declared class. **Only
   `contract-staged-pure` takes rule 2** (shadow-evaluate-first);
   `dispose-registered` takes rule 3 (dispose-before-evaluate).
   **`effectful-unknown` is NOT hot-applied in v1 — it refuses into a
   coherent full reload.** Native timers, next-ticks, subscriptions,
   singleton mutations, and host calls are retained runtime objects with no
   `ExecutionGeneration`/`HotRevision` field (`hermes_runtime_internal.h:65-76,
   825-827`), so neither shadow-first ordering nor a dispose bounds them,
   and neither a successful `hmr-applied` nor a `keep-last-good` receipt
   would imply correct ongoing behavior — the earlier "ordering bounds
   unknown effects" reading is withdrawn. **There is no v1 override:
   `effectful-unknown` always refuses into a coherent full reload, full
   stop** — v1 offers no author opt-in to hot-apply unknown effects, because
   no defined ordering, receipt, or throw semantics for that class exists
   yet (see Open Questions). The runtime still refuses (full-reload class) on
   observed class mismatch (e.g. a `contract-staged-pure` boundary
   registering a dispose). The post-v1 path
   to make `effectful-unknown` hot-eligible is a **candidate-effect lease**
   (named H1 mechanism): timer/subscription/host-callback/resource
   registrations are revision-owned — created inactive during shadow
   evaluation, activated only on commit, auto-cancelled on refusal, prior
   lease retired atomically on commit. Derivation/observation mechanics are
   H1/H2 work.
5. **A throwing dispose initiates coherent recovery, not silent success.**
   The effect already ran and cannot be un-run, and the staged records are
   unaffected — but a dispose throw means the last-committed incarnation did
   not clean up, so the apply does not proceed as an ordinary success: it is
   reported and the revision refuses into a coherent full reload
   (full-reload class) rather than committing over a half-torn-down
   incarnation.

**Remount joins publication (v1 single-root, primary rule):** the root
swap publishes in the same atomic transaction as records and slots, so a
remount failure refuses the revision *pre-commit* — last-good records,
slots, and pixels all stand, matching the landed pipeline's
adopt-records-only-on-successful-remount behavior (`hot-update.ts:24-31`).
**Contingency, only if H1 proves the joint runtime-thread transaction
infeasible: a versioned host-root lease** — prepare the candidate host-ops
tree, obtain a host activation token asynchronously, then commit slots and
activate the token under one revision fence, releasing queued app entries
only after activation settles (keeping activation atomic without a
synchronous cross-thread wait — LLP 0297 §4.4 preserved). **No persistent
stale-root steady state is authorized:** any retained `hmr-applied-degraded`
path must *immediately initiate a local coherent full reload* after
reporting its committed generation + revision, never leave a stale root
running old event surfaces against new slot targets. Either way no
refused-but-committed steady state exists.

**Required fixture (here and in H2's gate):** an update whose evaluation
registers an effect (timer/subscription) then throws; the corpus asserts
the refusal, the untouched live graph, and the documented leak.

**Typed refusal dispositions (normative).** Every refusal reason belongs to
exactly one class; the `hmr-refused` reason taxonomy partitions across
them, and the server's response is class-driven:

- **`keep-last-good` (no reload).** The new incarnation fails to evaluate
  (the developer is mid-keystroke — Contract's `module-eval-failed`
  behavior); a patch-pipeline throw; a remount failure inside the joint
  publication transaction (the revision never commits); a revision race
  lost (moved here from the full-reload class by the 2026-08-25 owner
  ruling on ibex LLP 0055 §12 ask 2: the H1 surface refuses a stale base
  at `begin` — single-flight + base-currency-at-begin — before any
  dispose/evaluation effect, so the refusal is pure and the producer
  restages from the device's live committed coordinates; no reload is
  needed). The next update resolves it.
- **`full-reload-current-authority`.** No accepting boundary; server/device
  boundary disagreement; `hot.invalidate()`; module
  changed outside the invalidation set; cross-boundary CJS named-import
  edge; `export-shape-changed` (§4.1); uncovered worklet-authoring module;
  multi-root targets (v1); declared effect-class mismatch (rule 4);
  `effectful-unknown` (rule 4, no v1 override); a throwing dispose (rule 5);
  anything unclassified. Answered with one coherent warm reload through the
  Phase 4 *server* path under the *unchanged* authority snapshot. **v1 fails
  closed: the coherent reload tears down and recreates the runtime** —
  because today's `dispatchReload` reuses the same `JSEngine`
  (`ExactRuntimeShell.swift:2341-2417` disposes the app via
  `__exactDisposeApp`, clearing only a few globals; its own comment states
  the runtime is reused) and ibex unpinning removes ESM/CJS records and
  dynamic requests but **not** timers, next-ticks, arbitrary globals, or
  host registrations (`hermes_module_runner.cc:2771-2825`), so a
  surviving-runtime reload cannot yet prove the §4.8 fresh-boot equivalence
  obligation. Advancing `ExecutionGeneration` in the *surviving* runtime
  (unpin + retire the state owners, fence stale completions) is a **post-v1
  optimization** gated on generation-owned cancellation/fencing for *every*
  ambient-effect surface with the H1 retirement fixtures. Policy is
  unchanged; only the execution generation transitions.
- **`regenerate-policy-and-restart-runtime`.** Package-integrity pin drift,
  principal or edge widening, and authority-generation drift — the classes
  where `generation.rs` itself refuses with "regenerate policy and restart
  the runtime" / "restart required" and ibex 0026 §8 requires re-arming.
  The current ws `reload` is **not** this operation — it neither
  regenerates the policy snapshot nor tears down the runtime — so this
  class must trigger the host's runtime-teardown + re-arm/boot path (on
  macOS dev, the runtime-shell recreation path). The server must never
  answer it with a plain reload: that loops the refusal forever or tempts
  an implementation to bypass the immutable ceiling.

**Equivalence obligation (LLP 0413 §14 item 13, Phase 4 exit 3), scoped:**
**reload-after-HMR equivalence** — after any HMR session, one full reload
of the same content must land on module state and pixels equivalent to a
fresh boot. The corpus runs edit-scripts both ways on state-free surfaces,
or diffs modulo *declared* preserved state (`hot.data`, Contract
slot-patch state) — raw both-ways tree equality would be unsound against
the design's own state-preservation semantics.

### 4.9 Relationship to the hybrid hot-edit encoding

Complementary, by construction. The flagged hot-edit hybrid remedy
(Phase 4 receipts; LLP 0413 §9.4) makes the *full-reload class* cheap: the
just-edited carrier ships as `javascript-factory-table` inside the next
coherent publication beside untouched HBC carriers, sidestepping the
root-carrier hermesc floor (~2.2 s on blog). This RFC makes the
*compatible-edit* path nearly free by not reloading at all. Both hang off
the same server-side epoch/speculation machinery; if the hybrid decision
is approved first, the full-reload class inherits it automatically. One
reading to preempt: a hot revision is a sanctioned update transaction
under LLP 0413 §5.5, not a §5.4 encoding *fallback* — "no source fallback
after application evaluation begins" governs failure recovery inside one
load, not this update path.

## 5. Security

- **Dev-only surface, structurally.** The hot context, update messages,
  and payload endpoint exist only in the dev-served/diagnostic posture.
  Nothing touches armed admission: HMR consumes generation authority, it
  never mints capability (Ibex LLP 0042); production admission refuses the
  dev commitment schema structurally, and `GenerationMode::Production`
  refuses `begin_update` before any of that is reachable.
- **The authority snapshot is immutable across generations and revisions.**
  No HMR event changes principals, import axes, policy edges, or
  integrity-pinned bytes; `ImmutableGenerationAdmissionV1` re-validates at
  every commit, and drift refuses into the restart class (§4.8). Publishing
  generation N+1 revokes the generation-N dev commitment (0042), so a
  refused/raced HMR revision cannot be admitted later as a warm start.
- **v1 transport scope: an enforced loopback peer gate.** The native
  startup-envelope routes serve their contents after target/platform
  selection with **no authorization gate**, and the ws upgrade checks only
  the request path — so a bearer delivered *inside* that envelope cannot gate
  access to itself (any client reaching the server obtains it), and the ws
  carries correctness-bearing `hmr-refused` receipts (they drive reload and
  per-device revision tracking, §4.3), not telemetry. Rather than assert an
  authorization the envelope does not provide — or *infer* safety from the
  configured host, the Host header, or "loopback by construction" — **v1
  enforces the boundary at the socket: HMR HTTP requests (`hmr-payload`) and
  every HMR ws control/receipt message are REFUSED from a non-loopback peer
  address, OR HMR is disabled whenever the server is LAN-bound**
  (`isExactDevServerLanExposedHost` true — which today only `logger.warn`s,
  `vite-plugin.ts:4695`). The socket already captures normalized peer
  addresses at connection (`vite-plugin.ts:5557`), so the check is feasible;
  it is a normative H2 acceptance criterion (§6 H2), not an inference. The
  earlier LAN / ENG-23118 equivalence claim is **withdrawn**. The `GET
  /__exact/hmr-payload` credential still exists (dev-session HMR credential:
  per-session, envelope-delivered, memory-held, rotated, constant-time
  compared, log-redacted; not the agent token — agent boot defaults off via
  `resolveNativeAgentConfig`, so 0413's agent-off lanes would otherwise have
  none), but behind the enforced loopback gate it is session hygiene, not the
  network boundary.
- **LAN/device follow-up: authenticated boot enrollment.** Physical-device
  and LAN HMR need a real trust anchor the public envelope is not. The
  follow-up is an **enrollment step** binding, for one boot instance,
  {boot/target identity, `runId`, the §4.2 ephemeral public verifier, the
  pull credential, ws socket-receipt authority} together: the device
  generates an ephemeral key and enrolls it against an operator/pairing-code
  anchor established, not assumed (there is no pre-existing authenticated dev
  channel to piggyback on). ibex 0042 requires delivery over an
  **authenticated** development channel and leaves new-consumer transport
  open (ibex 0042:238, 0042:361), so 0417 must not treat that as satisfied by
  a public envelope. The enrollment trust-anchor choice is OQ5.
- **Transport credential ≠ payload authenticity.** The credential gates
  the pull; the §4.2 session signature is what makes fetched records
  trustworthy, verified before any record is admitted.

## 6. Phased Plan

**Phase H0 — bounded spike (evidence before architecture).** Throwaway,
flag-gated, no Ibex changes: hand-carry one trivial Contract leaf edit
end-to-end — ws notice → payload fetch → re-evaluate the single module in
the live runtime (bypassing generation bookkeeping, spike-only) → invoke
the existing `__exactContractHotUpdate` handle → pixels. Measure the
on-device edit→correct-pixels distribution over ≥20 edits on hello, macOS,
emitting the §4.3 per-stage breakdown — the same instrument H2/H3 reuse.
*Exit:* a median **with a stated noise band** (a bootstrap confidence bound
over the ≥20-edit corpus), not a bare point estimate. *Decision gate
(one-sided):* a floor whose confidence bound clears 250 ms stops the program
and re-opens the §11 row's budget with the raw corpus.
**A pass is screening evidence only — H0 alone never authorizes H1
spend**, because the spike omits staging, verification, commitment checks,
relinking, cache retirement, and refusal handling. H1 proceeds only on
H0-pass *and* this RFC's acceptance; no §11 budget claim is made before
H2's integrated gate.

**Phase H1 — Ibex hot-revision surface** (with the Ibex repo; tracked in
vendor/ibex issues). Decouple `ExecutionGeneration` from the authority
counter; write the **HotRevision/slot spec section with Ibex, amending
ibex 0023 §2.3, 0024 §7.9, 0026 §8, and 0027 as one coherent set** (§4.1,
incl. the slot-vs-namespace distinction and the §4.1 export-shape slot
eligibility rule); **extend `generation.rs`'s authenticated graph, digest,
immutable ceiling, and its adversarial tests from `(source, specifier,
target)` onto `GraphEdgeKey`-plus-candidate/deferred tables** (§4.1 bridge
item 5, with the edge-ceiling fixture); build the mechanism (§4.1
items 3–4), the §4.2 keypair signature verification plus the 0042
verifier-delivery amendment (jointly with the ibex 0042 ticket), and the
§4.8 effect-class derivation plus the post-v1 **candidate-effect lease**;
define the §4.8 **full-reload generation transition** — **v1 is runtime
recreate**; the post-v1 surviving-runtime advance-and-retire carries its
stale-TLA / dynamic-import / CJS / prepared-carrier-table / publication-token
**and timer / next-tick / global / host-registration** fixtures proving
fresh-boot equivalence; adjudicate the §4.8 joint root-swap publication
(primary) vs the **versioned host-root lease** contingency. *Exit:*
Ibex fixtures prove the algebra live (stale-publication refusal, race
refusal, package-edit refusal into the restart class, edge-ceiling refusal
on a resolution_kind-distinct same-spelling edge, slot-switch atomicity,
export-shape-changed refusal, no-partial-records on refusal, full-reload
runtime-recreate coherence across a plain reload), and a host API exists to
stage+commit a hot revision from replacement verified artifacts.

**H1 entry obligations (super-refine residuals).** Before H1 implementation,
the H1 spec MUST discharge, each with its named fixture where noted:

1. **Per-slot incarnation predicate** — token validity is `token.incarnation
   == currentIncarnation[sourceId]` (revision is the transaction coordinate;
   unchanged slots keep their prior epoch), with the
   unchanged-module-TLA-survives / replaced-module-stale-completion-refuses
   fixture (codex r5 F1).
2. **Two-phase Contract adapter, or shadow-root-only** — decide between the
   prepare/activate adapter and disabling live slot patching for versioned
   shadow-root activation; no applying-in-place pipeline under §4.8 (codex r5
   F2, §4.5).
3. **Target/base-graph-bound signature** — the signed body binds normalized
   target descriptor, entry/profile, boot/consumer identity, and committed
   base-graph digest; HTTP selection + WS routing check the same fields
   (codex r5 F3, §4.2).
4. **`generation.rs` typed-graph extension** — digest/ceiling/tests extended
   onto `GraphEdgeKey`-plus-candidate/deferred tables, with the edge-ceiling
   fixture (two same-spelling edges of distinct `resolution_kind` over a
   non-empty binding map) (codex r4 F1, §4.1).
5. **Candidate-effect lease** — the only sanctioned mechanism for any future
   effectful hot path; no v1 effectful-unknown hot-apply exists without it
   (codex r4 F2, §4.8 rule 4).
6. **Getter-indirection** — namespace slots resolve through the slot at call
   time, not by a getter-captured `recordId`, so retargeting a slot is a
   concrete runner change (fable r5, §4.1).
7. **Counter unification (OQ1) + staging-seam shape (OQ2)** — ceded to Ibex;
   settled in the H1 spec.

**Phase H2 — Exact transport, hot context, and the integrated gate.** The
§4.3 messages and credential-gated payload endpoint; the §5 **enforced
loopback peer gate** (refuse non-loopback peers on `hmr-payload` and every
ws control/receipt, or disable HMR when LAN-bound) plus the session
credential + verifier; the §4.4 hot context replacing the sanitizer's
`undefined` rewrite; the §4.8 disposition classes wired end-to-end
(including the restart-class join to the host restart path, and the
device-side full-reload generation transition as **v1 runtime recreate** —
the H1 mechanism wired into `dispatchReload`);
the **LLP 0127 startup-mode amendment** (prepared-HBC boot with an active
hot revision is not `source-hmr`; a new mode value — e.g.
`"hbc+hot-revision"` — or an amended matrix) lands here, before the gate;
affinity rows; receipts. **H2 requires the FIFO-capable dedicated runtime
executor** (LLP 0297 §4.1/§4.2) so the serial apply-task ordering §4.7 relies
on holds; **HMR is explicitly disabled under `EXACT_RUNTIME_THREAD=0`**,
whose compatibility main executor does not provide FIFO ordering
(`ExactRuntimeExecutor.swift:32-47`). *Exit (the integrated performance gate):*
Contract leaf edit on **the hello lane**, full pipeline — **≤250 ms median
AND ≤500 ms p95** to correct pixels on-device, measured by the same
per-stage receipts as H0 over **≥30 measured edits** with the raw corpus
retained and the confidence statement LLP 0413 §10's measurement protocol
requires (§11 holds the budgets); this meets
the §11 row's *budget on the hello lane* (0413 §11 defines the row against
the *blog* reference lane, which the deferred-dynamic ticket currently
blocks from the on-device prepared lane, so row discharge on blog is H3 —
not goalpost-moving); every refusal produces its class-correct response;
**the enforced loopback peer gate refuses a simulated non-loopback
`hmr-payload` request and ws receipt**; the §4.8 equivalence corpus —
including the effect-then-throw fixture — is green; the runner gains
registered checks (no new root `check:*` without a registry entry).

**Phase H3 — breadth and diagnostics.** Multi-root transactional
application (shadow-mount all affected roots, publish slot switches + root
swaps as one runtime-thread batch — lifting the v1 single-root scope, OQ9);
the `exact-contract:sync` bridge; design-overrides state-preserving path
parity; the blog lane (gated on the deferred-dynamic admission ticket) —
**§11 row discharge or a written variance**; startup diagnostics on the
H2-amended 0127 mode matrix; Acto-visible HMR receipts (already the same
instrument). *Exit:* diagnostics answer "what generation/revision am I on
and why" from the launch banner.

**Phase H4 — deferred follow-ups (out of this RFC's exits):** React Fast
Refresh (main-surface web adoption, then the native refresh runtime); the §5
**authenticated boot enrollment** that lifts v1's loopback scope to
LAN/device; iOS device HMR (iOS is already runtime-thread-default-on,
ENG-23520 — the remaining gate is the LAN/device enrollment anchor, not a
threading flip) and Windows enablement (gated on its runtime-thread flip,
ENG-23494); dev worklet-eval surface. macOS loopback dev is the v1 platform.

**Relationship to LLP 0413:** this RFC discharges the §11 leaf-edit row
and §10 Phase 4 work item 3, closing the Phase 4 exits recorded
PARTIAL/MOOT in the join ticket, without reopening Phase 4's landed
machinery — it is the consumer that finally exercises `HmrOrigin::Exact`.

## 7. Open Questions

1. **Counter unification.** How do `ExecutionGeneration`, the HotRevision
   counter, the dev server's graph epoch, and the Swift-side engine counter
   relate — and is HotRevision a new `ModuleIncarnationKey` dimension or a
   re-scoping of the existing counter? Ibex owns the answer (H1 spec).
2. **Staging seam shape.** Revision-scoped successor to the dev-served
   capture table vs a new module-runner API — ibex's call, given the
   capture hook's quarantine semantics were designed for exactly-once.
3. **Payload shape.** Dev-served-style full records only, or additionally a
   Vite-runner-style fetchModule form for large boundaries?
   (Decision-blocking for H2's endpoint contract.)
4. **Slot semantics.** Slots per boundary export, per module, or per
   closure edge; what may cross via `hot.data` (plain values, or handles
   with disposers)? v1 assumes boundary re-evaluation through export slots
   (Vite parity); a parity-corpus counterexample forces finer relink. Open
   **only for the export-shape-compatible case** — any export
   add/remove/rename/interop-shape change is `export-shape-changed` → full
   reload (§4.1, §4.8), not a slot-granularity question.
5. **Enrollment trust anchor & relaunch transport.** v1 ships loopback-scoped,
   so no anchor is needed to ship (§5); the open choice is the LAN/device
   follow-up's enrollment anchor (operator/pairing-code vs another established
   authenticated channel) plus how a *relaunching* consumer re-obtains its
   bound session material — the ibex 0042 Q1 interlock, answered jointly with
   ibex, not locally.
6. **`hot.on`/`hot.send` bridging.** Ship in H2 with namespaced ws events,
   or defer and accept that Contract's first-paint self-heal stays web-only
   temporarily?
7. **Budget reality.** Does the floor (fetch RTT + verify + Hermes source
   eval + remount + dispatch→frame) fit 250 ms on the reference Mac — and
   on iOS hardware later? H0 screens; H2's integrated gate answers.
8. **Session fuse parameters.** Proposed default: N consecutive
   full-reload-class refusals auto-degrade the session to reload-every-edit
   with a visible banner — insurance against boundary-computation drift
   thrashing. Open: N, which reasons count, the reset condition.
9. **Multi-root batch shape (H3).** Fully transactional (all-or-nothing
   across roots and windows, ordered per-window apply receipts) vs
   restricted (bounded root sets)? Deferred by v1's single-root scope.
10. **EXFF interaction.** Should an HMR apply invalidate the route's
    precomputed first frame (LLP 0307), or is capture-on-next-quiesce
    sufficient to avoid stale relaunch frames after an edit session?
11. **Post-v1: could `effectful-unknown` ever become hot-eligible?** Only via
    the candidate-effect lease (§4.8 rule 4) — **not in v1, and it requires
    the candidate-effect lease plus a defined effect class with
    ordering/receipts/throw semantics before it could exist.** There is no
    normative v1 opt-in (the earlier `hot.acceptUnmanagedEffects()` override is
    removed).

## 8. Decision Requested

1. Adopt the §4.1 two-level execution model: ExecutionGeneration (Ibex's
   landed contract — no live record ever crosses it) plus HotRevision (the
   new intra-generation transaction, spec'd with Ibex in H1), with the
   §4.8 invariant as normative: atomic publication of records, bindings,
   slots, cache ownership, and the v1 single-root root swap — JS effects
   bounded by the dispose-registration-gated ordering contract and the
   effect-class manifest, never claimed rolled back.
2. Revive Ibex's `generation.rs` for its transaction/refusal **algebra**
   (not a rewrite) — **H1 extends its typed graph, digest, immutable
   ceiling, and adversarial tests onto `GraphEdgeKey`-plus-candidate/deferred
   tables** (the data model is not reused unchanged), with the rest of the
   H1 bridge scope and the coherent amendment set (ibex 0023 §2.3, 0024
   §7.9, 0026 §8, 0027) done with Ibex.
3. Extend `/__exact/ws` + a payload endpoint; **v1 enforces a loopback peer
   gate on the whole HMR channel — pull and ws control receipts — refusing
   non-loopback peer addresses or disabling HMR when LAN-bound** (§5),
   because the envelope-served bearer is not a LAN authorization gate and
   safety must be enforced, not inferred; authenticated boot enrollment is
   the LAN/device follow-up; payload authenticity comes from the §4.2
   ephemeral session signing keypair under an amended 0042, not from
   self-carried digests.
4. Contract reset-based HMR, **single-root in v1**, is the v1 target;
   multi-root targets and the React tier take the warm coherent full
   reload, with the corrected rationales in §4.5/§4.6.
5. Adopt the §4.8 typed refusal dispositions, including the restart class
   as a real host restart path that a plain ws reload must never
   impersonate, and the **fail-closed v1 defaults**: `effectful-unknown`,
   `export-shape-changed`, and a
   throwing dispose all refuse into a coherent full reload (v1 offers no
   author opt-in to hot-apply unknown effects), and **every full
   reload recreates the runtime** rather than surviving-runtime-advancing —
   the surviving-runtime advance is a post-v1 optimization gated on
   ambient-effect retirement fixtures.
6. Run the H0 spike first as a binding one-sided stop rule; an H0 pass is
   screening only — §11 budget claims bind exclusively at H2's integrated
   median + p95 gate on the hello lane, with row discharge at H3.


## Window relation (recorded 2026-08-26, LLP 0506 D2)

LLP 0504 §3 row 100 is non-window (D2).
LLP 0504 §3 row 104 is non-window (D2).
Both are cross-repo Ibex folds — the 0055 H2-entry obligations and the
0056 leg-4 Exact fold — carried on Ibex's own release clock. Neither
changes an EXWF frame, a protocol opcode, or a native ABI that LLP
0506 D1 names.
