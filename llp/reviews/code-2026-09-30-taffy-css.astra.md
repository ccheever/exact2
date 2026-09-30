# Code review: LLP 1074 as landed (d8a4a0ebf), 2026-09-30 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" --sandbox read-only`, one fresh session in a checkout at `c1b94670d`.
- **Method:** mutually blind to the other family (Grok 4.7 xhigh), identical brief (sha256 `b2678b47c6b357b0026ec67afa5977a8f9b80624e1fdeb69899e29c582a3bcb2`): a static source audit of `git diff d8a4a0ebf^ d8a4a0ebf` against LLP 1074, the patch notes and CSS. Asked for by Charlie after the landing. The author (Claude) is not a reviewer.
- **Disposition:** each finding checked in the source by the author; fixes in `2e21c345a`, the rest as `issues/2026093*-*.md` (see the end of this file).

---

## Round 1 (verbatim)

The commit is not correct as landed. The main problems are a history-dependent layout under `display: none`, incomplete enforcement of the compiler’s containing-block invariant, and content-region trials that can change an absolute descendant’s containing block. There are also root-layout and cross-host painting regressions. `c1b94670d` repairs its fixture but does not address these defects. This was a source-only audit: the Chrome results below are CSS-derived expectations, not browser runs performed during this review.

1. **HIGH — Hiding a static ancestor leaves a hoisted absolute descendant eligible for layout.**

   [kernel/src/layout/hoist.rs:28](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/layout/hoist.rs:28) builds ownership without checking visibility. [taffy_tree.rs:512](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/tree/taffy_tree.rs:512) retrieves a retained static-position record and walks ancestors without checking `display: none`. The solver’s visibility check at [absolute.rs:178](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:178) checks only the absolute box itself. Hidden layout clears layouts and caches, but leaves these static-position records.

   Concrete sequence: positioned grid root `R`, 200×200; static child `S`, 100×100; `S` contains `A { position:absolute; left:300px; top:0; width:20px; height:20px }`. Lay it out, then set `S` to `display:none`. Grid processes the hidden subtree at [grid/mod.rs:669](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/grid/mod.rs:669), then the hoisted pass at line 823 lays `A` out again. It restores a nonzero frame and contributes an overflow edge at 320. A fresh tree constructed with `S` hidden has no static-position record and skips `A`. Chrome gives the hidden descendant no box or overflow contribution.

   **Resolution:** suppress hoists through hidden ancestry and invalidate the associated records correctly. Add visible→hidden→visible comparisons against fresh layout for block, flex and grid. Merely rebuilding the current ownership map is insufficient because that map also ignores hidden ancestry.

2. **HIGH — A bound `position` bypasses the compiler’s containing-block guarantee.**

   [contract/lower/src/tags.rs:105](/Users/ccheever/projects/exact2-wt-taffy-css/contract/lower/src/tags.rs:105) rejects only a literal string `"static"`. Every other authored position expression takes `Some(_) => Ok(None)`, suppressing the injected `relative`.

   Example: positioned root `R`, 400×300; child `S`, 100×100, margin-left 50, `translate="0px 0px"` and `position=(flag ? "relative" : "static")`; `S` contains an absolute child with all four insets zero. With `flag=false`, Chrome’s identity transform makes `S` the containing block: the child is 100×100 at x=50. The kernel sees static position and uses `R`: 400×300 at x=0. A conditional class supplying position has the same hole, including an absent branch that restores the static default. On a scroll container, this defeats the invariant intended to prevent absolute descendants escaping their native scroll attachment.

   **Resolution:** enforce the invariant for every possible evaluated position, including unset values and class/state expressions. Either reject unsupported expressions or enforce a consistent runtime rule. Test transitions in both directions.

3. **HIGH — Content-region validation checks branch roots, not absolute descendants.**

   The new guard at [kernel/src/region/state.rs:739](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/region/state.rs:739) checks only `b.content` and `b.pending`. [region/tree.rs:24](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/region/tree.rs:24) builds a separate tree rooted at the owner, making that owner the fallback containing block.

   Accepted input: positioned `R`, width 400; static region owner `O`, width/height 100, margin-left 50, overflow hidden on both axes; static content branch `C`; inside `C`, `A { position:absolute; left:0; top:0; width:50%; height:20px }`. Both branch roots are static, so validation passes. Ordinary layout gives `A` width 200 against `R`. The trial gives it width 50 against `O`, subsequently published at the owner’s origin. This violates the trial’s independence certificate.

   **Resolution:** require a positioned owner, or validate all descendants whose containing-block dependency could cross the region boundary. Revalidate that property after relevant style and topology changes.

4. **MEDIUM — Apple computes static flex/grid items’ `z-index` before their parent exists.**

   [NodeViewIOS.swift:750](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:750) and [NodeViewMac.swift:777](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:777) derive `usedZIndex` from the native superview’s display. Creation calls `applyStyle` before mounting: [PresenterIOS.swift:693](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:693). Mounting does not refresh the value. Mac’s `prepareToMount` handles only text and also runs before attachment.

   Concrete input: a flex row containing two overlapping static 100×100 boxes; the first is red with `z-index:2`, the second blue with `margin-left:-100px`. Chrome paints red on top. A newly created Apple view computes zero without its parent and retains zero, allowing the later blue sibling to cover it. Reparenting between block and flex/grid parents, or changing the parent’s display, can likewise leave stale values without a child style update.

   **Resolution:** compute from final logical parent facts, or refresh after attachment, reparenting and parent-display changes. Preserve the temporary reorder lift when doing so. The follow-up’s explicitly positioned text fixture does not exercise this case.

5. **MEDIUM — The JS target’s isolation decision disagrees with the live host for dynamic position.**

   [host/web-js/src/style.rs:245](/Users/ccheever/projects/exact2-wt-taffy-css/host/web-js/src/style.rs:245) treats any dynamic position binding as definitely positioned. [host/web/src/layers.rs:127](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:127) combines that fact with the template and consequently excludes the element from isolation.

   Input: a positioned container with an absolute red 100×100 box followed by a blue 100×100 box whose position is `flag ? "relative" : "static"`. With `flag=false`, the live host sees a static box after a positioned box and adds isolation, putting blue on top. The JS build has already decided against isolation; Chrome paints the absolute red box over the static blue box.

   **Resolution:** reconcile painting facts when bindings change, or emit an equivalent conditional isolation rule. A compile-time “may be positioned” fact cannot substitute for the current value in this predicate.

6. **MEDIUM — A default-static grid root loses its absolute child’s grid area.**

   [grid/mod.rs:652](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/grid/mod.rs:652) decides whether the grid contains absolutes solely from `position != Static`. For a static root, it records the content-box static position and skips passing the grid area to the solver. The kernel nevertheless assigns the child to that root; [absolute.rs:163](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:163) subsequently places it with `area=None`.

   Input: default-static grid root, 200×100, columns `100px 100px`; direct child `A { position:absolute; grid-column:2 / 3; left:0; top:0; width:20px; height:20px }`. The kernel places `A` at x=0. The web host makes this root relative, so Chrome uses the second grid area and places it at x=100.

   **Resolution:** represent the root’s effective containing-block status in the grid algorithm and retain the grid area for its direct absolute children. The current browser harness masks this by making kernel roots relative.

7. **MEDIUM — Shrink-to-fit sizing ignores the static-position inset.**

   [absolute.rs:238](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:238) computes available width from explicit insets only. It measures the box before resolving its static position at line 348.

   Input: positioned block `R`, width 400; static block `S`, margin-left 100 and width 300; absolute child `A` with left/right/width auto and wrappable content whose min-content width is 50 and max-content width is 400. Its static left position is 100. CSS substitutes that static left position before shrink-to-fit sizing, so Chrome gives width 300. This solver measures with width 400 and then places that 400-wide box at x=100. The ratio path also uses this excessive available width.

   **Resolution:** incorporate the applicable static inline inset into sizing before measurement, with the corresponding RTL handling. Cover ordinary auto width and ratio-derived size.

8. **MEDIUM — Replacing relative positioning with isolation changes compositing and descendant stacking.**

   [layers.rs:89](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:89) and [layers.rs:152](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:152) introduce a real stacking context. `position:relative; z-index:auto` did not introduce one.

   Concrete input: a positioned blue 100×100 sibling, followed by a transparent static 100×100 wrapper overlapping it, containing a red child with `mix-blend-mode:multiply`. Under the old relative wrapper, the red child can blend with the blue backdrop, producing black. Under the new isolated wrapper, that external backdrop is excluded and the child remains red.

   Descendant z-order also changes: a wrapper’s positioned `z-index:2` descendant could previously paint above a later sibling at `z-index:1`; isolation traps it inside the wrapper’s stacking context. The RFC’s flat two-box `elementFromPoint` probe cannot establish equivalence for either case.

   **Resolution:** define and implement the intended stacking/compositing semantics, and add nested-z and blending tests. The claim that this is a layout-only change with unchanged painting needs correction.

9. **MEDIUM — The web painting classifier misses stacking contexts created by static flex/grid items’ `z-index`.**

   [layers.rs:43](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:43) omits `ZIndex` from `STACKS`; `paint_of` also has no parent-display input.

   Input: a 100-wide flex row with overlapping static items: red `A`, width 100, explicitly `z-index:0`; then blue `B`, width 100, margin-left −100. Chrome treats `A` as a stacking context and paints it above the non-stacking `B`. The classifier considers `A` unlayered, so it fails to isolate `B` to preserve the declared native sibling order. Both native used z-values are zero, where later `B` wins.

   **Resolution:** account for non-auto z-index on actual flex/grid items, including parent-display changes and reparenting. Simply classifying every static block with a z-index would also be wrong.

10. **MEDIUM — A root with one auto margin is incorrectly pulled backward when it exceeds its offer.**

    [compute/mod.rs:151](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/mod.rs:151) handles negative free space specially only when both margins are auto. The single-auto branch at line 159 assigns the negative remainder to the auto margin.

    Input: LTR offer width 300; root width 400, margin-left auto, margin-right 20. The implementation calculates margin-left −120 and places the root at x=−120. CSS’s block-width algorithm treats the auto margin as zero in this over-constrained case; Chrome places it at x=0 and adjusts the end margin.

    **Resolution:** apply CSS’s over-constraint treatment before distributing free space, including the single-auto cases and their RTL counterparts. Existing oversized-root fixtures exercise only two auto margins.

11. **MEDIUM — The web’s root containing-block helper activates otherwise ignored insets.**

    [host/web/src/element.rs:55](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/element.rs:55) appends `position:relative` for every static root without neutralizing its authored insets.

    Input: default-static root, width/height 100, `left:30px; top:40px`, no margins. Kernel layout correctly ignores those insets and places it at (0,0). The emitted browser element becomes relative and moves to (30,40). An authored z-index on a static block root can similarly acquire meaning.

    **Resolution:** establish the fallback containing block without changing static inset/z-index semantics, using an appropriate wrapper or normalized helper styles. Cover both web projections and dynamically bound root position.

12. **MEDIUM — The text-flow renderer reactivates insets on newly admitted static paragraphs.**

    [kernel/src/flow.rs:105](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/flow.rs:105) now admits static boxes regardless of insets, correctly reflecting CSS. [host/web-js/flow.js:29](/Users/ccheever/projects/exact2-wt-taffy-css/host/web-js/flow.js:29) mirrors that. But [host/web/textflow-glue.js:519](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/textflow-glue.js:519) changes a flowed static paragraph to relative without clearing its insets.

    Input: positioned block context, 200×200, with a top-left absolute exclusion and an overlapping auto-height static paragraph carrying `top:50%`. The kernel ignores top and admits the paragraph at y=0. Once the web renderer installs fragments, Chrome applies relative positioning and moves it down 100px.

    **Resolution:** contain the fragments without changing the paragraph’s static positioning semantics, or neutralize and restore all affected properties. Test static paragraphs with both point and percentage insets.

13. **MEDIUM — `contextTarget` introduces host transforms absent from the compiler rule.**

    [contract/lower/src/tags.rs:69](/Users/ccheever/projects/exact2-wt-taffy-css/contract/lower/src/tags.rs:69) recognizes materials and navigation props, but not `contextTarget`. [host/web/glue.js:323](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/glue.js:323) installs a transform on that preview; related code transforms its siblings too.

    Input: absolute panel width 400; static preview width 100 with `contextTarget` and `contextMagnify=false`; preview contains an absolute child with width 50%. Even with no visible displacement and scale 1, the emitted non-`none` transform makes the preview a containing block in Chrome, giving the child width 50. The kernel retains the panel as containing block and gives width 200.

    Messages’ newly explicit relative preview avoids this particular app instance, but the general producer remains unsound.

    **Resolution:** include host-driven transform recipients in the containing-block contract, including transformed siblings, or apply presentation transforms through wrappers that preserve the authored boxes’ containing-block relationships.

14. **MEDIUM — Native context panels still use their immediate superview as the containing region.**

    [PresenterMac.swift:999](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:999) clamps the panel to `panel.superview.bounds`. iOS does the same at [PresenterIOS.swift:1028](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1028). The browser instead uses the actual offset parent at [glue.js:326](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/glue.js:326).

    Concrete tree: positioned root 400×300; static intermediary at y=100, height 50; absolute panel height 100 underneath it, containing a positioned preview with magnification disabled. Its source is at y=20. The panel’s containing block is the root. The browser can align the panel at y=20. Native clamping uses the intermediary’s bounds and forces its local top to zero, placing it at page y=100.

    **Resolution:** find the logical containing region and convert its bounds into the panel’s native parent coordinates. Walking to an absolute panel is no longer sufficient to identify that region.

15. **MEDIUM — The new verification has material blind spots that conceal these failures.**

    [browser_position.rs:82](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/browser_position.rs:82) forces every kernel root to relative, including the “static” suite. That removes the default-static root path from comparison.

    The root-ratio fixtures at [browser_position.tsv:226](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/fixtures/browser_position.tsv:226), and corresponding flex/grid groups, assert only child frames. For example, the 400-wide ratio-2 block-root case checks its 10-high child, not whether the root is 200 high. The flex cases often check only a zero-width child and barely constrain the root’s dimensions.

    [layout_equality.rs:122](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/layout_equality.rs:122) generates explicit relative or absolute positions, but never an explicit static reset. Mutations are patches, so omission does not reset an existing position. Its display generation chooses flex or omission, never `display:none` or grid.

    Additionally, [browser_cases.rs:82](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/browser_cases.rs:82) drops transform/filter declarations, the exception checker accepts any mismatch in an excepted case, and its frame comparator at line 178 does not reject NaN.

    **Resolution:** assert root frames and finite values; preserve static kernel roots while representing the browser’s root helper separately; add explicit static/hidden/grid mutations; pin the expected residual mismatches; and test the compiler/web/native paths rather than treating the geometry corpus as proof of their equivalence.

16. **LOW — The documentation retains obsolete behavior and overstates the guarantees.**

    [LLP 1010:90](/Users/ccheever/projects/exact2-wt-taffy-css/llp/1010-scrolling-v1.spec.md:90) still describes the kernel’s “missing static” and the removed web rule that adds relative positioning over absolute children, insets and z-index.

    [LLP 1001:390](/Users/ccheever/projects/exact2-wt-taffy-css/llp/1001-kernel-v1.spec.md:390) and LLP 1074 §0 claim authored static is refused on the relevant boxes, without the expression loophole in finding 2. LLP 1074’s unchanged-paint claim excludes the compositing effects in finding 8. [EXACT-PATCHES.md:661](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/EXACT-PATCHES.md:661) describes root-ratio verification more strongly than the asserted frames justify.

    [QUEUE.md:110](/Users/ccheever/projects/exact2-wt-taffy-css/QUEUE.md:110) says an exclusion under a static parent “wraps nothing”; the actual refusal applies to auto-height admission. Definite-height paragraphs can still receive shapes through [flow.rs:341](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/flow.rs:341).

    The absolute-root exception is also incompletely described: the implementation explicitly keeps its origin at zero, ignoring insets, whereas LLP 1010 mentions only content sizing. An absolute root with `left:40px; top:30px` therefore remains at (0,0), unlike the corresponding browser box. This is an existing limitation, not a newly introduced regression.

    **Resolution:** update the declarations alongside the fixes, distinguish measured cases from general guarantees, and explicitly document retained root exceptions.

I traced the ownership refresh, both hoist-path sets, boundary selection, cache keys, `mark_dirty_to`, boundary replay, presented heights and rebuild paths. Apart from the hidden-record failure above, I did not establish another incremental-versus-fresh ownership defect. Root publication and the Apple/Linux document-extent calculations do carry nonzero root origins; I found no general assumption there that every root remains at zero. I also inspected the position/z-index consumers, `Prepared.swift`, `ReorderHold`, runner collection validation and the Messages changes. The two declared ratio gaps are not counted as new findings.

I changed no files and ran no builds, tests, interpreters, browsers or images. I did not verify actual app interactions, pixel output, accessibility, drag/reorder behavior, performance, or every combination of intrinsic/replaced sizing, keywords, percentages, `calc()`, RTL, scrollbar gutters and indefinite offers. The recorded verification claims and all 29 Messages states were not independently reproduced.
416,139
The commit is not correct as landed. The main problems are a history-dependent layout under `display: none`, incomplete enforcement of the compiler’s containing-block invariant, and content-region trials that can change an absolute descendant’s containing block. There are also root-layout and cross-host painting regressions. `c1b94670d` repairs its fixture but does not address these defects. This was a source-only audit: the Chrome results below are CSS-derived expectations, not browser runs performed during this review.

1. **HIGH — Hiding a static ancestor leaves a hoisted absolute descendant eligible for layout.**

   [kernel/src/layout/hoist.rs:28](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/layout/hoist.rs:28) builds ownership without checking visibility. [taffy_tree.rs:512](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/tree/taffy_tree.rs:512) retrieves a retained static-position record and walks ancestors without checking `display: none`. The solver’s visibility check at [absolute.rs:178](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:178) checks only the absolute box itself. Hidden layout clears layouts and caches, but leaves these static-position records.

   Concrete sequence: positioned grid root `R`, 200×200; static child `S`, 100×100; `S` contains `A { position:absolute; left:300px; top:0; width:20px; height:20px }`. Lay it out, then set `S` to `display:none`. Grid processes the hidden subtree at [grid/mod.rs:669](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/grid/mod.rs:669), then the hoisted pass at line 823 lays `A` out again. It restores a nonzero frame and contributes an overflow edge at 320. A fresh tree constructed with `S` hidden has no static-position record and skips `A`. Chrome gives the hidden descendant no box or overflow contribution.

   **Resolution:** suppress hoists through hidden ancestry and invalidate the associated records correctly. Add visible→hidden→visible comparisons against fresh layout for block, flex and grid. Merely rebuilding the current ownership map is insufficient because that map also ignores hidden ancestry.

2. **HIGH — A bound `position` bypasses the compiler’s containing-block guarantee.**

   [contract/lower/src/tags.rs:105](/Users/ccheever/projects/exact2-wt-taffy-css/contract/lower/src/tags.rs:105) rejects only a literal string `"static"`. Every other authored position expression takes `Some(_) => Ok(None)`, suppressing the injected `relative`.

   Example: positioned root `R`, 400×300; child `S`, 100×100, margin-left 50, `translate="0px 0px"` and `position=(flag ? "relative" : "static")`; `S` contains an absolute child with all four insets zero. With `flag=false`, Chrome’s identity transform makes `S` the containing block: the child is 100×100 at x=50. The kernel sees static position and uses `R`: 400×300 at x=0. A conditional class supplying position has the same hole, including an absent branch that restores the static default. On a scroll container, this defeats the invariant intended to prevent absolute descendants escaping their native scroll attachment.

   **Resolution:** enforce the invariant for every possible evaluated position, including unset values and class/state expressions. Either reject unsupported expressions or enforce a consistent runtime rule. Test transitions in both directions.

3. **HIGH — Content-region validation checks branch roots, not absolute descendants.**

   The new guard at [kernel/src/region/state.rs:739](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/region/state.rs:739) checks only `b.content` and `b.pending`. [region/tree.rs:24](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/region/tree.rs:24) builds a separate tree rooted at the owner, making that owner the fallback containing block.

   Accepted input: positioned `R`, width 400; static region owner `O`, width/height 100, margin-left 50, overflow hidden on both axes; static content branch `C`; inside `C`, `A { position:absolute; left:0; top:0; width:50%; height:20px }`. Both branch roots are static, so validation passes. Ordinary layout gives `A` width 200 against `R`. The trial gives it width 50 against `O`, subsequently published at the owner’s origin. This violates the trial’s independence certificate.

   **Resolution:** require a positioned owner, or validate all descendants whose containing-block dependency could cross the region boundary. Revalidate that property after relevant style and topology changes.

4. **MEDIUM — Apple computes static flex/grid items’ `z-index` before their parent exists.**

   [NodeViewIOS.swift:750](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:750) and [NodeViewMac.swift:777](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:777) derive `usedZIndex` from the native superview’s display. Creation calls `applyStyle` before mounting: [PresenterIOS.swift:693](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:693). Mounting does not refresh the value. Mac’s `prepareToMount` handles only text and also runs before attachment.

   Concrete input: a flex row containing two overlapping static 100×100 boxes; the first is red with `z-index:2`, the second blue with `margin-left:-100px`. Chrome paints red on top. A newly created Apple view computes zero without its parent and retains zero, allowing the later blue sibling to cover it. Reparenting between block and flex/grid parents, or changing the parent’s display, can likewise leave stale values without a child style update.

   **Resolution:** compute from final logical parent facts, or refresh after attachment, reparenting and parent-display changes. Preserve the temporary reorder lift when doing so. The follow-up’s explicitly positioned text fixture does not exercise this case.

5. **MEDIUM — The JS target’s isolation decision disagrees with the live host for dynamic position.**

   [host/web-js/src/style.rs:245](/Users/ccheever/projects/exact2-wt-taffy-css/host/web-js/src/style.rs:245) treats any dynamic position binding as definitely positioned. [host/web/src/layers.rs:127](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:127) combines that fact with the template and consequently excludes the element from isolation.

   Input: a positioned container with an absolute red 100×100 box followed by a blue 100×100 box whose position is `flag ? "relative" : "static"`. With `flag=false`, the live host sees a static box after a positioned box and adds isolation, putting blue on top. The JS build has already decided against isolation; Chrome paints the absolute red box over the static blue box.

   **Resolution:** reconcile painting facts when bindings change, or emit an equivalent conditional isolation rule. A compile-time “may be positioned” fact cannot substitute for the current value in this predicate.

6. **MEDIUM — A default-static grid root loses its absolute child’s grid area.**

   [grid/mod.rs:652](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/grid/mod.rs:652) decides whether the grid contains absolutes solely from `position != Static`. For a static root, it records the content-box static position and skips passing the grid area to the solver. The kernel nevertheless assigns the child to that root; [absolute.rs:163](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:163) subsequently places it with `area=None`.

   Input: default-static grid root, 200×100, columns `100px 100px`; direct child `A { position:absolute; grid-column:2 / 3; left:0; top:0; width:20px; height:20px }`. The kernel places `A` at x=0. The web host makes this root relative, so Chrome uses the second grid area and places it at x=100.

   **Resolution:** represent the root’s effective containing-block status in the grid algorithm and retain the grid area for its direct absolute children. The current browser harness masks this by making kernel roots relative.

7. **MEDIUM — Shrink-to-fit sizing ignores the static-position inset.**

   [absolute.rs:238](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/common/absolute.rs:238) computes available width from explicit insets only. It measures the box before resolving its static position at line 348.

   Input: positioned block `R`, width 400; static block `S`, margin-left 100 and width 300; absolute child `A` with left/right/width auto and wrappable content whose min-content width is 50 and max-content width is 400. Its static left position is 100. CSS substitutes that static left position before shrink-to-fit sizing, so Chrome gives width 300. This solver measures with width 400 and then places that 400-wide box at x=100. The ratio path also uses this excessive available width.

   **Resolution:** incorporate the applicable static inline inset into sizing before measurement, with the corresponding RTL handling. Cover ordinary auto width and ratio-derived size.

8. **MEDIUM — Replacing relative positioning with isolation changes compositing and descendant stacking.**

   [layers.rs:89](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:89) and [layers.rs:152](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:152) introduce a real stacking context. `position:relative; z-index:auto` did not introduce one.

   Concrete input: a positioned blue 100×100 sibling, followed by a transparent static 100×100 wrapper overlapping it, containing a red child with `mix-blend-mode:multiply`. Under the old relative wrapper, the red child can blend with the blue backdrop, producing black. Under the new isolated wrapper, that external backdrop is excluded and the child remains red.

   Descendant z-order also changes: a wrapper’s positioned `z-index:2` descendant could previously paint above a later sibling at `z-index:1`; isolation traps it inside the wrapper’s stacking context. The RFC’s flat two-box `elementFromPoint` probe cannot establish equivalence for either case.

   **Resolution:** define and implement the intended stacking/compositing semantics, and add nested-z and blending tests. The claim that this is a layout-only change with unchanged painting needs correction.

9. **MEDIUM — The web painting classifier misses stacking contexts created by static flex/grid items’ `z-index`.**

   [layers.rs:43](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/layers.rs:43) omits `ZIndex` from `STACKS`; `paint_of` also has no parent-display input.

   Input: a 100-wide flex row with overlapping static items: red `A`, width 100, explicitly `z-index:0`; then blue `B`, width 100, margin-left −100. Chrome treats `A` as a stacking context and paints it above the non-stacking `B`. The classifier considers `A` unlayered, so it fails to isolate `B` to preserve the declared native sibling order. Both native used z-values are zero, where later `B` wins.

   **Resolution:** account for non-auto z-index on actual flex/grid items, including parent-display changes and reparenting. Simply classifying every static block with a z-index would also be wrong.

10. **MEDIUM — A root with one auto margin is incorrectly pulled backward when it exceeds its offer.**

    [compute/mod.rs:151](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/src/compute/mod.rs:151) handles negative free space specially only when both margins are auto. The single-auto branch at line 159 assigns the negative remainder to the auto margin.

    Input: LTR offer width 300; root width 400, margin-left auto, margin-right 20. The implementation calculates margin-left −120 and places the root at x=−120. CSS’s block-width algorithm treats the auto margin as zero in this over-constrained case; Chrome places it at x=0 and adjusts the end margin.

    **Resolution:** apply CSS’s over-constraint treatment before distributing free space, including the single-auto cases and their RTL counterparts. Existing oversized-root fixtures exercise only two auto margins.

11. **MEDIUM — The web’s root containing-block helper activates otherwise ignored insets.**

    [host/web/src/element.rs:55](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/src/element.rs:55) appends `position:relative` for every static root without neutralizing its authored insets.

    Input: default-static root, width/height 100, `left:30px; top:40px`, no margins. Kernel layout correctly ignores those insets and places it at (0,0). The emitted browser element becomes relative and moves to (30,40). An authored z-index on a static block root can similarly acquire meaning.

    **Resolution:** establish the fallback containing block without changing static inset/z-index semantics, using an appropriate wrapper or normalized helper styles. Cover both web projections and dynamically bound root position.

12. **MEDIUM — The text-flow renderer reactivates insets on newly admitted static paragraphs.**

    [kernel/src/flow.rs:105](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/flow.rs:105) now admits static boxes regardless of insets, correctly reflecting CSS. [host/web-js/flow.js:29](/Users/ccheever/projects/exact2-wt-taffy-css/host/web-js/flow.js:29) mirrors that. But [host/web/textflow-glue.js:519](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/textflow-glue.js:519) changes a flowed static paragraph to relative without clearing its insets.

    Input: positioned block context, 200×200, with a top-left absolute exclusion and an overlapping auto-height static paragraph carrying `top:50%`. The kernel ignores top and admits the paragraph at y=0. Once the web renderer installs fragments, Chrome applies relative positioning and moves it down 100px.

    **Resolution:** contain the fragments without changing the paragraph’s static positioning semantics, or neutralize and restore all affected properties. Test static paragraphs with both point and percentage insets.

13. **MEDIUM — `contextTarget` introduces host transforms absent from the compiler rule.**

    [contract/lower/src/tags.rs:69](/Users/ccheever/projects/exact2-wt-taffy-css/contract/lower/src/tags.rs:69) recognizes materials and navigation props, but not `contextTarget`. [host/web/glue.js:323](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/glue.js:323) installs a transform on that preview; related code transforms its siblings too.

    Input: absolute panel width 400; static preview width 100 with `contextTarget` and `contextMagnify=false`; preview contains an absolute child with width 50%. Even with no visible displacement and scale 1, the emitted non-`none` transform makes the preview a containing block in Chrome, giving the child width 50. The kernel retains the panel as containing block and gives width 200.

    Messages’ newly explicit relative preview avoids this particular app instance, but the general producer remains unsound.

    **Resolution:** include host-driven transform recipients in the containing-block contract, including transformed siblings, or apply presentation transforms through wrappers that preserve the authored boxes’ containing-block relationships.

14. **MEDIUM — Native context panels still use their immediate superview as the containing region.**

    [PresenterMac.swift:999](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:999) clamps the panel to `panel.superview.bounds`. iOS does the same at [PresenterIOS.swift:1028](/Users/ccheever/projects/exact2-wt-taffy-css/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1028). The browser instead uses the actual offset parent at [glue.js:326](/Users/ccheever/projects/exact2-wt-taffy-css/host/web/glue.js:326).

    Concrete tree: positioned root 400×300; static intermediary at y=100, height 50; absolute panel height 100 underneath it, containing a positioned preview with magnification disabled. Its source is at y=20. The panel’s containing block is the root. The browser can align the panel at y=20. Native clamping uses the intermediary’s bounds and forces its local top to zero, placing it at page y=100.

    **Resolution:** find the logical containing region and convert its bounds into the panel’s native parent coordinates. Walking to an absolute panel is no longer sufficient to identify that region.

15. **MEDIUM — The new verification has material blind spots that conceal these failures.**

    [browser_position.rs:82](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/browser_position.rs:82) forces every kernel root to relative, including the “static” suite. That removes the default-static root path from comparison.

    The root-ratio fixtures at [browser_position.tsv:226](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/fixtures/browser_position.tsv:226), and corresponding flex/grid groups, assert only child frames. For example, the 400-wide ratio-2 block-root case checks its 10-high child, not whether the root is 200 high. The flex cases often check only a zero-width child and barely constrain the root’s dimensions.

    [layout_equality.rs:122](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/layout_equality.rs:122) generates explicit relative or absolute positions, but never an explicit static reset. Mutations are patches, so omission does not reset an existing position. Its display generation chooses flex or omission, never `display:none` or grid.

    Additionally, [browser_cases.rs:82](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/tests/it/browser_cases.rs:82) drops transform/filter declarations, the exception checker accepts any mismatch in an excepted case, and its frame comparator at line 178 does not reject NaN.

    **Resolution:** assert root frames and finite values; preserve static kernel roots while representing the browser’s root helper separately; add explicit static/hidden/grid mutations; pin the expected residual mismatches; and test the compiler/web/native paths rather than treating the geometry corpus as proof of their equivalence.

16. **LOW — The documentation retains obsolete behavior and overstates the guarantees.**

    [LLP 1010:90](/Users/ccheever/projects/exact2-wt-taffy-css/llp/1010-scrolling-v1.spec.md:90) still describes the kernel’s “missing static” and the removed web rule that adds relative positioning over absolute children, insets and z-index.

    [LLP 1001:390](/Users/ccheever/projects/exact2-wt-taffy-css/llp/1001-kernel-v1.spec.md:390) and LLP 1074 §0 claim authored static is refused on the relevant boxes, without the expression loophole in finding 2. LLP 1074’s unchanged-paint claim excludes the compositing effects in finding 8. [EXACT-PATCHES.md:661](/Users/ccheever/projects/exact2-wt-taffy-css/vendor/taffy/EXACT-PATCHES.md:661) describes root-ratio verification more strongly than the asserted frames justify.

    [QUEUE.md:110](/Users/ccheever/projects/exact2-wt-taffy-css/QUEUE.md:110) says an exclusion under a static parent “wraps nothing”; the actual refusal applies to auto-height admission. Definite-height paragraphs can still receive shapes through [flow.rs:341](/Users/ccheever/projects/exact2-wt-taffy-css/kernel/src/flow.rs:341).

    The absolute-root exception is also incompletely described: the implementation explicitly keeps its origin at zero, ignoring insets, whereas LLP 1010 mentions only content sizing. An absolute root with `left:40px; top:30px` therefore remains at (0,0), unlike the corresponding browser box. This is an existing limitation, not a newly introduced regression.

    **Resolution:** update the declarations alongside the fixes, distinguish measured cases from general guarantees, and explicitly document retained root exceptions.

I traced the ownership refresh, both hoist-path sets, boundary selection, cache keys, `mark_dirty_to`, boundary replay, presented heights and rebuild paths. Apart from the hidden-record failure above, I did not establish another incremental-versus-fresh ownership defect. Root publication and the Apple/Linux document-extent calculations do carry nonzero root origins; I found no general assumption there that every root remains at zero. I also inspected the position/z-index consumers, `Prepared.swift`, `ReorderHold`, runner collection validation and the Messages changes. The two declared ratio gaps are not counted as new findings.

I changed no files and ran no builds, tests, interpreters, browsers or images. I did not verify actual app interactions, pixel output, accessibility, drag/reorder behavior, performance, or every combination of intrinsic/replaced sizing, keywords, percentages, `calc()`, RTL, scrollbar gutters and indefinite offers. The recorded verification claims and all 29 Messages states were not independently reproduced.

---

## Disposition (the author, 2026-09-30)

Each finding was checked in the source. Fixed in the follow-up commit: 1 (a hoisted box under a hidden ancestor is laid out by nobody; `layout_equality` proves it against a fresh tree — the test fails at `origin/main` before the fix), 2 (a bound `position` on a containing box is refused unless every value it can take is positioned), 3 (a region's owner must be positioned), 4 (Apple reads the used z-index again once mounted), 6 and 11 together (a static root is lowered `relative` in the kernel, as the web's root element is, so both apply its insets), 10 (one auto margin on an over-constrained root is zero), 13 in part (`contextTarget` contains), 15 in part (the fixtures assert root frames, which found that a flex or grid root shorter than its ratio's height was not laid out at the floor; the differential draws explicit static resets, and a `display: none` draw found a pre-existing hidden-subtree bug), 16 (LLP 1010 §2 and the QUEUE gap list). Tickets: 5 → `issues/closed/20260930-web-js-paint-facts-disagree-with-the-live-host.md`; 7 → `absolute-shrink-to-fit-ignores-the-static-inset`; 8 → `web-isolation-is-a-stacking-context`; 9 → `web-paint-classifier-misses-flex-item-z-index`; 12 → `textflow-glue-reactivates-static-insets`; 13 (siblings, raised rows) → `host-transforms-outside-the-containing-block-rule`; 14 → `native-context-panels-clamp-to-the-superview`; 15 (the rest) → `position-fixtures-do-not-run-a-static-root`; 16 (the absolute root) → `absolute-root-ignores-its-insets`; the hidden-subtree bug → `hidden-subtree-keeps-stale-frames`.
