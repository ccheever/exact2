# Panel: LLP 1031 Brownfield embedding (claude)

- **Family:** Anthropic: Claude Fable 5.1, this Claude Code session (`claude-fable-5-1`), reading the repository directly.
- **Method:** a panel at Charlie's request, 2026-09-03 — "have a panel discussion about LLP 1031 and try to make it better." Not a refine loop: no verdict binds. The document was authored by a different session (Charlie Cheever / Codex), so this session reviews as one independent family beside grok 4.6 (xhigh) and codex gpt-5.6-sol (ultra), whose artifacts are `llp/reviews/1031-brownfield-embedding.{grok,sol}.md`. Written blind, before either external answer was read; the fold into r2 is by the same session and is recorded in 1031 §8.
- **Redacted:** nothing.
- **Date:** 2026-09-03.

---

## 1. Overall assessment

The direction is right and worth building: an instance-scoped runtime behind an opaque handle, a session that owns one runner and one plan, a view that presents it, and the two standalone apps demoted to adapters over the same surface. The ABI change is cheap now and impossible later, which the document says correctly. I would not build D6 as written, and I would not build D4 or D10 as written either; the rest is transcription of what the code already wants. Three things the document gets wrong come straight from the code: the unit a brownfield app links is the app's archive, not an app-agnostic core; a request today never reaches the presenter, so a host-performed request is a new ABI surface with a grant check that has to move; and the dev loop — the thing this repository exists for — is absent from the embedding API. One thing it leaves undecided that the implementation cannot: what a host with three Exact screens is.

## 2. Strengths

- **D1/D2** name the right unit and the right cut. Everything process-global in `abi.rs` (`thread_local! EXACT_BRIDGE`, `abi.rs:196–201`) and in `main.swift` (`presenter`, `canvases`, `frames`, `agentClock`, `stamps`, `window`) is exactly the list a session must own; the document's list in D1 matches the code's globals one for one.
- **D2's destruction sequence** and the generation token are right, and the executor already has the shape: `Executor::start` hands the wake a `ctx` it never reads (`executor.rs:36–48`), so a session pointer or generation is a one-line change there.
- **D5** — navigation as a command through the existing post-commit path (`Presenter.swift:910–911` `case "command"`), never ownership of a navigation controller — is the only design that survives a real app.
- **D8**'s honesty about process isolation (`panic = "abort"` in `Cargo.toml`) is correct and rare.
- **D9** keeps the eight operations and puts session selection in the carrier. Right, and `Agent.swift` already routes by op, so a `session` label on the carrier's request is routing, not an operation.
- **§3** refuses the right things.

## 3. Concerns

### Blocking — D6's "ExactCore" cannot be app-agnostic under `host!`

**Why.** `exact_apple::host!(caltrain_data::Caltrain, PLAN)` (`apps/caltrain/apple/src/lib.rs:9`) expands to `#[no_mangle] extern "C"` exports monomorphized over the app's `DataSource` type and its baked plan bytes (`abi.rs:203–320`). The runner, kernel, and motion are compiled *into that archive*; there is no separate "core" binary to prebuild, and the C names are fixed, so two different app archives cannot even share a process. D6's "ExactCore — runner, kernel, plan decoder, motion, C ABI" as a package distinct from the app's logic describes something the build does not produce. The sentence "Integrating a prebuilt surface requires Swift Package Manager and no Rust toolchain" is true only for the consumer of an already-built app archive — which is what `build.mjs` produces today — and for exactly one composition, `exact_js::Module` alone (1029 D2), where the archive has no app-specific Rust and the plan and bytecode could be files.

**Resolve.** Rewrite D6 around the **app artifact**: `exact bake` / `build.mjs` produce `lib<app>_apple.a` (runner + kernel + motion + the data crate + the baked plan + the compatibility id) plus the header and a Swift package that wraps it; a brownfield app links that archive and adds the Swift package. Say plainly: one Exact app artifact per host process. Name the app-agnostic prebuilt core as a later packaging for the TypeScript-only composition, not v1, with its trigger.

### Blocking — several surfaces in one host app is undecided, and the implementation must decide

**Why.** A native app that adopts Exact for settings, onboarding, and a dashboard has three surfaces. D1 says a session owns "one decoded/running plan"; `Runner::boot` refuses a plan with more than one root (`runner.rs:471–483` `NotOneRoot`); the envelope names one `plan` (1023 D2; `build.mjs`'s `exact.json`); `app_id` is one per data crate. The document never says whether three surfaces are three plans, one plan with an entry component, or three sessions of one plan.

**Resolve.** For v1: **one plan per app; any number of sessions of it**; a host that wants three different screens writes them as one Contract whose root switches on a slot the host cannot set — which is to say, it cannot, and the limitation is stated loudly. The honest multi-surface shape is N entry `.contract` files sharing `use`d components, baked against one data crate, listed in one envelope under a `plans` map with the default under `plan`, one update stream carrying all of them (the compatibility id is the binary's, not a plan's). Defer that to a later number with the trigger "a consumer with a second surface", and say that nothing in v1 forecloses it: the store, the compatibility id, and the identity gate are all per app already.

### Blocking — D4's host-performed request changes an invariant the code states, and the grant check has to move

**Why.** "A request never reaches the presenter" (LLP 1008 §4; `exact.h:80–83`). Today `executor.rs` runs every request on `ibex2::host` on the library's worker thread and the grant is enforced by ibex2's `Bindings` inside that call (`executor.rs:74–77`, `HostError::Denied`). If the containing app's client performs the request, the request must cross the C ABI *out* (method, URL, headers, body, ticket) and the outcome must cross *in* (`exact_fulfill(rt, ticket, status, headers, body)` / a failure with its kind), and the grant check must happen **before** the request leaves the library or the sentence "every request crosses the one Rust boundary where the grants are checked" (1027) becomes false. Nothing in D4 says where the check goes.

**Resolve.** The bridge checks the request against the data crate's `grants()` (ibex2's `GrantSet::parse` is already linked, `store.rs:14`) before handing it to *either* executor; a refused request never leaves. The default executor stays `executor.rs`; an embedder supplies an `ExactRequestFn` at `exact_create` and the library then hands it granted requests by ticket, and takes outcomes back through one new call. The store is the same shape: the snapshot in at create (the bytes of `snapshot_of`), the writes out as batch ops the host persists — or an `ExactStoreFn` triple. Both keep values on the host's side of the ABI as canonical strings, which is what `store.rs` does with the Keychain today.

### Material — the dev loop for an embedded surface is missing

**Why.** `main.swift:196–233` wires `EXACT_DEV_PLAN` (a file poll, or a URL through `PlanURL.swift`); the dev menu's Open Project… is the affordance a phone uses (`DevMenu.swift`); 1026 D12 and 1030.000 stage 4 collapse those into one update store with two policies. The RFC does not mention any of it. An embedded surface that cannot hot-reload from `dev.mjs` is not Exact.

**Resolve.** `ExactSession.reload(planBytes)` (the transactional swap with carry, `abi.rs` `boot_plan`), `ExactSession.connect(url)` / `disconnect()` (the `PlanURL` session, per runtime), and the update store's policy as session configuration. The dev menu, the 4-finger tap, `EXACT_DEV_PLAN`, and `EXACT_PLAN` stay in the standalone adapters.

### Material — the store across sessions of one app has no rule

**Why.** D1 gives each session "one store namespace"; D4 backs it with the app's storage. Two sessions of one app on Apple both read the Keychain at boot (`store.rs:31–41`) and both write after each commit (`host.rs` `persist`). A login in one is invisible to the other until it reboots, and two writes to one name race.

**Resolve.** State it: the store is **per app**, shared by name across sessions; a write is visible to another session at its next boot or `data_ready`; last write wins; no cross-session notification in v1. The kept answer (`Store::KEPT`, 1027 D4) is per resource name, so it shares the same way.

### Material — reentrancy at the handle

**Why.** The bridge is a `RefCell` in a `thread_local!` (`abi.rs:196–201`); a call while a call is in progress would `borrow_mut` twice and, under `panic = "abort"`, kill the process. The presenter's `apply` delivers `command` ops synchronously (`Presenter.swift:910–911`) while it is mid-batch; a delegate that calls back into the same session from there gets a fresh borrow — today that nests and happens to work. A delegate that calls into the session from inside the measure callback would not.

**Resolve.** The handle refuses a re-entrant call with an error batch (`"error":"busy"`), never traps; commands are delivered after the batch has been applied (the presenter's `waiting` queue already has the shape, `Presenter.swift:844–849`); and the rule is one sentence in D2.

### Material — D2 overstates what can stop being process-global

**Why.** CoreText font registration is process-wide by platform (`Text.swift:152–166`, `CTFontManagerRegisterFontsForURL(.process)`); `dlopen` of the GPU and WebKit arms is process-wide; a Metal device is shared. The catalog *map* (stack id → faces) is per plan and must be per session, since two plans number their stacks independently.

**Resolve.** Say which is which: per session — the catalog, the paragraph cache, the font cache keyed by stack; process-wide by nature — file registration (never unregistered in v1; a second registration of the same URL is already tolerated, `Text.swift:159–164`), the loaded modules, the device.

### Material — D10 refuses the fixture the rules require

**Why.** "A tiny in-repo showcase would prove the opposite of the claim." Under `rules/RULES.md` verification is by running, and `scripts/smoke.mjs` is the drive; nothing outside the repository can hold the embedding contract when the next lane edits `Presenter.swift`. `host/apple/macos/floor.swift` is the precedent: an empty AppKit app kept in-repo because `metrics.mjs` needs a floor.

**Resolve.** Both: a lean sample host in-repo (a native navigation controller, a native table, two sessions — one bounded, one content-height — a command that pushes a native screen, destroy with a request in flight) as the smoke fixture, and an external consumer as the proof. D10's ten points are the fixture's script.

### Minor — D3's content-height exchange needs one guard named

**Why.** `Host::layout` lays out under `Offer::definite(w, h)` (`host.rs:355–357`); content-height needs `Offer` with a max-content height and the resulting root height reported as the view's intrinsic size. Under Auto Layout, an intrinsic-size change relayouts the parent, which may change the view's width, which relayouts Exact.

**Resolve.** Report height only; relayout Exact only when the width actually changed or a commit happened; the loop terminates because width is the parent's decision and height is Exact's. Ship bounded first, content-height in the sample host, and say so.

### Minor — the handle can be an integer

**Why.** `exact-apple` is `#![deny(unsafe_code)]` (`lib.rs:30`); an opaque pointer dereferenced in the exports needs `unsafe` somewhere. The GPU module's ABI already hands out `u32` ids (`native.rs`).

**Resolve.** `ExactRuntime` as a `uint32_t` handle into a thread-local registry: no `unsafe`, and a destroyed handle refuses by name instead of being undefined — D2's late-callback rule for free.

## 4. Suggestions

- **§4's sequence**: instance the ABI first (Rust only, the host tests booting two runtimes), then the Swift extraction with the sample host as the second commit, then services, then packaging and measurement. Fold 1030.000 stage 1 (the asset row) *after* the session exists, so the asset resolver is per session from the start rather than the `EXACT_ASSETS` global (`Presenter.swift:230`).
- **Name the compatibility id here**: a brownfield host's "descriptor" (1030.000 §6) is the app archive's compatibility id plus what the host linked; the archive already knows it after 1030.000 stage 3. Say the descriptor is bake's output, not the host's.
- **Delete D7's table** until the sample host exists; keep the sentence that says what is measured and against what (`floor.swift`).
- Replace the Swift sketch in the Summary with one that shows the archive and the dev loop, so the reader sees what they link and how they iterate.

## 5. Open questions

- Which real native app is the external consumer (Q5)? Charlie's.
- Does the embedded surface's agent socket live in the sample host by default, or only under `EXACT_AGENT=1` as today? Leaning: as today.
- Is the app-agnostic core for TypeScript-only apps wanted at all before a second app exists? Leaning: no.

## 6. Recommended next step

Revise and stay Draft; do not gather another family beyond the two already convened. Land in this order: (1) the handle in Rust with two runtimes under test; (2) `ExactSession` + `ExactView` in one Swift package with the two adapters and the sample host; (3) the dev loop and the store on the session; (4) host services over the ABI; (5) packaging and the measured link delta; then 1030.000's stages on top of the session.
