# Panel: LLP 1031 Brownfield embedding (sol)

- **Family:** OpenAI: `codex exec -m gpt-5.6-sol -c model_reasoning_effort=ultra -s read-only --skip-git-repo-check -C <capsule> -o <out> -` (OpenAI Codex CLI 0.153.0), prompt on stdin, cwd = the read-only capsule; no repository access. Requested `gpt-5.6-sol` at `ultra`.
- **Method:** a panel at Charlie's request, 2026-09-03 — "have a panel discussion about LLP 1031 and try to make it better. once you are happy with it, go ahead and implement it." Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind** (no panelist saw another), **round 2 mutually visible** (each read all three round-1 answers — the two external families and the orchestrating session's own independent review — and wrote only where they differ, correct, or extend; six named forks were forced to a final position). The document's r1 author was a different session (Charlie Cheever / Codex); the orchestrator (Claude Fable 5.1, this session) reviewed it as a third family before reading either external answer, folded the panel into r2, and is the implementer. Capsule: a read-only copy of LLP 1031 r1 and the context LLPs 1000, 1005, 1008, 1009, 1010, 1012, 1016, 1018, 1020, 1023, 1023.001, 1024, 1026, 1027, 1029, 1030, 1030.000, `rules/RULES.md`, `rules/NOT-DOING.md`, `CLAUDE.md`, `QUEUE.md`, and the Apple host's code (`include/exact.h`, `src/*.rs`, `swift/*.swift`, both `main.swift`, `Presenter.swift`, `DevMenu.swift`, `AgentMac/AgentIOS.swift`, `Gpu.swift`, both `Package.swift`, `build.mjs`, `dev.mjs`, `host/web/build.mjs`, `scripts/app.mjs`, `apps/caltrain/apple`); no network; no repository access. Narration lines before the document marker are preserved verbatim.
- **Gates (round 1):** sha256 —
```
a5696f59bfc23e9460d5deba2a6dbe6309048f40d64beb878707d46557c7ae23  capsule/llp/1031-brownfield-embedding.rfc.md
5a4a3171eb83609d1ad1ffe983172afa664e3d1349db10d002787e06c5ca2d96  capsule/llp/1030-delivery-unified.rfc.md
8d74f8a6ea176cbc78236aebdba41481abc56e76d27511eb39f1497ed2bfb996  capsule/llp/1030.000-dev-server-as-deployer.rfc.md
de832379ed67d7d9aa73a39315d7d259bbd7976688df25e8e9c92e7095fa3257  brief-round1.md
de832379ed67d7d9aa73a39315d7d259bbd7976688df25e8e9c92e7095fa3257  -
```
- **Gates (round 2):** the same capsule plus `round1/{grok,sol,claude}.md`; the round-2 brief and the three round-1 answers —
```
83e5b4e02c841143c9324145f599a7e1d298d1fca197f2a18d596bd44ed69440  /private/tmp/claude-501/-Users-ccheever-projects-exact2/fcaf045f-6725-4650-bcb1-8d268854944e/scratchpad/panel-1031/capsule/round1/claude.md
75429bb7c249f60b8f8f2905fe6f0d2b6645d8bb5a4f07657a38949e2db6fd31  /private/tmp/claude-501/-Users-ccheever-projects-exact2/fcaf045f-6725-4650-bcb1-8d268854944e/scratchpad/panel-1031/capsule/round1/grok.md
98bd1a56facd1548550a720f6b565945941ce9a7201310999be72eeec773eb8e  /private/tmp/claude-501/-Users-ccheever-projects-exact2/fcaf045f-6725-4650-bcb1-8d268854944e/scratchpad/panel-1031/capsule/round1/sol.md
24dd1e9c83aaf5e2fc99244975f77e4bc012ecbd39a7548305ecbb2ff02c447e  /private/tmp/claude-501/-Users-ccheever-projects-exact2/fcaf045f-6725-4650-bcb1-8d268854944e/scratchpad/panel-1031/brief-round2.md
```
- **Runs (UTC):**
```
sol r1 start 2026-09-03T12:04:04Z
grok r1 start 2026-09-03T12:05:05Z
grok r1 end 2026-09-03T12:14:56Z rc=0
sol r1 end 2026-09-03T12:23:07Z rc=0
grok r2 start 2026-09-03T12:24:52Z
sol r2 start 2026-09-03T12:24:52Z
sol r2 end 2026-09-03T12:30:17Z rc=0
grok r2 end 2026-09-03T12:30:23Z rc=0
```
- **Peer:** grok `grok-4.6` at `xhigh`, and Claude Fable 5.1 (the orchestrating session, reviewing independently before reading either), same capsule, same rounds.
- **Disposition:** folded into LLP 1031 r2 the same day — §8 records which catches held and where each landed, and its "Round 2" table the final positions; the author re-verified every code claim against the tree before folding (the `thread_local!` bridge and fixed exports in `abi.rs`; `executor.rs` running every request on `ibex2::host` with the grant check inside ibex2's `Bindings`; `Text.install` wiping process-wide caches; `PlanURL.current` and `Exact.wake` as statics; commands delivered inside `Presenter.apply`). Implementation began the same day under 1031 §4.

---

## Round 1 — position (verbatim)

--- PANEL ANSWER ---

1. **Overall assessment**

The direction is right: the reusable primitive should be an embedded surface, with the standalone applications reduced to adapters. I would implement the ownership refactor, but I would not publish or build the SDK contract exactly as written. The RFC presently conflates a runtime session with the app-level composition, delivery stream, update store, and distributable binary; it also leaves the security ABI, reentrancy, process-shared resources, and multi-surface cardinality undecided. Those gaps are blocking because the current code forces answers, and several advertised properties would otherwise be false.

2. **Strengths**

- D1’s ownership direction is correct. Moving the runner, clock, queues, journal, presenter state, and lifecycle beneath `ExactSession`/`ExactView`, while leaving windows, scenes, navigation, menus, and `NSApplication` to adapters, matches the actual extraction boundary visible in the two `main.swift` files (`llp/1031-brownfield-embedding.rfc.md:91-117`; `host/apple/macos/main.swift:20-49`; `host/apple/ios/main.swift:48-55`).

- D8 is appropriately honest that in-process embedding cannot provide crash isolation (`llp/1031-brownfield-embedding.rfc.md:307-324`). That limitation should remain explicit.

- D5 correctly keeps commands as typed intentions and reuses LLP 1024 for native content instead of inventing a plugin system (`llp/1031-brownfield-embedding.rfc.md:229-246`).

- D9 is right that host labels and session routing do not create a ninth agent operation (`llp/1031-brownfield-embedding.rfc.md:326-343`; `rules/NOT-DOING.md:117-129`).

- D7 is right about something that could easily have gone wrong: GPU infrastructure may deliberately share a device while every surface and generation remains session-owned (`llp/1031-brownfield-embedding.rfc.md:300-305`). “No semantic session state is global” is the right rule; “nothing is shared” is not.

- The existing runner seams hold up better than expected. Tickets already permit forgotten late outcomes, store semantics are already snapshot-in/writes-out, and candidate plan replacement is transactional (`llp/1016-async-data-settlement.rfc.md:184-204`; `llp/1018-durable-client-state.rfc.md:82-135`; `host/apple/src/abi.rs:182-228`). No new runner mode or agent operation is required.

- The bounded-first sequence is correct (`llp/1031-brownfield-embedding.rfc.md:399-406`). The problem is that D3 and D10 currently advertise content-height more strongly than that sequence supports.

- An external hostile consumer is the right adoption proof (`llp/1031-brownfield-embedding.rfc.md:345-370`). It should complement, not replace, a small regression fixture.

3. **Concerns**

- **Blocking — Q1: D6’s app-agnostic ExactCore is incompatible with the current composition boundary.**

  **Why.** `Bridge<D>` owns a generic `Host<D>` (`host/apple/src/abi.rs:52-59`). The `host!` macro substitutes both the concrete `DataSource` and baked plan, creates one thread-local bridge, and emits the fixed `exact_*` C symbols (`host/apple/src/abi.rs:329-452`). Caltrain supplies both pieces directly (`apps/caltrain/apple/src/lib.rs:6-9`), and its static library includes the app data crate, runner, kernel, and plan (`apps/caltrain/apple/Cargo.toml:1-19`). That is exactly 1029 D2’s rule that the composition names its executors (`llp/1029-mixed-logic-and-engine-choice.rfc.md:155-178`), but it is not a universal core capable of constructing arbitrary compositions. Two app-specific archives would also collide on the same `#[no_mangle]` names.

  Today the honest artifact matrix is:

  | Composition | Brownfield consumer links | Reusable without rebuilding per app |
  |---|---|---|
  | Rust-only | Swift UI plus an app-specific native runtime/data/plan artifact | Swift UI; not the native composition |
  | TypeScript-only | Core, Swift UI, Hermes, plus plan/HBC/assets | Potentially Core/Hermes, but today the macro still makes the archive app-specific |
  | Mixed | Core/UI/Hermes plus an app-specific Rust logic/router artifact | Core/UI/Hermes only |
  | Swappable | Mixed artifacts plus reusable wasmtime | Core/UI/engines; native composition remains app-specific |

  “No Rust toolchain in the consuming Xcode project” is achievable by distributing an app-specific prebuilt XCFramework. “The same ExactCore binary instantiates every Rust composition” is not achievable without a new erased executor boundary.

  **Resolve.** D6 must choose either:

  1. an app-specific `AppExactRuntime.xcframework` plus reusable Swift UI and optional engine artifacts for v1; or
  2. a concrete, size-versioned executor/factory ABI through which universal `ExactCore` constructs Rust, Hermes, and wasm executors.

  A descriptor alone is not a factory for `data::App`. I favor the first option for this landing. A compiled prototype showing `Host<ErasedDataSource>` constructing all 1029 compositions without duplicate exports would change my mind.

- **Blocking — Q2: The RFC has no coherent cardinality for several distinct surfaces.**

  **Why.** The current envelope contains one `plan` (`llp/1023-one-url-serving.rfc.md:131-160`; `llp/1023.001-serving-stage-1.spec.md:11-37`), the update store selects one bundle plan (`llp/1026-dynamic-delivery.rfc.md:498-523`), tooling resolves one `app.contract` (`scripts/app.mjs:19-27`), and the boot gate compares that plan with one app identity (`llp/1023.001-serving-stage-1.spec.md:138-155`). Delivery streams are `(app, channel, compatibility id)` (`llp/1030-delivery-unified.rfc.md:206-217`).

  The proposed shapes have materially different consequences:

  - N plans in one envelope changes the wire format, atomic update unit, store, and boot selector.
  - One plan with named entry components preserves the stream and compatibility identity, but requires a plan entry table and `Runner::boot(entry)`. That contradicts the claim that plan and runner semantics remain unchanged (`llp/1031-brownfield-embedding.rfc.md:85-87`).
  - N sessions of one plan works now, but produces N instances of the same surface—not settings, onboarding, and dashboard.
  - Separate Exact apps need separate app IDs, envelopes, streams, and stores, and cannot currently be co-linked because of the fixed exports.

  **Resolve.** Declare v1 to be one Exact application definition—one composition, plan, app ID, compatibility ID, envelope, stream, and update context—with N sessions instantiating that same plan. Introduce an app-scoped `ExactApplication`/`ExactBundle` owner, and create sessions from it. Distinct entry surfaces should be deferred unless Charlie explicitly chooses a named-entry plan amendment. Existing named-entry support in omitted code would change this conclusion; none appears in the capsule.

- **Blocking — Q3: Host request/store services lack a secure, typed ABI.**

  **Why.** Requests currently never reach Swift: `Bridge::emit` gives them directly to the internal executor (`host/apple/src/abi.rs:107-114`; `host/apple/include/exact.h:93-96`). The ibex binding performs the fetch and enforces grants there (`host/apple/src/executor.rs:77-109`). The current worker queue has tickets but no public outbox, generation, cancellation, or safe late-callback object (`host/apple/src/executor.rs:21-59`).

  D4’s encoding statement is also inaccurate. `Request`, `Outcome`, store snapshots, and `StoreWrite` are not all plan `Value`s. Current commands use lossy JSON—unit and none both become `null`, records become positional arrays—and Swift receives `[Any]` (`host/apple/src/batch.rs:127-158`; `host/apple/src/batch.rs:221-240`; `host/apple/swift/Bridge.swift:15-26`; `host/apple/macos/Presenter.swift:908-912`).

  A further security trap is redirects: checking the initial URL in Rust is insufficient if the app’s `URLSession` automatically follows a redirect to an ungranted origin.

  **Resolve.** Specify this boundary:

  - Rust intersects composition grants with the signed host ceiling and refuses before invoking the native transport.
  - An authorized request crosses as bounded canonical `Request` bytes plus a one-shot `(generation, ticket)` reply token.
  - The native client must not auto-follow redirects; each redirected target is returned to Rust for another grant decision.
  - A completion may arrive from any thread but only copies canonical `Outcome` bytes into a thread-safe runtime queue. Pumping and `Runner::fulfill` occur on the owner thread.
  - Duplicate, malformed, unknown, or stale completions are dropped/refused before reaching the runner.
  - Async callbacks never retain `ExactRuntime *`; they retain a separate safe reply-port object.

  For storage, retain LLP 1018 literally: obtain the granted-name snapshot outside a mutable runtime call, pass it into boot, and emit ordered writes after commits (`llp/1018-durable-client-state.rfc.md:82-135`; `host/apple/src/host.rs:242-259`). Do not make arbitrary app storage a callback from inside `parse`. A versioned service-wire definition plus a redirect test would settle this.

- **Blocking — Q9: The opaque handle has neither a realizable Rust lifetime boundary nor a reentrancy rule.**

  **Why.** The current crate denies unsafe code and `abi.rs` promises a safe boundary (`host/apple/src/lib.rs:31`; `host/apple/src/abi.rs:1`). Creating an opaque pointer can use `Box::into_raw`, but dereferencing and reclaiming it cannot. Avoiding that with integer handles would require a process registry, contrary to the ownership direction.

  Every current export uses `RefCell::borrow_mut()` (`host/apple/src/abi.rs:339-452`). A true callback into the same runtime during measurement or a future service invocation therefore panics; under `panic = "abort"` that kills the host. Separately, commands are delivered inline while a Swift batch is still being applied (`host/apple/ios/Presenter.swift:915-955`; `host/apple/macos/Presenter.swift:869-912`). A delegate can apply a newer batch recursively, after which remaining operations from the older batch overwrite its mirror.

  **Resolve.** Put raw-pointer handling in one tiny audited FFI crate, leaving the core safe. Define the pointer as valid until `exact_destroy` returns; never place it in async callbacks. Each runtime needs an owner-thread check and an `idle/busy/destroying` gate tested before entering its mutable core.

  My rule would be:

  - same-runtime synchronous reentry returns a typed `REENTRANT` status without touching its active input/output buffers;
  - different-runtime reentry is allowed;
  - only callback-originated events and completions are queued—never arbitrary synchronous boot, resize, or agent calls;
  - commands are accumulated, the complete batch is applied, the apply gate clears, and only then are delegates invoked FIFO;
  - destroy while busy is refused/deferred and never frees an active stack.

  An explicit, non-fatally enforced rule that every command delegate is asynchronous would also work, but documentation-only prohibition would not.

- **Blocking — Q8: Session-owned state and deliberately process-shared resources are not separated.**

  **Why.** Instancing Rust alone will not produce isolated sessions:

  - `Text` has one static catalog and caches, and installing a plan clears them all (`host/apple/swift/Text.swift:72-106`). Its font callback also has no session context (`host/apple/include/exact.h:77-78`).
  - `webviews` is global and keyed only by runner-local `UInt32` node IDs (`host/apple/swift/WebModule.swift:128-170`; `host/apple/swift/WebModule.swift:427`).
  - `canvases` is global in both adapters (`host/apple/macos/main.swift:46-49`; `host/apple/ios/main.swift:48-52`).
  - UIKit resetting one presenter resets the global canvases (`host/apple/ios/Presenter.swift:847-854`).
  - The bridge wake callback is static (`host/apple/swift/Bridge.swift:40-47`).

  Conversely, CoreText font registration is process-scoped (`host/apple/swift/Text.swift:151-164`), LLP 1024 deliberately forbids `dlclose` in v1 (`llp/1024-native-modules.rfc.md:349-359`), `NSApplication` belongs to the containing app, and D7 correctly permits a shared GPU device.

  D3’s unmount promise is also currently false: UIKit treats surfaces as visible whenever the application is foregrounded, and AppKit treats a missing window as visible (`host/apple/ios/Gpu.swift:290-303`; `host/apple/macos/Gpu.swift:265-278`).

  **Resolve.** Make the ownership split normative:

  - Session-owned: font catalog and shaping caches, asset origin, Web/GPU/native node maps, callbacks, request queue, clock/frame demand, journal, runtime heaps, and update subscription.
  - Process-shared: monotonic CoreText registrations, loaded dylib/function tables, the host-owned application/run loop, and optionally a GPU device/queue. Shared objects contain no session-local routing state.
  - Every callback is keyed by runtime and generation.
  - Teardown invalidates the generation, removes only that session’s observers/frame demand/surfaces, and releases references. It does not unregister fonts, `dlclose`, terminate the application, or destroy a device still used elsewhere.
  - Mounted/visible state participates in GPU frame demand.

  I would change this only where a platform/module proof demonstrates that unloading or unregistering is safe after the last reference.

- **Blocking — Q4: The embedded dev loop and its delivery owner are absent.**

  **Why.** `PlanURL.boot` and `PlanURL.current` are static, and opening a project closes the prior connection (`host/apple/swift/PlanURL.swift:117-128`; `host/apple/swift/PlanURL.swift:156-167`). The file poller, URL loader, menu, and presenter reset are wired in the standalone mains (`host/apple/macos/main.swift:196-233`; `host/apple/ios/main.swift:134-170`). Porting that singleton into `ExactSession` would immediately conflict with 1026 D12 and 1030.000 stage 4, which require both loaders to collapse into the update store (`llp/1026-dynamic-delivery.rfc.md:607-625`; `llp/1030.000-dev-server-as-deployer.rfc.md:412-418`).

  There is also a document conflict: 1031 treats the host descriptor as authoritative (`llp/1031-brownfield-embedding.rfc.md:260-275`), while 1030.000 defers the brownfield descriptor and embedding SPI (`llp/1030.000-dev-server-as-deployer.rfc.md:481-512`), despite already exposing `brownfield: true` (`llp/1030.000-dev-server-as-deployer.rfc.md:195-215`).

  **Resolve.** The app-scoped `ExactApplication` should own the embedded bundle, descriptor, compatibility ID, asset generations, update-store selection, and one dev/production connection. Its API should expose connect/disconnect, structured delivery status, check/reload/activate, and session attachment. A verified candidate is staged once; each attached session transactionally attempts it and pins whichever generation it retained. Raw `boot_plan` should be internal/testing SPI.

  Standalone adapters should own only `EXACT_DEV_PLAN`, Open Project…, ⌘R, and the four-finger gesture. The embedding SDK installs none of those. Pull the brownfield host descriptor into 1030.000 stage 2/3 for this combined push. I would accept a temporary extracted `PlanURL` only if stage 4 were no longer landing now.

- **Blocking — Q6: Content-height containment is promised without a safe constraint exchange.**

  **Why.** The current host stores only `(width, height)` and always lays out under `Offer::definite(w, h)` (`host/apple/src/host.rs:59`; `host/apple/src/host.rs:349-355`; `host/apple/src/host.rs:466-475`). `EXACT_MAX_CONTENT` is a text-measure sentinel, not a viewport offer, and `exact_resize` has no offer kind (`host/apple/include/exact.h:27-29`; `host/apple/include/exact.h:104`).

  There is no authoritative intrinsic page height: `content` is emitted only for non-visible overflow nodes (`host/apple/src/host.rs:483-495`), while both presenters floor page size to the assigned viewport, preventing shrink and feeding height back into itself (`host/apple/macos/Presenter.swift:807-815`; `host/apple/ios/Presenter.swift:856-865`). The existing page extent also omits visible-overflow descendants (`llp/1010-scrolling-v1.spec.md:118-123`). Finally, D3 promises edge chaining, but UIKit explicitly says real nested pans do not chain; only agent wheel events do (`host/apple/ios/Presenter.swift:31-36`).

  D10 nevertheless requires content-height proof (`llp/1031-brownfield-embedding.rfc.md:350-353`), while the implementation sequence permits bounded-only (`llp/1031-brownfield-embedding.rfc.md:405-406`).

  **Resolve.** Ship bounded-only in this implementation and make D3/D10 conditional. To ship content-height now, specify an axis offer type, an independent natural extent, and this loop guard: assigned height is never a kernel input in max-content mode; layout runs on width/environment/generation/content changes; the intrinsic-height cache is invalidated only after applying a batch and only when the point-rounded value changed; a second Auto Layout pass at the same width returns the cache without rerunning the kernel.

  UIKit/AppKit probes must cover shrinking, image arrival, text-size changes, percent-height roots, width changes, and real-touch nested scrolling, with no more than two layout passes. That evidence would change my recommendation.

- **Blocking — Q7: The external proof cannot be the sole CI holder.**

  **Why.** The external consumer is unnamed (`llp/1031-brownfield-embedding.rfc.md:439-442`). `EXACT_APP_DIR` points at an Exact producer workspace, while `build.mjs` invokes Cargo and builds a standalone executable (`scripts/app.mjs:19-27`; `host/apple/build.mjs:188-215`). That does not prove a no-Rust SwiftPM consumer. Both current Swift packages are executable targets linked directly to an app archive (`host/apple/macos/Package.swift:8-20`; `host/apple/ios/Package.swift:10-23`).

  An external adopter proves usefulness; a lean in-repo black-box consumer holds ABI, packaging, and lifecycle behavior after adoption. Saying such a fixture “would prove the opposite” conflates those jobs.

  **Resolve.** With Charlie’s explicit authorization, add one lean consumer fixture under the existing five checks—not a sixth check or a new harness (`rules/RULES.md:11`; `rules/RULES.md:64-67`; `rules/RULES.md:78-84`). At minimum it should:

  - import the public package and link the built binary without invoking Cargo in the consumer build;
  - create two sessions with overlapping node IDs and interleave operations;
  - mount/unmount/remount in a native hierarchy;
  - inject allowed and denied requests, store writes, and a command delegate;
  - exercise cross-runtime creation and same-runtime reentrancy refusal;
  - destroy one session with a late completion while the other continues;
  - refuse a bad replacement while retaining the prior generation;
  - exercise content-height only if that mode is public.

  A named external host built hermetically in the existing checks could replace this fixture; nothing in the capsule identifies one.

- **Material — Q5: “One store namespace” does not define cross-session visibility.**

  **Why.** LLP 1018 defines snapshot isolation: each runner receives a snapshot before boot, mutates its own memory, and emits writes afterward (`llp/1018-durable-client-state.rfc.md:82-135`). Apple currently creates those snapshots independently and persists each host’s writes independently (`host/apple/src/store.rs:25-35`; `host/apple/src/host.rs:242-259`). Therefore session A’s login write is not visible in already-running session B. Ordinary hot reload may carry B’s existing store rather than reread the platform. Treating the store as a reactive cross-runtime bus would also conflict with the cut on cross-runtime shared data (`rules/NOT-DOING.md:68`).

  **Resolve.** For v1, same-app sessions should share a durable namespace, but retain snapshot isolation. The app-scoped store adapter serializes write batches; the last persisted write wins per key, and a newly prepared session sees it. Existing live sessions do not receive peer mutations. State that plainly. If the first consumer requires immediate cross-session login visibility, Charlie must authorize a separate settlement/invalidation rule; it should not be inferred during implementation.

- **Material — Q11: D7 and D8 contain absolute guarantees the current mechanism cannot meet.**

  **Why.** D7 says the containing app’s first pixel is outside Exact and always precedes optional initialization (`llp/1031-brownfield-embedding.rfc.md:295-298`). That is false for the standalone adapter, whose first application pixel is an Exact surface, and an embedding library cannot force its caller to paint before constructing it. Current draw code merely observes that a pixel is “on its way” and dispatches initialization to the next main-queue turn; it does not prove compositor presentation (`host/apple/macos/Presenter.swift:614-621`; `host/apple/ios/Presenter.swift:539-544`).

  D8 says expected external failures are returned, but executor thread creation still uses `.expect("the executor thread")` (`host/apple/src/executor.rs:45-59`).

  **Resolve.** Replace the first-pixel claim with: optional initialization never synchronously blocks the applicable first-paint gate; brownfield hosts signal their first paint before `prepare`, while standalone adapters use the first Exact presentation. Convert thread/module/package creation failures into structured session failures. A compositor-backed timing probe and removal of expected panics would settle both claims.

- **Minor — Q11: “Process-global bridge” is technically inaccurate.**

  **Why.** The current bridge is thread-local, not process-global (`host/apple/src/abi.rs:12`; `host/apple/src/abi.rs:329-341`). It behaves as a singleton only because the presenter uses the main thread; another thread reaches a different empty bridge.

  **Resolve.** Describe it as an implicit main-thread/TLS singleton. This does not alter the ownership conclusion.

4. **Suggestions**

- Replace the public `ExactSession(plan:services:store:)` example with `ExactApplication.load(...)` followed by `makeSession()`. Raw plan bytes bypass the descriptor, compatibility identity, assets, and update-store policy that D6 makes authoritative.

- Add three normative tables: artifact topology by composition, application-versus-session ownership, and deliberately process-shared resources. These would eliminate most ambiguity without adding machinery.

- Give the C ABI a lifecycle state diagram and explicit result structure. The existing `u32` length plus later `exact_out()` is awkward for reentrancy, wrong-thread, and destroyed-state errors.

- Distinguish the durable client-state store from the delivery update store everywhere. Both are called “store” but have different namespaces, security models, and lifetimes.

- Make the first release limitations prominent: one Exact application/plan, N instances of that plan, bounded containment, caller/main-thread serialization, and snapshot-isolated durable state.

- Rewrite D10 as two proofs: an in-repo conformance fixture and a named external adoption consumer. Neither substitutes for the other.

5. **Open questions**

- Does the first named consumer require three distinct entry surfaces, or only several instances of one surface? Charlie and that consumer must decide; this determines whether named plan entries are needed now.

- Is live cross-session authentication state a product requirement, or is snapshot isolation acceptable? Charlie and the first consumer must decide.

- Is content-height required for that consumer? Charlie decides; absent a concrete requirement and measured exchange, bounded-only should ship.

- Will packaging use an app-specific composition XCFramework or an erased universal core? The Apple/build implementer must provide the link prototype and artifact graph.

- Can the app transport disable redirects and expose each hop for Rust authorization? The ibex/network owner must settle that contract.

- Can `gpu_load` safely run more than once, or must one process owner vend devices/module tables? The GPU module owner must inspect the absent module implementation.

- Which existing check owns the public-package fixture, and does Charlie authorize that fixture as permitted apparatus? Charlie and the CI maintainer must answer.

- What named external UIKit/AppKit application is the adoption proof? The RFC itself leaves this unresolved.

6. **Recommended next step**

Revise and stay Draft. Another family is unlikely to resolve these product and artifact choices; the author and first consumer must choose them. I would land the combined 1031/1030.000 work in this order:

1. Revise 1031 to fix artifact topology, one-plan cardinality, app/session ownership, service wire formats, reentrancy, process sharing, snapshot semantics, and bounded-only scope.
2. Instance the Rust core and add the audited opaque-pointer FFI boundary; prove two handles, cross-handle calls, same-handle refusal, and safe destruction.
3. Extract bounded `ExactSession`/`ExactView`, move all Swift semantic state per instance, and reduce both standalone mains to adapters.
4. Land the lean public-package fixture under existing checks.
5. Add the app-scoped owner and request/store/command/log/asset adapters, including generation-safe completion ports.
6. Land 1030.000 stage 1: asset row and shader packaging against the new app/session asset resolver.
7. Land stage 2: `app.json`, generated host files, receipt, and—pulled forward from the deferred brownfield section—the host descriptor.
8. Produce the chosen Rust-only composition package and prove that its Swift consumer invokes no Rust toolchain.
9. Land stage 3: compute the compatibility ID from the real package and descriptor, then classifier, `exact deploy --dry-run`, `delivery`, and `state.delivery`.
10. Land stage 4: bundle publishing, update store, and the unified development/production loader; wire embedded hot reload directly to that app-scoped owner.
11. Reattach GPU, Web/native modules, Hermes, and wasmtime one at a time, validating descriptor/classifier facts for each.
12. Keep content-height deferred unless the platform probes pass, then run the named external hostile-host proof.

This order avoids extracting the singleton `PlanURL` only to delete it, avoids packaging against global asset lookup, and ensures the compatibility ID describes the binary and executors that actually ship.

---

## Round 2 — after reading the other two (verbatim)

--- PANEL ANSWER (round 2) ---

**F1 — FINAL.** Ship the app-specific archive this week: `lib<app>_apple.a` from `host!`, its generated Swift package, and separately selected engine/module artifacts. I agree with Sol §3 “Blocking — Q1” and Claude §3 “Blocking — D6,” and disagree with Grok §3 B1. Type erasure is a coherent destination, but doing it now creates a new `DataSource` factory/vtable ABI covering construction, grants, plan lifetime, destruction, panics, and every LLP 1029 composition. The app-specific archive is already prebuildable, so its Xcode consumer still needs no Rust toolchain; v1’s one-app rule also removes duplicate-export pressure. Reopen universal ExactCore when a named TypeScript-only consumer needs one unchanged core binary—or one host must co-link multiple Exact apps—and a prototype proves the erased lifecycle end to end.

**F2 — FINAL.** Accept the integer handle as built; a pointer is not required. I agree with Claude §3 “Minor — the handle can be an integer,” and reject Sol §3 “Blocking — Q9” insofar as it treats the registry as contrary to ownership: the session still owns exactly one runtime, while the registry safely allocates and validates it. The built refusal behavior also satisfies Grok §3 B3 without an audited unsafe island. One invariant must be checked before landing: IDs must be process-unique even though storage is thread-local, otherwise handle `1` from thread A could accidentally resolve to handle `1` on thread B; use a global atomic allocator, refuse exhaustion, and never wrap or reuse. I would change my mind only for a demonstrated requirement for thread-mobile runtimes or independently addressable ABI providers that this token design cannot satisfy.

**F3 — FINAL.** Add a thin app-scoped owner in this push, before the asset row. I agree with Sol §3 “Blocking — Q4”; Grok §3 M2 is right only about adapter-owned UI such as menus, gestures, and origin selection. `ExactApplication` should own `app_id`, the archive descriptor and compatibility ID, the durable-state namespace, and generation-addressed asset resolution; it later acquires one update-store connection. Sessions own runtimes and transactionally attempt candidates staged once by the application, retaining their prior generation and assets on refusal. Raw session `applyPlan` should be internal/testing SPI, not the public delivery owner; Claude §3 “Material — the dev loop” would otherwise produce one connection per session. I would defer this owner only if the whole push were explicitly narrowed to an internal static-plan extraction and the asset row, descriptor, updater, and public package were all postponed.

**F4 — FINAL.** Do not build host-replaceable request or durable-client-store crossings in this push. I agree with Grok §5 Open question 3’s scoped landing, while treating Sol §3 “Blocking — Q3” and Claude §3 “Blocking — D4” as the required design when that boundary is triggered. Ship command, log, and application-owned plan application; retain ibex2 and Keychain as the defaults, and make the fixture exercise an allowed request and a Rust-denied request through that existing path. Narrow D4 and D10 accordingly rather than advertising an unbuilt replacement seam. When added, the crossing must use canonical `Request`/`Outcome` bytes, reauthorize every redirect hop in Rust, retain a thread-safe reply port rather than the TLS runtime handle, key completions by `(generation, ticket)`, snapshot granted store names before boot, and emit ordered writes only after commit. A named adopter requiring its authenticated client or storage protection policy for first use would change my mind and pull that complete crossing—not a partial callback—into this release.

**F5 — FINAL.** I agree with Grok §3 M1, Sol §3 “Blocking — Q2,” and Claude §3 “Blocking — several surfaces”: v1 is one `app_id`, one plan, and N independent sessions of it; only a named adopter needing independently addressable roots would reopen named entries or N plans through an explicit LLP 1023/1030 amendment.

**F6 — FINAL.** Given today’s landings, I replace the remaining portions of Grok §6, Sol §6, and Claude §6 with this order:

1. Extract the thin `ExactApplication`, `ExactSession`, and bounded `ExactView`; make both standalone mains adapters and keep transactional plan application beneath the application owner. Let the shader and compatibility-ID lanes continue in parallel.

2. Land the lean hostile-host fixture immediately: two sessions of one plan, interleaved calls, mount/remount, command delivery after apply, bad-plan retention, invalid/busy-handle refusal, destroy with late work, and agent routing.

3. Land the asset row once shader packaging and its interface digest join. The application owns resolution/cache policy; each session pins the generation it successfully applied.

4. Complete stage 2 by having bake emit the brownfield app-archive descriptor beside the already-generated manifest, host files, and receipt, once the compatibility-ID lane joins.

5. Land the artifact classifier and `exact deploy --dry-run` against the complete asset/shader graph, compatibility ID, receipt, and descriptor.

6. Land the `delivery` resource, its commands, and `state.delivery`, first proving the honest embedded/`L = 0` result with no updater.

7. Leave request/store replacement as a triggered lane. If a first consumer triggers it, implement the complete crossing above and extend the fixture before packaging that consumer; otherwise record it as deferred, not stubbed.

8. Land the update store, then publishing: transactional selection/signing/anti-rollback/fallback and application-owned candidate staging first; origin CAS, per-stream heads, and `--watch` second. This store is independent of the deferred durable-client-store crossing.

9. Generate the distributable Swift package around the app-specific archive and record final linked deltas for stripped `L = 0`, `L = A`, and each optional artifact.

The two real dependency chains are ownership first—application/session/view before the fixture, asset resolver, and updater—and artifact truth first—shader/assets plus compatibility ID and descriptor before classification, with classifier and update store before publishing. Everything else may proceed in parallel.
