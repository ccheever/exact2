# LLP 0565: The 100ms Ladder — from one second to instant, in gated rungs

**Type:** RFC
**Status:** Accepted (2026-08-25, by Charlie Cheever — "accept the 0565 family"; basis: the converged three-round dual-family super-refine loop — grok SET READY on r3 @5dadd5939 (all six targets, 0 MATERIAL), codex 0 MATERIAL in pre-existing text on r3 with its 9 in-delta items folded as the r4 terminal labeled correction (UNREVIEWED by declaration); final verdicts bind r3; artifacts in llp/reviews/0565-startup-100ms-ladder.{codex,grok}.md; the re-ruled 0553 seam owner-re-confirmed. Mechanical status edit by agent under the author's explicit decision.)
**Systems:** Startup, Runtime, Ibex, Hermes, Kernel, Contract, Apple Hosts, Dev Server, Verification, Tooling
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-25
**Revised:** 2026-08-25 (r4 — the loop's TERMINAL LABELED CORRECTION, **UNREVIEWED** (LLP 0010.001 delta-round close: round 3 converged — grok SET READY on r3, all six targets; codex found ZERO MATERIAL in pre-existing text with 9 in-delta items, folded here rather than a fourth round). R1's product gate is now the single all-request restart-request→present p50 ≤100 ms (the leftover candidate-ready→present gate deleted; hit-only binding rejected as gameable — codex); M0-restart gains the restart→real-input-serviceable clock with the relay windowSendEvent witness (Acto re-arming is synthetic evidence, never the witness); the M0 split propagated to the lever table and §7 sequencing; layout owned exactly once path-specifically; R3's cap includes acquisition/admission (55+10+20+10=95, slack 5); measured-coverage vs budget-slack equations separated; children point at parent §5 as the single cap authority; parent Related 'quoted' line dropped) 2026-08-25 (r3 — round-2 dual-family fold (grok: 4/4 r1 folds RESOLVED, 1 new MATERIAL; codex: 13R/8P/0U of 21, ~10 new MATERIAL; overlapping standby catch resolved once). Parent: Band provenance corrected (Band A = MAIN-APP fixture, both arms' first-render quoted from the receipt — flag-on 1050 (1039–1113); Band C split by date w/ simulator provenance; Band D load caveat); §4 upgraded to a critical-path DAG (monotonic boundaries, critical-path-residue attribution for concurrent spans, overlap table + coverage checksum); §5 restated (R2 slice sum 225 vs envelope 250 = real slack; release admission/link no-≈0-claim — only HBC read is established; R1 = restart-request→present always, hit/miss/stale labels, component rows, ≤100 binds the HIT case, hit-rate diagnostic; R1 gates on M0-restart); M0 split M0-cold (.004) / M0-restart (.003); hot standby rebuilt at every new-generation admission (the edit→restart flow now hits); runtime-nucleus membership rule; nested-measurement authority split; the r2 Revised line's 'codex 22 MATERIAL' corrected to 21 by this entry (artifact carries the appended correction)) 2026-08-25 (r2 — round-1 dual-family fold; codex 22 MATERIAL (all six targets), grok 4 MATERIAL (parent/.001/.003); the three decisive codex claims spot-verified before folding. Parent changes: **§1 rewritten as qualified evidence bands** — the r1 table mixed non-comparable arms (the 1008 ms figure is the EXFF *flag-off* arm; flag-on's live render is 1050 ms; the 1011 ms release row is an older non-comparable fixture) and its "half host boot, half eval" conclusion is withdrawn [codex P-M1, grok M1]; **Milestone 0 added as the ladder's spine** — same-run critical-path attribution lands before any phase budget binds: G1 gains the exclusive-slice partition and TWO product clocks (first-correct-visible-frame AND first-correct-**real-input-serviceable**-frame — G1's current mandatory metric measures only the first, and EXFF frames are non-interactive until attach) [codex P-M2/P-M3, grok M2]; **the phase partition is defined once** (§4: spawn→host-ready | acquisition/admission | plan-eval or JS-eval | dispatch/layout | commit→present; envelopes are never summed with slices; R2/R3 restated over the partition with explicit slack) [codex X-M1]; **the Boot Activation Manifest introduced** (§6) as the single digest-bound activation authority .001/.002/.005 consume — replacing three competing closure definitions [codex X-M2]; **the 0553 seam re-ruled by authority and trigger** (0553 owns edit-driven classification/preservation/refusals; .003 owns the stable-host/supervisor swap mechanics used by explicit restart, crash recovery, config restart, and 0553's restart-runtime disposition; nested measurements may both apply) — the r1 ruling was too coarse and the prior owner-lane confirmation covered the coarse version, so re-confirmation is REQUIRED and flagged [codex P-M4]; **R1 redefined as two rows** (restart-request→candidate-ready; candidate-ready→present — the ≤100 ms user metric binds the second, with the dev hot-standby default and its mandatory cost row) [codex X-M3, grok M4]; the R2 admission+link row bound to its clock with its lever named (release ≈0 by construction; dev bound by .001's activation-boundary path) [grok M3].) 2026-08-25 (r1 — initial draft)
**Related:** LLP 0561 (the Startup Refresh Program this ladder extends — §9 register; L-G budget tables; the G1 corpus is the instrument Milestone 0 extends), LLP 0413 (§11 budget discipline verbatim; Phase 5 graph diet; §10's phase vocabulary the §4 partition refines), LLP 0307 (EXFF — §10–11 measured lanes; §13 correct-enough law; EXFF frames are non-interactive until attach), LLP 0331 (stable host / supervisor / worker ownership — cited by section in .003), LLP 0485 (flat plan; the PlanAdmissionCertificate .002 starts from; the Plan data the Boot Activation Manifest classifies), LLP 0517 (plan engine / PHI provider seam), LLP 0483 (web resumption — .002 is the native application of the same Plan architecture, not a twin transaction), LLP 0553 (dev-loop program — the §5 seam, re-ruled r2, re-confirmation pending), LLP 0413.001 + ibex 0056 (the composition/admission lockstep the Boot Activation Manifest amendment rides), LLP 0297 (threading), sub-LLPs 0565.001–0565.005, LLP 0559 (comparative frame; external-claim provenance discipline)

## Summary

The author's target: **process launch → live, interactive first frame
under 100 ms.** This RFC turns the gap into a ladder of three gated
rungs bound to the LLP 0561 G1 corpus, and commissions five lever RFCs
— but its first deliverable is honesty infrastructure: **Milestone 0**,
same-run critical-path attribution with two product clocks, because
round-1 review proved today's numbers cannot be summed into a budget
(they come from different lanes, fixtures, and instrument arms).

- **M0 — measure one thing correctly.** Two halves, two owners:
  **M0-cold** (owner .004) — G1 gains the §4 critical-path partition
  and two product clocks: *first correct visible frame* and *first
  correct real-input-serviceable frame* (receipted through the
  real-input path per the standing rule; agent synthetic taps never
  witness it). **M0-restart** (owner .003) — a registered G1
  restart-session variant: hit/miss/stale rows, component rows, the
  presentation-timestamped swap witness, AND a
  **restart→real-input-serviceable clock** witnessed through the
  real-input path (relay `windowSendEvent` minimum — Acto re-arming is
  synthetic-agent evidence and never satisfies it; presentation timing
  is a component). No rung budget binds before its half's receipts
  exist; R1 gates on M0-restart only.
- **R1 — the dev loop feels instant.** One product gate: the
  **all-request `restart-request→present` distribution, p50 ≤100 ms**
  (single-digit goal) — every restart counts, so the gate cannot be
  satisfied by a lucky minority of hits; hit/miss/stale labels and the
  component rows (candidate build, request dwell, authorized swap) are
  attribution, not gates. Dev default: hot standby **rebuilt at
  every new-generation admission** (an edit warms the next candidate
  during think-time) and after each swap; a stale candidate is
  discarded and rebuilt immediately; mandatory memory/CPU cost row.
  The user-felt invariant is no-gap continuity.
- **R2 — cold start ≤250 ms p50 (Class A), on the §4 partition with
  stated slack.** Levers: .001 (activation boundaries shrink JS eval),
  .004 (host-boot diet), the landed prepared/EXFF machinery.
- **R3 — cold start ≤100 ms p50 (Class A), warm-cache cold.** Levers:
  .002 (plan-runner-first boot), .004's deeper cuts, .005 contingent.

## 1. Evidence bands (qualified; never summed across bands)

Round 1 established that r1's table was invalid. The honest state:

- **Band A (dev, macOS Durand M4 Pro, MAIN-APP root fixture, EXFF
  interleaved A/B blocks n=6/arm, background bench load present —
  same-host paired arms, relative deltas are the signal;
  2026-08-24):** flag-off first render p50 **1008 ms** (991–1050);
  flag-on painted p50 **222 ms** (217–229) with first render
  **1050 ms** (1039–1113). Paint and first-render are different
  columns of one instrument; arms are never mixed.
- **Band B (dev, same host/date, prepared lane, agent-off):** main app
  admission 138.4 ms + link 112.5 ms + entryExecute 258.8 ms; blog
  evalMs prepared 458 vs legacy 3508 (warm medians, n=5). Main and
  blog are different scopes.
- **Band C (release, packaged-HBC, hello-shell):** whole-frame
  1011 ms p50 (2026-06-06 matrix) — and, from the SEPARATE 2026-06-07
  phase measurements on that lane (simulator provenance): VM ~4 ms;
  HBC read ~0; dispatch+layout ~7 ms; eval ~246 ms. Two dates, one
  band, labeled. **Non-comparable** with Band D's fixture per 0307's
  own text.
- **Band D (release, Caltrain, 2026-07-07, measured under heavy
  concurrent machine load per its own receipt):** EXFF painted 224 ms
  vs 334 ms flag-off, runtime-thread lane — relative delta is the
  signal.

What the bands support: the cost is *large* on every lane; JS
evaluation and pre-paint host work are both material; nothing finer.
**M0 exists because per-phase attribution from one run does not exist
yet.** Every §5 number is provisional until it does.

## 2. The five levers

| Sub-LLP | Lever | Attacks | Rung |
| --- | --- | --- | --- |
| 0565.001 | Activation boundaries + the Boot Activation Manifest's JS classes | JS eval on the critical path | R2 |
| 0565.002 | Plan-runner-first boot (PHI-provider attach; engine ownership unchanged) | JS leaves the critical path for plan-pure screens | R3 |
| 0565.003 | Stable-host keep-alive restart (0331 ownership as written) + **M0-restart** (the G1 restart-session variant) | the dev loop's whole thing | M0-restart, R1 |
| 0565.004 | Host-boot diet + **M0-cold** (the G1 cold-start extension) | spawn→host-ready and commit→present slices | M0-cold, R2/R3 |
| 0565.005 | Post-boot image (carrier-neutral, framework-realm-first) | eval for JS-heavy apps (contingent) | R3 |

## 3. Honest physics

Unchanged from r1 in substance: R3 binds warm-cache cold (process not
resident, page cache natural); truly-cold-disk gets a diagnostic row.
No sandwich-family framework ships sub-100 cold; R2 alone is
best-in-class. R3 additionally now depends on M0 showing a critical
path whose exclusive slices can sum under 100 with slack — if
attribution shows an OS floor above ~60 ms on Class A hardware, R3's
number moves to the floor plus slack and says so.

## 4. The phase partition (exclusive slices; defined once, used everywhere)

The partition is a **critical-path DAG, not a wall-clock sum**: each
node carries monotonic boundary timestamps; deliberately concurrent
spans (window vs kernel init; runtime-thread boot under presentation
setup) are attributed by **critical-path residue** — a span
contributes only the time it alone extends the path — with an
explicit overlap-allocation table and a coverage invariant
(critical-path slices + slack ≡ envelope; unattributed time is a
checksum failure, never silence). Nodes: `spawn→host-ready` (process,
dyld, frameworks, window/surface, kernel init) · `acquisition/
admission` (artifact fetch/mmap + admission + link) · `evaluation` · `dispatch/layout` · `commit→present` — with **layout
owned exactly once, path-specifically**: on the .002 plan path,
tree+layout live inside `evaluation` and `dispatch/layout` is absent
from that path's DAG; on the JS path, `evaluation` is activation-set
eval and layout lives in `dispatch/layout`. No path carries both. Two inclusive
envelopes are reported beside the slices — *paint envelope*
(spawn→first visible frame) and *interactive envelope* (spawn→first
real-input-serviceable frame) — and **envelopes are never summed with
slices**; a coverage checksum explains unattributed time. Artifact
acquisition is its own slice (r1 left it homeless).

## 5. Rung budgets (ALL provisional pending M0 receipts)

R2 (Class A, per-slice caps, **visible-frame envelope**):
spawn→host-ready ≤110 · acquisition/admission ≤35 dev (release:
HBC *read* ~0 is established; admission/link on release is NOT
established and M0 measures it — no ≈0 claim) · evaluation ≤55 ·
dispatch/layout+present ≤25 → slice sum 225, **envelope ≤250, slack
25**. R3 (**interactive envelope**): spawn→host-ready ≤55 ·
acquisition/admission ≤10 · plan-eval (incl. tree+layout) ≤20 ·
dispatch+present ≤10 → cap sum 95, **envelope cap ≤100, budget slack
5**. (Two separate equations, never conflated: measured critical-path
slices + measured slack ≡ the measured envelope — the coverage
checksum; slice caps + budget slack = the envelope cap.) R1 (**gated on M0-restart, not on
cold-start attribution**): the row is always
`restart-request→present`, labeled **hit** (warm candidate) /
**miss** (no candidate) / **stale** (candidate outdated by an edit);
component rows (candidate build, request dwell, authorized swap)
report separately so a pre-request-ready candidate can never produce
a negative or dwell-inflated number. **≤100 ms p50 binds the
ALL-REQUEST distribution** (goal single-digit) — the hot-standby
default is what makes that achievable, and hit/miss/stale attribution
explains any excursion; miss/stale shrink with R2/R3. 0413 §11 discipline verbatim: tightening routine; raising
needs corpus + writing. *[Labeled correction 2026-08-25 (spec-loop
round 2 — codex 003-M4): "rebuilt at every new-generation admission"
(here and in the Summary and .003) is implemented as the 0565.003.000
desired-generation register — every admission updates the register and
triggers reconciliation; under an edit burst, superseded intermediate
builds cancel bounded and only the latest settled generation must
complete; the daily edit→restart flow still hits a warm candidate. A
narrow semantics clarification, not a relaxation.]*

## 6. The Boot Activation Manifest (the shared activation authority)

One digest-bound manifest per route/composition, with **total
coverage**: every handler, task, cell, provider, and module is in
exactly one class — `runtime-nucleus` (the always-on runtime/ibex substrate set — fixed
by the runtime distribution, never compiler-extracted from a route
Plan), `plan-pure`
(needs no JS ever — 0485 PlanAdmissionCertificate lineage),
`eager-js` (in the boot activation set), `deferred-js` (activation
boundary; evaluated on first touch with activation+link+eval budgeted
and receipted). *[Labeled correction 2026-08-25 (recorded by the
0565.006/.003.000/.001.000 spec loop, round 2 — codex 001-M4): the
accepted .001 r4 rule is authoritative — deferred groups are admitted
AND linked at boot (0413.001 links during admission); first touch pays
**evaluation only**, budgeted and receipted; this line's
"activation+link+eval" first-touch enumeration is superseded.]* .001 defines the JS classes and their compiler-owned
extraction; .002 consumes `plan-pure`; .005 (if it proceeds) images
along the same classes. The manifest lands as a **lockstep
composition amendment** (0413.001 + ibex 0056 discipline: both
documents, parity gate, shared row bytes) with verifier recomputation
and a deterministic fallback to reachability-eager before evaluation.

## 7. Sequencing and the re-ruled 0553 seam

Order: **M0 first, in its two halves** (M0-cold: .004; M0-restart:
.003 — each rung gates only on its owning half) → **.003** (R1;
continuity independent of any M0; the ≤100 ms row waits on
M0-restart only) → **.001 + .004 cuts** (R2, gated on M0-cold) →
**.002** (R3; after the composition driver) → **.005 contingent**.

**The 0553 seam, re-ruled by authority and trigger [r2]:** LLP 0553
owns *edit-driven* work — classification, state
preservation/refusals, and edit-to-pixels budgets, including its
`regenerate-policy-and-restart-runtime` disposition. 0565.003 owns
the stable-host/supervisor **swap mechanics** — used by explicit
restart, crash recovery, config restart, *and* 0553's
restart-runtime disposition when it fires. Nested measurements may
both apply (an edit that triggers restart-runtime reports a 0553 row
and a .003 row), with authority split: **0553's rows are
authoritative for edit-to-pixels wall time; .003's for
swap-mechanics quality** — no wait is gated twice. r1's coarser ruling was confirmed by the owner lane; **the re-ruled
seam was RE-CONFIRMED by the 0553 owner-lane on 2026-08-25** (their
words: mechanism-based ownership was always going to leak, since
0553's ladder includes restart-class dispositions; the
authority-and-trigger split is strictly better; their dev-loop lanes
cite this §7 wording). No flag remains.

## 8. What this family does NOT do

Unchanged: no new measurement systems (M0 *extends* G1), no new
authorities outside the manifest amendment path, no Android rows, no
gate relaxations.

## Open questions

- OQ1: the R2/R3 route denominator — which fixture binds, and what
  fraction of real first screens must be plan-pure before R3 is a
  product claim rather than a qualified-route claim (codex OQ1; joins
  .002's purity census).
- OQ2: p50-only vs p50+p95 at R3 (0413 Phase 0 confidence rules).
- OQ3: iOS rungs after ENG-23520.
- OQ4: the real-input serviceability receipt's exact witness (relay
  `windowSendEvent` minimum per the standing rule) — M0 design work.

## Revision history

- r4 (2026-08-25): Terminal labeled correction (see Revised); loop closed.
- r3 (2026-08-25): Round-2 dual-family fold (see Revised).
- r2 (2026-08-25): Round-1 dual-family fold (see Revised).
- r1 (2026-08-25): Initial draft.
