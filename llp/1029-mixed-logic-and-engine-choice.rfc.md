# LLP 1029: Mixed by default — TypeScript and Rust behind one seam, and the engines an app chooses to carry

**Type:** RFC
**Status:** Draft
**Systems:** Runner (the executor slot: a third executor, `exact-wasm` on wasmtime, beside the native crate and `exact-js`; routing by source name and by digest), Build (bake compiles the data crate to wasm once and to precompiled code per target; per-source digests; the app template scaffolds both languages), Apple host and Linux host (which executors a binary links, chosen per app in one line), Delivery (LLP 1026's module card becomes per target; the runtime version carries the wasmtime version; Level A stays the default for the Rust half), Agent API (`state` reports which sources are interpreted), Contract (unchanged above the seam)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-03
**Revised:** 2026-09-09
**Related:** LLP 1028 (the measurements and the engine landscape this proposal rests on — F1–F4 the sizes and speeds, F6 the per-source idea, F7–F8 what is out of scope and why), LLP 1026 D2/D3/D4/D5/D10/D12 and §9 item 3 (the wasm executor as designed: kept whole, with wasmtime in D2's slot and D10 keyed per source — §D8 here lists every edit), LLP 1027 D3/D8/§8 (the lean VM, `Either`, and the trade that took the wasm module off staging — the last is reversed here, with the measurement that reverses it), LLP 1004 D4 (the seam: "expressions call the roster; data comes from a data source; nothing else crosses" — unchanged), LLP 1016 D1/D6 and LLP 1018 D4/D5 (the runner never does I/O; state is the Store; the properties per-source routing depends on), LLP 1012 (`state` and `logs`, where "interpreted" becomes visible), LLP 1023 D2/D3 (the envelope and the transactional swap the per-target card rides), `rules/RULES.md` §Scope (the boot path compiles nothing; modules ship as bytecode — precompiled code and Pulley bytecode both satisfy it) and §Budgets, `rules/DEFERRED.md` §Authoring models (the sentence 1027 added stands; §6 here says what this document takes off), `CLAUDE.md` (optional capability is a separate artifact or another executor, never a cargo feature on a core crate), `QUEUE.md` (the 2026-09-03 line this document replaces). External: wasmtime 47 (Cranelift, Pulley, `Module::serialize`), Shorebird's link percentage (the precedent for D4).

## Summary

**Development refinement (2026-09-09):**
[LLP 1029.000](1029.000-rust-development-reload.rfc.md) records Charlie's
request for live Rust edits: native initially, native shared-library
replacement on desktop, browser Wasm, and Pulley replacements on physical
iOS. It proposes replacing this draft's development portions of D2–D5/§7,
starting with whole modules and separating development from release engine
selection. Production Rust delivery and automatic per-source digests move
behind that work. Both documents remain Draft; the mixed-language direction
below is unchanged and is not part of the child implementation scope.

Three rulings from Charlie on 2026-09-03, in his words, after LLP 1028's
numbers were on the table:

1. *"seems like it would be pretty reasonable to allow you to include
   wasmtime optionally in this thing."*
2. *"I'd like it if you could make it so that you have the option to
   not ship wasmtime even in a Rust-logic app."*
3. *"The default I think should be mixed (Rust + TS) since I think
   writing too much Rust starts to get really unwieldy and TS will be
   easier to author and maintain at scale (Claude Code has more
   features and feels nicer than Codex for example)."*

This document is the design that follows. Nothing above the data seam
changes: a Contract app still declares `resource`, `send`, `mutation`,
and `shape`, and the runner still asks one `DataSource` (LLP 1004 D4).
Below the seam:

- **The default app is mixed** (D1). It has an `app.ts` and a `data/`
  crate from the first day, composed by LLP 1027 D8's `Either`; logic
  goes in TypeScript by default and a source moves to Rust one name at
  a time when it is hot, the byte-equality fixture holding both. The
  default binary carries the lean Hermes VM and native Rust, and no
  wasm engine.
- **Engines are named per app, in one line, and none is mandatory**
  (D2). A binary links exactly the executors its host crate's `host!`
  composition names: `exact_js::Module` for the TypeScript half,
  `data::App` for the Rust half, and — only when the app opts in —
  `exact_wasm::Swappable<data::App>`, which gives the Rust half the
  same over-the-air path and dev loop the TypeScript half has. A
  Rust-logic app that does not opt in ships no interpreter at all,
  which is LLP 1026 D12's Level A, kept as the default.
- **The wasm executor is `exact-wasm` on wasmtime** (D3). The bake
  compiles the app's data crate to one neutral wasm module and then,
  with Cranelift, to precompiled code per target; a host links
  wasmtime's runtime and no compiler: 0.49 MB on desktop, where the
  code runs at 1.25× native, and 0.58 MB on iOS with the Pulley
  interpreter (LLP 1028 F1–F3). LLP 1026 D3's ABI, D5's budgets, and
  D10's identity rule are kept whole; the last is refined **per source
  name** (D4), so an update that changed one source interprets that
  one and the rest stay native, and the agent's `state` says which.

What this reverses, and the number that reverses it: LLP 1027 §8's
take that the wasm executor "returns only if D6 is ever built," made
when the executor was assumed to cost a second interpreter of Hermes's
size. It costs 0.49–0.58 MB, is opt-in per app, and its interpreted
window on iOS closes per source at the next release. §D8 lists every
edit to 1026 and 1027; §6 names the trade; §7 stages the work; §8 asks
Charlie the questions that remain.

## 1. The rulings, and what each one fixes

**Mixed by default** fixes the shape of an app. LLP 1027 D8 made
"Rust and TypeScript side by side" possible and called the paved path
"`app.ts` alone." Charlie's ruling moves the paved path to *both*, with
TypeScript where logic goes first. The reason is authoring at scale:
substantial app logic is easier to write, read, and maintain in
TypeScript, for people and for the agents that now write most of it,
and the tooling those agents run in is better at it. Rust stays for
the source that measures hot, and for apps that want it whole
(Caltrain, ruled 1027 §10 Q5). The seam, the routing rule, and the
fixture already exist; this document makes them the default and says
what the template scaffolds (D1).

**Optional even for Rust logic** fixes the binary's floor. LLP 1026
D12 already had Level A — no interpreter, plan and assets update, code
by binary — and this ruling makes it the default for the Rust half of
every app, including a Rust-only one. The wasm executor is something
an app names, never something the language implies (D2).

**wasmtime optionally** fixes the engine. LLP 1028 F3 showed the
compiler need not ship and F1 showed the runtime is smaller than wasmi;
F4 showed the iOS interpreter is a choice this document asks Charlie
to make (§8 Q1). The executor's design is LLP 1026 D2–D5 with the
engine's name changed and the artifact made per target (D3).

## 2. Motivation, the honest version

- **Why the Rust half wants an over-the-air path at all, if TypeScript
  is the default.** Because the hot source is exactly the one a
  developer is most likely to need to fix in the field, and because
  the dev loop for a Rust edit on a phone is today a rebuild, an
  install, and a relaunch (LLP 1007 §7). With the executor linked, it
  is a 0.6 s module rebuild (1026 §5), a per-target compile in
  milliseconds (1028 F2), and a `{seq}` push — a restart with carry
  (1026 D4) on every host, interpreted on iOS and native-speed
  elsewhere.
- **Why it is opt-in and not free.** 0.58 MB on every install of an
  app that never updates its Rust is dead weight, and a second
  interpreter beside Hermes is a second thing to reason about. An app
  that wants it names it; the scaffold does not.
- **Why wasmtime and not wasmi.** One engine for every platform, the
  compiler in the bake and nowhere else, precompiled code at 1.25×
  native on desktop, 0.42 MB less than wasmi on the phone — against
  wasmi's 2.5× speed advantage under the iOS interpreter, which §8 Q1
  weighs. The ABI does not know the engine (1026 D2), so the choice is
  one crate's and reversible.
- **Why per source.** The Shorebird precedent (1028 F6) shows what
  developers actually want from code push: patch the thing you
  changed, keep the rest fast. For Rust the seam is the only place the
  two worlds meet, and it happens to be a function boundary with no
  shared state — the granularity is already right. It turns "the app
  is interpreted until the next release" into "the source you patched
  is interpreted," visible in `state`.

## 3. Design

### D1 — The default app is mixed: `app.ts` and `data/`, one seam, TypeScript first

An app scaffolded by the template has, below the seam, an `app.ts` (LLP
1027 D1's four exports) and a `data/` crate (LLP 1004 D4's `DataSource`),
composed in the host crate as

```rust
exact_apple::host!(exact_js::Either<data::App, exact_js::Module>, PLAN);
```

— LLP 1027 D8 verbatim. Each source name is answered by whichever side
declares it; a name declared by both is refused at bake
(`bake-source-twice`); `appId` must agree (`bake-app-id`); grants are
the union, checked by the host as today. The scaffold's `data/` crate
declares no source: it is the place a hot one moves to, and an empty
`DataSource` costs nothing in the binary or at boot.

**The paved path is: write it in TypeScript; move a source to Rust when
it measures hot, one name at a time, with the fixture holding both to
the same bytes** (1027 D5's byte-equality test becomes the template's
first test). "Hot" is a measurement in `metrics.mjs`, not a feeling;
1027 §5 puts the TypeScript tax at 7.5–29× per call in microseconds,
so most sources never move.

**What the default binary carries:** the lean Hermes VM (1.8 MB, after
first pixel, 1027 D3/D4) and the native Rust crate. No wasm engine.
Over-the-air updates for such an app are LLP 1026 Level A for the Rust
half (plan, assets, and — since it is bytecode — the TypeScript
module, 1027 D7) and a new binary for a Rust change.

**Fixtures.** Caltrain stays all Rust (ruled) and is the Rust-only
case. The mixed case is Caltrain's TypeScript twin (`js/tests/fixtures`)
with one source moved back to the Rust crate — `board`, the hot one —
driven through `Either` in the runner's suite. Weird Castle is the
first real app to author under the default, if Charlie says so (§8
Q5).

### D2 — Engines are named per app, in one line; none is mandatory

The `host!` composition is the one place an app says what its binary
links below the seam. Four types, three shapes:

| The app's composition | Binary carries | Over-the-air for the Rust half |
|---|---|---|
| `Either<data::App, exact_js::Module>` — **the default** | Hermes + native Rust | Level A: by binary |
| `exact_js::Module` — TypeScript only | Hermes | n/a |
| `data::App` — Rust only (Caltrain) | nothing | Level A: by binary |
| `Either<exact_wasm::Swappable<data::App>, exact_js::Module>` or `Swappable<data::App>` — **opted in** | + wasmtime's runtime (0.49 MB desktop, 0.58 MB iOS) | Level B: the module, per source (D4) |

`Swappable<D>` is a `DataSource` that owns the linked native crate
`D` and, once a module whose `app_id` matches is loaded, routes each
source name by digest (D4). Without a loaded module it *is* `D`. A
host links `exact-wasm` if and only if its composition names
`Swappable`; there is no cargo feature on `exact-runner`,
`exact-apple`, `exact-linux`, or `exact-js` (the optional-capability
rule), and no profile or environment variable that links it behind
the composition's back. **The dev client follows the same line**: an
app that did not opt in rebuilds natively on a Rust edit, as today,
and its dev client carries no executor — LLP 1026 D12's "Level B in
the dev client regardless" is narrowed to "when the app opted in"
(§8 Q2).

Ruling 2 by construction: a Rust-logic app that never names
`Swappable` ships no interpreter, and nothing in the toolchain can add
one.

### D3 — `exact-wasm` on wasmtime: the compiler in the bake, the runtime on the device

LLP 1026 D2–D5 stand with these substitutions:

- **The engine is wasmtime**, `default-features = false`, `runtime` +
  `std`, plus `pulley` on iOS. Cranelift is a dependency of the bake
  (`contract/cli`) and of nothing a host links. 1026 D2's "why wasmi"
  paragraph becomes this one; its reasons (no JIT on iOS, determinism,
  budgets, pure Rust) hold for wasmtime unchanged.
- **The bake compiles twice.** `app.module.wasm` as 1026 D2 (the data
  crate for `wasm32-unknown-unknown`, `web` profile, `wasm-opt`) — the
  canonical form, the fixture's input, and what the digests (D4) are
  computed over. Then `Engine::precompile_module` per target the app
  builds for — `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu` for the fleet, and `pulley64` for iOS —
  each written beside the plan as `app.module.<target>.cwasm`. Bake
  refuses to publish a bundle missing a target the binary declares,
  and **refuses a native artifact for an iOS target** — iOS takes
  Pulley bytecode or the neutral wasm only (1028 F5; 1030 §7). A
  serialized wasmtime module is compatible with an engine build, its
  configuration, and a target ISA baseline, not a version string; the
  receipt of all three is in the client's compatibility id (1030 D3a).
- **The device deserializes; it never validates or compiles.** A host
  `Module::deserialize`s its target's artifact (0.15 ms, 1028 F2) at
  the same placement as before: after first pixel, when the bytes are
  verified, before the served plan's boot. `rules/RULES.md` §Scope is
  satisfied literally on both platforms — precompiled code on desktop,
  Pulley bytecode on the phone — and the boot path compiles nothing.
- **The ABI is 1026 D3 unchanged**: importless, bytes in the module's
  linear memory, `exact_abi()` by number, canonical `Value` encoding,
  a malformed result is `DataError::Unavailable` naming the export.
- **Budgets are 1026 D5's** — fuel per call, a memory ceiling — with
  wasmtime's epoch interruption available as the wall-clock form
  `exact-js` already uses. The numbers are set on landing from 1028
  F2's floor with an order of magnitude of headroom.
- **The runtime version** (1026 D9) gains the wasmtime version, as it
  carries the bytecode version (1027 D7). A bundle compiled by another
  wasmtime is refused by name before download.
- **The module card** (1026 D4) becomes per target:

  ```json
  "module": {
    "wasm":   { "url": "./app.module.wasm", "sha256": "…", "bytes": 71684, "abi": 1 },
    "targets": {
      "aarch64-apple-darwin": { "url": "./app.module.aarch64-apple-darwin.cwasm", "sha256": "…", "bytes": 122000 },
      "pulley64":             { "url": "./app.module.pulley64.cwasm", "sha256": "…", "bytes": 138000 }
    },
    "sources": { "board": "<digest>", "stations": "<digest>", "search": "<digest>" }
  }
  ```

  A client fetches the neutral form (for the digests and the fixture)
  and its own target's artifact, verifies both, and boots atomically
  (1023 D3, one artifact longer). The "never a URL to native code"
  rule 1026 D4 cited for the neutral card is honored in substance: the
  artifact is the host's own executor's precompiled form of bytecode
  the same bundle carries, signed with it (1026 D11), version-pinned,
  and loadable by nothing else.
- **macOS needs an entitlement** to execute the loaded code under the
  hardened runtime (1028 F3); the dev client and `build.mjs` set it; a
  Developer ID build passes notarization with a justification; whether
  the Mac App Store accepts it is unverified (§8 Q6). Which of the two
  entitlements wasmtime's loader needs on macOS 26 is verified in stage
  0 (§7).

**The iOS interpreter is Pulley by default** — one engine, 0.42 MB
smaller, no validator on the device — and wasmi is the measured
fallback if the phone number says its 2.5× matters at the seam: the ABI
does not know the engine, so the swap is one crate's. §8 Q1.

### D4 — Identity per source: the linked crate answers what did not change

LLP 1026 D10 keyed native-by-digest on the whole module. Refined:

- **The bake computes a digest per export.** Over the neutral
  `app.module.wasm`, for each `answer`/`parse`-reachable source name:
  the canonical bytes of every wasm function reachable from that
  source's dispatch arm, plus the whole data section (conservative:
  a data change marks every source). Written into the module card's
  `sources` map and, for the binary's own build, beside the embedded
  runtime version.
- **`Swappable` routes per source.** A source whose digest in the
  selected bundle equals the embedded one is answered by the linked
  native crate; one whose digest differs is answered by the module.
  A source the module declares and the crate does not is the module's;
  the reverse is the crate's. The routing table is built once at
  module load and is a value in `state`.
- **Visible.** `state` gains `sources.interpreted: ["board"]` and
  `logs` says `module: 1 of 3 sources interpreted` at load; the agent
  API's `state` (LLP 1012) is where a smoke test asserts it and where
  `metrics.mjs` reads it. Shorebird's link percentage, as a list.
- **The precondition is the seam's own discipline.** Sources share no
  in-process mutable state: state is the Store (LLP 1018), a request
  is a value (LLP 1016 D1), a crate is a function of its inputs. The
  byte-equality fixture per source, run with the routing table forced
  each way, is the check — a crate with a cross-source cache fails it
  at bake, before anything ships.
- **Determinism of the digest.** Two bakes of the same source on the
  same toolchain produce the same wasm; a mismatch routes to the
  module, never to the crate — the error is one of speed, not of
  correctness.

A whole-module change (every digest differs) degrades to 1026 D10 as
written. The next binary embeds the changed code, its digests match
again, and every source is native.

**Panel bearing (2026-09-03, the LLP 1030 panel — codex sol #12,
folded into 1030 D3a).** Two corrections this design takes: (1) a
`wasm32` build can be byte-identical while the native build of the same
crate changed — a `#[cfg(target_os)]` branch, a native dependency, a
build script — so the wasm digest is *not* the native crate's identity.
The data crate is therefore **target-neutral by construction** (bake
refuses `cfg(target_*)` and native dependencies in `data/`; it already
depends on `exact-runner`'s types alone), and the identity that enters
a client's compatibility id is the crate's source-input digest, not its
wasm output. (2) Per-source routing needs a defined closure algorithm
over the module (globals, tables, element segments, indirect calls,
shared helpers, data) and provenance metadata in the module naming each
source's exports; until those exist the routing is **whole-module**
(1026 D10 as written), which stage 1 builds, and per source is stage 2
only once the closure is specified. The byte-equality fixture checks
determinism; it does not prove the absence of cross-source state, which
the target-neutral lint and the store discipline (LLP 1018) carry.

### D5 — What the dev loop and the release get, per shape

| | Default (mixed, not opted in) | Opted in (`Swappable`) |
|---|---|---|
| TypeScript edit | bake 20 ms → `{seq}` → restart with carry, every host (1027 D5) | same |
| Rust edit, dev client | native rebuild, install, relaunch (as today) | module rebuild 0.6 s + precompile ms → `{seq}` → restart with carry; native speed on desktop, interpreted on iOS |
| Rust edit, release | a new binary | Level B: the module in the update bundle; the changed sources interpreted until the next binary |
| Binary, iOS | Hermes 1.8 MB | + 0.58 MB |
| Binary, desktop | Hermes 1.8 MB | + 0.49 MB |

`dev.mjs`'s classification by path (1026 D4) decides which row an edit
is; a Rust edit in a not-opted-in app is `{rebuilt}`, as today.

### D6 — The interpreter rule, restated

LLP 1027 §8 said "at most one interpreter, chosen by the app's
language." The rule that survives the measurement: **a binary carries
the executors its app named — the lean Hermes VM if and only if there
is an `app.ts`, one wasm runtime if and only if the composition names
`Swappable`, and never a compiler.** The default app carries one; an
opted-in mixed app carries two, at 2.4 MB on iOS; a Rust-only app
carries none. Every case is chosen in one line the developer can read.

### D7 — Kept out of scope, each with its trigger

- **A JavaScript `WebAssembly` global** for TypeScript modules or ibex2
  (1028 F8). A different consumer; stacks on Hermes; the aliasing path
  unverified. ibex2's item, triggered by a module that imports a
  package shipping wasm (Rive, Lottie). Not in `exact-js`'s prelude.
- **Surfaces as wasm** (1026 D6). The per-frame case, where an
  interpreter's tax is real on iOS and unmeasured; §6 takes it off
  staging.
- **Other languages below the seam.** D3 makes any language that
  compiles to wasm a possible Rust-half replacement; nothing is paved
  for one (no template, no fixture, no types) until an app asks.
- **Function-level patching inside a source** (1028 F6). Impossible on
  iOS; the source is the granularity.

### D8 — What this changes on the record, edit by edit

- **LLP 1026 D2** — "wasmi" becomes "wasmtime, the runtime only; the
  compiler in the bake" (D3). "Why an interpreter, and why wasmi"
  becomes the paragraph in D3 above, with the F4 choice noted.
- **LLP 1026 D4** — the module card gains `targets` and `sources`; the
  neutral form stays as `wasm`.
- **LLP 1026 D10** — keyed per source (D4 here); the table's third row
  reads "the changed sources, interpreted (native code on desktop)."
- **LLP 1026 D12** — Level B's cost is 0.49–0.58 MB, not 1.0; "Level B
  in the dev client regardless" becomes "when the app opted in" (§8
  Q2); Level A is the default for the Rust half of every app.
- **LLP 1026 §9 item 3** — back on staging as §7 here; **D6 leaves
  staging** (§6).
- **LLP 1026 §10 Q1** — answered: 0.58 MB, opt-in.
- **LLP 1027 D8** — "the paved path is `app.ts` alone" becomes "the
  paved path is both, TypeScript first" (D1).
- **LLP 1027 §8, the first take** — "wasmi returns only if D6 is ever
  built" is reversed by 1028 F1–F3 and this document; the one-
  interpreter rule is D6 here.
- **`rules/DEFERRED.md` §Authoring models** — the sentence 1027 added
  stands; one clause proposed: *"Logic below the data seam is
  TypeScript by default or Rust, and an app is both by default."*
- **`QUEUE.md`** — the 2026-09-03 wasm-executor line is replaced by a
  pointer to this document, and deleted when §7 lands.

The edits to 1026 and 1027 are amendment notes with today's date, in
the pattern 1027 used on 1026's header and §7. They are made when this
document is accepted, not before.

## 4. Costs and budgets

- **Binary.** D5's table. Nothing on `rules/RULES.md`'s budgets; the
  phone floor for the default app is unchanged from 1027 (1.8 MB); an
  opted-in app adds 0.58 MB on iOS, ruled acceptable in principle by
  "reasonable to include wasmtime optionally."
- **Boot.** Unchanged: the executor is created after first pixel in
  the slot 1026 D2 / 1027 D4 use; deserialize + instantiate is 0.17 ms
  (1028 F2). The first frame is compiled data or the kept answer
  (1027 D4 as ruled); a module change restarts resources (1026 D4).
- **Per call.** On desktop, 1.25× native; on iOS, 5–13× native
  depending on §8 Q1, in microseconds, per resource change, never per
  frame. Only the changed sources pay it (D4).
- **Bake.** One more wasm32 build of the data crate (0.6 s warm, 1026
  §5), one Cranelift compile per target (2 ms for a 235-byte module;
  linear, expected tens to hundreds of ms for a real crate — measured
  in stage 1), the per-export digest walk (a pass over the module).
  The 100 ms edit-to-present budget is for the TypeScript loop and is
  untouched; the Rust loop's number is the dev client's, recorded in
  stage 1.
- **The bundle.** Per-target artifacts: ≈50 KB fixed overhead each
  plus the code; the neutral form beside them. Assets by digest (1026
  D11) mean a client fetches only its own target's file.
- **Checks.** None added. The per-source fixture is one test in the
  runner's suite; `boot.mjs` counts what it counts; `caps.mjs` counts
  files.
- **The second interpreter.** The real cost, in the mixed-and-opted-in
  case only: two engines to reason about and 2.4 MB. D6 keeps it a
  choice made in one visible line.

## 5. What this refuses

- **A compiler on the device.** Cranelift and Winch stay in the bake.
  The boot rule is the reason; the 5.6 MB is the second reason.
- **A WKWebView as the seam's executor** (1028 F7).
- **wasmer, V8, or JavaScriptCore in the executor slot** (1028 F5).
- **A JavaScript `WebAssembly` global in `exact-js`** (D7).
- **Linking `exact-wasm` by anything but the composition** — no
  feature flag, no profile, no environment variable.
- **Function-level patching, or any split finer than a source** (D7).
- **Per-platform logic overrides.** The seam is the same on every
  host; the executor differs, the code does not.

## 6. The trades this RFC owes (Charlie's, before Acceptance)

`rules/DEFERRED.md` §Moving something off this list: name what it
unblocks, take something off the doing-list. Onto the doing-list:
`exact-wasm` (one crate), the bake's per-target compile and digests,
the per-target module card, `Swappable`, the mixed scaffold. What
comes off:

- **LLP 1026 D6, surfaces as wasm, leaves staging.** It stays in 1026
  as an idea with its trigger (a per-frame tax measured under the
  interpreter on the three real surfaces), and nothing in this
  document builds toward it: wgpu stays in the app's module, surfaces
  are native, and a surface change is a new binary. The per-frame case
  is the one where an interpreter on iOS is honestly slow, and it is
  the one this document declines to pay for.
- **The on-device validator.** wasmi's `validate` on the phone (1026
  D2) goes: the bake validates, the device deserializes what the bake
  signed.
- **The platform-neutral module card as the only form** (1026 D4). The
  neutral file stays; the promise that it is the only thing a client
  fetches does not.
- **1027 §8's "one interpreter, chosen by the language"** as a rule:
  replaced by D6's "the executors the app named, none by default for
  the Rust half," which is smaller for the default app and larger
  only by choice.

Nothing on `DEFERRED.md` itself moves: the update economy was opened
by 1026 D11 on Charlie's 2026-09-02 ask; hot revision surfaces stay
where they are; the authoring-model sentence gains a clause and loses
nothing. If Charlie reads this document as opening a door that list
closes, the take is the first bullet.

## 7. Staging

Each stage names what it needs and how it is verified by running. An
implementer and a date are owed before any stage is a spec
(`rules/RULES.md` §Scope); the numbers here are enough to rule on.

0. **The phone numbers** (an afternoon with `build.mjs --device`). The
   1028 probe's module under Pulley and wasmi on an iPhone, and 1026
   §5's Caltrain module under both. Decides §8 Q1 with a measurement
   instead of a leaning. Also: which macOS entitlement wasmtime's
   loader needs.
1. **`exact-wasm` on wasmtime, in the dev client.** The crate with
   1026 D3's ABI table (`build.rs`-generated beside the ABI number);
   the bake's wasm32 build and per-target precompile; `Swappable` with
   whole-module identity (1026 D10 as written); the composition in
   Caltrain's hosts *for the fixture only* (Caltrain stays not opted
   in by default — the test names `Swappable`, the app does not).
   Verified by running: the byte-equality test across native,
   precompiled-native, and Pulley on the twenty cases; `dev.mjs` edit
   `board` → `{seq}` → the macOS dev client shows it with no rebuild;
   the same on an iOS simulator through Pulley; a module with an
   import refused by name; a looping module refused by budget with the
   last value on screen; a weird-castle module into a Caltrain dev
   client refused by `app_id`. The five checks green; the Rust-loop
   number recorded in `metrics.mjs` as a diagnostic row.
2. **Per source.** The digest walk in the bake; `sources` in the
   card; `Swappable` routing per name; `state.sources.interpreted`
   and the `logs` line; the fixture run with the table forced each
   way. Verified by driving it: edit `board` alone, `state` says
   `["board"]`, `stations` still answers from the crate (a counter in
   the test crate proves which side answered).
3. **The default scaffold.** The template emits `app.ts`, an empty
   `data/` crate, the `Either` composition, and the byte-equality test
   as its first test; `scripts/app.mjs` knows the shape; the docs say
   "TypeScript first; move a source when it measures hot." Verified:
   a new app boots on web, macOS, iOS, Linux with one TypeScript
   source, then with that source moved to Rust, same bytes.
4. **Release, Level B for an opted-in app.** Depends on 1026 Level A
   landing (the store, selection, signing). The per-target card
   consumed; the runtime version with the wasmtime version; the
   entitlement in `build.mjs`. Verified: a Caltrain release build
   opted in for the test picks up a `board` update from a static
   directory at the next launch, `state` says `["board"]`, and the
   next binary says `[]`.

## 8. Open questions (for Charlie)

1. **Pulley or wasmi on iOS?** Pulley: one engine, 0.58 MB, no
   validator on the device, 13× native on the call-heavy
   microbenchmark. wasmi: 1.0 MB, 5.4×, two engines in the program.
   Leaning: Pulley, and let stage 0's phone number reverse it if the
   seam-shaped tax is what a user would feel — it is per resource
   change, in microseconds, and only for changed sources.
2. **Does the dev client link the executor only when the app opted
   in** (D2), narrowing 1026 D12's "Level B in the dev client
   regardless"? Leaning: yes — the ruling was "the option to not ship
   wasmtime," and a dev client that carries what the release will not
   is a surprise; the cloud agent's loop (1026 §1) opts its apps in.
3. **Per-source routing in stage 1 or stage 2?** Leaning: stage 2;
   whole-module identity is 1026 D10 as written and is enough to
   prove the executor; the digest walk is its own small lane.
4. **The mixed scaffold: both files from day one** (D1), or `app.ts`
   alone with `data/` added on the first hot source? Leaning: both
   from day one — an empty crate costs nothing, and "move a source to
   Rust" should be a file edit, not a restructure.
5. **Weird Castle under the default?** Its logic is Rust today. Moving
   it to TypeScript with the seam's fixture holding each source equal
   is the first real exercise of D1; it is Charlie's app and his call.
6. **The macOS entitlement in App Store release builds** — accept it
   for opted-in apps? Leaning: yes; it is the same entitlement every
   browser and Electron app carries, and Level A needs none.
7. **The JavaScript `WebAssembly` global** — leave it to ibex2 with
   the Rive/Lottie trigger (D7)? Leaning: yes.
8. **Naming.** `exact-wasm`, `Swappable`, `app.module.<target>.cwasm`
   — or the implementer's.

## Ratification note

Draft r1, 2026-09-03, written the day of the rulings and after LLP
1028's probe ran; unreviewed. The numbers are this Mac's; the phone's
are stage 0. Nothing in the repo changes until Charlie accepts and an
implementer and a date are named; the amendment notes of §D8 are made
at acceptance.
