# LLP 1123 review: astra family (Codex `gpt-6-astra`)

Reviews of llp/1123-content-owners-and-local-safe-areas.rfc.md, with LLP 1123.001, LLP 1127 and the connected LLPs, under LLP 0005. The author and orchestrator is Claude Opus 5.5, for ide. ide asked for one adversarial round from Astra at maximum effort after the Sol loop reached consensus (`1123-content-owners-and-local-safe-areas.codex.md`, "Loop 3, round 25", READY).

## Round 1

- **Family:** astra. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6-astra`, reasoning effort `max` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T09:13:29Z to 2026-10-25T09:33:51Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** a fresh session told to review the documents as they stand before reading Sol's rounds, and then to say where it disagrees with them; the brief gave ide's goals and the implementation evidence.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `61e1945d08a21b79b1db975f790e9f944822b75ce31ce4ce1a74586961d538b1`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `65d4398722b59b727738799d30cbaa6d81d0aa936dc02ff6fc0090fbce75afbc`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D7/D9/D12 — Owner segment properties can make fitted sizing oscillate.**

   [D7’s fitted measurement](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:444) excludes percentage/fill dependencies. D12 excludes content-sized owners from query-container selection. Neither excludes D9 properties whose values depend on that owner’s resulting size.

   **Failure scenario:** Consider a bottom-anchored fitted sheet with zero clearance, bottom at y=800, maximum height at least 500, and a horizontal division at y=500–540. Its sole child has:

   ```css
   height: var(--exact-viewport-segment-height-0-0, 500px);
   ```

   At height 500, the content root occupies y=300–800. It has two segments; the first is 200 high. Fitting therefore shrinks the sheet to 200. It now occupies y=600–800, entirely on one segment. D9 consequently invalidates every segment property, activating the 500px fallback. The sheet grows again: **500 → 200 → 500**.

   This uses neither a `container:` fact nor container units, so their exclusions do not stop it. D9’s requirement to measure the content root *as laid out* prevents retaining the original values as an implicit solution. I evaluated this recurrence numerically; it is a counterexample to the specified rules, not an observed simulator failure.

   **Resolution:** Define intrinsic measurement semantics for owner properties that depend on the size being measured. Either refuse those dependencies when they contribute to fitted sizing, or specify a measurement rule that breaks the dependency consistently on every host. Cover both `navigationSizing="fitted"` and content-derived detents, including the transition between one and two segments.

2. **[major] LLP 1123 D2/D9/D12 — Top-layer custom-property reset and query-container selection are conflated.**

   [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:475) explicitly resets owner properties at dialog/popover roots. [D12](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:515) promises CSS container selection and passes units through to the browser. The composition needs an explicit rule: resetting custom properties does not itself reset query-container ancestry.

   There is already a concrete conflicting interpretation in the supplied implementation. [`env_for()`](/Users/admin/Developer/exact-owners-impl/kernel/src/arena/owners.rs:16) stops at a top-layer boundary and returns the document environment; [`ViewportUnit::basis()`](/Users/admin/Developer/exact-owners-impl/kernel/src/style/viewport.rs:78) uses that same environment for `cq*`.

   **Failure scenario:** A 300-point owner inside a 1,000-point document contains a modal dialog with `width: 50cqw`. The supplied native lookup selects the document and produces 500. The web-platform regression explicitly expects a modal dialog to retain its parent query container after `showModal()`, which gives a 150-point expectation for this fixture. CSS container units use the corresponding size-query selection rules. This is a source-derived conformance discrepancy; I did not execute the browser fixture. [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths), [top-layer query-container test](https://github.com/web-platform-tests/wpt/blob/master/css/css-conditional/container-queries/top-layer-dialog.html).

   **Resolution:** Specify query-container lookup independently of D9’s property reset, and qualify dialog, popover and fitted presented-owner boundaries against the browser oracle. Preserve CSS ancestry where applicable; if a different boundary is intentional, declare and implement that deviation on the web too. The existing popover test checks the safe-area property, so it does not settle this case.

3. **[major] LLP 1127 §6 and LLP 1123.001 §3 — Required Landmarks cases remain outside the design.**

   [1127 §6](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:251) defers the inspector, toolbar groups and split-wide search to an unspecified follow-up. [1123.001 §3](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:107) confirms that neither document specifies inspectors.

   **Failure scenario:** Duo Lab implements the proposed split and background extension, then tries to reproduce Landmarks: show the selection’s inspector, separate related toolbar actions into groups, and maintain one search across the split through collapse and expansion. There is no specified authored mapping for those operations, nor an inspector ownership and compact-presentation contract.

   These are concrete Landmarks behaviors: Apple demonstrates the inspector, `ToolbarSpacer` grouping, and search attached to the entire `NavigationSplitView`. [Apple’s Landmarks walkthrough](https://developer.apple.com/videos/play/wwdc2025/323/).

   **Resolution:** Include the missing design, or a concrete supported native-extension path, with ownership, visibility, compact adaptation and Duo Lab qualification. A separate LLP is reasonable, but its specification and acceptance criteria must be part of this work’s completion scope. Explicit deferral alone does not satisfy the requirement in this review brief.

**Where I disagree with Sol**

I disagree with **Loop 3, round 2, finding 10’s “Resolved by explicit deferral”** ([review](/Users/admin/Developer/exact-owners/llp/reviews/1123-content-owners-and-local-safe-areas.codex.md:613)). Deferral corrected the misleading inspector handoff; it did not fulfill the principal’s Landmarks requirement.

I also cannot accept round 25’s unqualified conclusion that container selection and fallback agree with CSS. The top-layer boundary above remains unqualified. The fitted-sheet cycle is a separate interaction missed by the existing exclusions; it does not invalidate the fixes to `container:` fact dependencies themselves.

**What I checked and found sound**

- The central separation is correct: document-wide `env()` values, owner-local inherited properties, and independent allocation/placement. CSS environment variables are document-global; ordinary custom properties supply the appropriate scoped mechanism. [CSS Environment Variables](https://drafts.csswg.org/css-env-1/).
- D9’s complete reset of absent segment properties prevents accidental inheritance from an enclosing owner. Registered safe-area/region properties and computed-value inheritance are coherent.
- Wrapper clearance preserves the authored box model. Stationary content, extended scrollers and background extension have distinct responsibilities; background extension now preserves the artwork’s composition.
- Keyboard policy resizing precedes residual accommodation. The distinction between scroller insets, wrapper padding, docked assistant bars and floating-keyboard reveal is consistent.
- Within the admitted two-pane arrangement scope, axis selection, division exclusion, stacking and inactivity are specified coherently. Unsupported pane dimensions are refused, and remaining UIKit measurements are explicitly required.
- The split model distinguishes router history from displayed column stacks. Selection associations, detail-base changes, cancellation and transition deduplication have explicit rules; I found no additional contradiction in that model.
- The connected amendments distinguish corrections made now from statements that must change with implementation.

I reviewed the current proposals and connected merge-base diff before consulting Sol’s rounds, then checked the capsule evidence and probe sources, selected implementation files, SDK declarations and primary standards. I changed no files and did not rerun builds, browser probes or simulator checks.