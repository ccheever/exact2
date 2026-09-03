# LLP 1026: Dynamic delivery — the app over the wire, from a cloud that builds it

**Type:** RFC
**Status:** Draft r2 (amended 2026-09-03 by LLP 1027, accepted: D2–D3's wasm *data* module leaves §9's staging — a phone binary carries one interpreter, chosen by the app's language; §7's "A JS engine" is withdrawn; §10 Q1 is 1027 §10 Q3, Q6 and Q7 are answered by 1027 D6/D5. Everything about plans, assets, the update store, signing, digest identity, and Level A stands.)
**Systems:** Serving (LLP 1023's envelope grows one platform-neutral card and an internet rung; the same envelope is the update manifest), Runner (a second `DataSource` executor: the app's data crate as a wasm module behind a bytes-only ABI), GPU (wgpu moves from the app's module to the host; the app's surfaces compile to wasm against WebGPU imports; shaders as assets), Apple host and Linux host (the executor; the update store, selection at launch, the swap rule; loaders follow redirects and speak HTTPS), Release hosts (the embedded bundle plus an on-disk update store — the update economy, minimal, D9–D12), Web dev loop (`dev.mjs` builds and serves the module and classifies a rebuild by what changed), Build (bake emits the update bundle beside the native archive; the cloud runs the same scripts), Launcher (LLP 1023 Stage 3: grant admission per origin), Native modules (LLP 1024: the roster ships in the binary — the honest boundary)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-02
**Revised:** 2026-09-02 (r2, the same day: Charlie asked for "the best of both worlds — everything compiled in and distributed efficiently up front, but with stuff dynamically swapped in during development or when deploying updates," and then why GPU Rust and native modules could not move. r2 answers both: D9–D12 are the embedded-plus-update model with native-by-digest-identity; D6 moves wgpu to the host so surfaces travel as wasm; D7 is the native-module boundary stated honestly instead of a flat no. §7's refusal of release fetch is withdrawn on his ask; §8 names the trades.)
**Related:** LLP 1023 (one URL per app — the base this extends; §8 is the set of doors this document walks through, one by one, with the reason for each; D1/D2 the URL contract and the envelope, D3 the transactional reload, D6 the registry and its "identity is not trust", D10 one plan is the invariant), LLP 1023.001 (Stage 1 as landed; its incident is D5's motivating case), LLP 1004 D4 (app computation is a Rust data crate — kept: the crate moves, the language does not change), LLP 1005 §3 (canonical value bytes, plan/src/value.rs:117–174 — the ABI's encoding, already declared), LLP 1016 D1/D6 (the runner never does I/O; grants — the properties that make a wire-delivered crate sandboxable, here enforced by the executor rather than by discipline), LLP 1018 D4/D5/D7 (bake sees an empty store; the token stays below the seam; store scoping), LLP 1007 §6 (what a reload carries — amended for a module change) and §7 (a native Rust change is a new binary — narrowed), LLP 1008 §9 + the dev menu (17350d0), LLP 1009 D1/D2/D4/D5 (wgpu is the one GPU API; the GPU module after first pixel; shaders compiled at first use — D6 keeps D1 and repackages D2), LLP 1024 D3/D5/D6 (one app artifact after the paint gate; no `dlclose`; live-swap cut — all kept), LLP 1012 (the agent carrier stays off the network — kept; the cloud has its own eyes), LLP 1015 (the headless Linux host: the cloud's verifier), `rules/RULES.md` §Scope (the boot path executes and compiles nothing; modules ship as bytecode — a wasm module is bytecode) and §Agents, `rules/NOT-DOING.md` §Runtime (hot revision surfaces; the update economy — §8 says what moves and what it costs), the 2026-08-29 ibex2 decision (an engine only at a measured call site, as another `DataSource` after first pixel — this RFC's executor slot is that slot). External precedent: Expo Updates (embedded update, `runtimeVersion`, launch policy, roll-back-to-embedded, code signing — the model D9–D12 imports, with D10 as the improvement a JavaScript runtime cannot make); the WebGPU C header (`webgpu-headers`, implemented by Dawn and wgpu-native) and the wasi-gfx proposal (WebGPU for wasm outside a browser) — D6's import surface has owners; wgpu 29.0.4's `custom` backend (`src/backend/custom.rs`, `dispatch.rs` — the hook D6 uses so surface code stays written against wgpu types). Predecessor, research never authority: exact1 LLP 0524 (native code hot reload: F1 "the portable mechanism is worker replacement", F2 the dylib's real price, F6 "fast native restart is the measured floor"), LLP 0421 (remote and OTA app admission: the eleven retrofit-hostile invariants — D5 inherits five), LLP 0347 (native payload delivery, skew, caching, integrity — D11 is its minimal form), LLP 0331 (`embedded-release`), `docs/hosting-ssr-ota.md` ("your web host is your update server" — kept, as static files), weird-castle `archive/exact1/server/sim-core/wasmtime-host` (a wasm host the predecessor already ran).

## Summary

Charlie's ask, in his words: everything compiled in and distributed
efficiently up front, and then stuff dynamically swapped in during
development or when deploying updates. This document's answer is a
list of *units* — the pieces an app is made of — each with the farthest
place it can travel and the reason it stops there:

| Unit | Today | This RFC | Stops at |
|---|---|---|---|
| The plan and its assets | the LAN, dev hosts (LLP 1023) | the internet; **an update, signed, static** (D9–D12) | nothing |
| The data crate | linked into every binary | **one wasm module, every native host** (D2–D5); native until an update changes it (D10) | nothing: it reaches the phone |
| GPU surfaces | wgpu + app code in one dylib | **the app's half as wasm against WebGPU imports; wgpu in the host** (D6) | per-frame CPU work on iOS (interpreted) |
| Native modules | a dylib in the bundle | the roster ships in the binary; an update changes their *use* (D7) | the platform: they *are* native code |
| The host | the binary | the binary, built by the cloud (D8); it *is* the runtime version | never over the wire (§7) |

**Both worlds, by construction.** One bake produces two outputs from
one source: the native archive the binary embeds (plan bytes, the data
crate and surfaces linked natively, assets, first pixel in budget,
nothing fetched at boot) and the update bundle (the same plan, the same
crate and surfaces as wasm, the same assets, digest-addressed, static
files). A client keeps a store of bundles and picks one at launch; the
embedded bundle is entry zero. **Native by digest identity** (D10):
when the selected bundle's module digest is the embedded one's, the
native code runs; only when an update changed the code does the
interpreter run — until the next binary, which embeds that code
natively again. The interpreted window is the update window and it
closes itself every release. Dev reload and production update are one
mechanism with two policies (D12).

**The boundary is not "Rust or not."** It is what the code needs from
the platform, and whether that need is a bounded, standard import
table the host implements (§3). The data crate needs nothing: a module
with zero imports. A GPU surface needs WebGPU, which is already a
standard import table — the web runs the GPU module exactly that way
today — so the host takes wgpu's role and the surface travels (D6). A
native module *is* platform code; there is nothing to move (D7).

Measured on this Mac (§5): the Caltrain data crate is a **72 KB** module
(32 KB gzip, zero imports); wasmi validates and instantiates it in
**about a millisecond**; its answers are **byte-identical** to the native
crate's on every source and the error path; the interpreter tax is
**8–10×** on calls that cost **1–10 µs** natively; a data-crate edit is a
fresh module in **0.6 s** warm. Caltrain's three surfaces make **8–12 wgpu
calls per frame** (unmeasured under an interpreter). The price is the
interpreter: **+1.0 MB** stripped (wasmi 2.0, `std` + `validate`) in
every binary that can take a code update. That number is the largest
cost here; §10 asks about it first.

This is a proposal with no implementer and no date; it lands only when
both are named (`rules/RULES.md` §Scope). The v1 bar does not move.

## 1. The loop, stated

1. A cloud agent has a checkout of the app and of exact2, edits
   `app.contract`, `data/src/lib.rs`, a surface in `gpu/`, and runs the
   dev loop the repository already has: the resident compiler bakes,
   `dev.mjs` serves `dist/` and the envelope, `build.mjs` produces the
   wasm and the client binaries.
2. It verifies with its own eyes: the headless Linux host (LLP 1015)
   boots the served URL (`host/linux/src/fetch.rs`), and the eight
   operations (LLP 1012) drive it over stdio — `tree`, `tap`,
   `screenshot`. The agent carrier never touches the network; it does
   not need to. The smoke is the verification.
3. The owner opens the same URL: a browser gets the web app; a Mac, a
   phone, or a Linux client gets the envelope, the plan, and the app
   module. An edit is a `{seq}`; the client restarts carrying state, in
   the ~20 ms shape the web has today plus one network hop.
4. When the cloud changes the *host* — kernel, runner, presenter, a
   native module — it builds a new client binary (D8) and the running
   one says so. That is the line.
5. **Deploying** is the same bake uploading its update bundle to a
   static origin under the app's runtime version, signed. Installed
   apps pick it up after their next first pixel and boot it at the
   launch after that (D11). The store submission is the same bake's
   native output, so the embedded bundle is always the latest update at
   build time — no drift, by construction.

Steps 1 and 2 exist. Step 3's plan half exists on the LAN. Step 3's
module half, the internet rung, and step 5 are this document.

## 2. What exact2 already has, and where each wall stands

- **Plans move.** LLP 1023 Stage 1 and 2 landed: envelope, digest,
  atomic swap, last-good, SSE with a `{seq, digest}` hello, `app_id` in
  the header with the boot gate. `PlanURL.swift` does it on macOS and
  iOS; `fetch.rs` boots the Linux host from a URL once per run. The
  transport is ibex2's — platform TLS on Apple, rustls elsewhere — with
  `max_body` enforced during receipt (ac156bee3). Redirects are not
  followed (fetch.rs:11–13). Everything binds the LAN; nothing is a
  cloud origin; nothing authenticates; release binaries never fetch.
- **Native code loads at runtime on Apple.** The GPU module is a dylib
  `dlopen`ed after the first pixel (`host/apple/swift/GpuModule.swift:56`),
  and LLP 1024 gives native modules the same shape. Loading after boot
  is proven; loading from the network is refused (LLP 1023 §8).
- **The seam is bytes-shaped already.** `DataSource` is four methods
  (runner/src/runner.rs:23–79): `query`/`answer` take a source name and
  `&[Value]` and return a `Value` or a `Request`; `parse` takes an
  `Outcome`; `grants` and `app_id` are strings; the `Store` is a
  snapshot of string pairs with writes that ride the commit
  (store.rs:69–158). Values have a canonical byte encoding with a depth
  bound (value.rs:117–174). Nothing in the seam is a pointer, a
  callback, or a handle.
- **The GPU module is wgpu plus app code in one artifact.** 1.7 MB of
  wgpu and naga and a few KB of surfaces, behind a C ABI, handing
  `&wgpu::Device` to `Surface::render` across a Rust boundary (LLP 1009
  D2). On the web the same surfaces run as wasm against the browser's
  WebGPU. wgpu 29.0.4, the pinned version, has a `custom` backend:
  `Instance::from_custom` over the `dispatch.rs` traits.
- **The web already moves the whole program.** `app.wasm` is fetched;
  `{rebuilt}` reloads the page (dev.mjs:144). The web needs nothing
  from this document; it is the oracle for what "the app over the
  wire" means.
- **The wall is the static link.** `caltrain-apple` is one staticlib of
  runner + kernel + data crate + baked plan (apps/caltrain/apple/
  Cargo.toml); the web `host!` macro is monomorphic on one `D`
  (host/web/src/abi.rs:106). A client can run a plan only against the
  crate it links, which is why `{rebuilt}` is session-terminal on native
  (LLP 1023 D3) and why 1023 §8 refuses native code on the wire: a plan
  claiming an app's identity drives that app's crate, which holds its
  Keychain token below the seam (LLP 1018 D5). The Stage 1 incident —
  a weird-castle plan served to a Caltrain binary, booted into a
  data-seam error (LLP 1023.001) — is what that wall protects.

## 3. The units: what each needs from the platform

Ask of each unit: can it be one artifact for every target, can the
platform execute it when downloaded, and *what does it need from the
host*?

- **The plan** needs nothing and is data. Neutral, executable
  everywhere, reaches nothing.
- **The data crate as wasm** needs nothing: the seam hands it values
  and a store snapshot and takes values and requests back. Neutral (one
  `wasm32-unknown-unknown` build, the target the web already uses),
  executable by an interpreter — a JIT cannot run on iOS regardless,
  and an interpreter is what the developer agreement's downloaded-code
  clause and Play's equivalent describe, the clause Expo Updates and
  the predecessor's Hermes bytecode lived under — and it reaches exactly
  what its imports name, which is nothing (D3).
- **A GPU surface** needs WebGPU: a device, a queue, pipelines, buffers,
  a pass. WebGPU is a standard with a C header two implementations
  share and a wasm-side proposal (wasi-gfx). On the web the surface's
  wasm already calls it through imports. A native host can play the
  browser's part: own wgpu, expose WebGPU as the module's imports. What
  stops it today is packaging — wgpu inside the app's dylib, a Rust
  reference across the boundary — not a law (D6).
- **A native module** needs the platform's own views: a `UIView`, MapKit,
  a PTY, the platform's text rendering. Its whole value is being
  native. There is no computation to move, and driving such a view from
  wasm through a per-module import table is building React Native's
  bridge (D7).
- **The host** is the compatibility contract itself (`kernelSchema`,
  `formatVersion`, the ABI number); making it travel makes the client
  version-less (§7).

So the data crate and the surfaces are the app; the plan is its shape;
the host and the native modules are the runtime version. Everything the
app itself is made of travels. exact1 reached the same split from the
other direction (LLP 0524 F1: worker replacement is the portable
mechanism; the dylib swap is desktop acceleration with a real price)
and never built the portable half because its worker spoke a large app
ABI. exact2's seam is four functions over bytes, and WebGPU is someone
else's ABI.

## 4. Design

### D1 — The internet rung: the same URL contract, three additions

LLP 1023 D1/D2 hold unchanged: one URL, two payloads, the envelope as a
pointer card, relative URLs against the final response URL. A cloud
origin needs what a LAN did not:

- **HTTPS.** Already the transport's; the loaders' URL check
  (`fetch.rs::is_url`) accepts it today. ATS on the iOS dev plist is
  satisfied by it rather than exempted.
- **Redirects, bounded.** The loaders follow up to five, same scheme or
  upgrading to `https`, never downgrading, never cross-origin for the
  plan and module (the envelope's pointers are same-origin, 1023 D1);
  the envelope itself may land on the final URL a redirect names —
  that is what "final response URL" was for.
- **A capability URL, not a header, for dev.** The artifact endpoints
  are reached at a path with an unguessable segment the cloud mints per
  project (`https://<host>/p/<token>/`). It is the credential: it can
  be typed into the dev menu's URL field, scanned as a QR, pasted into
  a chat — the affordances a phone actually has — and it survives the
  1023 §8 objection (a bearer in a *plain-HTTP* URL fails its threat
  model; over TLS a capability URL is the ordinary share-link model).
  The plan carries no session (LLP 1018 D4), so a leaked link leaks
  source-shaped data, the same exposure as the LAN default and no
  more. Rotation is minting a new one. Production does not use it:
  production is signed (D11) and the origin is baked in.
- **SSE over the internet** is the same stream; the cloud's reverse
  proxy must not buffer it. That is deployment, not a repo script
  (`rules/RULES.md` §Agents: no new apparatus).

LAN by default stays (1023 D8): a cloud origin is just an origin.

### D2 — The data module: the app's computation as one wasm artifact, every native host

The app's data crate is built a second time — the same source, the same
target the web uses — as a `cdylib` for `wasm32-unknown-unknown` with the
ABI in D3, by the bake beside `app.wasm`, under the `web` profile and
the same `wasm-opt` flags (build.mjs:37). The app gains no crate and no
file: the module crate is generated the way the Apple staticlib's
`build.rs` is, from the data crate's name. One artifact per app;
nothing per platform. (D6 puts the surfaces in the same artifact.)

**The executor** is a wasm interpreter behind a `DataSource` — a new
crate (name open, §10; `exact-wasm` for this text) that depends on
`exact-runner` for the trait and on wasmi, and that a host links or
does not. It is not a cargo feature on `exact-runner`, `exact-apple`, or
`exact-linux` (the optional-capability rule); it is the "another
executor" clause of LLP 1000 taken literally, and it is the slot the
2026-08-29 ibex2 decision reserved for an engine — the same shape, a
different bytecode. Dev clients and the launcher link it; a release
binary links it at Level B (D12) and not at Level A. The web `host!`
stays monomorphic: the browser is the web's executor and the web's
program already travels whole.

**Why an interpreter, and why wasmi.** A JIT cannot run on iOS; an
interpreter is what the downloaded-code clauses permit; and the
interpreter's determinism is the agent API's (LLP 1012): the same
module gives the same bytes on every host (§5 shows it does). wasmi is
pure Rust, no_std-capable, has fuel metering and store limits
(`Config::consume_fuel`, `StoreLimits`) for D5's budgets, and measured
at 1.0 MB with `std` + `validate` only. wasmtime's Pulley interpreter
is the alternative; wasmtime is several times the binary and its JIT
is dead weight on the phone — though on desktop its Cranelift can
compile the *same module* ahead of time, which is how D6's per-frame
tax disappears where the platform allows it. wasm3 is C and would be
the one non-Rust dependency in the host. The choice is one crate's;
the ABI does not know it.

**Placement.** After the first pixel — because a served or updated
app's module is only needed when its bytes differ from the embedded
code's (D10), and a URL-booted app in a dev client or the launcher
arrives after that binary painted its own baked plan (LLP 1023 D6). The
module is instantiated when its bytes are verified (validate +
instantiate ≈ 1 ms, §5), before the served plan's boot, so a resource
with no compiled boot value is answered at boot as today. The boot rule
(`rules/RULES.md` §Scope) is about the host's first pixel and is
untouched; `boot.mjs` counts the web and the web does not change.

**Memory and generations.** A module change drops the old instance —
wasmi instances are values; there is no `dlclose` problem (LLP 1024
D5) and no generation leak (exact1 0524 F2). The instance's linear
memory is 17 pages for Caltrain (§5).

### D3 — The ABI: the seam as bytes, versioned, importless

The module exports, and the executor calls, exactly the trait
(runner.rs:23–79) with every argument and result as bytes in the
module's linear memory:

| Export | In | Out |
|---|---|---|
| `exact_abi() -> u32` | — | the ABI version; mismatch refused by number |
| `alloc(len) -> ptr`, `free(ptr, len)` | — | the module's allocator, the probe's shape |
| `app_id() -> (ptr, len)` | — | UTF-8 |
| `grants() -> (ptr, len)` | — | UTF-8, one grant per line — a *request* (D5) |
| `answer(store, source, args) -> (ptr, len)` | store snapshot bytes; UTF-8; a `Value::List` in canonical bytes | tag `0` + `Value` bytes, tag `1` + `Request` bytes, tag `2` + `DataError` bytes; then the store writes |
| `parse(store, source, args, outcome) -> (ptr, len)` | as above plus `Outcome` bytes | tag `0` + `Value`, tag `2` + `DataError`; then the store writes |

`Value` bytes are the plan's canonical encoding (value.rs:117–174),
decoded with its depth bound. `Request`, `Outcome`, `DataError`, the
store snapshot, and `StoreWrite` get canonical encodings of the same
family — length-prefixed UTF-8 and bytes, tag bytes for enums —
declared in one table the executor crate's `build.rs` generates from,
next to the ABI number. The executor decodes every result with the
same refusal discipline as the plan decoder (LLP 1005 §2): a malformed
result is `DataError::Unavailable` naming the export, never a panic.

**The data half imports nothing.** The probe's module has zero imports
and instantiates against an empty linker. That is the enforcement of
LLP 1016 D1: not "the data crate does no I/O" but "the data crate
*cannot*." `Request`s are how it reaches out — data the host runs under
the grants (D5) — exactly as today. An import outside the two named
import modules (none for data; `webgpu` for surfaces, D6) is refused at
instantiate, by name.

**The trait side.** `exact-wasm` implements `DataSource` for a loaded
module: `query` is `answer` with the store; `answer`/`parse` marshal
and unmarshal; `app_id`/`grants` read once at load. The runner, the
plan, the Contract, and the kernel do not change. A native data crate
and its module answer the same bytes — the equality test in §5 is the
ABI's fixture.

### D4 — Pairing: the envelope names the module; plan and module swap together; resources restart

The envelope (LLP 1023 D2) grows one card:

```json
"module": { "url": "./app.module.wasm", "sha256": "<hex>", "bytes": 71684, "abi": 1 }
```

Platform-neutral — one artifact for every native host — so it does not
reopen the panel's removal of the `gpu` field (1023 §11): the rule was
"never a URL to native code," and this is bytecode for the host's own
executor, the same standing as the plan's bytecode for the runner.
Absent, the app needs no module beyond what the client links; present,
the client fetches both, verifies both, and boots atomically (1023 D3's
transaction, one artifact longer). `seq` covers the pair. The static
`exact.json` carries it — that static form *is* the update manifest
(D9); nothing else is invented for production.

**`{rebuilt}` splits.** `dev.mjs` already knows which files a rebuild
was for (dev.mjs:113–116). A rebuild whose changed files are all under
the app's `data/` or `gpu/` crates is a module change: the server
rebuilds the module (0.6 s warm for the data half, §5), re-hashes, and
pushes `{seq}`. A rebuild that touched kernel, runner, a host, or the
app's `web`/`apple`/`linux`/`modules` crates is what it is today:
`{rebuilt}`, session-terminal on native, "rebuild the native host."
The classification is by path and is honest because the crate graph is
layered (LLP 1000): the data crate depends on `exact-runner`'s types
only, the GPU crate on wgpu and `exact-gpu`'s trait only.

**Carry.** LLP 1007 §6 carries slots, matching resources, the clock,
and the store across a plan reload. A module change carries slots, the
clock, and the store — and **restarts every resource and every
surface**: "reused where its arguments still match" assumes the
function did not change, and the whole point of the edit is that it
did. Compiled boot values in the new plan are the new module's (bake
ran the same source), so the first frame after the swap is honest;
requests go out for the rest as after an action.

**Compatibility.** The pairing under one `seq` from one bake is what
LLP 1023 deferred as "a data-crate compatibility digest": a plan and
the module that bake produced together cannot mismatch. A client that
receives a plan with no `module` card runs it against its linked code
under the `app_id` gate, as today.

**On the web, nothing.** A `{rebuilt}` still reloads the page. Loading
the same module separately on the web, so a code edit becomes a restart
with carry there too ("one module under two loaders"), is a measured
take for a later revision: it costs a second fetch on the boot path the
web budgets hardest.

### D5 — Trust: identity routes, the client pins grants, admission is an act, stores are per origin

LLP 1023 D6's boundary — identity is not trust — gets sharper with code
on the wire, and the seam's shape is what keeps it tractable. Inherited
from exact1 LLP 0421 §7, the invariants that apply (five of eleven):
the agent surface is outside the app boundary; per-publisher data
partitioning; semantic resource budgets; update is capability-
preserving; the recovery path does not share fate with what it
recovers.

- **Grants are pinned by the client, never self-declared.** The
  module's `grants()` is a request. The effective grants are its
  intersection with what the client allows: a release binary or the
  app's own dev client allows exactly its embedded crate's grants — an
  update may never raise them (0421 invariant 10); the launcher shows
  the requested grants on first connect and the user admits them per
  origin, pinned, and a revision that asks for more re-prompts.
  `endow(data.grants())` (host/apple/src/abi.rs:148) is the one call
  site; it takes the effective set.
- **Stores are scoped by (`app_id`, origin).** A cloud-served app never
  reads the bundled app's Keychain entries unless the client *is* that
  app — its release binary or its own dev build — 1023 D6's "same-app
  dev reload keeps its own store," narrowed to same `app_id` *and* the
  client's embedded crate is that app. The launcher's memory-store
  default and per-connection durable opt-in stand (1023 D6).
- **Budgets.** Fuel per call and a linear-memory ceiling
  (`StoreLimits`) — a module that loops or grows past them answers
  `DataError::Unavailable("budget")`, the resource keeps its last
  value, `logs` is loud; a surface that exceeds its frame budget is
  skipped for that frame and reported. Semantic budgets (0421 invariant
  8) at the seam, where a runaway crate is a single call. The numbers
  are set on landing from §5's floor with an order of magnitude of
  headroom.
- **The interpreter is the sandbox.** No imports for the data half, one
  named import module for the surface half (D6); the host's own
  executor for `Request`s applies the pinned grants as it does today,
  before any transport sees them (LLP 1016 D6).
- **Refusals name things.** ABI number, digest, signature, `app_id`, a
  grant outside the pinned set, an import, a budget — each is a `logs`
  line with the name and the number, and the last good app stays on
  screen.
- **Production adds a key** (D11); dev does not (D1).

### D6 — GPU surfaces as wasm: wgpu moves to the host

LLP 1009 D1 stands — wgpu is the one GPU API, the `Surface` trait is
unchanged, the browser is the oracle. D2's *packaging* changes:

- **The host owns wgpu.** `libexact_gpu.dylib` becomes a host artifact:
  wgpu, naga, the C ABI it has today, plus the WebGPU import shim for
  the executor. Still bundled, still loaded on demand after first
  pixel (1009 D4), still absent from apps without a canvas. Its size
  does not move — it was 1.7 MB of wgpu and a few KB of app code.
- **The app's surfaces compile to wasm32 against WebGPU imports.** The
  GPU crate builds twice from one source, like the data crate: natively
  against real wgpu for the embedded archive, and for wasm32 against a
  thin wgpu **custom backend** (wgpu 29's `custom` feature,
  `Instance::from_custom` over `dispatch.rs`) whose implementation is
  `extern "C"` imports in one named import module, `webgpu`, shaped by
  the standard WebGPU C header — the surface Dawn and wgpu-native both
  implement, so the import table has owners other than us. The
  surface code does not change; `render(&mut self, frame, device,
  queue, target)` is the same function. wgpu objects still never cross
  the boundary (1009 D2): the module holds fixed-width handles, the
  host holds the objects. Textures and buffers live host-side, which
  is where D5's budgets want them.
- **On the web, this is already true.** The GPU module's wasm calls the
  browser's WebGPU through imports. The native host takes the browser's
  role and the readback fixture (1009 D1) compares *identical bytes*
  under two executors — the parity instrument gets stronger.
- **Both surfaces halves live in the one app module** (D2): no imports
  for the data half, `webgpu` for the surface half, one artifact, one
  digest, one card (D4). The surface roster — name and arity, which the
  compiler checks against (1009 D2) — is read from the module at load
  as `app_id` and `grants` are.
- **Shaders are assets.** A surface's WGSL is text the driver compiles
  at first use (LLP 1009 D5 asks NOT-DOING to say so). Read from the
  asset path at `bind` instead of the crate's source, a shader edit
  rides the asset rung (D11) and needs no module rebuild at all.

**The honest cost is per-frame CPU work under an interpreter.**
Caltrain's three surfaces encode 8–12 wgpu calls per frame (§5); at
the data module's measured per-call tax that is tens of microseconds,
invisible. A surface that generates geometry on the CPU every frame —
`stack.rs` builds six vertices per card per frame — pays 8–10× on that
work, and on iOS there is no escape from the interpreter. Three things
bound it: **D10** — a freshly installed or store-updated app always
runs its surfaces natively, so only the update window is interpreted;
**ahead-of-time on desktop** — the same module compiled by Cranelift
where the platform allows, at no change to the artifact; and **a
surface may simply wait for the next binary**, which the frame-budget
refusal in D5 makes a visible choice rather than a stutter. Unmeasured
(§5): the per-frame tax on the three real surfaces, which the landing
measures before D6 ships.

**The dylib rung** — a dev-capable desktop host fetching a rebuilt
native dylib — is now only about native modules (D7) and stays
deferred for exact1 0524 F6's reason: fast native restart is the
measured floor, and the cloud rebuilding the host and the client
relaunching (D8) has not been measured against it. iOS never: AMFI.

### D7 — Native modules: the roster ships in the binary

A native module (LLP 1024) is a platform view with its own megabytes —
a terminal, a map — and its value is exactly that it is the platform's
code: `UIView`s, MapKit, a PTY, CoreText. There is no computation to
move into a wasm module; what a wasm "controller" could do is drive
such a view through a per-module import table, and designing that
table per module kind is building React Native's bridge, which this
repository exists to not do (`rules/NOT-DOING.md` §Authoring models).
So:

- The tag → factory roster is frozen at `codesign` (1024 D3) and is
  part of the **runtime version** (D9): bake lints a plan's module tags
  against the roster of the binaries it targets, as it lints them
  against the app's own crate today (`bake-unknown-module`).
- An update changes how the plan *uses* a module — props, placement,
  handlers — never the module. A new module is a new binary. This is
  the same boundary Expo draws at native modules, and the same refusal
  machinery (1023 D10) names it.
- Desktop dev hosts may one day fetch a rebuilt module dylib (D6's last
  paragraph); iOS never.

### D8 — Three client shapes, and the version problem

**The dev client** — the app's own native host, built by the cloud for
each surface with the scripts that exist: `host/apple/build.mjs` (the
team identity for the phone: TestFlight or ad hoc; a `.app` download
for the Mac), `cargo build --release -p <app>-linux` for fleet. Installed
once; thereafter D1–D6 stream. The kernel version is the cloud's by
construction. This is the Expo dev-client shape and the primary one for
a cloud agent, because the agent controls both ends. It works today on
macOS and Linux minus the internet rung.

**The release binary** — the same host with the embedded bundle and an
update store (D9), checking its baked-in origin under D12's production
policy. Level A links no interpreter; Level B links it.

**The launcher** — LLP 1023 Stage 3, one binary, many apps — gains D5's
per-origin admission and is the phone's convenience. It runs plans of
*its* kernel schema and modules of *its* ABI number, and refuses others
naming both versions: Expo Go's version-skew problem, restated, with
the refusal machinery 1023 D10 already inherits. The cloud pins exact2
to the launcher's version or the launcher updates; nothing negotiates.

**Windows and Android** stay on NOT-DOING; nothing here forecloses them.
The module is neutral, and an interpreter is the shape Play's
downloaded-code exception is written around too.

### D9 — One bake, two outputs; the update store; the runtime version

**Bake emits both.** From one source, one run of `contract::bake`
produces the native archive the binary embeds — the plan as
`include_bytes!`, the data crate and surfaces linked natively, the
assets — and the **update bundle**: `exact.json` (the static envelope
of 1023 D2 with the `module` card, `assets` by digest, and D11's
signature), `app.plan`, `app.module.wasm`, and the assets, every file
digest-addressed. The two describe the same app at the same digests;
the binary embeds the update bundle's `exact.json` beside the archive
so the client knows, by digest, what it already has.

**The update store** is a directory in the app's container: bundles
keyed by the envelope's digest, each whole or absent (written to a
temporary name and renamed — the atomic-swap rule of 1023 D3 on disk),
plus one small record naming the selected bundle, the last good one,
and a failure count. Entry zero is the embedded bundle and is never
deleted. The dev loop's transactional swap and the file-poll locator
`EXACT_DEV_PLAN` are this store with a different policy (D12).

**Selection at launch** reads the record, boots the selected bundle's
plan (from the embedded bytes when it is entry zero, else a 12 KB file
read and the same validating decode), and runs the code D10 chooses. No
network on this path, ever: the check for a newer bundle is after first
pixel (D11). Boot stays inside its budget because selection is a stat
and a read.

**The runtime version** is what a bundle must match to be selected:
the kernel schema digest, `formatVersion`, the module ABI number, and
the native-module roster digest (D7). It is baked into the binary and
named in the envelope; bake lints against it; a bundle for another
runtime version is refused by name before download (the `kernelSchema`
pre-check LLP 1023 Stage 1 left owed). Bake writes it; nothing is
hand-declared. An app that changes its kernel — a new exact2 — ships a
new binary and its updates move to the new version's directory; the
old binaries keep receiving the old version's bundles until the
developer stops publishing them. Expo's `runtimeVersion`, derived
instead of authored.

### D10 — Native by digest identity

The binary knows the digest of the module built from its own source —
bake computed it (D9). When the selected bundle's `module` digest
equals the embedded one, the client runs **the linked native code**:
the native data crate, the native surfaces. Only when an update changed
the code does the executor instantiate the module — and only until the
next binary, which embeds that code natively again and whose bundle's
digest matches once more.

| Client state | The data seam | Surfaces |
|---|---|---|
| Fresh install, or after a store update | native | native |
| Plan-only or asset-only update | native — digest unchanged | native |
| Update that changed the crate or a surface | the module, interpreted (AOT on desktop) | the module |
| Bundle outside the runtime version | refused; the selected bundle stays | — |

The rule is a byte identity, not a version string: it cannot be
fooled by a mislabeled build, and it needs no negotiation. The
interpreter tax exists only in the update window, and every release
closes the window. This is the improvement a JavaScript runtime cannot
make — Expo runs JavaScript forever — and it is why "everything
compiled in" and "swapped in when deploying updates" are not in
tension.

### D11 — The update economy, minimal

This is the door `rules/NOT-DOING.md` §Runtime and LLP 1023 §8 keep
closed, opened on Charlie's ask (2026-09-02). exact1 LLP 0347 is the
long form; this is the least that is honest, and each item is stated
because it is retrofit-hostile:

- **Signing.** The developer's public key is baked into the binary
  (bake writes it beside the runtime version); the envelope carries an
  Ed25519 signature over its canonical bytes, which name every other
  file's digest. The CDN is untrusted. A bundle whose signature does
  not verify is not written to the store. Dev bundles (D1) are
  unsigned and are refused by a release binary for that reason alone.
  Key rotation is a new binary.
- **Anti-rollback.** `seq` is monotonic per runtime version; the client
  refuses a bundle whose `seq` is below the selected one's. A
  deliberate rollback is a *new* `seq` whose content is an older
  bundle's — Expo's roll-back-to-embedded directive is `seq + 1`
  pointing at entry zero.
- **Crash recovery.** A bundle that fails to reach first pixel twice
  in a row is marked bad and the last good bundle is selected, entry
  zero at worst; the failure is in `logs` and in the next envelope
  request's headers so the server can see it. exact2 needs this less
  than Expo does — a bad plan fails closed at decode and a bad module
  is budgeted and importless, so most failures are refusals, not
  crashes — but the counter is the backstop that must not share fate
  with what it recovers (0421 invariant 11).
- **Assets by digest.** The asset rung LLP 1023 D4 deferred, landed
  here: the envelope lists each asset by name and digest; the store
  fetches what it lacks and reuses what it has, embedded assets
  included. Fonts (LLP 1019) and shaders (D6) ride it.
- **Grants cannot rise** above the embedded crate's (D5). Neither can
  the roster (D7).
- **The check is off the boot path.** After first pixel, the client
  fetches the envelope from its baked-in origin, verifies, downloads
  what is new, writes the bundle whole, and **selects it for the next
  launch**. Applying live with state carried (the dev loop's shape) is
  a `Command` the app may issue when it decides the moment is right;
  the default never restarts a running app under its user.
- **Offline** is the store: the selected bundle is on disk; a failed
  check changes nothing.
- **The server is static files** — a directory per runtime version, the
  same `exact.json` bake wrote, at a CDN. No server generation
  (1023 §8, kept), no per-request work, no per-user targeting.

### D12 — One mechanism, two policies; Level A and Level B

Dev reload and production update are the **same store and the same
selection** with two policies:

| | Dev | Production |
|---|---|---|
| Origin | a typed or scanned capability URL (D1) | baked in at bake |
| Trust | the origin, over TLS | the signature (D11) |
| Arrival | SSE `{seq}`; the stream's hello | a check after first pixel |
| Apply | now, carrying state (LLP 1007 §6, D4) | next launch, unless the app asks |
| Store | the app's, or memory in the launcher (D5) | the app's |
| Grants | the client's pinned set (D5) | the embedded crate's |

What this collapses: `EXACT_DEV_PLAN`'s file poll, the separate URL
loaders in `PlanURL.swift` and `fetch.rs`, and the whole "network
loading is compiled only into dev-capable hosts" distinction become
one store with a policy — fewer code paths than today, not more
(§8's argument).

**Level A — plan and assets.** The store, selection, signing,
anti-rollback, crash recovery, assets by digest, the runtime version.
No interpreter in the release binary, no `module` card consumed; a
bundle whose plan needs code the embedded crate lacks is refused by
bake before it is ever published (the roster and the source names are
bake's to check). Level A covers every Contract-only change — most of
them — at zero binary cost, and it is most of the machinery.

**Level B — plus the app module.** The release binary links `exact-wasm`
(+1.0 MB) and the WebGPU import shim; the `module` card is consumed;
D10 applies. The interpreted window opens only for code updates and
closes at the next binary.

The developer chooses per app at bake. The cloud agent's loop (§1) is
Level B in the dev client regardless: the dev client is not a release.

### D13 — The cloud side is the dev loop, hosted

Nothing is invented server-side. `dev.mjs` (the resident compiler, the
watch, the envelope, SSE, `--loopback`) runs behind a TLS reverse proxy
at the capability path; bake emits the module and the update bundle
(D2, D9); `build.mjs` emits the client binaries (D8); the headless
Linux host is the agent's verifier (§1). Deploying an update is bake
plus an upload of static files, signed with a key the cloud holds for
the project — a thing an agent can do. `dev.mjs` learns three things:
build and hash the module, serve it with its MIME type, and classify a
rebuild (D4). `metrics.mjs` gains three diagnostic rows — cold
URL→first-frame and hot `{seq}`→first-frame over the internet, and
launch-with-a-downloaded-bundle vs embedded — never a sixth check. The
proxy and its certificate are deployment: no script, no config file in
this repo.

## 5. Measured, 2026-09-02

A scratch workspace outside the repo (the probe: the Caltrain data crate
behind D3's `query` half as a `cdylib` for `wasm32-unknown-unknown`,
`web` profile; a host binary on wasmi 2.0.0 that loads it, calls it,
and compares every answer to the native crate's bytes). Apple M5 Max,
macOS 26.6.2, rustc stable, `wasm-opt` with `build.mjs`'s flags. The
source is §11, verbatim; `scripts/probe/` if this lands, as LLP 1009 §3
did.

| | |
|---|---|
| Module, `web` profile, before `wasm-opt` | 79,020 B |
| after `wasm-opt -Oz` (the web build's flags) | **71,684 B** · 32,348 B gzip |
| Imports | **none** |
| Validate + compile | 0.3–0.9 ms |
| Instantiate | 0.1–0.2 ms · 17 pages (1.1 MB) |
| `board` (a 3 KB answer) | native 10.4 µs · wasmi 95–106 µs |
| `stations` | 1.3 µs · 12–13 µs |
| `search` | 0.4 µs · 7–8 µs |
| Unknown source (the error path) | — · 1.8 µs |
| Answers vs the native crate | **byte-identical**, all four |
| wasmi in a stripped release binary | +1.66 MB (default features) · **+1.0 MB** (`std` + `validate`) |
| Data-crate edit → module rebuilt, warm | **0.6 s** |

**Counted, not measured — the surfaces.** wgpu calls per `render`, from
the source: `aurora.rs` 8, `glass.rs` 12, `stack.rs` 8 (one
`write_buffer`, one pass, one or two draws each). At the data module's
per-call tax that is tens of microseconds per frame. `stack.rs` also
builds six vertices per card per frame on the CPU — the kind of work
D6 says to measure under the interpreter before D6 ships.

What the numbers say: the module is a fraction of the app wasm (172
KiB gzip, LLP 1007 §7); instantiation is below the noise of one network
round trip; the interpreter tax lands on calls measured in
microseconds, so a page's settlement stays well under a millisecond;
and a code edit reaches a client in the plan's own loop shape — 0.6 s of
build plus the hop — instead of a native rebuild, install, and relaunch.
The one number that argues back is the binary: 1.0 MB on a 2.3 MB iOS
host (LLP 1024 §2), and at Level B that is every install, not only the
dev client.

## 6. Costs and budgets

- **Binary:** +1.0 MB in dev clients and the launcher; +1.0 MB in
  release at Level B only; none at Level A; none on the web. The host's
  GPU dylib does not grow (D6). The largest cost here; §10 first
  question.
- **Build:** a second wasm32 compile of the data and GPU crates per
  edit, 0.6 s warm for the data half, in the cloud. The five checks
  gain nothing; the executor's and the store's tests are ordinary
  `cargo test`.
- **Boot:** the host's first pixel is untouched; selection is a stat
  and a read; a bundle with changed code adds ≈1 ms of instantiation.
- **Runtime:** 8–10× on data-crate calls that cost 1–10 µs natively and
  on per-frame surface CPU work, only in the update window (D10),
  AOT-compiled away on desktop. A crate or surface that does real
  computation waits for the binary, visibly (D5).
- **Surface:** one new crate (the executor), one envelope card, the
  update store and its record, signing and verification, assets by
  digest, wgpu repackaged into the host's dylib with an import shim,
  a `dev.mjs` classification, a carry rule. No plan, kernel, Contract,
  or runner-core change. The ABI table is the one new declaration
  authority, generated like the others. What is *removed*: the file
  poll, two dev-only loaders, the dev/release loading distinction.
- **Time budgets (RULES):** unchanged for the embedded app. Three
  diagnostic rows for the rest (D13).

## 7. What this refuses, with the trigger to revisit

- **The kernel and runner over the wire** — a native "browser" that
  runs the whole exact program as wasm behind a native presenter. It
  would make one client run any exact2 version, the far end of
  "dynamic." Refused: the interpreter tax would land on layout every
  frame rather than on a query; the kernel *is* the runtime version,
  and a version-less client reintroduces the N-version negotiation
  exact1 carried; the desktop already has the native form. Trigger: a
  measured need for one client across versions that the dev-client and
  launcher shapes (D8) cannot meet.
- **Native modules over the air** — D7's reason: they are platform
  code. Trigger: Apple ships a supported in-app native plugin API
  (1024 §5's own trigger).
- **A service.** Per-user targeting, staged rollouts, A/B, an update
  console, server-side anything. Static files per runtime version, and
  no more. This is the guard against the road exact1 took; the trigger
  is a product decision, not an engineering one.
- **In-process live swap.** LLP 1024 D6 stands; a module change is a
  restart with carry.
- **The data crate on the server.** 1023 §8's server-proxied seam is
  still no: the store is on the device (LLP 1018 D5).
- ~~**A JS engine.**~~ Withdrawn 2026-09-03: LLP 1027 puts the lean
  Hermes VM in this executor slot for TypeScript logic; the wasm data
  module (D2–D3) is not planned, so that one slot holds one engine.
- **The agent carrier on the network.** LLP 1012. The cloud has a Linux
  host.
- **The dylib rung**, deferred per D6.
- **Withdrawn in r2:** r1's refusal of release binaries that fetch —
  replaced by D9–D12 on Charlie's ask, at the cost §8 names.

## 8. The NOT-DOING and 1023 §8 trades this RFC owes (Charlie's, before Acceptance)

The rule: name what it unblocks, and take something off the doing-list
in the same PR.

- **`rules/NOT-DOING.md` §Runtime "Snapback / update economy"** →
  D9–D12, minimal and static. Unblocks: deploying an app change to
  installed apps without a store release, from a bake and an upload —
  Charlie's "when deploying updates." **The take, in the same PR, is
  apparatus that already exists:** `EXACT_DEV_PLAN`'s file poll and the
  two dev-only URL loaders collapse into the one update store with two
  policies (D12); the "network loading only in dev-capable hosts"
  distinction is deleted rather than maintained. Fewer paths after
  than before. And the guard is written into §7: no service.
- **1023 §8 "No native executables over the wire"** narrows to: *no
  native code to iOS; native dylibs, if ever, only to a dev-capable
  desktop host, and only for native modules; the app's computation and
  surfaces travel as one interpreted, budgeted wasm module whose only
  imports are WebGPU's.* Unblocks: a data-crate or surface edit
  reaching every native client without a rebuild — the cloud loop's
  Rust half. **Proposed take:** LLP 1023 Stage 3's system DNS-SD
  advertisement and Apple browse. A cloud URL is typed or scanned;
  1023 D7 itself records that mDNS "fails on guest and corporate
  networks constantly." The launcher keeps the URL field and gains a
  QR; discovery leaves.
- **1023 §8 "release binaries never fetch"** → they fetch after first
  pixel, from a baked-in origin, signed, into a store, for the next
  launch (D11). Take: covered by the first bullet.
- **1023 §8 "No auth on the artifact endpoints in v1"** → a capability
  URL over HTTPS for dev (D1), a signature for production (D11). Take:
  none needed — it removes a limitation; `--loopback` stays the
  embargo switch.
- **`rules/NOT-DOING.md` §Runtime "HBC compilation, hot revision
  surfaces, staged reload"** — not moved. A module change is not a hot
  revision surface: no patch format, no generations, a restart carrying
  state exactly as the parenthetical Charlie ruled (LLP 1007 §6,
  2026-08-28). This document asks the parenthetical to say "a plan or
  module reload."
- **`rules/NOT-DOING.md` §Runtime "GPU / WebGPU substrate … compiles no
  shaders at runtime"** — LLP 1009 §5's amendment, which D6 rides.
- **LLP 1009 D2 "the GPU is a separate module per app"** → "wgpu is a
  host module; the app's surfaces are in the app module" (D6). The
  trait, the roster, the readback fixture, and the after-first-pixel
  rule are unchanged; only the packaging moves.
- **LLP 1007 §7 "a native Rust change is a new binary"** → "a native
  Rust change *outside the app module* is a new binary."

## 9. Staging

Each stage ships alone. **No implementer is named; nothing here is
scheduled.** Written because Charlie asked for the exploration
(2026-09-02), not because it is being built.

1. **The internet rung** (D1, D13). HTTPS and bounded redirects in both
   loaders (`PlanURL.swift`, `fetch.rs`); the capability path is the
   server's business; the Linux display loop's live SSE half (already a
   QUEUE leftover). Verified by driving it: a fleet builder serving to
   this Mac and to a phone over the internet, the agent carrier
   unreachable, a rotated link refused.
2. **Level A** (D9, D11, D12). The update store and selection; bake's
   two outputs and the runtime version; signing and verification;
   anti-rollback; crash recovery; assets by digest; the dev policy
   folded onto the store so the file poll and the two loaders go. No
   interpreter yet. Verified by driving it: a Caltrain release build
   picks up a plan-and-image update from a static directory at the
   next launch; a bad signature never touches disk; a bundle that fails
   to paint twice falls back to embedded; a downgrade is refused; the
   dev client still reloads in ~20 ms through the same store.
3. ~~**The data module**~~ — *removed from staging 2026-09-03 (LLP 1027
   §8's take: over-the-air logic is TypeScript bytecode; Rust logic is
   Level A). D4's pairing and D5's admission survive for 1027's module;
   D6 keeps its own trigger.* (D2–D5, D10) in the dev client and the
   launcher first: the generated module crate in bake; the executor
   crate with its ABI table; the `module` card; `dev.mjs`
   classification; the carry rule; digest identity. Fixture: Caltrain's
   own crate as the module, the byte-equality test as the ABI's
   fixture. Verified by driving it: edit `board` in the cloud, `{seq}`,
   the phone shows it with no rebuild; a module with an import refused
   by name; a looping module refused by budget with the last value on
   screen; a weird-castle module into a Caltrain dev client refused by
   `app_id`; the Stage 1 incident replayed a third time. Then Level B:
   the same executor in a release build, a signed code update, native
   again after the next build.
4. **Surfaces as wasm** (D6). wgpu into the host's dylib with the
   `webgpu` import shim; the custom backend; the GPU crate's second
   build; the surface half of the module; the readback fixture under
   both executors; the per-frame tax on the three real surfaces
   measured first and recorded here.
5. **The launcher with admission** (D5, D8): 1023 Stage 3 minus DNS-SD,
   plus per-origin grant admission and the QR.

Deferred, each with its trigger in the text: the dylib rung for native
modules (D6/D7), the web loading the module separately (D4), key
rotation without a binary (D11), ahead-of-time compilation of the
module on desktop (D2/D6 — an engine choice, made when the per-frame
number asks for it).

## 10. Open questions (for Charlie)

1. **Is 1.0 MB acceptable** in the iOS dev client and the launcher, and
   is it acceptable in *release* at Level B? Level A exists so the
   answer can be "dev and launcher yes, release not yet." The
   alternatives are a smaller interpreter (wasm3, C) or no phone half.
2. **Apply policy in production:** next launch by default with an
   app-issued `Command` to apply now (D11), or apply-on-background as
   Expo's default? Leaning: next launch; a restart under the user's
   hands is the one thing this design must never do on its own.
3. **Does the launcher ever admit `secret.keep`?** A cloud-served app
   asking for the Keychain is the one grant whose misuse outlives the
   session. Leaning: never in the launcher; only in the app's own dev
   client and its release binary.
4. **The WebGPU import surface:** the standard C header verbatim (owned
   elsewhere, wide) or the subset the `custom` backend's traits
   actually need (narrower, ours to keep in step)? Leaning: the header,
   trimmed to what wgpu's dispatch traits call, generated from a table
   as the other ABIs are.
5. **Capability URL or a pairing code for the phone** in dev? The URL is
   the simplest thing that survives the threat model stated; a
   six-digit pairing code trades a scan for a keyboard.
6. **Should the web load the module separately** (D4), making the app
   module one artifact under two loaders and `{rebuilt}` a restart
   with carry on the web too? It is the web's boot path; it needs a
   number first.
7. **Should the cloud's bake run the module** through the same
   interpreter, so bake's boot values are provably the module's? The
   native crate and the module answer the same bytes today, so it may
   be ceremony.
8. **The executor crate's name**, and whether the ABI table lives in it
   or beside `format.json`.

## Ratification note

Draft r2, 2026-09-02, unreviewed. r1 was written after the probe in §5
ran; r2 folds Charlie's two asks the same day — the embedded-plus-update
model (D9–D12) and the GPU and native-module boundary restated (D6,
D7) — and withdraws r1's refusal of release fetch at the cost §8 names.
The numbers are this Mac's; the surface numbers are counted from
source, not measured. Charlie may say which doors in §8 open, ask for a
panel, or park it. Nothing in the repo changes until an implementer and
a date are named.

## 11. Appendix — the probe, verbatim

Outside the repo, so it adds no apparatus; here so §5 can be re-run.
A workspace of two crates, `[patch.crates-io] taffy` pointed at
`vendor/taffy` and exact2's `release`/`web` profiles copied (the
weird-castle shape). `datamod` is a `cdylib` depending on
`caltrain-data`, `exact-runner`, `exact-plan`; `hostprobe` is a bin
depending on the same plus `wasmi = { version = "2.0.0",
default-features = false, features = ["std", "validate"] }`. Build:
`cargo build -p datamod --target wasm32-unknown-unknown --profile web`,
`cargo build --release -p hostprobe`, then `wasm-opt -Oz
--enable-bulk-memory --enable-nontrapping-float-to-int` on the module.

`datamod/src/lib.rs`:

```rust
//! Probe: the Caltrain data crate behind a bytes-only ABI, built for
//! wasm32-unknown-unknown. `query(src, args) -> result` where args is a
//! canonical `Value::List` and the result is tag 0 + value bytes, or tag 1
//! + UTF-8 message.
use std::cell::RefCell;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

thread_local! {
    static SOURCE: RefCell<caltrain_data::Caltrain> = RefCell::new(caltrain_data::Caltrain);
}

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut v: Vec<u8> = Vec::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as u32
}

#[no_mangle]
pub extern "C" fn free(ptr: u32, len: u32) {
    unsafe { drop(Vec::from_raw_parts(ptr as *mut u8, 0, len as usize)) }
}

fn pack(v: Vec<u8>) -> u64 {
    let len = v.len() as u64;
    let p = v.as_ptr() as u64;
    std::mem::forget(v);
    (p << 32) | len
}

#[no_mangle]
pub extern "C" fn query(src_ptr: u32, src_len: u32, args_ptr: u32, args_len: u32) -> u64 {
    let src = unsafe { std::slice::from_raw_parts(src_ptr as *const u8, src_len as usize) };
    let args = unsafe { std::slice::from_raw_parts(args_ptr as *const u8, args_len as usize) };
    let out = (|| -> Result<Value, DataError> {
        let src = std::str::from_utf8(src).map_err(|_| DataError::BadArguments("source".into()))?;
        let args = match Value::from_bytes(args) {
            Ok(Value::List(items)) => items,
            _ => return Err(DataError::BadArguments("args".into())),
        };
        SOURCE.with(|s| s.borrow_mut().query(src, &args))
    })();
    let mut bytes = Vec::new();
    match out {
        Ok(v) => { bytes.push(0); bytes.extend(v.to_bytes()); }
        Err(e) => { bytes.push(1); bytes.extend(format!("{e:?}").into_bytes()); }
    }
    pack(bytes)
}
```

`hostprobe/src/main.rs`:

```rust
//! Probe host: load the data module under wasmi, call it, time it, and
//! check its answers are byte-identical to the native crate's.
use exact_plan::Value;
use exact_runner::DataSource;
use std::time::Instant;
use wasmi::{Engine, Linker, Module, Store};

fn main() {
    let path = std::env::args().nth(1).expect("wasm path");
    let bytes = std::fs::read(&path).unwrap();
    println!("module: {} bytes", bytes.len());

    let t = Instant::now();
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes[..]).unwrap();
    let t_compile = t.elapsed();
    let t = Instant::now();
    let mut store = Store::new(&engine, ());
    let linker = <Linker<()>>::new(&engine);
    let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
    let t_inst = t.elapsed();
    let memory = instance.get_memory(&store, "memory").unwrap();
    let alloc = instance.get_typed_func::<u32, u32>(&store, "alloc").unwrap();
    let free = instance.get_typed_func::<(u32, u32), ()>(&store, "free").unwrap();
    let query = instance.get_typed_func::<(u32, u32, u32, u32), u64>(&store, "query").unwrap();
    println!("validate+compile {:.2} ms, instantiate {:.2} ms, memory {} pages", t_compile.as_secs_f64() * 1e3, t_inst.as_secs_f64() * 1e3, memory.size(&store));

    let mut native = caltrain_data::Caltrain;
    let loc = Value::record(vec![Value::Number(37.3947), Value::Number(-122.0763)]);
    let calls: Vec<(&str, Vec<Value>)> = vec![
        ("board", vec![Value::str("sf"), Value::str("south"), Value::Number(1.0e12)]),
        ("stations", vec![loc.clone()]),
        ("search", vec![Value::str("pa"), loc.clone()]),
        ("nope", vec![]),
    ];
    for (name, args) in &calls {
        let args_bytes = Value::List(args.clone().into()).to_bytes();
        let mut call = |store: &mut Store<()>| -> Vec<u8> {
            let sp = alloc.call(&mut *store, name.len() as u32).unwrap();
            memory.write(&mut *store, sp as usize, name.as_bytes()).unwrap();
            let ap = alloc.call(&mut *store, args_bytes.len() as u32).unwrap();
            memory.write(&mut *store, ap as usize, &args_bytes).unwrap();
            let packed = query.call(&mut *store, (sp, name.len() as u32, ap, args_bytes.len() as u32)).unwrap();
            let (rp, rl) = ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize);
            let mut out = vec![0u8; rl];
            memory.read(&*store, rp, &mut out).unwrap();
            free.call(&mut *store, (rp as u32, rl as u32)).unwrap();
            free.call(&mut *store, (sp, name.len() as u32)).unwrap();
            free.call(&mut *store, (ap, args_bytes.len() as u32)).unwrap();
            out
        };
        let first = call(&mut store);
        let expect = match native.query(name, args) {
            Ok(v) => { let mut b = vec![0u8]; b.extend(v.to_bytes()); b }
            Err(e) => { let mut b = vec![1u8]; b.extend(format!("{e:?}").into_bytes()); b }
        };
        let same = first == expect;
        let n = 2000;
        let t = Instant::now();
        for _ in 0..n { let _ = call(&mut store); }
        let dt = t.elapsed();
        println!("wasmi  {name}: {:.1} us/call ({} bytes) byte-identical to native: {same}", dt.as_secs_f64() * 1e6 / n as f64, first.len());
    }
    println!("memory after: {} pages", memory.size(&store));
}
```
