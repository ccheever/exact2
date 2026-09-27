# LLP 1036: Elegance frictions

**Type:** Research
**Status:** Draft
**Systems:** All
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-12
**Related:** LLP 1000 (the map); `rules/RULES.md` §Scope (web is the standard), §Budgets (15-doc working set); `rules/DEFERRED.md` (the doing-list bar); LLP 1025 (the 2026-09-01 review); LLP 1035.005 (contract authoring ergonomics); `QUEUE.md`

## Summary

The core is elegant: layered crates with no build matrix, one Contract source,
the web as parity oracle, counted boot, eight agent operations under a seekable
clock. This document lists where that elegance currently leaks — seven frictions
observed in the tree on 2026-09-12, each with where it was seen, which rule it
strains, and what resolution looks like. It proposes nothing; each item names its
own next step.

## Findings

### F1. Native layout disagrees with CSS on percentage widths

`QUEUE.md` (2026-09-08): LLP 1032's blocks ended at 30,145 points while Taffy
sized their `width: 100%; max-width: 720px` column to 67,373, leaving a white
phantom scroll range. The readers work around it with the CSS-equivalent
definite preferred width plus `max-width: 100%`.

Strains `rules/RULES.md` §Scope — a semantic that could follow CSS follows CSS —
and the `AGENTS.md` parity rule: a kernel that disagrees with a bare `<div>`
reintroduces the four-disagreeing-default-layers bug class. Resolution is a
kernel fixture both engines agree on, per the QUEUE line.

### F2. `line-height` is a length, not CSS's ratio

`QUEUE.md` (2026-09-08): the kernel's `line_height` row is absolute points, so a
unitless `1.62` draws every line on top of the last. CSS's unitless value is a
multiple of the font size. Same rule strained as F1. Resolution is binary and
small: implement the ratio, or declare the deviation in LLP 1001 §1 the way the
`position: static` deviation is declared.

### F3. Contract files cannot be shared between apps

`QUEUE.md` (2026-09-08): the `Blocks`/`Runs` renderer is copied between
`apps/markdown` and `apps/llp` because `use … from` refuses a `..` segment and
any path outside the entry file's directory. A third reader is the agreed
trigger to widen it; widening has a compatibility-id consequence (the shared
file becomes an input to the bake).

Strains the delete-don't-deprecate spirit: a refused import is a forced fork,
and forks are how 8.2M-word corpora start. Resolution is the widening itself,
gated on the third reader as QUEUE says — not sooner.

### F4. Concurrent Apple builds ship each other's binaries

`QUEUE.md` (2026-09-08): `host/apple/build.mjs` links every app's `ExactMac`
into one product path (`host/apple/.build/<triple>/release/ExactMac`, staged
through one `.build/composition-<composition>`) and copies it into each app's
bundle. Two simultaneous builds of different apps race; seen for real as
`LLP.app` launching as Weatherlight. `scripts/exact.mjs` now refuses a bundle
whose executable carries another app's id — a smoke alarm, not the fix. The fix
is per-app product and scratch paths.

Strains the dev-loop promise (`--run` should just work) and agent ergonomics:
two agents in two terminals is the normal case here, not an edge. This is the
highest-severity item on the list because it ships wrong bytes silently absent
the alarm.

### F5. Host and script surface weight

`host/apple/` carries a C ABI, SwiftPM package, AppKit and UIKit presenters,
shared bridge, GPU-module ABI, and update adapters; `host/web/` carries eight
glue/storage files around the wasm; `scripts/` carries ~15 drivers with
overlapping jobs (`app`/`exact`/`deploy`/`origin`/`serve`/`smoke`/`agent`/
`metrics`/`boot`/`caps`/`filesystem`/`png`/`issue`). Each piece arrived for a
stated reason, and `scripts/app.mjs` being the one place that knows the
outside-the-repo app is good layering — but the discovery cost is real, and the
`AGENTS.md` dev-loop paragraph's length is the symptom.

Strains the "agents remove apparatus" rule asymmetrically: humans keep adding
drivers one reasonable addition at a time, exactly how the old repo reached 450
scripts. Resolution is deletion pressure per addition (the DEFERRED move rule:
name what comes off), starting with the two dev-only URL loaders' collapse
already planned under LLP 1026 D12.

### F6. Delivery is the heaviest concept in the repo

Baked-in origin, signed bundles, streams, per-host update adapters above
unmodified core hosts, and an env-var vocabulary to match
(`EXACT_UPDATE_TRUST`, `EXACT_UPDATE_RECEIPT`, `EXACT_UPDATE_GENESIS`,
`EXACT_APP_DIR`, `EXACT_SIM`). Justified — deploying to installed apps is the
point of LLP 1030 — but it is where "one Contract source" simplicity goes to
die, and the first thing a newcomer must hold in their head that is not the app.

Nothing to resolve by design; the friction to watch is whether delivery concepts
leak into authoring (they currently don't — `DeliveryState` in
`apps/caltrain/app.contract` is just a resource shape with two actions).

### F7. Corpus pressure against the 15-doc cap

The working set sits at 15 of 15 with six slots held by the 1035 umbrella; this
very document forced an archive to land. The cap is doing its job — it forced
the trade — but the backlog (`QUEUE.md`, 478 lines) against a full working set
means every new line of inquiry now evicts an active lane's orientation link.

Resolution is throughput, not a bigger cap: land and archive 1035 slices as
they complete (1035.000 slice 1 and 1035.002 slice 1 already have), and keep
archiving "built, transcribed, Accepted" specs the way 1014.000 was archived
for LLP 1025.

## Confidence

F1–F4 are quoted from dated `QUEUE.md` lines (2026-09-08); F4's severity rests
on a witnessed wrong-binary launch recorded there. F5 is a file count plus the
author's judgment that the `AGENTS.md` paragraph length measures it. F6 is
structural reading of `AGENTS.md` and LLP 1030's shape, not a defect claim. F7
is counted (`caps`: working set 15 of 15; `QUEUE.md`: 478 lines). All seven were
re-checked against the tree on the document date; none has been verified by a
fresh reproduction run.

## Follow-up repairs — 2026-09-11 PDT / 2026-09-12 UTC

Charlie requested concrete improvements and sub-LLPs, then directed Astra high
agents to implement all three. The implemented repairs are:

| Order | Repair | Result |
|---|---|---|
| 1 | [1036.000 — App-owned Apple builds](1036.000-app-owned-apple-builds.rfc.md) | Concurrent builds and agent launches select the right app, including simulator, device and sample-host outputs. |
| 2 | [1035.000.000 — CSS line height](1035.000.000-css-line-height.rfc.md) | Ratios inherit correctly, fixed lengths stay explicit, and zero stops meaning natural height on native. |
| 3 | [1035.000.001 — Reader percentage layout](1035.000.001-reader-percentage-layout.rfc.md) | The readers use the original CSS shape with a regression that prevents phantom scroll range. |

The CSS repairs refine the already-covering 1035.000 D10 and slices 2/3;
they live under that parent and were implemented independently of the unrelated
clipping and baseline work. The child RFCs retain implementation evidence and
the observed validation limits; the existing owner specs describe the new behavior.

**New evidence, separate from the findings above:** concurrent LLP/Weatherlight
builds now launch the correct apps in both completion orders. Build claims also
cover distinct Cargo package names that normalize to the same library output.
Line-height ratios, px lengths, normal and zero survive compiler, wire,
inheritance and host projection; the existing mixed-font metric-rounding question
remains in 1035.000 D6. The real reader reproduction required an overwide token
inside a padded code block. In the reader lane, fixing intrinsic cache reuse
reduced the native LLP column from 67,333 to 30,049 points, matching its children;
both readers ended at their declared 72-point bottom padding on all four hosts.
Those font-dependent heights precede typed-line-height integration. The combined
macOS check retained the same exact child extent and 72-point padding. No scroll
clamp or global invalidation was added.

**Not selected:** F3 retains the third-reader trigger; only Markdown and LLP
currently carry this renderer. F5/F6 do not justify a new umbrella or deleting
drivers by file count. In particular, 1026 D12's loader-collapse suggestion
must be read with 1030 D4's later implemented separation: an `L=0` host still
has a development connection and must not acquire the update store just to
share that code. This pass makes no new delivery proposal.

**Working-set trade (F7):** creating the three links replaced the links to the
as-built 1005, 1006 and 1007 specs. Completion archives the three child links,
leaving 12 of 15 current slots and 5 of 10 foundation slots. All corpus documents
remain reachable through the root map, parents and code references; the open
1035 slices keep their orientation links. No new driver, registry or blocking
check accompanies these repairs.
