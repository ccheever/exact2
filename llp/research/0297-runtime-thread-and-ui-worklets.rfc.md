# LLP 0297: Exact Threading Model — Dedicated Runtime Thread and the UI Worklet Runtime

**Type:** RFC
**Status:** Implemented
**Systems:** Runtime (Hermes bridge), Kernel (tree/layout/protocol), Motion, Apple Host (iOS/macOS), Windows Host, Contract (compiler), Agent API, Tooling
**Author:** Charlie Cheever / Claude
**Date:** 2026-07-04
**Revised:** 2026-08-26 (§4.1 gains a dated recording — **surface producers add no fifth execution context**: LLP 0527 proposed the producer as a first-class thing and flagged that adopting it as a *runtime* would change this RFC's owed work, and Accepted LLP 0545 §2 decision 4 ruled it a **role, not a runtime** ("attachment-class, capability-closed, no fifth execution context"). The delta to §4.1 is null, recorded as an answer rather than an absence. Executes 0504 §3 row 45's 0297 half; the 0510 half is §6.3's existing carrier binding and the authoring half is LLP 0328 D8/Q2's, so no 0508 amendment is owed. 0506 D1(d) burn-down lane.) 2026-08-25 (r26 — the §4.5 fourth-reader amendment lands, discharging
the owed reader amendment recorded by Accepted RFC 0490 §6 (its LLP 0297 row)
and named first by RFC 0115 §3 Tier 1 ratification (b): the read topology
gains the **GPU uniform sampler — main, sampler-class** (the registered
non-validating sampler class of LLP 0099/LLP 0313), sampling only at the
generated `gpu` clock phase under the GPU-uniform binding authority
(`tests/gpu/gpu-uniform-binding-v1.json`, RFC 0490 M3). Reader only: the
write-main-only topology, the command-shaped GPU-completion rule
(RFC 0490 §3.4.4), and the no-same-phase-submission-wait rule are unchanged.
Status unchanged: Implemented.)
2026-08-23 (RFC 0540 §3.6 amendment at its acceptance — the §4.4 sanctioned set is {live-resize wait, visible-content hold}; B9 uniqueness → set-membership; servicing form unchanged); 2026-07-27 (r25 — precedent-only addition: §11 gains the July
2026 RN Workers-versus-Worklets public exchange, in which both camps
independently corroborate this document's async-first rule (§5, the §6
standing rule), the one-UI-worklet-runtime convergence, and the
callback-affinity discipline — the incumbent's against-interest account of
main-runtime event routing starving workers of their own I/O is the
ecosystem-scale version of the failure the affinity registry exists to
prevent. No normative change; the resident-worker design consequences are
carried by LLP 0405 §1.4. Status unchanged: Implemented.)
2026-07-19 (r24 — LLP 0370 W1 amendment: the TUI runtime may
coalesce adjacent superseded resize constraints before layout, preserving the
newest sequence and recording every removed sample as `coalesced`; app input,
reload replay, and generation fences are non-coalescible barriers. The
successor-stamped reload replay is consumed first, and zero-size suspension
and initial-sample rules are unchanged. Status unchanged: Implemented.)
2026-07-17 (r23 — reconciles §4.9 terminology from the retired fidelity-checkpoint implementation to the accepted LLP 0368 r17 purpose-authored TUI product host. The threading topology and absent-context decisions are unchanged. Status unchanged: Implemented.)
2026-07-17 (r22 — makes the viewport latch exclusively main-owned: reload copies it into an immutable successor-stamped B5 message, and the runtime consumes the message without reading shared latch state. Absent-context rejection is structural over the app endowment, initial zero-size boot is explicit, the Motion wording is reconciled into LLP 0368, and affinity enforcement is assigned to the future registered on-demand host check plus Phase 1 exit. Status unchanged: Implemented.)
2026-07-17 (r21 — closes the terminal-host profile review: authoritative terminal size is a generation-independent viewport latch admitted before first publication and replayed first on reload; app events alone are stale-generation-dropped; every app-endowed Workers/Worklet op is enumerated and rejects with a typed absent-context result; the Motion domain is uninstantiated; TUI is explicit in §4.7; isolate flags, precomputed-first-frame, teardown timeout, zero-size layout, and affinity-gate enforcement are dispositioned. Status unchanged: Implemented.)
2026-07-16 (r20 — adds the bounded LLP 0368 checkpoint terminal-host profile in §4.9 (subsequently renamed the product-host profile by r23 without changing its topology). The TUI host starts dedicated-runtime-thread-on; keeps the tree/layout domain and app runtime on that thread; keeps a minimal main-role Context 1 solely for terminal input/presentation and B5 viewport constraints; has no UI worklet runtime, Motion recognizers/drivers/bindings, or Workers; and rejects text-input filter installs with a typed diagnostic. The profile fixes guardian/child responsibilities, resize, clocks, reload/teardown, and prospective callback affinities before host implementation. Status unchanged: Implemented.)
2026-07-13 (r19 — LLP 0099 M6's conditional install-format amendment is decided: React/TS math worklets normatively install UTF-8 function-expression source plus typed captures, not HBC. The existing build plugin already produces that artifact; the restricted typed installer deliberately accepts an explicit source-format enum and rejects unknown formats. A 64-artifact warm-install characterization in Ibex measured p50 0.081ms / p95 0.182ms against a 5ms mount-time ceiling, while no self-contained restricted-worklet HBC + capture artifact pipeline exists to consume. HBC may be added as a new explicit format if future cold-mount evidence warrants it; it is not implicit in the bytes. The same M6 native seam adds stable content+capture identity, fixed f32 invoke/output slots, validated SharedValue captures with stale shadows, and a generation-fenced bounded drop-oldest `runOnJS` drain into the app-runtime executor. The nested/inline authoring extraction and output lowering remain 0099 M6 authoring work. Status unchanged: Implemented.)
2026-07-13 (r18 — round-3 reconciliation of the transport record: the §4.4 "lock-free only" sentence is scoped to the data planes, with the shipped typed commit-output crossing named as the drain-to-staging carve-out's lock-backed condvar queue (its unbounded capacity under a stalled main recorded as a gap owned by 0099 M2's transport work) — the round-3 Codex review read the old sentence as contradicting the shipped `NSCondition` staging queue, and the scoping note makes the intended relationship explicit. Status unchanged: Implemented.)
2026-07-13 (r17 — Motion-set round-2 fixes: the §4.5 "block W4b" sentence is made historical (W4b shipped on the spike with the slot-lifetime invariants deferred to 0099 M1 — the r16 honesty note had left the old wording contradicting W4b's shipped status); §4.5's single-word slot shape adopts 0099's ledger refinement (a) — the tagged `AtomicU64`, superseding the `AtomicF32` spike spelling (refinement (b), the write topology, landed in r16); §4.8 gains a status marker separating the shipped worklet-generation reset from the not-yet-implemented slot semantics; `docs/callback-affinity.md` reconciled (the scheduled-drain row says buffered-unwired, and the island producer/store rows say @MainActor today). Status unchanged: Implemented.)
2026-07-13 (r16 — the LLP 0099 ledgered amendments land, applied in-set by the Motion-set super-refine round-1 reconcile (artifacts `llp/reviews/0099-0100-0281-0297-0313-0336-set.{fable,codex}-round1.md`): §4.5's access topology becomes read-three-places / **write-main-only** (imperative app-runtime writes serialize as Motion-applied commands; the r2-era "written from three places" superseded), with the decision-record control class named as the sole two-writer state OUTSIDE the slab; §4.4's plane inventory gains the acknowledged outcome log + consumed cursor, applied-watermark control atomics, and decision-record control words, and its prose + the §Summary diagram sweep the old "both directions/both ways" write phrasing (reads remain bidirectional); §4.3 gains two shipped-state honesty notes (the shipped install is source-text/JSON — HBC + capturedConsts is contract language owned by 0099 M6's conditional amendment; `scheduleOnAppRuntime`'s production drain is unwired — the escape is contract language until M6, and runOnJS-class delivery is bounded drop-oldest/observational); §4.5's W2 boundary states the shipped spike honestly (fixed 64-slot generation-less `Vec<AtomicU32>` + raw-pointer worklet binding = superseded substrate; the generation/tombstone invariants are contract language 0099 M1 implements); the `worklet_wedged` dev interruption is corrected to intended-pending-OQ1, not implemented behavior. Status unchanged: Implemented — the amendments narrow contract language to match shipped code and enumerate the new planes; no shipped behavior changed.)
2026-07-07 (r15: the TextInput `filter` prop ships — ENG-23560
promotes `worklet.installTextInputFilter` to a supported surface (plus a
remove op), adds build-time `'worklet'` directive extraction
(`createWorkletSourcePlugin`, the §4.6 bundler-plugin path), a Contract
`input filter=` binding, and same-keystroke `replace` verdicts end-to-end
(AppKit partial-string correction / NSTextView substitution / UIKit
programmatic apply) — the §4.3 fail-open replace degradation is gone.)
2026-07-07 (r14: A3 async-revert convergence fix, ENG-23550 —
the observed-revision watermark (`textInput.ackObservedRevision`) closes the
controlled-rejection gap the retired veto left under the flip; recorded in
the A3 status notes.)
2026-07-07 (r13: the per-platform default-on calls are made and
recorded in **LLP 0322** — macOS default-ON (unset `EXACT_RUNTIME_THREAD` ⇒
dedicated thread; `=0` is the kill switch back to the W1 executor), with
`EXACT_PRECOMPUTED_FIRST_FRAME` coupled to the effective mode per LLP 0307
§11; iOS stays opt-in pending its §7 gate (ENG-23520, blocked on the
ENG-23487 device IME legs + scroll/Stage Manager fixtures); Windows stays
opt-in pending the ENG-23494 validation surface. The text-heavy at-threshold
row is tracked as ENG-23521. ENG-22779 closes with this revision.)
2026-07-06 (r12: **Status → Implemented** — the r9 condition is
met: ENG-22896 closed with the eval-tail drain fix, and ENG-22897's
measurement remainder is recorded here and on the issue. W5 close-out runs
(macOS, Debug dev-server lane, real AX `edge_drag_live_resize`, visual
sentinel ARMED in `window` mode — per-window `screencapture -l` works again
on this macOS 26 host, superseding the r9 "broken app-wide" record; positive
`runtimeThread` provenance now flows through the relay after the ENG-22897
passthrough fix): flag-off published p95 static **0.37ms** / js **0.87ms** /
text-heavy **4.37ms**, all sentinel-gated PASS; flag-on static **0.71ms**
PASS / js **2.80ms** PASS / text-heavy **16.7–17.2ms across four clean
runs** — AT the 16.7ms one-frame threshold and ~3.8× the flag-off baseline
(§4.4 wait telemetry healthy: fallbackRatio 0.2, maxConsecutiveMisses 2;
supersedes both the r9 saturation FAIL and ENG-22936's interim 13.2ms) —
the one row default-on macOS must weigh. Packaged-Release no-dev-server
idle CPU: flag-off **1.8–2.6%** (the 16ms free-running pump is the floor),
flag-on **0.08–0.12%** — the predecessor <0.5% target is met **only
flag-on** (B8 wake-driven executor), converting the idle-CPU row into a
default-on argument. Harness characterization: a single-sample ~120px
sentinel edge-gap transient recurs across flag states and fixtures with
adjacent samples flush — a mid-hop capture artifact for the 96px
calibrated ceiling to absorb on the pinned host, not the unbounded
staleness class. Edge-drag parity is closed by the matrix itself (all runs
edge-drag). Still open operationally, tracked on issues not this status:
iOS device rows (ENG-22890 — on-device worklet numbers/IME matrix), the
per-platform default-on decisions, and the Windows dedicated-thread
follow-through behind the dormant ENG-22916 seam.)
2026-07-06 (r11: OQ4 closed by LLP 0313's acceptance. The
0297 threading authority is unchanged: the constraint-buffer route remains
the default for arbitrary layout-affecting bindings, same-frame layout
islands are the accepted bounded fast path for eligible subtrees, and the
live-resize servicing-wait carve-out is not widened except by an accepted record — RFC 0540 §3.6 added the RFC 0540 §3 visible-content hold (LLP 0543 §11, added 2026-08-23 at RFC 0540's acceptance) as the sanctioned set's second member.)
2026-07-05 (r10: OQ4 sharpened and the same-frame
layout-island path spun out as LLP 0313. No 0297 threading-authority change:
the constraint-buffer route remains the default until LLP 0313 is accepted;
OQ4 now records the investigation order.)
(r9: W5 measurement pass (ENG-22785/ENG-22897) —
§8 rows measured and recorded on those issues: worklet invoke overhead
p95 1.07µs (PASS, ~230× headroom), gesture-worklet same-frame @120Hz
p95 0.248ms per sample through the production recognizer→worklet→layer
path, 0/600 over the 8.33ms budget (PASS; `worklet.gestureDriverBurst`
DEBUG op + `testGestureBurstSameFramePerformance120Hz` are the harness),
agent-tree p95 under stress 3ms flag-on vs 18ms flag-off (PASS), cold
start FAIL → ENG-22896 (root-caused: agent bootstrap holds the runtime
thread post-eval; fix in flight), live-resize parity + idle CPU still
environment-gated → ENG-22897. Layout-binding next-frame row is not
measurable: no layout-binding mechanism exists yet (LLP 0099 Motion
domain; see OQ around Tier-2 layout bindings). The "Current state" cites
below were corrected for the line drift catalogued in
`docs/sync-boundary-inventory.md` §10. Status stays Accepted — not
Implemented — until ENG-22896 and ENG-22897 close W5 out.)
(r8: **W4b core SHIPPED** (ENG-22784) — the flip
runs end-to-end under `EXACT_RUNTIME_THREAD=1`: B7 split (tree domain =
`ExactEngineTreeDomain`, kernel parse/layout/sync-producer on the runtime
executor; presenter applies typed batches via `ExactEngineCommitPublisher`,
inline pre-flip — full mac suite byte-parity), B8 wake-driven loop (park
until `ex_hermes_next_timer` / host wake hook `ex_hermes_set_host_wake_hook`,
ibex `w4b-host-wake-hook`; no main-side tick under the flag), B4 input
enqueue (event flow = one ordered runtime-thread task; optimistic Bool),
B5 constraint marshals (viewport/keyboard/kernel-handle/screen-dims), §4.4
mechanism (resize-priority lane, ~4ms servicing wait in A4
drain-to-staging form via `waitForCommit` + staged main drain,
busy-skip → presenter frame/bounds stretch, 50ms drag-end settlement,
per-drag telemetry), §4.8 epoch fencing (reset rides the commit stream).
Live-verified on macOS under fatal B9: boot → 322-node first render →
agent tree → tap → theme flip → navigation → engine restart → re-render,
zero affinity/scope violations. Two flip-only bugs found and fixed: the
dispatch-handler installers raced the async runtime boot (silently
no-oping against the nil handle — same class as ENG-22840), and a
lock-order inversion between the ibex callbackMutex and the executor
condvar (notify now fires outside the mutex; the idle plan probes outside
the executor lock with a wake-generation check). Remaining for W4b exit:
the WS6 resize-harness fixtures flag-on, iOS scroll/IME/Stage-Manager
device gates, agent-tree p95 under stress, OQ11 cold-start recording.)
(r7: **W4a COMPLETE** (ENG-22783 Done) — the host
data-plane feed carries every record type; ENG-22820 resolved as
presenter-owned secondary windows (root-scoped projections of the engine
mirror; two latent multi-window bugs fixed and windows render again); A2
off-main measurement validated (`ExactOffMainTextMeasurementTests`);
the Motion geometry mirror decided as feed-frames + presenter snapshot
(no dedicated structure until LLP 0099's first driver); per-root a11y
mirrored; the A1 owner-scope audit is **always-on in dev builds** with a
clean suite; **OQ10 decided: hybrid as-built** (322-node data: boot
5.81ms, whole-tree-dirty ~10ms worst case; escalation trigger recorded in
`docs/host-data-plane-feed.md`). **W3 phase 2 shipped** — the ibex
`__hostCallAsync` promise channel (ibex `4f9dea9`) services the host-call
op switch on the main actor with no semaphores; JS wrappers are
async-first (`hostCallAsync` in `@exact/core/host-call-bridge`;
sync-typed desktop reads flipped to `MaybePromise`); all nine
`evaluateString` sites converted to the executor-marshaled
`evaluateStringAsync` (the window-close veto became the A3-style async
consent); **B9 assertions upgraded to crashes** in dev builds
(`EXACT_AFFINITY_CRASH=0` escape); input-dispatch decision recorded —
the sync transport is replaced by B4 at the flip together with the A3
preflight retirement; remaining B7 untangling lands with the W4b
executor swap, which is where it becomes expressible. **W2 tail:** the
worklet runtime is **resident** (created at boot, survives app-runtime
resets with generation fencing), filter worklets are wired into the real
text-input preflight (same-keystroke reject/accept, IME passthrough;
`replace` is honored end-to-end since ENG-23560's TextInput filter
prop), and the
OQ3 dual CDP target is implemented (`Exact — App` id 1 / `Exact — UI
Worklets` id 2, per-target token paths, simultaneous attach). The
gesture-worklet transform demo explicitly rides LLP 0099's first driver
per the geometry-mirror decision; worklet-stdlib HBC precompilation is
deferred with cause (create+prelude measured 0.57ms once per process);
on-device IME matrix and iOS hardware measurements remain
human/device-gated. **W4b groundwork:** the dedicated runtime-thread
executor exists and is unit-tested (B1 named thread + B8 wake-driven
loop, `ExactDedicatedRuntimeExecutor`) behind the staged
`EXACT_RUNTIME_THREAD=1` flag, default off.)
(r6: W1 shipped — executor seam, tree-domain scope,
B9 warning audits (`9c86d326`, ENG-22780 Done). W2 core shipped — the
`ex_worklet_*` restricted worklet runtime, kernel SharedValue slab (§4.5
spike), and Swift `ExactWorkletRuntime` (`097d3d0b` + ibex `d82e92a`,
ENG-22781); measured on the real invoke path: create 0.57ms, warm invoke
p95 2.8µs (§8 target 250µs), small-heap GC 0.238ms, steady RSS 1.59MB
(≤4MB budget). **Dependency note:** the W2 ibex work has been reconciled
into Ibex `main`; Exact tracks Ibex `main` during Phase 0 joint development
while recording tested submodule SHAs (standing record in LLP 0180 §9.1).
W4a's feed schema seeded at
`docs/host-data-plane-feed.md` (`56eccbde`, ENG-22783). Post-acceptance
addendum from the LLP 0299 Lynx dual-thread comparison — no design change:
adds Lynx as prior art (§11), OQ11 cold-start first-frame (§10), a
cold-start first-commit-to-pixel metric (§8), and a Context-2
capability-boundary clarification (§4.1).)
(r5: Draft → Accepted — both review families'
round-3 disposition was that author acceptance without a further round is
defensible after r4, and implementation was commissioned as ENG-22779.
The §6 upon-acceptance changes are applied in the same commit: predecessor
RFC header/B2/B4/B5/B6 amendments, LLP 0099 substrate note, LLP 0120
re-point, LLP 0161 forward note, EXACT_UNIFIED_SPEC §Threading Model
authority pointer, and the `threading-model` boundary row in
`exact-contracts.json`. W0 executed: standing rules in CLAUDE.md §6,
`docs/callback-affinity.md` created (B6 tracked artifact), the W1
sync-boundary inventory seeded at `docs/sync-boundary-inventory.md` (its
§10 records line-drift corrections to fold into this document's next
revision), and the child issues filed under ENG-22779: W1 = ENG-22780,
W2 = ENG-22781, W3 = ENG-22782, W4a = ENG-22783, W4b = ENG-22784,
W5 = ENG-22785, W6 = ENG-22786.)
(r4, after the third review round — round-3
sections appended to both artifacts; both families recommended one more
focused revision, after which author acceptance without a further round is
defensible. Applied in r4: A4's servicing wait is now **drain-to-staging**
(consume-without-apply, single ordered apply at wait end) with the
forced-by-B4 justification — the Claude round found naive swap-only
violates B4's consume-before-swap invariant, the GPT round found mid-wait
application reentrancy-unsafe, and drain-to-staging satisfies both; the
W4a owner scope is ambient over the runtime executor (covering the JSI
host functions, sync event dispatch, and eval paths that never touch a
Swift driver), with nesting and boot-handoff semantics; the A3
secure-fields row rewritten honestly — digest-only transactions make
content filtering unavailable by design, so secure limits become native
props; the read-only row's transition-window race named; §4.6 splits
math-class (no-strings) from filter-class (string-stdlib) worklets; §4.7
states UIKit exposes no public interactive-resize span — iPadOS
applicability is a W4b verification item, else B5-only and the carve-out
stays macOS-scoped; §4.4 phases the two waits as one mechanism, notes that a
future keyboard use would extend rather than reuse the sanctioned set (as
RFC 0540 §3.6's visible-content hold did on 2026-08-23, with its own B9
registration), and
routes prolonged post-drag stretch to the watchdog/supervisor; §6 adds the
B2 qualification; §8/W4b gain the text-heavy fixture and conditional
iPadOS wording; the ~2.2ms figure flagged as a pre-AppKit-presenter
baseline; §4.4's stale-id rule broadened to generation-stale slots.)
(r3, after the second review round — round-2
dispositions and new findings appended to both artifacts in `llp/reviews/`;
both families judged the round-1 concerns resolved (GPT: 5 resolved / 5
partial; Claude: 11/11 resolved) and recommended revise-and-stay-Draft.
Applied in r3: **A4** — B4's producer-blocking back-pressure amended so any
sanctioned main-side wait is a *servicing wait*, fixing the wait↔swap cycle
both round-2 reviews flagged; the W4a exit gate restated as an owner-scope
assertion (a thread-scoped one is unsatisfiable while W4a is still
single-threaded) with the state-mirror pointer exemption; A1's feed extended
with the selection document model, node transitions, and a module-id
pre-allocation plan, plus a tracked feed-schema artifact
(`docs/host-data-plane-feed.md`); §4.4 resize honesty — cites 0161's
published ~2.2ms layout-publish figure instead of an unpublished "~3ms",
adds skip-wait-when-runtime-busy, names the 50ms drag-end budget and on-miss
behavior, states the stretch-vs-hitching trade for JS-responsive apps, and
gates on a JS-responsive fixture plus a consecutive-fallback bound; A3
gains the migration table (read-only, masks, currency/phone, secure,
cross-field via SharedValue mirror) and deprecation mechanics; §4.8 defines
in-flight/missing-worklet/crash-restart semantics; §4.7 covers iPadOS Stage
Manager resize via the host-neutral interactive-resize span; §8 adds wait
telemetry; OQ9 gains its escalation clause; citation nits fixed.)
(r2, after the first review round — GPT review in
`llp/reviews/0297-runtime-thread-and-ui-worklets.gpt.md`, fresh-context Claude
review in `...claude.md`, both recommending revise-and-stay-Draft. Applied:
corrected Current-state reality-check errors (hermesLock is not airtight;
state mirror is a seqlock, not double-buffered; `CALLBACK_PENDING` lives in
`engine/mod.rs`; dispatch is staged through a holder + main-queue drain;
`measureTextNative` is at :1223); rewrote A1 with the full main-thread kernel
surface and added the pre-flip kernel-free-main workstream (W4a); rewrote A2
— the measurement core is already CoreText, the real work is off-main
validation + the font-registry invalidation race; rewrote A3 — the text-input
preflight is a controlled-input round trip that cannot move to a worklet, so
the migration is an ecosystem-visible input-contract change (Layer 1 +
optional worklet filters), now stated as such with a verdict schema and
IME/secure-entry matrix; §4.4 now defines Motion-domain data ownership,
id-lifecycle ordering, and names the live-resize mechanism as the single
sanctioned bounded-wait carve-out; added §4.8 lifecycle (HMR/generation
resets across two runtimes, eval/run-js/Design Mode routing, teardown
ordering); §4.5 names per-slot atomics vs seqlock and the 0297/0099 ownership
boundary; §4.6 softened to provable-eligibility framing with read-side
constraints; W1 audit scope extended to the `__hostCall`/`DispatchSemaphore`
sync surface; W4 split into W4a/W4b with per-platform staged default-on;
§8 metrics qualified and extended; OQ list updated. Declined: replacing the
worklet heap metric with RSS-only (kept both).)
**Related:** `hermes-runtime-queue-and-agent-reliability.rfc.md` (Initiative A: implemented; Initiative B: inherited and amended by this RFC, which becomes the rollout authority), LLP 0099 (Motion — owns SharedValue/gesture/driver semantics; execution substrate amended here), LLP 0121 (Hermes runtime explainer), LLP 0136 (Exact vs RN rendering §6 Threading), LLP 0161 (macOS live resize — the main-thread render path §4.4 preserves via the bounded-wait carve-out), LLP 0178 (macOS ANR zombies — the JS-on-main incident record), LLP 0120 (Windows runtime port — carries the "moves to a dedicated thread when Initiative B lands" note), LLP 0160 (Contract by default — Contract-first binding rule), LLP 0281 (Contract transient bindings as the language projection of SharedValues), `EXACT_UNIFIED_SPEC.md` §Threading Model (historical; superseded by this document upon acceptance), LLP 0299 (Lynx dual-thread comparison — sources the r6 addendum), LLP 0145 (Exact vs RN startup — item 5's precomputed-first-frame lever and the measured cold-start baseline behind OQ11), LLP 0252 (static-reactive SSG — the hydration/adoption model OQ11's native precomputed frame ports), LLP 0313 (same-frame layout islands — accepted answer to OQ4), LLP 0368 (TUI target/platform revival — §D3 terminal-host profile)

## Summary

This RFC defines the end-state threading model for Exact's native hosts and
the migration to it. Two decisions:

1. **The app JS runtime moves off the main thread.** One Hermes instance
   (the *app runtime*) runs on a dedicated, named thread together with the
   kernel's tree/layout domain and Taffy. This inherits Initiative B of
   `hermes-runtime-queue-and-agent-reliability.rfc.md` (B1–B9) wholesale,
   with four amendments recorded in §4.2.

2. **A single persistent UI worklet runtime is added.** A second, restricted
   Hermes instance (the *worklet runtime*) is owned by the main/UI thread,
   created at boot and kept warm for the process lifetime. It exists so that
   the small class of JS that must answer *synchronously on the UI thread* —
   gesture→value mapping, text-input filters, custom frame callbacks — has a
   home when the app runtime leaves main. It is the concrete realization of
   LLP 0099's "constrained worklet VM."

Deliberately **not** in this design: a pool of worklet runtimes. Sync
invocation from a single UI thread is serial by construction, and worklets
depend on install-once/invoke-many warm state; one persistent instance
dominates a pool on every axis (§5). Parallel background JS (a worker-isolate
pool, where 2–4 instances *is* the right shape) is real but separate future
work (§7, W6).

The result is the original `EXACT_UNIFIED_SPEC.md` three-layer threading
diagram, made precise and made migratable:

```text
┌────────────────────────────── MAIN / UI THREAD ──────────────────────────────┐
│ AppKit/UIKit/Win32 · presenter apply (opcode-ring consumer, display link)    │
│ Motion domain (Rust, main-owned): gesture recognizers · drivers · bindings   │
│ UI worklet runtime (Hermes #2): input filters · gesture worklets · frame cbs │
│ Constraint capture: keyboard / safe area / viewport → constraint buffer      │
└──────────────┬─────────────────────────────────────────────▲────────────────┘
   input-event │  constraint buffer │  SharedValue slab      │ opcode ring +
   queue       ▼  (atomic writes)   ▼  (main-writes, both    │ host data-plane
                                       read — r16; + control │
                                       words/outcome log)    │
┌────────────────────────────── RUNTIME THREAD ────────────────│──feed (§4.2)──┐
│ App runtime (Hermes #1): app JS · reconciler · agent JS endpoints            │
│ Kernel tree/layout domain: protocol dispatch · Taffy · text measure (CoreText)│
│ Module callback draining (fetch/WS/HTTP/timers)                              │
└──────────────────────────────────────────────────────────────────────────────┘
   Module-private threads (unchanged): PTY readers, IO, streaming bridges
   Future (W6): 0–N background worker isolates (async), incl. the agent isolate
```

## Motivation

### Why move the app runtime off main (recap)

The predecessor RFC's case has only strengthened since March 2026:

- **Incident record.** LLP 0178 documented a real while-running ANR class
  that existed *because* JS shares the main thread; the stdio fix removed one
  trigger and explicitly left "long-running JS on main" as an accepted
  hazard.
- **Performance direction.** The 60fps floor / 120fps stretch scroll targets
  and the sub-second cold start budget cannot be defended long-term while
  app JS, GC pauses, kernel dispatch, Taffy, and renderer apply all compete
  for one thread's frame budget.
- **Agent liveness.** The agent API — the repo's primary verification
  surface — hangs whenever main stalls. Initiative A made this observable;
  only thread separation makes it structurally survivable.
- **Ecosystem deadline.** The 2026 launch freezes the public module ABI. If
  third-party modules can assume "sync call ⇒ I am on the main thread," the
  migration becomes a breaking ecosystem event (React Native's New
  Architecture took ~5 years to roll out for exactly this reason). The
  threading contract must be settled before the module surface is public.
- **The assumption is metastasizing.** The Windows host deliberately cloned
  the main-thread model "matching today's macOS shell" (LLP 0120), and new
  sync couplings keep landing — e.g. `preflightTextInputTransaction`
  synchronously round-trips a keystroke through arbitrary app JS inside a
  platform event handler
  (`ios/ExactApp/ExactApp/Engine/ExactView.swift:468` and the duplicate
  preflight at `Engine/ExactEngine.swift:2237`; see A3).

### Why a UI worklet runtime (the new argument)

Initiative B alone converts every synchronous JS touchpoint to async. For
most of the module surface that is correct. But a small class of logic is
*legitimately* synchronous with the UI thread:

- **Text-input filters** — the spec's own v1-core commitment
  (`EXACT_UNIFIED_SPEC.md:522`, "TextInput with Worklet Input Filters"):
  masking/formatting must land in the same keystroke, not a frame later.
- **Gesture→value mapping** beyond what declarative recognizers express:
  custom resistance curves, rubber-banding, conditional snapping.
- **Custom per-frame callbacks** for effects the declarative tiers don't
  cover.

Forcing these through an async hop to the runtime thread would regress
exactly the interactions Exact exists to make excellent. The worklet runtime
is where the W1 sync-boundary audit sends the couplings that *should not*
become async. LLP 0099 already reserves this seat ("the worklet VM should
remain a constrained math-and-binding runtime, not a second general-purpose
programming environment," LLP 0099 §Design constraint 8); this RFC supplies
the substrate. (Note that the largest sync coupling in the tree today — the
controlled-input preflight — is *not* in this class and does not migrate to
a worklet; see A3.)

## Current state (2026-07-04, verified against HEAD; corrected in r2)

Where `ex_hermes_poll` — and therefore all app JS — runs today:

| Host | Driver | Thread |
|---|---|---|
| iOS | `CADisplayLink` added to `.main` (`ExactRuntimeEngine.swift:3156`) | main |
| macOS | 16ms `DispatchSourceTimer` on `DispatchQueue.main` (`ExactRuntimeEngine.swift:2868,3160-3166`) | main |
| Windows | 16ms `WM_TIMER` in the Win32 pump (`packages/exact-host-windows/src/windows_host.rs:112,1694`) | UI thread |
| ibex CLI | `#[tokio::main(flavor = "current_thread")]` (`vendor/ibex/src/bin/ibex/main.rs:229`) | single tokio thread |

Everything runs on that one thread: JS microtasks and timers →
`exact.dispatch` → kernel `processBuffer` → Taffy layout with the
text-measure callback (`Kernel/ExactKernel.swift:27-81` →
`Platform/PlatformTypes.swift:1223`) → node sync → render-snapshot rebuild →
presenter apply (`Engine/ExactEngine.swift:1585-1679`). Dispatches are
staged through a `DispatchDataHolder` and drained via a
`DispatchQueue.main.async` hop (`ExactRuntimeShell.swift:4077-4089`) — a
same-thread deferral, not a thread handoff. Despite the protocol's zero-copy
design, there is **no cross-thread transport in the render path today**.

Couplings that break under a naive queue move (the W1 audit's seed list):

1. Synchronous input dispatch, including sync JS return values
   (`ExactRuntimeShell.swift` `dispatchRendererEvent` `:4317-4368`, sync
   dispatch `:4341`; `ExactRuntimeEngine.swift:3098-3112`),
   with the controlled-input preflight as the extreme case — it dispatches
   into arbitrary app JS and compares the post-dispatch node text against
   the proposed text (`ExactView.swift:498,509-511`; see A3).
2. The text-measure callback executing on the layout caller's thread
   (`kernel/src/ffi.rs:1219-1371`); measurement itself is already CoreText
   (see A2) but its caches and the font registry are invalidated from main.
3. Main-thread kernel access far beyond the snapshot rebuild: the node-sync
   read pass (type, children, layout, style, text, ~20 string props,
   `mergedAccessibilityTree` — `ExactEngine.swift:1688-1847`, string props
   at `:1811`), event routing
   via `kernel.getEventHandler` (`ExactEngine.swift:2212,2234`), the
   selection API surface (`ExactView.swift`), `ModuleRegistry` registration
   and state-mirror reads, secondary-window layout
   (`ExactView.commitLayout`), and main-initiated kernel *writes* such as
   `remeasureTextForFontRegistryChange` (`ExactEngine.swift:369-385`) —
   all against a kernel with **no internal synchronization**
   (`kernel/src/ffi.rs:213`). JS reads the same kernel directly over FFI
   (`vendor/ibex/src/engine/hermes_runtime_ios.cc:320-557`).
4. `dispatchBuffer → finalizeProcessedBuffer` continuing inline into
   `@MainActor` engine state and platform views.
5. The `__hostCall` sync surface: 57 `DispatchSemaphore` sites in
   `ExactRuntimeEngine.swift` alone synchronously bounce a non-main caller
   onto `MainActor` and wait (screenshot capture `:1367`, host scrolling
   `:1451`, dialogs `:2279`, Design Mode panel `:1817` with its semaphore
   at `:1827`). Today the waiting
   caller is a background drain; after the move it is the runtime thread —
   each site is a priority-inversion and, if main ever waits on the runtime,
   deadlock candidate.

Already-built machinery this design stands on: per-runtime thread-affinity
capture and marshaling in the C++ layer (`hermes_runtime.cc:1412,652-662`),
mutex-guarded callback/task queues drained by the poll
(`hermes_runtime.cc:1822-1848`), the `CALLBACK_PENDING` atomic +
`ex_hermes_notify_callback` wake path (`vendor/ibex/src/engine/mod.rs:16-42`),
`hermesLock` on the Hermes entry points — not airtight: `setKernelHandle`
bypasses it (`ExactRuntimeEngine.swift:3114-3117`), and W1 inventories the
exceptions — `DisplayLinkCoordinator` on both Apple platforms
(ENG-22685/ENG-22690), and the **seqlock-protocol** shared-memory state
mirror (`kernel/src/modules/state_mirror.rs:3`; the historical
double-buffered machinery was removed by LLP 0159,
`kernel/src/protocol/buffer.rs:16-18`).

## Design

### 4.1 Execution contexts and decision rules

Four places logic can run, ordered by latency criticality. The rule for
authors (and for the W1 audit) is: **put logic in the lowest-numbered
context that can express it.**

| # | Context | Thread | Runs | Owner doc |
|---|---|---|---|---|
| 1 | Declarative / constraint | main (Rust) + runtime (Rust) | Tier-1 transitions; Tier-2 Motion recognizers, drivers, bindings; B5 constraint fast path (keyboard, safe area, viewport) | LLP 0099; predecessor B5 |
| 2 | UI worklet runtime | main (Hermes #2) | input filters, gesture worklets, custom frame callbacks | this RFC §4.3 |
| 3 | App runtime | runtime thread (Hermes #1) | app code, reconciler, module JS, agent endpoints, `evaluateString`/run-js/Design Mode (§4.8) | predecessor Initiative B |
| 4 | Workers | worker threads | parallel/background JS, agent isolate | future (W6) |

Context 1 handles the common case with **zero JS per frame** — keyboard
avoidance, springs, scroll-linked transforms, orientation. Context 2 is the
escape hatch for custom synchronous interaction logic. Context 3 is
everything else. If a surface seems to need Context 2, first check whether a
Tier-1/Tier-2 declarative primitive (or a missing one worth building) covers
it — worklets are the escape hatch, not the workhorse.

**Capability boundary (r6).** Context 2 worklets *compute and write
SharedValues*; they do not mutate the tree. Synchronous structural change on
the UI thread is deliberately **not** a worklet capability — it is Context 1
(a declarative primitive, existing or worth building). This is what lets
Exact keep one kernel tree with a single owner and skip a cross-thread tree
reconciliation. It is the sharpest divergence from Lynx, whose main-thread
scripts *can* imperatively mutate the element tree on the UI thread at the
cost of running the framework twice and reconciling two trees; see LLP 0299
§6.

> **Surface producers do NOT add a fifth context — recorded 2026-08-26
> (LLP 0527 §3.2 / OQ10; ruled by Accepted LLP 0545 §2 decision 4;
> 0504 §3 row 45's 0297 half).** LLP 0527 proposed the **surface
> producer** — camera, decode, draw/compute, ML — as a first-class
> thing, and flagged that if it were adopted as a *runtime* it would
> change this RFC's owed work by adding a context to the table above.
> **It was adopted as a role, not a runtime.** LLP 0545 §2 decision 4
> is explicit: producers are "attachment-class, capability-closed, **no
> fifth execution context**", and LLP 0527 itself states that "0297's
> canonical context list stands." **The delta to this section is
> therefore null**, and that is the recorded answer rather than an
> absence of one.
> What a producer *is*, in this table's terms: a producer is scheduled
> by the host on an existing context — its work runs where its
> placement puts it (in-process native, wasm, or child worker, i.e.
> Context 4's territory or below the JS contexts entirely) — and it
> declares its scheduling rather than owning a runtime. The
> lowest-numbered-context rule above is unaffected; a producer is not
> an escape hatch an author reaches for, and nothing about Context 2's
> capability boundary changes (a producer does not mutate the tree
> either).
> **Window relation: 0504 §3 row 45 is non-window (D2).** A null
> delta to §4.1 changes no freeze-relevant surface by construction,
> and the producer role LLP 0545 §2 adopted adds no execution
> context, no wire record, and no ABI here — LLP 0506 D1 names no
> surface this row touches.
> The companion halves of row 45 land elsewhere: LLP 0510 §6.3's
> existing carrier binding is the producer carrier path (no new record
> class), and the *authoring* surface LLP 0527 §3.4 sketches was
> explicitly illustrative — its grammar is LLP 0328 D8/Q2's
> adjudication, tracked as 0504 §3 row 36's residue, so no LLP 0508
> amendment is owed for it.

### 4.2 Context 3: the app runtime on a dedicated thread

Inherited from the predecessor RFC without restatement: **B1** (dedicated
named thread with explicit run loop, not GCD; QoS `.userInitiated`), **B2**
(async-only cross-queue contract; no `runSync`; `callModuleSync` audit and
migration), **B4** (double-buffered opcode ring, vsync-aligned consumption;
reverse input-event queue), **B5** (platform-constraint fast path), **B6**
(callback affinity table as a tracked artifact), **B7** (`@MainActor`
migration; optional `@RuntimeActor`), **B8** (wake-driven executor replacing
the 16ms poll), **B9** (debug queue assertions, warnings → crashes).

Four amendments — A1–A3 from re-verifying Initiative B against today's
code, A4 from the round-2 reviews:

- **A1 — The runtime thread owns the kernel; main consumes the host
  data-plane feed.** The kernel has no internal locking and keeps exactly
  one owner: the runtime thread. JS-side FFI reads stay synchronous and
  cheap because they share the kernel's thread. The main thread's kernel
  surface — which is far broader than a "snapshot rebuild" (Current state
  item 3) — is replaced by a **runtime-produced host data-plane feed**
  published alongside each commit and consumed lock-free on main. The feed
  must carry, at minimum: per-node records (type, parent/children, layout
  frame, style snapshot, text content, string props), node transitions
  (structured data consumed for main-side animation,
  `ExactEngine.swift:1830`), the merged accessibility/semantics tree, the
  event-handler routing table, the **selection document model** — main's
  selection surface is a set of interaction-time *computations*
  (`selectionDocumentTotalPositions`, `projectSelectionRange`,
  `selectionDocumentRangeForViewRange`, `constrainSelectionRange`,
  `selectionText`; `ExactView.swift:67-88`), so the feed carries per-node
  document-position structure and text sufficient to run them locally on
  main, with W4a deciding per operation where local computation is not
  worth the feed weight and an async hop is acceptable — and explicit
  node-removal/native-view-disposal records (replacing the
  prune-during-sync pass, loop at `ExactEngine.swift:1851-1862`). Module
  registration
  changes shape too: `registerModule` synchronously returns a `moduleId`
  consumed immediately (`ModuleRegistry.swift:116,172`), so ids are
  pre-allocated main-side from a reserved range (or module bootstrap moves
  to the runtime thread) rather than round-tripping; state-mirror slice
  pointers handed to modules remain raw seqlock-safe shared-memory reads
  and are unaffected. The feed is root-scoped (multi-window), diffable,
  versioned by commit generation so agent and a11y consumers can state
  their freshness, and its schema is a tracked artifact
  (`docs/host-data-plane-feed.md`, B6-style: authored in W4a, reviewed by
  every PR that extends it). Main-initiated
  kernel *writes* (font-registry remeasure, secondary-window viewport)
  become messages on the input/constraint queues. Building this feed and
  routing `ExactEngine`/`ExactView`/`ModuleRegistry` through it — while
  everything still runs on one thread — is its own pre-flip workstream
  (W4a) with a mechanical exit gate: a B9-style debug assertion that **no
  kernel FFI is entered outside the tree-domain execution scope**. The
  scope is **ambient over the runtime executor**: the W1 executor
  enters/exits it around every `ex_hermes_*` call, which covers all JS
  execution — poll-drained callbacks, the sync event-dispatch path,
  `evaluateString`/run-js — and therefore the C++ JSI host functions
  (`exact.getLayout` and friends) that hit kernel FFI without passing
  through any Swift driver; the dispatch/commit/layout drivers assert the
  same scope on the non-JS kernel paths. The scope nests (a counter, not a
  boolean — sync event dispatch can occur inside a commit while
  single-threaded), and kernel creation transfers ownership into the scope
  at boot, giving the post-flip degeneration to a plain thread check in
  `kernel/src/ffi.rs` a defined owner-handoff point. (A thread-identity
  assertion is unsatisfiable during W4a — the runtime's own kernel calls
  still legitimately run on main until the flip.) The gate covers FFI
  entry; handed-out state-mirror slices are exempt by design, being
  seqlock-safe raw reads. After W4a, the W4b flip is a pure scheduling
  change.
- **A2 — Text measurement moves with layout; the work is validation and
  invalidation ordering, not a port.** The measurement core is already
  CoreText — `CTFramesetterCreateWithAttributedString` /
  `CTFramesetterSuggestFrameSizeWithConstraints`
  (`PlatformTypes.swift:952,987-998`) — behind an `NSLock`-guarded
  measurement cache and font registry. Remaining work: (a) validate
  `UIFont`/`NSFont` descriptor construction off-main (documented
  thread-safe; verify our paths, including fallback and emoji shaping);
  (b) solve the cross-thread invalidation race — font registration on main
  invalidates the measurement cache (`PlatformTypes.swift:269,282`) while a
  runtime-thread Taffy pass may be mid-measure, and
  `remeasureTextForFontRegistryChange` becomes a cross-thread scheduled
  message (ENG-22728's invalidate-and-relayout flow rides the constraint
  queue). Both are W4a exit criteria — measurement must be proven off-main
  **before** any flip, not after. The speculative async cache remains the
  fallback, not the plan.
- **A3 — Text input becomes a native-owned contract; the preflight does not
  migrate, it retires.** Today's `preflightTextInputTransaction` is not a
  filter: it synchronously dispatches the change event into **arbitrary app
  JS** and accepts the keystroke only if the app's state round trip lands
  the proposed text back on the node (`ExactView.swift:498,509-511`) —
  controlled-component semantics with a synchronous veto. That logic cannot
  run in a restricted worklet, and making it async would break its
  semantics. The migration is therefore an **ecosystem-visible input
  contract change**, stated here explicitly: native-owned text editing with
  versioned async reconciliation becomes the base contract (the spec's
  Layer 1, `EXACT_UNIFIED_SPEC.md:512-517`), synchronous app-JS keystroke
  veto is **removed as a capability**, and synchronous behavior is offered
  only through Layer-2 worklet filters (§4.3). Apps relying on
  controlled-input round trips get a migration guide and a deprecation
  window (W3). This is the right trade — it is how every off-main-thread
  UI framework works — but it is a contract change, not a refactor, and
  the RFC owns saying so. The migration rows (the W3 guide expands each):

  | Today's controlled-input use | Replacement |
  |---|---|
  | Read-only / conditionally editable field | native `editable` prop (a W3 addition) — steady-state enforcement is native and airtight; transition-window edits (typed before an async disable lands) resolve via reconciliation revert |
  | Max length / charset masks | filter worklet `reject`/`replace`; on secure fields, native `maxLength`/charset props instead (next row) |
  | Formatting (currency, phone, …) | filter worklet `replace`; locale-aware helpers are part of the worklet stdlib surface (W2) |
  | Secure fields | content filtering is unavailable **by design**: secure transactions are digest-only (`ExactRenderSurface.swift:1521-1526`), and a digest supports equality checks, not inspection or `replace` — length/charset limits become native props enforced in the control; the W2 matrix records what, if anything, a digest-only filter can still legitimately do |
  | Cross-field / app-state-dependent veto | mirror the condition into a SharedValue the filter reads, or accept the async revert — both patterns named in the guide |

  Deprecation mechanics: during the W3 window the preflight path keeps
  working but emits a dev-overlay warning and a `/agent/diagnostics`
  counter naming the field and handler — preflight-dependent apps are
  discoverable, never silently broken.

  *W3 phase-1 status (ENG-22782):* the guide exists at
  `docs/text-input-migration.md`; the native `editable` prop (PropId 65)
  ships end-to-end (encoder → kernel string prop → AppKit/UIKit controls +
  preflight early-reject, dom-mirror `readOnly`); the deprecation
  instrumentation is live — per-field `deprecatedTextInputPreflight`
  counters on `/agent/diagnostics` plus a once-per-field dev warning for
  controlled inputs.

  *Async-revert convergence fix (ENG-23550, post-LLP-0322 flip):* the
  retired veto (ENG-22901) left "async revert" without a trigger when the
  app rejects a keystroke *without changing its controlled value* — no
  text op flows back, and the session store's stale-echo heuristic cannot
  distinguish "app JS hasn't seen the keystroke yet" from "app JS saw it
  and said no", so rejected characters stuck in the field. Fixed with an
  observed-revision watermark: the change dispatch carries the committed
  session revision, the renderer acks it after the app's flush turn
  (`textInput.ackObservedRevision`, async host call, fire-and-forget), and
  once `observedRenderRevision` reaches `lastDispatchedNativeRevision`, a
  still-divergent controlled value is a deliberate rejection — the engine
  applies it back into the field (`remote-update-applied`). Both
  watermarks ratchet forward only, so the mechanism self-heals under any
  interleaving; fast typing coalesces to a single revert on the final ack.
  Uncontrolled observer fields never revert — their ack instead syncs the
  engine's JS-text mirror to the native truth.
- **A4 — B4's producer-blocking back-pressure must not compose with any
  main-side wait (r3).** B4 as written blocks the *producer* (runtime
  thread) when the opcode back buffer fills, until main swaps
  (`hermes-runtime-queue-and-agent-reliability.rfc.md:526-528`). Composed
  with any main-waits-on-runtime step, that is a bounded cycle: main waits
  for a commit the runtime thread cannot finish because the ring is full
  and main is not swapping — degrading the §4.4 carve-out to guaranteed
  fallback under contention and falsifying its one-directionality claim.
  Amendment: any sanctioned main-side wait on runtime output (the §4.4
  sanctioned set: the live-resize mechanism and, since 2026-08-23, the RFC 0540 §3 visible-content hold (LLP 0543 §11, added 2026-08-23 at RFC 0540's acceptance)) is a **servicing wait in
  drain-to-staging form** — while waiting for commit generation ≥ N, main
  *consumes* the front buffer by copying its opcodes into a staging batch
  (no presenter application mid-wait), swaps, and repeats as needed; at
  wait end it applies the staged batch plus the arrived commit in order,
  once. Why exactly this form: B4's two-buffer protocol requires the front
  buffer to be fully consumed before it can be handed back to the producer
  — a naive swap-only wait would, on a second fill, return a buffer still
  holding unconsumed opcodes and corrupt commit ordering, so
  consume-before-swap is forced; and *applying* mid-wait would mutate
  platform views while the event-tracking resize loop is on the stack, a
  presenter-reentrancy hazard LLP 0161 already documents. Drain-to-staging
  keeps the producer permanently unblocked through any number of fills,
  applies nothing reentrantly, and preserves 0161's one-apply-per-tick
  sequence. B9 asserts that main-side waits are always this form.
  (Rejected: a never-blocking overflow ring — a larger change to B4 than
  the composition problem requires; full mid-wait application —
  reentrancy-unproven and unnecessary.)

### 4.3 Context 2: the UI worklet runtime

**Identity and lifecycle.** Exactly one instance per process. Created at
boot alongside the app runtime (its ~ms-scale creation cost is paid once,
off the critical path), preloaded once with the frozen worklet standard
library, resident for the process lifetime. The current implementation
evaluates that fixed prelude from source at boot. "Warm" is a property of
lifetime, not of pooling. (Reload/reset interaction: §4.8.)

**Ownership.** The main/UI thread is the single owner, preserving the
one-owner-per-Hermes-instance invariant. All invocations originate on main:
Motion-domain recognizers and drivers (Context 1) call into it when a
binding names a worklet; platform text-input plumbing calls filter worklets;
the display-link driver calls frame-callback worklets.

**Restricted environment.** No module loader, no `fetch`/network, no timers,
no `exact.dispatch`, no direct kernel access. The global surface is:

- math and a small frozen stdlib;
- SharedValue get/set;
- read access to the invoking event payload (gesture state, proposed text
  transaction incl. selection/marked-range/revision, frame timing);
- `measure(nodeId)` — synchronous node geometry read against the
  **main-side presenter snapshot** (never the kernel), the same source
  `ExactHitTester` already uses (`ExactView.swift:416-418`); gesture
  worklets need container bounds for clamping, and this keeps the read
  same-thread and lock-free;
- `log(...)` — buffered, delivered asynchronously to the console/agent log
  stream (worklets must be debuggable from day one);
- `scheduleOnAppRuntime(fn, args)` — the async-only escape to Context 3
  (Reanimated's `runOnJS` equivalent).

Worklets influence the tree exclusively by writing SharedValues that
declared bindings map to properties, plus the synchronous verdict for
filter-style invocations.

**Input-filter contract.** A filter worklet receives the proposed
transaction and returns a **transaction transform**, not a boolean:
`{action: "accept" | "reject" | "replace", text?, selection?}` — `replace`
is required for the spec's masking/formatting commitments
(`EXACT_UNIFIED_SPEC.md:522-536`). IME rules: composing transactions pass
through unfiltered (matching today's preflight skip, `ExactView.swift:469`,
and the spec's `isComposing` accept); the filter runs on composition
commit; dictation/autocorrect arrive as range replacements and are filtered
as such; secure-entry fields interact with the digest-only transaction
plumbing (`ExactTextInputTraits.isSecure`). The full IME × secure-entry ×
dictation matrix is a W2 exit artifact. **Failure policy is per-filter:**
the default is fail-open (accept unmodified, report the error), but a
filter may declare `failClosed` (reject on exception) — a masking filter
guarding sensitive input must not fail into pass-through.

**Cross-runtime rules** (extends the predecessor's Thread-Affinity Rules):

- worklet → app runtime: **async only** (`scheduleOnAppRuntime`).
  *M6 native state (r19, 2026-07-13): production forwarding is wired.
  Fixed-slot `runOnJS(callbackIdentity, ...f32)` records carry source
  identity, per-source sequence, and generation through a 256-record
  drop-oldest ring; compatibility `scheduleOnAppRuntime(name, args)` bytes
  share the async drain. Main copies/drains only; callback execution is
  enqueued on the app-runtime executor and rejected after a generation
  change. Missing app-bundle dispatch registries are defined no-ops.
  Delivery remains observational only and never correctness-bearing (LLP
  0099's plane-inventory note).*
- app runtime → worklet: **install/update only** — `installWorklet(id,
  artifact, capturedConsts, generation)` scheduled onto main (generation
  semantics: §4.8); captured values that must vary at runtime are
  SharedValues, not re-captures. *M6 format decision (r19): `artifact` is
  normatively UTF-8 function-expression source. The typed native installer
  serializes finite scalar/boolean constants and complete SharedValue
  handles, hashes source + captures for stable identity, and invokes through
  caller-owned fixed f32 slots without JSON. The 64-artifact characterization
  measured p50 0.081ms / p95 0.182ms warm install against a 5ms mount-time
  ceiling. HBC remains a future explicit enum case, not an inferred format;
  the current authoring plugin's nested/inline extraction and capture
  classification are still LLP 0099 M6 work.*
- worklet → main-thread platform APIs: forbidden. Worklets compute; the
  Motion domain and presenter apply.
- Nothing anywhere may synchronously block on another runtime. B9's debug
  assertions extend to the worklet runtime. (The sanctioned
  exceptions — the bounded live-resize wait and, since RFC 0540 §3.6
  (2026-08-23), the bounded visible-content hold, both main-on-runtime,
  never the reverse — are defined in §4.4 and instrumented.)

**Budget enforcement.** A worklet runs inside a frame or an input event;
overrun *is* jank by definition. Dev builds instrument every invocation:
warn at >1ms, report p95 per worklet id through `/agent/diagnostics` and the
perf overlay. The worklet heap is capped small (target ≤4MB) so its GC
pauses stay sub-millisecond; W2 validates this empirically and also reports
the **process-RSS delta** of the second runtime, which is the truer cost
(§8).

**Failure classes** (extends the predecessor's failure-class matrix):
`worklet_overrun` (budget exceeded; UI janks but everything keeps running —
observable via diagnostics) and `worklet_wedged` (a non-terminating worklet
freezes the UI thread; dev-mode INTENDS Hermes' async-break/timeout
facility to interrupt and report — that facility's viability is what
OQ1 still owns, and no interruption path is implemented today (r16
honesty note); release builds treat it as the main-thread hang it
is, caught by the Initiative A watchdog). Because the app runtime is on its
own thread, the agent API remains live in both cases — a categorical
improvement over today, where any JS wedge takes the agent down with it.

### 4.4 The two-domain kernel (resolving LLP 0099 × Initiative B)

LLP 0099 places "kernel (Rust) on the UI thread"; Initiative B moves the
kernel to the runtime thread. Both are right about their halves. This RFC
splits the kernel's *runtime residency* (not its crate structure) into:

- **Tree/layout domain** — retained tree, protocol dispatch, Taffy, text
  measure. Lives on the **runtime thread** (A1). Single owner, no locks.
  This domain is the `Kernel` struct.
- **Motion domain** — LLP 0099's gesture recognizers, animation drivers,
  animated bindings, plus worklet invocation. Lives on the **main thread**.
  To be precise about what "lives" means: the Motion domain is a
  **main-owned Rust data structure** — recognizer state, binding tables,
  and a geometry mirror fed by the host data-plane feed — *not* the
  `Kernel` struct, which main never touches after W4a. It is ticked by the
  display link.

Domain communication's DATA planes are lock-free (r18 scoping note:
"lock-free only" governs the per-frame data planes below; the
commit-output CROSSING is the §4.4 drain-to-staging carve-out, shipped
as a typed staging queue under an `NSCondition` — a lock-backed,
condvar-signaled queue by design, which is the carve-out's documented
shape, not a violation of it; its capacity is currently UNBOUNDED
under a stalled main, which is a recorded gap — the bounded
capacity/coalescing policy lands with 0099 M2's transport work, and
the B4 double-buffered-ring language describes the kernel-emission
side, not this crossing): the SharedValue slab
(read from both domains; **written from main only** — the LLP 0099
access-topology amendment, r16: imperative app-runtime writes travel as
commands applied by the Motion domain, so the value plane is
single-writer by construction), the constraint buffer (main → runtime),
the opcode ring + host data-plane feed (runtime → main), and — added by
LLP 0099's plane-inventory amendment (r16) — the **per-root acknowledged
outcome log** (main → runtime lifecycle/terminal records) with its
runtime-owned consumed cursor, the main-owned applied-watermark control
atomics, and the **decision-record control words** (RFC 0100's
`DecisionCell` — enumerated as the sole two-writer record class; the
value plane's single-writer rule is unchanged, and two-writer shared
state is legal ONLY as a record class enumerated here). The input-event
queue (B4/§Summary diagram) completes the picture. The lifetime rules
that make "lock-free only" true:

- **Binding and node lifetime ride the ring in tree order.** Binding
  attach/detach records are emitted with the commits that create/destroy
  their nodes (detach-before-destroy), so the Motion domain never holds a
  binding to a node it hasn't yet been told about or has already been told
  is gone.
- **Stale ids are tolerated, not raced.** Constraint-buffer or SharedValue
  writes naming just-unmounted nodes — or stale-generation slots (§4.8) —
  are defined no-ops on the consumer side.
- **Gesture hit-testing runs against the main-side presenter snapshot** —
  never the kernel tree — extending what `ExactHitTester` does today.

**Latency classes.** Compositor-path properties (transform, opacity) are
applied by the Motion domain directly through the presenter, same frame.
Layout-affecting animated bindings (0099's `value → layout*` arrow) route
through the constraint buffer and are consumed by the runtime-thread layout
pass at its next wake — **one frame of latency**. Honesty note (r2): this is
a **new cost relative to today's all-on-main model**, where everything is
same-frame — not a universal framework property (Flutter's single-threaded
pipeline lays out same-frame). We accept it because it preserves the
no-sync-cross-thread rule for the steady-state path; OQ4 keeps the door open
to a same-frame layout fast path if W4b data demands it. Note also that
layout-class responsiveness shares the runtime thread with app JS — a long
JS task or GC pause delays layout-affecting responses (keyboard, resize,
layout bindings). That is no worse than today, but it corrects the
predecessor's "regardless of JS load" phrasing, which is true only of the
compositor path; the budgeted-poll idea (predecessor Phase 6) is the
candidate mitigation if W4b measurements show it matters (OQ9).

**Live resize — the first sanctioned bounded-wait carve-out** (the set gained the RFC 0540 §3 visible-content hold (LLP 0543 §11, added 2026-08-23 at RFC 0540's acceptance); same A4 form, own B9 registration)**.** LLP 0161's
landed design is a *synchronous* main-thread sequence per resize tick
(flush → layout → snapshot → presenter apply → forced display), and today's
`updateViewportSize` flushes inline on macOS (`ExactEngine.swift:355-421`).
A naive one-frame constraint route would reintroduce exactly the
content-trails-the-border failure 0161 eliminated. Mechanism, preserving
0161's sync-tick-plus-budget-fallback shape across the thread boundary:

1. During `inLiveResize`, main writes the viewport constraint and issues a
   **resize-priority wake** to the runtime thread, then **waits, bounded
   (~4ms budget; LLP 0161 publishes ~2.2ms for host layout publish — a
   pre-AppKit-presenter baseline by 0161's own accounting, so the full
   direct-apply chain is an estimate W4b must measure)**, for the
   resulting layout commit + feed publish. The wait is a **servicing wait
   in drain-to-staging form** (A4): main keeps consuming the opcode ring
   into a staging batch and swapping while it waits — so B4 back-pressure
   can never block the runtime thread on main mid-wait — and applies the
   staged batch plus the resize commit once, in order, at wait end. If the
   runtime executor reports busy (a JS task or GC mid-flight — a wake
   cannot preempt a running task), main skips the wait entirely and
   stretches immediately.
2. On budget hit, main applies the fresh snapshot in the same tick —
   pixel-exact tracking, as today.
3. On budget miss, main falls back to **presenter-side geometry stretch**
   of the last snapshot for this tick; the true layout lands next tick.
   For text-heavy content, stretch is *distortion* (scaled glyphs) —
   acceptable for isolated ticks, gated for streaks: W4b bounds **maximum
   consecutive fallbacks**, not just the aggregate fallback ratio (LLP
   0161's stranded WS6 harness defines the ratio metric; W4b salvages that
   harness as its gate).
4. Drag end forces a settlement commit: a servicing wait with a **50ms**
   budget; on expiry the stretch persists and the true layout applies on
   arrival — no force-applied stale layout, no unbounded beachball.
   (Reverting to the last unstretched snapshot was considered and
   rejected: it presents wrong-sized, clipped content in the new window.
   Stretch persisting past ordinary arrival times means a wedged app
   runtime — that is Initiative A watchdog / §4.8 supervisor territory,
   and the prolonged-stretch case is called out in its diagnostics.)

> **Calibration note (2026-08-04, LLP 0418 / the resize-stretch-policy
> ticket):** the budget-miss fallback in item 3 now defaults to **honest
> content lag** — the last snapshot stays at its own geometry for the
> missed tick (stale wrap, clipped/trailing gap) and the true layout
> applies on arrival. The presenter-side geometry stretch remains
> implemented behind `EXACT_RESIZE_STRETCH=1`. Rationale: on text-heavy
> content the stretch is glyph distortion — the author-identified worst
> artifact of live resize — while optimized-lane wait telemetry on the
> blog (M5 Air Release-datum lane) shows ~96-98% budget hits at a
> ~10.7ms tick, so a missed tick is isolated and the honest lag is a
> single-frame trail. The wait mechanism, budget, and drag-end
> settlement are unchanged; only the miss-tick presentation policy
> flipped. W4b's consecutive-fallback bound now gates lag streaks
> rather than stretch streaks.

Expectation-setting: an app that re-renders on viewport change (the normal
pattern) keeps the runtime thread busy during a drag, so its budget hit
rate will be low and stretch-then-settle is its *designed* behavior, not a
failure. That is a different trade than today, where the same app's JS
response runs inline in the resize tick and manifests as hitching rather
than stretch. The W4b gate therefore runs two fixtures: the static
`resize-sentinel` (budget path must match the 0161 baseline) and a
**JS-responsive variant** (stretch path must hold the consecutive-fallback
bound).

This mechanism — the per-tick wait and the drag-end settlement are two
phases of the one carve-out — is **one-directional and non-cyclic** (main
waits on runtime; the runtime thread never waits on main — drain-to-staging
guarantees ring consumption continues, A4 — and B2's forbidden
bidirectional sync stays forbidden), **bounded**, **scoped to the
host-declared interactive-resize span** (AppKit `inLiveResize`, already
load-bearing in today's path, `ExactDesktopWindowBridge.swift:1669`;
iPadOS has no equivalent public span — §4.7), and **instrumented** (wait
attempts, timeout streaks, queued-main delay; §8). It is the one
sanctioned sync cross-thread wait *mechanism* in the system, serving a
sanctioned *set* of registered customers — {live-resize wait, the RFC 0540 §3 visible-content hold (LLP 0543 §11, added 2026-08-23 at RFC 0540's acceptance)}; B9 asserts
set-membership (no unregistered customer) and the servicing form. If interactive keyboard tracking
(drag-to-dismiss) ever needs the same shape because the B5 fast path
proves insufficient, that is a further *extension* of the sanctioned set — the
same A4 form with its own B9 registration and gate — not a reuse of either
member.

### 4.5 SharedValues: the data plane

LLP 0099 owns SharedValue semantics (types, drivers, binding grammar;
Contract projection per LLP 0281 §1). This RFC fixes their **storage and
access model**: SharedValues are slots in kernel-owned shared memory,
**read** from four places (r26; three before the RFC 0490 M3 fourth-reader
amendment) — the Motion domain (main, Rust), the
worklet runtime (main, via FFI host functions — same thread as Motion, so
no contention between them), the app runtime / tree domain (runtime
thread), and the **GPU uniform sampler** (main; the registered
non-validating sampler class; sampling pinned to the generated `gpu`
clock phase — after `bindingApplication`, before `observability` — for
descriptors registered under the GPU-uniform binding authority
`tests/gpu/gpu-uniform-binding-v1.json`; the copied uniform block is
consumed off-main by the encode executor; a reader ONLY — GPU
completions stay command-shaped per RFC 0490 §3.4.4 and never write the
slab; the fourth reader is the amendment Accepted RFC 0490 §6 owed this
document, named by RFC 0115 §3 Tier 1 ratification (b), landed r26 with
RFC 0490 M3) — and **written from main only** (r16, adopting LLP 0099's
access-topology amendment: imperative app-runtime writes are serialized
as commands the Motion domain applies at crossing-consume; wait-free
synchronous reads are unchanged; the sole two-writer shared state is
the decision-record control class enumerated in §4.4, deliberately
outside this slab, so the slab rule is exception-free by
construction). Synchronization is **per-slot atomics** for single-word per-frame
values (r17, adopting LLP 0099's ledger refinement (a): the **tagged `AtomicU64`** — generation+value in one word — superseding the earlier `AtomicF32` spike shape; exported C accessors replace the raw-pointer worklet binding) and the **seqlock protocol** — the
state mirror's existing primitive (`kernel/src/modules/state_mirror.rs:3`)
— for multi-word records. (r2 correction: earlier drafts called this
"double-buffered"; the double-buffer machinery was removed by LLP 0159.)

Ownership boundary, stated once: **W2 of this RFC builds the storage spike**
(slab, atomics, FFI accessors); **LLP 0099 owns the durable registry,
driver, and binding model on top of it**, including slot allocation/reclaim
(OQ2). The threading-facing lifetime invariants — generation/tombstone
semantics for slots, detach-before-destroy ordering (§4.4) — are *this*
document's normative content. (Historical: this sentence named them
"W4b blockers" — W4b then shipped on the spike slab with these
invariants explicitly DEFERRED to LLP 0099 M1; the deferral is
recorded here (r17) rather than left contradicting W4b's shipped
status.) *Shipped-state boundary
(r16): the W2 spike as shipped is a fixed 64-slot `Vec<AtomicU32>` with
a generation-less setter and a raw-pointer worklet binding — no
allocation generations, tombstones, epochs, reclaim, or reset sweep
exist in code yet; those invariants are contract language that LLP
0099 M1 implements (its §Registry is the implementing design), and the
raw-pointer binding is **superseded substrate** M1 replaces with
exported accessors.*

This is the structural advantage over Reanimated: where RN needed JSI
"shareables" to smuggle state between two JS heaps, Exact already has a
neutral, non-JS home for cross-context state — the kernel. No object
serialization crosses runtimes; worklet captures are constants at install
time plus SharedValue references at run time.

### 4.6 Worklet authoring and compilation

- **React/TS tier:** the `'worklet'` directive as specified
  (`EXACT_UNIFIED_SPEC.md:528`), extracted at build time by the bundler
  plugin as a UTF-8 function-expression source artifact, installed with its
  statically classified capture vector at mount. This source format is the
  r19/LLP 0099 M6 decision; nested/inline extraction and capture diagnostics
  remain the authoring-side implementation needed to satisfy the full rule.
- **Contract tier (Contract-first per LLP 0160 §5.2):** no directive.
  Contract's edge here is **provable eligibility with an explained
  fallback** — not that auto-workletization is unique (Reanimated 3's Babel
  plugin auto-workletizes callbacks in known API positions); it is that
  Contract can *decide soundly and say why*. Eligibility is a real
  static-analysis problem, narrower than "has a `writes` list": every write
  **and every read** must be SharedValue/transient-lane-backed (ordinary
  Contract `state` is reconciler state and disqualifies), every call must
  resolve to the worklet stdlib, and expression forms are constrained by
  **worklet class**: math-class worklets (per-frame gesture/animation
  logic) keep LLP 0099's no-strings rule as a GC-pressure policy inside
  the small heap — template strings are string ops and disqualify — while
  filter-class worklets (per-keystroke input filters) get the string
  stdlib, including the locale-aware helpers A3 requires. (0099's
  no-strings rule was written for its kernel register-machine VM, which
  §6 retires; it survives here re-scoped to the math class.) The compiler emits eligible actions for
  Context 2 and, for ineligible ones, a diagnostic naming the first
  disqualifying read/call/form. Rules and the IR→HBC pipeline are OQ6.

### 4.7 Platform scope

- **iOS/macOS:** full model as diagrammed. The two Apple hosts share
  `ExactRuntimeEngine` and share one code path / one flag; default-on is
  evaluated and staged **per platform** (W4b) since the exit gates differ
  (macOS: live-resize fixtures; iOS: scroll + IME matrix). iPadOS Stage
  Manager resizes windows interactively with the same
  content-trails-border physics as macOS, **but UIKit exposes no public
  equivalent of `inLiveResize`** — the system delivers discrete size
  commits and scales the app's presentation between them itself. Whether
  the app ever receives per-frame interactive resize (making the §4.4
  carve-out applicable behind a heuristic span, e.g.
  geometry-update-burst bracketing) is a **W4b verification item**: if
  the platform doesn't surface it, iPadOS is B5-only and the carve-out
  stays macOS-scoped. The iOS gate's Stage Manager fixture verifies
  whichever branch holds.
- **Windows:** same model via the same executor seam; the Win32 UI thread
  plays "main." Sequenced after Apple ships (LLP 0120's standing note).
- **Android (LLP 0287, in progress):** must adopt the executor seam and
  affinity table from its **first** runtime integration — it is the one
  host that can skip the migration entirely by never acquiring the
  main-thread assumption.
- **ibex CLI:** unchanged. Single-threaded tokio is correct for a headless
  runtime; it simply implements the executor trait trivially.
- **TUI (`exact-host-tui`, LLP 0368):** the bounded §4.9 profile applies
  from process start. It is a renderer host with platform input,
  presentation, reload, and teardown duties; the headless-CLI exception
  above does not extend to it.
- **Web:** out of scope. The browser owns threading; the dev-web and
  DOM-mirror paths are unaffected. A future Worker-based split is not this
  RFC.

### 4.8 Lifecycle: reload, reset, teardown (new in r2)

*(r17 status marker: the slot generation/tombstone/reclaim semantics in this section are the CONTRACT — LLP 0099 M1 is the implementing design, and none of it exists in code yet; the shipped spike has no generations at all. The worklet-runtime generation bump and reset ordering ARE shipped.)*

- **Hot reload / engine-generation reset.** Worklets are compiled from app
  code, so an app-runtime reload or generation reset must not leave
  last-generation logic driving live bindings. Every `installWorklet`
  carries the app-runtime **generation**; a reset bumps the generation and
  the worklet runtime atomically drops all stale-generation worklets before
  the new bundle's installs apply. SharedValue slots are generation-fenced
  the same way (allocation records carry the generation; reset reclaims;
  stale-generation slot writes are no-ops via the §4.4 stale-id rule, which
  closes the window where an in-flight tick writes between reset initiation
  and the drop applying). Invocation entry generation-checks: a recognizer
  or binding naming a worklet whose generation is stale — or whose
  generation's install has not yet arrived — is a defined no-op, and the
  binding holds its last value until the matching install lands. The
  worklet runtime itself — and its stdlib — persists across app-runtime
  resets; only installed app worklets and slots cycle.
- **`evaluateString`, agent `run-js`, Design Mode.** All are Context 3:
  they route through the runtime executor exactly like any other app-JS
  entry (the predecessor's Phase 2 named `evaluateString` explicitly; this
  RFC keeps that). None of them may target the worklet runtime; a dev-only
  diagnostic eval surface for worklets, if ever needed, is separate future
  work.
- **Teardown ordering.** Hermes destroy must run on the owning thread (the
  C++ layer captures `runtime_thread` at creation,
  `hermes_runtime.cc:1412`). Shutdown sequence: quiesce Motion ticks →
  drain and stop the worklet runtime on main → stop the runtime thread's
  executor after a final commit → destroy app runtime on the runtime
  thread → release the kernel. Restart-after-crash is not a clean reset:
  the host supervisor pauses Motion ticks and worklet invocation the moment
  the app runtime is declared dead, discards any partially-installed
  generation, and resumes only after the new generation's install batch
  completes. The doctor/zombie tooling (LLP 0178) learns the two-runtime
  shape.

### 4.9 Terminal product-host profile (LLP 0368)

This is the bounded profile for LLP 0368's local TUI product host. It is
an application of the model above, not a second threading model. It is
normative for `exact-host-tui`, so product implementation cannot acquire a
temporary all-on-main topology and call it a migratable seam. The profile may
grow only through a later LLP 0368 phase decision.

**Topology from process start.** The host child starts with the dedicated
runtime executor enabled. The headless-ibex single-thread exception in §4.7
is CLI-only and does not apply to a renderer host with platform input,
presentation, reload, and teardown duties. There is no TUI flag-off mode and
no single-threaded boot window that performs tree work on the main-role
thread. The child has two long-lived ownership threads:

- the **main-role terminal thread** owns decoded terminal input, the
  presenter/cell-buffer state, host-owned animation cadence, guardian
  channel endpoints, and publication consumption; and
- the **runtime thread** owns Hermes #1, app code and reconciliation, the
  unified protocol decoder, the kernel tree/layout domain, Taffy, and the
  pinned Rust grapheme-width measurement path.

The main-role thread never enters the `Kernel` tree/layout object. Immutable,
generation-stamped layout/semantics/cell-paint inputs cross runtime → main
through the host data-plane publication. Input events and constraints cross
main → runtime through their existing ordered queues. No side synchronously
waits for the other; in particular, terminal resize does **not** opt into the
§4.4 interactive-resize servicing-wait exception.

**Execution-context and kernel-domain matrix.** The numbering below is
exactly §4.1's numbering. "Absent" is a capability decision: an operation
requiring that context is rejected or routed to a present context, never run
on a convenient thread under another name.

| Authority item | TUI product disposition | Owner / crossing |
|---|---|---|
| Context 1 — declarative / constraint | **Present, minimal.** No Tier-1 transitions: `SetTransition` applies final state. No Tier-2 Motion recognizers, drivers, bindings, layout islands, or SharedValue sinks. Terminal viewport resize is Context 1: the guardian's typed size message is admitted on main and enqueued on the existing B5 constraint plane. | Main-role thread admits input/constraints and consumes publications; the runtime thread consumes B5 messages and lays out. |
| Context 2 — UI worklet runtime | **Absent.** No Hermes #2 is created. Every app-endowed operation requiring this absent context fails closed by default; today that is `worklet.*` ids 2100–2107 per the `host-call-operations.json` authority. Each returns typed `unsupported-context(ui-worklet-runtime)`; `worklet.syncMotionArtifacts` may additionally identify the absent `motion-domain`. Input otherwise remains usable without a filter. Product verification records any authored surface that relies on a rejected filter or another absent worklet capability. | Rejection is produced by host-call dispatch on the runtime thread regardless of sync/async transport and is distinguishable from the W4b wrong-transport refusal. It does not hop to main or install a fallback evaluator. |
| Context 3 — app runtime | **Present.** App code, reconciler, module JS, agent endpoints, run-js/eval, and reset-based reload execute on the dedicated runtime thread. | The wake-driven B8 executor owns Hermes #1 and every app-runtime entry. |
| Context 4 — Workers | **Absent.** No agent isolate or background JS worker is created for the product profile. Every app-endowed operation requiring this absent context fails closed by default; today that is `workers.*` ids 2000–2001 per the `host-call-operations.json` authority. Each returns typed `unsupported-context(workers)`. TUI agent endpoints remain Context-3-resident; `EXACT_AGENT_ISOLATE` and `EXACT_WORKER_ISOLATES` are inert on this host and publish a typed diagnostic rather than silently no-oping. Loss of wedge-time agent reachability is an accepted v1 product limitation recorded in verification evidence. | Rejection is constructed on the dedicated runtime thread; work is not borrowed by Context 3 synchronously. |
| Tree/layout kernel domain | **Present.** Unified protocol decode, retained tree, Taffy, semantics/layout production, and pinned grapheme measurement live together on the runtime thread. | Single owner; publishes immutable, generation-complete host data. |
| Motion kernel domain | **Uninstantiated.** No recognizer state, binding table, geometry mirror, SharedValue sink, worklet invocation, layout island, or Motion tick source is created in v1. Every app-endowed operation requiring this absent domain fails closed by default; today that includes `island.*` ids 1350–1356, `motion.*` ids 1400–1404, and `worklet.syncMotionArtifacts` id 2107 per `host-call-operations.json`. The domain boundary is preserved by the absence of any Motion consumer. Host-owned spinner/skeleton cadence is painter state, not a Motion tick. | No owner object exists in v1. Rejections are constructed on the runtime thread. The main-role painter clock neither reads the tree/layout domain nor calls app JS. |

The ENG-23560 filter rejection is intentionally narrower than rejecting
`TextInput`: ordinary editing, focus, change dispatch, cursor movement, and
reconciliation remain Context 1/3 operations. A future TUI UI-worklet
runtime is a v2 design choice, not v1 product scope.

**Guardian process versus host child.** The guardian is outside these four
in-process execution contexts. It exclusively owns the physical TTY and its
termios lifecycle, samples winsize, reads raw input, validates and writes the
serialized paint stream, and restores terminal state after every child exit
or channel failure. It does not own app semantics, layout, focus, event
routing, or cell painting. The sessionless child owns no physical-terminal
descriptor. Its main-role thread decodes guardian input/control frames,
produces semantic input/constraint messages, consumes immutable runtime
publications, and sends serialized paint records to the guardian. A guardian
probe or mode acknowledgement is terminal control-plane state, never app
runtime authority.

**Resize is one closed B5 transaction.** The guardian coalesces physical
resize notification bursts, samples the authoritative terminal size, and
sends a typed `(columns, rows, sequence)` message. The child main-role thread
validates monotonic sequence and zero-size state and stores the latest
authoritative `(columns, rows, sequence)` in a **viewport latch** owned by
the child host, not by an app generation. It then enqueues the root constraint
plus a B5 wake. The runtime may not publish the first generation until the
initial guardian sample has been admitted. The runtime thread consumes that
exact constraint, relayouts, and publishes one generation-stamped
snapshot/buffer. Main may
coalesce superseded unpublished generations but presents only a complete
generation; it never painter-adjusts geometry. Zero columns or rows suspends
layout, painting, and input hit-testing while holding the last complete
generation without destroying the runtime; the next non-zero transaction
performs a full relayout. If the initial admitted sample is zero-size, no
complete generation exists and none is published; this is a suspended boot,
not a failure, and the first non-zero transaction produces the first
generation. The guardian's monotonic size sequence is independent
of app generation; replaying the latch on reload is not rejected as a stale
or duplicate physical notification. Disconnect closes admission and enters
teardown rather than synthesizing a size. Rapid resize, resize-during-reload,
zero→nonzero, and disconnect-mid-transaction are mandatory host tests.

**LLP 0370 W1 pre-layout-coalescing amendment.** Before layout begins, the
runtime may replace a consecutive run of already-enqueued physical resize
constraints with its newest member. The survivor retains its exact immutable
dimensions, guardian sequence, successor app-generation stamp, and correlation
identity; each removed member reaches the terminal disposition `coalesced`.
Coalescing stops at the first admitted app-input command, reload replay,
generation fence, or other transaction, so it cannot reorder geometry across
admitted work. A successor-stamped reload-replay constraint is itself a
non-coalescible barrier and is consumed first; a physical resize admitted
during reload is a later successor message. Initial-sample gating, zero-size
suspension, and the mutable-latch prohibition above remain unchanged.

**Clock ownership.** Hermes timers use the runtime thread's B8 next-deadline
plan and host wake hook; there is no terminal polling timer for app JS. The
main-role thread owns at most one monotonic host-paint clock for
renderer-owned indicators and bounded scroll cadence. It never invokes app
JS, the worklet API, Motion drivers, or tree/layout work, and it stops when
there is no dirty animated cell state. Resize is message-driven, not clocked.

**Reload and teardown.** A compile failure never resets a running generation:
the guardian stays attached and the last good HBC keeps rendering while the
typed error is published through the host/log channel. For an admitted HBC
reload, main first closes new app-input admission for the retiring generation
and stops its host-paint clock. Physical resize messages continue to update
the generation-independent, exclusively main-owned viewport latch even while
app admission is closed. Main copies the current latch into an immutable,
successor-stamped B5 constraint message before scheduling the reload
transaction; it never exposes the mutable latch to the runtime thread. The
runtime executor then performs reset FIFO on its owning thread, drops
stale-generation **app events** and publications, consumes that already
enqueued message as the successor's first root constraint, and remounts. A
newer physical resize received during the transaction updates the latch and
enqueues a later successor-stamped B5 message, so it cannot be lost. The
runtime publishes the successor's first complete generation before main
reopens app-input admission. Host constraints are re-latched and replayed,
never stale-generation-dropped. There is no worklet-install or SharedValue
reset phase in this profile. Because the TUI is runtime-thread-on from process
start, `EXACT_PRECOMPUTED_FIRST_FRAME` follows that effective mode; it may not
introduce a main-thread tree-work boot window.

Teardown is guardian-survivable and ordered: main closes child ingress and
the paint clock; the runtime executor drains already-admitted work through a
final fence, destroys Hermes #1 on the runtime thread, then releases the
tree/layout domain there; main joins that thread and closes the child channel
endpoints. The guardian treats EOF, malformed framing, timeout, signal, and
ordinary exit identically for ownership purposes: stop forwarding input,
reap the child, restore the saved terminal state, and only then exit. Neither
reload nor teardown adds a cross-thread synchronous wait; thread join occurs
only after runtime-owned destruction has completed and no callback can be
admitted. If the runtime cannot reach the final fence within the guardian's
bounded shutdown deadline, the guardian kills the child and restores the TTY;
the child never waits forever in a join while retaining terminal ownership.

**Affinity gate.** `docs/callback-affinity.md` contains prospective rows for
every product-host callback/crossing introduced by this profile. Those rows are
merge-blocking contracts: the implementation replaces prospective site names
with concrete symbols if they differ, and may not add an unclassified
guardian, Hermes, or kernel callback. The product-host implementation adds
an on-demand `exact-verify.json` entry covering its concrete callback sites;
that registered check and the LLP 0368 Phase 1 exit review enforce the table.
This is not a per-push `ciGuard`.

## 5. Why one worklet runtime, not a pool

Considered and rejected: a pool of 2–4 Hermes instances invoked
synchronously from the UI thread.

1. **Concurrency is structurally unusable.** Sync invocation from the UI
   thread means the UI thread is inside the worklet for the duration. One
   thread ⇒ one invocation at a time ⇒ pool members beyond the first are
   idle by construction. AppKit/UIKit have exactly one UI thread regardless
   of window count.
2. **Pooling destroys the warmth it is meant to provide.** The worklet model
   is install-once/invoke-many: installed closures, captured constants,
   SharedValue bindings, and warmed inline caches live in the runtime
   between invocations. Round-robin dispatch means installing every worklet
   into every member and keeping N copies of mutable worklet state coherent
   — reintroducing the cross-runtime coordination problem this layer exists
   to avoid, and turning "which instance served this call" into a source of
   nondeterminism.
3. **The startup cost a pool amortizes doesn't exist here.** The single
   runtime is created once at boot and never torn down; warm invocation on a
   resident Hermes instance is microseconds. There is nothing to pool away.
4. **Memory.** Each instance carries a heap *and* VM overhead; N× that on
   low-end devices for benefit (1) already showed to be zero.

Where a small pool **is** the right shape: background worker isolates —
parallel, async, stateless-per-job JS (data processing, the predecessor
Phase 6 agent isolate). That is W6, explicitly out of scope here, and its
instances are precisely *not* sync-from-UI.

## 6. What this RFC changes in existing documents

Upon acceptance:

- `hermes-runtime-queue-and-agent-reliability.rfc.md` — header gains
  "Initiative B: rollout owned by LLP 0297 (amendments A1–A4)"; its B-series
  sections remain the design of record for the mechanics they specify. Its
  B5 "regardless of JS load" claim is qualified per §4.4 (compositor path
  only), its B4 producer-blocking back-pressure policy is amended per A4
  (drain-to-staging servicing waits), and its blanket B2 "no synchronous
  dispatch between the runtime thread and main thread" rule gains the
  registered exceptions of the §4.4 mechanism's sanctioned set (live resize;
  the RFC 0540 §3 hold since 2026-08-23) — one-directional, bounded,
  servicing form; B2's deadlock-by-construction argument is
  preserved because bidirectional blocking remains impossible.
- LLP 0099 — substrate note: "kernel (Rust) on the UI thread" is refined to
  the two-domain model of LLP 0297 §4.4 (Motion domain = main-owned
  structure, not the `Kernel` struct); Tier-2 semantics unchanged; "the
  worklet VM" = LLP 0297's UI worklet runtime; SharedValue storage
  primitives per LLP 0297 §4.5.
- `EXACT_UNIFIED_SPEC.md` §Threading Model — already banner-marked
  historical (LLP 0151 idea 12); this document becomes the threading
  authority, and `exact-contracts.json` gains a `threading-model` boundary
  row pointing here (one authority per boundary, LLP 0150). The spec's
  §TextInput Layer 1/2 contract is affirmed and its Layer-1-as-base
  consequence (A3) becomes normative.
- LLP 0120 — its "when Initiative B lands" note re-points here.
- LLP 0161 — gains a forward note: the direct-apply contract survives as
  the §4.4 bounded-wait carve-out; its WS6 harness is salvaged as the W4b
  gate.

## 7. Migration plan

Workstreams, each with an exit gate. W0–W1 are cheap and start immediately;
nothing moves threads before W4a completes.

- **W0 — Policy + tracking (days).** File the Linear tickets. Adopt two
  standing rules effective immediately: (a) the public module API is
  **async-first** — no new `callModuleSync` surface without an LLP-logged
  exception; (b) every new runtime callback declares thread affinity in the
  B6 table (`docs/callback-affinity.md`, created here) — unclassified
  callbacks block merge. *Exit: rules in CLAUDE.md §6 + tickets filed.*
- **W1 — Executor seam + audit (predecessor Phase 2; ~1–2 weeks).**
  `ExactRuntimeExecutor` protocol wrapping today's main-queue behavior; all
  `ex_hermes_*` calls routed through it on all hosts (including the
  unlocked stragglers like `setKernelHandle`); B9 assertions as warnings;
  full inventory of sync couplings — the callback surface **and** the
  `__hostCall`/`DispatchSemaphore` sync surface (57 sites in
  `ExactRuntimeEngine.swift` alone), `evaluateString` call sites, agent
  `run-js`, debugger attach, and HMR paths — **each tagged with its
  destination: `async` | `worklet` | `constraint-path` |
  `input-contract-change (A3)`**. *Exit: zero unclassified callbacks;
  counted, tagged inventory; no behavior change.*
- **W2 — Worklet runtime spike (parallel with W1's tail).** Second Hermes
  instance behind a dev flag; SharedValue slab spike (per-slot atomic
  f32/vec2); one real input filter exercising the
  accept/reject/**replace** verdict schema and one gesture worklet driving
  a transform binding; `measure(nodeId)` against the presenter snapshot;
  generation-tagged installs; invocation-budget instrumentation. *Exit:
  filter verdict served synchronously in a demo app; the IME ×
  secure-entry × dictation matrix written and exercised; measured install
  cost, invoke overhead, worklet-GC pause, **and process-RSS delta**
  published; ibex multi-runtime-per-process audit complete (OQ5).*
- **W3 — Sync-boundary migration (predecessor Phase 3).** Every W1-tagged
  coupling moved to its destination; the **text-input contract change
  (A3)** lands here — Layer-1 native-owned editing as base, preflight
  retired behind a deprecation window with a migration guide;
  `@MainActor` closures untangled (B7); assertions upgraded to crashes.
  *Exit: test suite + dev workflows clean under crashing assertions, still
  on main.*
- **W4a — Kernel-free main (new in r2; pre-flip, still single-threaded).**
  Build the host data-plane feed (A1) and route `ExactEngine`, `ExactView`,
  and `ModuleRegistry` through it with zero behavior change; A2 off-main
  measurement validation + font-registry invalidation ordering; Motion-
  domain geometry mirror fed from the feed. *Exit: the A1 owner-scope
  assertion (no kernel FFI outside the tree-domain execution scope) holds
  across the test suite and dev workflows; the feed schema artifact
  (`docs/host-data-plane-feed.md`) is published; feed per-commit cost
  measured against today's `syncNodesFromKernel` pass (OQ10).*
- **W4b — The flip (predecessor Phase 4), Apple first.** Dedicated thread
  (B1), wake-driven executor (B8), opcode ring + input queue (B4),
  constraint fast path (B5), live-resize bounded-wait mechanism (§4.4).
  One flag, both Apple hosts on one code path; **default-on staged per
  platform**. *Exit gates: predecessor Phase 4 ordering-invariant suite
  green; macOS — the salvaged LLP 0161 WS6 harness on three fixtures
  (static, JS-responsive, text-heavy): budget-path parity with baseline,
  consecutive-fallback bound held, wait telemetry recorded; iOS — scroll
  fixture holds 60fps with an artificially busy app runtime, the W2 IME
  matrix passes on device, and the Stage Manager fixture resolves the §4.7
  verification item (the carve-out engages and holds the fallback bounds,
  or iPadOS is confirmed B5-only); agent `/agent/tree` p95 < 500ms under
  main-thread stress.*
- **W5 — Validation + Windows (predecessor Phase 5).** Success-metrics
  table (§8) measured and recorded; Windows follows via the executor seam.
- **W6 — Deferred.** Background worker-isolate pool; agent isolate
  (predecessor Phase 6); budgeted-poll preemption if OQ9 data demands it;
  web Worker split. Out of scope until the model above is shipped and
  stable.

## 8. Success metrics

Inherits the predecessor's table (main-thread busy time, agent-tree latency
under stress, touch-to-glass +≤2ms, keyboard-avoidance parity, idle CPU
<0.5%, diagnostics always-responsive) and adds:

| Metric | Target | Limit |
|---|---|---|
| Worklet invoke overhead (warm, excl. user code) | p95 < 0.25ms | < 0.5ms |
| Input-filter verdict | same keystroke (sync parity with today) | no async hop, ever |
| Gesture worklet → transform on glass | same frame @120Hz (≤8.3ms) | ≤1 frame |
| Arbitrary layout-affecting animated binding (**constraint route**) | next frame | <=2 frames |
| Eligible same-frame layout island (LLP 0313) | same display tick at 120Hz; island layout+apply p95 <=1ms within the v0 gate | <=1 frame, with fallback diagnostic on demotion |
| Layout-class response under app-JS stall | compositor projection and eligible layout islands keep moving with 0 UI-thread frames dropped; arbitrary constraint-route layout may be delayed by the stall (accepted; diagnosed) | compositor/island paths unaffected |
| Live-resize fallback ratio + max consecutive fallbacks (steady drag; static, JS-responsive, and text-heavy fixtures; macOS — plus iPadOS iff the §4.7 span exists) | within LLP 0161-harness budget, parity with baseline | gate blocks default-on |
| Live-resize wait telemetry (attempts, timeout streaks, queued-main delay) | recorded per drag, published via diagnostics | required for the W4b gate |
| Worklet runtime heap | ≤4MB steady state | 8MB |
| Worklet runtime process-RSS delta | measured in W2, budget set from measurement | recorded before W4b |
| UI-thread frames dropped during app-runtime GC / 100ms JS stall (compositor path) | 0 | 0 |
| Cold-start first-commit-to-first-pixel — first frame produced in the boot window, not across the flip (r6; OQ11) | ≥ parity with all-on-main baseline; precomputed-frame path (LLP 0145 item 5) targets first paint far below the JS-eval-bound baseline (native dispatch+layout ~7–11ms vs. ~230ms JS) | measured & recorded, W4b/W5 |

Every metric must be agent-observable (perf overlay + `/agent/diagnostics`);
checks register in `exact-verify.json` per LLP 0150 — no orphan `check:*`
scripts.

## 9. Alternatives considered

- **Initiative B alone (no worklet runtime).** Forces sync verdicts async;
  regresses input filtering and gesture-driven interaction — the flagship
  use cases. Rejected; it also strands the spec's worklet commitments.
- **Worklet pool of 2–4.** §5. Rejected for sync-from-UI; deferred as the
  correct shape for W6 workers.
- **Priority scheduling inside one runtime** (RuntimeScheduler-style): UI
  latency stays coupled to app-JS GC and long tasks; RN built worklets *on
  top of* its scheduler for a reason. Rejected.
- **No JS on the UI thread ever (Flutter model).** Message-passing only;
  well-documented pain for gesture-driven interaction; contradicts the
  spec. The declarative Context 1 gets us Flutter's benefits for the common
  case without giving up the escape hatch. Rejected as a hard rule.
- **A different engine for worklets (QuickJS/JSC micro-VM).** A second
  engine class means a second bytecode toolchain, debugger story, and
  binary. Hermes-everywhere keeps one HBC pipeline. Rejected for v1.
- **Shared-heap concurrency around one Hermes instance.** Not thread-safe;
  predecessor Alternative D. Rejected.
- **Keeping the synchronous controlled-input contract via cross-thread
  wait.** Would make every keystroke a main-blocks-on-runtime wait with
  arbitrary app JS on the other side — unbounded, user-visible, and
  deadlock-adjacent. Rejected in favor of the A3 contract change.

## 10. Open questions

1. **OQ1 — Worklet interruption.** Exact dev-mode mechanism for breaking a
   wedged worklet (Hermes async-break vs. watchdog-and-report only), and
   whether release builds get any preemption at all.
2. **OQ2 — SharedValue slot lifecycle.** Allocation/reclaim protocol across
   unmount, HMR reset, and engine-generation changes; dev-mode leak
   detection. (Owned by LLP 0099's implementation on top of the §4.5
   storage; the generation-fencing invariants in §4.4/§4.8 are this
   document's and are not open.)
3. **OQ3 — Debugger UX.** ✅ **Decided (ENG-22781, W2 exit gate).** Two CDP
   targets (app + worklet runtimes): naming, simultaneous attach, source
   maps for compiled Contract worklets. The worklet runtime does **not**
   inherit Reanimated's years-of-undebuggability mistake — it gets a
   first-class debug target. Decision:
   - **One CDP server, one target per Hermes runtime.** The app runtime
     keeps target id `1` / title `Exact — App` (today's single
     `/json/list` entry, `CDPServer.swift:256-265`). The UI worklet
     runtime is exposed as a **second** target with id `2` / title
     `Exact — UI Worklets`, its own `webSocketDebuggerUrl` on a distinct
     token path so DevTools attaches to the intended Hermes instance,
     DEBUG only. A merged single target multiplexing both VMs is
     **rejected**: DevTools has no model for two VMs behind one target, so
     breakpoints, call stacks, and scope chains would collide.
   - **Simultaneous attach is allowed and safe.** The two runtimes are
     independent Hermes instances with independent debuggers; the ibex
     debugger layer already marshals per-runtime (`debug_mutex` + condvar
     eval, `hermes_runtime_debugger.cc`; inventory §4b), so two DevTools
     fronts attach without cross-talk. Each target's WS carries that
     runtime's own auth token.
   - **Source maps.** Compiled Contract worklets ship a source map beside
     the emitted function-expression HBC; the worklet target advertises it
     so breakpoints land on Contract lines, not the desugared
     `(function(event){…})` form. Until the compiler emits worklet source
     maps (tracked with OQ6), the worklet target debugs at emitted-JS
     level with a documented limitation — not *no* debugging.
   - **Build posture (repo debt policy).** The dual-target CDP
     implementation lands **when the worklet runtime becomes resident**
     (it is currently spike-gated behind `EXACT_WORKLET_SPIKE=1`,
     `ExactWorkletRuntimeSpike.swift`); it is not built speculatively
     against a non-resident runtime. The naming, attach model, and
     source-map contract above are the specification that implementation
     follows. This closes the W2 exit gate: the decision exists before the
     worklet runtime ships resident.
4. **OQ4 — Layout-animated properties.** ✅ **Decided (ENG-22918, LLP 0313
   Accepted).** The default remains the one-frame constraint-buffer route
   (§4.4): arbitrary layout-affecting Motion bindings wake the runtime-thread
   tree/layout domain and apply on the next commit. The accepted same-frame
   upgrade is **layout islands**: bounded UI-thread Taffy sub-passes over
   feed-backed, layout-closed subtrees with SharedValue-only dynamic inputs,
   input-matched settlement, per-slot freshness checks, and conservative
   model-geometry-only accessibility frames during motion. The decision order
   is projection first when presentation can honestly hide relayout, layout
   islands for eligible subtrees, and no widening of the §4.4 servicing-wait
   carve-out beyond live resize without a separate accepted record (RFC 0540
   §3.6, accepted 2026-08-23, is that record for the visible-content hold). The §8
   metric rows now distinguish the arbitrary constraint route from eligible
   same-frame islands.
5. **OQ5 — ibex multi-runtime audit.** Verify per-process globals
   (`g_active_module_id` thread-local, `TEXT_MEASURE_CALLBACK` process
   global, module registry) are correct with two runtimes on two threads;
   W2 exit criterion.
6. **OQ6 — Contract worklet eligibility.** The exact provable-eligibility
   rules (read-side constraints, call resolution, expression forms) and the
   compiler's IR→HBC emission path; interaction with LLP 0281 value
   channels.
7. **OQ7 — Runtime-thread lifecycle on iOS.** Behavior across scene
   backgrounding, `beginBackgroundTask`, and process suspension — both
   runtimes must quiesce cleanly to avoid watchdog kills.
8. **OQ8 — Presented vs model geometry (new in r2).** During a
   Motion-driven transform, agent hit-tests, the semantics tree, and JS
   `exact.getLayout` see model geometry while the screen shows presented
   geometry. Which surfaces need per-frame reconciliation (agent hit-test
   probably; semantics probably not mid-gesture), and where is the
   divergence documented?
9. **OQ9 — Runtime-thread preemption (new in r2).** Do layout-class
   constraint responses need a budgeted-poll/preemption mechanism so a long
   JS task cannot delay keyboard/resize layout indefinitely, or is
   stall-behind-JS accepted and documented? Default: accepted + recorded
   (§8); predecessor Phase 6's budgeted poll is the candidate if data says
   otherwise. Escalation: if the W4b JS-responsive resize fixture cannot
   hold its gate without preemption, OQ9 becomes blocking and the budgeted
   poll lands in W4b rather than W6.
10. **OQ10 — Feed cost (new in r2).** Per-commit cost of the host
    data-plane feed vs today's `syncNodesFromKernel` read pass, measured in
    W4a; informs whether the feed is full-snapshot, dirty-diff, or hybrid.
11. **OQ11 — Cold-start first-frame (new in r6; from LLP 0299; grounded in
    LLP 0145 data).** Not the regression the naive framing suggests, and the
    axis where the sandwich model gives Exact a ceiling above Lynx. A naive
    flip that let the runtime thread produce the *first* commit and hop it to
    main would add a cross-thread hop — but the first frame can be produced
    in the **single-threaded boot window** (A1's kernel ownership-handoff
    point) and painted before the dedicated-thread steady-state loop starts,
    so the hop applies to frame 2+, not frame 1. The flip is therefore
    **neutral to first-frame** when designed for it. The real target is
    beating Lynx, not merely avoiding a regression: Lynx's Instant First-Frame
    Rendering *always* runs JS synchronously at launch to build its element
    tree, whereas Exact's binary protocol + Rust kernel let the first frame's
    tree+layout be **precomputed at build time** and painted with zero app-JS
    eval — the lever LLP 0145 item 5 already names ("present a precomputed
    first app-content frame before JavaScript hydration completes"). The
    measured asymmetry that makes this worth it (LLP 0145, 2026-06-06 release
    HBC lane): native protocol dispatch + Taffy layout is ~7–11ms and Hermes
    creation ~4ms, while the JS module-load/eval path is ~230–246ms — so a
    precomputed frame paints ~two orders of magnitude sooner than the
    JS-bound path, and Hermes VM creation is *not* a cold-start liability
    (contra the RN intuition). Constraints (the same class Lynx's IFR has):
    the first screen must be static or host-preset; device-dependent text
    metrics mean the shipped artifact is the tree+styles with a cheap native
    relayout, not a frozen pixel buffer; and the live runtime must
    **adopt/hydrate** the precomputed tree without a flash — the web SSG path
    already has this adoption model (LLP 0252), which the native path must
    port. Time-to-first-*paint* beats Lynx via the precomputed frame;
    time-to-*interactive* still depends on closing the ~230ms eval gap
    (LLP 0128/0145). Explicitly **not** adopting Lynx's run-the-framework-twice
    / two-trees-reconciled model — Exact moves the same work to build time
    instead (single-owner kernel tree §4.4, JS-light main thread §4.1).
    Default: measure both paths in W4b/W5 (§8 metric); the precomputed-frame
    path is the escalation if the boot-window baseline doesn't reach parity.

## 11. Precedent

React Native's New Architecture + Reanimated/`react-native-worklets`
(converged on exactly one UI worklet runtime; validates the SharedValue and
restricted-environment model — LLP 0099 §References), Flutter (proof that
declarative-first with no UI-thread JS covers most of an app framework, and
of the pain where it doesn't), Chrome (renderer-process isolation as the
liveness pattern the agent API needs). Exact's differences — the Rust
kernel as a neutral shared-state store and Contract's statically-provable
action eligibility — are what let this design be smaller than Reanimated
rather than a re-implementation of it.

**Lynx (r6; full analysis in LLP 0299).** ByteDance's Lynx is the closest
shipping production analog to this design: two persistent JS runtimes with
one (PrimJS, QuickJS-derived) on the UI thread, a restricted UI-thread API
surface, async-default events with an opt-in `main-thread:` sync tier, and
bytecode-first UI-thread scripts — every one a convergence with this RFC,
and the strongest external evidence it is aimed correctly. Lynx diverges on
the **seam**: it runs layout on its render/main thread (insulated from
background app JS) and runs the framework **twice** — a synchronous
main-thread build plus a parallel background render, with the two element
trees reconciled for consistency — to achieve Instant First-Frame Rendering.
Exact instead keeps a **single** kernel tree owned by the runtime thread
(A1), runs layout there with app JS, presents on a JS-light main thread, and
moves cross-context state through the neutral Rust kernel (SharedValues, no
JSON serialization; §4.5) rather than serializing between two JS heaps as
Lynx's JSON-serializable captures do. Net: Exact is structurally cleaner on
single-tree ownership and cross-thread state; Lynx leads on cold-start
first-frame (OQ11); the two agree on the fundamental shape.

**The RN Workers-versus-Worklets exchange (r25; July 2026, public
maintainer statements — not audited source).** A third-party "Workers for
React Native" announcement and the react-native-worklets maintainer's
response corroborate three of this document's standing positions from both
camps of the RN ecosystem at once. (1) **Async-first is convergent:**
neither side offers synchronous cross-thread calls into workers, and the
announcement states there is no plan to add them — independent support for
§5's rejection of sync-from-UI pools and the §6 async-first standing rule.
(2) **Layering is convergent:** both sides agree a persistent runtime with
an event loop plus shared cells is the substrate and a Worker-style API is
expressible above it ("a worklet on a dedicated runtime is effectively a
Web Worker without onMessage") — the relationship this document already
encodes between Context 2, Context 4, and LLP 0099's SharedValues. (3)
**Callback affinity is the load-bearing discipline:** the incumbent's
against-interest admission is that RN native modules deliver events to the
main RN runtime on the JS thread and forward them to workers with copied
payloads, so a busy JS thread starves workers of I/O they themselves
initiated, and that modules widely assume the JS thread and are not
thread-safe — the ecosystem-scale version of the failure
`docs/callback-affinity.md`'s declare-before-merge rule exists to prevent.
The Worklets team reimplemented `fetch` from scratch to escape it. The
resident-worker design consequences (completion routing, bundle-per-runtime
economics, Hermes transferable-ArrayBuffer evidence) are carried by
LLP 0405 §1.4, not here.
