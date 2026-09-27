# Panel: LLP 1030 Delivery, unified + LLP 1030.000 The dev server as the deployer (sol)

- **Family:** OpenAI: `codex exec -m gpt-5.6-sol -c model_reasoning_effort=ultra -s read-only --skip-git-repo-check -C <capsule> -o <out> -` (OpenAI Codex CLI 0.153.0), prompt on stdin, cwd = the read-only capsule; no repository access. Requested `gpt-5.6-sol` at `ultra` (Charlie named "sol ultra").
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
- **Peer:** grok `grok-4.6` at `xhigh`, same capsule, same rounds.
- **Disposition:** folded into LLP 1030 r2 and 1030.000 r2 the same day — 1030 §8 records which catches held and where each landed; the author re-verified every code claim before folding (`dev.mjs:104–149` rebuilds whole on any watched change with no path classification; `gpu/reflect/src/lib.rs:195–199` embeds `SOURCE`; `build.mjs:155–184, 285–286, 303–320` signs with a development identity, `get-task-allow`, no archive, no notarization, no macOS `Info.plist`). Round-2 dispositions are in 1030 §8 ("Round 2"); both families converged and neither recommends another family.

---

## Round 1 — position (verbatim)

## 1. Overall assessment

The core theory is directionally right: delivery should be derived from artifact identity, client capability, and the carriers available to that client. The layer inventory, stripped profile, and explicit acknowledgment that some changes require a binary are substantial improvements over a generic “OTA” story. The central abstraction is not yet sound, however: one change can require several ordered carriers, and `runtimeVersion` does not identify the binary cohort against which a bundle is safe. `1030.000` then treats static publication, native builds, store submission, and release as one transaction even though they have different trust boundaries and failure semantics. I would not build `1030.000` as written; I would first revise the protocol and then implement a narrow bundle-update slice.

## 2. Strengths

- `1030 D1` is unusually honest about the breadth of the problem: Contract, native and interpreted logic, assets, GPU pieces, host metadata, native modules, the runtime, and stripped clients are considered together.

- `1030 D4` correctly refuses to pretend every app is OTA-capable. Level 0 and the no-TypeScript/no-Wasm cases answer Charlie’s “heavier mechanisms” question plainly.

- `1030 D2` and `D10` correctly make host metadata generated from one declaration and expose per-platform differences instead of forcing a false lowest common denominator.

- `1030 D5` accurately recognizes asynchronous propagation and the long tail. The embedded fallback, anti-rollback intent, crash fallback, and visibility in `D7` are the right operational instincts.

- The GPU decomposition in `1030 D8`—surface code, shader, and GPU runtime—is the right conceptual decomposition even though the proposed digest does not yet exist.

- `1030 D9` identifies the two defensible runtime-upgrade policies: freeze an old cohort or pay to dual-publish. That is much better than promising implicit compatibility.

- `1030.000 D1` is right that Vite HMR is not Exact’s runtime model and that Rolldown belongs inside bake. `D3`’s immutable hashed objects followed by a signed pointer is the correct primitive for one update stream.

- `1030.000 D4/D7` usefully place continuous, manual, scheduled, stripped, per-platform, and brownfield delivery in one policy vocabulary.

## 3. Concerns

1. **Blocking — proposed foundations are described as existing (`1030 §1`, `D1`; `1030.000` staging).**

   **Why:** `1030 §1` says production embedded-plus-update, native modules, and mixed Wasm execution already exist and that “none of that changes.” In fact, LLPs 1023, 1024, 1026, 1028, and 1029 remain Draft, and substantial parts of accepted LLP 1027 are not implemented. More importantly, the binding file still lists “Snapback / update economy” under DEFERRED at `rules/DEFERRED.md:52-58`; `rules/RULES.md:47-50` requires an explicit trade to move it. The RFC therefore obscures both implementation dependency and current authority.

   **Resolve:** Add a Built / Accepted-but-unbuilt / Proposed column to every prerequisite, state exactly which LLPs 1030 amends or supersedes, and have Charlie formally move the update economy off DEFERRED before implementation.

2. **Blocking — `runtimeVersion` does not define a safe compatibility cohort (`1030 D1/D3/D9`; `1030.000 D3/D4`).**

   **Why:** `V` omits platform, architecture, distribution profile, trust-key set, exact executor configuration, entitlement/capability set, native-code identity, shipped icon roster, and minimum embedded generation. Suppose binary B changes native Rust without changing the listed schema/ABI versions. A and B then fetch the same `V` manifest; a later plan baked against B can reach A and invoke A’s old code. The same failure occurs if B adds alternate icon `holiday` and a later bundle selects it. Signing-key rotation is impossible in one `V` namespace for the same reason.

   **Resolve:** Define an immutable per-platform compatibility descriptor, hash it into `compatibilityId`, and bake that identity into both binary and update path. Include every update-visible native capability and trust-root set. Keep separate `releaseId`, `binaryId`, `compatibilityId`, and per-stream `seq` concepts.

3. **Blocking — Levels 0/A/B are internally contradictory and not actually an ordering (`1030 D1/D3/D4`; `1030.000 D3/D4`).**

   **Why:** `1030 D1` correctly says HBC requires Level B, but `D4` says the default Level-A phone carries TypeScript updates. `1030.000 D3` shows one `a1b2` runtime covering iOS B, macOS A, and Linux 0 even though `1030 D4` says the level is recorded in the runtime version. Association files require no installed update store, so `D3` rule 2 incorrectly turns an association-only change for a Level-0 client into a binary change despite `D1` assigning it to the static origin.

   **Resolve:** Model orthogonal capabilities: update store present, accepted artifact kinds, executor fingerprints, trust epoch, and activation modes. Keep 0/A/B only as user-facing presets. A mixed app carrying Hermes is B by definition.

4. **Blocking — one carrier for the whole source delta is the wrong safety model (`1030 D3`; `1030.000 D3/D5`).**

   **Why:** Co-occurrence is not dependency. An independent TypeScript fix and entitlement edit in one commit should not necessarily withhold the safe fix from old binaries. Conversely, examining only changed artifacts misses a bundle’s dependency on unchanged but incompatible native state. A universal-link migration can require an association addition first, then a binary, then delayed association removal after the old-client tail; there is no single carrier. The developer override replaces a claimed safety invariant with an unverifiable assertion.

   **Resolve:** Bake a complete candidate snapshot against each frozen compatibility descriptor and compute a dependency-closed deployment DAG. The classifier should return ordered carrier actions, not one carrier. `--only bundle` must refuse unless the selected closure is safe. Distinguish client transactional swap, per-stream pointer atomicity, and cross-platform orchestration; only the first two can be atomic.

5. **Material — the claimed existing dev classifier does not exist (`1030 D3`; `1030.000 D1/D3`).**

   **Why:** The current watcher gathers filenames but sends every accepted edit through the same full web rebuild and `{rebuilt}` event (`host/web/dev.mjs:104-149`). It does not watch app-root `app.ts`, ordinary assets, icons, or the proposed manifest, and its extension filter excludes TypeScript, images, and fonts (`host/web/dev.mjs:104-107`). SSE clients are bare response objects and all receive the same event; there is no runtime/capability handshake (`host/web/dev.mjs:54-62`, `host/web/dev.mjs:154-165`). Calling digest comparison “microseconds” also excludes the producer work required to know those digests.

   **Resolve:** Call this new work. Introduce an artifact ownership graph, add a client compatibility handshake or client-side classification, and use a conservative path classifier for the fast dev loop while exact artifact classification runs after producers complete.

6. **Blocking — the static update and publication protocol is underspecified (`1030 D3/D5`; `1030.000 D2/D3/D8`).**

   **Why:** The document simultaneously puts the app at `U`, an envelope at `U/exact.json`, and an `exact.json` in every runtime directory. A static host cannot select the old client’s manifest without a deterministic distinct path. “Any static host” also supplies no authenticated write protocol or conditional update primitive. A committed local `deploy.lock` cannot allocate monotonic sequences across CI jobs or worktrees, and committing this tool-generated ledger conflicts with `rules/RULES.md:16` and `rules/DEFERRED.md:150`. Uploading a pointer last is safe only for a single writer and suitably configured caches; it does not make store submissions reversible.

   **Resolve:** Give installed clients a baked path such as `U/.exact/updates/<compatibilityId>/exact.json`; keep `U` as the human-facing URL. Define a publish adapter with conditional put/CAS, cache requirements, and immutable receipts. Use a global release ID for correlation and a remotely serialized `seq` per `(app, channel, compatibilityId, trustEpoch)` stream. Treat multi-platform work as a resumable saga.

7. **Blocking — the trust model is incomplete and contains a key error (`1030 D1/D9`; `1030.000 D3/D8`).**

   **Why:** The binary must contain public verification keys, never “the signing key” as `D1` states. The documents do not specify canonical signed bytes, domain binding to app/channel/cohort/sequence, key IDs, rotation overlap, compromise behavior, or trust epochs. `1030 D9` also overgeneralizes `plan/build.rs`: its domain-separated digest is format identity, while plan payload integrity is raw SHA-256, as the current dev loop demonstrates at `host/web/dev.mjs:84`.

   **Resolve:** Specify a versioned signature envelope and artifact-hash registry. Embed a verification-key set and trust epoch in the compatibility descriptor; rotate through overlapping multi-signatures or separate streams. State the rollback/freeze threat model for fresh installs as well as already-running clients.

8. **Blocking — the manifest conflates source declaration, artifact identity, and deployment policy (`1030 D2/D3`; `1030.000 D4/D8`).**

   **Why:** `1030 D2` makes the whole manifest digest binary identity, while `1030.000 D8` puts schedules, channels, release policy, live runtimes, and secret references in that same file. Changing `automatic` to `manual` would therefore become a binary change. Final provisioning profiles, signatures, toolchains, and platform-console capabilities can change the binary without changing the manifest. Build-number allocation can also make every dry run look like a binary change; today the values are merely hardcoded at `host/apple/build.mjs:172-173`. Identity is currently derived independently from the crate at `host/apple/build.mjs:48-49` and from app layout at `scripts/app.mjs:24-27`.

   **Resolve:** Canonicalize typed per-artifact and per-platform projections. `[app]` and relevant platform fields contribute to artifacts; `[deploy]` does not. Generate and validate all existing identity sites from one logical app ID, and record final toolchain, profile, signing, and external-capability inputs in an immutable build receipt.

9. **Blocking — making the LAN dev server the production deployer creates unsafe snapshots and an unnecessary credential boundary (`1030.000 D3/D5`).**

   **Why:** The current server builds into one global `host/web/dist` (`host/web/dev.mjs:39-45`, `host/web/dev.mjs:123-149`), while Apple outputs are fixed and replaced in place (`host/apple/build.mjs:51-52`, `host/apple/build.mjs:269-300`). A subsequent save can mix generations with an in-progress deploy, and the 200 ms debounce can publish the first half of a multi-file edit. A dry-run followed by `--yes` has a time-of-check/time-of-use gap. Combining this with a server bound to `0.0.0.0` makes a long-running LAN-facing process the holder of production keys.

   **Resolve:** Share one classifier/build library, but run production deployment as `exact deploy --watch` in a separate least-privileged process. Deploy an immutable commit/tree snapshot into a run-specific staging directory. Continuous production should consume verified snapshots, not mutable editor state.

10. **Blocking — the described Apple release path is not an extension of the current builder (`1030.000 D5`; `1028 F3/F5`; `1029 D3`).**

   **Why:** The current iOS path deliberately selects device-bound development profiles (`host/apple/build.mjs:112-130`), an `Apple Development` identity (`host/apple/build.mjs:133-145`), and `get-task-allow` entitlements (`host/apple/build.mjs:155-161`). It hand-assembles a development `.app` and installs it; there is no archive, export, or IPA (`host/apple/build.mjs:295-336`). The macOS path emits a bare executable, signs with a development or ad-hoc identity, passes `--timestamp=none`, and has neither hardened runtime nor entitlements (`host/apple/build.mjs:269-292`). Developer-ID distribution and Mac App Store distribution are different pipelines: the former needs a properly nested-signed app, hardened runtime, secure timestamp, notarization and stapling; the latter needs App Store distribution, sandbox/profile work, and review.

   Downloaded `.cwasm` is machine code on macOS. An executable-memory entitlement may make it run, but it does not guarantee App Store acceptance of downloaded functionality. The same policy caveat applies to Hermes/Pulley updates and especially a generic launcher; Expo or Shorebird precedent is not blanket approval. TestFlight external review, App Review, processing, legal agreements, and release are asynchronous states, not an API call that can be rolled back.

   **Resolve:** Build four explicit lanes: local Apple development, iOS distribution, direct Developer-ID/notarized macOS, and Mac App Store. Run signed-device and review-policy spikes before choosing `.cwasm` for store binaries; retain neutral Wasm/Pulley or binary delivery as fallbacks.

11. **Blocking — the runtime-upgrade and durable-state guarantees exceed the cited mechanisms (`1030 D9`; `1018 D1/D2/D6/D7`; `1007 §6`).**

   **Why:** LLP 1007’s “conform or restart” applies to carried slots and kept resource answers, not arbitrary persistent Store data. LLP 1018’s implemented secret store contains opaque strings, its plain persisted tier is unbuilt, and Linux is memory-only. Equal shapes do not imply equal semantics, and “starts fresh” is data loss rather than durable survival. Keychain continuity depends on team/signature/access group and entitlements, not merely `app_id`. The document also permits the update-store encoding to change while promising it will survive without migrations. “Works forever” is unsupportable once OS support, certificates, network APIs, or store policy move.

   **Resolve:** Separate the updater’s state from app durable state. Freeze or version the update-store codec. Give app data explicit schema epochs, invalidation, or migration rules, scoped by platform/tier. Narrow the binary guarantee to “a valid update cannot destroy the embedded fallback on a supported OS.” Gate promotion on V→V′ state, secret, crash-marker, and downgrade fixtures even if that verification remains outside the 60-second developer gate.

12. **Blocking — the Wasm/native identity scheme is not sound (`1030 D1/D3`; `1029 D3/D4`; `1028 F1-F4`).**

   **Why:** A Wasm digest cannot identify target-native Rust: `#[cfg(target_os = "ios")]`, build scripts, native dependencies, compiler flags, and target code may change while the `wasm32` output remains identical. Per-source reachability is also not defined by the single dispatch-style `answer`/`parse` exports; globals, tables, elements, indirect calls, shared helpers, and data affect semantic closure. A byte-equality fixture cannot prove absence of cross-source mutable state. Wasmtime serialized modules are trusted, engine-specific artifacts whose compatibility includes the exact engine build, configuration/tunables, and target ISA—not merely a version and broad triple. Finally, 1028’s +0.58 MB and speed figures were measured on macOS arm64, not on an iPhone (`1028 F1-F4` explicitly says the phone is owed).

   **Resolve:** Initially use whole-module identity. Track a separate target-native build-input digest. If per-source swapping is retained, require explicit source exports/provenance metadata and a defined closure algorithm. Put the complete Hermes/Wasmtime compiler-runtime receipt, configuration, target, and CPU baseline in compatibility identity.

13. **Material — the shader classification is presently false (`1030 D1/D8`; `1030.000` Stage 1).**

   **Why:** `gpu/reflect` returns generated Rust and source paths, not an interface object or digest (`gpu/reflect/src/lib.rs:40-47`, `gpu/reflect/src/lib.rs:49-82`). The generated Rust embeds the complete WGSL in `SOURCE` and `MODULE` (`gpu/reflect/src/lib.rs:184-207`), so every WGSL edit currently changes compiled Rust. Even a new digest over the currently generated text would change with every source edit. Entry-point IO, fragment outputs, overrides, required device features/limits, workgroup assumptions, and pipeline descriptor coupling also need consideration before “interface unchanged” implies safe replacement.

   **Resolve:** Split externally loaded WGSL bytes from generated interface declarations. Define a versioned canonical compatibility record and pipeline fixture. Construct and validate a candidate pipeline before replacing the last-good one; until that exists, classify WGSL with its surface as binary-coupled.

14. **Material — propagation tools promise more than the platforms and APIs provide (`1030 D5/D7`; `1030.000 D4/D6`).**

   **Why:** `BGAppRefreshTask` is discretionary, quota- and user-controlled, and can be suppressed after force-quit; it is neither a schedule nor an SLA. It also needs identifiers, launch registration, expiration handling, and appropriate binary capabilities. Silent push requires APNs/server infrastructure and background modes. The `apply` configuration conflates checking with activation and even gives a two-value default (`next-launch + background`) to a single-valued row. LLP 1012’s `state` is an agent diagnostic operation; Contract cannot read `state.delivery.update` or issue the described apply action through it. Access logs count requests, not clients, so `exact reach` is not a census. A sunset card is advisory and works only for checking A/B clients; Level 0, offline, or pre-card binaries cannot be forced.

   **Resolve:** Split `check` from `activate`; describe background refresh as opportunistic staging. Define a normal app-facing `delivery.update` resource and concrete check/apply commands, with agent `state` merely mirroring them. Rename reach to recent request-weighted activity unless a privacy-reviewed install identifier is introduced. Call sunset an advisory retirement notice.

15. **Blocking — the one-URL mobile affordance omits half of universal/app links (`1030 D1/D10`; `1030.000 D2`).**

   **Why:** An AASA file is insufficient: the iOS binary needs the associated-domains entitlement. Android needs manifest intent filters and certificate fingerprints as well as `assetlinks.json`. A generic launcher cannot dynamically claim arbitrary customer domains because its entitlement is signed into its binary. A LAN HTTP URL is not a normal universal link, and typing/navigating within Safari is not guaranteed to transfer to the app. Adding or removing domain ownership is an ordered expansion/contraction migration because association files are OS/CDN cached. The launcher and an app-specific binary also cannot ambiguously own the same path without an explicit mapping.

   iOS alternate icons are correctly limited to a shipped set, but `setAlternateIconName` is asynchronous, foreground/system-mediated, and user-visible; it is not a silently applied plan property.

   **Resolve:** Specify three distinct paths: typed/QR/mDNS entry for LAN development, an Exact-controlled link domain for a generic launcher, and app-specific universal links baked into app binaries. Define path ownership and association migration ordering. Model alternate-icon selection as an explicit user/system action and include the shipped roster in compatibility identity.

16. **Material — the Web, Linux, and Android platform table is too optimistic (`1030 D10`; `1030.000 D2/D5/Q6`).**

   **Why:** A page title or favicon can change on navigation, but an installed PWA’s icon, name, and splash are browser-controlled and cached; an already-open page does not become current merely because `index.html` was replaced. Service-worker lifecycle and CDN/browser cache headers are absent. On Linux, a running app may change its window title/icon, but an installed `.desktop` file and theme icon are package-managed and cached; publishing a binary download does not update an installation. Android would require AndroidManifest generation, intent filters, signing certificates, AAB/APK and ABI handling, version codes, Play signing and tracks, permissions/resources, WorkManager/background behavior, in-app-update choices, and downloaded-code policy. It is not “association file plus Play API.”

   **Resolve:** Separate live content from installed-shell metadata and from acquisition/package updates. Define web cache/service-worker policy explicitly. Keep Android out of v1 if desired, but replace the two-row claim with a complete deferred platform lane.

17. **Blocking — `D1` still lacks layers needed for a claim of completeness (`1030 D1/D6`).**

   **Why:** Missing nodes include:

   - The binary-pinned grant/admission ceiling; a bundle cannot silently acquire stronger grants.
   - Public trust roots versus private signing credentials, key rotation, and trust epochs.
   - Update-store schema, disk quota/GC, resumable download, corruption handling, and crash-consistent selection markers.
   - Platform/architecture/minimum-OS/executor-configuration receipts.
   - Distribution artifacts such as archives, IPAs, signed app bundles, packages, repository metadata, notarization tickets, and store build trains.
   - External platform state: App ID capabilities, profiles, APNs, associated domains, privacy/export declarations, store agreements, and review status.
   - App durable-data schema and migration/invalidation.
   - Service workers, browser/CDN caching, TLS/DNS ownership, and origin migrations.
   - Widgets, extensions, helpers, and background agents as separate signed targets.
   - Acquisition/package updating, which is different from publishing a downloadable binary.

   `D6` may declare some of these out of scope, but then the theory is not yet “every layer.”

   **Resolve:** Add them as artifact, capability, or external-control-plane nodes, each with identity, owner, carrier/action, compatibility effect, and verification. Explicitly narrow v1 where appropriate.

18. **Material — brownfield and dual-publish costs are understated (`1030 D9`; `1030.000 D7/Q7`).**

   **Why:** `brownfield: true` cannot safely publish every bundle merely because Exact declines to build the outer binary; it must know which capabilities, grants, executors, native modules, store layout, threading rules, file protection, and lifecycle the host actually shipped. Likewise an Exact commit is not a reproducible toolchain: it does not pin Rust, Node, Hermes, Wasmtime configuration, Xcode/SDK, dependencies, or app source. Current resolution explicitly binds scripts to the current Exact root and workspace (`scripts/app.mjs:17-27`). Running arbitrary old build code with current production keys is also a supply-chain boundary. Two live versions therefore cost much more than “a second bake” or “double the static directory.”

   **Resolve:** Define a signed brownfield host descriptor and embedding SPI, with “outer host rebuild required” as a classifier action. For dual publish, use a content-addressed toolchain capsule plus dependency lock and app-source reference, with the current publisher handling credentials. If that is too expensive, support freeze plus explicit emergency backports first.

19. **Minor — staging and estimates are out of dependency order (`1030.000` Stages 1–3; `1030 D9`; `1030.000 D4`).**

   **Why:** Stage 1 expects an interface-changing shader edit to report a native rebuild, while the reflected-interface classifier arrives in Stage 3. “TestFlight is an hour” is an illustration, not a bound, and same-`seq` simultaneous publication across divergent streams is not generally achievable.

   **Resolve:** Move the shader acceptance case to the classifier stage, label store timings as non-guaranteed observations, and use a release ID—not equal sequence counters—to correlate platform publications.

## 4. Suggestions

1. Write a short delivery-protocol revision defining artifact graph, compatibility descriptor, `releaseId`, stream ID, signed head, client selection, rollback, and the multi-platform saga.  
   **Cost:** Several focused design days plus envelope/update-store fixture changes.

2. Start v1 with one origin adapter, one channel, one writer, Level A bundles, manual/internal native release, and embedded fallback. Add code OTA and automatic production only after the protocol survives failure tests.  
   **Cost:** Delays the broadest policy matrix but substantially reduces the first security and operations surface.

3. Build a manifest compiler that emits canonical per-artifact projections and a signed build receipt rather than hashing TOML wholesale.  
   **Cost:** Low-to-medium implementation cost and migration of current crate-derived identities.

4. Separate the fast dev observer, privileged deploy watcher, unprivileged builders, and isolated signers while sharing one library.  
   **Cost:** Another process boundary, persisted job state, and CI wiring.

5. Run five physical platform spikes before ratification: iOS archive/TestFlight, direct notarized macOS, Mac App Store with the chosen Wasm artifact, real-device background refresh, and installed-PWA metadata/cache behavior.  
   **Cost:** Several engineering days plus credentials and potentially unbounded review waiting time.

6. Add failure fixtures for concurrent deploys, stale writers, missing blobs, cache skew, interrupted downloads, disk full, corrupt manifests, key rotation, crash before healthy mark, and V→V′ state carry.  
   **Cost:** Medium-sized asynchronous integration suite and a disposable test origin.

7. Specify ordered expansion/contraction procedures for trust keys, universal-link domains, backend APIs, runtime retirement, and native capability rosters.  
   **Cost:** Low design cost; medium orchestrator/state-machine cost.

8. Produce one real brownfield sample with a host descriptor and lifecycle API before claiming the general case.  
   **Cost:** High relative to the core bundle slice; reasonable to defer until after first-party hosts work.

## 5. Positions on the open questions

### 1030 §5

1. **Position:** Level 0 should be a distinct link-time stripped profile, internally represented as `updateStore: absent`, not Level A with checks disabled.  
   **Reason:** Charlie asked for stripped binaries, and removing updater/trust/store code reduces both size and attack surface.

2. **Position:** Keep atomic installable snapshots per compatibility stream, but reject whole-source-Δ atomicity and the semantic override.  
   **Reason:** A dependency-closed graph plus signed pointer prevents half-bundles without withholding unrelated safe changes.

3. **Position:** Include the sunset card in v1, but name it an advisory retirement notice rather than forced update.  
   **Reason:** It is useful for checking A/B clients but cannot reach Level 0, offline, or incompatible old clients.

4. **Position:** Put diagnostic delivery data in the existing agent `state`; add a separate normal runtime resource and commands for Contract.  
   **Reason:** There should be no ninth agent operation, but Contract cannot consume the agent-only diagnostic carrier.

5. **Position:** Freeze should be the default; emergency backport may be explicit, and general dual-publish should be opt-in.  
   **Reason:** Safe dual-publishing requires hermetic old toolchains and source compatibility, not merely another directory.

6. **Position:** No—the current reflected interface is neither computed as a digest nor sufficient yet.  
   **Reason:** WGSL is embedded in generated Rust today, and the compatibility record omits relevant pipeline and device requirements.

### 1030.000 §5

1. **Position:** Default to continuous verified web/bundle publication and internal native builds; production binary promotion should be explicit or phased.  
   **Reason:** Store submissions are slow and partially irreversible, while watch-on-save can observe incomplete edits.

2. **Position:** Use a separate `exact deploy --watch` process sharing the same implementation library with `dev.mjs`.  
   **Reason:** One code path does not require one privilege boundary or one mutable output directory.

3. **Position:** Use a separate TOML manifest with typed sections; exclude deployment-control fields from artifact identity.  
   **Reason:** Entitlements do not belong in Contract, but schedules and secret references do not belong in binary compatibility either.

4. **Position:** Use a dedicated least-privilege secret manager/KMS and isolated signer, with separate OTA, App Store Connect, and Developer-ID credentials.  
   **Reason:** Local Keychain use is suitable for local development, not sufficient as the cloud trust architecture.

5. **Position:** Ship the launcher through TestFlight/developer distribution first; treat App Store release as a separate product and policy decision.  
   **Reason:** Arbitrary app loading, domain ownership, downloaded functionality, grants, and review need real evidence.

6. **Position:** Keep Android on DEFERRED for v1, but do not confirm that only two platform-specific rows are needed.  
   **Reason:** Android requires a complete build, signing, packaging, links, lifecycle, background, store, and policy lane.

7. **Position:** Pin a content-addressed toolchain capsule, dependency lock, configuration receipt, and app-source revision; neither a tag nor a raw commit alone suffices.  
   **Reason:** A commit identifies source but does not reproduce compilers, SDKs, dependencies, executor settings, or old app input.

## 6. Named disagreements to argue in round 2

1. **Whole-change atomicity versus dependency-closed release graphs.** I expect the author to defend “heaviest carrier wins” as conservatively safe; my position is that it is simultaneously over-conservative for independent changes and unsafe when unchanged native dependencies are omitted. Round 2 should force a choice of publish unit and whether any human safety override remains.

2. **One dev-server process and automatic production versus one library with separate trust boundaries.** I expect disagreement over whether separating `exact deploy --watch` betrays the “one loop” goal. My position is that one user-facing workflow does not justify giving a LAN watcher production credentials or deploying mutable save state.

3. **“Conform or restart” as a durable-state migration policy.** I expect the author to view existing reload semantics as sufficient. My position is that the document must either add state epochs/migration/invalidation with platform-scoped guarantees or retract the claim that durable state survives runtime upgrades.

## 7. Recommended next step

**Revise and stay Draft.** The blockers are protocol-level and factual, not matters another family needs to discover: compatibility cohorts, multi-carrier ordering, trust rotation, static publication concurrency, state migration, and platform release boundaries must be rewritten first. After that revision, round 2 should test the disagreements above and require concrete results from the Apple, background-refresh, shader, and concurrent-publish spikes before the author asks Charlie to accept.
---

## Round 2 — deltas after reading the grok round-1 answer (verbatim)

## Corrections and extensions

- Grok endorses too much of the short theory. Assigning a lightest carrier to each artifact is useful; reducing an entire change to one carrier is not. Domain association and trust-key rotations require ordered origin → binary → tail wait → cleanup actions, which D3 cannot represent (`llp/1030-delivery-unified.rfc.md:20-29`, `147-173`).

- Grok B2 overstates that D3 necessarily freezes the web. The web explicitly has no runtime version, and its hashed files plus `index.html` publish separately (`llp/1030-delivery-unified.rfc.md:111`; `llp/1030.000-dev-server-as-deployer.rfc.md:149-155`). The real defect is that publication scope and ordering are undefined, not that the text unambiguously forbids the web publish.

- Grok M9 incorrectly implies that 1029 proposes downloaded native `.cwasm` on iOS. Its iOS target is `pulley64`, and the device executes interpreted Pulley bytecode (`llp/1029-mixed-logic-and-engine-choice.rfc.md:193-206`, `242-245`). Native AOT must indeed be refused on iOS; the unresolved concern is downloaded executable code on macOS.

- “Developer ID + notarization will pass with a justification” is too certain. The cited research expressly leaves the required Wasmtime entitlement unverified (`llp/1028-wasm-engines-measured.research.md:163-172`). Notarization also does not establish Mac App Store policy acceptance. Execution is settled by a signed/notarized physical build; store policy only by an actual submission. Additionally, `notarytool` does not accept a bare `.app`; it must be packaged as ZIP, DMG, or PKG before submission (`llp/1030.000-dev-server-as-deployer.rfc.md:218-223`).

- The universal-link statement “the tap must come from another app” is too categorical. Safari can open an eligible cross-domain universal link; same-domain navigation generally stays in Safari, and typing/pasting is not reliable. Grok’s central conclusion remains: LAN HTTP from `dev.mjs` cannot be the universal-link mechanism (`host/web/dev.mjs:38`, `223-234`).

- Generic updater networking is not itself an Apple Required Reason API. Grok’s proposed privacy-manifest row should instead be derived from the APIs used, data collected, and SDK declarations. Likewise, a dSYM is not a D1 client-delivery layer: D1 defines artifacts moved to installed clients (`llp/1030-delivery-unified.rfc.md:88-89`). Symbols belong in 1030.000’s release outputs and receipts.

- Grok M11 is right that promotion needs a new destination `seq` and signature, but “signature over the wrong origin” is not an existing protocol check. LLP 1026 signs canonical envelope bytes but does not define app/channel/origin domain binding (`llp/1026-dynamic-delivery.rfc.md:569-575`). That binding is missing security protocol, not an already-enforced rule.

- Grok M15 overreads `RULES.md`: implementer/date is mandatory for a Spec, not an exploratory Draft RFC (`rules/RULES.md:58-60`). 1030.000 says the same (`llp/1030.000-dev-server-as-deployer.rfc.md:347-351`). Splitting it is still warranted on scope and trust-boundary grounds.

- The agent-operation count is governed directly by LLP 1012 and `DEFERRED.md`, not primarily “LLP 1024’s count rule” (`llp/1012-agent-api-v1.spec.md:33-42`, `106-109`; `rules/DEFERRED.md:109-116`).

Important blockers Grok missed:

- 1030 presents update delivery as existing and unchanged, although 1026 and 1029 remain Draft and the update economy remains on binding DEFERRED (`llp/1030-delivery-unified.rfc.md:53-82`; `llp/1026-dynamic-delivery.rfc.md:805-809`; `rules/DEFERRED.md:52`; `rules/RULES.md:47-50`). Charlie must explicitly approve the trade 1026 proposes before acceptance.

- `runtimeVersion` is not a binary compatibility cohort. It omits platform, architecture, distribution profile, native capability identity, accepted artifact kinds, executor configuration, trust roots, update-store codec, and shipped icon roster (`llp/1030-delivery-unified.rfc.md:64-72`). The contradiction is visible: 1030 says level is included in `V`, while 1030.000 shows one `a1b2` shared by three different levels (`llp/1030-delivery-unified.rfc.md:209-213`; `llp/1030.000-dev-server-as-deployer.rfc.md:134-141`).

- D1 puts “the signing key” in the binary; it must contain public verification keys (`llp/1030-delivery-unified.rfc.md:109`; `llp/1026-dynamic-delivery.rfc.md:569-575`). Canonical signed bytes, stream/domain binding, key IDs, rotation overlap, trust epochs, and compromise handling remain unspecified.

- The manifest conflates artifact identity with deployment policy. 1030 makes its digest part of binary identity, while 1030.000 puts schedules, channels, live runtimes, secret references, and release policy in the same file (`llp/1030-delivery-unified.rfc.md:121-134`; `llp/1030.000-dev-server-as-deployer.rfc.md:163-179`, `293-311`). As written, changing `release: automatic` to `internal` becomes a binary change.

- Native Rust cannot be identified by the corresponding Wasm digest: target `cfg`, build scripts, native dependencies, compiler flags, and toolchains can alter native behavior without altering the Wasm (`llp/1030-delivery-unified.rfc.md:96-98`). The per-source reachability scheme also does not define closure over tables, indirect calls, globals, or shared state (`llp/1029-mixed-logic-and-engine-choice.rfc.md:247-277`).

- Grok’s praise for D9’s constants should be withdrawn. “Durable state survives” contradicts “starts fresh,” and “forever” cannot cover future OS or certificate changes (`llp/1030-delivery-unified.rfc.md:396-424`). D9 also conflates the domain-separated format digest with raw payload SHA-256 (`llp/1023-one-url-serving.rfc.md:145-151`; `host/web/dev.mjs:84`).

- Agent `state` does not provide the Contract-facing update API that D5/D6 assume. Diagnostics belong under `state.delivery`; application code needs an ordinary resource plus explicit check/apply commands (`llp/1030-delivery-unified.rfc.md:274-277`, `315-335`).

- `exact reach` cannot count clients without an installation identifier. Access logs count requests, so the honest output is recent request-weighted activity, not a census (`llp/1030-delivery-unified.rfc.md:283-288`; `llp/1030.000-dev-server-as-deployer.rfc.md:257-263`).

## What Grok caught that I missed or sharpened

- Grok sharpened my capability-axis concern into the right deletion: Level B should not remain an independently authored setting.

- Grok caught that channel promotion must generate and sign a destination head rather than literally copy one (`llp/1030.000-dev-server-as-deployer.rfc.md:193-201`).

- Grok caught the stale “no browser JavaScript app” sentence and the nonexistent promised refusals section (`llp/1030.000-dev-server-as-deployer.rfc.md:28-33`; `llp/1030-delivery-unified.rfc.md:30-35`).

- Grok caught that today’s Apple host is an application host, not the brownfield embedding SPI D7 says it “already has” (`llp/1030.000-dev-server-as-deployer.rfc.md:277-291`).

- Grok caught the Play App Signing certificate requirement, localized display names, dark/tinted icon slots, and the need to call the alternate-icon API only when selection changes.

## Final positions on the disagreements

1. **Level B:** remove it as an authored protocol/configuration axis. Use `updateStore: absent|present` plus binary-derived accepted artifact kinds and executors; “B” may remain derived reporting shorthand. The current classifier sends the default phone’s HBC-only edit to a binary despite D4 saying TypeScript travels (`llp/1030-delivery-unified.rfc.md:153-160`, `215-223`).

2. **Whole-change atomicity:** reject “heaviest carrier wins.” Keep atomic installed snapshots and atomic signed stream heads, but produce a dependency-closed ordered deployment DAG across origin, bundle, binary, and external platform actions. Remove the human safety override; independent work can be published by splitting and rebaking independently verified snapshots.

3. **Dev server versus deployer:** one implementation library and user-facing workflow, separate processes. `dev.mjs` observes and classifies; `exact deploy --watch` or CI snapshots, signs, publishes, and resumes slow jobs. A LAN-bound watcher over mutable global output must not hold production credentials (`host/web/dev.mjs:38-45`, `104-150`).

4. **Release default:** continuous for verified web and bundles; internal distribution for native binaries; explicit production promotion. App Review and release are asynchronous external state, not another atomic carrier.

5. **Durable state:** retract the blanket survival guarantee. Version updater state separately; give app state explicit epochs and reset semantics; state the signing-team/access-group requirements for secrets. The V→V′ fixture must test updater records, secrets, crash markers, and downgrade behavior.

6. **Freeze versus dual-publish:** freeze only in v1. Dual-publish creates the pre-1.0 compatibility burden `RULES.md` forbids; reconsider it only with hermetic historical toolchains and immutable inputs.

7. **Shader digest:** the current reflected output is insufficient. It embeds WGSL in generated Rust and omits fragment outputs, overrides, device requirements, and Rust-owned pipeline coupling (`gpu/reflect/src/lib.rs:184-207`, `210-239`, `738-834`). Settle this with externalized WGSL plus incompatibility fixtures and candidate-pipeline validation on Chrome, Metal, and Vulkan; until then WGSL is binary-coupled.

8. **One URL:** production may use one HTTPS app identity, envelope origin, and deep-link authority. Development still enters a launcher/dev client through a typed, scanned, or discovered LAN URL; universal links are not that mechanism.

9. **`state.delivery`:** keep it inside the existing agent `state`, but separately define the normal app-facing delivery resource and commands. No ninth agent operation.

10. **Secrets and old toolchains:** one managed secret backend is acceptable, but OTA, App Store Connect, and Developer-ID credentials need distinct keys, principals, and scopes. If dual-publish returns, neither a tag nor a commit is sufficient; require a content-addressed toolchain/SDK/dependency capsule and app-source revision.

## Severity reconciliation

| Concern | Grok | My round 1 | Final |
|---|---:|---:|---|
| Static routing, cache propagation, CAS, sequences, and `deploy.lock` | Material | Blocking | **Blocking.** Hashed-files-first cannot guarantee that a newly visible head’s blobs are available across CDN edges, and a local lockfile cannot serialize writers. |
| Production deployment from `dev.mjs` | Material | Blocking | **Blocking for 1030.000 as written.** Mutable save state, LAN exposure, production credentials, and resumable store jobs cannot share this process. |
| Durable-state migration | Minor | Blocking | **Blocking while D9 states it as a guarantee.** Narrowing it to explicit reset semantics would reduce the residual implementation work to material. |
| Missing D1 inventory | Material | Blocking | **Blocking** for compatibility, trust, updater-state, platform-receipt, external-control-plane, and durable-data nodes; **material** for localized names, icon appearances, and release-side symbols. |
| Native `.cwasm` on Apple | Material | Blocking identity concern | **Not an iOS defect as currently targeted; blocking for any unverified Mac App Store lane, material wording otherwise.** |
| Splitting 1030.000 | Material | Not raised | **Material.** The rules citation was wrong, but stages 5–8 are separate programs with different owners and trust boundaries. |

Additional final blockers absent from Grok’s grading are the unresolved DEFERRED trade, compatibility identity, verification-key protocol, manifest projections, and native/Wasm identity.

## The one change

- **1030:** replace D3/D4 with one per-platform compatibility-stream artifact graph: immutable compatibility descriptor, capabilities derived from the binary, complete-snapshot dependency closure, and an ordered deployment DAG. This single rewrite resolves the unsafe `runtimeVersion`, duplicate Level-B authority, false one-carrier rule, association migrations, and trust rotation.

- **1030.000:** sever production publication from the LAN dev-server process. Preserve classify-on-edit, but make a separate least-privileged deploy process consume an immutable snapshot. This removes the largest avoidable security and consistency failure without sacrificing one command or one shared implementation.

## Open-question changes only

- **1030 §5.5:** changed to freeze-only in v1; dual-publish is deferred until post-1.0 compatibility and hermetic historical builds are allowed.

- **1030.000 §5.7:** consequently moot for v1; if reopened, pin a content-addressed toolchain/build capsule, not a commit or tag.

No other 1030.000 answer changed from my round-one position.

## Recommended next step

Keep both Draft; another model family is unnecessary now. Charlie should first approve or reject the required DEFERRED trade. If approved, the author should revise 1030 around the compatibility graph/DAG, recut 1030.000 to the bundle-delivery slice, and leave production binary automation, landing, reach/background behavior, and brownfield for follow-on LLPs.

Before those later lanes are accepted, run four proofs: concurrent/CAS publication under cache skew, externalized shader compatibility across real backends, signed iOS/direct-macOS/Mac-App-Store distribution, and V→V′ updater/app-state carry.