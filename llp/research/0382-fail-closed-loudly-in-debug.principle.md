# LLP 0382: Fail Closed Loudly in Debug

**Type:** Principle
**Status:** Active (curation pass 2026-08-23)
**Systems:** Runtime, Apple Hosts, GPU, Kernel, Ibex boundary, Developer Experience, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-07-22
**Revised:** 2026-07-22 (rev 2 — round-1 review response: per-layer diagnostics build capability replacing the `cfg(debug_assertions)` prescription that is compiled out of Apple Debug lanes; typed safe-field disclosure contract replacing the categorical "naming is not disclosure" claim; bounded nonblocking emission promoted to MUST; governed-class taxonomy; activation conditions and Exact-only scope pending ibex adoption; ledger reframed and marked with retrofit status)
**Related:** LLP 0383 (derived-pin registry — the same week's other recurring tax), LLP 0384 (armed-Debug observability tier — the carrier that makes these diagnostics queryable; the event-shape contract below is shared with its channel C), LLP 0297 (threading contract — the declaration-plus-inventory precedent adoption follows), LLP 0307 (precomputed first frame — context for ledger case 2: that incident was a cross-subsystem implementation gap in the replacement reset, not an LLP 0307 policy defect), ibex LLP 0002 (host embedding ABI — owner of the ibex-side Release/Debug redaction posture), ibex LLP 0023 (Virtual Filesystem Namespace and Path Identity Spec — its §4 governs the operator-terminal arming refusal that may print a host path, and §7.2 owns the error-class ordering / existence-oracle constraints the safe-field contract defers to; path-scoped, so refusal classes it does not cover default to this Principle's contract), `issues/20260720-webgpu-dev-root-binding-gap.md` and `issues/20260722-webgpu-track-live-pixels-gap.md` (on branch `agent/eng-25076-final-integration-v2` until the ENG-25076 merge)

> **Super-refine status (2026-07-22):** rev 2 was reviewed twice by fresh
> Claude Fable 5 Max (READY both rounds) and OpenAI gpt-5.6-sol Ultra (NOT
> READY both rounds); artifacts under `llp/reviews/0382-*`. The loop stopped
> without convergence because the residual concerns are author decisions:
> (1) whether the concrete refusal-event schema/ABI is owned here, in LLP
> 0384, or in a companion Spot Spec the reviewers suggest splitting out;
> (2) the disclosure trust boundary (which fields reach an unauthenticated
> broker vs an operator-only projection). Verified round-2 factual fixes to
> fold on resume: separate *every-occurrence accounting* from *bounded sink
> emission* (Scope table vs bounded-emission MUST currently read as a
> contradiction); relabel case 2's retrofits as provisional instrumentation,
> not conforming implementation; qualify the FinalizationRegistry example by
> ibex gitlink (present at main's `ef5b86a8`, already remedied on the branch
> line). Stays Draft.
>
> **Decisions resolved 2026-07-22** (author): the concrete refusal-event
> schema/ABI moves to a **new companion Spec** this Principle references
> (keeping 0382 the semantic contract); the disclosure boundary is a
> shared-broker (path-free) vs operator-terminal (ibex LLP 0023 §4) tier
> split. Implementation is programmed in
> `issues/20260722-armed-devex-implementation-program.md` (tickets T1/T3).
> Ready to fold these + the verified factual fixes into a rev 3 and resume
> the loop toward Review.

## The rule

**Every governed fail-closed refusal must name itself observably in
diagnostic builds. Shipping builds keep their redaction. A governed refusal
that stays silent in a diagnostic build is a defect, reviewable as such.**

"Name itself" means one bounded diagnostic event at the refusal site
carrying:

- a **stable site ID** (`gpu.canvas.root.closed`,
  `ibex.arming.root-binding.missing` style — dotted, greppable, testable),
- the **refused identity** as a classified, closed-form field (root id,
  operation name, view id, pin name — never free-form paths, payload
  bytes, tokens, or digests-of-secrets), and
- **allowlisted scalar facts** sufficient to distinguish this refusal from
  its neighbors (the failed clause index, the expected-vs-seen generation,
  a boolean per guard condition).

This is a closed, typed contract, not free prose: what may appear in each
field is decided by the enforcing layer, and layers with an explicit
disclosure policy (ibex LLP 0023's operator-terminal and existence-oracle
rules) always win over this principle. Naming *which* check refused is
almost always safe; naming *what the check saw* is a per-boundary
decision.

"Observably" means the event reaches a channel the developer actually has
during that run — today the process log; LLP 0384's channel C makes the
same events queryable through the agent API without changing this
principle.

## Why (the case ledger)

The armed WebGPU bring-up (ENG-25076, 2026-07-20 → 07-22) hit four
diagnostic failures around fail-closed boundaries. Each enforcement was
correct policy; each cost hours because the *diagnosis* channel was
missing or too blunt. (Citations to `issues/…` and the Swift files are on
branch `agent/eng-25076-final-integration-v2` until the ENG-25076 merge;
the ibex citations are on that branch's ibex line, commit `8be164c6`.)

1. **Redacted refusal.** The armed dev-served boot failed with only
   `Module resolution failed (ERR_IBEX_MODULE_RESOLUTION)`. The real
   cause — "authorization context refused arming: host path has no
   authenticated logical-root binding" — was recovered only by patching a
   temporary `eprintln` into ibex's `module_resolution_error_json` and
   reverting it. Diagnosis by patching the enforcement layer is the
   antipattern this principle ends. *(Status: the named escape hatch this
   principle proposes for it is future ibex work; see Adoption.)*
2. **Collapsed multi-cause refusal.** The first live GPU-canvas frame died
   as `Protocol frame rejected; waiting for a clean commit`, behind three
   separate silent guards (the closed-root dispatch route, the
   `trustedRoot.isWellFormed` guard, the host registry's `stage()`
   refusals). Three instrument-rebuild-relaunch cycles later, a one-line
   diagnostic (`dispatch rejected: GPU canvas root 0 is closed`) localized
   a real cross-subsystem bug — the LLP 0307 replacement reset closing the
   incoming generation's negotiated root. *(Status: these three sites were
   retrofitted with `#if DEBUG` diagnostics in the same change; they are
   the worked example, not outstanding work.)*
3. **Unrelated work sheltered behind a silent policy gate.**
   `setDispatchHandler`'s early return under `exactAmbientIngressAdmitted`
   silently skipped not just the gated ambient installation but also
   handler storage and the viewport push — a behavior bug hiding inside a
   correct, intentionally silent gate. Silent guards conceal their
   neighbors' bugs, not only their own refusals. *(Status: fixed on the
   branch; the gate itself is a deliberate closure and under the taxonomy
   below emits once per runtime, not per call.)*
4. **Insufficiently specific refusal.** The arming failure "provider or
   checked private registry is unavailable" *did* log — but could not say
   which condition (in that incident, which of the two stale pins) failed.
   Loudness includes specificity: distinguishable causes get
   distinguishable events.

An adjacent lesson, deliberately *outside* this rule: ibex's
`FinalizationRegistry` stub feature-detects and never fires — quiet
capability degradation, not a fail-closed refusal. Its remedy is honest
capability absence (LLP 0115's disposition), governed by the
graceful-degradation norms, not by this principle.

## Scope: what is governed

A **governed refusal** is a fail-closed decision on a security, integrity,
or admission boundary. Classes and their expected emission:

| Class | Examples | Emission in diagnostic builds |
| --- | --- | --- |
| Policy denial | admission refused, unendowed operation, unauthenticated root | every occurrence (bounded per site) |
| Integrity terminal | `.fatal` validation outcomes, quarantine, generation reset | every occurrence — these are the hardest to diagnose and are explicitly in scope |
| Staleness/replay refusal | stale generation, replayed call id, superseded producer | first occurrence per (site, generation); repeats counted, not printed |
| Deliberate standing closure | a channel intentionally not installed (case 3's gate) | once per runtime with the site ID; per-call silence is correct |

Explicitly **not** governed: expected capability absence on a platform
(graceful degradation stays quiet by design), cooperative cancellation,
and ordinary teardown/shutdown ordering. When a reviewer cannot tell which
class a guard is, the guard's site annotation (below) says so — the
classification is written down, not inferred.

## Norms

- **MUST (coverage):** a new governed refusal site, or an existing one
  materially modified by a change, lands with its diagnostic event and a
  site ID. Reviewers block on its absence exactly as they block on a
  missing callback-affinity declaration (LLP 0297 precedent).
- **MUST (diagnostic-build capability, per layer):** the diagnostics are
  compiled under an explicit per-layer capability, not an assumed
  optimization level: Swift uses `#if DEBUG`; **Rust and ibex archives use
  a dedicated cargo feature (working name `refusal-diagnostics`) that the
  Debug build lanes enable** — `ios/build-kernel.sh` builds those archives
  `--release` even for Debug app configurations, so `cfg(debug_assertions)`
  is compiled out exactly where the diagnostics are needed and MUST NOT be
  the gate. Shipping/Release artifacts exclude the capability; a positive
  diagnostic-lane test and a negative Release-artifact test prove both
  directions once the capability exists.
- **MUST (shipping posture):** shipping builds' *observable refusal
  behavior and redaction* are unchanged by this principle — no new
  messages, channels, fields, or timing-relevant work in Release. (This
  replaces rev 1's "byte-identical" phrasing with the invariant that is
  actually testable across relinked artifacts.)
- **MUST (bounded, nonblocking emission):** on hot paths or paths fed by
  untrusted input, emission is bounded and nonblocking — a bounded key
  space, first-occurrence or token-bucket behavior per site, saturating
  occurrence/loss counters, and the rule that sink failure or saturation
  can never change the enforcement result or block the enforcing thread.
  (Precedent: ibex's bounded nonblocking host log queue; the agent log
  store's cap-and-count discipline.)
- **MUST (disclosure):** events carry only the typed safe fields above.
  Free-form paths, payload bytes, tokens, and secret-derived values are
  prohibited in every build. Where even the site-level distinction is
  oracle-sensitive (ibex LLP 0023 §7.2), the enforcing layer's policy
  wins, and the deep-diagnosis escape hatch is an explicitly named,
  documented gate **owned by that layer and compiled inside the same
  diagnostics capability** — never a patch-and-revert, never present in
  shipping artifacts.
- **SHOULD (specificity):** distinguishable causes get distinguishable
  events. A refusal with N guard clauses either reports which clause
  failed or is split until it can (case 2's `stage()` retrofit — one line
  carrying `registered`/`identityMatch`/`closed` bits — is the worked
  example).
- **SHOULD (message shape):** the human-readable line follows
  `[<Subsystem>] <site-id>: <facts>`; the structured event (site ID +
  fields) is the contract, the prose is a projection of it. LLP 0384's
  channel C consumes the structured event as `category=refusal` — no new
  log level.

## Adoption and activation

- **Scope:** this Principle binds **Exact-owned code** on landing. The
  ibex side (module resolution, arming, CapSec refusals) adopts via an
  ibex LLP amendment (LLP 0002 and/or LLP 0023, which owns disclosure
  there); until that lands, ibex sites are worklist items, not review
  blockers.
- **Carrier:** one canonical instruction in the agent/reviewer guidance
  (CLAUDE.md §6-style, as LLP 0297 did), plus site annotations at governed
  guards. Coverage is checked against annotations, not inferred from
  grepping `guard`/`return .rejected` shapes (which mis-fires both ways).
- **Conformance:** when the first annotated sites land, a registered
  verify check (governance profile, per the no-orphan-`check:*` rule)
  asserts annotated sites compile their diagnostics in the diagnostic
  lanes. Site IDs — not English message text — are the stable assertion
  surface for tests.
- **Activation:** the Principle stays `Draft`/prospective until (a) the
  diagnostics capability exists in the Rust/ibex build lanes, (b) the
  carrier instruction is landed, and (c) the first retrofit wave (ledger
  items and their nearest neighbors, including the currently silent
  `.fatal` paths in the GPU staging validator) is annotated. Flipping to
  `Active` with an effective date is the author's call once those hold.

## Non-goals

- Not a weakening of any enforcement, posture, or shipping redaction.
- Not a general logging framework, log-level system, or tracing design.
- Not a mandate over graceful degradation, cancellation, or teardown
  silence (see Scope).
- Not retroactive: existing guards are governed when touched or when the
  retrofit wave reaches them.

## Open questions

1. Should the typed emission primitive (`refuse_debug!(site_id, …)` in
   Rust, a Swift analogue) land as part of the first retrofit wave, giving
   tests and LLP 0384's ring one structured producer from day one? (Both
   round-1 reviews suggest yes; it is implementation, not principle, so it
   is left to the first implementing change.)
2. Does the site-ID namespace need a registry file, or are annotations
   plus the conformance check enough until the count grows? (Start with
   annotations; revisit at ~50 sites.)
3. Which existing intentionally-silent closures (beyond case 3's gate)
   deserve the once-per-runtime emission, and which are better left in the
   not-governed column? The retrofit wave's review answers this per site.
