# Panel: LLP 1031 Brownfield embedding (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`. Reported model not independently verifiable from the CLI; requested `grok-4.6` at `xhigh`.
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
- **Peer:** codex `gpt-5.6-sol` at `ultra`, and Claude Fable 5.1 (the orchestrating session, reviewing independently before reading either), same capsule, same rounds.
- **Disposition:** folded into LLP 1031 r2 the same day — §8 records which catches held and where each landed, and its "Round 2" table the final positions; the author re-verified every code claim against the tree before folding (the `thread_local!` bridge and fixed exports in `abi.rs`; `executor.rs` running every request on `ibex2::host` with the grant check inside ibex2's `Bindings`; `Text.install` wiping process-wide caches; `PlanURL.current` and `Exact.wake` as statics; commands delivered inside `Presenter.apply`). Implementation began the same day under 1031 §4.

---

## Round 1 — position (verbatim)

I'll start by reading the binding rules and the RFC, then ground the review in the Apple host code and the LLPs this design sits on.Next I’ll read the Apple ABI and the delivery LLPs so the panel points sit on actual code, not the RFC’s summary of it.The ABI is a process-global singleton; next I’ll read the presenter, store, executor, and the related LLPs the RFC claims to sit on.I have enough of the ABI, presenter, and delivery docs to write the panel from code, not from the RFC’s summary of it.--- PANEL ANSWER ---

## 1. Overall assessment

The ownership cut is right and I would build *that*: `ExactSession` + `ExactView` as the primitive, standalone `main.swift` as an adapter, every C call taking an `ExactRuntime *`, no second presenter and no new Contract vocabulary. I would not build the RFC as written. D6’s app-agnostic **ExactCore** (C ABI included) contradicts the `host!` macro that *is* the C ABI; D4’s host-performed requests and host-supplied store have no Apple crossing today and would silently drop LLP 1016/1018’s grant boundary if naively pushed into Swift; D1’s “several sessions in one process” is unimplementable on the current font catalog, GPU `dlopen`, and `RefCell` handle without rules the RFC either overclaims (D2: nothing process-global) or omits (reentrancy, store sharing, the dev loop). Fold those into r2, then implement — the singleton refactor is the thing this push actually needs, and 1030/1030.000 can ride beside it if the ABI is instanced *before* the update store lands.

## 2. Strengths

- **D1 is the right primitive.** `exact_in` / `exact_out` / `exact_boot` have no instance (`host/apple/include/exact.h:85–119`). `host!` puts one `thread_local!` `RefCell<Bridge<D>>` behind fixed `#[no_mangle]` names (`host/apple/src/abi.rs:337–347`). Swift wraps that as a static `enum Exact` with one `wake` (`host/apple/swift/Bridge.swift:15–42`). Both `main.swift` files own the process, window, presenter, canvases, clock, and PlanURL (`host/apple/macos/main.swift:37–47, 196–215`; `host/apple/ios/main.swift:50–51, 137–171`). That is a whole app, not a view. Making standalone a thin adapter over the same surface is the only way embedding does not grow a second presenter (the four-disagreeing-layers bug `rules/RULES.md` exists to prevent).
- **D5 is correct and small.** Commands already cross as `{"op":"command",…}` after the commit (`host/apple/src/batch.rs:145–147`; applied at `Presenter.swift:910–911`). Exact must not touch `UINavigationController`. LLP 1024 remains the only native-content path; embedding must not grow a plugin system.
- **D8 is honest.** An in-process library cannot promise process isolation. Stating `panic=abort` / native UB as process death, and bounding only expected refusals, is the promise a brownfield team can actually use.
- **D9 does not add a ninth operation.** Session selection as carrier routing is the only shape LLP 1012 and `rules/NOT-DOING.md` §Agent API allow. `tree`/`state`/`logs` are already runner-side behind `exact_agent` (`exact.h:117–119`); `tap`/`type`/`layout`/`screenshot`/`clock` are already presenter-side (`host/apple/swift/Agent.swift:6–9, 43–54`). Coordinates through `ExactView` is the conversion those host ops need.
- **D3’s two containment shapes are a native offer, not a Contract semantic.** Right. `Presenter.viewport` is a `PageScrollView` (`host/apple/macos/Presenter.swift:770–771`); 1008 §1 is a standalone page. Nested overflow already chains per LLP 1010. Bounded-first in §4 step 3 is the implementable order (see concern on D10 fighting it).
- **D7’s cost table is the right measurement.** Link delta against a minimal host, optional artifacts paid separately, containing app’s first pixel always first. Matches LLP 1000’s “another executor / on-demand artifact, never a cargo feature,” 1009 D2, 1027 D3, 1029 D2/D6.
- **Smallest embedding is baked, Rust-only, no update store.** Agrees with 1030 D4’s `L = 0` and with §3’s “no dynamic delivery requirement.” That is the shape that can land beside 1030 without waiting for 1026 Level A.
- **Destroy generation/token** is the correct extension of the reload path that already drops the old executor’s outcomes (`abi.rs:214–220`).
- **Values, not an object bridge.** Keeps `rules/NOT-DOING.md`’s refusal of a shared heap and 1005’s canonical `Value` seam.

## 3. Concerns

### B1. ExactCore as specified cannot contain the C ABI — **Blocking** (Q1, Q11)

**Why.** D6 puts “runner, kernel, plan decoder, motion, **C ABI**” in a prebuilt ExactCore “installed with Swift Package Manager and no Rust toolchain.” The C ABI is not a crate. It is `host!` (`abi.rs:337–381`): a `thread_local!` `Bridge<$data>`, `#[no_mangle] extern "C" fn exact_boot` that boots `$plan` with `<$data as Default>::default()`. Caltrain *is* that one line (`apps/caltrain/apple/src/lib.rs:9`). The archive the Swift packages link is `libcaltrain_apple` (`host/apple/macos/Package.swift:8–20`). LLP 1029 D2 is explicit: “The `host!` composition is the one place an app says what its binary links” and names four types (`Either<data::App, exact_js::Module>`, `exact_js::Module`, `data::App`, `Swappable<…>`). A prebuilt ExactCore that still uses `host!` is either Caltrain-shaped or it is not the C ABI.

Two `host!` archives in one process also collide: every app exports the same `exact_boot` name. N Exact apps, or even ExactCore + an app artifact both emitting the ABI, will not link.

1023 already stated the constraint instead of fighting it: “The ABI macro compiles in exactly one `D: DataSource`, so a client can only boot a plan whose data crate it already links” (`llp/1023-one-url-serving.rfc.md:105–108`).

**Position (Q1).** The unit a brownfield app *links*, per composition:

| Composition (1029 D2) | Prebuilt, does not change per app | App-specific (from `exact bake`) |
|---|---|---|
| TypeScript-only (`exact_js::Module`) | ExactCore + ExactAppleUI + ExactHermes | plan + `.hbc` (loaded, not linked) |
| Rust-only (`data::App`, Caltrain) | ExactCore + ExactAppleUI | native DataSource artifact |
| Mixed (`Either<App, Module>`) | ExactCore + ExactAppleUI + ExactHermes | native DataSource artifact + bytecode |
| + `Swappable` | + ExactWasm | the wasm/cwasm module as a bundle file |
| + canvas / iframe / hyphenated tags | ExactGPU / ExactWeb as on-demand dylibs | app GPU crate, app native-module dylib |

What can be a prebuilt binary: ExactCore **without** a monomorphized ABI and **without** baked plan bytes; ExactAppleUI; ExactHermes; ExactWasm; ExactGPU; ExactWeb. What cannot: the DataSource crate, the app GPU crate, the native-module roster. “SPM and no Rust toolchain” is true for *consumers of a baked TypeScript-only (or already-baked) surface*. It is false for anyone editing native Rust, which the RFC already says — and it is false for ExactCore-as-`host!`.

1029 D2 stays the *authoring* line. Bake records `E` in the host descriptor (1030 D3a/D4). ExactCore exports the instance C ABI **once**, type-erased; the app artifact *registers* a DataSource (vtable, or Hermes module bytes) at `exact_create`. `exact_boot` of baked bytes becomes the standalone adapter’s convenience, not the SDK’s.

What would change my mind: an XCFramework of today’s `host!` that boots a plan whose `DataSource` was not compiled into it. That cannot exist without the type-erasure above.

**Resolve.** Rewrite D6: ExactCore = type-erased runner/kernel/motion/instance ABI; the app’s `host!` shrinks to a registration shim produced by bake; plan bytes are an argument, not a `static`. Pull 1030.000’s deferred **host descriptor** (`llp/1030.000-dev-server-as-deployer.rfc.md:506–512`) into this push — D6’s “linkage and the descriptor must agree” is otherwise a check against a document that 1030.000 r2 explicitly left for a later number. Do not generate the containing app’s `Info.plist` from Exact’s `app.json` (1030 D2 is for the standalone adapter).

### B2. Host-performed requests and a host-supplied store have no Apple ABI, and the grant check lives in the wrong process if you invent one casually — **Blocking** (Q3, Q11)

**Why.** D4 says the containing app’s client performs the request “through LLP 1016’s tokened queue,” and the store is “a namespaced implementation of LLP 1018, backed by the app’s chosen storage.”

Today a request **never reaches Swift**:

- `exact.h:93–96`: “A request the app sends (LLP 1016) runs on the library’s own executor thread — ibex2::host — never through the host.”
- `abi.rs:107–114`: `emit` takes `h.take_requests()` and `x.run(r)` into the internal `Executor`.
- `executor.rs:45–56, 77–104`: worker thread; `b.fetch.send(req)`; `HostError::Denied` is the grant refusal.
- LLP 1016 D2 (`llp/1016-async-data-settlement.rfc.md:125–132`): Apple “a `request` op never reaches Swift.” The web *does* emit `{"op":"request",ticket,…}` and has `exact_fulfill`.
- Apple `batch.rs` has `command` (`:145–147`) and no `request` op.
- `exact_pump` takes only `now_ms` (`exact.h:96`). Outcomes live inside the library. There is no `exact_fulfill` on Apple.

The **runner’s** ticketed queue (`Runner::fulfill`) is enough. The Apple `Executor` is the *performer*, not the queue. Pump-draining-the-internal-executor is not a host-supplied transport.

The grant check is in ibex2 `Bindings` at `fetch.send` (`executor.rs:101–104`; 1016 D6). If Swift’s `URLSession` performs the request and Exact only `fulfill`s the bytes, **the one Rust boundary where grants are checked is gone**, unless Exact refuses ungranted work *before* the op crosses the ABI (as `glue.js` does on the web). D4’s “unknown or ungranted work is refused loudly” is otherwise a Swift honor system.

The store is the same pattern:

- `store.rs:1–4, 14–22, 25–35`: endow, snapshot before boot, nothing through Swift.
- `host.rs:246–258`: `persist` writes `Secrets::set`/`forget` after each commit, on the main thread.
- 1018 D6: “Nothing reaches Swift: no ABI change.”
- 1018 D7: Keychain is *bundle*-scoped (code signature / access group), not session-scoped.

A host-supplied store must still: (1) snapshot granted names into the runner before boot (first frame is a returning user — 1018 D1), (2) accept `StoreWrite`s after commit, (3) refuse ungranted names in Rust (`Store::set` already does) before the host is asked. An open callback that Swift can write arbitrary keys through is a grant bypass.

**Position (Q3).**

*Requests, each direction:*

- Out: the existing web shape — `{"op":"request","ticket":N,"method","url","headers","body"}` in the batch, **after** Exact has matched the URL against the data crate’s `net.fetch` grants. Swift never sees an ungranted URL.
- In: `exact_fulfill(runtime, ticket, generation, status, len)` with the body in `exact_in`; forgotten tickets drop (1016 D5); generation mismatch drops (D2’s token). Then `exact_pump` as today, but draining a host-fed queue rather than `Executor::drain` only.
- Default adapter: keep `ibex2::host` as the performer when `ExactServices` does not replace transport. Do not remove the internal executor; make it the default implementation of the same seam.
- The ticketed queue as built is enough **on the runner**. It is not enough **on the Apple ABI**.

*Store, host-supplied form:*

- Config at `exact_create`: a snapshot `[(name, value)]` of granted names, or a read callback invoked synchronously before boot (1018 requires synchronous; an async secure-store is the splash-screen bug 1018 rejected).
- Out: either `{"op":"store","tier","name","value"|null}` in the batch (web already has this) or a persist function-pointer in `ExactRuntimeConfig`. Writes already roll back with a refused commit (`host.rs` persist runs from `commit` after settlement).
- Namespace: see M3. The host’s backing is one per `app_id`, not per session handle.

What would change my mind: keeping D4 as “optional replacement” and shipping v1 embedding on ibex2 + Keychain only. That is a coherent descoping — but then D10 item 3 and D4 as written are lies and must be pulled from the first release.

**Resolve.** Add the two ABI crossings (request op + `exact_fulfill`; store snapshot in + store op out). State that grants are checked in Rust before either crossing. Keep ibex2/Keychain as the adapter default. Do not send kernel pointers or an open object bridge — D4 is right about that half.

### B3. Same-handle reentrancy will abort the containing process on ordinary delegate work — **Blocking** (Q9)

**Why.** The bridge is `RefCell<Bridge<D>>` (`abi.rs:329–330, 373–381`). A nested `borrow_mut` panics. D8 names `panic=abort` as process death (workspace `Cargo.toml` is not in this capsule; the RFC treats abort as given). The presenter already knows the runner must not be re-entered from *view* events (`Presenter.swift:839–848, 869–880`: `applying` queues them). **Commands are not queued.** They fire inside `apply` (`Presenter.swift:910–911`) while the C call that produced the batch is still on the stack. D5’s `ExactSessionDelegate` is specified to “push, present, dismiss.” A `viewDidLoad` that `exact_create`s another session is a second handle (survivable once the ABI is instanced). A `viewDidLoad` that talks to the *same* session — or a naive `exact_dispatch` from the delegate — is a `RefCell` panic and, under abort, kills the host app. That is expected brownfield input, not UB. D8 promised expected failures return.

**Position (Q9).** Rule at `ExactRuntime *`: **same-handle calls are refused with an error batch; they are not queued and not undefined.** Cross-handle calls on the same thread are allowed. Command (and any other) delegates run **after** the batch apply completes, the way `waiting` events already do. Creating session B from session A’s *post-apply* delegate is allowed. Held by a reentrancy flag on the Bridge (check, return `error`, do not `borrow_mut` twice) plus the Swift wrapper. Queueing nested calls is the wrong shape: it reorders commits relative to the delegate’s world.

What would change my mind: a documented “delegates must be async” rule *and* Swift-side enforcement that makes a synchronous callback a compile-time or immediately-returned error without touching the RefCell. Still need the C-side flag; a C caller can ignore Swift.

**Resolve.** Write the rule in D2. Move `onCommand` out of the `applying` critical section. Put a generation + in-call flag on the handle.

### B4. D2’s “nothing remains process-global” is false; implementing destroy that way breaks sibling sessions — **Blocking** (Q8, Q11)

**Why.** D2: “No runner, bridge buffer, font catalog, wake callback, clock, store, or journal remains process-global.” The code and D7 disagree, and the code wins.

- Fonts: `host.rs:162–164` — “Font installation is process-global on Apple.” `Text.install` `removeAll`s process-wide `fonts` / `paragraphs` / `catalog` (`Text.swift:73–75, 103–106`) and registers with `CTFontManagerRegisterFontsForURL(..., .process)` (`Text.swift:151–153`). `Bridge.boot` calls `exact_set_fonts(installFonts)` every time (`Bridge.swift:46–47, 67–69`). Session 2’s boot **wipes session 1’s catalog** and can collide on PostScript names (`Text.swift:155–162` already special-cases `alreadyRegistered` / `duplicatedName` for *reloads*, not siblings).
- GPU: `GpuModule.load` `dlopen`s once (`GpuModule.swift:55–73`); `gpu_load()` is process init. D7 already says a GPU module may share one device. `dlclose` on session destroy is typically unsafe for wgpu.
- WebKit arm: the same `dlopen` pattern (`WebModule.swift:43`).
- `NSApplication` / `UIApplication`: the containing app’s. D1 correctly says Exact never creates them; D2’s “nothing process-global” still reads as if Exact should pretend they are instance state.
- Wake: `Exact.wake` is one static (`Bridge.swift:42`). Instancing the ABI without instancing wake ctx makes session A’s pump run session B’s batch.

D1 and D10 *require* two live sessions. Building D2 as a purity rule will either refuse the second session or destroy the first’s text/GPU on the second’s boot/teardown.

**Position (Q8).** Share deliberately, refcount, do not pretend otherwise:

| Thing | Share? | Teardown |
|---|---|---|
| `ExactRuntime` buffers, runner, clock, journal, store *handle*, wake ctx | never | freed on `exact_destroy` |
| `Text` catalog / paragraph cache | **per session** (today’s static is a bug under D1) | drop the session map; do not wipe siblings |
| `CTFontManager` process registration | **yes, by URL, refcounted** | unregister a URL only when the last session using it is gone |
| GPU/Web/native-module **dylib** (`dlopen`) | **yes, once** | never `dlclose` while any session exists; prefer never |
| GPU **device** | **yes**, as D7 already says | destroy per-surface instances (`gpu_destroy`); keep the device |
| GPU **surface instances** | never | destroy with the session (D2 step 3 is right about *instances*) |
| `NSApplication` / scene / window | the host app’s | Exact does not own them |

What would change my mind: a CoreText API that is actually session-scoped. There isn’t one. The RFC should say so the way 1008/host.rs already does.

**Resolve.** Replace D2’s “nothing process-global” with the table. Make `Text`’s maps instance-owned (keyed from the runtime, passed through the measure ctx that already exists). Refcount font URLs. `Canvases` per `ExactView`; `GpuModule` process-shared.

---

### M1. Several surfaces vs one plan is unspecified and fights 1023/1030 if you guess wrong — **Material** (Q2)

**Why.** Motivation: settings, onboarding, dashboard as three surfaces. D1: a session owns “one decoded/running plan.” 1023 D10: one plan is the invariant. The envelope names one `plan` (`llp/1023-one-url-serving.rfc.md:131–138`; `PlanURL.swift:308–314`). `Runner::boot` gates `app_id` (`llp/1023.001-serving-stage-1.spec.md:149–150`: `AppMismatch { plan, host }`). A stream is `(app, channel, compatibility id)` (1030 D3a). D10’s proof wants two independent sessions. None of this says whether those are two mounts of one plan or three products.

Four shapes and what each does:

1. **N `.contract` files, one data crate, N plans in one envelope.** Breaks 1023 D2 (one `plan` key) and D10. Compatibility id unchanged if the crate is one (1030 D3a includes crate identity, not plan bytes). `Runner::boot` accepts all of them if `app_id` matches. Streams: still one per app, so N plans cannot be versioned independently without forking 1030 D3a. **Do not do this in v1.**
2. **One plan, session names an entry component.** Needs a Contract/runner boot parameter the RFC forbids itself from adding (§3: no new tag, state plane, or agent op). **Not v1.**
3. **N sessions of one plan.** Envelope, stream, compatibility id, `app_id` gate: unchanged. Each session is a full boot of the same bytes with its own store handle, clock, and tree. Proves isolation (D10 items 1, 6, 8). Does **not** give settings vs dashboard unless the plan’s own state selects the screen — and then one session that survives unmount (§5 Q1) is the product shape, not three.
4. **One Exact app per host binary in v1; extra apps deferred to 1023 D6’s `BundleEntry` registry** (native multi-app / launcher, unbuilt). Cleanest. Two sessions in D10 are two mounts of Caltrain (or of the sample host’s one plan), not three products.

**Position (Q2).** v1 is (4)+(3): **one `app_id`, one plan, N sessions of that plan.** Product “three features” is either Contract routing inside that plan plus one session remounted in three native containers, or a later 1023 D6 registry (N apps, N envelopes, N streams). Do not grow `plans: []` on the envelope in this push.

What would change my mind: a named consumer with three unrelated `.contract` files sharing one crate that cannot be routes of one plan. Then N plans against one `app_id` is a 1023 amendment, written as such, with the envelope still one *stream*.

**Resolve.** Say this in D1. D10 item 1 is “two sessions of one plan.” Charlie’s §6 Q2 (“multiple isolated sessions required from first release?”) should be answered **yes for isolation, no for N products.**

### M2. The embedding API has no dev-loop seam, and 1030.000 stage 4 will otherwise invent a second one — **Material** (Q4)

**Why.** The RFC does not mention the dev loop. Today:

- `EXACT_DEV_PLAN` file poll or URL (`macos/main.swift:196–232`; `ios/main.swift:137–171`).
- `PlanURL.boot` is a process-global closure that `exact_boot_plan`s and `presenter.reset()`s (`PlanURL.swift:127`; `main.swift:202–214`).
- Dev menu / 4-finger tap live on the window (`ios/DevMenu.swift:25–34, 59–76`).
- `{rebuilt}` is session-terminal (`PlanURL.swift:431–437`).
- 1026 D12 / 1030.000 stage 4 collapse the file poll and the two URL loaders into **one update store with two policies**.

If 1030.000 stage 4 lands on the current globals, the store is process-global and every embedded surface reloads together — or the embedder cannot hot-reload at all. If 1031 lands without `applyPlan`, the adapter cannot attach `dev.mjs` to an `ExactView`.

**Position (Q4).** The session exposes **`applyPlan(bytes, carry:)`** (today’s `exact_boot_plan` + `Host::carry`, `host.rs:228–230`) and optionally **binds an update-store client** as a service, the same way it binds transport. It does not grow a ninth agent op; `{seq}` is a caller of `applyPlan`. The adapter (standalone, and a brownfield debug overlay if the embedder wants one) keeps: the dev menu, the 4-finger tap, `PlanURL` / SSE subscribe, the `{rebuilt}` “rebuild the host” message. An `L = 0` embedder never links the store; they still call `applyPlan` from their own debug UI. Do not force 1026’s store into the embedding SPI.

What would change my mind: 1030.000 stage 4 slipping out of this push. Then `applyPlan` is still required; only the store client can wait.

**Resolve.** Add a short D11 (or a D4 bullet): session SPI = `applyPlan` + optional store binding; window-level chrome stays the adapter. Land 1031 step 1 before 1030.000 stage 4 so the store is not born global.

### M3. Store namespace vs the Keychain is ambiguous; two sessions of one app must share secrets — **Material** (Q5)

**Why.** D1: each session has “one store namespace.” D4: the app’s storage backs it. 1018 D7: Apple’s store is the Keychain, scoped by **code signature / access group**, keyed by grant names (`secret.keep castle.session`). `snapshot_of` reads those names at boot (`store.rs:25–35`); `persist` writes them (`host.rs:246–258`). There is no session id in the key.

If two sessions isolate: a login in settings is invisible in the dashboard — adoption-killing, and not how `localStorage` (the web standard 1018 binds to) works. If they share the durable map but each has its own in-memory `Store`: last `persist` wins; session B that booted earlier has a stale snapshot until it re-reads; concurrent writes are last-write-wins with no transaction. That is the web’s model (`localStorage` is one origin, last write wins).

**Position (Q5).** Durable secrets are **per `app_id` (and grant name), shared across sessions of that app.** A login in one is visible in the other on the next snapshot (boot or an explicit re-read — v1 can be “visible after remount / next boot,” which is already 1026’s production apply-at-next-launch shape). In-memory `Store`s may diverge between persists; that is said loudly. Different `app_id`s never share. Host-supplied backing must be safe for concurrent `set` of the same name (Keychain is; a raw `Dictionary` is not). Agent mode stays memory-empty (`store.rs:17–21`) **per process**, not per session — a drive starts from nothing.

What would change my mind: a consumer that wants two logged-in accounts of the same app in one process. That is a different product (store namespace = app_id + account), not v1.

**Resolve.** Define “namespace” in D1 as `app_id`, not session handle. Document last-write-wins. Do not invent merge.

### M4. Content-height is not Auto-Layout-safe as specified; D10 requires it while §4 defers it — **Material** (Q6)

**Why.** Layout is always `Offer::definite(w, h)` (`host.rs:468–473`). A root is as tall as its content under that offer (1008 §1; `Presenter.fitDocument` at `Presenter.swift:807–815` sizes the *document*, and the window `NSScrollView` scrolls it). `EXACT_MAX_CONTENT` already exists (`exact.h:27–29`) and is used for **text measure**, not root layout (`measure.rs` via `AxisOffer::MaxContent`). D3’s content-height mode is a new use of an existing constant, not a free ABI.

The Auto Layout loop: parent assigns width → Exact lays out → reports `intrinsicContentSize.height` → parent may change width (scrollbar, readable-content guide, collapsing stack) → Exact lays out → height changes → … The guard cannot be “relayout when the offer changes,” because the height *we reported* must not become the next definite height offer (that is the RN/Yoga loop). Guard: **in content-height mode, only width invalidates layout; the height offer is always max-content; applying the intrinsic height as a bounds change does not call `exact_resize`.**

D10 item 2 requires a content-height surface in the *proof*. §4 step 3 says bounded first, content-height only after a measured exchange. §5 Q2 leans yes for adoption. Those three sentences do not agree.

**Position (Q6).** Ship **bounded-only** in the first implementation. Say so in D3 and cut D10 item 2 to “bounded in a native hierarchy; content-height is a named follow-up.” Nested overflow inside the surface stays as LLP 1010. Outer native scrolling of Exact content waits on one measured exchange with the loop guard above.

What would change my mind: that measurement, in UIKit and AppKit, with rotation and a scrollbar-caused width change, showing no loop. Then it can join the same PR. Not before.

### M5. D10’s “in-repo showcase proves the opposite” fights the five checks — **Material** (Q7)

**Why.** D10 wants an unnamed external UIKit/AppKit consumer driven via `EXACT_APP_DIR`. `rules/RULES.md`: five checks, 60 s, agents add no apparatus without a human. 1008 §6 already uses `host/apple/macos/floor.swift` as the empty-app floor `metrics.mjs` builds — an in-repo host that is *not* Caltrain. Weird Castle is the external-app precedent, and it is named.

An in-repo sample host does not prove “Exact feels like a view library in a years-old app.” It *does* prove the ABI, two sessions, destroy-with-outstanding-request, command delegate, agent ops, and that standalone still works through the same handle. Without it, the five checks cannot hold the embedding contract after the unnamed consumer’s repo moves. D10 item 10 (“removes Exact without changing the host’s architecture”) is not a check anyone can run on an unnamed app.

**Position (Q7).** Split proof from fixture. **Fixture (CI, in-repo):** a lean UIKit/AppKit host in the `floor.swift` slot — not an Exact-owned window — that creates two sessions of one plan, mounts a bounded `ExactView`, handles a command, destroys one session with a request in flight, and is driven with the eight operations. No sixth check: drive it from `scripts/smoke.mjs` / `scripts/agent.mjs`. **Proof (product):** a named external consumer, when Charlie names it, via `EXACT_APP_DIR`. Until then the RFC may guide the singleton refactor and must not grow SDK apparatus (D10’s own last sentence, and §5 Q7, already know this).

Minimum fixture: D10 items 1, 4, 6, 8, plus one bounded mount and a destroy. Not content-height (M4), not every optional artifact (bytes can be `metrics.mjs --long`), not “removes Exact from a real app.”

**Resolve.** Rewrite D10’s “tiny in-repo showcase would prove the opposite.” It would prove the *wrong claim* if it were Caltrain in a second window. A hostile *in-repo* host is the fixture; an external app is the proof. Charlie still owes the name before packaging is called a product (`§5 Q7`).

### M6. 1031 D6 / 1030.000 brownfield are the same object, currently on two calendars — **Material** (Q10)

**Why.** 1031 D6 requires a baked host descriptor, linkage/descriptor agreement, and SPM packaging. 1030.000 r2 **deferred** brownfield, including that descriptor, the embedding SPI, and “rebuild the host app” as a classifier action (`:506–512`), while keeping a `brownfield: true` policy knob (`:214`) that publishes against it. Charlie asked both in one push. Implementing 1030.000 stages 1–4 as written and 1031 D6 as written produces a classifier that cannot see embedders and an SDK that cannot check `E`.

**Resolve.** See §6. The descriptor is a 1031 step-5 / 1030.000-stage-3 joint. Stages 1–2 of 1030.000 do not need it.

### m1. D2’s C sketch is missing the calls D4/D9 need — **Minor**

The sketch (`1031` D2) shows `create`/`destroy`/`in`/`out`/`boot_plan`/`dispatch`. Prose says every existing call takes the handle, including pump, fonts, agent. It does not show `exact_fulfill`, store snapshot, `applyPlan` vs baked `exact_boot`, or `ExactRuntimeConfig` contents (measure, wake, snapshot, transport mode, generation). An implementer filling that in will invent a second ABI. Put the full header in r2.

### m2. `prepare()` (D7) vs “the boot path compiles nothing”

`prepare()` “does not create platform views, run app logic before policy permits it, or compile code.” 1027 D4 already loads Hermes after first pixel; 1009 D2 loads GPU after first pixel. Say `prepare()` is that same slot, callable by the embedder after *their* first pixel, and that it must not move work onto the boot path `scripts/boot.mjs` counts. Otherwise someone will “warm” Hermes in `exact_create`.

### m3. File-size budget on the extract

`Presenter.swift` is already near the 1,500-line cap. Step 2 (ExactSession + ExactView, two `main.swift`s as adapters) must split, not grow, that file. Not a design issue; it will fail `caps` if ignored.

## 4. Suggestions

1. **Replace `host!` with register-at-create**, as in B1. Keep the macro as a one-liner that *registers* `D` and optional baked bytes, so 1029 D2 still reads as one line in the app crate. The C symbols live in ExactCore exactly once.
2. **Default services stay ibex2 + Keychain**; D4’s replacements are a second landing behind the ABI in B2. That lets steps 1–3 of §4 ship without pretending the containing app’s `URLSession` is wired.
3. **Fire commands after apply**, as in B3 — even if you rejected the rest of my reentrancy rule, this one is a one-line presenter change that matches 1008 §5’s “an event arriving while a batch is being applied waits.”
4. **`applyPlan` is the only new session method the dev loop needs.** PlanURL, SSE, the update store, the menu, the 4-finger tap are all callers. This is how 1030.000 stage 4 collapses three loaders into one store without growing an embedding-specific loader.
5. **Measure ExactCore as a link delta against `floor.swift`**, not against stripping Caltrain from `ExactMac`. The RFC already suspects 2.48 MB is a fat proxy; 1008 §6 already has the empty-app floor. Use it. Do not add a script; extend `metrics.mjs --long`.
6. **Name the shared/process table in D2** (B4) even if Charlie answers §6 Q2 as “one session is enough for v1.” Fonts still have to be session-scoped the moment a reload and a live session coexist — which they already do (`Text.checkpoint` / `restore` is the reload form of the same bug).
7. **Do not size-version the ABI in v1.** D2’s “size-versioned when exposed as a distributable SDK” plus “no compatibility promise before 1.0” is two sentences that an implementer will turn into a `struct { uint32_t size; … }` shim. `rules/RULES.md`: no shims before 1.0. Put a comment in the header that the layout is not stable, and stop.

## 5. Open questions

1. **Who is the first consumer?** Only Charlie. Without a name, packaging (D6, §4 steps 5–7) must not accrete SPM machinery, privacy-manifest generators, or a second set of host files. The singleton refactor (steps 1–3) does not need one. Leaning: implement 1–3 and the in-repo fixture; gate 5–7 on a name.
2. **Does v1 require multiple sessions in one process?** Charlie, §6 Q2. Leaning **yes for the ABI and the fixture** (a handle that cannot instance is the thing we are replacing), **no for N products**. What would change my mind: a consumer that will not ship without three simultaneous Exact screens of *different* plans.
3. **Is host-supplied transport in the first package?** Charlie, §5 Q3 of the RFC already leans request/store/command/log. Leaning: **command + log + applyPlan in the first package; transport and store replacement behind the ABI but with ibex2/Keychain as default**, so Weird Castle’s grants keep working the day the handle is instanced. What would change my mind: the named consumer’s client cannot be wrapped and must be the performer on day one — then B2 is on the critical path.
4. **`ExactRuntimeConfig` contents.** The RFC sketches the type and does not fill it. Leaning: measure fn + ctx, wake fn + ctx, snapshot, optional persist fn, optional request-performer-is-host flag, generation. Not a serial queue (RFC §5 Q4 lean is right: caller/main-thread, matching 1008).
5. **Does a brownfield `L = A` host emit the descriptor from *its* Xcode build, or from `exact bake`?** 1030.000 §6 says the host app’s build emits it. Leaning: bake writes a template; the host’s build (a small SwiftPM plugin or a `build.mjs` invocation they opt into) signs the actual linked set (`L`, `E`, roster, grant ceiling, store location). An unsigned descriptor is not identity. Charlie / whoever owns 1030.000 stage 3.
6. **Hermes per session vs per process.** RFC §5 Q5 leans no share of heaps. I agree for v1. The lean VM is 1.81 MB of text (1027 D3); two sessions paying two runtimes is the honest number D7 already wants measured. Engine team can reopen after a measurement.
7. **`panic = "abort"`** is asserted by D8 and not visible in the capsule’s `Cargo.toml` files. Whoever has the workspace manifest should confirm before ExactCore is advertised as abort-on-panic into other people’s processes. If it is *not* abort, B3 is still a panic inside `RefCell` on the main thread — still fatal in practice, slightly less honest in D8.

## 6. Recommended next step

**Revise and stay Draft; do not gather another family; fold B1–B4 and M1–M5 into r2 and implement immediately.** Another family would re-derive the same ABI facts. The document is not ready to accept: ExactCore-as-`host!`, host-performed I/O without an Apple fulfill, reentrancy-abort, and process-global fonts would make the first implementation wrong.

**Land in this order** (Q10), minimizing rework with 1030.000’s four stages:

0. **Fold r2** of 1031: type-erased ExactCore; request/store ABI; reentrancy rule; process-global table; one plan / N sessions; `applyPlan`; bounded-only; in-repo fixture vs external proof. Same day as code if needed — this is transcription of decisions, not a new RFC.

1. **1031 §4 step 1 — instance the ABI** (handle, per-runtime buffers, per-runtime wake, two runtimes in `host/apple/tests`). Standalone still uses one handle. **This precedes 1030.000 stage 4.** If the update store is born on the current `thread_local!` Bridge, it is global and 1031 has to tear it apart later. 1030.000 stages 1–2 (asset row / shader packaging; `app.json` / generated host files for the *standalone* adapter) can run in parallel; they do not touch the C ABI.

2. **1031 §4 step 2 — ExactSession + ExactView**, `main.swift`s become adapters. PlanURL, DevMenu, NSApplication stay in the adapter. Split `Presenter.swift` under the 1,500-line cap. Standalone smoke must stay green through the new handle only (the RFC is right that this is the anti-divergence mechanism).

3. **1031 §4 step 3 — bounded containment only.** Content-height not in this push unless M4’s measurement is already in hand.

4. **1030.000 stage 3 — classifier, compatibility id, `delivery` resource — jointly with the host descriptor** (pulled out of 1030.000 §6). Standalone binaries get a descriptor from bake; brownfield is the same schema. 1031’s `applyPlan` is the reload the classifier’s `{seq}` calls. **1031 step 1 must already be done** so `state.delivery` is per session.

5. **1031 §4 step 4 — inject host services on the B2 ABI**, with ibex2/Keychain as default. Command delegate post-apply. Log sink. Transport/store replacement can ship as SPI here without a first consumer using them.

6. **1030.000 stage 4 — bundle publishing / update store.** Now it can be a client of `applyPlan` and a per-app (not per-process) store. `EXACT_DEV_PLAN` and PlanURL collapse here, as 1026 D12 wanted. Brownfield `L = 0` never links this and still hot-reloads via `applyPlan`.

7. **1031 §4 step 5 — package ExactCore as the type-erased XCFramework**, measure link delta against the in-repo fixture (the `floor.swift` slot). Optional artifacts (step 6) one at a time after that. **Not before** the descriptor exists (step 4 of this list) and **not before** Charlie names a consumer if this is called a product rather than an internal refactor.

8. **In-repo hostile fixture** in the same PR as step 2 or 3, driven by the existing eight operations. External consumer (RFC step 7) when named — not a gate on the singleton refactor.

**Which 1031 steps must precede which 1030.000 stages**

- 1031 step 1 (instance ABI) **before** 1030.000 stage 4 (store), and before any `delivery` state that would otherwise be process-global.
- 1031 step 2 (session/view) **before** claiming standalone is an adapter, and before the fixture.
- 1030.000 stages 1–2 **independent** of 1031; do not let generated `Info.plist` flow into a brownfield host.
- Host descriptor (from 1030.000 §6) **with** 1030.000 stage 3 and **before** 1031 step 5 packaging.
- 1031 `applyPlan` **before** 1030.000 stage 4’s collapse of PlanURL, or PlanURL gets wired to a global again.
- 1031 steps 5–7 (SPM product, optional artifacts, external proof) **after** 1030.000 stages 1–3, and after a named consumer for 7.

I expected D2’s instance handle, D5’s command-as-intention, D8’s honesty about abort, D9’s no-ninth-op, and “baked Rust-only is the smallest shape” to be the parts that might be over-designed. They held. The parts that did not hold are the ones that ignored the `host!` macro, the Apple request path that never reaches Swift, `Text.install`’s process wipe, and the `RefCell` sitting on the only thread UIKit will call you on.


---

## Round 2 — after reading the other two (verbatim)

I'll read the three round-1 answers, the RFC, and the governing rules so the round-2 positions rest on the same documents the panel used.Round-1 answers are in. Next I’ll check the RFC and what’s already on the branch so the forks match code, not the original sketch.I have the three answers and the landed work. Round 2 is only the forks, with positions I would actually implement this week.--- PANEL ANSWER (round 2) ---

## Diffs only

**F1 / D6.** I retract r1 B1’s “type-erase ExactCore this week.” Sol §3 Q1 option 1 and Claude’s Blocking D6 are the build: `lib<app>_apple.a` from `host!`, plus the Swift package and optional engine/module artifacts. v1 is one Exact app per process (F5), so colliding `exact_boot` names are not this week’s problem. Erasing `Host<D>` so a prebuilt core can construct an arbitrary `data::App` is a runner/host refactor with no TypeScript-only consumer assigned — SDK apparatus, not the singleton extraction. Keep from r1 B1: do not generate a brownfield host’s `Info.plist` from Exact `app.json` (stage 2 is the standalone adapter). The trigger for a universal core remains Sol’s compiled `Host<ErasedDataSource>` prototype, or a second Exact app in one process.

**F2.** Disagree with Sol §3 Q9 that a registry is “contrary to the ownership direction.” Claude’s Minor (“the handle can be an integer”) as built is the right object: TLS map of id → one `Bridge`, nothing semantic, session owns the runtime, ids from 1 never reused, destroyed/invented ids refuse by name, same-handle reentry returns `busy` from a buffer no runtime owns, setters so the library never reads a caller config pointer. That is the GPU module’s `u32` shape and it keeps `#![deny(unsafe_code)]`. A `Box::into_raw` pointer still needs a liveness set or it is use-after-destroy — a registry plus an unsafe crate. `rules/RULES.md` adds no apparatus for that.

Extension I take from Sol §3 Q3/Q9, which the integer does not replace: owner-thread stays LLP 1008; `exact_destroy` while busy is refused (same-handle); async completions must not look up the TLS registry on the executor thread. Today’s wake already hops to main and `exact_pump`s. When host-performed I/O exists, completions copy canonical bytes into a thread-safe queue keyed by `(generation, ticket)` (the never-reused id is the generation), and the callback retains that reply port, not a runtime. A pointer on the worker would be worse.

**F3.** Disagree with Sol §3 Q4 / Suggestions that `ExactApplication.load` + `makeSession()` is required now. That object would own the host descriptor (`1030.000` §6, still deferred), the asset resolver (unlanded), and the update store (stage 4). A hollow owner is a shim. Disagree with Claude’s Material dev-loop putting `connect`/`disconnect` on the session: that extracts `PlanURL` onto each session, so two sessions fight `PlanURL.current` or open two SSE connections, and 1026 D12 / 1030.000 stage 4 then delete it. Sol is right that extracting `PlanURL` only to collapse it is waste; wrong that the replacement owner must therefore exist today. r1 M2 stands, narrowed: `applyPlan(bytes, carry:)` is the only new session method this week. Loaders, menu, 4-finger tap stay in the adapter. Sol’s owner is added in the same PR as the update store. Disagree with Sol that raw `boot_plan` is internal SPI this week — that forces every `L = 0` embedder through the store.

**F4.** I take the descoping r1 B2 already named as a mind-changer, against r1 B2’s “add the crossings now.” Sol §3 Q3 showed the crossing is thicker than a batch op: the native client must not auto-follow redirects (an initial-URL grant is not enough), completions are canonical bytes into a concurrent queue keyed by `(generation, ticket)`, callbacks retain a reply port. Claude’s Blocking D4 is right that the grant check must run before anything leaves. Those rules are the contract *when* we build it. We do not build it this week: no named consumer needs their `URLSession` as performer; ibex2 + Keychain already honor grants the day the handle is instanced; a security ABI with no assigned implementer is what `rules/RULES.md` §Scope forbids. Pull D4’s replacements and D10 item 3 from the first release. Ship command (fire after apply — all three), log, `applyPlan`. Do not take Claude’s `ExactRequestFn` as the primary shape: when built, the web’s `{"op":"request"}` after the Rust grant check plus `exact_fulfill` is the seam; a function pointer is only the “host is the performer” switch. Redirect following in today’s ibex2 `NSURLSession` is an ibex question, not a reason to invent the embedding crossing this week.

**F5.** One correction of Claude’s Blocking several-surfaces: a host cannot inject an entry, but the plan’s own state can still select settings vs dashboard, and one session that survives unmount can be remounted in three native containers. “It cannot” is too loud.

**F6.** Disagree with Sol’s 12-step list putting the unsafe FFI crate, `ExactApplication`, and request/store adapters before the asset row, and packaging before the classifier. Disagree with Claude’s “then 1030.000’s stages on top” — stage 2 minus descriptor already landed; shader digest and bake compatibility id are in flight. Disagree with r1 putting type-erased ExactCore packaging and the request ABI on the critical path. Agree with Claude’s Suggestions that the Apple asset resolver must not be born as `EXACT_ASSETS`.

---

## Final positions

**F1.** This week the brownfield app links the app-specific archive (`lib<app>_apple.a` from `host!`), the Swift package, and optional engine/module artifacts — I now agree with Sol §3 Q1 option 1 and Claude’s Blocking D6. I would not type-erase `DataSource` or ship a universal ExactCore until a TypeScript-only consumer exists or a second Exact app must co-link. What would change my mind: that consumer, a second app in one process, or Sol’s erased-`Host` prototype already in hand.

**F2.** The integer handle is acceptable as built; it must not become a pointer. I now agree with Claude’s Minor as implemented. What would change my mind: TLS lookup from the executor thread without a hop (fix the hop, still not a pointer), or a hard requirement to `dlclose` a runtime by address (1024 already forbids `dlclose` in v1).

**F3.** Not required in this push. `applyPlan` on the session plus adapter-owned loaders is enough until the store lands; I still hold r1 M2, and I agree with Claude that the session needs a reload primitive (`applyPlan` / `reload`, not `connect`). Sol’s `ExactApplication` is the right owner *when* it has an archive, a descriptor, an asset resolver, and one connection to hold — that PR, not this one. What would change my mind: the asset row or update store landing in the same PR as session/view extraction; then introduce the owner there so those are never born global.

**F4.** Ship only command, log, and `applyPlan`. Leave transport/store replacement behind the ABI; ibex2/Keychain stay the only performers. I now agree with the conservative half of r1 §5 Q3 / suggestion 2, against r1 B2’s resolve. Adopt Sol §3 Q3’s redirect / reply-port / concurrent-queue rules as the next landing’s contract, not this week’s code. The fixture may drive a granted and a denied fetch through the default executor (the grant check already lives in `executor.rs`); that is not `exact_fulfill`. What would change my mind: Charlie names a consumer whose client cannot be wrapped and must be the performer this week — then build Sol’s full shape, including no auto-follow, in that same PR.

**F5.** I agree with all three: one plan, one `app_id`, N sessions of it in v1; named entries or N plans deferred, trigger = a consumer with a second surface that is not a route of that plan.

**F6.** Order for the rest, given the handle and `app.json`/generated host files already in, and shader digest + bake compatibility id in flight:

1. **Session/view extraction** — `ExactSession` / `ExactView`, both `main.swift`s as adapters, `applyPlan`, commands after apply, split `Presenter.swift` under the cap. Depends on the handle (done).
2. **In-repo fixture** — two sessions of one plan, bounded mount, command, destroy, agent ops. Depends on 1.
3. **Asset row** — kernel row can parallel; Apple resolver is per-session, not `EXACT_ASSETS`. Depends on 1.
4. **Classifier and `exact deploy --dry-run`** — depends on bake compatibility id (in flight), not on 1–3. Parallel with 1–3.
5. **`delivery` resource** — depends on 4; per-session because the handle landed.
6. **Update store and publishing** — collapse PlanURL here. Depends on `applyPlan` (1) and 4–5. `ExactApplication` is born in this PR.
7. **Packaging and the link measurement** — against the fixture / `floor.swift`, optional artifacts one at a time. Depends on 1–2. Product-shaped SPM waits on a named consumer. Does not wait on 8.
8. **Request/store crossings** — Sol’s security ABI, ibex2/Keychain default, fixture grows allowed/denied host-performed. Depends only on the handle; not this week’s extraction.

Two dependencies that are real: **session/view before** the fixture, the Apple asset resolver, the store, and packaging; **`applyPlan` + classifier before** the update store. Shader packaging stays independent. The brownfield host descriptor stays with 6, not with already-landed stage 2.

