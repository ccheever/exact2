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

**Motion** — the `transition=` attr only (opacity, transform, backgroundColor, borderRadius).

- No SharedValue graph, no gesture system, no worklets, no `runOnJS`, no motion commands,
  no scroll binding, no UI worklet runtime.
- This is the largest single deferral and the most likely to be contested. The argument
  for deferring: worklets force a second JS runtime on a second thread, which is where a
  large share of the old repo's threading complexity, callback-affinity ledger, and
  sanctioned sync-wait exceptions originated.

**Tooling** — no Design Mode, no Guide system, no devtools UI, no TUI host, no blog/CMS.

**Agent API** — 8 operations, not 90:

`tree` · `screenshot` · `tap` · `type` · `state` · `layout` · `logs` · `wait`

Not shipping: session record/replay, causal trace, behavior diff and verify, mutation
dry-run, contract witness / dataflow / source-map / bindings, accessibility audit, plan
drag, correlate, advance time, visual query, network, perf, preferences, pasteboard,
onboarding, revalidate, code grant/resume/cancel.

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
