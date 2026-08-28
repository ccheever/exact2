# LLP 0559: Flutter Lessons for the Refresh Program

**Type:** Research
**Status:** Draft
**Systems:** All (a synthesis; owns no boundary — advisory input to the 0478–0558 Refresh corpus)
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-24
**Revised:** 2026-08-24 (r2 — §8 dispositions recorded under the author's same-day "all of it" decision: asks 1/4/5/8 TAKEN as LLP 0553 §5 A1, LLP 0560 (new Principle, Draft), the RFC 0490 pipeline addendum, and LLP 0160 §5.3 A1 (+ AGENTS.md and `lane-report.mjs` carriers); asks 2/3/6 ticketed under `issues/20260824-*`; ask 7 closed as covered by LLP 0507 §2's conformance machinery) 2026-08-24 (r1 — initial draft from the 2026-08-24 conversation; every Flutter-side claim carries §1's provenance label and none has been re-verified against current Flutter sources during authoring)
**Related:** LLP 0504 (the Refresh map — this document is advisory input to that program, in 0504's own "advisory derived map, owns nothing" spirit), LLP 0552 (the end state the Refresh corpus describes; §1's pattern sentence is the frame every finding here maps onto), LLP 0491 + LLP 0487 (kernel refresh + layout conformance corpus — F2, F4), LLP 0490 (GPU substrate — F8), RFC 0492 (motion refresh — F6, F8), RFC 0493 (text-input behavior declarations — F1), RFC 0495 (Acto on the substrate; the causal receipt stream — F4, F7), RFC 0496 (verification refresh — F4), RFC 0497 (Facet refresh — F6), RFC 0498 (Aquifer — F3), LLP 0500 + LLP 0553 + LLP 0553.001 (dev-loop decision, program, and patch envelope — F5, the deep-dive finding), LLP 0524 + LLP 0331 §5.2 (native-code reload research on the supervisor/worker architecture — F5), LLP 0532 / LLP 0535 / LLP 0539 (authoring refresh and its deliberate language growth — F9), LLP 0540 / LLP 0543 (lists — F1, F8), LLP 0508 (Contract language v1 — F9), LLP 0541 (edition identity — F5), LLP 0288 (Contract as the web production target — F2), LLP 0297 (threading contract — F8), LLP 0154 / LLP 0160 (route single-source; Contract-first policy and its §5.3 decision log — F3, F9), LLP 0406 (Expose / Linux DRM-KMS milestone — F10), RFC 0086 (`llp/contract/` — the refresh ladder 0553 adopts; F5)

## Summary

Flutter is the most instructive external comparable for the Refresh program
because it made the **opposite** bet on Exact's biggest architectural question
— own the pixels versus borrow the platform — and the **same** bet on several
others: a fully specified frame pipeline, a deterministic artifact you can
test against, declarative configuration interpreted by an engine, and hot
reload as a first-class architectural concern rather than a tooling
afterthought. A decade of Flutter in production therefore functions as a
natural experiment against much of the Refresh corpus.

Findings, in three buckets. **Validations (F1–F3):** Flutter's permanent
bug tail from owning platform behaviors validates the sandwich model and
RFC 0493's declare-don't-own shape; Flutter web's canvas dead-end validates
LLP 0288's real-DOM bet and the corpus-as-instrument answer to the 2×
layout tax; Flutter's state-management vacuum validates Contract's
language-level state and Aquifer. **Bars to clear another way (F4–F7):**
Flutter's test economy and golden tests came from owning the raster — Exact's
semantics tree plus receipts must be the equivalent deterministic artifact,
and no presenter behavior may exist outside it; Flutter's stateful hot reload
is its single most adoption-driving feature, Contract's reset-based HMR is
below that bar today, and the 0553 design as specified clears it and exceeds
it on explainability, dev/prod fidelity, and native-code reach — the gap is
execution, not architecture; the Facet catalog, not the engine, is the
adoption surface, and motion must be baked into it; receipts can beat
DevTools because Flutter never had a causal story. **Warnings (F8–F10):**
Impeller's three-year mid-flight renderer replacement argues for an
enumerable, precompilable pipeline set in 0490 from day one and for treating
the substrate contract as expensive to revise; the app-as-data bet is more
radical than Flutter's and needs a monitored expressiveness ceiling; and
Flutter's narrow C-ABI embedder API — the reason it runs on car dashboards
and DRM/KMS — is the model for keeping the presenter/host-ops seam small
enough that the LLP 0406 milestone is an implementation, not a port.

This document decides nothing and owns no boundary. §8 lists advisory asks
addressed to named owners; each is theirs to take or refuse.

## 1. Scope, method, and provenance

**What this is:** comparative research mapping Flutter's design decisions and
their observed decade-scale consequences onto the Refresh program's tracks.
Each finding names the Exact documents it bears on. It follows the corpus's
own comparative-research precedent (LLP 0339, the Vercel native-SDK
comparison; LLP 0420, the PocketJS lessons).

**Provenance of Flutter-side claims:** stated from the authoring model's
general knowledge of Flutter through early 2026 — its architecture
documentation, the Impeller design rationale, the flutter.dev hot-reload
limitations page, DevTools, the embedder API, and widely reported production
experience. **None were re-verified against Flutter sources during
authoring.** Where a claim would be load-bearing for an Exact decision, the
owning document should verify it independently before relying on it; §7
grades confidence per finding.

**Provenance of Exact-side claims:** LLP 0552 §1, LLP 0553 (header, Summary,
§1, §5), LLP 0553.001, and LLP 0524 (header, Summary) were read during
authoring; other Exact citations come from the repository's standing
authorities (CLAUDE.md, LLP 0504's ledger) and are believed current as of
2026-08-24 but were not individually re-opened.

**What this is not:** not a proposal, not a scorecard of Flutter, and not a
claim that Exact should converge on Flutter. Several findings are precisely
that Flutter's path is the one to avoid.

## 2. The structural comparison in one paragraph

Flutter owns everything above the display: its own rasterizer
(Skia, now Impeller), its own text stack, its own scrolling, its own
widgets, its own semantics tree, with the platform reduced to a surface and
an input stream. Exact's sandwich model is the inverse: the platform keeps
rendering, text, scrolling, and accessibility; the Rust kernel owns layout
and the decision plane; presenters place native views at computed
coordinates. Yet beneath that opposition, Flutter's deepest strengths — a
specified build→layout→paint→composite pipeline, cheap declarative config
interpreted by retained machinery (the Widget/Element/RenderObject split),
one deterministic artifact to test against, hot reload bought with
architecture — are the same instincts as the Refresh's *decision as data,
one engine, receipts out the side, corpus as instrument* (LLP 0552 §1,
quoting 0504 §1). The meta-observation this document keeps returning to:
**Flutter's wins all trace to specifying the pipeline and owning a
deterministic testable artifact; its losses all trace to taking ownership of
behaviors platforms already do well.** The Refresh program is, in effect, an
attempt to get the first without paying for the second.

## 3. Findings A — Flutter's pain validates decided Refresh bets

### F1. The uncanny-valley tail is real, permanent, and the sandwich model's strongest evidence

Flutter's single costliest class of decisions was taking ownership of
platform-feeling behaviors: text editing, IME and autofill, selection
handles, scroll physics, context menus, accessibility. Ten years in, this
tail still dominates its issue tracker; every behavior owned is a bug
annuity that never amortizes, because the platforms keep moving and
"feels native" is a moving target on N platforms at once.

This validates: the sandwich model as such; native text and native scroll
views as load-bearing choices (lists v2 riding native scrolling — LLP
0540/0543); RFC 0493's shape, where Exact *declares* text-input behavior and
the platform owns the mechanics — precisely the boundary Flutter never had.

**The lesson to hold during Refresh:** whenever cross-platform consistency
tempts a track to take ownership of a platform behavior, the price Flutter
paid is the realistic estimate, and it is paid forever. The legitimate
exceptions are the surfaces Exact deliberately owns (the 0490 substrate),
where the opposite discipline applies (F8).

### F2. Flutter web is the cautionary tale that justifies the 2× layout tax

Flutter web renders to canvas via wasm (CanvasKit, later skwasm; the HTML
renderer was deprecated). The consequences are structural, not incidental:
multi-megabyte payloads, broken text selection, invisibility to SEO,
accessibility through a lagging projected DOM overlay. Fighting the web
instead of using it lost the web.

Flutter avoided a two-engine layout problem by choosing one engine
everywhere — and this is the price. Exact's LLP 0288 choice (Contract to
real DOM, browser CSS as a deliberate second layout engine) is the opposite
bet, and the Refresh's resolution of its cost — "the 2× tax does not
collapse to one implementation but to bounded per-engine behavior plus a
corpus" (0552 §1, citing 0479 §2.1 / 0485 §4.1) — is the correct answer to
the problem Flutter dodged. Notably, Flutter *could not* have taken this
path: it never built a differential conformance corpus because one engine
never needed one. The 0487/0491 corpus harness is therefore not overhead on
the web bet; it is the asset that makes the web bet possible. It deserves
the same institutional weight Flutter gives its golden-test infrastructure.

### F3. The state-management vacuum is Flutter's largest ecosystem wound; language-level state closes it before it opens

Flutter shipped `setState` and nothing else. The ecosystem fractured —
provider, bloc, riverpod, getx, and successors — and "which state
management?" remains the canonical Flutter onboarding question a decade in.
The vacuum was not neutrality; it was a decision by omission that cost more
than any wrong choice would have.

Contract building `state` / `derive` / `action` / `resource` / `task` into
the language, with Aquifer (RFC 0498) extending the same cell model to
server data, is the correct opposite. The guard the Flutter experience
recommends: Flutter's escape hatch — arbitrary Dart in `build()` — became
the main road. Exact's escape hatch is the React tier, and the LLP 0160
§5.3 decision log is the early-warning instrument that idioms are leaking
around the paved path. Those counts are worth actually watching (see F9 for
the sharper version of this).

## 4. Findings B — Flutter's strengths set bars the Refresh must clear another way

### F4. Owning the raster bought Flutter its test economy; the semantics tree must be Exact's raster

`flutter test` pumps frames against a fake clock with no device, GPU, or
platform: thousands of widget tests in seconds, plus golden-file image
tests for pixels. That test economy — cheap, deterministic, headless —
drove ecosystem quality more than any engine feature, and it exists because
Flutter owns one deterministic artifact (its own raster and render tree) on
every platform.

Exact cannot assert pixels cross-platform, by design. The equivalent
deterministic artifact must be the semantics/agent tree plus the receipt
stream — which is exactly what RFC 0495/0496 build, and what contract
blocks already assert against. Two consequences:

1. **The discipline that makes it work:** no presenter may ship behavior or
   state that is invisible in the tree. The moment a platform host carries
   interaction the tree cannot see, the testability story dies silently.
   Flutter's semantics tree was an accessibility bolt-on that perpetually
   lagged its render tree; Exact inverting that — agents and contracts
   *drive* the tree, so it cannot lag — is one of the genuinely
   better-than-Flutter design points in the program. It should be protected
   structurally (a registered check or principle), not by vigilance. §8
   ask 4.
2. **Where golden tests become legitimate:** on the 0490 substrate Exact
   *does* own the pixels. Flutter-style golden-file testing is available
   exactly there, and only there. Worth adopting deliberately rather than
   rediscovering (§9 OQ2).

### F5. Stateful hot reload was Flutter's killer feature, bought with architecture; the 0553 design clears the bar on paper and exceeds it in four ways — the gap is execution

This finding is the deep dive; it was sharpened by direct comparison against
LLP 0553 §5 during authoring.

**What Flutter has.** The Dart VM was chosen substantially for JIT
hot-swap; the Widget/Element split (throwaway config, retained identity)
is what makes re-running build against live state safe. The result: edit a
color five screens into a flow and keep your place, sub-second, on device,
mid-animation. It is the single most adoption-driving feature Flutter has —
its developers evangelize the loop, not the rasterizer. Its limits are as
instructive: preservation semantics are *emergent* from VM + Element
machinery, so Flutter maintains a prose page of cases where reload silently
cannot apply and you must restart; there is no explanation when reload does
the wrong thing; hot reload works only in JIT debug mode, whose performance
is not representative of AOT release (profile mode is representative but
has no reload); and native code — platform channels, FFI — is a full
rebuild, always.

**Where Contract is today:** below the bar, clearly. The live experience is
Vite plus reset-based HMR — every edit is a remount, state gone. LLP 0417
is Accepted and specified to the fixture level but unimplemented; 0553 §1's
own words are "five reload systems with one decided direction and no
program" (the program is now that document, Accepted 2026-08-23).

**The comparison, dimension by dimension:**

| Dimension | Flutter (shipped, ~decade of polish) | Contract today | Refresh design (0500 / 0553 / 0553.001 / 0524) |
|---|---|---|---|
| State across an edit | Preserved; semantics emergent from the VM, with a documented prose list of silent can't-apply cases | Lost (reset-based HMR) | Specified: the 0086 ladder, per-slot dispositions, named-slot identity by declaration tuple, Aquifer cell reuse only on exact `RuntimeCellKey` equality (0553 §5) |
| Explanation of resets | None | None | Typed explanation + dependency path on **every** non-`kept` disposition, enforced by a registered check (0553 §5) |
| Dev/prod fidelity | Dev = JIT debug; reload unavailable in the representative (profile/release AOT) modes | Dev web path differs from production runner | Dev runs the **production** plan runner and the same Rust engine (0500 D2) |
| Native code | Full rebuild, always | Full rebuild | 0331 §5.2 supervisor/worker direction; 0524's evidence-gated program ("edit Rust while the phone app stays open") |
| Scroll position, keyed regions, motion anchors | Preserved through reload today | Lost | **Scheduled, not claimed** — 0553 §5 retracts the earlier overclaim; D3 lands it or fail-closes with explanations |
| Arbitrary imperative app logic | Dart VM swaps method bodies in place | n/a (reset) | Fast-Refresh-class, registration-identity-keyed (0553) — more reset cases than Dart reload for the plain-TS slice |
| Latency | Routinely sub-second on large apps | Fast but state-destroying | Budget rows exist (0553 §6); unproven |

**The read:** the design, if landed with budgets met, clears Flutter's bar
on the Contract-authored surface and is categorically ahead on four
dimensions Flutter cannot retrofit — specified preservation, explainable
resets, dev-on-the-production-runner, and native-code reload. This is
possible *because* of the app-as-data bet: Flutter could never specify
reload semantics over arbitrary Dart object state; declared cells make the
semantics finite. Two honesty notes bound the claim. First, all of it is
spec against Flutter's decade shipped — the polish (mid-animation,
on-device, huge apps) is the part that takes years, and the
scroll/keyed/anchor row is the daily-felt one (§8 ask 1). Second, the
plain-TS slice getting Fast-Refresh-class semantics is a quiet force
pushing app logic into Contract declarations — aligned with the program's
direction, and worth stating as an expected consequence rather than
discovering as a complaint.

### F6. The catalog is the adoption surface, not the engine — and motion must be baked in

Developers choose Flutter because a mid-skill developer gets a
decent-looking, animated, accessible app fast. Material's opinionated
completeness — theming, focus, keyboard handling, disabled states, motion,
accessibility in every widget, with zero author effort — mattered more to
adoption than Skia or Impeller ever did. The part most imitators miss:
Material feels *alive* by default; implicit animation is everywhere, and a
catalog that is static-but-correct loses to one that moves.

Facet (RFC 0497, the Figma-derived authority, 47 bindings) is the analog,
and Material sets its bar: every component handles focus, keyboard,
disabled, RTL, dynamic type, and dark mode without author effort — and the
0492 motion refresh should land **inside Facet recipes as defaults**, not
only as authoring capability an app must opt into. §8 ask 2.

### F7. Dev-tool truth comes from the framework's own data structures — and receipts can beat DevTools

Flutter DevTools (widget inspector, layout explorer, rebuild stats, the
frame timeline with UI/raster split) works because it reads structures the
framework itself maintains. But Flutter has no *causal* story: "why did
this widget rebuild?" is notoriously unanswerable; rebuild counters were
bolted on. The Refresh's receipts — "one causal evidence stream" (0552 §1),
RFC 0495's causal DAG with `causedBy`, already reachable via
`exact_causal_trace` — are the structurally stronger design. The Flutter
lesson is to **productize** it: a frame timeline attributing cost per phase
(JS → plan evaluation → layout → host apply → present) with causal answers
to "why did this node relayout" would be a headline dev-loop feature
Flutter never achieved, and 0553 is its natural home. §8 ask 3.

## 5. Findings C — warnings from Flutter's hardest years

### F8. Impeller: never let the renderer compile anything at runtime, and treat the substrate contract as expensive to revise

Skia's runtime shader generation caused early-onset jank Flutter could not
fix for years; Impeller's answer — ahead-of-time compiled, enumerable
pipeline variants, no runtime shader compilation ever — required replacing
the production renderer mid-flight, roughly three years from announcement to
default across iOS and Android. Two lessons for LLP 0490:

1. **Normative from day one:** every pipeline variant enumerable and
   precompilable; nothing on the substrate generates or compiles GPU
   programs in response to app content. §8 ask 5.
2. **Sequencing:** Motion (0492) and lists (0540/0543) will lean on the
   substrate; getting its contract right before they do is cheap, and
   revising it under a live ecosystem costs what Impeller cost.

The adjacent threading lesson transfers even though the composition problem
is inverted (Flutter composited foreign platform views *into* its raster,
through four successive Android architectures; Exact composites owned GPU
surfaces into a native tree — structurally easier). Flutter's worst jank
and deadlock classes were cross-thread synchronous waits among UI, raster,
and platform threads, and their endpoint — merging threads on iOS, async
everywhere else — is LLP 0297's posture arrived at the hard way. Keep the
sanctioned-wait set tiny forever.

### F9. The app-as-data bet is more radical than Flutter's; monitor the expressiveness ceiling deliberately

Flutter kept `build()` as arbitrary imperative Dart — a huge escape valve
that meant the framework's declarative story never hit a hard ceiling.
Declarative systems with hard ceilings (SwiftUI's result builders at their
worst) generate ecosystem workarounds that ossify into folklore. Contract
makes the whole app data evaluated by one engine — stronger for
verification, receipts, reload (F5), and the agent surface, but it means
the language must grow *on purpose* wherever authors hit walls.

The 0532/0535/0539 pattern — actionSegments, forms, per-kind extensions
landing as specified language growth rather than ad-hoc escape hatches — is
the right mechanism. The metric worth instrumenting: how often real
surfaces take the `react-exception` path (LLP 0160 §5.3) *because Contract
could not express something*, versus ecosystem preference or familiarity.
The first kind is a language backlog; the second is fine. Today the log
line is free text; separating those two reasons cheaply would make the
ceiling observable. §8 ask 8.

### F10. The embedder API is why Flutter runs on car dashboards; the presenter/host-ops seam is Exact's embedder API

Flutter reaches embedded Linux, DRM/KMS consoles, and custom compositors
(eLinux, flutter-pi, automotive deployments) because the engine/embedder
contract is narrow — on the order of dozens of C-ABI functions — versioned,
and stable enough that third parties wrote embedders without Google's help.
The LLP 0406 milestone (Exact/Expose pixels through Linux DRM/KMS on the
Nothing Phone, no Android userspace in the display path) is, structurally,
"write a custom embedder."

The lesson: keep the presenter/host-ops surface deliberately narrow,
versioned, and corpus-conformance-tested, so that the wire format plus the
host-ops contract is the *entire* obligation a new host must satisfy. If
that holds, the DRM/KMS host — and every future host — is a small, boring
implementation rather than a port. §8 ask 7.

## 6. Where Flutter is simply ahead today (so the comparison stays honest)

- **Maturity and polish.** A decade shipped: hot reload mid-animation on
  device, DevTools, crash-free rendering at enormous install bases. Most
  Refresh counterparts are Accepted specs, not shipped systems; every
  "categorically ahead" claim in this document is a design claim.
- **One language, one stack.** Dart everywhere — app, framework, much of
  the tooling — means no boundary friction inside app code. Exact's
  three-layer stack (Contract / TypeScript / Rust) is more structured and
  more verifiable, but boundary friction is a real risk Flutter simply does
  not have. The Refresh's implicit answer — Contract absorbs more of the
  app so the TS boundary shrinks — should be recognized as the load-bearing
  mitigation it is.
- **Release engineering.** Stable/beta channels, a published
  breaking-change policy, and mechanical migrations (`dart fix`) let
  Flutter break APIs continuously without shedding its ecosystem. Exact's
  breaking window (LLP 0506) is a one-time version; post-window and toward
  the 2026 launch, a standing migration-tooling analog is the piece not yet
  in the corpus. §8 ask 6.
- **Ecosystem mass.** pub.dev's package depth and a decade of Stack
  Overflow answers. Exact's counterweights are the JS/TS ecosystem via the
  React tier and Hermes, and the agent-first Guide/verification loop — a
  bet that paved, machine-checked paths can substitute for folklore mass.
  That bet is plausible and unproven.

## 7. Confidence

Flutter-side facts are graded on the authoring model's knowledge through
early 2026 (§1); Exact-side mappings are graded on what was read during
authoring.

| Finding | Confidence | Basis and caveats |
|---|---|---|
| F1 uncanny-valley tail | High | Long-run, widely documented; not sensitive to recent Flutter changes |
| F2 Flutter web | High | Structural consequences of canvas rendering; renderer lineup details (CanvasKit/skwasm) could have shifted post-cutoff without changing the finding |
| F3 state vacuum | High | Decade-scale, stable observation |
| F4 test economy / semantics-as-raster | High (Flutter side) / Medium (the "nothing outside the tree" guard's enforceability — needs an owner's assessment) |
| F5 hot reload | High on Flutter's semantics and limits; High on Contract-today (read directly); Medium-High on the design comparison — 0553 §5 was read, but budgets and D3 are unlanded, so "clears the bar" is conditional |
| F6 catalog/motion | High on the Material observation; the Facet-gap specifics (which components lack baked-in motion) were not audited here |
| F7 causal receipts | High on Flutter's lack of a causal story; Medium on productization effort |
| F8 Impeller / threading | High on the shader-jank → AOT-pipelines arc and its cost; timeline figures are approximate |
| F9 expressiveness ceiling | Medium — a structural-risk argument, not an observed Exact failure; the mitigation (specified language growth) is already the program's practice |
| F10 embedder API | High on Flutter's embedder reality; "dozens of functions" is an order-of-magnitude characterization, not a count |

## 8. Advisory asks (addressed to owners; theirs to take or refuse)

*Dispositions recorded 2026-08-24 (author-decided, "all of it"): asks 1,
4, 5, and 8 TAKEN as amendments/documents; asks 2, 3, and 6 TICKETED;
ask 7 CLOSED as already covered in substance.*

1. **0553 owner:** treat D3 (keyed-region / scroll-container / motion-anchor
   preservation across a patch) as adoption-critical, not polish — it is
   the daily-felt half of Flutter's killer feature, and 0553 §5 currently
   holds it honestly open.
   **TAKEN** as LLP 0553 §5 Amendment A1 (scope-out is a deferral, not a
   discharge; a named, owned obligation survives program completion).
2. **0492/0497 owners:** land motion in Facet recipe *defaults*, so the
   catalog feels alive without author opt-in.
   **TICKETED:** `issues/20260824-facet-motion-recipe-defaults.md` —
   0497 already schedules motion-bearing components onto 0492's
   evaluator; the ticket widens that row's definition of done to default
   motion on ordinary components.
3. **0553/0495 owners:** productize the causal frame timeline (per-phase
   cost attribution + `causedBy` answers) as a headline dev-loop deliverable.
   **TICKETED:** `issues/20260824-causal-frame-timeline-devtool.md`
   (new scope; not forced into 0553's Accepted D-list).
4. **0496 owner (or a new principle):** make "no presenter behavior
   invisible to the semantics tree" a structural guard — a registered check
   or conformance rule, not vigilance.
   **TAKEN** as LLP 0560 (Principle, Draft) — review-discipline now, a
   registered 0496 check as the intended end state.
5. **0490 owner:** state the enumerable/precompilable pipeline constraint
   normatively before Motion and lists take dependencies on the substrate.
   **TAKEN** as RFC 0490's Addendum (2026-08-24): enumerable,
   precompiled/warmed pipelines on the framework path; app WGSL
   explicitly outside; golden-file testing recorded as an option.
6. **Post-window planning (0504/0506 successors):** commission the standing
   migration-tooling analog to `dart fix` plus a channel/deprecation policy
   ahead of the 2026 launch.
   **TICKETED:** `issues/20260824-post-window-migration-tooling.md`
   (post-window by 0506 D2; trigger is the window exit firing).
7. **Host-ops/presenter owners (0406 path):** treat the presenter/host-ops
   seam as a versioned embedder ABI with corpus conformance, sized so a
   DRM/KMS host is a small implementation.
   **CLOSED — already covered in substance:** LLP 0507 §2 carries
   conformance classes, a conformance suite, and golden fixtures with a
   no-conformance-before-the-schema-package precondition. Revisit at
   0406 milestone planning.
8. **0160 §5.3 stewards:** structure the react-exception log line enough to
   separate "Contract could not express it" from "preference," so the
   expressiveness ceiling (F9) is observable.
   **TAKEN** as LLP 0160 §5.3 Amendment A1 (`capability:` /
   `preference:` reason classes; legacy free text = unclassified), with
   the AGENTS.md §4/§10 and `scripts/lane-report.mjs` carriers updated.

## 9. Open questions

- **OQ1 (profile-mode analog):** 0500 D2 removes Flutter's dev/release
  fidelity gap for the engine path, but is there any remaining dev-mode
  divergence (interpreted JS vs HBC, dev-server delivery) large enough to
  warrant a named "representative mode" for performance work?
- **OQ2 (golden tests on the substrate):** should 0490 adopt Flutter-style
  golden-file testing for owned-pixel surfaces, and under which corpus
  authority?
- **OQ3 (verification of Flutter-side claims):** which findings, if any,
  become load-bearing enough for a decision that their Flutter-side facts
  should be re-verified against current sources first (§1's label)?

## Revision history

- **r1 (2026-08-24):** Initial draft, written from the 2026-08-24
  conversation with Charlie ("what can we learn from Flutter wrt the
  Refresh program"), including the F5 deep dive prompted by his follow-up
  on hot-reload parity. Exact-side reads during authoring: 0552 §1, 0553
  (header/Summary/§1/§5), 0553.001 (header), 0524 (header/Summary).
