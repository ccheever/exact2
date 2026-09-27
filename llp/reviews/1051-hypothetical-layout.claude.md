# Review: LLP 1051 and LLP 1051.000, 2026-09-25 (claude)

- **Family:** Anthropic — Claude Opus 5.5, run as a fresh Claude Code subagent (general-purpose, with the parent session's model and effort). It had none of the authoring session's context. It was read-only by instruction and worked against `<session scratchpad>/review-wt` at `e992cefe`, on 2026-09-25 (Pacific), in about 25 minutes and 65 tool calls. It was allowed the web and checked CSS Transitions and CSS Scroll Anchoring.
- **Independence:** this is the least independent of the three reviews. It is the same model and family as the author, and the session that asked for it wrote both documents. That session is not counted as a reviewer.
- **Method:** Charlie asked for a one-round sanity check: is it directionally right, and is anything major missing, leaving out nitpicks and underspecification that implementation settles. The prompt was the shared brief (sha256 `032b327649f85ce5c49759f8d433014c0064defffafa04374240f54b21882372`), verbatim, after a preamble. The preamble named the worktree, forbade edits, builds and tests, and said the documents came from another session. The review was blind to the Astra and Grok reviews.
- **Reviewed revision:** LLP 1051 and LLP 1051.000 as committed at `e992cefe` (sha256 `5f2afcfe…c675` and `2e4edf25…0a0f`), read in a detached worktree of that commit.
- **Redacted:** nothing.
- **Disposition:** none yet. The author has not revised; see the author's summary to Charlie (2026-09-25).

---

## Review (verbatim)

**Verdict**: Right, with a major correction. The foundations hold up. There is one engine per host: the kernel on native, and the browser as the oracle on the web. Geometry reads happen only in handlers, and the compiler refuses them anywhere else. Paint-only rows carry anything continuous. The what-if goes through `txn::apply` instead of a separate overlay. I checked whether the kernel can run a what-if as a transaction that is laid out and rolled back, and it can. `txn::apply` writes a known, listable set of state: arena columns, the engine mirror, the selector index and text revisions. `measure_height_targets` already lays out without publishing and restores afterwards (kernel.rs:562-634). There are two traps. `DestroyView` frees slots and bumps their generations, so rollback must restore the exact `NodeKey`s, or express removal as detaching instead. And paragraph revisions must never be reused. The correction is above the kernel. The outline answers constraint questions well. It leaves the tree axis unbuilt for apps, even though Jordan's example was reparenting. And too much of its weight goes to physics.

**Major gaps or problems**

1. **Apps can't ask a tree what-if, and the runner can't produce one.** `what_if` takes wire ops, but actions write state. D4's "a what-if of those writes" needs the runner to settle and render a hypothetical state, then undo it. The runner's checkpoint restores slots, the store, resources and pending requests, but not the instance tree: a failed `update()` leaves the runner unusable until the host restarts it (runner/src/runner/lists.rs:133-137). Two of §9's four candidate consumers have no nodes to transact over: photo zoom's return to an unmounted source, and reorder over unmounted rows. Unmounted rows exist only in the collection's height model (`Heights`, `NeedsMeasurement`). The same gap means `layout at +300` would disagree with `clock +300; layout` whenever a timer falls due. **Fix:** make the commit itself the lookahead. Every host lays out a commit before presenting it: on native in the same call (host/apple/src/host.rs:983), and on the web by reading after the batch is applied. That gives before-and-after frames for free, which is enough for FLIP-style motion on paint-only rows. This is React's pattern (`useLayoutEffect`, which reads after commit and before paint), and also Fabric's layout animations, SwiftUI's `matchedGeometryEffect` and View Transitions. It is exact on the web and needs no revert. Keep the rolled-back pass for constraint questions on existing nodes and for agents' `layout if`. Build an always-refused runner commit, with staged instance updates, only when a consumer truly must decide before committing. F2's drift argument is about second engines, not about having more than one way to ask.

2. **The web what-if is not a no-op as specified.**
   - (a) Easing transitions are emitted as CSS `transition` (host/web/src/css.rs:72-76), and only springs run as WAAPI, so D8's premise is wrong for most motion. Under CSS Transitions §3, an inline `transition: none` followed by the forced style *cancels* a running transition. A read can therefore snap an accordion that is mid-animation, which is exactly the retargeting case. Nodes that are animating but untouched are measured at their animated values. The web would then answer "presented" where native answers "committed" or "target", and the parity fixtures would be comparing different things.
   - (b) Shrinking content inside a scrolled ancestor clamps its scroll offset during the forced layout. Scroll anchoring also applies its queued adjustments before `getBoundingClientRect()` returns; that is where the spec ends the suppression window. The inverse batch undoes neither. D8 refuses only what-ifs that *move* scrolled subtrees.
   - (c) The DOM lags the kernel within a dispatch: `advance` folds every timer that is due into one batch, applied after wasm returns (host/web/src/host.rs:625-636).
   - (d) The DOM also holds state the kernel doesn't know about: gesture overlays, WAAPI springs, collection row positions and textflow output. An inverse derived from the kernel won't restore it.

   **Fix:** snapshot the touched elements and their ancestors' scroll offsets, and restore from that snapshot. Flush pending ops before a read. Control running animations through `getAnimations()` rather than `transition: none`. Say which layout each query means. Limit v1 web what-ifs to style edits on existing nodes.

3. **Answers can be provisional or stale in ways the epoch can't detect.** Images lay out at 0×0 until the host reports their size (kernel/src/layout.rs:756-765). On macOS, a paragraph of 64 KiB or more gets an estimated height, marked pending, in ordinary layout (RegionReaderMac.swift:120-130, 430-447; Text.swift:816). `set_intrinsic_size`, `set_env` and `invalidate_text_metrics` all change layout without bumping the epoch (kernel.rs:697-785). On the web, image loads, font loads and browser-run transitions move layout with no kernel event at all, so a path's "wall changed" re-solve never gets a signal. F5's "exact by construction" holds only once inputs have settled. **Fix:** stamp each answer with a layout generation that every layout input bumps, and add a completeness flag like the content region's "miss" signal.

4. **The time half is overbuilt for its consumers.** D6–D7 amount to a physics engine: laws, root finding against moving walls, re-solving, contact and rest events, and a new timing function. They move a DEFERRED line, yet no named consumer needs bouncing (restitution). UI flings decelerate and rubber-band; they don't bounce. The sheet and the pager are snap problems. The web-standard answer to those is declarative CSS scroll snap, which exact2 already admits (schema.json:848-855). The iOS host already projects flings against snap areas taken from layout (NodeViewIOS.swift:690-707). Start there rather than with app-side `projectFling` using UIKit's constant. §2.5's claim that no platform projects against layout overlooks scroll snap.

**What it gets right**:
- The committed/presented/hypothetical vocabulary, and the failure taxonomy.
- A second engine on the web only as a checked predictor, never the answer of record.
- Folding `measure_height_targets` in and deleting its private path.
- The three invariants, with `rehydrate` as the oracle.
- The ball analysis: event times beat small steps, rest rules matter, and step independence is the property the agent clock needs.
- Sampling at presentation time. I confirmed that Apple samples motion when its display-link callback runs, not at the frame's target time (Session.swift:919-921).
- Honesty about which DEFERRED lines move.

**Anything you would cut**:
- D6–D7, and D5's central-difference velocity, into a separate RFC that waits for a consumer.
- The `path()` timing.
- `layout at` and `layout settled`, until they are defined properly or declared motion-only.
- The single cross-host velocity estimator. It is separable, and on iOS it works against "the platform recognizes".

What I checked and what I took on trust:
- **Checked in code:** the transaction and the layout pipeline, the runner's commit and failure path, web batching, CSS emission and the glue, how Apple sequences layout, samples motion and measures text, and the scroll-snap rows.
- **Checked against specs:** CSS Transitions and CSS Scroll Anchoring.
- **Taken on trust:** the survey of other systems, the physics arithmetic, the macOS hit-testing inference, browser version claims, and Preflex.

Files read: about 35 (plus two web specs).
