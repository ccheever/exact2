# LLP 1030: Delivery, unified — hot reload in dev and updates in production, on every layer of an app

**Type:** RFC
**Status:** Draft r2 (r1 written 2026-09-03 on Charlie's ask; r2 the same day, folding a two-round dual-family panel Charlie convened — grok-4.6 at xhigh and codex gpt-5.6-sol at ultra, round 1 blind, round 2 mutually visible; artifacts `llp/reviews/1030-delivery-unified.{grok,sol}.md`; dispositions in §8. §5's rulings of 2026-09-03 are inline: all seven ruled)
**Systems:** Build (bake as the producer of every artifact, its digest, and the build receipt; the change classifier), Delivery (the update bundle, the store, the compatibility id and its streams, the two levels — generalized from LLP 1026 to every layer), Web dev loop (`dev.mjs` as the first carrier), Apple host and Linux host (what a binary embeds, what it can take from the wire, the host metadata bake now owns), Runner (what a reload carries, what an update restarts; the `delivery` resource and command), Plan format (the compatibility id as a derived key), Native modules (LLP 1024: the roster in the binary), Contract (one resource and one command, D7), Agent API (`state.delivery` mirrors them)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-03
**Related:** LLP 1030.000 (the implementation: the dev server classifies, a separate verb publishes, one URL, policies), LLP 1026 D4/D5/D9/D10/D11/D12 (the embedded-plus-update model, grants pinned by the client, the store, the runtime version, digest identity, the update economy, Level A/B — this document is that model stated for every layer; §6 says what it amends), LLP 1029 D2/D3/D4 (engines named per app; the wasm executor; identity per source — the layers this document places), LLP 1027 D5/D7 (TypeScript to bytecode; the module card carries bytecode), LLP 1023 D1/D2/D3/D6/D10 (the URL contract, the envelope, the transactional swap, identity is not trust, one plan is the invariant), LLP 1007 §6/§7 (what a dev reload carries; a native Rust change is a new binary), LLP 1018 D4/D5/D7 (bake sees an empty store; the token below the seam; the platform facts — signing, keychains), LLP 1019 (declared fonts as assets), LLP 1020 (the webview), LLP 1024 D3/D7 (one app artifact; the roster ships in the binary), LLP 1009 D5 (shaders validated at build, compiled at first use — from the crate, D8), LLP 1012 (`state` is the agent's; the carrier stays off the network), `rules/RULES.md` §Scope and §Budgets, `rules/DEFERRED.md` §Runtime ("Snapback / update economy" is still on the list — 1026 D11 opened it on Charlie's ask and the trade is 1026's to make at its acceptance; nothing here presumes it) and §Process. External precedent: Expo Updates and EAS, Shorebird, Apple's App Store Connect, TestFlight, notarization, `BGAppRefreshTask`, universal links (`apple-app-site-association`) and Android `assetlinks.json`. Predecessor, research never authority: exact1 LLP 0347, 0421, 0524, `docs/hosting-ssr-ota.md`.

## Summary

Charlie, 2026-09-03: a unified theory of hot reloading in development and
over-the-air updates in production, spanning every layer of an
application — the Contract, the Rust logic, the wasm'd logic, the
TypeScript logic, assets, the host binary with its entitlements, icons,
and metadata, native modules, WebGPU and shaders, the runtime itself
when it is upgraded, and icons and metadata that are live on some
platforms and not others — recognizing that an app may ship without
TypeScript, without wasm, or without an update mechanism at all, and
can then be updated only through heavier means.

The theory is short. **An app is a set of artifacts, each with a
digest. A change is the set of digests that differ. For each installed
client there is a set of carriers — a restart with carry, the update
bundle, the static origin, a new binary — and each artifact of the
change has a lightest carrier that can move it to that client, which
depends on three facts about the client: its compatibility id, the
executors it linked, and whether it carries an update store at all.
Development and production are the same act under two policies: in
dev the carriers are the dev server and the client is one you control;
in production the carriers are static files and stores, and the
clients are many, in many cohorts, arriving at different times.**

r2 corrects r1 where the panel showed it wrong: the level ladder is two
axes, not three rungs (D4); the cohort key is a per-platform
**compatibility id**, not the runtime version (D3a); the classifier
answers **per platform and per stream**, with the static origin as its
own carrier and the bake computing dependency from the artifact graph
rather than from co-occurrence (D3); the dev server's classifier is
new work, not a hardening of code that exists (D3); shaders are
compiled into the binary today and stay binary-coupled until they are
packaged as assets (D8); freeze is the only v1 policy for a runtime
upgrade (D9); the durable-state guarantee is narrowed to what the
mechanisms give (D9); reach, background refresh, and the sunset card
are stated at their real strength (D5); and D1 gains the rows both
panelists found missing. What r1 had that held: the inventory as a
table, host metadata as a baked manifest (D2), Level 0 (D4),
propagation without a service (D5), the GPU as three sub-layers (D8),
the constants of an upgrade (D9), and the per-platform honesty about
icons and names (D10). §7 is what this refuses; §8 the panel fold.

## 1. What exists, what is accepted but unbuilt, and what is proposed

The panel's first catch was that r1 wrote "none of that changes" over
a foundation that is mostly not built. The honest table:

| Piece | Where | State |
|---|---|---|
| The dev loop: restart with carry over SSE, `{rebuilt}` reloads | LLP 1007 §6, `dev.mjs` | **built** |
| The URL contract, the envelope, the transactional swap, Stage 1 | LLP 1023 D1–D3, 1023.001 | **built** (Stage 1); the launcher (Stage 3) and the asset slice (D4) unbuilt |
| TypeScript logic as bytecode behind the seam, `fetch`, kept answers | LLP 1027 stages 1–4 | **built**; delivery of bytecode (D7) unbuilt |
| The embedded-plus-update model: store, selection, signing, anti-rollback, crash fallback, assets by digest, Level A/B | LLP 1026 D9–D12 | **Draft, unbuilt** — "Snapback / update economy" **left `DEFERRED.md` 2026-09-03** on Charlie's ruling, in 1026 D11's minimal form, with the take 1026 §8 named; the service half stays refused |
| Native modules, the roster in the binary | LLP 1024 | Draft, unbuilt |
| The wasm executor on wasmtime, engines per app, identity per source | LLP 1029 | Draft, unbuilt; 1028 measured on this Mac only |
| Everything this document adds | here | proposed |

So this document is a theory written ahead of most of its mechanisms,
which `rules/RULES.md` permits for an RFC and forbids for a spec. It
depends on 1026 landing; it amends 1026 where §6 says; it does not
move anything off `DEFERRED.md` itself.

## 2. Design

### D1 — The inventory: every layer, its artifact, its identity, its carriers

The table is the document. A carrier is the lightest thing that can
move that layer to a client that already has the app; where the
carrier differs by platform the cell says so. "Binary" means a new
binary through whatever channel that platform has.

| Layer | Artifact | Produced by | Identity | Dev carrier | Production carrier | The client must have |
|---|---|---|---|---|---|---|
| **Contract** (`app.contract`) | `app.plan` | compile + bake, 8–13 ms | plan sha256; format and kernel schema in the compatibility id | `{seq}` — restart with carry (1007 §6) | bundle | a matching compatibility id |
| **Compiled boot values, empty-store placeholders** | inside the plan | bake, through the app's executor (1027 D5) | the plan's | with the plan | with the plan | — |
| **TypeScript logic** (`app.ts`) | `app.hbc`; JavaScript for the web | Rolldown + `hermesc`, 20 ms | sha256; the Hermes receipt in the compatibility id | `{seq}` as a module change: restart with carry, resources restart (1026 D4) | bundle — always interpreted (1027 D7) | Hermes linked |
| **Rust logic, native** (`data/`) | in the binary | cargo | the crate's identity is part of the compatibility id when the app did not opt in (D3a) | rebuild, install, relaunch | **binary** | — |
| **Rust logic, as a module** (opted in, 1029 D2) | `app.module.wasm` + a per-target artifact | cargo wasm32 (0.6 s) + Cranelift in the bake | whole-module digest first (1026 D10); per source later (1029 D4) — see D3a on why the wasm digest is not the native crate's | `{seq}` as a module change | bundle — the changed module interpreted until the next binary | `exact-wasm` linked; **iOS takes Pulley or wasm bytecode only — bake refuses a native artifact for an iOS target**; on macOS a native artifact needs the entitlement 1028 F3 leaves unmeasured, passes Developer ID with notarization on a justification, and whether the **Mac App Store** accepts one is unverified (1029 §8 Q6) |
| **Logic in another wasm-compiled language** | the same module | its toolchain | the same | the same | the same | the same |
| **GPU surfaces** — the Rust half: pipeline layout, vertex formats, draw logic (D8) | in the binary or the app module (1024 D3) | cargo | the surface crate's, with the reflected shader interface (D8) | rebuild | **binary** (1026 D6 is off staging, 1029 §6) | — |
| **Shaders** — WGSL, compute included (D8) | **today: embedded in generated Rust (`SOURCE`), so part of the binary**; after the packaging ruled into 1030.000 stage 1: asset files, validated at build, compiled at first use | bake; `gpu/reflect` | asset sha256 **and** the reflected-interface digest | today: a GPU-crate rebuild; after packaging: the asset row | today: **binary**; after packaging: bundle if the interface digest is unchanged, else binary | the GPU module (1009 D2) |
| **GPU data assets** — textures, meshes, lookup tables | files | bake | sha256 each | asset row | bundle | — |
| **The GPU module itself** — wgpu, `exact-gpu`, the loader | the optional module (1009 D2) | cargo | in the compatibility id | rebuild | **binary** | — |
| **Assets** — images, declared fonts (1019), deck HTML (1020), strings | files | bake | sha256 each | the asset row (D10): `{seq}` names the changed digests, the host fetches them and re-renders — bundled-only today (1023 D4), the slice is 1030.000 stage 1 | bundle, by digest | — |
| **Native modules** (1024) | the app module artifact + the roster | cargo, swiftc | roster digest in the compatibility id | rebuild | **binary** (1024 D7) | — |
| **exact2 itself** — kernel, runner, hosts, motion, GPU glue, the updater | the executable | cargo, swiftc | kernel schema digest, format, ABI numbers, the updater's store codec version — the compatibility id | rebuild | **binary**; a new compatibility id | — |
| **Host metadata** — `Info.plist` strings and permissions, URL schemes, background modes and task identifiers, the associated-domains and other entitlements, App Sandbox and hardened-runtime entitlements, app groups and keychain access groups, icons and their dark/tinted appearances, launch screen, localized `InfoPlist.strings`, bundle id, the store-required monotonic version, the privacy manifest (derived from the APIs and data the app and exact2 actually use, not a fixed row) | generated from the manifest's `[app]` and `[host.<platform>]` sections (D2) | bake | the manifest projection's digest, part of the binary's identity; the shipped icon roster and bundle-visible capabilities also in the compatibility id | rebuild and reinstall | **binary**, through the store's review; per item, D10's table says which platforms can take it live | — |
| **The deep-link association files** — `apple-app-site-association`, `assetlinks.json` | static files at the origin's well-known paths | bake, from the manifest | sha256 | served by `dev.mjs` (no universal-link claim in dev — 1030.000 D2) | **the origin** — Apple's CDN and devices cache it on the order of a day; the entitlement that claims a domain is host metadata, a binary | — |
| **Cache policy** for origin files | headers bake emits (`immutable` for hashed files; `no-store` for the envelope, `index.html`, well-known) | bake | — | `dev.mjs` sends them | **the origin** | — |
| **The verification keys** for bundles (public; the private signing key never ships) | in the binary | bake | the key set — the trust epoch — in the compatibility id | none in dev | **binary**; rotation is a new compatibility id (overlap is owed, §6) | — |
| **The grant ceiling** (1026 D5) | in the binary | bake | in the compatibility id | — | **binary** — a bundle can never raise it | — |
| **Durable state** (1018) | the store | the app, at run time | — | carried across `{seq}`; kept answers (1027 D4) | stays where its shape still conforms; D9 says what that promises | — |
| **The update store's own record** | on disk | the updater | its codec version, in the compatibility id | — | frozen per compatibility id; a new binary reads its own or selects entry zero | — |
| **The web app** | `index.html`, `glue.js`, `app.wasm`, the plan, the JavaScript module | `build.mjs` | content hashes | `{seq}`; `{rebuilt}` reloads | **the origin** — a fresh load is current; an open page is not, and an installed PWA's icon, name, and splash are the browser's cached copies; no service worker in v1 | — |
| **The envelope** (`exact.json`) | the manifest of all of the above, per stream (D3a) | bake; `dev.mjs` overlays the live form | `seq` per stream; the digests it names | is the dev carrier | is the production manifest | — |
| **Distribution artifacts** — archive, IPA, signed bundle, package, notarization ticket, symbols | beside the binary | the binary pipeline | — | — | travel with the binary; not layers of the app, and not the bundle's | — |
| **Dev clients and the launcher** (1026 D8, 1023 Stage 3) | separate binaries with their own compatibility ids | `build.mjs` | their own | — | TestFlight, a `.app`, a package | — |
| **A brownfield host app** | the developer's app embedding exact2 | theirs | theirs, plus a signed host descriptor (1030.000 D7) | their dev build with the dev store policy | their release; exact's bundle and store inside their container | a container directory, first pixel of their own, and a descriptor of what they linked |

Not layers, and why, are in D6. Read down the production column and
the theory is visible: four carriers — the bundle, the origin, the
binary, the host app's release — and the layer and the platform decide
which. Read across and the same layer has the same identity in dev and
in production; only the policy differs.

### D2 — Host metadata is a baked artifact from one manifest; policy is not identity

Entitlements, `Info.plist`, icons, the launch screen, the bundle id,
version numbers, URL schemes, background modes, and the privacy
manifest are today partly written by `build.mjs` from the crate name
and the team (`build.mjs:155–184`, for iOS only; macOS gets neither a
bundle nor entitlements) and partly nowhere. They become **one
declared manifest per app** — the same standing `kernel/tables/
schema.json` has for the kernel: edit the declaration, never the
generated file. Bake generates every platform file from it and
`build.mjs` consumes the generated files.

The panel's correction: the manifest is not one blob whose digest is
the binary's identity. It has typed sections, and only some are
identity:

- `[app]` — id, name, origin, the native-client affordance — and
  `[host.<platform>]` — everything in D1's host-metadata row —
  project to **per-platform artifacts** whose digests are part of the
  binary's identity and, where a bundle can depend on them (the icon
  roster, background modes, entitlements a plan can invoke), part of
  the compatibility id (D3a).
- `[deploy]` (1030.000 D4) — schedules, channels, release policy,
  live streams, secret references — is **policy** and contributes to
  no artifact's identity. Changing `automatic` to `manual` is not a
  binary.
- What the manifest cannot know — the provisioning profile actually
  used, the signing identity, the toolchain, the SDK — goes into a
  **build receipt** the binary pipeline writes beside the binary, so
  "what did this binary actually contain" is answered by a file, not
  by a guess.

Identity is derived once. Today the bundle id comes from the crate
name (`build.mjs:48–49`) and the app's layout from `scripts/app.mjs:
24–27`; both read the manifest's `[app]` after this. Store-console
metadata (screenshots, descriptions, ratings, agreements) is not an
artifact of the app and is not managed here (D6).

**Desktop window consumer (2026-09-07):** `host.macos.window` optionally declares
`width`, `height`, `minWidth`, and `minHeight` in points, baked into the existing
Info.plist as `ExactWindow`. Fieldnotes uses 1100×760 with a 760×560 minimum.
The standalone adapter reads its title from the baked display name and restores
a configured app's frame under its bundle identity. Bare development executables
read the existing adjacent Info.plist. Agent/smoke runs skip restoration and use
the existing viewport unless `EXACT_WINDOW_WIDTH`/`EXACT_WINDOW_HEIGHT` overrides it.

**The macOS design (2026-10-02):** AppKit draws its macOS 26 design (Liquid
Glass bezels, new control metrics) by the SDK the executable records in
`LC_BUILD_VERSION`, not the SDK it compiled against. SwiftPM's link drives clang
with `--sysroot`, from which clang reads no SDK version, so every Exact macOS app
had recorded its deployment target (`sdk 14.0`) and drawn as on macOS 14; the
build now passes `-isysroot` to the linker driver, as the iOS build already did,
and refuses an executable whose recorded SDK is not the one meant. An app that
wants the earlier design says `host.macos.designRequiresCompatibility: true`:
macOS 27 ignores Apple's `UIDesignRequiresCompatibility` key (probed on 27.0.1:
the key is read, the new design drawn), so the link records the macOS 15 SDK,
the last before the new design, which also turns off every other AppKit
behaviour keyed on a later SDK; App Store Connect requires uploads built with a
recent SDK and reads the recorded one. An app whose `minimumOS` is 26 or later
cannot ask for it. `host.ios.designRequiresCompatibility` is the same on iOS: iOS
27 ignores the key too (probed on the iOS 27.0 simulator: the key's build and the
plain one draw identical pixels), and the link records the iOS 18 SDK, under
which UIKit draws the iOS 18 design, a glass button configuration as a bordered
one. The hosts read the recorded SDK (`LinkedDesign.liquidGlass`) where they
choose between a glass look and an earlier one.

### D3a — The compatibility id: the cohort a bundle is safe for

LLP 1026 D9's runtime version — kernel schema, format, module ABI,
roster digest, plus the bytecode and wasmtime versions from 1027 and
1029 — is not enough to say which installed binaries a bundle is safe
for. The panel's counterexample: two binaries with the same runtime
version but different native Rust (a change that moved no schema or
ABI) fetch the same manifest, and a plan baked against the newer crate
reaches the older one and drives its old code by name. The same holds
for an icon a plan selects that only the newer binary shipped, a
background mode only one has, or a verification key only one trusts.

The **compatibility id** is the digest of everything a bundle may
depend on and cannot replace, computed per platform by bake and baked
into the binary:

- the kernel schema digest, `formatVersion`, every ABI number, the
  updater's store codec version;
- the executor set and each engine's receipt — the Hermes build, the
  wasmtime build, its configuration and target ISA baseline (a
  serialized wasmtime module is compatible with an engine build and
  configuration, not with a version string);
- the native data crate's identity when the app did not opt into
  `Swappable`, and the GPU surfaces' and native modules' rosters;
- the shipped icon roster and every host capability a plan can
  invoke — background modes, associated domains, URL schemes;
- the verification-key set (the trust epoch) and the grant ceiling;
- platform, architecture, minimum OS; whether an update store is
  present (D4) and which artifact kinds it accepts.

**Why the wasm digest is not the native crate's identity.** The panel
caught what 1026 D10 and 1029 D4 assumed: a `wasm32` build of the data
crate can be byte-identical while the native build changed — a
`#[cfg(target_os)]` branch, a native dependency, a build script. Two
answers, both taken: the data crate is **target-neutral by
construction** (bake refuses `cfg(target_*)` and native dependencies
in it — it already depends on `exact-runner`'s types only), and the
identity that enters the compatibility id is the crate's **source
input digest** (its tree, its lockfile, its toolchain), not its wasm
output. The wasm module's digest still decides native-versus-module
routing (1026 D10) for an opted-in app, whole module first; per-source
routing (1029 D4) waits for a defined closure algorithm and provenance
metadata in the module, and is not claimed here.

**What must not fork it.** The id hashes what makes a bundle unsafe on
a binary and nothing else: `[deploy]`, schedules, the live list, and a
host change a bundle cannot observe — a faster painter, a fixed
presenter bug — leave it unchanged, so a host bug-fix binary keeps its
streams (D9). The test, in three binaries: two that differ only by a
painter speedup share the id; one that adds an alternate icon does
not; one that rotates the verification-key set does not, and that is a
new trust epoch and a new stream.

Three ids, kept apart: the **compatibility id** names a cohort; a
**stream** is `(app, channel, compatibility id)` and has its own
monotonic signed `seq` and its own path on the origin —
`U/.exact/<compatibilityId>/exact.json`, so a static host serves the
right manifest to each cohort with no negotiation. The signed head
binds the app id, the channel, the compatibility id, the `seq`, and
the key id into the signed bytes — LLP 1026 D11 signs the envelope's
canonical bytes but binds no stream, which is why a head could be
replayed onto another channel; §6 owes 1026 that binding. A **release
id**
correlates one deploy across every stream and platform it touched,
since equal `seq` numbers across streams are not a thing.

### D3 — The classifier: per platform, per stream, dependency from the graph

For a change `Δ` and a target platform `P`, bake computes, for every
live stream `S` on `P` (D5), what can reach `S`'s cohort and by what
carrier, and for the origin what changes there. The output is a table
(1030.000 D3 prints it), not a single verdict:

1. **The origin row.** The web app, the association files, cache
   policy: published to the origin; a fresh load is current. Never
   gated on a native binary, and never gating one.
2. **Per `(P, S)`: the bundle row.** A **candidate snapshot** — the
   whole app as baked from this source — is checked against `S`'s
   frozen compatibility id: every source the plan names exists in that
   cohort's Rust or in the bytecode the bundle carries; every shape
   conforms; every icon the plan selects is in the cohort's roster;
   every capability the plan invokes is one the cohort has; every
   executor the bundle needs is one the cohort linked. If the check
   passes, the snapshot is publishable to `S`: plan, assets, and the
   code its cohort can run. If it fails, the table says which artifact
   failed and why, and **nothing is published to `S`** — the snapshot
   is whole or absent, which is the client's transactional swap (1023
   D3) stated for the publisher.
3. **Per `P`: the binary row.** If `Δ` touches a binary layer on `P`
   (exact2 itself, native Rust without `Swappable`, surfaces, native
   modules, host metadata for `P`, the key set), a binary is needed on
   `P`, and the table says whether it moves the compatibility id.
4. **A client with no store** (Level 0, D4) is a cohort whose only
   carrier is the binary; the origin row still applies to its web
   twin.

Two things the panel settled that r1 had wrong:

- **Dependency is computed, not asserted, and co-occurrence is not
  dependency.** r1 said a change spanning a bundle layer and a binary
  layer is a binary, whole, with a developer override to say
  otherwise. Both panelists rejected both halves: withholding an
  independent TypeScript fix from old cohorts because an entitlement
  changed in the same commit is over-conservative, and a human
  assertion of independence is unverifiable. The check in rule 2 is
  what decides: it is bake's own analysis of what the snapshot needs
  from a cohort, and a snapshot that needs nothing the cohort lacks is
  publishable to it regardless of what else changed. There is no
  semantic override. What the check cannot see — a source whose name
  and shape are unchanged but whose meaning moved — is the one gap,
  and it is the developer's, named in the table as "same name, same
  shape, new native code: cohort `S` will run the old code."
- **The dev classifier is new work.** r1 claimed `dev.mjs` classifies
  a rebuild by path. It does not: every watched change runs the whole
  wasm build and pushes `{rebuilt}` (`dev.mjs:104–149`); it does not
  watch `app.ts`, images, fonts, or a manifest, and its clients get one
  event with no capability handshake. 1030.000 stage 3 builds the
  classifier; until then the fast loop uses a conservative path
  classifier (a change under `data/` or `gpu/` is a module change;
  anything else under a watched crate is `{rebuilt}`) and the exact
  digest form runs after producers finish, since digests exist only
  once artifacts do.

**Implementation (2026-09-05).** The actual app build writes `compat.json`
and `artifacts.json` beside its plan. `scripts/app.mjs` completes an outer
receipt after Cargo succeeds, using selected compiler units' dep-info,
loaded/generated files, declared build-script inputs and environment,
native link inputs, and the Swift package's selected source composition.
Binary input identity excludes the replaceable plan/receipt/static bytes;
compiled product hashes are packaging evidence, not a source-delta proxy.
Dev and deploy call the same `classifyArtifacts` comparison. Unbuilt native
targets are reported as unbuilt; neither caller reconstructs targets or
grants through `contract compat`.

The signed stream envelope retains its cohort's actual compatibility
inputs, binary input digest, source parameter/result shapes, and canvas
call names/arities. The latter two are the complete baked plan's prior
demand, including deferred templates and branches; they are **not** a new
registry or a claim to enumerate every unused export in native code. A
candidate demand absent from that receipt is conservatively refused by
name. Matching demands retain that cohort's old implementation. Shader
assets additionally require their reflected interfaces, and publication
requires a signing key that the frozen cohort carries. Existing streams
without authenticated capability evidence require a binary decision; an
unsigned audit wrapper cannot grant capabilities.

An update-capable production classification build precedes publication's assigned sequence.
It explicitly marks `embedded.analysis: true`, with no embedded sequence,
and all native initial-boot/preparation gates refuse it. Receipt inspection
remains available. Release packaging still uses the authenticated publisher
receipt (or the existing explicit genesis path); classification artifacts
cannot silently fall back to an updater-free running app.

In dev the table reduces to what a connected client is told: "restart
with carry," "module change," "asset," or "rebuild the native host" —
and a dev client that did not opt into `Swappable` is told the last of
these for a Rust edit, exactly as a release cohort would be told "a
binary." The dev loop never pretends a layer is lighter than it is in
production.

### D4 — Two axes, not three rungs: an update store, and the executors linked

r1's Level 0/A/B was one ladder for two facts, and the default app
fell through it: the default mixed app carries TypeScript through the
bundle, which r1's rule 4 allowed only at Level B while D4 called the
default Level A. The facts are orthogonal and are already recorded in
different places:

- **`L` — is there an update store?** `0`: the updater crate is not
  linked at all — no store, no check, no verification keys, no network
  path; the embedded bundle is the app forever, and every change is a
  binary. `A`: the store, selection, signing, anti-rollback, crash
  fallback, assets by digest (1026 D9–D11). Chosen per platform in the
  manifest; recorded in the compatibility id.
- **`E` — which executors are linked?** From the app's `host!`
  composition (1029 D2): Hermes if there is an `app.ts`; wasmtime if
  the composition names `Swappable`. Recorded in the compatibility id.

What the bundle can carry to a client is then `plan + assets` whenever
`L = A`, plus bytecode iff Hermes ∈ `E`, plus wasm modules iff
wasmtime ∈ `E`. The words "Level B" retire — a preset word for
`L = A` with a nonempty `E` is how D4 and D3 would disagree again — and
LLP 1026 D12 and 1027 D7 are amended to `(L, E)` (§6). The default mixed app is `L = A, E = {Hermes}` and its
TypeScript rides the bundle; Caltrain is `L = A, E = {}`; an opted-in
mixed app is `L = A, E = {Hermes, wasmtime}`; the stripped build is
`L = 0`.

**The heavier mechanism, stated plainly.** An app with no TypeScript
and no wasm has no code the bundle can carry: every logic change is a
binary. An app with TypeScript but no wasm carries its TypeScript, its
plan, and its assets; its Rust changes are a binary. An `L = 0` app
carries nothing. None of these is a failure of the mechanism; each is
a choice the manifest and the composition record and the classifier
reports.

**Implemented 2026-09-04 (Codex, Charlie's principles-audit fix request):**
`exact-apple` and `exact-linux` no longer depend on the updater. The higher
`exact-apple-update` / `exact-linux-update` crates compose store ownership,
selection, verification and transport over the hosts' narrow delivery and
asset-resolver interfaces. The app bake generates its entry from this
platform's `store.L`: the core entry for `0`, the adapter for `A`. There are
no Cargo features on a core crate. Cargo may compile a declared but unused
adapter dependency; L=0's final artifact must omit it, which is established
by artifact inspection, not the dependency declaration alone.

Apple C ABI 4 exposes a nullable delivery function table: L=0 returns null.
`ExactKit` contains only generic lifecycle, presentation and asset-provider seams;
`ExactUpdates` is a separate Swift target owning store selection, commands and
after-paint checks. Two thin app compositions use the same renderer. The shared
Apple build reads the actual Cargo bake output for its target and selects the
matching Swift graph (`embedded` or `updating`), in separate build directories;
it never infers the choice from the development machine or a runtime flag. The runner's delivery value and the debug
dev connection remain available. Linux takes an optional adapter; its
selected-asset resolver preserves complete-roster tombstones and boot-time
integrity fallback without a dependency on the store. L=0 compatibility
metadata contains no verification keys, trust epoch, store codec or accepted
bundle kinds, and no update origin. The publisher emits a binary row for it
without discovering or reading retired streams when all selected platforms are
L=0; in a mixed run, retired records identified as an L=0 platform are omitted.
Binary publication remains the separate unbuilt
verb described in 1030.000 §6.

### D5 — Propagation: what reaches whom, when, and the aids that need no service

An update does not reach every client at once, and some clients it
never reaches. The developer is owed an honest model and the few
levers that exist without a server, at their real strength.

**Reach by carrier.**

- **Restart with carry** reaches every connected dev client
  immediately.
- **The origin** reaches a browser on its next fresh load. An open
  page, an installed PWA's shell metadata, and Apple's association
  cache are not "instant"; cache policy (D1) is what bounds them.
- **The bundle** reaches a client when that client next runs its
  check (after first pixel, 1026 D11) and applies at its next launch
  unless the app activates it sooner (D7). Minutes for an active user;
  days for an occasional one; never for a client that never launches
  again. **The tail is unbounded**, not "weeks": automatic updates off,
  a managed device frozen at a version, an app never opened again.
- **The binary through a store** reaches a client after review and
  the user's own update cadence; **through a side channel** —
  TestFlight, a notarized `.app`, a Linux package — in hours, to a
  small audience. A published download is not an installation: the
  installed shell (a `.desktop` file, a PWA) is the package manager's
  or the browser's to refresh.

**Live streams.** A binary change that moves the compatibility id
leaves every client that has not updated in the old cohort. From that
moment the developer publishes to two streams, and the classifier
computes each. The live list is explicit in the deploy configuration;
a cohort is retired by the developer, and the store's own analytics
is where they learn when it is empty. There is no negotiation and no
inference.

**Aids that stay within "static files, no service," each at its real
strength:**

- **A check the OS runs in the background** — `BGAppRefreshTask` on
  iOS, a timer while the app runs on macOS and Linux — runs the same
  check against the same static envelope and stages the bundle. On
  iOS it is opportunistic: quota-limited, off in Low Power Mode, off
  when the user disabled Background App Refresh or force-quit the app,
  and it needs the `fetch` background mode and a permitted task
  identifier in `Info.plist`, which is host metadata — an app that
  shipped without them cannot gain it from a bundle. It is a modest
  accelerator of the after-first-pixel check, not a push.
- **A push the developer already has.** exact2 sends no pushes and
  relays none. If the app has its own, a silent push is a nudge to run
  `check` now, and the host exposes that call to the app's native
  side.
- **`check` and `activate` are two acts** (D7). The app reads that an
  update is staged and chooses the moment to activate it — a screen
  boundary, an idle minute, a "restart to update" row it renders
  itself — the same on every host.
- **The sunset card** — `"sunset": { "message", "store" }` in a
  stream's envelope — is an **advisory retirement notice**. A client
  whose binary contains the reader and still checks shows the message
  and the link; nothing is forced. It cannot reach `L = 0`, a client
  that never checks, or a binary older than the reader. Forced update
  for those is the store, or nothing.
- **Reach, measured from what exists.** A client's envelope request
  carries its stream and selected `seq` in headers; the origin's
  access log then gives **request-weighted recent activity** per
  stream, not a census of installs — and `exact reach` (1030.000 D6)
  says so, prints "unknown" where the host does not log headers, and
  keeps no addresses.

**Compatibility windows are the developer's, and the classifier makes
them visible.** A change to a server API the app talks to is not a
delivery layer, but the model says what it means: cohorts on old
streams keep calling the old way for the whole tail, and the developer
keeps the old way working or retires the cohort. Expansion before
contraction — add the new, wait out the tail, remove the old — is the
shape for a universal-link domain, a trust key, a native capability,
and an API alike. These are **named procedures the developer runs in
order**, each step a deploy the table shows, not an orchestrator that
sequences carriers on its own; the common case is independent rows,
and the saga is not v1.

### D6 — What turns out not to be a layer

- **Feature flags and remote config.** A resource answered by a fetch
  under the grants — app logic, not delivery.
- **Server-side anything.** The origin serves files bake wrote; the
  API the app calls is the app's business (1023 §8, 1026 §7).
- **The agent carrier.** Off the network, on every host (LLP 1012).
- **The external control plane** — App ID capabilities, provisioning
  profiles, APNs keys, store agreements, export compliance, privacy
  answers, review status. Not artifacts; **inputs the binary pipeline
  depends on**, recorded in the build receipt (D2) and gating a
  submission (1030.000). Not managed here.
- **The store's console** — screenshots, descriptions, ratings,
  pricing.
- **Widgets, extensions, watch apps, helpers.** Separate signed targets
  with binary in every column; rows in D1 when an app has one.
- **Acquisition.** Publishing a downloadable binary is not updating an
  installation; that is the package manager's or the store's.
- **A "version string."** Every identity here is a digest or a derived
  key; the version and build numbers exist because stores require
  them, are generated by bake from a monotonic counter, and decide
  nothing except whether a store accepts the upload.

### D7 — What the app can read and do, and what the agent sees

`state` (LLP 1012) is the agent's diagnostic surface; a Contract app
cannot read it. So the app-facing half is a **resource and a command**
the runner answers itself, on every host the same way:

- `delivery` — a resource: the stream, the selected and embedded
  `seq`, whether an update is staged, the sunset card if any, which
  sources are interpreted. Rendered however the app likes.
- `delivery.check` and `delivery.activate` — commands: run the check
  now; apply the staged bundle now with carry.

`state.delivery` **mirrors** the resource for smoke tests, `metrics.
mjs`, and a developer's eyes, and adds what the agent alone needs —
the compatibility id, `L` and `E`, the entry-zero digest. No ninth
agent operation (LLP 1012's eight, and `rules/DEFERRED.md`). An
`L = 0` client answers
`delivery` with its embedded entry and nothing staged, which is the
honest statement that it cannot be told anything.

**Landed 2026-09-04** (the runner's `Delivery`, `runner/src/delivery.rs`;
1030.000 §4 stage 3). Two spellings differ from the above and the
code forced both: a Contract identifier holds no dot, so the source is
`exactDelivery` and the commands are `deliveryCheck` and
`deliveryActivate`; they are ordinary commands and reach the host by
name. The runner answers the resource before the data seam, from the
binary's `compat.json` handed in at boot before the first batch;
`state.delivery` mirrors it and adds the compatibility id, `L`, and
`E`. The entry-zero digest and everything the store has to say (the
stream, `seq`, staged, the sunset) still read as the embedded answer
until the store is wired into the hosts (stage 4).

### D8 — GPU: three sub-layers, and the interface digest — once shaders are assets

Charlie's addition: WebGPU, shaders, WGSL, "and anything like that."
The GPU is three layers with three carriers, and the panel corrected
r1 on where the first one is today.

- **WGSL source.** LLP 1009 D5 validates shaders at build and compiles
  them at first use — **from the crate**: `gpu/reflect` embeds each
  shader's full text as `SOURCE` in generated Rust (`gpu/reflect/src/
  lib.rs:195–199`), so every WGSL edit changes compiled Rust and today
  a shader is part of the binary. The hot path is real but has a
  **packaging prerequisite**: shaders as digest-addressed assets loaded
  at `bind`, `SOURCE` out of the dylib, the reflection kept for the
  interface — **ruled 2026-09-03 into 1030.000 stage 1**, so hot
  shaders arrive with the asset row. After that, in dev a `.wgsl` edit
  is one `{seq}` and the surface builds a candidate pipeline and swaps
  it in only if it validates; in production it rides the bundle by
  digest. (Landed: the packaging 2026-09-03, the asset row 2026-09-04 —
  a `.wgsl` edit is one `{seq}` on the web and macOS, refused by name
  when its interface moved; 1030.000 §4 stage 1.)
- **The surface's Rust** — pipeline layout, bind groups, vertex
  formats, draw logic. A binary (1026 D6 off staging, 1029 §6).
- **The GPU module** — wgpu, `exact-gpu`, the loader. exact2 itself; a
  binary and a new compatibility id.

**The interface digest, defined.** Hashing `reflect()`'s output would
move on every color edit, because the output carries `SOURCE`. The
digest is over the **generated interface minus the source text**:
entry points and their stage IO, bindings and group layouts, host-
shareable structs, vertex inputs, compute workgroup sizes (`lib.rs:
210–232`), and — to be added to the reflection — `override`
constants, fragment outputs, and required device features and limits.
Its fixture is a set of shader edits known to be compatible and known
to be incompatible, checked on the three real backends — the browser,
Metal, Vulkan — since a candidate pipeline that validates on one may
not on another. A shader edit whose interface digest is unchanged is
an asset change;
one that changes it is a binary change, because the surface's Rust
that binds it is in the binary. The GPU crate must take its layouts
from the generated descriptors only, never by hand, or the digest
does not describe what the binary binds. Until the packaging lands,
WGSL is binary-coupled and the classifier says so.

**"Anything like that."** Textures, meshes, lookup tables are assets.
A compute shader is a shader. Pipeline caches are the host's and are
rebuilt on a digest change. A new surface (a new `canvas` arity in the
roster, 1009) is a binary, as a new native module is.

### D9 — When the runtime itself changes: what stays constant, what may change, how it migrates

Charlie's second addition: the developer upgrades exact2 and everything
that comes with it. This touches every row of D1 at once, and the
theory has to say what it promises — no more than the mechanisms give.

**What may change** on an exact2 upgrade, pre-1.0 (`rules/DEFERRED.
md` §Deliberately worse: no backwards compatibility, no API stability,
names change): the kernel schema, the plan format, the Contract
language and its compiler, the runner's semantics, the host presenters,
the module ABI, the TypeScript seam, the engine receipts, the native-
module table ABI, the agent API, the store's canonical encoding. The
compatibility id is derived from most of these, so an upgrade that
touches any of them moves it, and an upgrade that touches none — a
host bug fix, a faster painter — does not: the new binary keeps its
compatibility id and its streams. That case needs no rule.

**What is guaranteed to remain constant** across an upgrade:

1. **The app's identity** — `app_id` (1023 D5), the bundle id, the
   origin URL — and the stream path each cohort was baked with.
2. **A valid update can never destroy the embedded fallback, on an OS
   that still runs the binary.** Entry zero is never deleted and always
   boots (1026 D9); bake never
   publishes a snapshot a cohort cannot run (D3); a client refuses a
   stream that is not its own by path, before download. That is the
   whole promise — not "works forever," which no binary can promise
   against an OS, a certificate, or a store policy that moves.
3. **Durable state, at the strength the mechanisms give.** A carried
   slot and a kept resource answer survive where their shape still
   conforms and start fresh where it does not (1007 §6, 1027 D4) —
   and starting fresh is **data loss**, said loudly: pre-1.0 a shape
   change can cost the user their last station, and there are no
   migration scripts. Secrets survive where the Keychain's own rule
   holds — the same team, signature, and access group (1018 D7), not
   merely the same `app_id`. The plain persisted tier of 1018 is
   unbuilt and Linux is memory-only, so nothing more is promised for
   them. The updater's own record is versioned separately and frozen
   per compatibility id (D1) — an unknown major selects entry zero —
   so app data and updater state never share a codec change.
4. **Digests mean what they meant.** Payload integrity is raw SHA-256
   over bytes (1023 D2; `dev.mjs:84`); format identity is the domain-
   separated digest of `plan/build.rs`; neither is redefined by an
   upgrade, so a new bake's digests compare with an old one's.
5. **The envelope's major** (`exact: 1`) and its additive rule (1023
   D2). The signed form is versioned the same way.

**How it migrates.** An exact2 upgrade is a binary change that moves
the compatibility id from `V` to `V'`. **Freeze is the only v1
policy**: `V`'s cohort stays on the last snapshot published to its
stream; new snapshots go to `V'`; when the developer retires `V` its
envelope gains the sunset card. This is the policy the binding rules
allow — dual-publishing to `V` needs `V`'s toolchain, and a pinned
old toolchain compiling a source that must avoid new features is a
legacy branch (`rules/RULES.md` §Scope, "no legacy branches before
1.0"), and a commit alone does not even reproduce it (Rust, Node,
Hermes, wasmtime configuration, SDKs, dependencies). Two later doors,
named and not built: an **emergency backport** — one explicit snapshot
to a frozen stream, baked from a content-addressed toolchain capsule
with its own lock and the old app source, by the current publisher
holding the credentials — and general dual-publishing, a 1.0
conversation.

**The migration test.** Before the first snapshot for `V'` is
published, the fixtures that exist run under `V'`: the byte-equality
suite for every source under every executor, the smoke drive on every
host, and — added here — the **store-carry test**: boot `V'`'s binary
over a store written by `V`'s, and assert every value that still
conforms is kept, every value that does not starts fresh with no
refusal, the secrets are readable, a crash marker written by `V` is
honored, and a downgrade to `V` is refused. It runs in the minutes-
loop on the fleet, not in the gate.

**What this does not promise, pre-1.0:** that a Contract written for
`V` compiles unchanged under `V'`, that a native module built against
`V`'s table ABI loads under `V'`, that a TypeScript module's
`app.d.ts` is the same. Each is caught at bake, on the developer's
machine, before anything is published — the only place a pre-1.0
break is acceptable.

### D10 — Icons, images, names: live in dev, and live in production only where the platform says so

Charlie's third addition: images and icons change during development
too, and app icons and metadata "can be updated live on some platforms
and not on others."

**In dev, every asset is hot** — once the asset row exists (1030.000
stage 1). An edit to an image, a font, a deck page, a strings file, or
(after D8's packaging) a shader changes one digest; `dev.mjs` pushes
`{seq}` naming it; the host fetches the new bytes by digest and
re-renders what referenced it — an `img` repaints, a declared font
re-registers (1019), a surface builds its candidate pipeline —
carrying state as any `{seq}` does. Today assets are bundled-only on
native and an image edit is a rebuild; that is the gap the first stage
closes, because it is the one every designer hits on day one.
(Closed 2026-09-04: 1030.000 stage 1 landed — its §4 says what was
driven and what was not.)

**In production, the platform decides, per item.** The manifest (D2)
declares each once; bake generates per platform; the classifier
reports per platform:

| Item | Web | Linux | macOS | iOS |
|---|---|---|---|---|
| Images, fonts, strings, shaders (after D8), deck pages in the app | bundle / origin | bundle | bundle | bundle |
| App icon | the favicon and `manifest.json` are origin files; an **installed PWA's** icon is the browser's cached copy | a running app's window icon is live; the **installed** `.desktop` icon is package-managed | the Dock icon can be set while running; the Finder icon is the bundle's: **binary** | **switch among icons shipped in the binary** (`setAlternateIconName`): asynchronous, foreground only, and iOS shows a system alert the app cannot suppress — so it is an explicit act, taken only when the selected name changes, never on every boot. A new icon, and the dark and tinted appearances, are asset-catalog slots: **binary** |
| Display name | `<title>` live; the installed PWA's name is cached | `.desktop`: package-managed | binary | binary; localized names are `InfoPlist.strings`, binary |
| Launch screen / splash | origin; a PWA's splash is cached | n/a | n/a | binary, and the OS caches it |
| Permission strings, URL schemes, background modes, entitlements | the association file is an origin file; the **entitlement** that claims a domain is binary | n/a | binary | binary |
| Store listing — screenshots, description, rating | not an artifact (D6) | — | store console | store console |

Two consequences the theory owes:

- **"Switch among shipped" is a real carrier on iOS**, and it is the
  bundle's: an app that ships three icons can select one from the plan
  (a host-visible property the Contract sets, honored on iOS by the
  alternate-icon API with its alert, on the web by the manifest), and
  the selection travels with the plan. The shipped roster is in the
  compatibility id (D3a), so a plan that selects an icon a cohort did
  not ship is refused for that stream by the classifier, not
  discovered on a phone.
- **A platform that cannot take a change live is not made to look as
  if it can.** The classifier's per-platform row is the truth ("icon:
  origin live, installed PWA cached, iOS binary"), and the dev loop
  shows the same truth: an icon edit hot-reloads the web page's favicon
  and tells the iOS dev client "rebuild the native host." One source,
  one manifest, per-platform carriers, no override files
  (`rules/DEFERRED.md` §Authoring models).

## 3. Costs and budgets

- **Boot.** Unchanged from 1026 D9: selection is a stat and a read;
  the check is after first pixel; an `L = 0` binary does neither.
- **The classifier** runs at bake over a few dozen artifacts and,
  in the exact form, only after producers finish; the fast dev loop
  keeps its conservative path form. Not on any budget.
- **The manifest** (D2) adds one generated step to `build.mjs` and
  removes the hand-written `Info.plist` and entitlements it carries.
- **Two live streams** double the static directory and nothing else.
- **Checks.** None added; the store-carry test is in the minutes-loop.

## 4. Staging

This document is a theory and an inventory; it is built through
1030.000's stages. What lands under this number directly, each
verifiable by driving an app: the inventory as a table `metrics.mjs`
can read, the `delivery` resource and its two commands with
`state.delivery` mirroring them, and the sunset card in the envelope.

## 5. Open questions (for Charlie)

1. **Is `L = 0` a distinct link-time profile** — the updater crate,
   its keys, and its store not linked — rather than Level A with the
   check disabled? Both panelists: yes. Leaning: yes; "stripped" is a
   promise about the binary. **Ruled 2026-09-03: "omit. smaller."**
2. **The publish unit.** Dependency-closed snapshots per stream,
   computed by bake, with no semantic override (D3 as revised)? Both
   panelists rejected r1's whole-Δ rule and its override; sol asked
   for ordered carrier actions, grok for per-`(platform, V)` atomicity
   with no override beyond bake's checks. The revision is both.
   **Ruled 2026-09-03: confirmed.**
3. **The sunset card in v1**, named an advisory retirement notice that
   reaches only `L = A` clients with the reader? Both: yes. **Ruled
   2026-09-03: yes.**
4. **`delivery` as a resource and two commands** for the app, with
   `state.delivery` as the agent's mirror (D7)? Sol's correction of
   r1, which had put it on `state` alone. Leaning: yes. **Ruled
   2026-09-03: yes.**
5. **Freeze as the only v1 upgrade policy** (D9), with the emergency
   backport and dual-publishing as named later doors? Both: yes.
   **Ruled 2026-09-03: yes ("seems reasonable").**
6. **Shaders: binary-coupled until packaged** (D8), with the interface
   digest defined as the generated interface minus source? Both: yes,
   with the packaging first. **Open — Charlie asked "what is
   alternative?" (2026-09-03).** The alternatives, in order of how
   much they give: (a) **do the packaging in 1030.000 stage 1**, so a
   shader edit is hot from the first day — `SOURCE` leaves the
   generated Rust, the GPU crate loads WGSL by digest at `bind`, the
   reflection stays for the interface digest; a small GPU-crate lane,
   and what grok's round 2 recommended; (b) **defer the packaging**
   and live with today: every WGSL edit is a rebuild on every host,
   shader hot reload arrives whenever someone picks the lane up; (c)
   **package without the interface check**, treating every WGSL edit
   as an asset — refused, because a binding change would then reach a
   binary that binds the old layout and fail at `bind`. The
   recommendation is (a): the packaging is small and it is the only
   thing between the theory and hot shaders. **Ruled 2026-09-03: (a)
   ("do your recs") — the packaging is in 1030.000 stage 1.**
7. **The data crate target-neutral by construction** (D3a) — bake
   refuses `cfg(target_*)` and native dependencies in `data/` — so
   its source digest can stand in the compatibility id? New in r2.
   Leaning: yes; the crate already depends on `exact-runner` alone.
   **Ruled 2026-09-03: yes.**

## 6. What this amends, at acceptance

- **LLP 1026 D9** — the runtime version becomes the compatibility id
  (D3a), per platform; **D12** — Level A/B become `(L, E)` (D4) and
  "Level B in the dev client regardless" is 1029's question; **D11** —
  the key in the binary is the verification key, and the signed head
  binds app, channel, compatibility id, `seq`, and key id (D3a); key
  ids and rotation overlap are owed there too; **D4** — the module
  card is per stream and per target (1029 D3).
- **LLP 1027 D7** — "Level B is free where Level A was" becomes "the
  bundle carries bytecode because Hermes ∈ `E`."
- **LLP 1029 D4** — per-source identity waits for a closure algorithm
  and provenance metadata; whole-module first (D3a).
- **`rules/DEFERRED.md`** — the update-economy line was struck
  2026-09-03 on Charlie's ruling, with 1026 §8's reason and take; this
  document moves nothing further.

## 7. What this refuses

- **A service.** No per-user targeting, no cohorts by identity, no
  update console, no analytics beyond a log the developer owns
  (1026 §7, `rules/DEFERRED.md`).
- **Native machine code to iOS**, by any carrier. Pulley or wasm
  bytecode only; bake refuses the artifact.
- **Pretending a platform can take a change live when it cannot**
  (D10), and pretending a carrier reaches clients it cannot (D5).
- **A semantic override** of the classifier (D3).
- **Dual-publishing before 1.0** (D9).
- **A ninth agent operation** (D7).
- **Migration scripts for app state** before 1.0 (D9): conform or
  start fresh, said loudly.

## 8. Panel fold (r2, 2026-09-03)

Two rounds; round 1 blind, round 2 mutually visible. The artifacts
carry both rounds verbatim. What each catch did to this document:

| Catch | Who | Held? | Where it landed |
|---|---|---|---|
| Level A/B contradicts the mixed default; collapse to `(L, E)` | grok B1, sol #3 | yes — a TypeScript-only Δ on the default app classified as a binary under r1's rules | D4, §6 |
| The classifier is not per platform; an icon edit is "a binary" on the web; the web has no runtime version; association files are origin files | grok B2, sol #4/#6 | yes | D3 (origin row, per-`(P, S)` rows) |
| `runtimeVersion` does not identify the cohort; two binaries, same version, different native Rust | sol #2 | yes | D3a |
| Whole-Δ atomicity is over-conservative and the override is unverifiable; compute dependency from the graph | sol #4, grok B2 | yes | D3 |
| `dev.mjs` has no path classifier; r1 cited 1026 D4 as landed | grok M1, sol #5 | yes — `dev.mjs:104–149` | D3, §1 |
| Shaders are embedded (`SOURCE`); the interface digest must exclude source text; packaging first | grok M2, sol #13 | yes — `lib.rs:195–199` | D8, D1 |
| Dual-publish is a legacy branch before 1.0; a commit is not a toolchain | grok M3, sol #18 | yes | D9 |
| The wasm digest is not the native crate's identity (`cfg(target_*)`) | sol #12 | yes | D3a (target-neutral crate; source digest) |
| Durable-state guarantees exceed the mechanisms; "works forever" | sol #11 | yes | D9 items 2–3 |
| Cache policy is a layer | grok M4 | yes | D1 |
| The binary holds verification keys, not "the signing key"; rotation | sol #7 | yes; rotation overlap owed | D1, D9 |
| The manifest conflates identity and policy | sol #8 | yes | D2 |
| `state` is agent-only; the app needs a resource and commands | sol #14 | yes | D7 |
| BGAppRefreshTask oversold; check vs activate; sunset advisory; reach is not a census | grok M6/M10, sol #14 | yes | D5 |
| Alternate icons: system alert, foreground, async; roster in the cohort | grok M7, sol #15 | yes | D10, D3a |
| AASA cached ~a day; the domain entitlement is binary; assetlinks needs the Play signing cert | grok M8, sol #15 | yes | D1, D5, D10 |
| Native `cwasm` never to iOS; macOS store risk | grok M9, sol #10 | yes | D1, §7 |
| Missing D1 rows (entitlements, privacy manifest, symbols, sandbox, icon appearances, localized names, groups, version, grant ceiling, updater codec, distribution artifacts, external control plane) | grok M14, sol #17 | yes, as rows or D6 entries | D1, D6 |
| Doubled host-metadata rows | grok M14 | yes | D1 |
| r1 §1 "none of that changes" over unbuilt foundations; DEFERRED still lists the update economy | sol #1 | yes | §1, §6 |
| Summary promised a §7 that did not exist | grok m1 | yes | §7 |
| The tail is unbounded, not weeks; TestFlight "an hour" is internal only | grok m3/m4 | yes | D5, 1030.000 |
| Widgets/extensions as separate targets; Android as a whole lane | sol #16/#17 | yes | D6, 1030.000 |
| 1030.000 is several LLPs; cut to the bundle slice | grok M15, sol suggestion 2 | yes | 1030.000 r2 |

### Round 2

Both panelists read each other and converged; grok retracted its
whole-Δ atomicity rule in sol's favor, sol's citations were corrected
where grok showed them wrong, and no disagreement remains between the
two families. Final positions and what each did here:

| Question | Final position (both) | Here |
|---|---|---|
| Level B as a word | delete it as an authority; `(L, E)` only; no "preset" | D4 |
| Publish unit | dependency-closed snapshot per `(platform, compatibility id)` plus an origin row; no override; ordered migrations are procedures, not an orchestrator | D3, D5 |
| Cohort key | the compatibility id, which must not fork on a painter-only host fix; `seq` per stream at the origin; a release id across streams | D3a |
| One process or two | one library, two processes; the LAN process never signs or uploads | 1030.000 D5 |
| `release` default | `internal`, both; "the wrong default" if `automatic` — Charlie's word, recorded as such. **Ruled 2026-09-03: continuous everywhere**, with the store's own phased rollout as the default mitigation; the panel's reasoning is recorded beside the ruling | 1030.000 D4, §5 Q1 |
| Durable state | item 3 as narrowed; item 2 "on a supported OS"; the fixture is the guarantee | D9 |
| Shader digest | packaging first; then the generated interface minus source, plus overrides, fragment outputs, device features; validate-before-replace is the apply rule, not a layer | D8 |
| Apple lanes | four, named, not built here; the Mac App Store's acceptance of a wasmtime artifact is unverified | 1030.000 §6, D1 |
| Dual-publish | not v1; a commit is not a toolchain capsule | D9 |
| Secrets | one custody system, separate named credentials, an isolated signer | 1030.000 D7 |
| How much of 1030.000 | stages 1–4; the rest a later number | 1030.000 r2 |

Where the two still differ in shape, not in verdict: sol wants the
classifier to emit an **ordered deployment DAG** across origin, bundle,
binary, and external actions; grok wants ordered migrations to be
**procedures the developer runs**, with the table showing each step.
r2 takes grok's for v1 (D5) and names sol's as the shape if a saga is
ever built. Sol's round-2 additions, all taken: `notarytool` takes a
zipped `.app`, a `.dmg`, or a `.pkg`, never a bare bundle (1030.000
§6); Safari can open a cross-domain universal link and same-domain
navigation stays in Safari (1030.000 D2); the signed head must bind
app, channel, stream, `seq`, and key id, which 1026 D11 does not (D3a,
§6); a head made visible before its blobs reach every CDN edge is the
mid-publish case again, so the publisher reads the blobs back before
writing the head and a client that misses a blob keeps its last good
head (1030.000 D3); the agent-operation count is LLP 1012's and
`DEFERRED.md`'s, not 1024's (D7); the privacy manifest is derived
from the APIs used, not a fixed row (D1); symbols are release outputs
in 1030.000, not a client layer (D1 says so); shader fixtures run on
the three real backends (D8).

Grok's corrections of sol, taken: a committed lockfile is not barred
by the generated-file rule (`Cargo.lock` is committed) — the case
against `deploy.lock` is operational and 1030.000 D3 now says so;
canonical signed bytes exist in 1026 D11 and r1's error was the D1 row
that said "signing key" for a public key; 1026 D11 already had a
directory per version, and r1's 1030.000 D2 contradicted it in one
sentence rather than lacking a path. Sol's catches grok missed, all
folded in r2: the key wording, `[deploy]` inside identity, the digest
citation in D9, the one-`V`-three-levels example, `state` being agent-
only, the grant-ceiling row, the wasm-versus-native identity, four
lanes, the debounce and TOCTOU, the icon alert, request-weighted reach,
the launcher's domain, installed-shell metadata, the host descriptor,
snapback still on `DEFERRED.md`.

Both recommend: revise and stay Draft; do not gather another family;
Charlie answers `release`'s default, freeze-only, and the process
split; the wasmtime-entitlement afternoon, a real-device associated-
domains run, and a concurrent-publish spike gate a later binary-lane
document, not this one.

## Ratification note

Draft r2, 2026-09-03, r1 and r2 the same day; the panel's two rounds
are folded and its artifacts carry both verbatim. The inventory is as
complete as the panel and the author could make it, and is not called
complete: a layer missing from D1 is a bug in this document. Nothing
measured beyond LLP 1028; nothing in the repo changes until Charlie
answers §5 and an implementer and a date are named against 1030.000's
stages. The update economy left `rules/DEFERRED.md` 2026-09-03, in
its minimal static form, on Charlie's ruling.
