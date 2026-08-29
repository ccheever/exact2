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
  across two files, neither marked generated. The door stays open the way React's
  does — we design nothing that closes it: an owned-pixel surface is a leaf node
  with a kernel-owned box (like `NativeView`), GPU content never influences layout,
  the host owns the frame, and animatable properties are extensible (CSS
  `@property`). When it is built, it compiles no shaders at runtime (LLP 0559 F8).
- Server generation in every form: SSR, streaming, static export, progressive forms,
  hydration, route payloads, response caching.
- Aquifer data tier, durable worker tier, capability capsules, durable capability grants.
- Snapback / update economy.
- HBC compilation, hot revision surfaces, staged reload. (A dev reload *carries
  state* — slots by name where their types still fit, settled resources where
  their arguments still match, the clock — and is otherwise a restart: no patch
  format, no generations, no identity matching. LLP 1007 §6; Charlie, 2026-08-28.)
- Islands and inline islands.
- Cross-runtime shared data.

**Components** — roughly 15 built-in tags, not 40; roughly 12 Facet components, not 47.

- No `video`, `webview`, `canvas`, `lottie`, `rive`, `fileinput`, `pager`.
- No camera anything.
- No virtualList v2 (cert wires, extent demand, proxy lanes). A straightforward windowed
  list — and if it misses 60fps, that is a kernel bug worth fixing properly.

**Motion** — **in v1**, in the LLP 1002 shape: CSS's `transition` model. Targets
are kernel style rows (`translate`, `scale`, `rotate`, `opacity`); a `transition`
row on the node says how they get there; the web host emits it as CSS and does
nothing per frame; every other host runs `exact-motion`, which is held to the
browser by fixtures. One declared deviation, `spring()`, lowered to keyframes on
the web. Gestures are in as follow-and-release: the platform recognizes, the
engine holds a value and springs it back with the release velocity. The seekable
clock is in, and it is the reason motion is testable: an agent advances time to
`settle_time()` and reads; it never waits.

Moved off this list 2026-08-28 (LLP 1002 D6): **delegation to CSS on the web** — it
unblocks a web host that ships zero motion bytes and a parity corpus with the
browser as the oracle, the same shape layout already has. In exchange, not in v1:

- **A gesture arena, claims, leases, compositions, or an interactive-navigation
  model.** Recognition, hit-testing, and scroll-vs-pan arbitration are the
  platform's (`touch-action`, `UIGestureRecognizer`); owning them is the
  permanent bug annuity LLP 0559 F1 describes. Scroll always wins.
- **A second value graph.** No shared-value plane, derived values, bindings, or
  plan node graph. The style row is the binding.
- **Layout transitions.** Gated on a demonstrated incremental-relayout number.
  There isn't one.
- **Decay, sequence, and repeat drivers; `@keyframes`.** A spring carries release
  velocity; nothing else needs a driver.
- **Reduced-motion policy in the engine.** The producer emits `transition: none`
  when the host reports the preference, as a stylesheet's media query would.
- **A Core Animation executor.** Permitted by LLP 1002 D2, not built; the Apple
  lane measures whether it earns its place.
- **`runOnJS` and the escape hatch / runtime graph admission** — never existed here.

**Tooling** — no Design Mode, no Guide system, no devtools UI, no TUI host, no blog/CMS.

**Agent API** — 8 operations, not 90:

`tree` · `screenshot` · `tap` · `type` · `state` · `layout` · `logs` · `clock`

**A ninth operation replaces one of the eight, same PR.** A new input is a form of
`tap` or `type` (a wheel is a `tap`; so would a drag be); a new question is answered
from `tree`, `state`, or `layout`. The old repo's six primitives grew eighty wire names
one reasonable "view" at a time; the count is the cost. (Charlie, 2026-08-29.)

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
- **No speculative specs.** Specifying something you are about to implement is fine and
  expected — the deciding already happened upstream. Specifying something nobody is
  assigned to build is not written at all. The old corpus is 330 RFCs of deliberation
  against 52 specs of conclusion; the new repo imports conclusions.
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
