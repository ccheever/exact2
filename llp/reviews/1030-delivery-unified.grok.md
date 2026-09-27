# Panel: LLP 1030 Delivery, unified + LLP 1030.000 The dev server as the deployer (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`. Reported model not independently verifiable from the CLI; requested `grok-4.6` at `xhigh`.
- **Method:** a panel at Charlie's request, 2026-09-03 — "do a panel discussion with grok 4.6 xhigh and sol ultra about them and see if that influences your thoughts at all, and update as appropriate." Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind** (neither panelist saw the other), **round 2 mutually visible** (each read the other's round-1 answer in full and wrote only where they differ, correct, or extend; the named disagreements were forced to a final position). The author (Claude Fable 5.1, this session) wrote the documents and is not a reviewer. Capsule: a read-only copy of LLP 1028, 1029, 1030 r1, 1030.000 r1 and the context LLPs 1000, 1007, 1009, 1012, 1018, 1023, 1024, 1026, 1027, `rules/RULES.md`, `rules/DEFERRED.md`, `CLAUDE.md`, `QUEUE.md`, `host/web/dev.mjs`, `host/apple/build.mjs`, `gpu/reflect/src/lib.rs`, `scripts/app.mjs`; no network; no repository access. Panelists' narration lines before the document marker are preserved verbatim.
- **Gates (round 1):** sha256 —
```
a57e18881f326d035a77c742283608fda80b7a5cf639ad23d643667817dec159  capsule/llp/1030-delivery-unified.rfc.md
410b96bf0760271bc15f80833e38d06010dd7bfa995aa799f9ce6a9ef47a5279  capsule/llp/1030.000-dev-server-as-deployer.rfc.md
8e5f9c9e5e874c73d4aba1b086fbc072ed37c5458ed56627f51179caece6a641  capsule/llp/1029-mixed-logic-and-engine-choice.rfc.md
8e31902d501d0dab8656b017eed80e476b60eb6bc30fa2c7bf9157fb7e75da07  capsule/llp/1028-wasm-engines-measured.research.md
b4d9eb5c57cb66dcd854ef75af58a04435bcc995a529099de32b808a423925d7  brief-round1.md
1ae7060d7dd2fcbb6a816f9f51dcf7a0f2f69e84980869e1edf63a7809e9efb4  -
```
- **Gates (round 2):** the same capsule plus `round1/{grok,sol}.md`; the round-2 briefs and the capsule —
```
7bd63773d5d11547b8ecd53fe1914793fa95c9d48f6d52935c953c4a43914036  brief-round2-grok.md
4eb984d2883bf06ed63815116e4c846429031aaa3fa2e8cb9d48d32b461d7410  brief-round2-sol.md
62829f17ce5fd55eeef3d142c142b3209278d850b2727061f70745b746e765b5  capsule/round1/grok.md
d578cb9652edd07c2601a5dd1f26313c16986a2c2926dfd2264d271129f7c848  capsule/round1/sol.md
21190781c82f714938c5eb295595e4e4e44c1788d1ebe86dccb24902b8ed8272  -
```
- **Runs (UTC):**
```
grok r1 start 2026-09-03T10:40:45Z
sol r1 start 2026-09-03T10:40:45Z
grok r1 end 2026-09-03T11:15:54Z rc=0
sol r1 end 2026-09-03T11:33:35Z rc=0
grok r2 start 2026-09-03T11:34:36Z
sol r2 start 2026-09-03T11:34:36Z
grok r2 end 2026-09-03T11:42:57Z rc=0
sol r2 end 2026-09-03T11:44:16Z rc=0
```
- **Peer:** codex `gpt-5.6-sol` at `ultra` (OpenAI Codex CLI 0.153.0), same capsule, same rounds.
- **Disposition:** folded into LLP 1030 r2 and 1030.000 r2 the same day — 1030 §8 records which catches held and where each landed; the author re-verified every code claim before folding (`dev.mjs:104–149` rebuilds whole on any watched change with no path classification; `gpu/reflect/src/lib.rs:195–199` embeds `SOURCE`; `build.mjs:155–184, 285–286, 303–320` signs with a development identity, `get-task-allow`, no archive, no notarization, no macOS `Info.plist`). Round-2 dispositions are in 1030 §8 ("Round 2"); both families converged and neither recommends another family.

---

## Round 1 — position (verbatim)

I'll start by reading the review skill and the two primary RFCs, then the supporting docs and code they rest on.Next I’ll read the implementation RFC and the same-day context docs the two primaries rest on.I’ll now check the cited context LLPs and the code those RFCs claim about.I’ll pull the remaining cited sections and the code files the RFCs rest their claims on.I’ll check the Apple build, the web-host spec, and the GPU/native-module RFCs against the claims.The theory is right; the classifier and the implementation RFC are not yet something I would build as written. Below is the round-1 review.

---

# Panel review — LLP 1030 / 1030.000 (round 1, Grok 4.6)

Provenance: Grok 4.6 · xAI · 2026-09-03 · not redacted · independent family review of Draft r1. Did not author the documents.

## 1. Overall assessment

The core sentence is right: an app is a set of digested artifacts; a change is the digests that differ; each artifact has one lightest carrier; development and production are the same act under two policies. That is the correct generalization of LLP 1026’s embedded-plus-update model across the inventory Charlie named, and D5/D9/D10 are the parts 1026 did not have.

It is not yet a complete theory. The Level A/B ladder contradicts the mixed default and the classifier; the classifier is not actually per-platform and would freeze the web when iOS needs a binary; several “already true” claims about `dev.mjs` and shaders are false of the code; D1 double-counts host metadata and splits associated domains from their entitlement.

I would not build 1030.000 as written. I would revise 1030 until the classifier and the level axes are something you can implement without a second document to undo them, then build a narrowed 1030.000: asset row, manifest, classifier dry-run, bundle publish. The one verb is not crazy. Hanging App Store submission off the watch loop with `release: automatic` as the default is.

## 2. Strengths

- **The theory sentence** (1030 Summary, D3 opening). Digest identity, lightest carrier, two policies. This is the right shape. Expo’s `runtimeVersion` + embedded update is the precedent; 1026 D10’s native-by-digest is the improvement; 1030 is the inventory that makes both span every layer.
- **D2, host metadata as a baked artifact.** `host/apple/build.mjs` really does this by hand today: iOS `Info.plist` and entitlements are string templates from crate name and team (`build.mjs:155–184`, written at `build.mjs:303, 317`); macOS writes neither; icons, privacy manifest, associated domains, background modes are nowhere. One declared manifest, generate, never edit the output, digest in the binary’s identity — that is the same move as `kernel/tables/schema.json`, and it is the half of “OTA” people learn the hard way.
- **D4 Level 0.** This is what 1023 §8 described as every release binary, restated as a choice rather than a law. Kiosk, brownfield-that-owns-updates, “no network path in the release” are real. The classifier reporting every change to it as a binary is the point.
- **D5 propagation without a service.** Reach-by-carrier, live runtime versions as an explicit list, sunset as a static file, `check now` as a nudge the app’s own push may call, apply as a `Command`. This respects `rules/DEFERRED.md` §Runtime (no service) and 1026 §7. The long-running desktop app that never relaunches is the right design case.
- **D8’s three GPU sub-layers, in principle.** Surface Rust / WGSL / GPU module is the right split. `gpu/reflect` already emits entry points, bindings, host-shareable structs, vertex layouts, and compute workgroup sizes (`gpu/reflect/src/lib.rs:210–232`). An interface digest is a real thing you can hash, not a vibe. Taking 1026 D6 (surfaces as wasm) off staging and keeping surfaces as a binary is the honest 1029 §6 call.
- **D9 constants vs. may-change.** The five constants (app identity, installed binary keeps working, durable state conforms-or-restarts, digests comparable, envelope major additive) are the right freeze surface for a pre-1.0 runtime that otherwise names-change (`rules/DEFERRED.md` §Deliberately worse). Freeze-by-default on upgrade matches the tail. The store-carry test is the one new check worth adding, and putting it on the minutes-loop matches `RULES.md`.
- **D10’s per-platform honesty, as a table.** “A platform that cannot take a change live is not made to look as if it can” is the rule 1030 exists to enforce. iOS `setAlternateIconName` as switch-among-shipped, Finder icon as binary, launch screen as binary-plus-OS-cache — those three are right.
- **1030.000 D1, not Vite.** Correct. After 1027 the TypeScript module is one file, a restart with carry, not ESM HMR (`rules/DEFERRED.md` §Runtime: no hot revision surfaces). Rolldown is already the bake’s bundler. A deck (1020) is where a Vite app lives. Do not reopen this.
- **1030.000 D3 dry-run as the default, one table, atomic static publish** (content-addressed files first, signed `exact.json` last). The table-before-yes is the feature. The mid-publish client seeing only a complete envelope is the 1023 D3 rule applied to production.
- **1030.000 D4 as knobs, not modes.** Continuous / manual / schedule / per-platform / channels-as-origins / Level 0 / brownfield in one configuration is the right answer to “we need to support all flavors.”
- **1030.000 D7 brownfield.** Host owns the binary; exact2 is a library; `brownfield: true` publishes bundles only. That is the only brownfield shape that does not become a second product.
- **`state.delivery` on `state`, not a ninth operation.** Matches LLP 1012 / `DEFERRED.md` §Agent API.

## 3. Concerns

### Blocking

**B1. Level A/B is one ladder for two axes, and the default app falls through a hole.**  
Sections: 1030 D4 table vs. D4 last paragraph vs. 1030.000 D4 `level` default; against 1027 D7 and 1029 D1/D2.

The D4 table says Level A moves `{plan, assets, association}` and Level B moves bytecode and wasm. The next paragraph says the default app is “mixed, not opted in, **Level A on the phone**” and “carries plan, assets, **and TypeScript**.” 1030.000 D4 defaults `level` to “`A` on the phone, `B` where the app named an executor.” 1029 D2’s default composition names `exact_js::Module` — it named an executor. 1027 D7 already restated this the other way: a TypeScript app “always links `exact-js` … Level B is free where Level A was.”

Feed a TypeScript-only Δ through D3 as written with `L = A`: rule 1 no, rule 2 no, rule 3 no (bytecode ∉ `{plan, assets, association}`), rule 4 requires `L = B`, rule 5 → **binary**. The default app Charlie asked for would take a store release to change `app.ts`. That is a failure of the theory’s main job.

The deeper mistake is keeping 1026’s A/B after 1029 split executors into their own axis. D3 already takes `(V, E, L)`. `L` should be “is there an updater?” (`0` or `A`). `E` already decides which code the bundle may carry. “Level B” is `L = A` plus a nonempty `E`. Two authorities for the same fact (the `host!` composition *and* a `level: B` knob) is the bug class this repo exists to not have.

*Resolve:* Delete Level B. `L ∈ {0, A}`. Default mixed is `L = A, E = {hermes}`. Rust-only Caltrain is `L = A, E = {}`. Opted-in mixed is `L = A, E = {hermes, wasmtime}`. Stripped is `L = 0`. The manifest records `L`; the composition records `E`; bake refuses a bundle card the binary cannot run. Restate 1027 D7 in those words.

**B2. The classifier is not per-platform; the atomic rule as stated freezes the web.**  
Sections: 1030 D3 rules 1–5 and the atomic paragraph; D10; 1030.000 D3 example.

Rule 1: if Δ touches any binary layer — including **host metadata** — the carrier is a binary. D10: an icon is live on the web and a binary on iOS. Those two sentences cannot both be the rule. An icon-only edit hits rule 1 and never reaches D10. A Rust-plus-Contract edit is a binary for iOS; the same Δ is a new `app.wasm` for the web, which has **no runtime version** (D1 web row: “every load is current; no runtime version”). The atomic sentence — “nothing is published to `V`” — would refuse the web origin because it shares `U` with the native bundles.

The 1030.000 D3 example shows per-platform icon rows, then “nothing published to a1b2 (atomic)” because Rust changed. It never shows the icon-only case, which is the case that falsifies the five-rule list.

Association is in rule 3’s bundle set, but D1’s production carrier for AASA/`assetlinks.json` is “the static origin.” A native client does not apply AASA from the update store; Apple’s CDN fetches it. Lumping it into the bundle is a category error, and it is the wrong atomicity: publishing a new AASA should not wait on a native binary, and a native binary cannot make the OS notice a new AASA.

*Resolve:* The classifier returns **one carrier per (platform, V)**, plus a separate **origin** carrier for `{web app, AASA, assetlinks, hashed static files}`. Atomicity is per `(platform, V)` for the native bundle, never across the web origin. Rule 1’s “binary layer” is per platform (host metadata is binary on iOS/macOS, live on web). Association is an origin file, not a bundle member. Spell the override: it cannot override bake’s “plan names a source the embedded crate lacks” (1026 D12); it can only assert independence of artifacts the digest graph does not already couple.

**B3. “One URL you can just load” does not work in development as specified.**  
Section: 1030.000 D2.

Universal links require HTTPS, a 200 from `/.well-known/apple-app-site-association` with no redirects, and an `applinks:` entry in the **entitlements** of an installed app. `dev.mjs` is HTTP on the LAN (`dev.mjs:38, 223–234`, `0.0.0.0` unless `--loopback`). Apple will not fetch AASA from `http://192.168.x.x:8765`. Pasting the URL in Safari usually does not trigger a universal link even in production (the tap must come from another app; Safari often stays in Safari). Smart App Banner is unmentioned.

So: production HTTPS origin as the app’s identity, the update manifest, and the deep-link authority — yes, that dovetails, that is 1023 D1 taken to the end. Development: the phone still types or scans a URL into a dev client / launcher, which is 1023 Stage 3 and 1026 D8. Those are two mechanisms. Pretending the landing affordance makes them one is the lie D10 says not to tell.

*Resolve:* D2 splits. Production: `U` is HTTPS, AASA/assetlinks at well-known, baked origin, optional Smart App Banner. Dev: LAN URL, envelope by `Accept`, no universal-link claim; the launcher/dev-client URL field is the phone’s way in. Associated-domains entitlement is a D1 **binary** row; the AASA file is an origin row. Do not write “instant.”

**B4. 1030.000 D5’s Apple binary pipeline would not work as described.**  
`host/apple/build.mjs` today: development identity (`Apple Development`, `build.mjs:134–146, 308–320`), `get-task-allow` always on (`build.mjs:160`), iOS `Info.plist` written for simulator/device (`build.mjs:164–184, 303`), macOS **not** given an `Info.plist` or entitlements by this file, codesign without `--options runtime` (`build.mjs:285–286, 318–320`), no archive, no notarization, no App Store Connect, no version bump.

D5 sequences “an archive, `xcrun notarytool` for a macOS `.app` or `.dmg`, and the App Store Connect API for an iOS or macOS upload to TestFlight and, on approval, release” as one last mile. Those are two distribution channels:

| Channel | Sign | Notarize | Upload |
|---|---|---|---|
| Mac App Store / iOS App Store | Distribution identity, no `get-task-allow`, App Store profile | No (Apple re-signs) | ASC / Transporter; encryption questions, privacy nutrition, monotonic `CFBundleVersion` |
| Direct macOS | Developer ID, **hardened runtime** (`--options runtime`), entitlements that include whatever wasmtime actually needs | Yes, `notarytool` **and staple** | Not ASC |

A store build cannot reuse the development entitlements function. A Developer ID build is not an ASC upload. Stapling is omitted. The first submit of a version is blocked on console metadata D6 says is “not managed here” (export compliance, privacy answers, agreements). `release: automatic` cannot release what ASC will not accept.

*Resolve:* Two explicit pipelines, both behind `exact deploy`, neither a small patch to `build.mjs`. Do not default either to production auto-release (see positions). Measure the wasmtime entitlement (1028 F3, 1029 §7 stage 0) before generating it; do not guess `allow-jit` vs `allow-unsigned-executable-memory`.

### Material

**M1. D3 claims a classifier `dev.mjs` does not have.**  
1030 D3: “`dev.mjs` already classifies a rebuild by path (1026 D4: a change under `data/` or `gpu/` is a module change; under the kernel, a host, or `modules/` is `{rebuilt}`). The classifier is that code, made exact by digests instead of paths.”

1026 D4 (lines 317–323 of that RFC) **proposed** the split and cited `dev.mjs:113–116` as “knows which files a rebuild was for.” The code still does this: any watched `.rs/.toml/.json/.wgsl/.js/.html` under kernel, plan, motion, runner, `host/web`, `gpu`, Taffy, or the app’s `data`/`web`/`gpu` triggers a full wasm rebuild and `{rebuilt}` (`dev.mjs:105–152`). There is no path classification. A `data/` edit today is a page reload, not a module `{seq}`. The classifier is new work, not a digest-hardening of existing code.

*Resolve:* Say that. Stage 3 implements it. Do not cite 1026 D4 as landed.

**M2. D8 treats shaders as assets; they are not. Stage 1 cannot pass as specified.**  
1009 D5: WGSL is validated at build and compiled at first use **from the crate**. `gpu/reflect` embeds `SOURCE` and `MODULE` in the generated Rust (`lib.rs:195–199`). `dev.mjs:106` watches `.wgsl` as a reason to rebuild the wasm. 1026 D6’s “read from the asset path at `bind` instead of the crate’s source” is unbuilt and rode a D6 that 1029 took off staging.

1030 D8: “in dev the surface recompiles its pipeline on its next use … (the GPU crate’s watch already rebuilds on `.wgsl`, 1007 §6).” The parenthetical is today’s **binary** rebuild, not a pipeline recompile. 1030.000 stage 1 wants `aurora.wgsl` color → one `{seq}`, bindings → “rebuild the native host.” That requires detaching `SOURCE` from the dylib first. Interface digest: hash `emit_entries` + `emit_bindings` + `emit_structs` + `emit_vertex_inputs` (workgroup size is already there, `lib.rs:222–232`), **excluding** `SOURCE`/`MODULE`. Hashing `reflect()`’s full string would change on every color edit and collapse D8.

Also missing from the reflected interface: WGSL `override` constants, and any bind-group layout a surface writes by hand instead of from the generated `GROUP_n`. The GPU crate must keep the generated descriptors as the only layout.

*Resolve:* Stage 1 lists a packaging prerequisite: shaders as digest-addressed assets, `SOURCE` not in the dylib, interface digest defined as the generated interface minus source text. Until then a `.wgsl` edit is a GPU-crate rebuild, i.e. a binary.

**M3. Dual-publish in v1 fights binding rules.**  
1030 D9; `rules/RULES.md` §Scope “Delete; don’t deprecate. No compat shims, no migration paths, no legacy branches before 1.0”; `DEFERRED.md` “No backwards compatibility, at all, before 1.0.”

Dual-publish is a pinned old toolchain plus a source that compiles under both. That is a legacy branch. Pre-1.0, names change; the dual-publish bake will fail constantly, which is the compiler doing its job. Freeze is the only policy that matches the rules. Dual-publish is a 1.0 conversation.

*Resolve:* Freeze is the only v1 policy. Keep dual-publish as a named later door, not a deploy-configuration value that `exact deploy` implements.

**M4. Atomic static publish without cache policy will half-serve.**  
1030.000 D3 step 4.

Content-addressed files first, `exact.json` last, is necessary and not sufficient. If the origin is a CDN (1026 D11), `exact.json` cached for 60s is the Expo footgun: a client that sees the new envelope can 404 a new digest, a client that sees the old envelope is fine, a client that sees a *cached new* envelope against *not-yet-propagated* files is the mid-publish case you claimed to close. Hashed files: `Cache-Control: immutable`. Envelopes and `index.html` and well-known: `no-store`. Bake must emit this (headers on the origin, `_headers` / CDN config / `Cache-Control` on upload). Put a “cache policy” row in D1; it is a layer you will discover the hard way.

**M5. `deploy.lock` committed is the wrong ledger.**  
1030.000 D8.

The last published bundle for `V` is already `V`’s `exact.json` on the origin. A git lockfile every deploy rewrites will merge-conflict, drift from the origin, and lie to the classifier. Commit a pointer to the origin if you must; do not commit the ledger.

**M6. Background refresh is oversold; it is itself a binary.**  
1030 D5, 1030.000 D4 `apply: background`, D6.

`BGAppRefreshTask` requires `UIBackgroundModes` = `fetch` and `BGTaskSchedulerPermittedIdentifiers` in `Info.plist` — host metadata, **binary**. An app that ships without them cannot gain refresh via a bundle. The system runs it opportunistically (often ~daily, not in Low Power Mode, not if the user killed the app, not if Background App Refresh is off). It is a weak accelerator of the after-first-pixel check, not “background push without a push.” A macOS timer while the app runs is the right desktop tool and does not survive quit — which is already “apply at next launch.”

*Resolve:* `apply: background` is a bake-time host-metadata bit, not a deploy knob you can flip later. Describe iOS refresh as best-effort, not a lever that closes the tail.

**M7. iOS alternate icons are a real carrier with a system dialog; D10 understates it.**  
`setAlternateIconName` only selects among icons **already in the bundle and listed in `Info.plist` (`CFBundleAlternateIcons`)**. A new file is a binary — D10 has this. What it misses: iOS presents a system alert you cannot suppress; calling it on every plan boot would nag; the primary icon cannot be replaced, only switched away from (nil restores it); iOS 18 dark/tinted appearances are asset-catalog slots, also binary. The plan may carry the selected name at Level A; the host must call the API only when the name *changes*.

**M8. AASA is not instant; associated domains are a second layer.**  
1030 D1 association row: “the static origin — instant, but the OS caches it.”

Apple’s CDN (`app-site-association.cdn-apple.com`) caches for on the order of a day; devices cache too; developer mode is the bypass. Adding a **domain** is `com.apple.developer.associated-domains` — entitlements, binary, store re-review. Changing **paths** in AASA is a static file, delayed by that cache. Android `assetlinks.json` must name the **Play App Signing** cert fingerprint, not the upload key — a brownfield footgun 1030.000 D7 does not mention. Android stays on `DEFERRED.md`; the row is fine as a door, not as a v1 claim.

**M9. Downloaded native `cwasm` on Apple is not “honored in substance.”**  
1030 D1 wasm row; 1029 D3; 1023 §8; 1028 F5.

iOS: Pulley (or wasmi) only. Native AOT pages are downloaded machine code; AMFI will kill them; 3.3.2 will not save you. macOS App Store + hardened runtime: wasmtime deserialize needs executable memory; which entitlement is **unverified** (1028 F3). Developer ID + notarization will pass with a justification; MAS review may not treat “it’s bytecode we AOT’d in the bake” as interpreted code. Linux is fine.

*Resolve:* 1030 D1, “the client must have”: iOS = Pulley/wasm + hbc; macOS native cwasm only if the measured entitlement is in the **binary** and the channel is not MAS, or MAS is an explicit accepted risk (1029 §8 Q6). Bake refuses a native cwasm card on an iOS target.

**M10. Sunset cannot reach Level 0, or any binary that predates the reader.**  
1030 D5, D7, D9 item 2.

A Level 0 binary has no check, so it never fetches the envelope that carries `"sunset"`. A binary shipped before D5 cannot show a card its code does not read. D9’s “installed binary keeps working forever” is true; “the one thing an old binary can be told” is true only of binaries that already contain the reader. Forced update for Level 0 is the store, or nothing.

*Resolve:* Sunset is v1 for `L = A` binaries that include the reader. Level 0: say so.

**M11. Channel “promotion is copying static files … with a new `seq`.”**  
1030.000 D4.

`seq` is signed (1026 D11). Copying `exact.json` between `beta/` and `prod/` either reuses a signature over the wrong origin/seq or breaks anti-rollback. Promotion is a bake or a re-sign that writes a new `seq` for the destination origin. Directories-as-channels is still the right refusal of a service.

**M12. Brownfield is a different host than the one that exists.**  
1030.000 D7: “a Swift package over the Rust staticlib on Apple (the shared Swift `host/apple` already has).”

`host/apple` is an application (`ExactIOS` / `ExactMac`), not an embeddable view that takes a `UIView`, yields `UIApplicationDelegate`, and does not own first pixel. First pixel of the host vs. exact2’s 100 ms budget is unstated. Entitlements, associated domains, ATS, background modes are the **host app’s** — bake must not generate their `Info.plist`. Stage 8 is a new shape, not a wrapping of today’s presenter. Fine as a later stage; not “already has.”

**M13. `dev.mjs --deploy` as the binary path is the crazy part.**  
1030.000 D5, D3; Charlie’s “is that crazy?”

One **verb** that bake + classify + publish bundles + (policy) submit binaries, callable from a terminal, CI, or a schedule — not crazy; 1026 D13 already said it. The watch loop (`dev.mjs:113–152`) is a 200 ms debounce around a wasm rebuild and SSE. Notarization and ASC uploads take minutes, need a Mac, and must survive the process dying. `--deploy` turning that loop into a store submit, with defaults `binaries: continuous` and `release: automatic`, is how a save ships to production.

The classify-and-print-on-every-edit behavior (D5 guardrail 2 without `--yes`) is the actual gift. Keep it. Run publishes from `exact deploy`, on CI, with `--only bundle` as what a laptop may do unattended.

**M14. Missing or doubled D1 rows.**

Doubled: “Icons, display name, launch screen…” and “Host metadata” both own icons, launch screen, entitlements. Fold; D10 is the per-platform carrier column of one host-metadata row.

Missing (each with the carrier I would write):

| Layer | Why | Carrier |
|---|---|---|
| Associated-domains **entitlement** (vs AASA file) | See M8 | binary |
| Privacy manifest of **exact2 itself** merged with the app’s | MAS rejects an incomplete `PrivacyInfo.xcprivacy`; the updater is a network reason | binary |
| Cache policy for origin files | M4 | origin (bake-emitted headers) |
| `dSYM` / symbols | You cannot symbolicate a binary you submitted without them | beside the binary, not the bundle |
| macOS App Sandbox + hardened-runtime entitlements | MAS requires sandbox; wasmtime needs a JIT-class exception | binary |
| iOS 18 icon appearances (dark/tinted) | asset catalog | binary |
| InfoPlist.strings (localized display name) | not `CFBundleDisplayName` alone | binary |
| App groups / keychain access groups | 1018 D7 keys on code signature; groups are entitlements | binary |
| WGSL as **runtime asset** vs embedded `SOURCE` | M2 | until packaging changes: binary |
| Store-required monotonic version | D6 says version numbers “decide nothing”; ASC refuses non-increasing `CFBundleVersion` | generated, but they decide upload |

Widgets/extensions/watch: D6’s “out of scope until an app has one, then a D1 row with binary in every column” is the right deferral.

**M15. 1030.000 is several LLPs.**  
Eight stages, no implementer, no date. `RULES.md` §Scope: a spec without those is not written; an RFC you are about to build is. Stages 5–8 (store pipeline, reach, landing, brownfield) are not “about to build” on the same day as the theory. Working set is 15/15 (`QUEUE.md`). Landing both numbers means archiving two links.

*Resolve:* 1030 = theory. 1030.000 = stages 1–4 (asset row **with** shader packaging, manifest, classifier dry-run, bundle publish). Binary pipeline / landing / brownfield / `exact reach` = 1030.001 when someone is named.

### Minor

**m1.** 1030 Summary promises “what this refuses (§7)”; §7 is D7 (`state.delivery`). There is no refusals section in 1030. 1030.000 §3 has one. Add 1030 §7: no service, no native machine code to iOS, no pretending live, no dual-publish matrix before 1.0, no ninth agent op.

**m2.** Native Rust identity in D1 cites 1026 D10 (whole module). 1029 D4 keyed it per source. Say 1029 D4.

**m3.** “Automatic updates are the default on iOS and most users have them on; the tail is weeks.” The tail is unbounded (MDM freeze, automatic updates off, never launches). D5’s sunset exists because of that; the weeks sentence undersells it.

**m4.** TestFlight “an hour” is internal testers. External is Beta App Review, often a day. `release: internal` should mean internal TestFlight, not external.

**m5.** 1030.000 D1 “no browser JavaScript app” is stale after 1027 D6 (`app.js` on the web). The *HMR* rejection still holds; the sentence should say one script, restart with carry.

**m6.** `exact reach` over CDN logs: many CDNs do not log custom headers by default; IPs are in those logs. Histogram of `(runtimeVersion, seq)` is fine; retaining IPs is not “no identity.” Print `unknown` when headers are not logged — already said — and add “do not keep IPs.”

**m7.** D9 item 3 (durable state): pre-1.0, a kernel/shape change wipes what no longer fits. Say that loudly. “No migration scripts” is the rule; users can lose the last station.

**m8.** D3’s override “spelled out, never implied” is not spelled out.

**m9.** 1028/1029 bearing: Pulley-vs-wasmi does not change 1030. Per-target cards do; 1030 D1 has them. Phone numbers still owed (1029 §7 stage 0) before any Level-A-plus-wasmtime size claim is a budget.

## 4. Suggestions

1. **Collapse B into `(L, E)`** — cost: a D4 rewrite and an amendment note on 1026 D12 / 1027 D7; saves a year of “is TS Level A or B?”
2. **Classifier output is a table keyed by `(platform, V)` plus an origin row** — cost: the D3 five-rule list becomes a function, which 1030.000 D3 already prints; the five-rule prose is what dies.
3. **Shader packaging before “hot WGSL”** — cost: one GPU-crate change (load WGSL by digest at `bind`, keep reflect for the interface). Without it, delete the D8 “already” parenthetical.
4. **`exact deploy` is a library; `dev.mjs` classifies; CI publishes** — cost: not folding EAS Build into the SSE process; one extra command in the README. `--deploy` if it exists is `--only bundle` or dry-run.
5. **Default policy: `bundles: continuous`, `binaries: internal`, `release: internal`** — cost: one word in D4 against Charlie’s “continuous everywhere”; still continuous for the thing that is actually instant (web + bundle). Production store submit is a human `--yes` on a binary row.
6. **Cache headers as a bake output** — cost: small; prevents the first production incident.
7. **Ledger on the origin, not `deploy.lock` in git** — cost: `exact reach` and the classifier read `exact.json` over HTTPS; no merge conflicts.
8. **Sunset copy names Level 0 as unreachable** — cost: a sentence.
9. **Measure the macOS wasmtime entitlement before D2 generates it** — cost: 1029’s already-planned afternoon; do not pick a name in 1030.
10. **Split 1030.000 after stage 4** — cost: another number later; keeps this RFC buildable.

## 5. Positions on the open questions

### 1030 §5

1. **Level 0 is the updater crate not linked** (absence, not a disabled check). Reason: “stripped” is a promise about the binary; a no-op check is still a network path and still code. Pair with deleting Level B (B1).
2. **Atomic-per-(platform, V) is the default; the override cannot override bake’s source-roster check.** Reason: a half-published bundle is the failure mode; an override that ships a plan naming missing Rust is how you get it anyway.
3. **Sunset in v1 for `L = A` binaries that contain the reader; not for Level 0.** Reason: ten lines in a binary that already fetches; zero lines that can be retrofitted into Level 0.
4. **`state.delivery` on `state`, not a ninth operation.** Reason: LLP 1024’s count rule; it is a question `state` already exists to answer.
5. **Freeze is the only v1 upgrade policy; dual-publish is not a v1 value.** Reason: `DEFERRED.md` before 1.0; freeze matches the store’s drain.
6. **The reflected-interface digest is enough once it hashes the generated interface excluding `SOURCE`, including workgroup size — and once shaders are actually assets.** Reason: the binary binds the interface; color-only edits must not move that digest; today’s `reflect()` return value would.

### 1030.000 §5

1. **`release: internal` as the default, not `automatic`.** Reason: Charlie asked for continuous; that is bundles (and web) on every platform at once. Store auto-promote is how a bad binary reaches everyone during review’s tail. One word to change, in the safer direction.
2. **`dev.mjs` prints the classify table on each edit; `exact deploy --watch` (or CI) is the publisher; do not put `--deploy` on the SSE process.** Reason: one server for the loop, one verb for publish, two processes so a notarization cannot take down `{seq}`.
3. **`app.toml` beside `app.contract`, TOML.** Reason: host metadata is not UI; the Contract compiler must not learn entitlements; same standing as `Cargo.toml`.
4. **One secret store, whatever 1026 D13 already holds for the bundle key.** Reason: two custodies is how keys rot in the one you forgot.
5. **Launcher is TestFlight-only until it is a product.** Reason: the store listing is a product decision; the typed URL works without it.
6. **Android stays on `DEFERRED.md`; keep `assetlinks.json` as a generated file so the door is a file not a redesign.** Reason: a row is cheap; a Play pipeline is a surface.
7. **Pin a commit, not a tag — and do not implement dual-publish yet.** Reason: there are no releases; weird-castle already consumes by path; a pin without a matrix is just “this exact2.”

## 6. Named disagreements to argue in round 2

1. **Keep Level B as a word vs. collapse to `(L ∈ {0,A}, E)`.** I expect the author to keep B for continuity with 1026 D12. The mixed default is the test: if a TypeScript-only change on the paved-path app is not a bundle, B is wrong.
2. **`release: automatic` because Charlie said “continuous everywhere” vs. `internal` because store promote is not the same act as bundle publish.** This is the one the human has to settle; both panelists should answer it in one sentence without restating the ask.
3. **Whether `exact deploy` from the dev-server process is in v1.** I say the verb is in, the watch-loop submit is out, and stages 5–8 are another LLP. If the other family calls the whole of 1030.000 buildable as staged, that is the fork: theory-complete vs. EAS-complete.

(Secondary, if there is room: freeze-only vs. dual-publish as a v1 knob; I will not move unless the other family has a pre-1.0 compatibility story that `DEFERRED.md` allows.)

## 7. Recommended next step

**Revise and stay Draft.** Do not gather another family until D3/D4/D1 in 1030 are internally consistent — a second family on the current text will re-find B1/B2, not settle them. After that revision, this panel’s round 2 can force the three named disagreements; Charlie answers those; then 1030 can be accepted as theory.

1030.000 stays Draft and should be cut to stages 1–4 plus the classify-on-edit printer. Nothing in the repo changes until an implementer and a date are named against those stages (`RULES.md` §Scope). The working set is full; accepting either number archives a link.

What I would not wait on: D2 (the manifest), D10’s honesty rule, D5’s no-service propagation, D9’s freeze constants, 1030.000 D1 (not Vite), dry-run-by-default. Those held.

---

## Round 2 — deltas after reading the sol round-1 answer (verbatim)

I'll start from the round-1 answers and the same review documents, then write only the deltas.I have both round-1 reviews. Next I’ll check the RFCs and the cited code against Sol’s claims, then write only the deltas.I’ll verify the disputed facts in the cited code and LLPs before locking positions.# Panel review — LLP 1030 / 1030.000 (round 2, Grok 4.6)

Provenance: Grok 4.6 · xAI · 2026-09-03 · not redacted · mutually visible with Sol’s round 1. Same documents, same gates. Only deltas.

---

## 1. Corrections

### Where Sol is wrong about the code or the cited LLPs

**`deploy.lock` is not forbidden by the generated-file rule.** Sol cites `rules/RULES.md:16` and `rules/DEFERRED.md:150` as if a committed ledger were the same class as `build.rs` output. Those sentences are “generated files are built, never committed” — `Info.plist`, reflected Rust, wasm. A lockfile is a different category (`Cargo.lock` is committed). The *operational* case against `deploy.lock` still holds (merge conflicts, worktree/CI seq allocation, origin already is the ledger). Cite that, not the generated-file rule; the wrong citation makes a good point look like a process nit.

**Canonical signed bytes already exist next door.** Sol writes that the documents “do not specify canonical signed bytes.” LLP 1026 D11 does: public key baked into the binary; Ed25519 over the envelope’s canonical bytes, which name every other file’s digest; rotation is a new binary (`llp/1026-dynamic-delivery.rfc.md` around the signing bullets under D11). 1030’s actual error is D1’s row “**The signing key** for bundles … in the binary” (`llp/1030-delivery-unified.rfc.md` D1 table), which *regresses* 1026’s correct public-key wording. Missing key IDs, overlap, and compromise behavior are a real 1030 gap; “no canonical signed bytes” is not.

**A static selection path already exists; 1030.000 then garbles it.** 1026 D11 already says “a directory per runtime version, the same `exact.json` bake wrote.” 1030.000 D2 then says both “the update manifest at `U/exact.json`” *and* “per runtime version directory” in one sentence (`llp/1030.000-dev-server-as-deployer.rfc.md` D2 production paragraph). The bug is the contradiction, not the absence of a path. Sol’s `U/.exact/updates/<compatibilityId>/exact.json` is a fine spelling; it is not a greenfield protocol.

**“A mixed app carrying Hermes is B by definition” reimports the hole.** That sentence is 1027 D7 (`llp/1027-typescript-data-sources.rfc.md`, the “Level A and Level B restated” paragraph: a TypeScript app always links `exact-js`, so “Level B is free where Level A was”). 1029 D2 then keeps the default mixed composition as Level A for the Rust half while the binary still carries Hermes (`llp/1029-mixed-logic-and-engine-choice.rfc.md` D2 table, default row). Picking 1027’s B as the resolution is how 1030 D4 fell through. The mixed default is `L = A, E = {hermes}`, not B.

**1030 §1’s “None of that changes” is not an implementation claim.** The heading “What already exists, and what it already says” is sloppy, and Sol is right that 1023/1024/1026/1028/1029 are Draft and that 1027 is Accepted with stages 2/4–7 unbuilt (`QUEUE.md`). The sentence “None of that changes” means 1030 does not reverse those designs. Do not grade 1030 as if it asserted production OTA is in the tree. Grade the heading as a status lie; grade 1030.000 stage 4+ as blocked on 1026 still being Draft *and* on `rules/DEFERRED.md:52` still listing “Snapback / update economy” (1026 D11 claims Charlie opened that door; `rules/RULES.md:47-50` still requires a written trade).

**1029 D4 is more than Sol credits, and it is not a default-path blocker.** 1029 D4 already takes the whole data section and “every wasm function reachable from that source’s dispatch arm.” Sol is right that table/indirect-call closure is not specified, and that a wasm32 digest is not native identity under `#[cfg(target_os = "ios")]`. The default 1029 D1 app does not opt into `Swappable`. That identity hole is Material on D1’s native-Rust row and Blocking only for opted-in cwasm OTA, not for 1030’s paved path.

**D6’s widget deferral is the right shape.** Sol lists widgets/extensions/watch as missing completeness. 1030 D6 already says they are binary rows when an app has one. Retract the ratification note’s “inventory is complete.” Do not add empty product surfaces to D1.

### Where Sol caught what I missed (one line each)

- D1 says the **signing key** lives in the binary; 1026 D11 bakes the **public** key. I treated the row as a binary layer and missed the key-material error.
- `[deploy]` in the same file whose digest is binary identity makes `automatic` → `manual` a binary change (`1030 D2` + `1030.000 D8`).
- D9 item 4 cites `plan/build.rs` domain separation for payload integrity; that digest is format identity (`llp/1023-one-url-serving.rfc.md` Related, and D2: `plan.sha256` is raw SHA-256 of plan bytes). `host/web/dev.mjs:84` and `:204` match 1023, not 1030 D9.
- One `a1b2` covering iOS B / macOS A / Linux 0 (`1030.000 D3` example) contradicts “the runtime version records the level” (`1030 D4`).
- Level 0 still publishes AASA/`assetlinks.json` on the origin; D3 rule 2 turning that into a binary is a category error I stated for bundles, not for L=0.
- `host/web/dev.mjs:104-107` does not watch app-root `app.ts`, icons, or ordinary assets; `.ts`/images/fonts are outside `wanted`.
- `apply`’s default is two values on a one-value row (`1030.000 D4`).
- Agent `state` (LLP 1012: the eight operations) cannot be what Contract reads for an in-app update row (`1030 D5` / `1030.000 D6`).
- 1026 D11 already has “grants cannot rise”; 1030 D1 dropped that row.
- A wasm32 digest is not native identity (`#[cfg]`, build scripts, flags).
- Four Apple distribution lanes, not two: local development, iOS distribution, Developer ID + notarize, Mac App Store.
- Watch-loop publish of `host/web/dist` (`dev.mjs:39-45, 123-149`) plus a 200 ms debounce is a mixed-generation / half-edit snapshot; dry-run then `--yes` is TOCTOU.
- `setAlternateIconName` is async, foreground, system-alerted — I had the shipped-set limit, not the UX.
- Reach over CDN logs counts requests, not clients.
- A generic launcher cannot claim arbitrary customer domains; its associated-domains entitlement is signed into *its* binary.
- PWA name/icon/splash and Linux `.desktop`/theme icons are installed-shell metadata, not “replace the file.”
- Brownfield needs a host descriptor; `host/apple` is `ExactIOS`/`ExactMac`, not an embeddable view (`1030.000 D7`).
- Snapback is still on `DEFERRED.md:52`.

---

## 2. The named disagreements — final positions

### A. Level B as a word vs `(L ∈ {0,A}, E)` *(my r1 #1; Sol’s orthogonal-capabilities resolve)*

**Final: delete B as an authority.** `L` is “is there an updater?” (`0` or `A`). `E` is the `host!` composition. A TypeScript-only Δ on the default mixed app is a bundle because `E` contains Hermes, not because a `level: B` knob said so.

Sol’s extra axes (trust epoch, activation mode, accepted artifact kinds) belong in the **compatibility descriptor**, not in `L`. Keeping B as a “user-facing preset” is how D4’s last paragraph and D3 rule 4 will disagree again.

**What would settle it:** feed a TypeScript-only edit through the D3 table for 1029 D1’s default app. If the answer is not “bundle,” the theory has failed Charlie’s paved path. 1027 D7 is restated in `(L, E)` words, not by reviving B.

### B. Whole-Δ heaviest-carrier vs a dependency-closed publish unit *(Sol r1 #1; my r1 Q2, which I now move)*

**Final: Sol is right that co-occurrence is not dependency. I retract “atomic-per-`(platform, V)` over the whole source Δ, with a spelled-out override.”**

The publish unit is a **dependency-closed snapshot per `(platform, compatibilityId)`**, plus a separate **origin** action (web app, AASA, `assetlinks.json`, hashed static files, cache headers). A commit that contains an independent `app.ts` fix and an entitlement edit publishes the bytecode bundle to old binaries and queues a binary for the entitlement; it does not freeze the web origin.

The developer override goes. Independence is a graph fact. `--only bundle` refuses unless that closure is bundle-safe for the named cohort (1026 D12’s “plan names a source the embedded crate lacks” stays, as bake’s refusal, not a human assertion).

A general multi-carrier saga is more than v1. Ordered **named migrations** (add AASA path → ship binary with associated-domains → wait out Apple’s CDN → remove old path) are procedures, not an orchestrator. Sol’s universal-link expansion/contraction is the example that forces precedes-edges; the common case is independent rows.

**What would settle it:** one commit with an independent `app.ts` fix and an entitlement edit. The dry-run table must show `bundle` for the TS cohort and `binary` for the entitlement cohort, and must publish the web origin — not “nothing published to a1b2 (atomic)” as `1030.000 D3` prints today.

### C. `runtimeVersion` vs `compatibilityId` *(Sol r1 concern 2; my r1 B2, which named per-platform carriers but left V as 1026 defined it)*

**Final: the directory key is a `compatibilityId`, not 1026’s `V` as currently defined.** 1026 D9’s tuple (kernel schema, `formatVersion`, module ABI, roster digest, plus 1027 bytecode version, plus 1029 wasmtime version) is “what code this binary can run.” It omits platform, arch, distribution profile, verification-key set / trust epoch, native-code identity, shipped icon roster, grant ceiling, associated-domains entitlement, min OS. The holiday-icon case and a native `#[cfg]` change that does not move the wasm32 digest are the proofs.

**Constraint Sol’s recipe must pass, or it will explode the live list:** 1030 D9’s host-bug-fix case — “a faster painter; the new binary keeps its runtime version” — must **not** fork `compatibilityId`. Hash what makes a bundle unsafe on that binary; exclude `[deploy]`, schedules, live lists, and painter-only host changes. `releaseId` correlates platforms; `seq` is monotonic per `(app, channel, compatibilityId, trustEpoch)` stream, allocated at the origin, not in git.

**What would settle it:** two binaries that differ only by a host painter speedup share `compatibilityId`; adding alternate icon `holiday` does not; rotating the verification-key set does not (new trust epoch / new stream).

### D. One dev-server process vs one library and two trust boundaries *(my r1 #3; Sol r1 #2 — we already agreed on the split; this is the remaining “betrays the one loop” fork with the author)*

**Final: one library, two processes.** `dev.mjs` classifies and prints. `exact deploy` (CI, a schedule, or `exact deploy --watch`) publishes a **commit/tree snapshot** into a run-specific staging directory. The LAN process bound to `0.0.0.0` (`dev.mjs:38, 223-234`) does not hold production keys and does not upload from mutable `host/web/dist`.

Charlie’s “maybe the dev server should trigger a build” is answered: the table is in the loop; notarization and ASC are not. One verb, not one privilege boundary.

Sol’s extras I adopt: 200 ms debounce can publish the first half of a multi-file edit; dry-run/`--yes` is TOCTOU unless `--yes` re-bakes; Apple outputs are replaced in place (`host/apple/build.mjs:51-52, 269-300`).

No remaining panel disagreement. Charlie can still pick the flag; we would both call that the wrong pick.

### E. `release: automatic` vs `internal` *(my r1 #2; Sol Q1 already landed on internal)*

**Final: `bundles: continuous`, `binaries: internal`, `release: internal`.** Charlie asked for continuous *everywhere*. That is web + bundles on every platform at once. Store auto-promote is a different act, partially irreversible, and blocked on console metadata 1030 D6 refuses to manage.

No remaining panel disagreement. Charlie can still pick `automatic`; we would both call that the wrong default.

### F. “Conform or restart” as the durable-state upgrade policy *(Sol r1 #3; my r1 m7, which I now upgrade)*

**Final: retract D9 item 3 as written.** 1007 §6 (`llp/1007-web-host-v1.spec.md` §6) is reload carry of slots and matching resources — “what no longer fits starts fresh.” That is not a Store-across-runtime-upgrade guarantee. 1018’s implemented `secret` tier is opaque strings in Keychain/`localStorage`; `plain` is unbuilt; Linux is memory-only (`llp/1018-durable-client-state.rfc.md` D2 and the Linux bullet). Equal shapes are not equal semantics. Keychain continuity is team / code signature / access group, not `app_id` alone (`host/apple/build.mjs:280-283` already says ad-hoc signing loses secrets). The update-store codec cannot “change” and also “survive without migrations.” “Works forever” is false once OS support, certificates, or store policy move.

Replacement, still with no migration scripts (`rules/RULES.md` “Delete; don’t deprecate”; `DEFERRED.md` “No backwards compatibility, at all, before 1.0”):

- Secrets survive a runtime upgrade **if** the platform keychain continuity conditions hold.
- Plain-tier values, once that tier exists, conform-or-restart at the declared shape.
- Update-store codec is versioned independently; unknown major → entry zero.
- Narrow D9 item 2 to: a valid update cannot destroy the embedded fallback **on a supported OS**.
- The store-carry fixture stays, and is the check, not the prose guarantee.

**What would settle remaining wording:** boot `V'` over a store written by `V` with a shape change (start-fresh, no refusal) and with an unchanged secret (Keychain hit only if team/signature match; miss if ad-hoc → Development).

### G. Shader interface digest: enough vs not *(my r1 Q6 vs Sol r1 concern 13)*

**Final: Sol is right; I overclaimed Q6.** `gpu/reflect` returns generated Rust plus source paths (`gpu/reflect/src/lib.rs:40-47, 49-82`). `SOURCE`/`MODULE` embed the WGSL (`lib.rs:194-199`). Today a color edit is a GPU-crate rebuild, i.e. a binary. Stage 1’s “color → `{seq}`, bindings → rebuild” cannot pass until shaders are digest-addressed assets and `SOURCE` is not in the dylib.

After that packaging, hashing `emit_entries` + `emit_bindings` + `emit_structs` + `emit_vertex_inputs` (workgroup size is already there, `lib.rs:222-232`) **excluding** `SOURCE`/`MODULE` is necessary and **not sufficient**. Still missing: WGSL `override` constants, fragment-output formats, required device features/limits, and any bind-group layout a surface writes by hand. Until the GPU crate keeps generated descriptors as the only layout, classify WGSL with its surface as binary-coupled.

Pipeline construct-and-validate-before-replace is the **apply** rule for a shader asset, not a new D1 layer. Do not block 1030 on a pipeline-cache protocol.

### H. Two Apple pipelines vs four *(my r1 B4 vs Sol r1 concern 10)*

**Final: four lanes, named in 1030, not built in 1030.000.** Local Apple development (what `host/apple/build.mjs` actually is: Development identity, `get-task-allow`, no archive, macOS unsigned-or-ad-hoc with `--timestamp=none` and no hardened runtime — `build.mjs:133-145, 155-161, 269-292, 295-336`). iOS distribution (ASC / TestFlight / App Store). Direct Developer ID + hardened runtime + notarize **and staple**. Mac App Store (sandbox, different profile, Apple re-signs; wasmtime entitlement is a review question, not a fact — 1028 F3’s “the App Store accepts it with a reason” is unverified).

I collapsed 2 and 4 into “store.” That is wrong: MAS sandbox plus the cwasm entitlement is a different pipeline from iOS Pulley.

1030.000 stage 5 does not implement these. A later LLP does, after the entitlement spike 1029 §7 stage 0 already owes.

### I. Dual-publish in v1 *(secondary in my r1; Sol concern 18)*

**Final: freeze is the only v1 policy.** Dual-publish is a pinned old toolchain plus a source that compiles under both — a legacy branch, banned by `rules/RULES.md` §Scope and `DEFERRED.md` §Deliberately worse before 1.0. Sol is right that an Exact commit is not a reproducible toolchain (`scripts/app.mjs:17-27` binds to the current root) and that running old bake code with current production keys is a supply-chain boundary. If dual-publish ever exists, it needs a content-addressed toolchain capsule; a commit pin is not that. It is not a `1030.000 D4` value.

### J. Secrets: one store vs KMS + isolated signer *(my r1 1030.000 Q4 vs Sol Q4)*

**Final: one custody system, many named credentials, isolated signer.** I said “one secret store” to stop a second vault rotting. That is not “one key.” OTA Ed25519, App Store Connect API, Developer ID, and notary are different credentials by construction. The LAN watcher is not the signer.

### K. How much 1030.000 to keep *(my r1 M15 vs Sol “revise the protocol, then a narrow bundle slice”)*

**Final: both, sequenced.** 1030 is rewritten until D3/D4/cohort/origin are implementable. 1030.000 is cut to stages 1–4 (asset row **with** shader packaging, manifest with `[deploy]` out of artifact identity, classify-and-print, bundle publish to one origin / one channel / one writer). Stages 5–8 (store pipeline, reach, landing, brownfield) are another number when someone is named. Working set is 15/15 (`QUEUE.md`, `RULES.md` §Scope).

---

## 3. Severity reconciliation

Only where we graded differently. Shared Blocking (A/B hole, classifier/atomicity, one-URL/AASA, Apple D5 as written) stays Blocking.

| Issue | Me r1 | Sol r1 | Final | Why |
|---|---|---|---|---|
| “Already exists” / Draft prerequisites / snapback still on DEFERRED | missed as its own item | Blocking | **Material on 1030; Blocking on 1030.000 stage 4+** | Theory may rest on same-day Drafts; it may not claim they are code. Bundle publish cannot start while `DEFERRED.md:52` still lists the update economy and 1026 is Draft. |
| `runtimeVersion` underspecified | folded into B2 | Blocking | **Blocking** | Cohort key is the classifier’s input. Holiday-icon and native-`cfg` are unsafe in one `V`. |
| Signing-key wording + trust epochs | missed | Blocking | **Blocking on D1’s “signing key”; Material on rotation-overlap** | Public vs private is a factual error against 1026 D11. Key IDs/compromise can land as a short D1/D9 addendum; 1026 already has Ed25519-over-envelope and rotation-is-binary. |
| `[deploy]` inside binary identity | missed | Blocking | **Blocking** | A policy-only edit must not move binary identity or `compatibilityId`. |
| LAN process as production deployer | Material (M13) | Blocking | **Blocking if `--deploy` submits stores; disappears if 1030.000 is cut and the process is split** | Same fact, severity follows whether stage 5 stays. |
| Durable-state “survives” / “forever” | Minor (m7) | Blocking | **Material** | False guarantee; retract/narrow D9 items 2–3. Not a wire protocol hole. |
| Wasm/native identity, per-source closure, cwasm trust | Minor cite (m2) | Blocking | **Material on D1’s native-Rust identity column; Blocking only for opted-in native cwasm OTA** | Default app is not `Swappable`. Do not hold 1030’s paved path on 1029 D4’s reachability algorithm. |
| D1 completeness (grants, update-store, SW/cache, distribution artifacts, external console state) | Material (M14, shorter list) | Blocking | **Material** | Add the rows that are true of Caltrain/Castle today (grant ceiling, verification keys, associated-domains *entitlement*, privacy manifest, cache headers, update-store, monotonic `CFBundleVersion`). Defer widgets via D6. Retract “complete.” |
| Shader classification false of the code | Material | Material | **Material** | Stage 1’s shader acceptance case is false until packaging. Not a reason to reject the three-sub-layer split. |
| `deploy.lock` / seq allocation / cache headers | Material | Blocking (inside concern 6) | **Material** | Wrong ledger and missing `Cache-Control` will half-serve; they do not make the classifier’s rule unsound. Origin is the ledger; hashed files `immutable`, envelopes/HTML/well-known `no-store`. |
| Brownfield / dual-publish cost | Material | Material | **Material; dual-publish out of v1** | Host descriptor before any `brownfield: true` publish. |
| 1030.000 is several LLPs / stage order | Material (M15) + Minor (m1–m9) | Minor on staging | **Material** | Stages 5–8 are not “about to build” (`RULES.md` §Scope). Shader case in stage 1 is a dependency inversion. |

I adopt Sol’s Apple-lane split, grant-ceiling row, Contract-vs-agent `state`, `apply` schema, and request-weighted reach without upgrading those last three above Material.

---

## 4. The one change

**1030:** Rewrite D3 and D4 as one rule: `(L ∈ {0,A}, E)`, classifier output is a table keyed by `(platform, compatibilityId)` plus an origin row, association is origin not bundle, whole-Δ atomicity and the semantic override are deleted. That is the theory’s main job. Without it the default mixed app takes a store release to change `app.ts`, and the D3 example’s “nothing published to a1b2” freezes the web. Cohort identity, per-platform honesty, and the publish unit are the same rewrite, not three.

**1030.000:** Cut to stages 1–4; `exact deploy` is not `dev.mjs --deploy`; defaults `binaries: internal` / `release: internal`. That is Charlie’s “is that crazy?” answered in the safer direction, and it is what makes the RFC a thing someone can be named to build (`RULES.md` §Scope; working set 15/15).

---

## 5. Open questions (only where r1 was wrong or Sol moved me)

### 1030 §5

2. **Dependency-closed snapshot per `(platform, compatibilityId)` plus origin; no semantic override.** Whole-Δ atomicity withholds unrelated fixes and still misses unchanged native dependencies.
4. **Agent `state.delivery` stays on `state` (no ninth op); Contract gets a normal `delivery.update` resource the agent record merely mirrors.** 1012’s `state` is an agent diagnostic (`llp/1012-agent-api-v1.spec.md` §1); D5’s in-app row cannot read it.
6. **Not enough.** Packaging first; then a compatibility record that is not `reflect()`’s full string; then pipeline validate-before-replace. Today’s generated interface including `SOURCE` classifies every WGSL edit as binary.

(1, 3, 5 unchanged: Level 0 is absence of the crate; sunset in v1 for `L = A` binaries that contain the reader; freeze-only.)

### 1030.000 §5

4. **One custody system, separate named credentials, isolated signer — not “whatever 1026 D13 holds” as the whole architecture, and not one key.** Local Keychain remains the dev identity story (`build.mjs:133-146, 280-286`).

(1, 2, 3, 5, 6, 7 unchanged in direction: `release: internal`; separate deploy process; `app.toml`; TestFlight-only launcher; Android stays on DEFERRED without confirming “two rows”; pin a commit and do not implement dual-publish. Sol is right that a commit is not a toolchain capsule *if* dual-publish ever returns.)

---

## 6. Recommended next step

**Revise and stay Draft. Do not gather another family. Do not accept.**

The remaining forks are not ones another model needs to discover. Charlie answers three words — `release: internal`, freeze-only, deploy-not-in-the-LAN-process — and the author rewrites 1030 D1/D3/D4/D8/D9 and cuts 1030.000 to stages 1–4. Snapback comes off `DEFERRED.md` with a written trade before any bundle-publish stage is a spec.

The wasmtime-entitlement afternoon (1028 F3 / 1029 §7 stage 0), a real-device AASA/associated-domains run, and a concurrent-publish/cache spike can run in parallel. They gate a later binary-pipeline LLP, not this revision.

What I would still not wait on, and Sol did not move: D2’s one manifest, D10’s honesty rule, D5’s no-service propagation, D9’s freeze-by-default *constants* (once items 2–3 are narrowed), 1030.000 D1 (not Vite), dry-run-by-default. Those held.
