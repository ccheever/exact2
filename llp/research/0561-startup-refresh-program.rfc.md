# LLP 0561: The Startup Refresh Program — cold start becomes a gated product surface on real hardware

**Type:** RFC
**Status:** Accepted (2026-08-24, by Charlie Cheever — all five §7 asks decided in one message; the decisions are recorded verbatim on the asks and folded into §4/§5. Mechanical status edit by agent under the author's explicit decision.)
**Systems:** Startup, Runtime, Ibex, Hermes, Kernel, Contract, Apple Hosts, Android Host, Windows Host, Dev Server, Release, Verification, Tooling
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-24
**Revised:** 2026-08-25 (r12 — §9 register rows from the LLP 0413.001 composition-cutover lane: the P2 agent-session row CLOSED (agent-on sessions ADMIT `["app","agent"]` through the landed ibex LLP 0056 driver; `startup-lane-fail-loud` v3 arm green agent-on AND agent-off on Durand; agent-on warm medians prepared 956 ms evalMs / 1227 ms report-ready vs legacy 1568 / 1839 at the final tree — the dev loop takes the prepared win), the 0413.001 row moved to IMPLEMENTED (live cutover, dark flag retired, fail-loud minimum 3, O-4 discharged, ibex step-6 authorization fixed 9.2 s → 0.32 s @8f8a1d039), and the main-app labs-home observation joined the prepared-lane `.contract` mount ticket. Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-25 (r11 — §9 register row updates from the G1 measurement lane: the G1 Class A protocol executed on the Apartment MacBook Air (first corpus session landed, `docs/reports/startup/g1/20260825-air-m5-class-a-blog-legacy/`; corpus Class A leg valid, Class B still pending), and the run's three new finding tickets joined as rows — `issues/20260825-blog-native-mount-prepared-lane-red.md` (P1, prepared-lane blog never paints, blocks class-a binding), `issues/20260825-macos-release-launch-eenv-viewport-refusal.md` (P1, packaged macOS Release launch refused before engine construction, blocks the §4.3 EXFF receipts), `issues/20260825-exff-web-font-requirement-skip.md` (P3); the compat-loads row gained the parseFree/G1 collision note. Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-25 (r10 — §9 register row update from the L-P/EXFF residue lane: the row CLOSED — all six residue items dispositioned (dev-lane EXFF evidence confirmed on Durand with the flag unset — painted 222 ms p50 vs 1008 ms flag-off first render, adoption clean every run, and the pre-flip ~4.4 s dev attach pain confirmed gone on the flipped lane; 0307 gained §13 correct-enough enumeration, §14 plan-runner join note, §15 boot receipt / RFC 0495 join with the relay's bounded-ingress parser; `startup-budget-exff-paint` registered red-pending with the per-run `exff` corpus receipt block). Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r9 — §9 register row update from the eager-control-metric lane: the row CLOSED — persisted tuple-keyed control-metric boot cache landed on main (toggle 7.04→0.19 ms, searchfield 9.11→0.03 ms medians, Durand same-binary A/B n=15/arm, 0 repairs; `docs/native-module-boot-policy.md` carries the design; the ticket's `.lazy`-flip expectation was evaluated and ruled out — the modules stay audited-eager with cache-read-cheap inits). Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r8 — §9 register row updates from the Acto thin-boot lane: the author-directed thin-boot row CLOSED (boot shim landed; boot-eager agent share 266 modules / 2.87MB → 32 modules / 254KB, measurement filling LLP 0413.001 §7's agent-package placeholder), two residue rows added — `issues/20260824-agent-logs-shim-endpoint.md` (the Apple dev shell's 750ms error poll arms the full graph shortly after bind) and `issues/20260824-caltrain-contract-memo-reachability-red.md` (pre-existing reachability red found while gating). Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r7 — §9 register row updates from the G3 re-run lane: the default flip LANDED (dev-served lanes default to `prepared-graph`; Durand macOS receipts in `issues/closed/20260824-prepared-graph-default-flip.md` — blog warm evalMs 3508→458, main app admits agent-off, the agent-on dev-loop fallback measured free), the flip ticket's row closed, the agent-session row re-labeled as the dev-loop residue, and the new `issues/20260824-prepared-lane-boot-compat-loads.md` observability row added. Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r6 — §9 register rows from the lazy-native-module-init landing: the lazy row CLOSED (implementation on main, matching the planned kind-split shape), two new rows added — `issues/20260824-eager-control-metric-boot-cost.md` (measured by the landing's receipts as the dominant remaining module boot cost) and `issues/20260824-lazy-module-state-mirror-freshness.md` (residue). Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r5 — §9 register row update: the dynamic-import-edge disagreement blocker CLOSED by the producer-side served-subset fix (`issues/closed/20260824-prepared-graph-dynamic-import-edge-disagreement.md`); the default-flip row now names the remaining gate precisely (agent-session row gates the dev-loop cell; the agent-off macOS evidence re-run is unblocked). Mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r4 — §9 register row updates from the G3 execution lane: G2 landed (`startup-lane-fail-loud` registered + green on the legacy default), the G3 flip attempted and REVERTED with macOS receipts, and the two admission-blocker tickets added as rows — `issues/20260824-prepared-graph-dynamic-import-edge-disagreement.md`, `issues/20260824-prepared-graph-agent-session-unavailable.md`; mechanical ledger maintenance per §9's own rule, no charter change.) 2026-08-24 (r3 — the commissioned work register added as §9 per the author's direction that all startup work be visibly part of the Refresh ("everyone knows we have to do all the stuff that makes this fast"): every ticket/gate in one table, `issues/20260824-lazy-native-module-init.md` and `issues/20260824-llp0561-budget-gates-g1-g6.md` joined to the program, AGENTS.md §6's performance rule now points here, and the exact-9e orchestration session notified.) 2026-08-24 (r2 — acceptance-effects edit under the author's same-day decisions: Status → Accepted; §7 asks annotated TAKEN with the author's words; **the program is rescoped Apple-first** (ask 3: "we just have to be fast on macOS and iOS for today… when we focus on Android we can worry about Android") — §4.1 Class C and §4.4's gate timing marked DEFERRED-to-Android-focus, §5 G5 likewise; the G3 default flip is decided "immediately or as soon as possible" and commissioned as `issues/20260824-prepared-graph-default-flip.md`; ask 5 executed — the quarantined `startup-budget` graph ratchet REMOVED from `exact-verify.json` (check object + `governance`/`contract` profile refs; registry-consistency verified green), revival owned by `issues/20260824-startup-budget-ratchet-revival.md`; the 0504 ledger row added.) 2026-08-24 (r1 — initial draft, commissioned by the author's 2026-08-24 direction that this joins the Refresh family; the author's product judgment in §2 is quoted from that direction)
**Related:** LLP 0413 (parse-free startup, Accepted — **consumed whole**: the module-pipeline authority this program gates but never re-decides; its §2 measurements, §10 Phases 0–6 (Phase 5 graph diet, Phase 6 legacy retirement), §11 budget discipline and its reference-Mac lane, the startup report / `__exactStartupReport`), `docs/startup-report-host-surfacing.md` (the report surfacing contract; today's lane is `legacy-source-table`, `parseFree: false`), LLP 0307 (precomputed first frame — §11's `EXACT_PRECOMPUTED_FIRST_FRAME` follows the effective threading mode; the machinery L-P extends), LLP 0485 (flat plan — the app's initial tree as data; what makes a first frame without JS possible), LLP 0287 (the lane review that decided the Android native-View host approach), LLP 0553 (dev-loop program — the sibling program; its §6 owns the *reload* budget rows (leaf-edit, warm full reload); this program owns *cold product start*; the two share 0413's measurement discipline and must not double-own a row), RFC 0496 (verification refresh — `purpose: budget` / enforcement semantics the L-G gates register under), LLP 0128 (startup-graph ratchet — the existing `startup-budget` check, quarantined red at authoring time), LLP 0145 (startup artifact measurement — bytes/modules, manual), RFC 0494 (router manifest — the route-level laziness join in L-D), Draft LLP 0351 / RFC 0537 (server-generation choices; progressive delivery — the baked-plan staleness interplay in L-P), LLP 0559 (Flutter lessons — F5's dev-loop framing; §1's provenance discipline adopted for this document's external claims), LLP 0504 (the Refresh map — on acceptance this program takes a ledger row; a mechanical map refresh, not a charter change)

## Summary

**The author's product judgment (2026-08-24): startup speed is one of the
biggest problems with Exact today — it makes the product largely unusable,
and not just on Android futures: on iOS and macOS now.** The repository's
own measurements agree: the default startup lane is still
`legacy-source-table` with `parseFree: false`, and LLP 0413 §2 measured
~4.7 s warm to ~8.6 s cold on a *reference Apple Silicon Mac* for a simple
blog — against a standing product direction of < 1000 ms. A July incident
showed the failure mode is silent by design: when HBC is unavailable the
host soft-falls-back to a ~10 s source parse and nothing goes red.

LLP 0413 is the module-pipeline fix and this program does not re-decide a
word of it. What has no owner is startup as a **product surface**: no
registered check binds wall-clock startup time on any platform or hardware
class (the existing `startup-budget` check is the LLP 0128 *graph* ratchet
— artifact shape, not time — and is quarantined red at authoring time; the
lighthouse launch legs carry a first-render *watchdog*, not a budget); no
budget exists for any device other than the reference Mac; the
first-frame-without-JS opportunity the flat plan creates is machinery
(LLP 0307) without a program; and the Android host is being brought up
with no startup discipline attached, which is exactly how React Native and
Xamarin each inherited a decade of Android cold-start pain.

This RFC is the program, in the Refresh family's shape: **lanes sharing
one instrument** (the 0413 startup report as the receipt stream; budgets
as registered gates on named hardware classes), consuming 0413/0307/0485
whole. Five lanes: **L-G** budget gates on real hardware; **L-F**
fail-loud lane enforcement (silent fallback becomes a red); **L-P** the
plan-first frame (first correct frame from the baked plan before JS
attaches — the card RN and Xamarin never held); **L-A** Android boot
mechanics as normative bring-up requirements; **L-D** the eager-graph
budget joins onto 0413 Phase 5. Android is the forcing deadline; iOS and
macOS are the present tense.

## 1. What exists today (receipts, not aspiration)

- **The default lane is the slow one.** `docs/startup-report-host-surfacing.md`:
  today's native startup runs `lane: "legacy-source-table"`,
  `parseFree: false`. 0413's prepared-graph lane is Phase 2, behind
  `EXACT_STARTUP_LANE=prepared-graph`.
- **Measured cost on fast hardware** (0413 §2.2, cited not restated):
  ~4.2–4.7 s of evaluation-through-first-dispatch with warm artifacts;
  8.6 s cold; 545 eager modules / 8.5 MB artifact for a blog, ~7.5 MB of
  it raw source compiled after process start — the exact thing 0413's
  invariant forbids.
- **The silent fallback** (2026-07-25 incident): `startup.hbc.status:
  "unavailable"` is a soft fallback to a ~10 s source parse. Nothing
  fails loudly; the cost appears only in the load waterfall.
- **What is instrumented:** `__exactStartupReport` (lane, parseFree,
  eager counts, transform counters) and `__exactLoadTimings` exist and
  were designed for surfacing. The graph ratchet (`startup-budget`,
  LLP 0128) and artifact measurement (`startup-artifact-measurement`,
  LLP 0145, manual) measure artifact *shape*. The lighthouse launch legs
  (iOS proven release-bytecode path; Android scripts landed) assert
  functional evidence under a first-render watchdog.
- **What is not instrumented anywhere:** wall-clock process-launch →
  first-correct-frame, on any platform, as a registered gate.
- **Android state:** early host (LLP 0287 direction), Kotlin codegen
  bridges, emulator PoC with a11y delegate, lighthouse-release scripts
  and an HBC eval smoke check — and no startup requirement of any kind
  attached to bring-up.

## 2. Why this is a Refresh program

The author's direction commissioning this document: startup should be
part of the Refresh, because it is one of the biggest problems with
Exact now and makes it largely unusable even on iOS and macOS.

The Refresh pattern (0504 §1, quoted via 0552 §1) applies without
strain: startup decisions become **data** (prepared graphs, per-principal
HBC carriers, baked entry plans), evaluated by owned engines, with
**receipts out the side** (the startup report and load-timing chain are
the receipt stream this program gates on), and **an instrument instead of
hope** (budgets as registered checks on named hardware, not prose in
AGENTS.md). The sibling precedent is LLP 0553: several existing systems,
one decided direction, no program — until the program document named the
lanes and the gates. 0553 owns the reload rows; this program owns cold
product start; both ride 0413's measurement discipline.

**External evidence, provenance-labeled per LLP 0559 §1** (authoring-model
knowledge, not re-verified): React Native and Xamarin both shipped
Android hosts first and attached startup discipline later, and both spent
years buying it back (Mono AOT programs; RN's Hermes adoption, merged
`.so`, and mmap'd bytecode were all retrofits). Their first frame
*required* the managed runtime; Exact's does not have to — the flat plan
makes the initial tree data the kernel can render before Hermes finishes
initializing. That is the one structural startup advantage no sandwich
predecessor held, and it is currently machinery without a program.

## 3. Program shape: consumed whole vs. owned here

**Consumed whole (re-decided nowhere):**

| Authority | What this program takes |
| --- | --- |
| LLP 0413 | The entire module pipeline: lanes, HBC invariant, per-principal carriers, Phase 5 graph diet, Phase 6 legacy retirement, §11 budget discipline and its reference-Mac rows, the startup report |
| LLP 0307 | Precomputed-first-frame machinery and its threading-mode coupling |
| LLP 0485 | The plan as the app's initial tree |
| LLP 0287 | The Android native-View host direction |
| LLP 0553 | Ownership of reload budgets (this program never gates reload) |

**Owned here:** hardware-class budget tables and their registered gates
(L-G); fail-loud lane enforcement (L-F); the plan-first-frame lane (L-P);
Android boot mechanics as bring-up requirements (L-A); the eager-graph
budget row and route-laziness posture joined onto 0413 Phase 5 / 0494
(L-D); and the sequencing that binds gates to milestones.

## 4. Lanes

### 4.1 L-G — Budgets become gates on named hardware

Three hardware classes, one measurement protocol (0413 Phase 0's,
extended to devices):

- **Class A — reference Apple Silicon Mac.** 0413 §11's existing lane
  and rows, adopted unchanged (≤ 750 ms median / ≤ 1000 ms p95
  process-launch → correct first visible frame; 500 ms stretch).
- **Class B — reference iPhone** (recent-generation, release-bytecode
  lighthouse configuration). Provisional: ≤ 1000 ms median / ≤ 1500 ms
  p95.
- **Class C — mid-tier physical Android** (the *binding* class for
  Android claims; a mid-range device of roughly the current Android
  install-base median, not a flagship). Provisional: TTID ≤ 1500 ms
  median / ≤ 2500 ms p95; TTFD (JS attached, first interaction
  serviceable) ≤ 2500 ms median / ≤ 4000 ms p95.
  **DEFERRED (ask 3, author-decided 2026-08-24): the program is
  Apple-first — Classes A and B bind now; Class C's device naming and
  gate activate when Android becomes the focus.** L-A §4.4 remains
  standing design guidance for the Android host in the meantime.

Rules, adopted verbatim from 0413 §11's discipline: budgets are
intentionally aggressive; tightening is routine; raising one requires the
raw corpus and a written explanation. Cold is defined by the protocol
(process not resident; page cache state stated; device thermals sane) and
every number reports median/p95 from N ≥ 5 runs. The gates register under
RFC 0496 semantics (`purpose: budget`) with honest `hosts` fields;
Class B/C legs are hardware-gated like the lists-v2 ProMotion evidence
was, not emulator-approximated. Until G1 lands device measurements, the
Class B/C rows above are **proposals for the owner to tighten or
replace**, not claims.

### 4.2 L-F — Fail-loud: the silent fallback becomes a red

A registered check (per platform as legs land) asserting from the startup
report: `parseFree: true`, `runtimeSourceTransformCount: 0`,
`runtimeDynamicFunctionCompileCount: 0`, and the expected lane — so
`legacy-source-table` running where `prepared-graph` is claimed, or
`hbc.status: "unavailable"` in a gated context, is a failure, never a
10-second shrug. This is the cheapest lane and lands first; the July
incident is its justification. (The soft fallback itself is correct
*product* behavior — users should not crash because a cache missed; the
gate makes the fallback loud in verification, not fatal in the field.)

### 4.3 L-P — The plan-first frame (first frame without JS)

The flat plan makes the initial tree data. The lane: the app bundle
carries a baked entry plan; on launch the kernel evaluates it and computes
layout, the presenter renders it, and Hermes initialization + module
evaluation proceed concurrently, attaching afterward (0413's pipeline
unchanged — it just stops being the thing the first frame waits for).
LLP 0307's precomputed-first-frame machinery is the seed; this lane makes
it a program with per-platform joins rather than a flag.

**Scoping-spike correction (2026-08-24, evidence in
`issues/20260824-lp-exff-residue.md`): 0307 is Implemented through
Phase C and this lane is largely SHIPPED on macOS.** What exists:
runtime-captured artifacts (any framework, returning boots) AND
build-time generation from `.contract` routes with iOS staging behind a
registered packaging check (first installs); per-route/per-appearance
artifacts; custom-font requirements; list scroll-restoration seeds with
a one-shot pre-eval handoff; Phase C true adoption with
adopted/recreated diagnostics; determinism pinned by the registered
`precomputed-first-frame-metrics` check; **EXFF default-on on macOS
today** (the flag composes with the runtime-thread mode per 0307 §11 +
LLP 0322; measured 224 ms paint vs 334 ms without, release
runtime-thread lane). And the §4.3 text-measurement wrinkle is solved
*architecturally*: the artifact carries the tree + a ComputeLayout
command — the kernel lays out at replay with live platform text
measurement; geometry is never baked. This lane's remaining work is the
residue ticket's list (iOS default rides the ENG-23520 runtime-thread
gate; enumeration note; 0495 receipt join; dev-lane evidence;
physical-device rows on ENG-23321/22922), not a build.

Owned questions, not silently assumed: **text measurement.** A baked
frame's layout depends on font metrics that vary by device and user
settings (most sharply on Android: OEM fonts, font scale). The lane must
choose per platform between measure-and-correct on first real layout
(render the baked frame, correct when metrics arrive — never a blank
wait) and a device-local measurement cache seeded at install/first run.
**Staleness:** a baked plan can be older than the newest OTA-delivered
plan (RFC 0537's territory); the lane adopts the delivery owner's
version rules and never invents its own. **Honesty rule:** the plan-first
frame must be *correct-enough by declaration* — what may differ from the
post-attach frame (text metrics pending correction) is enumerated, and
anything else differing is a defect with a receipt.

Provisional budget for the lane's own leg: baked-plan first frame
≤ 400 ms median on Classes A/B, ≤ 800 ms on Class C — same
proposal-until-measured status as §4.1.

### 4.4 L-A — Android boot mechanics (normative at bring-up)

The checklist that is cheap now and a retrofit later, stated as
requirements for the Android host before it leaves incubator
(provenance for the external items: LLP 0559 §1 discipline — these are
the standard, widely documented Android startup practices RN/Xamarin
adopted late):

1. **One merged `.so`** for kernel + ibex-runtime + Hermes; per-library
   `dlopen`/relocation cost is why RN merged theirs.
2. **`.hbc` stored uncompressed and page-aligned in the APK** so it
   mmaps directly — no copy, no decompress.
3. **A Baseline Profile ships with the host** covering the presenter/
   boot path (routinely a double-digit-percent startup win on ART).
4. **No `ContentProvider` auto-init**; explicit initialization only.
5. **`RegisterNatives` at library load**; no lazy JNI lookup on the
   boot path.
6. **JNI batch discipline:** one crossing applies one protocol batch;
   per-node JNI calls on the boot path are a defect. The binary
   protocol already has the right shape — the Kotlin side must keep it.
7. **Text-measurement caching across the JNI seam**, keyed by
   text/font/constraint class, shared with L-P's correction strategy.
8. **The Class C gate (L-G) is an incubator-exit conjunct:** Android
   does not graduate while its cold start is unmeasured or over budget
   on physical mid-tier hardware. *(Timing per the ask-3 decision: this
   conjunct arms when Android becomes the focus; until then it is
   standing design guidance, not an active gate.)*

### 4.5 L-D — The graph diet gets a budget row

0413 Phase 5 owns the mechanism (lazy boundaries, graph diet); this lane
adds what the program needs from it: an **eager-module-count budget per
route** as a registered row (the report already carries
`eagerModuleCount`), and the posture that route-level laziness is the
default — joined with the 0494 router manifest (route-split loading) and
the 0351 ladder (choose the least runtime-capable strategy). 545 eager
modules for a blog is the disease; parse-free makes each module cheaper
and this lane makes most of them not load at all.

## 5. Sequencing

- **G1 — Measure.** Extend 0413 Phase 0's protocol to Class B/C
  hardware; land the raw corpus as receipts. No budget binds before its
  class is measured. *Exit:* corpus committed, provisional rows
  confirmed or replaced.
- **G2 — Fail-loud.** L-F checks registered and green on the lanes that
  claim prepared-graph; red where the claim is false. Cheapest, first.
- **G3 — The default flip** (0413-owned; this program asks, never
  re-decides): Phase 6 retirement of `legacy-source-table` as the
  default lane, per platform, gated by G2. §7 ask 2 puts the timing to
  0413's owner explicitly.
- **G4 — Plan-first frame default on Apple** (L-P), behind G1's Class
  A/B numbers; macOS first (0307's proven ground), iOS with the
  release-lighthouse leg. *(Spike correction 2026-08-24: G4-macOS is
  already satisfied — EXFF is default-on with the runtime thread. G4's
  real remaining content is the iOS runtime-thread gate (ENG-23520,
  0297-owned) plus the residue ticket's governance joins.)*
- **G5 — Android bring-up under L-A**, with the Class C gate as the
  incubator-exit conjunct. *(Deferred with ask 3: G5 activates at
  Android focus; G1–G4 and G6 are the live Apple-first sequence.)*
- **G6 — Budgets bind in release legs:** the lighthouse launch checks
  gain their L-G budget assertions (watchdog → budget), completing the
  move from "boots eventually" to "boots on budget."

## 6. Honest tensions

- **Dev-lane vs product-lane:** this program gates release-shaped
  startup. Dev-server startup and reload belong to LLP 0553; the
  boundary is the lane, and no row is owned twice.
- **Hardware gates cost fleet discipline:** Class B/C legs are manual or
  fleet-routed like the other hardware-evidence checks; the program
  accepts fewer, more meaningful runs over CI-cheap emulator numbers
  that would legitimize the wrong thing.
- **The plan-first frame is honest only with the enumeration rule:**
  without §4.3's correct-enough-by-declaration discipline it becomes a
  splash screen wearing the app's clothes. The receipts make the
  difference checkable.
- **Quarantine debt:** the existing `startup-budget` graph ratchet is
  quarantined red (expiry 2026-09-06). This program's L-G does not
  supersede it — artifact-shape ratchets and wall-clock gates are
  complementary — but the quarantine must resolve, not roll.

## 7. Asks (all five decided by the author, 2026-08-24, in one message)

1. **Adopt the program as a Refresh member:** on acceptance, the 0504
   ledger takes a row (mechanical map refresh) and the lanes above become
   commissioned work.
   **TAKEN — "yes."** Status flipped; the 0504 row added.
2. **To 0413's owner:** name the Phase 6 default-flip timing per
   platform, gated on G2 — or state what else it waits for.
   **TAKEN — "immediately or as soon as possible."** Commissioned as
   `issues/20260824-prepared-graph-default-flip.md` (P1: the one-constant
   flip at `startup-lane-lowering.ts` plus the G2 fail-loud check and
   macOS/iOS report-asserted evidence legs, Apple-scoped per ask 3).
3. **Decide the Class C reference device** (a named mid-range model and
   OS floor), so "mid-tier Android" stops being adjectival.
   **TAKEN as a rescope — "we just have to be fast on macOS and iOS for
   today. when we focus on android we can worry about android."** The
   program is Apple-first: Classes A/B bind now; Class C naming, the
   §4.4 item-8 gate, and G5 are deferred to Android focus (§4.1/§4.4/§5
   annotated).
4. **Confirm the L-P Apple ordering** (macOS then iOS) or reorder.
   **TAKEN — "yes macos first is fine."**
5. **Resolve or retype the quarantined `startup-budget` ratchet** before
   its 2026-09-06 expiry, so the program does not build gates beside a
   standing red.
   **TAKEN — "throw out the alarm and just do the things to fix it. when
   i have time we can bring it back and try to fix it."** Executed: the
   check object and its `governance`/`contract` profile references
   removed from `exact-verify.json` (registry consistency verified
   green; the removal commit preserves the forensic config); the
   quarantine ticket annotated; revival owned by
   `issues/20260824-startup-budget-ratchet-revival.md`. The "things to
   fix it" are this program's L-D lane on 0413 Phase 5.

## 8. Open questions

- **OQ1:** L-P's Android text-measurement strategy — measure-and-correct
  vs. device-local cache vs. both (§4.3); needs a spike with receipts.
- **OQ2:** Windows' place in the class table — Class B-equivalent desktop
  row now, or after the Windows host's next milestone?
- **OQ3:** Whether a Class D (low-end Android) advisory row is worth
  tracking pre-1.0, or noise until Class C is green.
- **OQ4:** Whether the agent-boot path (`EXACT_AGENT_BOOT=1`) gets its
  own budget row or stays outside product startup.
- **OQ5:** Baked-plan size discipline — does L-P need its own bytes
  budget so plans don't become the new 8.5 MB artifact?

## 9. Commissioned work register (2026-08-24 — the authoritative "what makes startup fast" list)

Every startup-affecting work item routes through this register; an agent
picking up startup work starts here. Rows are tickets unless marked
external. AGENTS.md §6's performance-direction rule points here.

| Work | Lane/gate | State (2026-08-24) |
| --- | --- | --- |
| `issues/closed/20260824-prepared-graph-default-flip.md` | G2 fail-loud + G3 default flip (the dev-lane ~4.4 s attach fix) | **G3 LANDED 2026-08-24 on the re-run** (after the same-day attempt+revert): the served-subset producer fix closed the agent-off refusal and the default dev-served lane is `prepared-graph` — Durand macOS receipts: main app agent-off admits (564 modules / 6 carriers / entryExecute ~259 ms, no fallback), blog warm evalMs 3508 → **458** (7.7x), hello 1130 → **167**; `startup-lane-fail-loud` green agent-off (`--agent-off`, lane unset) AND in the registered agent-on dev-loop mode (documented P2 fallback triple accepted exactly; measured free — no publication fetch, ≤12 ms wall delta). The dev-loop cell rides legacy until the agent-session row closes; `EXACT_STARTUP_LANE=legacy-source-table` is the kill switch |
| `issues/closed/20260824-prepared-graph-dynamic-import-edge-disagreement.md` | G3 blocker: ERR_MODULE_LINK artifact/resolver graph disagreement on lazily-split dynamic-import route-registry edges (main app + blog, agent-off) | **CLOSED 2026-08-24** — producer-side fix (the served-subset dynamic-edge rule in `packages/exact-devtools/src/prepared-graph-producer.ts`): a literal dynamic-import edge is declared AND bound iff its target is a published record; unserved boundaries stay host-bridged call-time edges, reported in `hostBridgedDynamicImportEdges`. Pinned red-first at unit level (main-app multi-edge registry map + blog `.contract` shapes) and through the real ibex dev-unarmed admission core on a lazily-split fixture. The on-device re-verification rides the flip ticket's runbook |
| `issues/closed/20260824-prepared-graph-agent-session-unavailable.md` | Post-flip dev-loop residue (was a G3 blocker): prepared publications refused agent-enabled sessions by design, so the Acto dev loop rode the documented §5.4 legacy fallback | **CLOSED 2026-08-25** — the LLP 0413.001 composition cutover: agent-enabled sessions declare `["app","agent"]` and ADMIT through the landed ibex LLP 0056 nine-step driver (vendor/ibex @8f8a1d039). Durand receipts in the ticket: `startup-lane-fail-loud` dev-loop agent-on `ok: true` with the v3 arm (admitted, app 592 + agent 33 records verified, embedded chars 0); agent-off `["app"]` admitted and green; agent-on warm medians (5/arm, main app, final tree incl. the prepared-lane ESM-interop cure) prepared evalMs **956 ms** / report-ready **1227 ms** vs legacy 1568 / 1839 — the dev-loop cell now takes the prepared win (−612 ms/boot) instead of riding legacy; all three registered legs incl. the first-paint gates green |
| `issues/20260824-prepared-lane-boot-compat-loads.md` | L-F observability: the agent-off prepared boot crosses 3 host-bridged compat loads (of 564 modules, main app) that nothing names; surface identities, then tighten the gate's counter bound | Open — P3, filed from the G3 re-run |
| `issues/20260824-llp0561-budget-gates-g1-g6.md` | G1 measurement corpus (Classes A/B) + G6 lighthouse budget binding + the L-D eager-module-count row | Gates registered red-pending (2026-08-24); **G1 Class A protocol EXECUTED 2026-08-25** (Apartment MacBook Air M5, worktree @906abeeb1): first corpus session landed at `docs/reports/startup/g1/20260825-air-m5-class-a-blog-legacy/` (n=30, raw receipts, 7 ms noise floor) — the corpus check's Class A leg validates; the intended prepared-lane BINDING session is blocked by two ticketed reds found during the run (rows below) plus the compat-loads parseFree collision, so `startup-budget-class-a`/`startup-budget-exff-paint` are honest FAILs naming their blockers; Class B + physical-device lighthouse launch path still open (simctl-only today) |
| `issues/20260825-blog-native-mount-prepared-lane-red.md` | Blog cannot mount natively on the prepared-graph lane (both carrier encodings AND the unpinned flipped default — the blog never paints; found by the G1 Class A run, invisible to the flip lane's in-VM-only receipts) | Open — P1; blocks the G1 Class A binding session |
| `issues/closed/20260825-macos-release-launch-eenv-viewport-refusal.md` | Every packaged macOS Release ExactAppMac launch refused: EENV mints before any viewport exists (`plan-environment-malformed` at +7 ms); no registered check launches this configuration | **CLOSED 2026-08-25** — shell sequencing fix: a Caltrain-selected launch with no viewport yet freezes the closed launch facts and completes selection from the engine's first-viewport callback (event-driven; EENV mints against the real first width; JS-world entry points fenced while pending; LLP 0514 M7-M8 invariants intact; two-round LLP 0334 delta review NEUTRAL). Air M5 Release evidence: selection + mint persisted at +107 ms, zero `plan-environment-*` refusals (was 16/16). New registered probe `macos-release-launch` launches the packaged Release app; it currently reports the pre-existing ratified fail-closed CapSec `arming-refused` boundary (`issues/20260728-apple-dev-served-arming-needs-v2-target-advertisement.md`) — the §4.3 EXFF paint sits before that boundary, so the `startup-budget-exff-paint` receipts are unblocked for the G1 lane |
| `issues/20260825-exff-web-font-requirement-skip.md` | EXFF silently never paints web-font routes (blog: "Inter Variable" unsatisfied every boot, artifact re-captured anyway) | Open — P3 |
| `issues/closed/20260824-lp-exff-residue.md` | L-P/G4 residue: dev-lane EXFF evidence, the correct-enough enumeration note, 0495 receipt join, EXFF paint budget row | **CLOSED 2026-08-25** — all six items dispositioned: dev-lane evidence CONFIRMED on Durand (flag unset, returning-boot capture → boot-window replay; interleaved n=6/arm: painted **222 ms** p50 vs 1008 ms flag-off first render, adoption 6/0/0/0 every run; the pre-flip ~4.4 s attach pain is gone on this lane — warm dev TTFD ≈ 1.0 s Debug, residual = 0553 territory); the §4.3 correct-enough enumeration landed as 0307 §13; the 0495 receipt join landed as 0307 §15 (`ExactPrecomputedFirstFrameBootReceipt` in host diagnostics + the presentation-domain causal receipt, live-verified; relay bounded-ingress parser added); the EXFF paint budget row registered as `startup-budget-exff-paint` (provisional ≤ 400 ms median Class A, red-pending on the G1 corpus, per-run `exff` receipt block in the session schema); G4-iOS stays on ENG-23520; the plan-runner join recorded as 0307 §14 |
| `issues/closed/20260824-lazy-native-module-init.md` | Attach-path boot tax: module instances construct eagerly at boot (verified: `CLLocationManager` every launch); lazy-until-first-dispatch registry; Android lazy from day one | **CLOSED 2026-08-24 — landed on main**: `ModuleRegistry` lazy-until-first-dispatch (Apple; kernel registration stays boot-window per 0297 A1, matching the planned kind-split shape) + Android day-one lazy declarations; 3 audited eager exceptions (brightness, toggle, searchfield — `docs/native-module-boot-policy.md`); Location's `CLLocationManager` and the iOS Taptic prewarm off the boot path (~7 ms measured Class A for location; per-module receipts in the closed ticket); receipts flag toggle+searchfield metric measurement (~60–77 ms combined, Class A Debug) as the dominant remaining module boot cost → row below |
| `llp/0413.001-prepared-graph-package-composition.spec.md` | The P2 fix: per-role packages + fail-closed union admission — **ACCEPTED r6 2026-08-24 with §10 obligations O-1–O-6** (author option-1 ruling after a 4-round dual-family loop; artifacts in llp/reviews/). **Exact-side dark half LANDED 2026-08-24**: the `docs/schemas/prepared-composition/v1/` schema package + generated §4.1 refusal registry + registered `prepared-composition-schema-parity` gate in `governance` (O-1 start; TS vector half green, O-6 matrix-completeness inside it — 17 exact-side fixtures landed); the §6 produce algorithm behind `EXACT_PREPARED_COMPOSITION=1` (default off — fixtures A1–A3, A5a, A9–A11, F44/F45 incl. the boot-core membership pin); the §5 report fields (`compositionSchemaVersion`, absent ≡ 1) + the `startup-lane-fail-loud` versioned arm truth table with its own O-5 fixture suite (rows 40–42; v1 arm = the landed P2 triple, so the gate stays green on main); the §4 steps-1–5 TS admission mirror (nothing calls it live). NO composition admits — the ibex package-aware leg (0056 §5/§9) remains; O-4's stale-cache row and the real policy digest stay open behind the dark flag. **r7/A1 lockstep conformance LANDED 2026-08-24** (same lane): the 38-row lockstep registry regenerated (O-2 DISCHARGED — parser + parity check speak the amendment format; the cross-repo lockstep leg arms automatically when the vendor/ibex pointer advances past 94c85abab), §3.1 item-2 attestation triples adopted as the produce generation's ONE carrier (delivery wrapper reconciled; packages carry `producerBinaryDigest`), and the §3.3 channel records (`ibex/prepared-composition-commitment/1` digest-only + mandatory `ibex/composition-verifier-expectations/1` incl. `resolverInventoryDigest`/`nowUnixMs`) are now the schema-package authorities ibex 0056 §3.2/§3.3 await; the steps-1–5 mirror runs 2b against the expectations struct (r7 #5 target-profile fixture landed) and #22 as the attestation invariant | **IMPLEMENTED 2026-08-25** — the ibex leg landed (0056 Implemented @0961c6931; step-6 one-pass authorization perf fix @8f8a1d039 after the first live boots measured the per-root sweep at ~6.2-9.2 s admission, now 0.32-0.35 s) and the exact-side went LIVE: the dev prepared lane serves per-role carrier-bearing `ibex/prepared-package/1` compositions for BOTH session shapes (dark flag retired), the host drives `ibex_dev_unarmed_composition_prepared_startup_v1`, the fail-loud minimum is 3 (v3 arm = the only agent-on green), and O-4 is discharged (live-check before warm reuse, freshness revalidation + re-stamp, the REAL policy digest over live serving-policy inputs, session-state unit rows). Durand evidence: v3 arm green agent-on AND agent-off (all three registered legs incl. the first-paint gates at the final tree); warm agent-on prepared 956 ms evalMs vs legacy 1568 ms. Residue: composition carriers ship the empty v3 source map (§3.1 4 KiB string bound); the `.contract` prepared-lane mount defect class also hits the main-app labs home (owned by `issues/20260825-blog-native-mount-prepared-lane-red.md`) |
| `issues/closed/20260824-acto-thin-boot-footprint.md` | Author-directed (2026-08-24): the Acto agent bootstrap becomes a minimal boot shim + lazy-loaded rest | **CLOSED 2026-08-24** — thin shim landed (`agent/server-boot.ts` + `agent/server-core.ts`; full graph lazy on first agent request). Measured: boot-eager agent share 266 modules / 2.87MB → **32 modules / 254KB** (8.3x/11.3x); the ticket's numbers fill LLP 0413.001 §7's agent-package placeholder. Residue rows below |
| `issues/20260824-agent-logs-shim-endpoint.md` | Thin-boot residue: the Apple dev shell's 750ms `/agent/logs` error poll arms the full agent graph ~0.75-1s after bind — off the boot critical path but not idle-forever; shim-served logs endpoint or host-side restraint | Open |
| `issues/20260824-caltrain-contract-memo-reachability-red.md` | Pre-existing `static-startup-reachability` red on main (caltrain-contract reaches `runtime/memo.ts`), found while gating the thin-boot lane | Open |
| **LLP 0565 + 0565.001–.005** (the 100ms Ladder) | **ACCEPTED r4 2026-08-25** after a converged 3-round dual-family super-refine (verdicts bind r3 @5dadd5939; artifacts in llp/reviews/). Rungs: R1 all-request dev restart ≤100ms p50 / R2 cold ≤250ms / R3 cold ≤100ms — all gated on M0 (M0-cold: .004; M0-restart: .003). First dispatchable lanes: M0's two halves, then .003 | **IMPLEMENTATION IN FLIGHT (2026-08-25)** — the spec trio (0565.006 M0 instrument; 0565.003.000 stage-then-swap; 0565.001.000 boot-activation manifest) authored and converged through a 5-counted-round dual-family super-refine (final verdicts bind r5; one voided round disclosed and rerun; artifacts in `llp/reviews/0565.006-m0-attribution-instrument.*`; Status Review — acceptance recommended, author's call). **Lane I1 LANDED @c0d75a8e5**: `startup-g1-session/2` + restart-session kind + typed cold/restart loader + `startup-m0-cold-attribution`/`startup-m0-restart` evidence checks (red-pending, `startup-refresh-g1`), verdict suite 55→81 — dual-family review-gated (6M+1m/1M+2m round 1, all fixed; re-review grok CLEAN + 2 codex residuals fixed with regressions). **Lane I2 LANDED @73ec3cfe8**: the M0-cold producer — host `bootAttribution` diagnostics block (0307 §15 shape; proxy-fidelity presents, honest), relay bounded-ingress parser, `bench-startup-ondevice --emit-v2/--corpus-out/--hardware-class` assembling cell-aggregated `startup-g1-session/2` ledgers (mapped-domain epoch joins w/ mandatory uncertainty, raw inputs retained, HEAD/dirty binding, fail-closed lane labels; dual-family review-gated — grok CLEAN, codex 5M+1m all fixed; live non-reference smoke: prepared-arm contradiction correctly refused, legacy-arm session validates 0-errors). **Lane I3 LANDED @ca8db66aa**: stage-then-swap WP1 — the supervisor core (supervisor-global worker identities + channel credentials; the §3 worker-set state machine over the landed WorkerLifecycle w/ bounded reap custody; the §3.4 request ledger w/ coalescing/supersession and per-request labels; the §4 desired-generation register w/ crash freeze-preemption and presented-gated resume; the §5 publication lease prepared→submitted→presented|aborted|lapsed w/ CAS, ack deadline, effects-only-at-ack, in-ack epoch fence; the §8 restart event ledger w/ all-admitted watermark + digest and mechanical receipt projections; host-facing effects as named WP2 trait seams). Dual-family review-gated across three rounds (codex 11M+1m → 4M → clean-by-fix w/ grok CLEAN twice); crate suite 475+13+14+1 green, clippy/fmt clean. Remaining, in rank order: .003 WP2 (host integration: presenter slot switch, input fencing, staging-channel multiplex/QoS + O-S1 artifacts, host-side credential verification, dev topology wiring); the Clock I probe lane (0565.006 §3.2 proof chains); M0-cold Class A v2 receipts (`issues/20260825-m0-cold-v2-receipts-on-reference-hardware.md` — reference hardware); then .001.000 (needs the ibex-side lockstep amendment pair) and .002 |
| `issues/20260824-startup-budget-ratchet-revival.md` | The LLP 0128 graph-shape ratchet's return (ask 5) | Open — author-timed |
| `issues/closed/20260824-eager-control-metric-boot-cost.md` | Toggle + AdoptedSearchField main-thread platform-control metric measurement at boot (~60–77 ms combined on Class A Debug, the dominant module boot cost after lazy-init) | **CLOSED 2026-08-24 — landed on main**: persisted tuple-keyed control-metric boot cache (`ExactControlMetricBootCache`: OS version/appearance/text-size/scale + per-module schemaVersion; miss = the pre-change sync measure; deferred staggered validation republish through the 0555 §2.3 all-node sweep, skip-on-tuple-change, degenerate-refusing; inert under XCTest; kill switch `EXACT_CONTROL_METRIC_CACHE=0`). Modules stay audited-eager (a `.lazy` flip would answer the measure callback nil — ruled out in the ticket); the eager graph shrinks in cost, not count. Measured same-binary A/B (Durand, Debug, n=15/arm): toggle 7.04→0.19 ms, searchfield 9.11→0.03 ms medians, 15/15 hits, 0 repairs; dual-family reviewed (Claude + codex, all findings fixed). Story: `docs/native-module-boot-policy.md` |
| `issues/20260824-lazy-module-state-mirror-freshness.md` | Lazy state-mirror residue: boot-seed staleness window (Location), wrapper-side ensure story, structural guard for future mirror-backed modules | Open — residue of the lazy-init landing |
| G4-iOS (EXFF default on iOS) | Rides ENG-23520, the LLP 0297 iOS runtime-thread gate — no 0561-local work | External |
| Physical-device paint-continuity + quiet-machine absolutes | ENG-23321 / ENG-22922 (0307-tracked) | External |
| Ibex builtins-prelude diet | Ibex-side (the prelude is embedded startup source, not lazy-able from this repo) | External |

The register is maintained like 0504's ledger: rows flip with dated
events; closing a ticket updates its row; new startup work adds a row
rather than living only in a ticket.

## Revision history

- **r1 (2026-08-24):** Initial draft. Commissioned by the author's
  2026-08-24 direction ("make doing all this part of the refresh";
  startup "one of the biggest problems w exact now… largely unusable…
  even on iOS and macOS") during the LLP 0559 follow-on conversation on
  React Native/Xamarin Android startup history. Repository facts
  verified during authoring: the default `legacy-source-table` lane and
  report fields (`docs/startup-report-host-surfacing.md`), 0413 §2.2/§11
  tables and Phases 0–6, the registered-check inventory (no wall-clock
  startup gate; `startup-budget` = LLP 0128 graph ratchet, quarantined;
  `startup-artifact-measurement` = LLP 0145, manual), and the Android
  host/lighthouse state (`platform-approaches.md`, `android/`,
  `scripts/check-android-*`).
