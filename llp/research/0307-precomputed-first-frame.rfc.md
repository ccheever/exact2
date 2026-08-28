# LLP 0307: Precomputed First Frame — Native First Paint Before JavaScript

- **Type:** RFC
- **Status:** Implemented (curation pass 2026-08-23; was "Accepted (Phase A shipped `8d94f42a`; Phase B v0 shipped — §8; Phase C shipped — §9)")
- **Systems:** native-host (macOS/iOS runtime shell), engine, binary protocol, startup performance
- **Author:** Claude (with Charlie Cheever)
- **Date:** 2026-07-05
- **Revised:** 2026-08-26 (**§12.1 added** — `listSeeds` re-specified per RFC 0540 §8.8 / LLP 0543 §12.7: the seed becomes the persisted form of one semantic anchor intent (`item | boundary | none`, keys never raw addresses) plus retained boxes and head/tail indices; it is consumed at replay by the presenter and layout engine rather than by the Contract runtime after bundle eval, replacing §12's one-shot global handoff; JS-windowed EXFF artifacts regenerate at 0540's L4 so pre-JS pixels are extents + retained boxes; §12's no-flash acceptance is unchanged and joins the `list-chat-fixture` physical arm, which is the harness the open device row was waiting for. Executes 0504 §3 row 61; delivery post-window per 0540 §8.6. 0506 D1(d) burn-down lane.) 2026-08-25 (§13 correct-enough-by-declaration enumeration + §14 plan-runner generation join, per LLP 0561 §4.3's honesty rule and `issues/closed/20260824-lp-exff-residue.md` items 2/6; §13 is the list the L-F/L-P checks cite. Additive amendment to an Implemented document — no mechanics changed.) 2026-07-14 (§12 eligible-replay list-seed handoff and first-window adoption; physical-device and quiet-machine rows remain open)
- **Related:** LLP 0297 (OQ11, §8 cold-start metric), LLP 0299 §5, LLP 0145 item 5, LLP 0128, LLP 0252, LLP 0561 §4.3 (L-P consumes this machinery; §13 here is its enumeration rule), ENG-22826, ENG-22779

## 1. Problem

On the last paired cold-start matrix (LLP 0145, 2026-06-06, release packaged-HBC
lane) Exact reached first frame in **1011ms p50** vs. React Native's 754ms, and
Lynx targets the RN-or-better class via Instant First-Frame Rendering. The
phase breakdown (LLP 0299 §5) shows the entire gap is JavaScript: Hermes VM
creation ~4ms, HBC read ~0ms, native protocol dispatch + Taffy layout ~7ms —
but bytecode eval ~246ms, dominated by ~230ms of JS module pre-entry.

The asymmetry is the opportunity: Exact's first frame is, materially, a binary
opcode buffer fed to the Rust kernel. Producing that buffer is the only step
that needs JavaScript — and for a returning user (or, later, a static route at
build time) the buffer already exists. Lynx cannot skip its JS: its first
frame always runs MTS to build the element tree. Exact can paint with zero
app-JS eval.

## 2. Design overview — three phases

- **Phase A (v0, shipped with this RFC):** runtime-captured artifact. Capture
  the dispatch batches of a cold boot's first successful render; on the next
  launch, replay them into the fresh kernel **before any JS evaluation** and
  paint. When the real bundle evaluates and its first dispatch arrives, the
  existing bundle-replacement machinery resets the kernel and applies the live
  tree in the same main-thread turn — an atomic discard-and-replace. Opt-in
  via `EXACT_PRECOMPUTED_FIRST_FRAME=1`. macOS is the measured lane; the code
  is platform-neutral where free.
- **Phase B (build-time artifact):** emit the same container from the Contract
  IR + static/preset data at `exact build` time (LLP 0145 item 5's "precomputed
  first app-content frame"), memory-mapped at launch. This is what makes
  *first installs* fast, not just returning users, and is the true
  beat-Lynx lever. Needs deterministic op emission from the Contract
  compiler's IR, font/asset preregistration, and Phase-C relayout.
  *Status (ENG-22893): v0 shipped — see §8.*
- **Phase C (flip integration + adoption-proper):** produce the boot-window
  first frame under `EXACT_RUNTIME_THREAD=1` at the LLP 0297 A1 ownership-
  handoff point so frame 1 never crosses the thread flip; replace v0's
  discard-and-replace with true adoption (no-op diff when the live tree
  matches, LLP 0252's "structural differences rejected with diagnostics"
  ported natively); host-side relayout of the replayed tree at the current
  viewport (the LLP 0161 live-resize machinery already proves host-driven
  relayout without JS).

## 3. v0 mechanics (Phase A, as implemented)

### Artifact

A single file, `Caches/<bundle-id>/precomputed-first-frame.v1.exactframe`:

```
"EXFF" magic | u32 formatVersion | u32 headerLen | header JSON
| u32 bufferCount | { u32 byteLen | dispatch-buffer bytes }*
```

The header carries the staleness key and the parsed surface root ids:

- `sourceURL` — normalized custom/dev-server URL (app + initial route identity)
- `protocolVersion` — from the captured dispatch headers (diagnostic)
- `buildFingerprint` — host executable mtime + version; protocol tables and
  prop decoders may drift between builds, so replay never crosses builds
- `appearance` — `light` / `dark`; buffers embed appearance-resolved colors
- `viewportWidth/Height` — `ComputeLayout` ops embed width/height, so v0
  replays only on an exact viewport match (Phase C removes this)

### Capture

The first successful drain after a cold boot's bundle eval *is* the first
frame. Both drain seams call the same armed-once capture hook
(`capturePrecomputedFirstFrameIfArmed`): the post-eval
`processPendingDispatches` path and the dispatch-callback inline path (apps
that mount asynchronously deliver their first frame there). v0 captures only
primary-surface boots (`rootIds == {0}`); the artifact is written atomically
off-main and overwritten on every capture, so it is at most one
bundle-generation stale.

### Replay + paint

`paintPrecomputedFirstFrameIfEligible()` runs at the top of
`ensureStartupBundleLoaded()` — after the engine + kernel exist and the
viewport is known, before the dev-server connect / embedded-bytecode load that
leads to JS eval. Replay feeds each buffer through
`engine.dispatchBuffer(_, postProcess: false)` and finalizes once with the
artifact's root hints — the same parse/layout/present path live dispatches
use. AppKit commits the frame at the end of that main-thread turn, ~10ms
later, while fetch + eval still have hundreds of ms to run.

### Hydration (v0 = atomic discard-and-replace)

`completeBundleEval` already sets `shouldResetEngineOnNextDispatch = true`
after every successful eval, and both drain paths run
reset-prune-apply synchronously in one main-thread turn — so the first live
dispatch atomically replaces the precomputed tree with the real one, with no
intermediate blank frame (single CA commit). When bundle and route are
unchanged — the common cold-start case — the replacement is pixel-identical
by construction. The replay path also sets the same flag itself,
belt-and-braces. (The ENG-22756 defense-in-depth comment anticipated exactly
this boot order: "embedded first paint, then dev takeover".)

### Failure containment

Any key mismatch skips replay with a logged reason. A replay that nets zero
ops clears the artifact (poisoned) and resets the engine. Under
`EXACT_RUNTIME_THREAD=1` v0 skips replay entirely (the tree domain is
executor-confined; boot-window integration is Phase C). Hermetic test runs
never reach the seam, and the flag defaults off everywhere.

## 4. What v0 deliberately does not do

- **No font preregistration.** Custom fonts register during JS boot, so the
  preview lays out with fallback metrics (a native FOUT analog) until the live
  frame replaces it. Phase B bundles font preregistration.
- **No interactivity during the preview window.** Handler ids in the replayed
  tree have no live JS behind them; presses in the ~fetch+eval window are
  dropped. Same constraint class as Lynx IFR.
- **No flag-on (runtime-thread) support** — Phase C.
- **No secondary windows** (`rootIds == {0}` guard).
- **No cross-viewport relayout** — exact-match key instead (Phase C).
- **Stale-by-one-generation exposure:** the artifact reflects the previous
  boot's bundle; if the bundle changed, the user may glimpse the old UI for
  the fetch+eval window. Bounded by overwrite-on-every-boot, the
  buildFingerprint key, and the opt-in flag; Phase B artifacts are exact by
  construction.

## 5. Measurement

Protocol: `scripts/measure-precomputed-first-frame.mjs` — spawns the app under
a PTY, parses NSLog wall-clock timestamps for `Precomputed first frame
painted` and `First render ready` against spawn time, N runs each mode,
reports p50/p90. Results recorded on ENG-22826 and in the §8 table
(first-commit-to-first-pixel) once the LLP 0297 W5 metrics land: the v0
acceptance is *painted-before-eval* (precomputed paint strictly earlier than
the JS-bound first render, with the gap ≈ fetch + eval time), plus no
flag-off regression.

## 6. Risks

- **Wrong-content flash** on bundle/route drift within an unchanged
  fingerprint+URL key (dev-server edits between boots). Accepted for the
  dev-lane v0 behind its flag; Phase B removes it for release.
- **Protocol drift** across builds — guarded by `buildFingerprint`.
- **Cached UI content on disk** (privacy) — dev opt-in only in v0; Phase B
  artifacts ship in the app bundle instead of Caches.

## 7. Open questions

- OQ-A: should Phase B artifacts be per-route (navigation cold starts) or
  boot-route only? **Answered by ENG-23323: per-route, via a route
  dimension in the artifact file name** — a shared route-token derivation
  (normalized sourceURL path → `route-<token>`, byte-paired between
  `precomputedFirstFrameFileName` in the JS codec and
  `ExactPrecomputedFirstFrame.routeToken(forSourceURL:)`) names bundled
  AND captured artifacts; the root route keeps the historical unsuffixed
  names. Cold boots into deep routes replay their own artifact and
  capture to their own store file. Staging is one generator run per route
  (cost scales linearly; stage the routes worth paying for). In-app
  navigation-time replay (painting a precomputed frame on route change
  inside a running app) remains out of scope — see §11.
- OQ-B: appearance-pair capture (persist light+dark variants) vs. re-resolving
  semantic colors natively at replay. *(Phase B v0 answers this for bundled
  artifacts: the generator renders both appearances and the host picks the
  file matching its current appearance at boot.)*
- OQ-C: whether Phase C adoption should reuse the web adoption diagnostics
  shape (LLP 0300/0252) for parity of tooling. **Answered by Phase C: yes**
  — the native result is `{adoptedNodes, recreatedNodes, addedNodes,
  removedNodes, mismatches}`, the LLP 0300 vocabulary.

## 8. Phase B v0 (ENG-22893, shipped)

The build-time generation path exists and is measured; the pieces:

- `scripts/generate-precomputed-first-frame.mjs` mounts a `.contract` route
  headlessly on the **protocol path** — the same host-ops → `BufferWriter`
  pipeline the app JS runs natively, with `exact.dispatch` stubbed to collect
  buffers and `exact.screenWidth/Height` pinning the viewport — and writes
  one EXFF artifact per appearance
  (`precomputed-first-frame.<appearance>.exactframe`). Layout and text
  measurement are NOT baked in: buffers carry the tree + a ComputeLayout
  command, executed by the kernel at replay, exactly as for live dispatches.
  The module graph resolves with native platform variants
  (`exactPlatformVariantBuildPlugin({platform: 'native'})`), and appearance
  is forced through the theme store's host input
  (`setThemeSystemAppearance`). `--from-captured` copies sourceURL/viewport
  from a Phase A capture so generated keys match the host boot identity.
- The container codec is duplicated by design in
  `packages/exact-devtools/src/precomputed-first-frame.ts` (JS writer) and
  `ExactPrecomputedFirstFrame.swift` (host reader); the byte layout is pinned
  by tests on both sides. Change them together or not at all.
- Host lookup order at paint: runtime-captured artifact (Phase A key policy)
  first, then the bundled artifact — from
  `EXACT_PRECOMPUTED_FIRST_FRAME_BUNDLED_DIR` (dev/measurement seam) or the
  app bundle resource. Bundled validation skips `buildFingerprint` (the host
  executable is unknowable at generation time; artifacts are exact by
  construction against the JS that shipped with the build) and keeps the
  sourceURL/appearance/viewport checks. Protocol drift fails safe through
  the existing zero-ops containment; a bundled zero-ops replay is ignored
  for the process rather than cleared.
- `scripts/measure-precomputed-first-frame.mjs --mode bundled` simulates a
  first install by deleting the Caches artifact before every run and reports
  the painted origin (`bundled` vs `captured`) so a leaked capture cannot
  fake the numbers.

Phase B remainders are tracked on ENG-22922 (the productization split off the
closed ENG-22826 umbrella):

- **CI metric fixtures — done (ENG-22922).** Two pinned Contract fixtures
  (`js/src/__tests__/fixtures/first-frame/{static,dynamic}.contract`) are
  regenerated through the shipping generator by
  `scripts/check-precomputed-first-frame-metrics.mjs`
  (`precomputed-first-frame-metrics` in `exact-verify.json`, contract profile;
  fast in-process rot guard in
  `precomputed-first-frame-metric-fixtures.test.ts`). **Determinism is now
  characterized:** given a fixed `(route, appearance, viewport, bundle)` the
  dispatch buffers are byte-identical run-to-run — the *only* EXFF variance is
  the `capturedAt` header timestamp, which the check excludes by comparing
  decoded buffers, not raw file bytes. The two fixtures pin the two ends of the
  characterization: the static fixture is appearance-invariant (explicit
  colors → light and dark buffers collapse to one digest) and the dynamic
  fixture is appearance-dependent (theme-derived colors → light ≠ dark, proving
  the generator's appearance forcing reaches the dispatched frame). This closes
  the last unmet ENG-22826 acceptance bullet.
- **Bundle-resource packaging — done (item 5, ENG-22922).** Every native
  build now packages whatever is staged in
  `ios/ExactApp/ExactApp/Resources/first-frame/` into the app bundle as the
  `first-frame` folder reference, on both the ExactApp (iOS) and ExactAppMac
  targets (`project.yml`, mirroring `Resources/bytecode`), and
  `ExactPrecomputedFirstFrame.bundledArtifactURL` reads that subdirectory
  (env-override dir first, Resources-root fallback kept). Staging is one
  command — `bun scripts/generate-precomputed-first-frame.mjs --stage-ios`
  — which renders the main app home route with boot-identity defaults
  (sourceURL = `AppSettings.defaultCustomURL`, nominal 1040x844 viewport;
  flag-on boots relayout across viewport drift, §9). Staged `.exactframe`
  binaries are gitignored: regenerate, never commit — an empty folder
  degrades to a normal boot. The four hand-maintained surfaces (staging
  folder, project.yml/pbxproj, Swift lookup, generator defaults — the
  sourceURL default is cross-pinned to `AppSettings.defaultCustomURL`) are
  held together by `scripts/check-precomputed-first-frame-packaging.mjs`
  (`precomputed-first-frame-packaging`, governance + contract profiles).
  There is still no `exact build` project-CLI command; when one exists, the
  staging step is the hook it calls.
- **Still open (native/device, ENG-22922 follow-ups):** the release-lane
  packaged-HBC paired cold-start matrix vs RN (item 2), the iOS
  simulator/device measured lane (item 3), font preregistration for
  custom-font routes — the measured route ships on system fonts (item 4),
  and per-route artifacts (OQ-A) plus the flag default-on decision once
  2–3 are green (item 6).

## 9. Phase C implementation notes (ENG-22894, shipped 2026-07-05)

macOS lane; iOS compiles and shares every seam but is unmeasured (Phase B
artifacts and the iOS measured lane stay tracked on ENG-22893 / the
ENG-22826 umbrella).

- **Boot-window replay under the flip.** The A1 ownership handoff is now an
  explicit object: `ExactTreeDomainBootWindow` (one-way close), open from
  tree-domain creation under `EXACT_RUNTIME_THREAD=1` until the first
  tree-touching executor schedule. `assertConfinement` admits main while
  the window is open. The shell replays at the first valid viewport via
  `engine.onBootWindowFirstViewport` — fired synchronously inside the
  viewport flush BEFORE the first executor-side tree write — parses,
  finalizes, and drains the staged commit in the same main turn: frame 1
  never crosses the thread flip (OQ11 outcome 1). Every tree-work scheduler
  (`reset`, constraint enqueue, viewport flush, dispatch drain) closes the
  window first; a missed window logs `skipped (boot window missed)` and
  degrades to a normal boot.
- **Host relayout across viewport drift.** The boot-window seam tolerates a
  viewport-only key mismatch (identity keys — sourceURL, buildFingerprint,
  appearance — remain hard stops): it replays at the artifact's embedded
  viewport, and the viewport commit the same flush schedules immediately
  re-lays-out the replayed roots at the true size on the runtime thread.
  This closes the §4 "no cross-viewport relayout" gap for the flag-on lane
  (observed live: first flush 1040x812 vs artifact 1040x844 — 32pt of
  chrome height arrives after the first flush — self-heals within the same
  boot). The inline (flag-off) seam keeps the exact-match key: it runs
  after the viewport settles and has no guaranteed follow-up relayout.
- **Adoption instead of discard-and-replace (both modes).** The hydration
  reset now carries `adoptionCandidate` (set when a precomputed preview is
  on screen); the presenter defers its wipe and the sync batch published
  WITH the reset adopts matching mirror nodes in place — view identity,
  scroll offsets, and text-editing sessions survive; a type-mismatched id
  is rebuilt (never merged across types) and reported; ids the live tree
  does not reclaim are swept, so a duplicated tree is impossible by
  construction. Flag-on the pair is staged atomically (`publishTogether`)
  and applies in one main-side drain turn — no intermediate wiped frame;
  flag-off the inline publisher already applies both in one turn.
  Fallbacks: an adoption reset with no paired sync (zero-ops parse, second
  reset, patch/geometry draining first) performs the deferred wipe — a
  preview never outlives its epoch (§4.8 fencing covered by test).
- **Flag-on capture.** The runtime-thread drain consumes an armed-once
  latch (`ExactPrecomputedFirstFrameState`, lock-boxed) and hops the
  batches to main for the same capture/key/write path the inline drain
  uses.
- **Measured (macOS Debug dev lane, 8 runs/mode, heavy machine load —
  relative numbers are the signal):** flag-on: content painted p50
  **453ms** (p90 513) vs first render p50 3145ms without the artifact —
  ~2.6s earlier, 0 failures, adoption `adopted=322 recreated=0` every run,
  zero B9 violations under fatal asserts. Flag-off unregressed: painted
  p50 685ms, first render 2730 vs 2796ms (noise). The flag-on boot-window
  paint lands EARLIER than the flag-off inline paint (the first-viewport
  flush precedes `onViewportReady`). Flag-on first-render (~3s) still
  carries the ENG-22896 cold-start stall, fixed separately.
- **Tests:** `ExactTreeDomainBootWindowTests`,
  `ExactPrecomputedBootWindowReplayTests` (boot-window replay on main with
  zero B9 violations; §4.8 fencing; atomic reset+sync drain),
  `ExactPresenterAdoptionTests` (adopt/mismatch/fallback/plain-reset),
  `ExactPrecomputedFirstFrameStateTests`.
- **Phase B composition (post-rebase):** the shared replay core resolves
  captured-then-bundled (Phase B order) in BOTH seams, so a first-install
  boot under `EXACT_RUNTIME_THREAD=1` paints the bundled artifact in the
  boot window; the viewport-drift tolerance applies to both origins
  (bundled identity = sourceURL + appearance; captured adds
  buildFingerprint).

## 10. Release-lane measurement evidence (ENG-23321, 2026-07-07)

First **packaged-HBC (Release)** numbers for the shipped pipeline — ad-hoc
signed Release builds booting the embedded `caltrain-bundle.hbc` (no dev
server), bundled artifacts generated by the shipping generator for the
caltrain route twin, `scripts/measure-precomputed-first-frame.mjs
--mode bundled` (Caches capture cleared before every run; painted origin
`bundled` in every counted run, 0 skips, 0 failures). Machine under heavy
parallel-agent load — cross-mode deltas measured back-to-back are the
signal, absolute values are inflated. iOS numbers ride a `simctl launch
--console-pty` adapter, so they carry constant spawn overhead in both
modes.

| Lane (Release, packaged HBC, caltrain 449-op first frame) | painted p50 | first render p50 |
| --- | --- | --- |
| macOS, main-thread (`EXACT_RUNTIME_THREAD=0`), flag-off | — | 403–629ms (load-dependent) |
| macOS, main-thread, bundled EXFF | 578–589ms | 647–700ms |
| macOS, runtime-thread (`=1`), flag-off | — | 334ms |
| macOS, runtime-thread, bundled EXFF | **224ms** | 339ms |
| iOS sim (iPhone 17), main-thread, flag-off | — | 582ms |
| iOS sim, main-thread, bundled EXFF | 542ms | 581ms |
| iOS sim, runtime-thread, flag-off | — | 589ms |
| iOS sim, runtime-thread, bundled EXFF | 506ms | 573ms |

Findings:

- **The Release lane is not eval-bound**, so the Debug-lane 2.8× win does
  not transfer: packaged HBC evaluates fast enough that flag-off first
  render lands in the 330–630ms band and EXFF can only shave the JS-eval
  slice.
- **The win concentrates in the runtime-thread boot window**: macOS
  flag-on paints at 224ms vs 334ms flag-off first render (~33% earlier,
  no live-render cost — 339 vs 334ms is noise). On the main-thread inline
  seam the replay waits behind app boot and paints ≈ when the live render
  would anyway (macOS: painted 578 vs 626ms loaded flag-off; it also
  delays live render ~60–75ms). iOS sim mirrors the structure with
  smaller margins (painted 506–542 vs 582–589ms, live render unregressed).
- **Live iOS seed→replay→adoption verified** (Release sim):
  `painted (bundled: 449 ops, 93 nodes, 8.4ms, inline, epoch 0)` then
  `hydrated via adoption (adopted=36, recreated=56, added=0, removed=1)`.
  The recreates come from route-twin vs standalone-bundle node-order
  divergence — per-route/per-bundle artifact pairing (OQ-A, ENG-23323)
  is what fixes adoption quality.
- **Stored-settings trap:** a runtime capture inherits `customURL` from
  the settings DB, so a seed boot on a dev machine can key artifacts to a
  stale URL; pin identity with `EXACT_APP_CUSTOM_URL` (macOS) /
  `SIMCTL_CHILD_EXACT_APP_CUSTOM_URL` (sim) when generating or measuring.
- **Historical limitation of this 2026-07-07 run:** no RN baseline app existed
  on that worktree, so the LLP 0145 `hello-shell` 1011/754ms baselines were
  not directly comparable to these caltrain numbers. The current-fixture RN
  column was restored and measured below. Physical-device iOS
  (ENG-22890-class gating) and a quiet-machine re-run remain open.

### 2026-07-09 current-fixture paired rerun

ENG-23321 restored the RN Release app and reran the corrected `hello-shell`
fixture with one equivalent title, subtitle, and pressable on the Exact React,
RN, and Contract-EXFF sources. Release artifacts were packaged HBC (Exact iOS
711,995B / macOS 712,019B, HBC 99; RN 1,690,539B, HBC 96). The EXFF protocol
payload is 44 ops / 1,002B (10 nodes); the live Exact React tree is 28 ops / 7
nodes.

All rows used 3 warmups + 30 measured pairs, explicit
`EXACT_RUNTIME_THREAD=1`, pinned root identity, agent boot off, capture removal
before every bundled launch, and alternating pair order. Every counted EXFF
sample reported origin `bundled`, hydration, zero failures, and zero actual
skips. Raw reports and contemporaneous load snapshots are under
`notes-archive/benchmarks/2026-07-09-eng-23321-current-startup-matrix/`.

| Platform / comparison | flag-off or RN content p50 | bundled Exact paint p50 | bundled Exact live/interactive p50 | paired result |
| --- | ---: | ---: | ---: | --- |
| macOS, flag-off vs bundled | 832ms live | **187ms** | 826ms live | paint 636ms earlier; live delta −5ms |
| iOS sim, flag-off vs bundled | 981ms live | **350ms** | 1009ms live | paint 631ms earlier; live delta +19ms |
| iOS sim, RN vs bundled Exact | RN content 415.9ms | **356.5ms** | Exact interactive 1036.6ms (RN 415.9ms) | Exact content −59.4ms; interactive +620.7ms |

The within-pair median Exact-minus-RN deltas are −58.8ms to first content and
+611.8ms to first interactive. That is the intended semantic split: bundled
EXFF wins this simulator run's observer-to-content row, while RN still wins
interactive readiness. App-relative origins remain non-identical and are not
the headline comparison.

The Contract preview → React live adoption result was stable at
`adopted=2, recreated=5, added=0, removed=3`; the visible fixture matches, but
the two authoring paths do not share node order/identity. This is adoption
quality residue, not fabricated first-paint success.

The host was not quiet: unrelated Rust/Deno work kept launch-start/end load
averages in roughly the 9–20 range and caused a visible iOS mid-run spike.
Alternating order bounds order bias but does not turn these into publishable
absolute claims. No physical-device number was produced.

## 11. Per-route artifacts + the default-on decision (ENG-23323, 2026-07-07)

**Per-route artifacts (OQ-A) shipped.** The artifact file name now carries
the route dimension: `precomputed-first-frame.route-<token>.<appearance>
.exactframe` for deep routes, the historical unsuffixed names for the root
route (full back-compat — existing artifacts, packaging, and the
measurement harness are untouched). The token derivation is byte-paired
between the JS codec and the Swift host (pure string parsing on both
sides — URL classes percent-encode differently across languages — with a
mirrored test-vector table in both suites). Applies to BOTH origins: the
bundled lookup tries the boot route's file first and falls back to the
root file (whose key check then decides), and the runtime capture saves
to a per-route store file, so alternating boot routes no longer thrash a
single capture. Verified live on a Release build: first-install deep-route
boot painted `(bundled: 449 ops)` from the route-suffixed file, captured
to `precomputed-first-frame.v1.route-caltrain-contract.exactframe`, and
the second boot painted `(captured: 338 ops)` with perfect adoption
(adopted=92, recreated=0).

**Default-on decision: `EXACT_PRECOMPUTED_FIRST_FRAME` stays opt-in.**
The §10 release-lane evidence shows the flag is a clear win only inside
the `EXACT_RUNTIME_THREAD=1` boot window (macOS painted 224ms vs 334ms
flag-off, no live-render cost); on the main-thread inline seam it paints
no earlier than the eval path and delays live first render ~60–75ms.
Decision: **re-gate EXFF default-on on the LLP 0297 runtime-thread
default-on decision** — when the runtime thread flips default, flip this
flag with it (they compose; neither regresses the other). Flipping EXFF
alone on the main-thread lane would be a net loss in Release builds.
The paired RN simulator column is now present in §10. Remaining prerequisites
tracked on ENG-23321 are the physical-device lane and quiet-machine absolutes.

**Executed 2026-07-07 (LLP 0322, ENG-22779 close-out):** the runtime
thread flipped default-on for macOS, and `EXACT_PRECOMPUTED_FIRST_FRAME`
now follows the *effective runtime-thread mode*
(`ExactPrecomputedFirstFrame.resolveEnabled`): unset ⇒ on wherever the
dedicated thread runs (the macOS default), off on the main-thread seam
(iOS/Windows defaults, and under the `EXACT_RUNTIME_THREAD=0` kill
switch); explicit `1`/`0` still win. The coupling is to the mode, not the
platform, exactly so the "EXFF alone on the main-thread lane" loss above
can never happen by default.

## 12. Route-specific list restoration seeds (ENG-24897)

An EXFF header may now carry `listSeeds`: list identity, measurement
snapshot, last logical anchor, and scroll offset. The field is optional, so
existing artifacts remain byte-compatible. The Contract build-time generator
captures the mounted window's measurements and anchor from the same runtime
that emitted the protocol buffers; the Swift codec preserves the paired state
for pre-JS route adoption.

Implementation sync (2026-07-14): the native shell stages `listSeeds` only
after the artifact has passed eligibility checks and replayed successfully.
Immediately before the next source or HBC bundle evaluation it publishes the
seed array on the runtime executor; FIFO evaluation puts that one-shot handoff
ahead of app code without a cross-thread synchronous wait. The Contract
runtime validates the whole array, deletes the handoff global, and consumes
each identity once. A matching `virtualList` initializes its measurement
snapshot, anchor/offset, first logical window, and physical scroll command
before the live tree mounts. Invalid or duplicate entries reject the entire
handoff rather than risk adopting pixels with contradictory list state.

Acceptance requires the replayed route to seed list measurement/window state
before its live first render. A restored deep route must therefore show the
same correctly sized logical rows in the pre-JS frame and after adoption,
without a top-of-list flash or a second, contradictory first window. RFC 0045
owns navigation policy and persistence; this RFC owns pairing that state with
the route-specific first-frame artifact. Swift codec/bootstrap coverage and a
Contract runtime regression now prove the eligible handoff and first-window
selection deterministically. They do **not** prove physical-device paint
continuity; the device no-flash/adoption row remains open.

### 12.1 `listSeeds` re-specified under RFC 0540 (2026-08-26; 0504 §3 row 61)

RFC 0540 §8.8 re-specifies this section, and LLP 0543 §12.7 fixes the
seed's shape. Recorded here as this document's owner fold. Decided
content only; **delivery is post-window** (RFC 0540 §8.6 classifies
the lists-v2 surface as D2) and lands at 0540's **L4** rung.

1. **`listSeeds` becomes the §2.5 restoration record.** It stops being
   a list-shaped bag of measurement state and becomes the persisted
   form of **one semantic anchor intent** — the tagged sum
   `item {collectionStableId, itemKey, logicalEdge, viewportOffset}` |
   `boundary {collectionStableId, edge, absoluteOffset}` | `none`,
   never a raw address, because raw refs retire across producer
   generations while logical keys rebind. Concretely (0543 §12.7) the
   record this document keeps is
   `{intent, retained: [{key: KeyV1, summary}], headIndex, tailIndex}`,
   and extents carry **stable per-collection ids** so a seed survives
   a collection's re-creation.
2. **Who consumes it changes.** The seed is consumed at replay by the
   **presenter and the layout engine** — *not* by the Contract runtime
   after bundle evaluation. That inverts the 2026-07-14 implementation
   sync above: the one-shot global handoff staged ahead of app code,
   with the Contract runtime validating the array and a matching
   `virtualList` initializing itself, is the JS-windowed mechanism and
   is replaced. What survives is the *acceptance property*, restated
   below, not the handoff mechanism. Restoration is one semantic intent
   with **carrier-owned projections**, and this document owns exactly
   one of them — the EXFF pairing. The others are the Router's
   (0494 R-E, row 63), Aquifer cell replay (0498 §4.7), and 0088's
   one-shot `initialScrollKey`/`initialMeasurements` inputs.
3. **Artifact regeneration at L4.** EXFF artifacts containing
   JS-windowed slots are **regenerated** at 0540's L4 rung, so the
   pre-JS pixels a replayed deep route shows are **extents plus
   retained boxes** — the collection's own vocabulary — rather than a
   captured JS window. Until that regeneration, the shipped artifacts
   and the mechanism in §12 above stay in force; this is a re-spec
   with a named landing rung, not an immediate deprecation.
4. **The acceptance bar is unchanged, and gains a fixture.** §12's
   acceptance — no top-of-list flash, and the same rows pre-JS and
   after adoption — stands exactly as written and now **joins the
   chat fixture** (`list-chat-fixture`, 0543 §16.3), which runs on
   physical Apple Silicon with a real-input leg. That is the harness
   the "device no-flash/adoption row remains open" note above has been
   waiting for; the row closes on that fixture's arm, not on this
   recording. LLP 0543 §12.7 also keeps this document's EXFF
   measurement pairing intact — 0540 does not move the pairing, only
   the seed's contents and its consumer.

## 13. Correct-enough by declaration (LLP 0561 §4.3; 2026-08-25)

LLP 0561 §4.3's honesty rule for the plan-first frame: what may differ
between the replayed (baked) frame and the post-attach frame is
**enumerated here; anything else differing is a defect with a receipt**
(the adoption diagnostics — `adopted/recreated/added/removed/mismatches`
— and the §15 receipt join are the receipts). This is the list the
LLP 0561 L-P/L-F checks cite.

**Declared allowed differences:**

1. **Data staleness until attach.** The replayed frame shows the
   artifact's data, not this boot's:
   - *Captured origin:* at most one bundle-generation stale (§4 —
     bounded by overwrite-on-every-capture and the `buildFingerprint`
     key). Dev-server edits between boots inside an unchanged key can
     show the previous boot's UI for the fetch+eval window (§6, accepted
     for the dev lane).
   - *Bundled origin:* the route's build-time evaluation — `resource`
     fetches, clocks (`now()`), and any dynamic data render their
     generation-time values until the live tree attaches.
2. **No interactivity during the preview window.** Handler ids in the
   replayed tree have no live JS behind them; presses and gestures in
   the fetch+eval window are dropped (§4, the Lynx-IFR constraint
   class). A behavior difference, not a pixel difference.
3. **Adoption-recreation state residue.** View identity, scroll
   offsets, and text-editing sessions survive only for *adopted* nodes
   (§9); a recreated node (type mismatch, or node-order divergence
   across authoring paths, §10) loses that per-view state. The
   `recreatedNodes` count is the receipt; the pixels must still match.
4. **Pre-seed list windows (legacy artifacts only).** An artifact
   without §12 `listSeeds` may replay a deep route at its captured
   top-of-list window while the live tree restores scroll state at
   attach. With seeds present this difference is **not** allowed — a
   contradictory first window rejects the handoff and is a defect
   (§12).

**Explicitly not allowed** (each is a defect or a skip, never a silent
difference): wrong-appearance pixels (appearance is a hard key stop);
wrong-size layout (viewport drift self-heals via host relayout in the
boot window, and the inline seam keys on exact match — a painted frame
at the wrong geometry is a defect); fallback-metric text (text is never
baked — the kernel lays out at replay with live platform measurement,
and routes with custom fonts either preregister the bundled files under
the same family ids or skip the replay entirely, ENG-23322); and any
structural/content divergence beyond items 1–4 (adoption reports it in
`mismatches`).

## 14. Plan-runner generation join (design note; LLP 0561 residue item 6)

Today's artifact is captured/generated **protocol buffers** plus a
ComputeLayout command, replayed through the ordinary dispatch path.
Post-M2, when the Rust plan runner (LLP 0485's flat plan; the LLP 0413
orbit) can evaluate a baked entry plan directly, Phase B generation can
move onto it: the generator would emit (or the host would carry) the
entry plan itself and let the kernel produce the same first frame
without the headless-JS mount step. The outcome is equivalent today —
same tree, same replay-time layout, same adoption path — so **nothing is
built now**; this note records the join so the plan-runner milestone
picks it up deliberately. The EXFF container's buffers would then be a
derived cache of the plan rather than the primary artifact, and the §13
enumeration carries over unchanged (the plan is exactly as stale as the
buffers it replaces).

## 15. Boot receipt: the RFC 0495 join (LLP 0561 residue item 3; 2026-08-25)

The paint and adoption diagnostics stop being NSLog-only. One structured
record per boot that painted — `ExactPrecomputedFirstFrameBootReceipt`
(painted origin, mode, ops/node counts, replay cost, host wall-clock
epoch timestamps for paint and the adoption apply, hydration outcome
`adopted`/`wiped`/`pending` with the §9 adoption counts, and the
paint→hydration delta). Epoch timestamps, deliberately not
session-relative: the boot-window paint precedes the load-timing
session's origin, so a session-relative field would be a lie there, and
a spawn-epoch instrument joins wall clocks directly. Surfaced two ways:

- **Host diagnostics snapshot:** `precomputedFirstFrame` on the
  `/__exact/host/status` snapshot (assembled live; `pending` while the
  preview is still on screen). Absence = no precomputed frame painted
  this boot (skips remain HostLog lines).
- **Causal receipt stream:** once hydration settles, the shell publishes
  `globalThis.__exactPrecomputedFirstFrameReceipt` into the app runtime
  (executor-marshaled, fire-and-forget — the §12 list-seed seam), and the
  Acto receipt DAG mints it as a presentation-domain receipt
  (`kind: "precomputed-first-frame"`, outcome = hydration; source
  `host-precomputed-first-frame-boot-receipt` — the continuous native
  present-timing producer remains unavailable). Boot-scoped: at most one
  per process; queryable via `query-receipts`/`exact_causal_trace`.

The external spawn-epoch instrument
(`scripts/measure-precomputed-first-frame.mjs`) remains the measurement
meter for LLP 0561 G1 corpus sessions; the receipt is the in-app,
agent-assertable record (its `paintedAtEpochMs` joins against the
spawner's epoch for launch-relative numbers). Reporting can never fail a
successful startup: encode or publish failures degrade to the
diagnostics-snapshot surface alone. The dev-server relay re-ingests the
snapshot through a closed, bounded field list; the receipt's parser
(`parsePrecomputedFirstFrameReceipt`, vite-plugin) enum-checks and caps
it like every other host-status block.
