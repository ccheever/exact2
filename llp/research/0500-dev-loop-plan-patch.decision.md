# LLP 0500: Dev-Loop Direction — the plan patch is the HMR unit; dev runs the production runner

**Type:** Decision
**Status:** Active (decided by Charlie Cheever, 2026-08-20)
**Systems:** Dev loop / HMR, Contract (compiler/runtime), Web, Rust Native, kernel, Devtools, Design Mode, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-20
**Related:** LLP 0417 (native source HMR, Accepted — the ExecutionGeneration/HotRevision algebra D1 adopts as the single spine; its counter-unification open item resolves onto D1), RFC 0086 (`llp/contract/` — refresh and state-preserving HMR, Draft: the identity condition R1 carries into the plan format), LLP 0485 + `issues/20260820-llp0485-track-r-plan-format.md` (Track R's gate satisfied 2026-08-20; plan format v1 starting — the window R1 must land inside) + `issues/20260820-llp0485-plan-format-identity-stability-hmr.md` (the R1 ask, filed with this decision), RFC 0480 (OQ: "is the plan editable enough for Design Mode" — answered in direction by D1), LLP 0483 (web on the flat plan — the fixed runner D2 puts in the dev loop), RFC 0498 (Aquifer — identity-keyed cells are the state-preservation mechanism: state-preserving HMR and restoration-as-replay are the same replay), RFC 0401 (canonical identity family — component identity under patches), LLP 0413 (parse-free startup; its phase-4 HMR generation join lands on D1's spine), LLP 0419 (evidence-lane candidate transport, known-broken — the patches-with-receipts direction for embedded reload), RFC 0402 §OQ (recipe HMR is full-reset today), RFC 0492 / RFC 0494 / RFC 0497 (motion segments, manifest, recipes — plan segments whose edit stories D1 answers uniformly), RFC 0495 (revision receipts join the causal evidence stream), LLP 0400 (Design Mode — live edits as overlay patches on the same channel)

## Context

The dev loop is five reload systems with five version concepts and no
shared spine: web dev (Vite + Contract's reset-based HMR — every edit
loses state), native JS-source HMR (LLP 0417, Accepted, with a mature
generation/revision algebra but four unreconciled counters), the
parse-free startup path (LLP 0413, its HMR generation join open),
embedded/release reload (M3 + LLP 0419, transport known-broken), and
the language-side intent (RFC 0086: Contract must not be "boxed into
reset-only development forever" — unimplemented). Dev additionally
renders through a different path than production on web (DOM mirror vs
fixed runner), a divergence that has repeatedly shipped
production-only breakage.

Meanwhile every refresh program (RFC 0492/0494/0497/0498) deferred its
hot-reload question, and RFC 0480 left "is the plan editable enough
for Design Mode" open. The flat plan changes the premise: reset-based
HMR exists because a tree-walking runtime cannot diff itself — **a
plan is diffable data**.

## Decision

**D1 — The plan patch is the HMR unit, and LLP 0417's generation
algebra is the single spine.** An edit compiles to a plan patch — a
diff of segments (view, recipe, motion, manifest, capability
declarations) — applied as a **HotRevision** within an
**ExecutionGeneration**, with a 0417-style fail-closed refusal
vocabulary (identity-incompatible or effect-unsafe changes refuse into
coherent full reload). All reload systems converge on this one spine:
0417's four counters unify onto it, 0413's generation join lands on
it, embedded reload (0419's repair) carries plan patches with
receipts, and every revision emits a receipt into the RFC 0495 causal
stream — an edit session becomes replayable evidence. State survives
by identity: a patch that does not change a component's identity
(RFC 0401) or a cell's identity (RFC 0498 — **matched on the LLP 0485
§3.3.16 declaration identity**; the evaluated dependency tuple feeds
the runtime cache key's canonical arguments and is not the patch key)
preserves its state, making state-preserving HMR and
restoration-as-replay the same mechanism and landing RFC 0086's goal
as a corollary rather than a machine. Matching is never sufficiency:
the runner recomputes the runtime cell key after every patch and
reuses canonical cell state only on exact runtime-key equality (RFC
0498 §4.1's reuse gate — a same-stamp key change exposes at most the
per-subscriber projection; a partition-axis change exposes nothing).
*(Amended 2026-08-20, decided by Charlie with RFC 0498 r6: the
original parenthetical read "declaration + dependency tuple", which
would have discarded cell state on every dependency-expression edit.)*

**D2 — Dev runs the production runner.** The web dev loop runs the
fixed runner (LLP 0483) with a patch channel; the native dev loop runs
the Rust plan runner. "Dev mode" is *runner + patch channel + agent
surface* — never a different renderer. The DOM-mirror-vs-production
divergence class is retired with the mirror. Design Mode edits ride
the same patch channel as file-save HMR (a knob edit is an overlay
patch; bake persists it to source), so the human editor, agent-driven
editing, and the file loop share one mechanism.

*(D1 clarification — 2026-08-23, decided by Charlie Cheever with
LLP 0553's acceptance (its §11 ask 1a): D1's "an edit compiles to a
plan patch" reads as **the plan patch is the HMR unit wherever a plan
is the live artifact**; 0417's generation algebra is the universal
spine for every reload system; verified source-module records (0417)
and Fast Refresh module updates are typed payload adapters on the
same coordinate/refusal/receipt bus, converging on the plan-patch
unit as each production plan runner becomes its tier's default —
LLP 0553 §3/§3.4/§9 D7. This clarifies scope; it re-decides nothing.)*

Recorded nuance: D1/D2 are direction, decided now because five
programs' deferred HMR questions and Track R's format work all need
the sentence; the full dev-loop program RFC (patch format details,
budgets, channel protocol, embedded rollout) deliberately **waits for
Track R's plan format v1** and is written against it.

## Requirement carried into the freeze window

**R1 — Plan-format identity stability.** The plan format v1
(`issues/20260820-llp0485-track-r-plan-format.md`, chunk 1) must
guarantee stable component and cell identity rules sufficient for
state-preserving refresh — RFC 0086's condition, restated for the
plan: identity must be derivable from the format across a patch, not
from object graph position. If the format hardens without this,
state-preserving HMR is foreclosed and returns later as an ABI
migration. Filed as
`issues/20260820-llp0485-plan-format-identity-stability-hmr.md` with
this decision; Track R's gate was satisfied 2026-08-20, so the window
is open now.

## Consequences

- The five reload systems become one program's phases instead of five
  designs; the program RFC (post-Track-R) consumes D1/D2 rather than
  re-deciding them.
- RFC 0492/0494/0497/0498 answer their deferred HMR questions
  uniformly: "segments patch under D1's spine."
- The DOM mirror's retirement becomes a planned deletion (with 0483),
  not an accident of drift.
- Dev-loop budgets get a better ceiling: plan-only edits skip the JS
  graph entirely, making the <500 ms stretch target plausible
  end-to-end; the budget rows land with the program RFC.
- Costs accepted with the decision: wasm in the web dev loop (runner +
  patch channel), and 0417's H1 obligations remain binding — D1 builds
  on its algebra, not around it.
