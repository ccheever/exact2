# What Exact v1 Does Not Do

This is the most valuable file in the repo. It is longer than the doing-list on purpose.

## The bar that makes this list derivable

v1 is done when **one real application** — not a demo — runs from a single Contract
source on web, macOS, iOS, and Linux, hitting the time budgets in `RULES.md`.

**Open decision:** name the app. Recommendation is the Caltrain app — it already exists
in Contract, it has real lists, navigation, search, text, and theming, and "does it
still work" is answerable in seconds. Everything not required by that app does not exist.

## Surfaces

- **Windows.** A working Direct2D host exists in the old repo. It is real work, and it
  doubles the native matrix. Port it after the loop is proven.
- **Android.** Same.

Every surface multiplies the sweep, the presenter count, and the number of ways one
change can break.

## Authoring models

- **React tier.** The door stays open — meaning we design nothing that forecloses it.
  We do not build it. No React Facet bindings, no RecipeIR generation, no dual-framework
  parity gate. One authoring model, one set of bugs.
- **Rust Native roots.** No Rust-owned UI roots, no deployment-manifest registrations,
  no app-ABI generator.
- **Platform-suffixed route overrides** (`.native.tsx`, `.mac.tsx`, ...). One route, one
  file. If a platform needs different behavior, that is a branch inside the component or
  a bug in the presenter.

## Features carried over as "no"

**Runtime**

- GPU / WebGPU substrate. The old repo carries ~139,000 lines of Rust adapter code
  across two files, neither marked generated.
- Server generation in every form: SSR, streaming, static export, progressive forms,
  hydration, route payloads, response caching.
- Aquifer data tier, durable worker tier, capability capsules, durable capability grants.
- Snapback / update economy.
- HBC compilation, hot revision surfaces, staged reload.
- Islands and inline islands.
- Cross-runtime shared data.

**Components** — roughly 15 built-in tags, not 40; roughly 12 Facet components, not 47.

- No `video`, `webview`, `canvas`, `lottie`, `rive`, `fileinput`, `pager`.
- No camera anything.
- No virtualList v2 (cert wires, extent demand, proxy lanes). A straightforward windowed
  list — and if it misses 60fps, that is a kernel bug worth fixing properly.

**Motion** — **in v1**, in the RFC 0492 shape: one Rust evaluator crate, one clock,
compiled to plan data, no JS on the frame path. Sinks are `transform` and `opacity`.
Gestures are in; they are the continuous-input path, not decoration. The virtualizable
clock is in, and it is the reason motion is in at all: it makes animation seekable, so
the verification loop advances time instead of waiting for a settle.

Not in v1:

- **Delegation/handback to Core Animation or CSS.** That is the second-executor seam.
  One evaluator everywhere at a measured power cost — the same call the old repo made.
- **A Tier-1/Tier-2 split.** `transition=` is a lowering onto the same graph, never an
  independent system. The old repo's motion ticket cluster lives entirely on that seam.
- **Layout sinks.** Gated on a demonstrated incremental-relayout number. There isn't one.
- **State machines, Design Mode knobs, the escape hatch / runtime graph admission, and
  `runOnJS`** (already deleted by decision in the old repo — no shim, no migration).

**Tooling** — no Design Mode, no Guide system, no devtools UI, no TUI host, no blog/CMS.

**Agent API** — 8 operations, not 90:

`tree` · `screenshot` · `tap` · `type` · `state` · `layout` · `logs` · `clock`

Not shipping: session record/replay, causal trace, behavior diff and verify, mutation
dry-run, contract witness / dataflow / source-map / bindings, accessibility audit, plan
drag, correlate, visual query, network, perf, preferences, pasteboard, onboarding,
revalidate, code grant/resume/cancel.

`clock` replaces a `wait` operation on purpose. If the motion graph is closed-form under
a virtual clock, an agent advances time and reads the result; it never sleeps waiting for
an animation to settle. Settle timing is the single largest source of flake in the old
repo's agent loop, and under `RULES.md` a flaky check is worse than no check.

## Process

This half matters more than the feature half.

- **No governance corpus.** 20 docs, capped. No lane manifest, no verify registry with
  675 entries across 64 profiles, no contracts authority map, no issue priority scoring.
- **No design-doc-before-code requirement.** Docs are written after a subsystem exists,
  describing what it does.
- **No refine loops.** No dual-family review rounds, no READY verdicts, no revision ledgers.
- **No per-PR ceremony.** No framework decision log, no lane debt ledger, no provenance
  headers on hand-written files.
- **No blocking check the author did not run locally in under a minute.**
- **No 90-minute gate.** If verification takes 90 minutes it runs behind you, or it does
  not run.

## Deliberately worse

- No backwards compatibility, at all, before 1.0.
- No public API stability. Names change.
- No migration guides.
- Generated files are built, never committed.
- Sparse prose. The code and the checks are the authority.

## Moving something off this list

Write one line naming what it unblocks, and take something off the doing-list in the
same PR. If nothing can come off, the answer is no.
