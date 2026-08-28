# LLP 0560: Presenter Behavior Is Semantics-Tree-Visible

**Type:** Principle
**Status:** Draft
**Systems:** Apple Hosts, Windows Host, Web Host, Android Host, Presenters, Acto, Verification, Contract, Kernel
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-24
**Revised:** 2026-08-24 (r1 — initial draft; commissioned by the author's 2026-08-24 decision taking LLP 0559 §8 ask 4)
**Related:** LLP 0559 (F4 — the finding this principle encodes: Flutter's owned raster bought a deterministic testable artifact; Exact's substitute is the semantics tree plus receipts, and the substitution holds only if presenters cannot ship behavior outside it), RFC 0495 (Acto on the substrate — every projection derived from the structure the engine executes; the receipt stream), RFC 0496 (verification refresh — the registry a future conformance check lands under), LLP 0321 (Acto agent subsystem), LLP 0485 §3.3.13 (the Semantics projection), LLP 0306 (agent input verification fidelity — the phantom-verification lesson this principle inverts), LLP 0223 (presenter cutover), `docs/agent-api.md`

## Principle

**No presenter may ship behavior or state that is invisible to the
semantics tree and its receipts.** Exact deliberately does not own its
pixels (the Sandwich Model); its testability story substitutes the
semantics/agent tree, the receipt stream, and contract verification for
the owned-raster determinism that frameworks like Flutter get for free.
That substitution is load-bearing — contracts, Acto, and the verification
registry all assert against the tree — and it holds only if the tree is
*complete*. The moment a platform presenter carries interaction or visual
state the tree cannot see, the testability story dies silently: nothing
fails, coverage just quietly stops meaning anything.

Flutter's semantics tree was an accessibility bolt-on that perpetually
lagged its render tree (LLP 0559 F4). Exact inverts that — the tree is
the primary artifact — and this principle is what keeps the inversion
true as presenters grow.

## Rules

**ALWAYS:**

- Any interactive behavior a presenter implements — gesture recognition,
  hover/pressed visual state, focus movement, keyboard handling, context
  menus, drag feedback — is represented in, or derivable from, the
  semantics tree, its receipts, or a registered Acto projection.
- A new presenter capability lands **with** its tree/receipt
  representation in the same change, never as a follow-up. "Pixels first,
  semantics later" is how the gap opens.
- Presenter-side state machines with duration — an in-flight animation, an
  open native menu, a scroll deceleration, an armed gesture — expose their
  state at minimum at diagnostics level, so "why does the tree disagree
  with the screen" is answerable.

**NEVER:**

- A presenter-only interaction path that Acto can neither observe nor
  drive. This is LLP 0306's phantom-verification lesson inverted: 0306
  warns that agents can exercise paths users never hit; this principle
  forbids the mirror image — paths users hit that agents cannot see.
- Verifying a presenter behavior by screenshot alone because the tree has
  no representation of it. A screenshot-only check is the symptom that
  this principle is already violated; fix the representation, then check.

**Bounded exception — OS-owned chrome:** where the platform mandates
ownership of the middle of an interaction (system context menus, IME
candidate windows, share sheets, permission dialogs), the presenter
represents the **trigger and the outcome** in the tree and receipts even
though the OS owns what happens between them. The exception covers the
OS-owned span only, never the presenter's own behavior around it.

## Enforcement

Today: review discipline — a presenter change adding behavior is reviewed
against this principle the way threading changes are reviewed against
LLP 0297. Intended end state: a registered conformance check under
RFC 0496's registry asserting that new host operations and presenter
event sources carry a tree/receipt representation (shape to be defined by
the 0495/0496 owners; this principle constrains, it does not design).
Until that check exists, this document is citable in review the way
LLP 0309 is for process hygiene.

## Rationale

The semantics tree is Exact's raster (LLP 0559 F4): the one deterministic,
cross-platform artifact that contracts compile against, agents drive, and
the verification registry checks. Everything in the program that replaces
eyeballing pixels — contract blocks, Acto targeting, `observeAfter`
receipts, the causal stream — assumes tree completeness. The cost of
holding this principle is small and paid at design time (every presenter
feature answers "what does the tree say?"); the cost of losing it is paid
forever and discovered late.
