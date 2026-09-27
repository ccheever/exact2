# Review: LLP 1051 and LLP 1051.000, 2026-09-25 (astra)

- **Family:** OpenAI — `gpt-6-astra` through `codex exec` (codex-cli 0.157.0; `model_reasoning_effort` `max`, confirmed in the run header), `--sandbox read-only`, approval never, `-C <session scratchpad>/review-wt` at `e992cefe`; 2026-09-26T01:19:26Z to 01:39:24Z UTC (2026-09-25 Pacific); 31 shell reads, three web lookups (the DOM mutation-record algorithm, CSS Scroll Anchoring; it also links CSS Transitions); 134,722 tokens; codex session `01a0db4b-b6f6-73b2-89db-4b9e207f08b0`.
- **Method:** Charlie asked for a one-round sanity check: is it directionally right, and is anything major missing, excluding nitpicks and underspecification that implementation settles. The shared brief (sha256 `032b327649f85ce5c49759f8d433014c0064defffafa04374240f54b21882372`) was the prompt, verbatim. Blind to the Grok and Claude reviews. Static inspection; no tests run.
- **Reviewed revision:** LLP 1051 and LLP 1051.000 as committed at `e992cefe` (sha256 `5f2afcfe…c675` and `2e4edf25…0a0f`), read in a detached worktree of that commit.
- **Redacted:** nothing.
- **Disposition:** LLP 1051.000 §R, the revision of 2026-09-27 (each finding taken, not taken or overtaken, with the reason). The author's summary to Charlie (2026-09-25) came first.

---

## Review (verbatim)

**Verdict:** Right with a major correction. I would build synchronous geometry queries that reuse the real layout implementation, starting with the sheet and bounded previews. Native transaction rollback is a credible implementation strategy. But the proposed general, invisible transaction does not yet hold across the browser and native text measurement. Those boundaries need narrowing or isolation before this can support the advertised tree changes. I would also separate the geometry capability from the much larger moving-obstacle physics proposal.

**Major gaps or problems**

1. **The web operation is not generally reversible.** Preventing paint does not prevent effects. In `host/web/glue.js`, property updates can navigate an iframe, and destruction tears down media, editors and motion. Refusing *moves* of sensitive subtrees does not cover those operations. Moreover, `transition: none` followed by forced style resolution cancels existing CSS transitions; restoring the declaration does not restore their timelines. Mutation observers retain records even when mutations are reversed. These follow from the [CSS transition rules](https://drafts.csswg.org/css-transitions/#starting) and [DOM mutation-record rules](https://dom.spec.whatwg.org/#queue-a-mutation-record).

   Define a supported, demonstrably reversible subset and a measurement path that excludes effectful host operations. Reject unsupported queries before touching the live tree. An isolated browser measurement tree could cover additional cases where its layout context can be reproduced, but cloning is not a universal exact fallback. This needs proving before promising that any valid kernel transaction is also a valid web hypothetical.

2. **Native rollback must extend beyond kernel writes.** I checked that `measure_height_targets` computes without publishing frames, and read its preservation tests. I also checked that `txn::apply` currently validates and mutates; it has no rollback machinery. Adding the proposed journal is reasonable, but the host measurer is not necessarily a pure function.

   In [RegionReaderMac.swift](/private/tmp/claude-501/-Users-ccheever-projects-exact2/a0c0a3ad-f4b0-43ef-8858-ec7092fd9861/scratchpad/review-wt/host/apple/Sources/ExactKit/Mac/RegionReaderMac.swift:428), measurement can replace or remove live paragraph owners and return provisional heights. Rolling back arena columns cannot restore those owners. Ordinary layout also refuses registered content regions. Text caches introduce another hazard: rolling back revision counters must not allow hypothetical metrics to match a later, different committed paragraph.

   The fix is speculation-safe measurement ownership and cache identity, plus explicit unavailable/provisional answers where exact metrics are missing. Otherwise the gigantic-Markdown use case can either disturb live presentation or return an estimate labelled exact.

3. **The Contract-to-hypothetical-tree bridge is missing.** Kernel operations are downstream of Contract state, conditional views, routes and virtualized rows. A handler asking “what would this state change produce?” needs those operations generated first. The existing [runner update path](/private/tmp/claude-501/-Users-ccheever-projects-exact2/a0c0a3ad-f4b0-43ef-8858-ec7092fd9861/scratchpad/review-wt/runner/src/runner/lists.rs:140) mutates the instance tree and ID allocator before applying kernel operations; settlement can also consult data sources. A layout context passed to the stdlib does not supply speculative realization.

   Specify a bounded, side-effect-free realization path using available data, or explicitly restrict the capability to already-realized trees. There is a useful precedent in `runner/src/instance/text.rs`, which realizes virtual rows for reading. Without this bridge, upcoming routes and unmounted-row previews remain separate substantial projects.

4. **A commit epoch does not identify geometry.** I checked `compute_layout`, `set_intrinsic_size`, `set_env` and `invalidate_text_metrics`: geometry inputs can change without advancing the commit epoch. Scroll and presentation time add host-owned changes. Consequently, an answer can pass the proposed freshness check while its wall or destination has moved.

   Queries and paths need a snapshot of the relevant geometry inputs, including offer, measurement readiness and presentation state, with invalidation for changes outside transactions. The RFC’s question about geometry-change notification is therefore essential to the moving-wall promise. It can be an internal dependency mechanism; it need not introduce unrestricted geometry feedback into application state.

5. **Arbitrary layout samples do not provide conservative collision detection.** D5’s finite-difference velocity is not a bound on motion between samples. Layout is not generally piecewise linear in an authored height: Taffy’s flex-shrink distribution, which I checked, contains ratios involving changing flex bases. Text wrapping adds discontinuities. Even linear dependence on height does not make a central difference exact when height follows a spring.

   Thus “query frames at any time” does not by itself supply the maximum closing speed and discontinuity locations required by D6. A moving wall could cross the ball between samples. Initially support static geometry and explicitly supported motion curves with defensible bounds. General layout-driven obstacles require a separate design for discontinuities, penetration and bounded prediction.

**What it gets right:** Reusing the actual layout engine, distinguishing committed and presented geometry, keeping reads out of derives, retaining the browser as the oracle, and testing hypothetical-versus-committed results are all sound. The static-wall ball example usefully demonstrates why collision time belongs to a continuous trajectory.

This was static inspection at `e992cefe`; I read relevant implementations and existing tests but did not execute tests or change files. I checked the transaction, measurement, runner, invalidation and motion-sampling claims described above. I took the broader platform survey, Preflex account, performance figures and ball’s numerical tables on trust.

**Anything you would cut:** Cut the requirement that geometry queries and motion projection become one general mechanism. Keep projection as a consumer of geometry. Defer arbitrary layout-driven moving walls and collision-triggered effects until a concrete consumer requires them. The sheet’s geometry-derived stops and photo momentum against static bounds would establish substantial value with a much smaller correctness burden.
