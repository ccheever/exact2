# RFC 0084: Contract Layout, Styling, and Host Mapping

**Type:** RFC
**Status:** Draft
**Systems:** Contract, Tooling, Layout
**Author:** Charlie Cheever / GPT-5 Codex
**Date:** 2026-03-27
**Revised:** 2026-08-19
**Related:** RFC 0043, RFC 0074, RFC 0075, RFC 0081, RFC 0085, RFC 0088, RFC 0089, RFC 0092, LLP 0195 (responsive/adaptive layout), LLP 0199 (declarative layout intent and relationships), LLP 0288 (Contract as the production-web target), LLP 0349 (programmable layout), LLP 0486 (one layout language, two engines — the doctrine and machinery grown from this RFC's direction stub), LLP 0487 (the choice-layout semantic kernel)
**Track:** Experimental
**Parent:** RFC 0080

## Summary

Define the built-in Contract view primitives, how they lower onto Exact host primitives, and how layout and styling should be described honestly in a Taffy-backed system.

## Layout Reality

Taffy is a flexbox/grid layout engine. It is not a general relational constraint solver.

Therefore Contract v0 should target:

- higher-level layout patterns that lower cleanly onto flexbox/grid/positioning
- a few semantically meaningful container forms
- explicit escape hatches for lower-level props

It should **not** claim full Auto Layout-style relational constraints.

## Built-in View Primitives

v0 built-ins:

- containers: `column`, `row`, `stack`, `scroll`, `section`
- leaves: `text`, `button`, `input`, `toggle`, `image`, `spacer`, `spinner`, `skeleton`
- control flow: `when`, `else`, `each`

The base surface above is intentionally small. RFC 0088 adds dedicated collection primitives for scroll-heavy app surfaces, and RFC 0089/0092 layer motion and accessibility semantics onto the same host mapping.

## Host Mapping

Illustrative lowering:

| Contract | Exact host concept | Notes |
|---|---|---|
| `column` | `View` | flex direction column |
| `row` | `View` | flex direction row |
| `stack` | `View` | overlapping/positioned child container |
| `scroll` | `ScrollView` | axis + inset handling |
| `section` | `View` + semantics | labeled group/section |
| `text` | `Text` | plain/inline text node lowering as needed |
| `button` | `Pressable` + `Text` | default press-capable control |
| `input` | `TextInput` | form field |
| `toggle` | `Toggle`/`Switch` | bool control |
| `image` | `Image` | source normalization reused from Exact |

The Contract surface is intentionally smaller than the Exact primitive set.

## Layout Sugar

Useful v0 sugar:

- `row` / `column`
- `spacing`
- `align`
- `grow`
- `safe-area`
- `scroll axis="x|y"`

These are not new layout engines. They are compact frontends over capabilities Exact already has.

## Direction Stub: A Contract Layout Algebra, Not CSS Emulation

Contract may eventually expose a small **layout algebra**: composable
operators with Exact-defined semantics, rather than a growing imitation of CSS
properties. `row`, `column`, `grid`, and `stack` are already the first such
operators. Candidate additions include choosing among valid arrangements,
expressing bounded relationships, and invoking registered specialized layout
algorithms.

The distinction is one of authority. Contract states the requested arrangement
and its invariants; CSS is one possible web lowering, not the definition of the
Contract feature. A host may lower an operator directly to CSS or Taffy only
when that preserves its specified behavior. Otherwise the operator needs an
Exact-owned evaluator and a web realization that retains real DOM semantics
and source order. Contract should not inherit unrelated CSS behavior such as
the cascade, margin collapsing, anonymous boxes, or incidental intrinsic-size
quirks merely to make a new operator look familiar.

The lowering decision is **per instance, not per operator** (LLP 0486
§8.2's realization ladder): the compiler certifies, at each use site,
whether a pure CSS lowering preserves the operator's specified behavior
there — for choice layout, `@container` queries qualify only when every
fit condition is a pure size threshold and the container's inline size is
provably independent of its children — and otherwise realizes that
instance through the Exact-owned evaluator. One operator may take both
paths in the same app; certification is static, and the conformance
fixtures decide disputes.

The first experiment should be a bounded **choice layout**. A component
declares a small set of arrangements, required fit conditions, and explicit
preferences; Exact deterministically selects the highest-preference valid
candidate under the available constraints. This represents the author's
intent more directly than scattered breakpoint tests while remaining local to
one container. Syntax and the cost model remain deliberately unfrozen;
the operator's semantic kernel — selection predicates, admission rules,
and state/identity via the same-children restriction — is now specified
in LLP 0487, whose certified-form vocabularies remain fixture-governed
per its §1 maturity marker.

This direction composes existing ownership rather than replacing it:

- LLP 0195 owns size classes, explicit responsive branching, and the
  container-size feedback channel.
- LLP 0199 owns declarative relationships and their IR representation.
- LLP 0349 owns registered programmable-layout evaluation, budgets,
  invalidation, and child-frame authority.
- LLP 0486 owns the cross-host doctrine that keeps those lowerings
  honest — parity tiers, the Contract Layout Profile authority, the
  dual-engine conformance instrument, and the shared feedback channel —
  and LLP 0487 owns choice layout's semantic kernel.
- This RFC owns which concepts graduate into Contract's public layout
  vocabulary and how their host lowerings remain honest; the graduation
  evidence LLP 0486 §8.3 enumerates is submitted to this RFC.

Taffy flex/grid remains the default layout path. No alternative operator
graduates without deterministic cross-host behavior, bounded invalidation,
diagnostics, accessibility/DOM-order preservation, and evidence that it makes
real Contract surfaces materially simpler.

## Collections and Scrolling

Real apps need more than inline `each`.

The collection model should distinguish between:

- `each` for small inline repeated regions inside an existing layout
- `list` for ordinary scrollable collections where all items may be mounted
- `virtual-list` for large/windowed collections
- `section-list` for grouped and heterogeneous data with headers/footers

These should lower to Exact scroll/container primitives with explicit item identity, visible-range metadata, and hooks for pagination and pull-to-refresh.

RFC 0088 defines this in detail.

## Non-goals for v0

- arbitrary sibling-to-sibling relational constraints
- baseline-linked layout solvers
- a full design tool replacement
- pixel-perfect automatic layout from semantic intent alone

## Styling Layers

Contract styling should have three layers:

1. **Semantic props**
   Examples: `emphasis`, `tone`, `role`, `spacing`
2. **App-level tokens/helpers**
   Design-system values defined by the app or module layer
3. **Raw style props**
   Exact/canonical style props as an escape hatch

This keeps the framework modest while still giving agents a more compact common path.

## Semantic Prop Scope

The built-in semantic vocabulary should stay intentionally small.

Rationale:

- too small and it becomes decorative
- too large and the framework becomes a design system

Contract should not try to solve the whole design-token problem in v0.

## Safe Area and Platform Semantics

Contract may expose semantic container-level affordances such as:

- `safe-area=true`
- `scroll axis="y"`
- named emphasis/tone roles

But the underlying platform-specific interpretation should still route through Exact's existing host/window-management logic rather than inventing separate behavior.

## Web and Native Alignment

Because Contract lowers through Exact:

- browser development should exercise the same host mapping semantics as native Exact where practical
- web-specific rendering quirks should be handled in the Exact layer, not by forking Contract semantics

This keeps Contract from becoming "one framework for web, another for native."

## Motion and Semantics Hooks

Host mapping must leave room for:

- enter/exit/change/reorder transitions keyed by stable view identity
- reduced-motion-aware behavior
- accessibility roles, labels, focusability, and live-region announcements emitted by default for built-ins

Those concerns are specified more fully in RFC 0089 and RFC 0092, but the host mapping layer must reserve the necessary metadata channels from the start.

## Hard Parts

- Designing sugar that is actually useful without misleading authors about the underlying layout engine.
- Keeping semantic styling helpful without overcommitting the framework to a built-in design language.
- Mapping high-level Contract controls to Exact primitives consistently across browser and native targets.

## Open Questions

- Should v0 include named layout patterns beyond `row`, `column`, `stack`, and `scroll`?
- Does a bounded choice-layout corpus outperform explicit LLP 0195 size-class
  branching in clarity, stability, and runtime cost?
- Is the portable contract a shared `measure(constraints) -> result` protocol,
  or only a closed catalog of built-in operators?
- Which layout invariants require frame parity across hosts, and which require
  only semantic/relational parity?
- Should semantic props compile directly to raw styles, or route through a theme/token resolver?
- How much host primitive surface should be exposed directly as escape hatches?
- How much collection functionality should be built into `list`/`virtual-list` primitives versus delegated to the router/data layer?
