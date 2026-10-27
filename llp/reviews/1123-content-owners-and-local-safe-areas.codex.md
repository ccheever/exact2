# LLP 1123 review: codex family (Codex `gpt-6.1-sol`)

Reviews of llp/1123-content-owners-and-local-safe-areas.rfc.md under LLP 0005 / LLP 0010.001 (super-refine). The author/reviser is Claude Fable 5.1, for ide; the orchestrator is Claude Opus 5.5. Per the owner's instruction, the codex family is the only reviewer family in this loop. Each round's body is verbatim.

## Round 1

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` 2026-10-08, model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** fresh session with the repository read-only. Instructed not to read `llp/reviews/` or the orchestrator's notes; that blindness is instruction-based and self-attested.
- **Target revision:** sha256 `85c0a44ad43bcee6e9438f1891438e1827bcadd77484ce041d4a9e42fa483b4d`.
- **Prompt:** `/tmp/af/r1.md`.
- **Verdict:** NOT READY.

Codex-family review, round 1 · OpenAI/Codex · 2026-10-08 · Unredacted  
Target: [LLP 1123](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md), revision `85c0a44ad43bcee6e9438f1891438e1827bcadd77484ce041d4a9e42fa483b4d`.

Independent, read-only review using the repository and primary documentation. Blindness is instruction-based and self-attested; no content from `llp/reviews/` or `/tmp/af/` was read.

**Overall assessment.** The proposal has a sound architecture and substantial platform coverage. The settled choices fit together: document facts remain session-wide, owners supply local geometry, and clearance sits outside the authored box model. However, D3 does not yet give a coherent, deterministic credit calculation across the expressions D5 and D9 admit, and D9 specifies behavior that differs from its browser implementation. Those are observable correctness concerns requiring revision.

**Strengths.**

- **D1–D2:** Separating document geometry, owner allocation, logical ancestry and rendering ancestry directly addresses the sheet-resizes-background defect. Publication, cancellation, retirement and rebuild behavior receive appropriate attention.
- **D3–D4:** The additive model, explicit credit, per-edge opt-out and preservation of authored box sizing form a useful author-facing contract. All 13 table results match the stated arithmetic; the 633/665-point box-sizing examples are correct.
- **D6–D8:** The feedback fence, consumer requirement, numeric geometry and authored readable width respect the related LLPs and avoid speculative API expansion.
- **Staged plan and Tests:** The complete iOS slice, production web chrome, transition-time coordinate tests and proof-before-deletion requirements are strong. These test observable behavior rather than merely successful compilation.

**Apple, HIG and CSS verification.** I checked the 73 unique Apple URLs. None produced a confirmed broken link, including the constructed SwiftUI overload paths and Duo Tech Talks. Some API pages expose JavaScript shells; claim verification used Apple’s indexed documentation, HIG structured content and video transcripts.

The principal Apple claims are supported:

- The tvOS figures are correct: **60 points vertically and 80 horizontally**. These are HIG recommendations; implementation should continue measuring `safeAreaInsets`, rather than hardcoding those numbers. [Apple HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout#tvOS)
- `.automatic` generally resolves to `.formSheet` from iOS 18 onward, and `viewIsAppearing` occurs after the view has established its geometry and safe area, before appearance. [Automatic presentation](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/automatic), [View-controller appearance](https://developer.apple.com/documentation/uikit/displaying-and-managing-views-with-a-view-controller)
- Custom detent heights exclude the bottom safe area; the dismissed keyboard guide can retain a safe-area baseline. [WWDC22 UIKit](https://developer.apple.com/videos/play/wwdc2022/10068/), [WWDC23 keyboard](https://developer.apple.com/videos/play/wwdc2023/10281/)
- The Duo talks substantiate arrangements as layout rather than navigation, and standard-sheet adaptation around a partial fold. [Adaptive layouts](https://developer.apple.com/videos/play/tech-talks/111463/), [Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/)

The Safari simulator measurements remain author-supplied observations; I did not independently reproduce them.

The document-wide `env()` distinction, top-layer ancestry behavior, containing-block examples and `clamp(100px, 20px, 50px) = 100px` are consistent with the cited CSS specifications. The two CSS discrepancies below need correction.

**Concerns.**

1. **MATERIAL — D3’s credit rule is not coherent across the admitted expression grammar.**

   The baseline calculation and subsequent containing-block shrink can invalidate the claimed clearance. Consider an allocation width of 100, right `local = 34`, document right inset 34, and:

   ```css
   padding-right: min(20%, env(safe-area-inset-right));
   ```

   Under the stated baseline excluding injected clearance, authored padding is 20, credit is 20, and wrapper clearance is 14. D3 then makes the containing block 86 wide. CSS padding consequently becomes 17.2, leaving actual clearance of **31.2**, below the required 34. Freezing the authored padding at 20 instead contradicts the final containing-block percentage basis. [CSS percentage padding](https://www.w3.org/TR/CSS2/box.html#padding-properties)

   There is also an ambiguity when both eligible sources occur in a linear expression. With document inset 10 and owner inset 34:

   ```css
   padding-right:
     calc(env(safe-area-inset-right) + var(--exact-safe-area-inset-right));
   ```

   Authored padding is 44. Taking `X` as `env()` gives credit 10 and used inset 68; taking `X` as the custom property gives credit 34 and used inset 44. Both match the current wording.

   **Resolution:** Define one deterministic credit calculation over the admitted expressions, including mixed sources and percentage-dependent values. It must preserve the final CSS percentage basis, the required local clearance and idempotence. Add these two examples with agreed expected results. This is a correction to the arithmetic contract, not a demand for a particular solver or storage design.

2. **MATERIAL — D9’s invalid-value behavior contradicts its registered CSS semantics.**

   D9 registers inherited `<length>` properties, but says a cycle reads the initial value, and Tests requires `0px`. For such registered properties, invalid-at-computed-value-time behavior is effectively `unset`: an inherited property takes its inherited value. Only when inheritance cannot supply one does the initial value apply. [CSS Variables §3.1](https://www.w3.org/TR/css-variables-1/#invalid-variables)

   For example, beneath a parent whose bottom property is 34px, mutually referencing top and bottom overrides form a cycle. The child’s bottom property inherits 34px; D9 currently requires the native implementation to return zero while the browser resolves the authored text differently.

   D9 also says the host supplies values on the **authored root**, then says the web supplies them on the **wrapper**. That distinction affects inheritance when an authored root declaration becomes invalid.

   **Resolution:** Correct cycle handling and the zero-valued test. Specify consistent host-default placement and author precedence across targets. Native resolution must inherit registered **computed values**, including resolving relative lengths at the declaring element, as the browser does. [Properties and Values API §2.7](https://drafts.css-houdini.org/css-properties-values-api/#substitution)

3. **minor — D5 incorrectly says `none` is unavailable in these functions.**

   CSS Values 4 permits `none` for either outer argument of `clamp()`. It does not permit it as an ordinary argument to `min()` or `max()`. [CSS Values 4 §10.2](https://www.w3.org/TR/css-values-4/#funcdef-clamp)

   **Resolution:** Admit the CSS endpoint syntax, or explicitly describe its exclusion as an Exact subset. Do not attribute the exclusion to CSS.

4. **minor — The scroller executor needs clearer wording about effective clearance.**

   UIKit’s automatic adjustment adds safe-area contributions to `contentInset`; selecting `.scrollableAxes` or `.always` does not itself express D3’s credited residual or D4’s edge mask. A full-bounds primary scroller can otherwise receive native clearance in addition to padding already credited by D3. [UIKit adjusted content inset](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset)

   **Resolution:** State that the scroller’s effective adjustment must produce the same residual and opt-out result as the wrapper executor. The compensation technique is implementation-phase material; verify credited padding, opt-outs, short content and changing large-title insets during stage 2.

5. **minor — Several statements need narrower wording or explicit amendments.**

   - D2 publishes throughout transitions; D9 says values change “never mid-transition.” Clarify that each coherent transition-time commit updates D9.
   - “tvOS has no sheets” is too broad: Apple’s HIG discusses modal sheets on tvOS. Describe Exact’s full-screen mapping and the absence of the iOS detent-sheet mechanism. [HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets)
   - D3’s color-only propagation is a chosen analogy. CSS root-background propagation also includes background images. Label the narrower behavior accurately. [CSS root backgrounds](https://www.w3.org/TR/css-backgrounds-3/#root-background)
   - Add **LLP 1035.001 D4–D5** to the amendment list: their viewport-moving and active-container measurement language must be reconciled with D1–D2.
   - Clarify that stacking selects the active owner; it does not replace document order or platform focus behavior within that owner.

**Suggestions and implementation obligations.** Preserve the staged plan. Add focused conformance cases for the material findings rather than expanding the RFC into an implementation manual. During fitted-sizing implementation, verify that UIKit’s excluded bottom-safe-area detent units do not cause clearance to be added twice. Assign the implementer before turning this proposal into the operative implementation specification.

The authorized first-class attributes and D9 custom properties are consistent with the owner’s direction and LLP 1081. Consumer-gated geometry and deferred local fold segments fit LLP 1039 and LLP 1078. I do not treat the expressly authorized narrow admissions as new DEFERRED violations.

**Open questions.** Q1’s recommendation to retain `page` and assess a later default change separately is reasonable. Q2 naturally follows the owner model. Q3’s consumer-gated keyboard variables are also reasonable. None requires reopening the settled architecture.

**Recommended next step.** Revise D3’s credit calculation and D9’s registered-property semantics, correct the minor statements, and obtain the next blind review on the resulting revision. The remaining API recipes and implementation mechanics should stay implementation obligations.

VERDICT: NOT READY

## Round 2

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` 2026-10-08, model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** a fresh session, blind to round 1. Instructed not to read `llp/reviews/` or the orchestrator's notes; that blindness is self-attested.
- **Target revision:** sha256 `c6fe79561617c3b26e0860cd071f35fbe97e69b05ac026e78139207ee8639520`.
- **Prompt:** `/tmp/af/r2.md`.
- **Verdict:** NOT READY.

Codex-family independent review, round 2 · 2026-10-08 · read-only · instruction-based blindness, self-attested. Prior review artifacts and `/tmp/af/` were not read.

Target: [LLP 1123](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md)  
SHA-256, verified unchanged: `c6fe79561617c3b26e0860cd071f35fbe97e69b05ac026e78139207ee8639520`

The proposal has a sound overall direction and preserves the owner’s settled decisions. Two material issues remain: the percentage calculation conflicts with the scroller executor, and the container fact’s feedback fence does not prevent feedback through authored chrome. Both can be corrected without redesigning content owners.

The strongest parts are:

- **D1–D2:** separating session-wide document facts from local allocations, and separating logical ancestry from rendering ancestry. The treatment of containment, cancellation, generation identity, coordinates and recovery addresses the consequences of moving content into separate native mounts.
- **D3–D4:** a concrete credit rule, explicit box-sizing behavior, and an opt-out that changes clearance rather than allocation. Removing the existing authored-width subtraction follows CSS.
- **D5 and D9:** CSS comparison functions and registered inherited custom properties provide suitable authoring vocabulary. D9 explicitly handles computed inheritance, overrides and invalid declarations.
- **Staged plan and Tests:** the narrow-sheet/wide-document proof, transition-time geometry checks, production web chrome and preservation of workarounds until their replacements pass are appropriate acceptance criteria.

The external-source and arithmetic checks support most of the proposal:

- The constructed Apple links I checked resolved, including the trait, keyboard, SwiftUI presentation and Duo documentation paths. The two Duo Tech Talk links identify the relevant arrangement and sheet behavior talks. The documentation also confirms that reserved-region frames already include their margins. These remain beta API claims, as the proposal acknowledges. [Reserved-region frame](https://developer.apple.com/documentation/uikit/uiview/reservedregion/frame), [adaptive layouts](https://developer.apple.com/videos/play/tech-talks/111463/), [Duo sheet behavior](https://developer.apple.com/videos/play/tech-talks/111466/).
- **The tvOS figures are correct:** the HIG recommends 60 points at the top and bottom and 80 at the sides. These are guidance figures; sampling `safeAreaInsets` rather than baking those constants is the correct design. [HIG Layout](https://developer.apple.com/design/human-interface-guidelines/layout).
- Apple confirms that `viewIsAppearing` receives accurate view geometry and updated traits, that a transformed view’s `frame` is undefined, and that custom-detent heights exclude the bottom safe area when edge-attached. The stated `.automatic` mapping to `.formSheet` on iOS 18 and later is also documented. [Appearance callback](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewisappearing(_:)), [UIView frame](https://developer.apple.com/documentation/uikit/uiview/frame), [custom detents](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/detent/custom(identifier:resolver:)), [automatic presentation](https://developer.apple.com/documentation/uikit/uimodalpresentationstyle/automatic).
- D5 correctly gives the minimum argument precedence in `clamp()`, permits `none` at either outer argument, and requires consistent types. The 32-value/nesting figures are minimum implementation support requirements in CSS; choosing them as Exact’s limits is consistent with the existing parser’s declared subset. [CSS Values 4](https://www.w3.org/TR/css-values-4/#calc-notation).
- D9’s typed computed inheritance and `unset` behavior for an invalid registered, inherited property agree with the cited standards. The top-layer ancestor-clipping and `display: none` claims also check out. [CSS Variables §3.1](https://www.w3.org/TR/css-variables-1/#invalid-variables), [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api/), [CSS top layer](https://drafts.csswg.org/css-position-4/#top-layer).
- I recalculated every D3 table row. All agree with the stated formula. The percentage example gives clearance **17.5**, containing-block width **82.5**, padding **16.5**, and total inset **34**. Percentage padding using containing-block width on all four edges is CSS behavior. [CSS padding](https://www.w3.org/TR/CSS2/box.html#padding-properties).

The concerns are:

1. **MATERIAL — D3’s percentage equation contradicts the horizontal-scroller executor.**

   The equation fixes `W = A − c_left − c_right`, because side clearance is wrapper padding. But “One executor per edge” makes wrapper padding zero on a primary scroller’s scrolling axis.

   Apply the worked example to a horizontal primary scroller: allocation width 100, right inset 34, and root padding `min(20%, env(safe-area-inset-right))`. With zero right wrapper padding, the root’s containing block is 100, so CSS resolves the padding to **20**, and the credited residual clearance is **14**. The unconditional fixed-point equation instead requires a containing block of **82.5**, padding **16.5**, and clearance **17.5**.

   Two implementers cannot satisfy both prescriptions. This changes authored layout and padding, not merely the technique used to compensate UIKit.

   **Resolution:** distinguish residual clearance from the portion executed as wrapper padding. Resolve percentages against the actual containing block after wrapper-executed clearance only. State the result for horizontal and bidirectional primary scrollers, and extend the percentage fixture accordingly. Alternatively, explicitly restrict the primary-scroller rule’s admitted axes.

2. **MATERIAL — D6’s feedback fence is insufficient for the values it publishes.**

   Refusing fitted owners and content-derived detents does not make all reported geometry independent of authored content. `safeTop`, for example, includes the native bar projected from the authored header.

   An owner can retain fixed bounds while a branch on `safeTop` selects a level-1 heading when the inset is small and an inline heading when it is large. The existing [bar projection](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:517) maps heading level to large-title mode. That permits a cycle such as **106 → inline title → 54 → large title → 106**, without any window resize or content-sized owner. A branch choosing `form` versus `page` from the same owner’s reported width creates another possible feedback path.

   Thus “nothing it reports can change what it reports” does not follow from the proposed admission rule. This matters specifically because D6 amends LLP 1039 D5’s refusal of layout feedback.

   **Resolution:** require independence of the reported allocation and chrome from the fact’s dependent branches, including presentation sizing and projected bar configuration. State what is refused when that condition is violated. Deferring D6 until a consumer demonstrates that fence is also consistent with its stated landing condition. No particular compiler or ABI implementation is required at this altitude.

3. **minor — The tvOS presentation row overstates its HIG attribution.**

   The Sheets HIG says tvOS sheets are modal; its explicit full-screen-sheet description applies to watchOS. Exact’s `.overFullScreen` mapping may be entirely appropriate, but the cited page does not establish the universal claim “a tvOS sheet is a full-screen modal.” [HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets).

   **Resolution:** separate Apple’s modality guidance from Exact’s chosen presentation mapping.

4. **minor — D3’s artwork recipe needs its containing block stated.**

   Merely positioning a layer absolutely inside an authored root does not guarantee that it reaches the clearance region. A positioned root that has already been narrowed by wrapper clearance gives that layer a smaller containing block.

   **Resolution:** provide one working recipe specifying the containing block and offsets, or show the D4 opt-out with foreground clearance authored separately. This is author-guide material.

5. **minor — Implementation assignment and scope recording remain acceptance obligations.**

   “Implementer: to be assigned on acceptance” is acceptable for the requested proposal, but it must become a named assignment and landing date before this becomes an implementation commitment. Record the narrowly authorized first-class attributes in the binding scope documentation when they land.

   The explicit review request supersedes `DEFERRED.md`’s general prohibition on refine loops for this task; it is not a reason to reject this review. Likewise, the owner has already settled first-class attributes and D9, so neither needs reopening.

Useful implementation-phase obligations are to test a cancelled `env()` term whose syntactic reference still receives credit, a translucent propagated background painted once, nested route/tab ownership clearing shared obstruction once, and viewport stability when a primary route’s header disappears. The fitted-sheet fixture should also distinguish final allocation height from the safe-area-excluding value returned to UIKit’s detent resolver. These are non-blocking test obligations, not demands for more normative machinery.

The remaining open questions are limited: which consumer first needs D6, and whether Q1’s separate default change should ever move from `page` to `automatic`. Neither needs resolution to implement the ownership and clearance slices.

Recommended next step: make a focused revision resolving concerns 1 and 2, correct the minor source attribution, and retain the remaining details as implementation obligations. Keep the document in Review until that revision is checked.

VERDICT: NOT READY

## Round 3 (delta round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` 2026-10-08, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral session.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** delta round under LLP 0010.001. A fresh session read the full target plus the unified diff from the round-2 revision (`/tmp/af/r3.diff`), with each concern classified IN-DELTA or PRE-EXISTING. Blind to rounds 1 and 2; that blindness is self-attested.
- **Target revision:** sha256 `6c6b83526a9ed4b81eda5757d4e8659073b15118546bd67c6d4cb078b0597e5e`.
- **Prompt:** `/tmp/af/r3.md`.
- **Verdict:** NOT READY. One IN-DELTA MATERIAL concern; **no PRE-EXISTING MATERIAL concern**.

**Overall assessment:** The proposal is coherent, and delta intents (1) and (3) are resolved. Intent (2) remains incomplete: D6’s enforcement predicates allow feedback that its independence requirement forbids. I found **one IN-DELTA MATERIAL concern and no PRE-EXISTING MATERIAL concern**.

**Review provenance:** Codex family, independent reviewer; 2026-10-08; read-only review with instruction-based blindness, self-attested. I read the target in full and only the permitted `/tmp/af/r3.diff` from that directory. The target hash matched before and after review:

`6c6b83526a9ed4b81eda5757d4e8659073b15118546bd67c6d4cb078b0597e5e`

This assessment covers that revision only.

The strengths are substantial:

- D1–D2 separate document facts from owner allocations and rendering ancestry, including cancellation, mount ownership, coordinates, focus and hit testing.
- D3 now consistently assigns side clearance to wrapper padding. I recomputed all **15 table rows** successfully. The horizontal-scroller fixture also checks: `W = 82.5`, clearance `17.5`, authored padding `16.5`, total clearance `34`. Padding percentages use containing-block width on every edge, as [CSS 2 specifies](https://www.w3.org/TR/CSS2/box.html#padding-properties).
- D5’s conflicting-bound `clamp()` result, `none` handling and comparison semantics match [CSS Values 4](https://www.w3.org/TR/css-values-4/#comp-func). D9’s registered-property inheritance and invalid-value treatment match [CSS Variables §3.1](https://www.w3.org/TR/css-variables-1/#invalid-variables).
- The tvOS correction accurately separates HIG guidance from Exact’s chosen presentation mapping. Apple describes tvOS sheets as modal and attributes the full-screen description to watchOS; the installed SDK marks `UISheetPresentationController` unavailable on tvOS. [Apple HIG: Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets)
- The SDK confirms custom detents’ safe-area accounting, keyboard-guide defaults and `viewIsAppearing` availability. The newly recorded test obligations appropriately leave implementation details to the implementation phase.

The concerns are:

1. **IN-DELTA · MATERIAL — D6’s enforcement is narrower than its independence requirement.**  
   [D6’s feedback fence](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:319) requires runtime intervention only when a fact is re-answered and chrome changes **in the same batch**.

   A periodic action can instead read the already committed `safeTop`, store heading level 2 when it exceeds 80 and level 1 otherwise, and drive `aria-level` from that state. The action batch need not re-answer the fact. UIKit’s subsequent layout publishes the new inset, allowing the next timer action to repeat **106 → 54 → 106**. The stated bake condition also describes `when`/`derive` dependencies, without covering this action-to-state path.

   Likewise, restricting bake analysis to the same owner’s subtree misses a route fact controlling an ancestor’s tablist. Ancestor bars can contribute to that route’s safe area, as [Apple documents](https://developer.apple.com/documentation/uikit/uiview/safeareainsets).

   **Resolution:** Make enforcement cover causal dependencies affecting the queried owner, including action/state paths, later batches and ancestor chrome. Conservative refusal is sufficient; no particular tracing algorithm needs specification here. This identifies a defective guard condition, rather than requesting implementation detail.

2. **IN-DELTA · minor — The artwork image does not necessarily fill its containing block.**  
   [D3’s artwork recipe](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:249) correctly identifies the positioned wrapper as the containing block. However, `image position="absolute" inset=0` retains intrinsic dimensions when width and height are auto. Replaced elements do not stretch between opposing insets. [CSS 2 §10.3.8](https://www.w3.org/TR/CSS2/visudet.html#abs-replaced-width)

   **Resolution:** Specify `width="100%" height="100%"`, with the intended `object-fit`, or use a non-replaced background layer. This is a small recipe correction.

The whole-document pass found no additional defective decision in unchanged text. The recorded obligations concerning translucent backgrounds, nested clearance, disappearing headers and fitted-sheet accounting are implementation-phase material and do not independently block readiness.

**Recommended next step:** Correct D6’s enforcement scope and the image recipe, then verify those corrections in a further authorized delta round. If the loop closes without that review, label the final revision unreviewed under LLP 0010.001.

VERDICT: NOT READY

## Round 4 (final delta round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` 2026-10-08, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral session.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** delta round under LLP 0010.001. A fresh session read the full target plus the unified diff from the round-3 revision (`/tmp/af/r4.diff`). Blind to rounds 1 through 3; that blindness is self-attested.
- **Target revision:** sha256 `1f92e6d038b8be9cdd8ed02cad53d2e00cb783d8b50d21c901c4ae17c2c0d306`.
- **Prompt:** `/tmp/af/r4.md`.
- **Verdict:** READY.

**Overall assessment: READY.** Both delta intents are resolved, and the whole-document review found no unresolved MATERIAL correctness, coherence, feasibility, or decision-quality concern.

This round-4 Codex-family review binds to SHA-256 `1f92e6d038b8be9cdd8ed02cad53d2e00cb783d8b50d21c901c4ae17c2c0d306`. I read all 446 lines and the permitted diff, preserved instruction-based blindness, and rechecked the hash. No target change was observed during review.

**Strengths and verification**

- **D6 closes the stated feedback gaps.** Its independence requirement now explicitly includes actions that read the fact and write state, effects in later batches, and the queried owner’s own and ancestor chrome and presentation. Bake-time enforcement refuses uncertain paths; runtime enforcement extends beyond the settlement that originally read the fact. This is deliberately conservative and no longer depends on detecting a same-batch change within the queried subtree.
- **The artwork correction is sound.** Explicit `width="100%"` and `height="100%"` size the replaced element against the positioned wrapper’s padding box; `object-fit="cover"` preserves the image’s aspect ratio while filling that box. Opposing insets alone do not give a replaced image the stretching behavior of a non-replaced box. [CSS 2 sizing rules](https://www.w3.org/TR/CSS2/visudet.html#abs-replaced-width), [CSS Images](https://www.w3.org/TR/css-images-3/#the-object-fit).
- **D3’s arithmetic checks out.** Recomputing all 15 table rows gives the stated content insets: `34, 50, 34, 59, 42, 34, 68, 34, 34, 16, 34, 34, 16, 44, 34`. For the percentage case, `c = (34 − 0.2×100)/(1 − 0.2) = 17.5`, leaving `W = 82.5` and authored padding `16.5`. The horizontal-primary-scroller fixture correctly retains those numbers and a horizontal content inset of zero: side clearance remains wrapper padding. The percentage basis matches [CSS 2 §8.4](https://www.w3.org/TR/CSS2/box.html#padding-properties).
- **D5 and D9 remain consistent with CSS.** The minimum wins in `clamp(100px, 20px, 50px)`; `none` is permitted in either outer clamp argument. The cyclic registered-property fixture also has the correct expectation: with `<length>` syntax and `inherits: true`, computed-value invalidity behaves as `unset`, so the parent’s `34px` or the owner wrapper’s host value is inherited. [CSS Values](https://www.w3.org/TR/css-values-4/#comp-func), [CSS Variables](https://www.w3.org/TR/css-variables-1/#invalid-at-computed-value-time).
- **The Apple mapping is comprehensive at proposal altitude.** It distinguishes allocation, safe areas, margins, scroll adjustment, keyboard occupancy, traits, containment, and presentation. The consequential claims are supported: safe areas are view-local; `viewIsAppearing` has accurate geometry before first display; custom detents exclude bottom safe-area accommodation; reserved-region frames include margins. [UIKit safe areas](https://developer.apple.com/documentation/uikit/uiview/safeareainsets), [appearance lifecycle](https://developer.apple.com/documentation/uikit/uiviewcontroller/viewisappearing(_:)), [custom detents](https://developer.apple.com/videos/play/wwdc2022/10068/), [reserved regions](https://developer.apple.com/documentation/uikit/uiview/reservedregion). The Safari measurements remain reported simulator evidence, appropriately distinguished from hardware verification.

**Concerns**

1. **IN-DELTA — minor: regression coverage should follow D6’s expansion.** The test list still describes direct heading-level feedback and a generic dynamic chrome change.  
   **Resolution:** carry explicit stage-6 fixtures for action → state → later-batch chrome, descendant reads affecting ancestor chrome, and refusal of an unprovable dependency. These verify the settled enforcement decision; they are implementation-phase obligations and do not block readiness.

2. **PRE-EXISTING — minor: one Apple source label is inaccurate.** The link described as “sheets and the fold” points to *Design for iPhone Duo*. Its sheet-behavior section supports the claim.  
   **Resolution:** correct the label when convenient; the evidence and design conclusion remain valid. [Apple’s talk](https://developer.apple.com/videos/play/tech-talks/111466/).

**Recommended next step:** this revision is ready for the author’s acceptance decision and implementer assignment. Carry the minor fixture obligation into implementation. No further normative expansion is needed for this family’s readiness.

VERDICT: READY

## Close-out

- **Converged:** the codex family is READY on revision sha256 `1f92e6d038b8be9cdd8ed02cad53d2e00cb783d8b50d21c901c4ae17c2c0d306`. No later unreviewed revision exists.
- **Rounds:** four. Round 1 was a full review, NOT READY. Round 2 was a full review, NOT READY. Round 3 was a delta review, NOT READY, with no pre-existing MATERIAL concern. Round 4 was a delta review, READY.
- **Families:** single-family (codex) at the owner's instruction. The usual second family was not run.
- **Carried to implementation (non-blocking):**
  - Stage-6 fixtures for D6: action → state → later-batch chrome; a descendant read affecting ancestor chrome; refusal of an unprovable dependency.
  - Correct the label on the Apple "Design for iPhone Duo" sheet link.
- **Status:** stays `Review`. Acceptance is the author's decision.

# Consensus loop (two families), 2026-10-08

## Loop 2, round 1

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec`, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** fresh session, blind to all reviews; that blindness is self-attested. The Claude Fable 5.1 family reviewed in parallel; neither saw the other.
- **Author and reviser:** Claude Opus 5.5.
- **Target revision:** sha256 `488a8cc85fbb6fa2f4172913e16d174f418c828211aa314c96978f9129b1f1ae`.
- **Prompt:** `/tmp/af/c1-prompt.md`.
- **Verdict:** NOT READY.

The proposal is **not ready**. Its core model is coherent, and the staged plan is broadly implementable, but three unresolved decisions affect correctness.

This review binds to SHA-256 `488a8cc85fbb6fa2f4172913e16d174f418c828211aa314c96978f9129b1f1ae`, verified unchanged at completion. I reviewed independently, read only, with instruction-based blindness, self-attested. I did not read the excluded review directories. I opened all 91 distinct cited URLs and found no broken destination; Apple API claims were checked using indexed documentation and installed SDK declarations where pages returned JavaScript shells. The reported simulator measurements were not independently reproduced.

The proposal’s strengths are substantial:

- D2 separates document facts, local allocations, transforms and rendering ancestry clearly. This directly addresses the sheet resizing the document behind it.
- D3 gives clearance and credit a precise mathematical contract, including percentage padding and multiple solutions.
- D9 specifies registration, computed-value inheritance and invalid overrides consistently with [CSS Variables §3.1](https://www.w3.org/TR/css-variables-1/#invalid-variables).
- The Cases catalogue covers transitions, detents, keyboard variants, embedding and multiple hosts. The staged plan keeps workarounds until the first complete iOS slice passes.
- The amendment list, schema authority and consumer gates generally respect exact2’s rules. Scroller compensation techniques, handle leasing and individual transition fixtures appropriately remain implementation obligations.

My concerns follow.

1. **MATERIAL — A named scroller does not necessarily protect the owner’s other content.**

   [D3’s executor rule](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:317) assigns all top and bottom clearance to any named primary scroller and sets those wrapper paddings to zero. D7 applies the same assumption to docked keyboard overlap.

   Consider a route containing a fixed introductory control, a named scrolling list, and a fixed composer below the list. Scroller insets cannot move either fixed sibling. The introductory control can remain under the navigation bar; keyboard accommodation applied only to the list can leave the composer covered.

   Existing `navigationScroll` resolution accepts a descendant scroller without requiring it to span the owner. UIKit also derives `adjustedContentInset` from the **scroll view’s** safe area, which can differ from the owner’s. [Apple documentation](https://developer.apple.com/documentation/uikit/uiscrollview/adjustedcontentinset)

   **Resolution:** qualify executor selection per edge. Transfer clearance only when the selected scroller actually provides clearance for the content affected by that edge; otherwise retain wrapper clearance or define the appropriate allocation arrangement. Add catalogue cases for a partial-height scroller and fixed siblings, including a focused composer outside the primary scroller. This preserves automatic clearance and the single-executor principle.

2. **MATERIAL — Root fitting has conflicting requirements.**

   C1 says `viewport-fit="auto"` fits the document inside the safe area. [D1](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:229) makes fitting depend on primary chrome while saying routes and controllers cannot change document facts. Stage 1 prescribes `wantsWholeView` with presented navigations excluded. The implementation obligation at line 502 requires `exactViewport()` to remain unchanged when a primary header and its bar disappear.

   Today, [`wantsWholeView`](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:767) depends on whether a navigation bar shows. Merely excluding presented navigations therefore still lets a primary bar disappearing switch fitting between whole bounds and safe bounds. Counting stable container presence would behave differently, but C1 does not state that exception.

   **Resolution:** define root fitting consistently from session policy and primary containment, explicitly distinguishing container existence from bar visibility. Reconcile C1, D1 and stage 1, and make header-visibility invariance normative. The existing test obligation identifies the desired result but does not resolve the conflicting instructions.

3. **MATERIAL — The active inner-camera case is omitted under an unsupported generalization.**

   The [reserved-region mapping](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:189) states, “Occlusion regions are already the safe area.” C13–C16 cover the strip and fold but provide no disposition for activation and deactivation of the inner camera.

   Apple distinguishes reserved regions from standard safe-area considerations. The inner-camera region becomes present when active, and custom components can use reserved-region geometry to reposition content. The cited material does **not establish** that every such occlusion is represented sufficiently by four edge insets. [HIG: Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo), [Apple’s adaptive-layout talk](https://developer.apple.com/videos/play/tech-talks/111463/)

   **Resolution:** add an explicit active/inactive inner-camera case and identify the documented protection supplied by owner allocation or safe-area changes. Narrow the universal claim. If rectangular occlusion handling needs a separate consumer, state that deferral and native-container dependency explicitly, including the corresponding correction to LLP 1078. A new occlusion mechanism need not be designed here.

4. **minor — The inner-screen strip case needs orientation and presentation qualifications.**

   [C14](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:80) simply repeats C13 at the inner screen’s size. Apple specifies horizontal bars on the inner display in portrait, and standard inner-display sheets use horizontal bars. [Raise the bar](https://developer.apple.com/videos/play/tech-talks/111462/), [Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/)

   **Resolution:** qualify C14 as the applicable vertical-bar configuration and explicitly cover portrait, inner sheets, and disabling vertical bars. D2/D3 already provide the underlying measured-geometry behavior; this requires catalogue correction.

5. **minor — Modal and nonmodal sheets need an explicit scope distinction.**

   [C17–C22](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:88) describe inert backgrounds and focus going to the presented owner. Apple’s HIG also defines nonmodal iOS/iPadOS sheets, where interaction with the presenting content continues. UIKit supports this through undimmed detents. [HIG Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets), [largestUndimmedDetentIdentifier](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/largestundimmeddetentidentifier)

   **Resolution:** add a case explicitly stating that these rules concern Exact’s admitted modal presentations and whether nonmodal sheets are deferred. This does not require adding a new attribute.

6. **minor — Execution ownership remains pending.**

   The header says the implementer will be assigned on acceptance. `rules/RULES.md` requires an implementer and date for a spec.

   **Resolution:** retain this as an RFC until implementation ownership is assigned, and name the consumers required for stage 6 before admitting those slices.

The recommended next step is a focused revision resolving concerns 1–3 and correcting the catalogue scope in concerns 4–5, followed by independent review of the new hash. The settled architecture can remain intact.

VERDICT: NOT READY

## Loop 2, round 2 (delta round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec`, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c2.diff`, and this family's own punch list `/tmp/af/c2-punch-codex.md`.
- **Blindness breach, disclosed by the reviewer:** a repository search exclusion failed and exposed excerpts of review artifacts, including one sentence of the other family's review. **This round does not count as a clean, mutually blind round.**
- **Target revision:** sha256 `022869ec62a6e3552f16024852a8e8cc0ccc4489c6220838a1b3d346fa46ee25`.
- **Prompt:** `/tmp/af/c2-prompt-codex.md`.
- **Verdict:** NOT READY.

One MATERIAL concern remains: the scroller rule still fails to protect some fixed siblings.

I read the full proposal and delta. The target hash matches `022869ec62a6e3552f16024852a8e8cc0ccc4489c6220838a1b3d346fa46ee25`, verified again after review. No revision was applied.

Blindness disclosure: a repository search exclusion failed and exposed review excerpts, including one sentence from the other family’s target review. This response cannot count as a clean mutually blind round.

The six punch-list dispositions verify as follows:

| Item | Verification |
|---|---|
| 1. Scroller and fixed siblings/composer | **Not fully FOLDED.** D7 fixes the focused composer case. D3 still leaves parallel fixed content unprotected, as described below. |
| 2. Root fitting | **FOLDED correctly at the decision level.** C1, D1 and D9 agree on the three cases. One stale test expectation remains. |
| 3. Inner camera and occlusion generalization | **FOLDED correctly.** C14a names the case, the unsupported equivalence is removed, and the LLP 1078 amendment is explicit. Simulator confirmation remains a recorded obligation. |
| 4. C14 orientation and presentation | **Partially FOLDED.** The orientation correction is sound; the assertion about sheets remains too broad. |
| 5. Modal versus nonmodal sheets | **FOLDED correctly.** C24a clearly defers nonmodal presentations. |
| 6. Implementer | **RECORDED-AS-OBLIGATION correctly.** The target remains an RFC, with assignment required on acceptance. |

The remaining concerns are:

1. **PRE-EXISTING — MATERIAL; persistent punch item 1.** [D3’s revised transfer rule](/Users/admin/.config/tuft/sessions/1791430083.031489/worktrees/exact2/llp/1123-content-owners-and-local-safe-areas.rfc.md:331) tests whether the scroller reaches an edge, but does not establish that its inset protects the other content at that edge.

   Consider one owner containing a horizontal flex row: a full-height primary scroller on the left and a non-scrolling pane with a button at the top on the right. With zero authored padding and `local.top = 106`, no in-flow box lies above the scroller. D3 therefore assigns the top clearance to that scroller and sets wrapper top clearance to zero. The scroller’s content starts below the bar; the adjacent button remains at y=0 under it.

   The revision fixes siblings above and below the scroller, but the original fixed-sibling defect persists. Transfer eligibility needs to ensure that the scroller protects all content requiring that edge’s clearance, or retain wrapper clearance for the parallel-content case. This affects which controls receive clearance and is MATERIAL under the altitude rule.

2. **IN-DELTA — minor.** C14 says bars are horizontal “in sheets.” Apple’s linked talk distinguishes centered inner-display sheets from right-placed inner-display sheets, which receive a vertical bar; outer-display sheets can also have vertical toolbars. Qualify the statement by display and placement. The edge-neutral `safeAreaInsets` decision remains sound. [Apple, “Raise the bar with iPhone Duo”](https://developer.apple.com/videos/play/tech-talks/111462/).

3. **IN-DELTA — minor; diff-created contradiction.** The test at line 515 still requires the root custom properties to equal `env()` without qualification. D9 and the newly added test at line 514 explicitly require them to differ under `auto` with a native primary container. Restrict the equality expectation to the applicable fitting cases.

4. **PRE-EXISTING — minor.** The HIG Modality row at line 225 still cites LLP 1007 §4 for modal-route dismissal. The corrected Related entry points to the §10 addendum, “Modal routes.” Align the remaining citation.

The new API URLs resolve to the expected Apple documentation pages: [UITraitOverrides](https://developer.apple.com/documentation/uikit/uitraitoverrides-swift.struct), [tabBarMinimizeBehavior](https://developer.apple.com/documentation/uikit/uitabbarcontroller/tabbarminimizebehavior), and [largestUndimmedDetentIdentifier](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller/largestundimmeddetentidentifier). Their browser-readable responses provide page identification rather than full API text. The new Tech Talk citation was checked against its transcript. The corrected CSS Backgrounds §2.11.1 reference matches the root-background rule. [CSS Backgrounds 3](https://www.w3.org/TR/css-backgrounds-3/#root-background).

The whole-document pass found no additional MATERIAL concern. The camera confirmation, fitted-size accounting, and named implementer remain obligations; I am not demanding further implementation detail or reopening the settled decisions.

VERDICT: NOT READY
## Loop 2, round 3 (delta round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec`, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral.
- **Date:** 2026-10-09. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c3.diff`, and this family's own punch list `/tmp/af/c3-punch-codex.md`. The prompt required every repository search to exclude `llp/reviews/`; the reviewer reports reading no review content. Blindness is self-attested. The Claude Fable family reviewed the same revision in parallel.
- **Target revision:** sha256 `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4`.
- **Prompt:** `/tmp/af/c3-prompt-codex.md`.
- **Verdict:** READY.

Reviewed the full proposal at SHA-256 `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4`; the hash remained unchanged. The repository stayed read-only. I read no `llp/reviews/` content and only the two authorized `/tmp/af` files.

All four round-2 dispositions are **FOLDED correctly**:

| Finding | Verification |
|---|---|
| Scroller beside a pane takes the top clearance | D3 now requires the scroller to span the whole edge; otherwise the wrapper clears both panes. C33a agrees. |
| C14 overgeneralizes sheet bars | The revised wording distinguishes centred inner sheets from cover-screen and side-placed sheets. [Apple’s Tech Talk](https://developer.apple.com/videos/play/tech-talks/111462/) supports the distinction and corrected title. |
| Root test contradicts D9 | The test now distinguishes `cover` from fitted roots; the preceding test covers `auto` with a native primary container. |
| Modality cites the wrong section | LLP 1007’s §10 addendum, “Modal routes,” contains the cited dismissal policy. |

**IN-DELTA:** No unresolved MATERIAL or minor concern found. The additional changes preserve the owner model, identify the required lexer extension correctly, and correct the LLP 1078 quotation. No external URLs were added or changed.

**PRE-EXISTING, minor:** The Safari evidence table says probes used both `viewport-fit` values but does not distinguish their readings. Label the readings by policy, particularly the unfolded bottom inset of 34, so their relationship to D1’s `auto` rule is clear. This is an evidence-presentation correction, not a defective design decision.

The whole-document review found no unresolved PRE-EXISTING MATERIAL concern. The recorded implementation obligations remain non-blocking under the altitude rule.

VERDICT: READY

## Loop 2, round 4 (delta round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec`, model `gpt-6.1-sol`, reasoning effort `xhigh`, sandbox `read-only`, ephemeral.
- **Date:** 2026-10-09. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c4.diff`, and this family's own punch list `/tmp/af/c4-punch-codex.md`, which also describes an author-initiated factual correction (a new Safari measurement). Repository searches excluded `llp/reviews/`. Blindness is self-attested. The Claude Fable family reviewed the same revision in parallel.
- **Target revision:** sha256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b`.
- **Prompt:** `/tmp/af/c4-prompt-codex.md`.
- **Verdict:** READY.

The punch-list disposition is correct. I found no MATERIAL concern in the delta or the pre-existing text.

Reviewed the full proposal at SHA-256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b`; the hash remained unchanged. Blindness is self-attested: I read only the two permitted `/tmp/af` files and saw no review content.

- **Item 1 — FOLDED correctly.** The evidence now explicitly attributes the readings to both `cover` and `auto`, including the unfolded bottom inset of 34.
- **Author-initiated correction — sound.** Treating the measurement as reported evidence, D1 and the acceptance amendments correctly remove the false browser analogy. The three native fitting rules, their inset values, and the web host’s delegation to the browser remain unchanged.
- **Other delta edits — sound.** The scroller restatement preserves the existing per-edge eligibility and clearance accounting. The LLP 1078 quotation matches its source, and the Lab exists.
- **Links.** No new external destinations were introduced. The existing [WebKit reference](https://webkit.org/blog/7929/designing-websites-for-iphone-x/) supports the safe-area padding and `max()` idiom.

One **IN-DELTA, minor** wording concern remains at line 47: “Exact’s `auto` does” would be more precise as “Exact’s **native** `auto` does,” because C38 and D1 explicitly preserve Safari’s behavior on the web. The decision is clear from those sections; this is nonblocking.

No additional PRE-EXISTING concerns identified. The recorded implementation obligations require testing, not further proposal-level closure.

VERDICT: READY

## Close-out (loop 2)

- **Converged:** both families are READY on revision sha256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b` (round 4). Both were also READY on the preceding revision, `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4` (round 3). No later unreviewed revision exists.
- **Rounds:** four. Round 1 was a full review (codex NOT READY, Fable NOT READY). Round 2 was a delta round (codex NOT READY with a disclosed blindness breach, so not a clean round; Fable READY). Round 3 was a delta round (both READY). Round 4 was a delta round over an author-initiated factual correction, a new Safari measurement that showed `env(safe-area-inset-bottom)` 34 under `viewport-fit=auto` (both READY).
- **Carried to implementation (non-blocking, detail only):**
  - Codex: say "Exact's native `auto`" in §"Evidence".
  - Fable: the Lab bullet's tense and its narration; the Evidence table header's single labelled column; D9's root sentence read against the web.
- **Status:** stays `Review`. Acceptance is the author's decision.

## Loop 3 (r2), round 1

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T05:00:38Z to 2026-10-25T05:10:17Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial, a fresh session with the worktree `llp/1123-owner-values` read-only and a capsule (the brief, plain-UIKit probe measurements and sources from 2026-10-24, the prior safe-area comparison). Not told to avoid `llp/reviews/`. One review covering three documents.
- **Target revisions:** LLP 1123 r2 sha256 `7c91b4df7a43fdab4876bb42214577ef43de8f6c792a6451339c71e0c6530ef0`; LLP 1123.001 r1 `9c49714639265e4098c60aa382158028e8594d68ef212c891ebb6d247f666df2`; LLP 1127 r19 `273b8f39c5d6c14ba47767b99326efe80863a2913c8680f576bdfdb8e2793376`; brief `ef3a205e25b53e704aaaa6427e840e68307cab6db12b9190bdd7094245928f6d`.
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[blocker] 1123 D3/D9 — Delegating clearance to a scroller leaves other content unprotected.**

   **Failure:** An owner has `local.top = 106`, a full-width primary scroller, and an absolutely positioned foreground button at `top: 0`. The scroller reaches the top, so D3 removes the wrapper’s top padding and gives the scroller a content inset. The button remains at y=0, under the bar. D9 nevertheless reports `--exact-safe-area-inset-top: 0`, so padding the button with the recommended property cannot clear it.

   This also makes “the cleared content root” ambiguous: the scroller’s content and its stationary siblings have different effective origins.

   **Resolution:** Define clearance by subtree and distinguish the physical scrollport from the cleared content space. Preserve a safe containing block for stationary foreground content. Test this composition, including segment coordinates and large-title collapse.

2. **[major] 1123 D9, “Segments” and “Registration” — Absent segments inherit from the outer owner.**

   **Failure:** The document has two horizontal segments, with `--exact-viewport-segment-left-1-0: 495.5px`. UIKit moves a sheet onto the left segment. D9 leaves the sheet’s nonexistent `-1-0` properties unset. Because unregistered custom properties inherit, the sheet reads **495.5px**, rather than taking `var(..., 0px)`’s fallback. The same leak affects arrangement panes and top-layer resets. Removing a declaration does not create an inheritance boundary. [CSS Variables 1](https://www.w3.org/TR/css-variables-1/#defining-variables)

   **Resolution:** At every owner boundary, explicitly reset every absent segment property to its guaranteed-invalid initial value, then set existing segments. Specify equivalent native behavior and test nested owners and fold-to-flat transitions.

3. **[major] 1123 D9, authored overrides — Unregistered segments cannot have the registered-property semantics promised here.**

   **Failure:** A parent with `font-size: 20px` declares `--exact-viewport-segment-width-0-0: 2em`; its child has `font-size: 10px` and uses that property as its width. D9 promises an inherited computed length of **40px**. An unregistered browser property inherits the tokens `2em`, which become **20px** when consumed by the child. Likewise, a cyclic unregistered override becomes guaranteed-invalid; it does not inherit the wrapper’s value as D9 promises for every property. [CSS Variables 1](https://www.w3.org/TR/css-variables-1/#invalid-variables), [Properties and Values API §2.4](https://drafts.css-houdini.org/css-properties-values-api/#computed-value-time-behavior)

   **Resolution:** Specify separate semantics for registered insets and unregistered segments. Either implement ordinary token substitution natively or restrict segment overrides explicitly. `style/roles.rs` supplies named colour references and overrides; it does not already provide this general per-element computation machinery.

4. **[major] 1123.001 D4/D6, versus 1123 D2 — Arrangement placement omits stacking and visibility.**

   **Failure:** In an unfolded overlay, both panes have identical rectangles. UIKit puts the **first**, primary pane on top. But 1123 D2 preserves ordinary owners’ sibling ranking, which puts the later secondary pane above it. Publishing rectangles alone cannot reconcile these answers. With `split horizontal` on the closed Duo, UIKit hides the secondary without any authored `display: none`; its runner visibility and focus eligibility are unspecified.

   **Resolution:** Publish arrangement `zIndex` and `isHidden` coherently with placement, and define their effect on painting, point reads, accessibility, focus and restoration. D4’s proposed `container:` read also needs a naming mechanism for panes without `navigationKey`, plus fields for arrangement state; 1123 D6 currently provides neither.

5. **[major] 1123 D3/C33 — Mandatory side wrapper padding prevents the platform’s horizontal scrolling behavior.**

   **Failure:** Beside the Duo’s 84-point strip, or a split sidebar, a horizontal carousel is permanently narrowed. Only its propagated background colour reaches beneath the chrome; its cards cannot scroll there. Landmarks deliberately allows horizontal content to pass beneath the sidebar and inspector when the scroll view touches those edges. [Apple’s horizontal-scrolling sample](https://developer.apple.com/documentation/swiftui/landmarks-extending-horizontal-scrolling-under-a-sidebar-or-inspector)

   **Resolution:** Support separate horizontal scrollport allocation and resting content insets, with appropriate indicator treatment. Qualify nested carousels, bidirectional scrollers and both sidebar edges. C13’s claim that horizontal scrolling surfaces reach underneath needs to distinguish a painted background from scrolling content.

6. **[major] 1123.001 D2/D6 — The fallback is insufficiently specified, and its web executor is missing.**

   **Failure:** Give both panes `width: 75%`, or conflicting minimum widths in a narrow container. “Half each unless their dimensions say otherwise” does not determine the resulting allocation. D2 also does not say whether CSS widths are converted to controller dimensions before or after padding, borders and `box-sizing`, or whether those sizes remain active on the authored roots afterward.

   On the web, “the kernel places the panes” supplies no runtime mechanism: the default JS target drives DOM layout, and the wasm web host also delegates layout to the browser.

   **Resolution:** Define constraint resolution, box-model conversion, square-container behavior, RTL placement and fold intersection. Specify a browser executor and how its algorithm stays consistent with the native fallback. The full-window Duo table is evidence for those inputs, not a complete algorithm.

7. **[major] 1123 Summary/D9/Tests — Controller regions expressed from another origin are not that view’s own regions.**

   **Failure:** The capsule’s iPhone route root reports margins `[168,16,34,16]`; its safe-area-pinned inner view reports `[8,8,8,8]`. D9’s subtraction produces `[0,16,0,16]`. For corner adaptation, the inner view reports a bottom inset of 8.33, while subtracting the controller’s bottom clearance produces zero.

   Thus D9’s explicit controller-region policy contradicts the Summary and test requiring every value to equal what UIKit reports “for a view there.”

   **Resolution:** Choose the intended meaning. Querying the actual content view gives per-view UIKit semantics. Translating the controller’s regions is also a useful policy, but name and describe it accurately, and test against translated controller rectangles rather than the inner view’s API results.

8. **[major] 1123 D9/D10 — Root region coordinates disagree under `auto` with native navigation.**

   **Failure:** D1 gives the document the full session view. Suppose its safe top is 62. D9 correctly exposes root `--exact-safe-area-inset-top: 62px`. But D10 says regions are measured from the safe area even when the native primary container occupies the whole view. A controller margin beginning at y=62 therefore becomes a zero top inset, despite being described as a distance from the layout viewport. An overlay outside owners padding with the margin property remains under the status bar.

   **Resolution:** Translate regions from the actual document rectangle. Subtract safe-area fitting only when the document was physically fitted. Reconcile the custom properties, `env()` values and declared deviations for all three D1 modes.

9. **[major] 1123 D3/C30 — Opting out of clearance does not reproduce Landmarks’ background extension.**

   **Failure:** An image expanded beneath the sidebar and inspector loses visible image area behind those panels. Landmarks instead preserves the image’s visible composition and extends mirrored, blurred edge imagery underneath. Colour propagation and an oversized image provide different results. [Apple’s background-extension sample](https://developer.apple.com/documentation/swiftui/landmarks-applying-a-background-extension-effect)

   **Resolution:** Document a supported platform mechanism or extension seam, with a defined relationship to foreground clearance. Add distinct cases for side extension, extension above the top safe area, and stretching during overscroll. The latter is an additional behavior in Apple’s sample, not a consequence of background extension. [Landmarks overview](https://developer.apple.com/documentation/swiftui/landmarks-building-an-app-with-liquid-glass?changes=latest_major)

10. **[major] 1123.001 §3 and 1127 D7/qualification — The Landmarks inspector is handed to a document that does not specify it.**

    **Failure:** 1123.001 assigns inspectors to LLP 1127, but 1127 specifies primary and secondary navigation columns without an inspector contract. Opening an inspector therefore has no defined authored subtree, owner allocation, visibility or compact presentation.

    The same qualification gap covers grouped toolbar items and global search. D7’s per-column headers and existing header search do not establish search scoped to the entire split, which is the distinction Apple makes in Landmarks. [Apple’s toolbar and search examples](https://developer.apple.com/videos/play/wwdc2025/323/)

    **Resolution:** Provide concrete supported mappings or documented native hatches and ownership rules; otherwise explicitly defer these cases and remove the incorrect inspector handoff. Qualification should exercise inspector show/hide, grouped items, and one global search across collapse and expansion.

11. **[minor] Cross-references and writing — Several passages give conflicting implementation directions.**

    **Failure:** An implementer following 1127’s Related section finds owner segments “deferred” in 1123 D6, although r19 relies on D9. In 1123.001 D5, the quoted guidance warns against placement inside a navigation split view, then the next sentence calls a split column an expected location. The distinction is unexplained. The RealWorld section in 1123 recounts a dated debugging and regression-test history inside the normative ownership definition.

    **Resolution:** Update references, verify the arrangement quotation and state the supported containment rule directly. Reduce the RealWorld passage to the ownership constraint and move implementation history out of the decision.

What I checked and found sound:

- Document-wide `env()` plus inherited local properties is a coherent distinction under [CSS Env 1](https://drafts.csswg.org/css-env-1/).
- Keeping viewport units document-wide, while percentages use owner blocks, is internally consistent; I support D1’s recommendation for Q4.
- Removing padding credit eliminates the previous percentage fixed point. The wrapper’s content-box overflow examples are correct.
- Installed controller geometry, coherent size/inset publication, and per-sheet allocations match the supplied UIKit evidence.
- 1127’s separation of selected entries from row associations avoids making column ownership depend on row mounting. Coordinator-based pop deduplication and cancellation rules are sensible.

This was a source and specification review against the supplied probe evidence; I did not rerun the simulators.

## Loop 3, round 2

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T05:24:52Z to 2026-10-25T05:32:43Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round; the brief listed how each round-1 finding was answered; the capsule gained evidence §4 and §5.
- **Target revisions:** LLP 1123 r3 sha256 `cc847fa250a6f4ca9c7f5cf5a61202b1805ed6ed4ee262c722c627e592b2ba4d`; LLP 1123.001 r2 `7846d99abc4e8f48964d6c6b8224192ddeec5cb8364b988dfcda9eb6f20588c2`; LLP 1127 r20 `c6191561371d191f9235045a5bfb12a5758905aaaca1143dbffce68e09dd870a`; brief `b9841c8eff6b1b40f58e4c4290d0fc6cf6b75cdee7b55c6e61e2824e2e93d7d9`.
- **Verdict:** NOT READY.

Verdict: NOT READY

The revisions resolve the original stationary-content failure and most of the value-semantics findings. Four specification gaps remain.

1. **[major] 1123 D3/C8 — The new wrapper reintroduces layout changes during large-title collapse.**  
   [D3](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:358) lays the scroller out in the cleared content root before extending it. That root follows the current `local` inset.

   **Failure:** In an 874-point allocation with bottom inset 34, changing the top inset from 168 to 116 changes the scroller’s logical height from 672 to 724. A child sized to 50% changes from 336 to 362 while the person scrolls. Extending the physical scrollport afterward does not undo that layout change. This contradicts C8’s promise that collapsing does not relayout the route; normalizing `scrollTop` alone cannot preserve content geometry.

   The existing `NavigationCoversIOS` code deliberately uses the expanded `scrollOrigin` when determining scrolling-content layout. The revised rule does not preserve that distinction. The new probe’s fixed-height, 2000-point content does not exercise it.

   **Resolution:** Define separately the live clearance for stationary content, the scroller’s layout basis, its physical scrollport, and its offset origin throughout collapse. Qualify percentage-sized content and content positions during an actual collapse and expansion, with the stationary sibling present.

2. **[major] 1123 D3/D11 — “To the allocation’s edge” and “clearance plus root padding” disagree for supported authored roots.**  
   The [reach rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:355) assumes the authored root fills the cleared area. D2 expressly permits narrower roots and margins, and D8 recommends centered readable widths.

   **Failure:** Give an owner a 1000-point allocation, left clearance 100, and no right clearance. A 400-point authored root centered in the remaining 900 points starts at x=350. Its full-width carousel touches both root content edges. Reaching the allocation’s edges requires extensions of **350 left and 250 right**; clearance plus root padding supplies **100 left and 0 right**. These produce different scrollports and resting insets. Borders and overflowing content-box roots introduce further discrepancies.

   D11 imports the same distance rule, so its extension geometry is ambiguous too.

   **Resolution:** Define reach eligibility and distance using explicit rectangles. State how authored root margins, borders, constrained sizes and overflow affect it, preserving authored constraints. Add centered-root and bordered-root cases; the existing full-width padding example cannot settle this.

3. **[major] 1123.001 D6 — The web executor misses movement relative to a stationary fold.**  
   [D6](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:91) recomputes on arrangement size or viewport-segment changes. Neither changes when an earlier sibling moves an otherwise unchanged arrangement. `ResizeObserver` observes size, not position. [Resize Observer specification](https://drafts.csswg.org/resize-observer/)

   **Failure:** On the laptop fold at y=455.5–495.5, place a 500×500 arrangement at y=100. Its local division is 355.5–395.5. Increase an earlier sibling’s height by 100: the arrangement moves to y=200, and its division must become 255.5–295.5. The specified callbacks need not fire. Keeping the old primary height now carries that pane across the fold.

   Inherited D9 properties do not solve this: they remain in the owner content root’s coordinates, rather than automatically becoming an ordinary arrangement box’s coordinates.

   **Resolution:** Specify the coordinate conversion and invalidate placement whenever the arrangement’s position relative to the divisions changes, including owner-placement changes. Test movement without resizing on both web targets.

4. **[major] 1123 D9/amendments — The segment property domain does not fit the fixed twelve-property contract.**  
   [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:466) admits indexed `top`, `left`, `width` and `height` properties without stating an index restriction, while the reset and schema rules require exactly twelve properties.

   **Failure:** Exact already admits a 2×2 segment grid through LLP 1078. Four fields for its four segments require sixteen properties. The revision does not identify which twelve exist or how `(1,1)` is published and reset. Separately, existing `env(viewport-segment-right …)` and `-bottom` expressions have no direct counterpart in D9, although the lowering warning promises the corresponding owner property. CSS and Exact’s existing segment vocabulary include both fields. [CSS Environment Variables §2.3](https://www.w3.org/TR/css-env-1/#viewport-segments)

   **Resolution:** Declare the complete field/index domain and reset every admitted property at each boundary. Either provide the six existing fields or specify migration expressions for omitted ones. If local grids are deliberately restricted, state and enforce that restriction. Qualify a 2×2 grid becoming smaller, including a nested owner.

5. **[minor] 1123.001 D2/D6/D7 — The shared qualification table includes inputs the proposal refuses.**  
   D2 refuses authored pane dimensions, but D6 and D7 require the kernel, JS runtime and Contract smoke to run every row of §1, including fractional widths, conflicting minima and an exact 300-point primary.

   **Failure:** Those smoke fixtures fail lowering; the specified fallback also has no dimension-constraint algorithm capable of producing their measured answers.

   **Resolution:** Separate UIKit measurement rows from supported Contract qualification rows. Test refused dimensions as lowering failures, and reserve cross-executor geometry comparisons for admitted inputs.

Round-1 findings, in order:

1. **Resolved:** Keeping wrapper clearance protects the stationary absolute sibling; finding 1 above concerns the resulting collapse behavior.
2. **Resolved for the stated two-segment cases:** Explicit `initial` resets prevent inheritance leaks; the larger property-domain gap is finding 4 above.
3. **Resolved:** Refusing authored segment declarations removes the unregistered override mismatch.
4. **Resolved:** Placement now carries stacking and hidden state, with runner inactivity and a proposed pane naming mechanism.
5. **Resolved in policy:** Horizontal and nested scroller reach is now specified; finding 2 above identifies an unresolved boundary case.
6. **Not fully resolved:** Deferring dimensions and adding explicit fallback rules and web executors addresses most of it, but position invalidation remains missing.
7. **Resolved:** The Summary, D9 and tests now consistently describe translated controller regions.
8. **Resolved:** Root custom-property coordinates follow the actual document rectangle; the deliberate difference from `env()` is stated.
9. **Resolved in design:** D11 provides a concrete platform extension mechanism and separates top-edge artwork and overscroll behavior.
10. **Resolved by explicit deferral:** Neither document claims to implement the inspector, toolbar groups or split-wide search.
11. **Resolved:** The identified cross-references, containment contradiction and RealWorld narration were corrected.

What I checked and found sound:

- The document-wide `env()` and owner-local custom-property distinction remains coherent.
- Guaranteed-invalid resets are the correct way to stop absent unregistered segment values inheriting. [CSS Variables §2.2](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid)
- Registered inset properties and host-only segment properties now have appropriately distinct override semantics.
- Evidence §4 supports the corrected distinction between controller regions and a safe-area-pinned subview’s own regions.
- `UIBackgroundExtensionView` with automatic content placement disabled supports explicitly retaining the laid-out content frame while filling the surrounding extension. [Apple documentation](https://developer.apple.com/documentation/uikit/uibackgroundextensionview/automaticallyplacescontentview)
- 1127’s selection/association separation, coordinator-based pop handling, and controller-derived split geometry remain sound on this review.

I reviewed the three revisions, round-1 findings, capsule evidence and probe sources, relevant implementation code, SDK headers and web specifications. I made no changes and did not rerun the simulators.

## Loop 3, round 3

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T05:38:17Z to 2026-10-25T05:46:58Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round, extended to the connected LLPs for correctness (ide: overwrite text that no longer applies); the capsule gained exact diffs and the large-title collapse measurement.
- **Target revisions:** LLP 1123 r4 sha256 `9a0fc493426461a73ab77f439dfda8f2d27d97f9fcee08eb785133b204cafd62`; LLP 1123.001 r3 `e58f6188eb05d9ba7b56618e5fd6321480110b3aa1c6c67b5a949ce8af0e6c52`; LLP 1127 r20 `c6191561371d191f9235045a5bfb12a5758905aaaca1143dbffce68e09dd870a`; brief `6dcfa8d47641991954ef55ac67d501ce58d9f17bc66d7857b654326d8996cc7a`; connected LLPs 1039, 1078, 1081, 1115.002, 1130 as in the worktree.
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1127 D4/D5 — History-menu navigation contradicts the current navigation contract.**

   [D4](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:106) turns a multi-route history pop into repeated Back actions and incorrectly says today’s pushed stack dispatches one Back. [LLP 1115.000 D4.A.3](/Users/admin/Developer/exact-owners/llp/1115.000-no-tells-on-every-host.rfc.md:315) now specifies `navigate` with the chosen URL, or one router commit to the exact chosen visit. `NavigationIOS.swift` implements that distinction.

   **Failure:** A person chooses an older visit while the app has different `navigate` and Back handlers. The proposed split invokes intermediate Back handlers, potentially stopping at an intermediate confirmation; the current stack invokes `navigate` once.

   **Resolution:** Rebase D4, D5, qualification and §5 onto the current history-selection contract. Specify any deliberate exception for clearing a detail separately, and test repeated URLs, refusal and navigation during the transition.

2. **[major] LLP 1123.001 D6 — Translating owner segments does not produce the arrangement’s segments.**

   [D6](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:86) translates the owner’s rectangles, then gives those rectangles to the panes. It never intersects them with the arrangement’s content bounds.

   **Failure:** In a 951×669 owner with a vertical band at 455.5–495.5, place a 600×400 arrangement at `(200,100)`. Translation produces rectangles `(-200,-100,455.5,669)` and `(295.5,-100,455.5,669)`. Allocating those rectangles puts panes outside the arrangement. The intersections are instead `(0,0,255.5,400)` and `(295.5,0,304.5,400)`.

   **Resolution:** Define intersection with the arrangement’s content box, removal of empty intersections, and which divisions actually cross that box before choosing placements. Account for padding and borders when finding its origin. Also name the document-segment source when no enclosing owner exists. Add offset arrangements crossing a fold and arrangements wholly on one side.

3. **[major] LLP 1127 §5 — The proposed C ABI version is already occupied.**

   [The amendment](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:242) changes `navigate`’s input and specifies ABI **15 → 16**, but [the current header](/Users/admin/Developer/exact-owners/host/apple/include/exact.h:35) already declares version **16**.

   **Failure:** Following this instruction changes the event payload while retaining the existing version number; version checks cannot distinguish the old and new input forms.

   **Resolution:** Specify a fresh version above the current baseline—17 today—and qualify rejection of the preceding form. Keep this separate from the native-module ABI bump.

4. **[major] Connected split decisions remain contradictory, despite the requested rewrite-in-place policy.**

   [LLP 1116.004 D1.2](/Users/admin/Developer/exact-owners/llp/1116.004-what-round-6-asks.rfc.md:93) still says a phone keeps the pushed stack. [LLP 1115.002 D4](/Users/admin/Developer/exact-owners/llp/1115.002-the-macs-structure.rfc.md:490) still says the iPad keeps its width threshold. These prescribe presentation policy; they are not merely descriptions of current implementation.

   **Failure:** An implementer following either decision keeps a wide Duo phone pushed or selects an iPad split by Exact’s threshold, while 1127 delegates presentation to UIKit. 1127 §5 acknowledges these amendments but postpones them until acceptance.

   **Resolution:** Rewrite these decisions now, under the same pending-acceptance convention used for 1039. Update the connected bar rule to identify an N1 base as pushed. Keep statements about currently built presenters separately identified as such.

5. **[major] LLP 1123 “Made with each stage” — The implementation-dependent amendment list is incomplete.**

   [LLP 1116.000 D1.2](/Users/admin/Developer/exact-owners/llp/1116.000-what-round-2-asks.rfc.md:104) requires scrolling-content layout to exclude the adjusted bottom inset. D3 now requires layout against the extended box. [LLP 1116 D2.1](/Users/admin/Developer/exact-owners/llp/1116-what-the-bakeoff-asks-of-exact2.rfc.md:265) retains the cover-based bottom-reaching algorithm. [LLP 1131 D2](/Users/admin/Developer/exact-owners/llp/1131-content-under-a-fixed-control.rfc.md:97) retains covers, `HostCover.inset`, unchanged logical layout and a separate band writer. None appears in 1123’s staged amendment list.

   **Failure:** Retaining 1116.000’s layout inset defeats D3’s extended-box percentage basis. Following 1131 without reconciliation leaves competing instructions for the scroll frame, inset and logical port when a route also has authored control bars.

   **Resolution:** Add these passages to stage 2’s obligations. Explicitly replace the old layout-inset policy and define how authored-control bands compose with owner clearance. Preserve 1130’s separate document-page behavior where it still applies.

6. **[major] LLP 1123 “Safe areas, margins and bars” — The API mapping still specifies the previous scroller executor.**

   [The `contentInsetAdjustmentBehavior` row](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:185) limits automatic adjustment to the primary scroller’s top and bottom, says side clearance is wrapper padding, and makes every other scroller `.never`.

   **Failure:** Following that mapping prevents a horizontal carousel, including one nested inside a vertical list, from using D3’s automatic-adjustment reconciliation under the side strip.

   **Resolution:** Rewrite the row around D3’s reached edges, scrolling axes and reconciliation. Distinguish adjustment from the primary scroller’s separate role in driving bar collapse.

7. **[major] LLP 1123 D7 — Native `resizes-content` still has two incompatible scopes.**

   [D7’s `none` rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:450) preserves document policy, expressly including `resizes-content` shrinking the document viewport. C29 and stage 5 instead keep the native document and UIKit allocation full size and apply that policy to the editing route’s local block.

   **Failure:** With a sheet editor, root `interactive-widget="resizes-content"` and owner `keyboardAvoidance="none"`, one instruction shrinks the background document; the other changes only the editing route.

   **Resolution:** Put the native-versus-ordinary-page distinction directly in D7 and reconcile D1’s policy wording. State precisely what explicit `resizes-content` does when extra owner accommodation is disabled.

8. **[minor] Connected prose still contains unqualified superseded statements.**

   [LLP 1081 §8](/Users/admin/Developer/exact-owners/llp/1081-names-exact-invents.rfc.md:323) still says “`--exact-*` refused from author source,” although D5 now scopes the refusal. [LLP 1130 Related](/Users/admin/Developer/exact-owners/llp/1130-a-page-under-the-status-bar.rfc.md:11) still describes route clearance through covers, despite its new amendment header.

   **Failure:** Searching the corpus still returns blanket instructions that contradict the amended decisions.

   **Resolution:** Qualify the former as host-private properties and update the latter to D3. Under the requested corpus-wide rule, revision summaries also need consistent wording.

Round-2 findings, in order:

1. **Resolved:** The extended scroll-box basis separates scrolling-content layout from live stationary clearance; collapse/expansion qualification now includes percentage content.
2. **Resolved:** Rectangle-based reach rejects the reported centered-root and bordered-root cases and gives D11 the same eligibility rule.
3. **Resolved for the reported movement case:** Coordinate translation and commit-time offset checking cover movement without resize. Finding 2 above concerns the remaining intersection step.
4. **Resolved:** Six fields × four index pairs gives twenty-four properties, with explicit boundary resets and an explicit larger-grid limit.
5. **Resolved:** Shared qualification excludes pane-dimension rows and tests dimensions as lowering refusals.

What I checked and found sound:

- D12’s nearest size-container model, omission of content-sized owners and small-viewport fallback follow [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths). Keeping viewport units document-wide remains coherent.
- D9 consistently distinguishes translated controller regions from a subview’s own margins. Registered inset overrides and guaranteed-invalid segment resets remain appropriate.
- The substantive amendments to 1039, 1078 and 1115.002 D8.2 correctly express the newly approved decisions.
- 1127’s selection/association separation, base selection, coordinator deduplication and visibility publication remain sound. Inspector, toolbar groups and split-wide search are explicitly deferred.

This was a read-only specification and source review, including the capsule evidence and corpus searches. I did not rerun the simulator probes.
## Loop 3, round 4

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T05:52:46Z to 2026-10-25T05:59:42Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; the capsule gained exact diffs of every document against round 3 and of the connected LLPs against origin/main.
- **Target revisions:** LLP 1123 r4 sha256 `f810ac1eb1100eb8e54d8fcfcaec6ae08fe6ff19f186af81b5dad33ae4774a37`; LLP 1123.001 r4 `5efdd81fd5ef7cf7bd6af5a5658cff1ad18a3c9422ea9496907aaf1311425faf`; LLP 1127 r21 `3da4e404503f11b13c5ed0a599df7a2b702ca1fea173be923b448d4c779f4a3a`; brief `c6035819aef61df3bd551f08908f0f14b7d36b7a7c4cd873a46782e0decd0bee`; connected LLPs 1039, 1078, 1081, 1115.000, 1115.002, 1116.004, 1130 as in the worktree.
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1127 D4/D5 — Adjacent history-menu choices still have conflicting dispatch rules.**

   [D4](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:105) makes a menu selection a history selection that never presses authored Back. But clearing explicitly includes a history selection of the list, then says clearing with `b = 1` is one Back. D5 likewise assigns one Back to a one-route pop and a history selection to a menu pop, without making those cases disjoint.

   **Failure:** With P = `[root, A]`, choose the list from the history menu. One rule invokes A’s Back control, potentially opening a confirmation; another invokes `navigate` directly. With N1 = `[A, B]`, choosing A presents the same classification problem. LLP 1115.000 D4.A.3 currently specifies jumps over several visits; `NavigationIOS.completeTransition` distinguishes them by visit distance.

   **Resolution:** State the adjacent-menu policy explicitly and make the cases disjoint. If menu origin changes adjacent navigation semantics, amend 1115.000 and specify how the host identifies that origin. Qualify an adjacent menu choice both within N1 and from the base to the list.

2. **[major] LLP 1123.001 D6 — Coordinate conversion handles translation but not scale.**

   [D6](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:86) subtracts the content box’s offset before intersecting segments. Neither D5 nor D6 excludes transformed arrangements or ancestors.

   **Failure:** In the 951-point owner, put a 400×200 arrangement at x=100 with `scale(2)` and origin `(0,0)`. The owner’s fold at 455.5–495.5 is **177.75–197.75** in the arrangement’s coordinates. Translation alone produces **355.5–395.5**, placing panes across the actual fold. The web’s `getBoundingClientRect` measurement includes transforms; subtracting rectangle origins does not invert them. [CSSOM View](https://drafts.csswg.org/cssom-view/#dom-element-getclientrects)

   **Resolution:** Define conversion through the rendering transform, with intersection in a consistent coordinate space. Explicitly delimit transforms whose mapped divisions the axis-based algorithm cannot represent. Qualify scaling on the arrangement and on an ancestor.

3. **[major] LLP 1123.001 D5/D6 — Web placement still misses movement without a commit.**

   [The web executor](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:91) checks size changes, viewport-segment changes and layout commits. D5 warns about arrangements inside scrollers; it does not refuse them.

   **Failure:** Place an arrangement at y=100 across the laptop fold. Its local band is 355.5–395.5. Scroll its ancestor by 100 without an application handler: its top becomes 0 and the band must become 455.5–495.5. No arrangement resize, segment change or application commit is required. The old pane placement remains.

   **Resolution:** Cover scrolling and other admitted movement that occurs independently of commits, or explicitly refuse unsupported containment. Test scrolling across a stationary fold on both web targets. The sibling-height case fixed previously does not cover this case.

4. **[major] LLP 1123.001 D6 — The fallback does not define a 2×2 segment grid.**

   [D6’s placement rules](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:88) operate on two segments, while 1123 D9 expressly admits four and 1078 already supports a substituted 2×2 grid.

   **Failure:** A 1000×600 arrangement with centered 40-point horizontal and vertical bands has four 480×280 segments. The split chooses the horizontal axis, but “takes the first … and the second” does not say whether to use the top pair, combine rows, or fall back to equal halves. Those answers respectively abandon the lower area, cross the perpendicular band, or cross the vertical band.

   **Resolution:** Define selection or projection of multi-axis grids, including the perpendicular-band policy, or explicitly exclude these inputs from arrangement placement. Add the admitted grid case to qualification.

5. **[major] Connected keyboard decisions remain inconsistent, and their amendments are missing.**

   [LLP 1039 D4](/Users/admin/Developer/exact-owners/llp/1039-viewport-facts.rfc.md:131) still requires `exactViewport().height` to shrink on iOS under `resizes-content`. [1123 “What it replaces”](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:526) likewise says the keyboard ends the viewport, without limiting that statement to an ordinary page. Both conflict with revised D7/C29 for native owners.

   [1078’s keyboard section](/Users/admin/Developer/exact-owners/llp/1078-foldables-follow-the-web.rfc.md:599) also describes shortening document segments and explicitly cites duo-lab’s note sheet. Its reconciliation is absent from the staged amendments.

   **Failure:** An implementer preserves 1039’s rule while implementing owner-local keyboard resizing. Opening a sheet keyboard then changes background viewport branches, despite C29 requiring the document to remain full size.

   **Resolution:** Amend 1039’s policy and verification text with the native-owner distinction. Qualify 1123’s viewport sentence. Add 1078’s keyboard section to stage 5’s obligations, distinguishing document segments from the editing owner’s local geometry. Preserve ordinary-page behavior.

6. **[minor] LLP 1123 C8/D3 versus D12 — The “no content layout changes” promise needs an authored-length exception.**

   [D12](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:516) deliberately makes container units follow live clearance, whereas C8/D3 promise collapse changes no scrolling-content layout.

   **Failure:** In the supplied 874-point example, the content root grows from 672 to 724 points during collapse. A scroller child with `height: 50cqh` therefore grows from **336 to 362**, despite the unconditional promise.

   **Resolution:** Preserve the live container-unit semantics and qualify C8/D3: automatic inset changes preserve the scroller’s layout basis, while explicitly authored lengths that depend on changing owner geometry may relayout. Add that distinction to qualification. Container units refer to the selected container’s dimensions. [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths)

7. **[minor] Superseded terminology remains in the reviewed documents.**

   [1123’s arrangement API row](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:222) still says `navigationArrangement` waits for an app and points to D7. The HIG table says arrangements await their own LLP, although 1123.001 now supplies that proposal. [1127’s runner qualification](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:210) still requires “D4’s Back steps,” which r21 deletes.

   **Failure:** Readers find an obsolete property proposal and a test instruction referring to a removed algorithm.

   **Resolution:** Point the arrangement passages to 1123.001’s pending `-exact-arrangement` decision; replace “Back steps” with the distinct Back, history-selection and clearing operations.

**Round-3 findings, in order:**

1. **Partially resolved:** Multi-visit selection now follows the current contract; adjacent-menu classification remains finding 1.
2. **Resolved for the reported case:** Content-box translation, intersection, empty removal and the document source are specified.
3. **Resolved:** The C ABI advances beyond 16, with preceding-cohort exclusion qualified separately from the module ABI.
4. **Resolved:** The identified split and bar decisions now carry the pending amendments.
5. **Resolved for the reported passages:** 1116.000, 1116 and 1131 are listed, with band composition specified; finding 5 identifies another omission.
6. **Resolved:** The mapping now distinguishes reached scrollers, axis reconciliation and primary-scroll tracking.
7. **Partially resolved:** D7’s `none` rule is corrected; the remaining viewport instructions are finding 5.
8. **Resolved for the identified passages:** 1081 §8 and 1130’s Related text are corrected.

**Checked and found sound:**

- The supplied arrangement intersection example produces the stated rectangles.
- The expanded-inset layout basis now preserves percentage-based scrolling-content geometry while stationary content follows live clearance.
- The document-wide `env()`/owner-local property distinction, registered inset overrides and explicit segment resets remain coherent.
- Split selection versus association, base retention, coordinator deduplication and visibility publication remain sound.
- The identified connected split amendments and separate C/module ABI changes are consistent.

This was a read-only specification and source review against the capsule evidence and cited standards. No files were changed; simulator probes were not rerun.
## Loop 3, round 5

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T06:03:31Z to 2026-10-25T06:13:34Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 4.
- **Target revisions:** LLP 1123 r5 sha256 `d4b9c06266a3201e540eb8179e028919ccb194640d904997ce88a909dfacc894`; LLP 1123.001 r5 `f3e71f2b6808a64d563e7d9f5701cf1019b26e63def075df3af059e790182726`; LLP 1127 r22 `0f83fdc817c75597b5f0bbc453cefdc3b994ea177d4677b689ae37d73c545dd4`; brief `863ef91b6986bfc0894b9f393940bc70dc4e0261db7f25f5901e35bb046aa591`; connected LLPs as in the worktree.
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123.001 D6 — Placement still misses admitted movement between commits.**

   [The web executor](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:91) now covers page scrolling, but its remaining triggers do not cover browser-driven layout motion.

   **Failure:** Release an admitted height drag on an earlier in-flow sibling, letting its height spring from 100 to 200. A fixed 500×500 arrangement moves from y=100 to y=200 without changing size. With the laptop band at 455.5–495.5, its local band must move from **355.5–395.5** to **255.5–295.5**. Neither ancestor scrolling nor a transform is involved. The browser can run the animation between application commits; observing the arrangement’s unchanged size supplies no notification. [Resize Observer observes size changes.](https://drafts.csswg.org/resize-observer/)

   **Resolution:** Specify placement updates during admitted layout animations and other asynchronous layout movement, or explicitly exclude those compositions. Qualify an animated preceding sibling on both web targets. Checking again after the initiating commit is insufficient.

2. **[major] LLP 1123.001 D6 rule 3 — A default overlay still has two possible answers for a 2×2 grid.**

   [Rule 3](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:89) accepts two columns **or** two rows along an allowed axis. With the default allowing both axes, both conditions hold. Rule 1 selects an axis expressly for **split**; no corresponding overlay tie-break is stated.

   **Failure:** A 1000×600 overlay has centered 40-point bands on both axes. The primary can take either the trailing **480×600 column** or the bottom **1000×280 row**. Both satisfy rule 3, and they put the content in different places. Tests “along each axis” do not establish which answer an unrestricted overlay must produce.

   **Resolution:** Define a deterministic choice when both axes qualify, including square containers, or explicitly exclude that input. Test unrestricted overlays separately from `overlay horizontal` and `overlay vertical`.

3. **[major] Connected LLP 1012 §1 — The agent protocol still requires document-level keyboard resizing.**

   [The `layout` reply contract](/Users/admin/Developer/exact-owners/llp/1012-agent-api-v1.spec.md:260) unconditionally says that under `resizes-content`, “`viewport` itself shrinks to the keyboard’s top.” LLP 1123’s staged amendments do not identify this passage.

   **Failure:** Focus an editor inside a native sheet under `resizes-content`. Revised 1039 D4 and 1123 C29 require the document viewport to remain full size while the editing owner’s block shortens. Following 1012 instead makes the agent report a shortened document viewport, disagreeing with `exactViewport()` and the new ownership contract.

   **Resolution:** Apply the native-owner distinction to the protocol text and add its amendment to stage 5. Qualify equality between `layout.viewport` and `exactViewport()` with both an owner-contained editor and an ordinary page editor.

4. **[minor] LLP 1123 “Amendments” — The new keyboard amendment drops the policy distinction.**

   [The stage-5 amendment](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:630) says an editor inside an owner “ends that owner’s block and its segments … at the keyboard,” without restricting block shortening to `resizes-content`.

   **Failure:** With default policy and a bottom-reaching primary scroller, D7 and stage 5 require an inset while preserving the block and percentage-content layout. Implementing this amendment literally shortens the block instead.

   **Resolution:** Scope block shortening explicitly to `resizes-content`. State separately how any keyboard clipping of reported segments works under inset-only accommodation; it must not implicitly resize the block.

5. **[minor] LLP 1127 D4 — “Only the history menu” contradicts clearing.**

   [The history-selection definition](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:108) calls a multi-visit pop something “only the history menu makes.” The next bullet correctly assigns the same classification to other clearings.

   **Failure:** After two expanded selections leave P=`[root, A, B]`, b=2 and N1=`[B]`, collapse and press Back. Clearing reaches the root across two visits through the button, without using the menu.

   **Resolution:** Remove the exclusivity claim or include the multi-visit clearing cases. Keep the visit-distance dispatch rule.

**Round-4 findings, in order:**

1. **Resolved:** Both adjacent-menu cases now dispatch one Back by visit distance; finding 5 is a separate wording contradiction.
2. **Resolved for the reported cases:** Transformed arrangements and ancestors explicitly lose fallback segments, with qualification added; UIKit measurement remains an implementation obligation.
3. **Partially resolved:** Scroller containment and page scrolling are addressed; independent layout motion remains finding 1.
4. **Partially resolved:** Split projection and perpendicular-band behavior are defined; unrestricted overlays remain ambiguous in finding 2.
5. **Partially resolved:** The identified 1039 and viewport passages are corrected and the requested obligations added; findings 3–4 cover remaining keyboard inconsistencies.
6. **Resolved:** Live container-unit lengths are explicitly excepted, and the `50cqh` qualification is added.
7. **Resolved:** The arrangement references and model-based test terminology are corrected.

**Checked and found sound:**

- The adjacent-menu classification agrees with `NavigationIOS.completeTransition`’s multi-visit distinction.
- Arrangement translation, content-box intersection and empty-intersection removal produce the supplied worked rectangles.
- The split’s 2×2 projection now spans the perpendicular axis and explicitly permits that axis’s band through both panes.
- Document-wide `env()`, owner-local properties, translated controller regions and explicit absent-segment resets remain coherent. Registered inset values and unregistered segment values retain distinct semantics. [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api/#computed-value-time-behavior)
- The expanded-inset scrolling-content basis, live stationary clearance and authored container-unit exception fit together.
- Selection versus association, coordinator deduplication, cancellation, visibility publication and the connected split/bar amendments remain sound on this review.
- Inspector, toolbar-group and split-wide-search support remains explicitly deferred.

I reviewed the revisions, supplied deltas, previous findings, capsule evidence, relevant source, SDK headers and web specifications. I made no changes and did not rerun simulator probes.
## Loop 3, round 6

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T06:17:34Z to 2026-10-25T06:26:17Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 5; the brief added implementation evidence from the Duo simulator.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `437e8b686999e9714d1403b6c8897c9c3120d95de70838d2a56bc4d6426082c0`; llp/1123.001-arrangements.rfc.md `e9c74c239d6e22b7555830e9056c6dd84da03cb46008cce4c99a47253c9a0124`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `cfd11ba98f37ddfe080eb00fb6b08062856b05571f00e9ab2dd154442e81860a`; brief `5103b270dc8311255688fa63044646dada2a65e37cf9bd0d3701315ea9ae6c5a`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123.001 D6 rule 3 — The overlay tie-break suppresses adaptation to a single fold.**

   [Rule 3](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:89) now chooses the shape’s axis whenever both axes are allowed, even when only the other axis has a division.

   **Failure:** A 500×500 default overlay crosses a book fold, with its local vertical band at x=230–270. Rule 1 chooses down because the box is square. There are no two rows, so both panes fill the box and cross the fold. The same box with `overlay horizontal` separates its panes into the two 230-point columns. Allowing both axes therefore disables fold accommodation that allowing one provides. This also contradicts §1’s overlay rule: a division across an allowed axis produces side-by-side placement.

   **Resolution:** First identify the allowed axes that actually have divisions. Use the sole qualifying axis; apply the shape tie-break only when both qualify. Qualify default overlays with a book fold in a square/tall box and a laptop fold in a wide box, against UIKit.

2. **[major] LLP 1123 D7 and stage-5 amendments — “The default policy” conflicts with iOS’s specified and implemented default.**

   [The new amendment](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:630) says the default shortens no block. But [LLP 1008 §9](/Users/admin/Developer/exact-owners/llp/1008-apple-host-v1.spec.md:1060) expressly makes `resizes-content` iOS’s default, and says its owner-block shortening applies to both default and explicit policy. [The presenter implements that default](/Users/admin/Developer/exact-owners/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:145). LLP 1116 D2 records it too.

   **Failure:** Open the keyboard in a route whose root omits `interactive-widget`, with a bottom-reaching primary scroller containing percentage-sized content. R6 requires unchanged layout plus an inset; 1008 requires a shorter editing block. Both describe the same unannotated app.

   **Resolution:** Decide whether stage 5 changes iOS’s default or preserves it. State the effective policy explicitly, distinguish it from `keyboardAvoidance="auto"`, and reconcile 1008, 1116 and qualification. Merely saying “explicit `resizes-content`” leaves the existing default unresolved.

3. **[major] LLP 1123.001 D6 — Placement still misses asynchronous intrinsic-size changes.**

   [The web triggers](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:91) cover the reported spring, but not all admitted layout movement.

   **Failure:** Put an image with an unresolved intrinsic height before a fixed 500×500 arrangement in a fixed-size owner. Its eventual load adds 100 points of height, moving the arrangement from y=100 to y=200. Its local laptop band must change from 355.5–395.5 to 255.5–295.5. The arrangement and owner retain their sizes; there is no animation, scroll or application commit. With no authored image-load handler, the JS target need not dispatch anything. LLP 1011 explicitly delegates this intrinsic-size layout change to the browser.

   Thus observing the arrangement’s size and following animation events still leaves stale pane placement.

   **Resolution:** Include browser layout changes caused by image/media readiness and font loading in placement invalidation. Qualify an earlier image loading without an authored handler on both web targets, with arrangement and owner sizes unchanged.

4. **[major] LLP 1123 D7/D9 and stage-5 amendments — Preserving segment extents is incorrect when keyboard accommodation shortens the wrapper.**

   [D7](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:447) pads the wrapper when the focused editor is outside a bottom-reaching primary scroller; stage 5 expressly says the wrapper shortens. [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:467) defines segments from the content root’s box. The new amendment nevertheless preserves owner segment extents under its default policy without limiting that promise to scroller-inset accommodation.

   **Failure:** An 800-point content root has a fold band at 350–390 and a composer below its list. Adding 300 points of keyboard padding shortens the content root to 500. Its lower segment must become **110** points tall; preserving its previous **410** points reports geometry beyond the content root.

   **Resolution:** Distinguish scroller-only insetting, wrapper accommodation and explicit block resizing. Preserve segment extents only when their defining content-root rectangle stays unchanged; otherwise recompute them. Qualify the composer case alongside the bottom-reaching-scroller case.

5. **[minor] Connected LLPs still contain superseded, unqualified claims.**

   The corpus audit found additional passages beyond the already listed amendments:

   - [LLP 1039 §2](/Users/admin/Developer/exact-owners/llp/1039-viewport-facts.rfc.md:62) still says iOS frames the session to the keyboard and reports that viewport on every host, without D4’s native-owner distinction.
   - [LLP 1001’s viewport-segment deviation](/Users/admin/Developer/exact-owners/llp/1001-kernel-v1.spec.md:137) still describes keyboard shortening of the iOS document viewport and document segments without that distinction. Its amendment is absent from stage 5’s list.
   - [LLP 1008 §9, “Reserved regions, evaluated”](/Users/admin/Developer/exact-owners/llp/1008-apple-host-v1.spec.md:713) still asserts that the occlusions are already safe-area insets, whereas revised 1078 D5 and 1123 C14a correctly leave the camera’s inclusion unconfirmed. The same passage says no consumer or kernel support exists for fold geometry, despite LLP 1078 and Duo Lab.

   **Failure:** A reader following these passages gets document-level keyboard resizing or treats an unverified camera clearance as established.

   **Resolution:** Rewrite the remaining claims with their current scope. Add 1001’s keyboard paragraph to stage 5’s explicit obligations and reconcile 1008’s occlusion/support statements.

**Round-5 findings, in order:**

1. **Resolved for the reported spring case:** animation-frame placement is specified; finding 3 identifies remaining asynchronous movement.
2. **Ambiguity resolved, with a regression:** unrestricted 2×2 overlays now have one answer; finding 1 covers single-axis folds.
3. **Resolved as a staged obligation:** LLP 1012’s reply and owner/ordinary-page equality qualification are explicitly named.
4. **Partially resolved:** shortening is scoped to `resizes-content`, but the default-policy conflict and wrapper case remain findings 2 and 4.
5. **Resolved:** history selection now includes multi-visit clearing initiated by the button or a drag.

**Checked and found sound:**

- Arrangement translation, content-box intersection, empty-intersection removal, perpendicular-band projection, stacking and hidden-state publication remain coherent.
- Document-wide `env()`, translated controller regions, registered inset overrides and explicit absent-segment resets retain their distinct meanings. The nearest-container and small-viewport fallback rules agree with [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- The expanded-inset scrolling-content basis and explicit live-container-unit exception fit together.
- LLP 1127’s selection/association separation, visit-distance dispatch, coordinator deduplication, cancellation, visibility publication and ABI distinctions remain sound on this review. Inspector, toolbar groups and split-wide search remain explicitly deferred.

I reviewed the revisions, deltas, prior findings, capsule evidence, connected text, relevant source, SDK headers and web specifications, and checked the numerical counterexamples. No files changed; simulator probes were not rerun.
## Loop 3, round 7

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T06:27:53Z to 2026-10-25T06:40:50Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 6.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `0fa3d5df9bc56e43ae9000141281f442317f0be7d64f3ff655408f6dc55c4278`; llp/1123.001-arrangements.rfc.md `914f1ece4c9967fd8347b6550e1fcb1675131eddc8cff105b7d0fb7161daec1c`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `cfd11ba98f37ddfe080eb00fb6b08062856b05571f00e9ab2dd154442e81860a`; brief `ea145f0164ce42b3b10b39b243e5b33ecbf0ad255018c5e99532726b32da3409`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D7 — Choosing the keyboard executor can change the condition that chose it.**

   [D7](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:447) chooses between a scroller inset and wrapper padding using D3’s bottom-reach test, but does not specify which layout that test uses.

   **Failure:** Use `resizes-visual`, an 800-point owner with clearance opted out, a root filling its containing block, and a fixed-height 500-point primary scroller containing the editor. The docked keyboard starts at y=500.

   Initially the scroller does not reach the root’s bottom, so D7 adds 300 points of wrapper padding. The content root becomes 500 points high; now the scroller reaches its bottom. D7 therefore selects a scroller inset instead, removing the wrapper padding. The root returns to 800 points and the scroller stops reaching it. Evaluating reach against the accommodated layout has no stable answer.

   **Resolution:** Define reach for keyboard-executor selection against layout before Exact’s keyboard accommodation, after any platform/policy resize. Ensure injected padding cannot change that selection. Qualify this fixed-height-scroller case alongside the filling-list and composer cases.

2. **[major] LLP 1123 D9 — Keyboard shortening is accounted for in segments, but not in safe-area and region values.**

   [D9’s table](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:465) still reports `local` on an opted-out edge and computes regions by subtracting only that edge’s clearance.

   **Failure:** An 800-point controller has a 34-point bottom safe inset. Set `-exact-safe-area="bottom"`, use `resizes-visual`, and focus a composer outside the primary scroller. A keyboard occupying y=500–800 produces 300 points of wrapper padding, so the content root ends at y=500.

   D9 still publishes bottom safe inset **34**. A view occupying that content-root rectangle has bottom safe inset **0**: its bottom no longer overlaps the controller’s bottom obstruction. Padding with the published property consequently adds an unnecessary 34-point gap above the keyboard. Controller-region insets have the same translation problem. UIKit’s insets describe the portion of the queried view that is covered. [Apple’s safe-area documentation](https://developer.apple.com/documentation/uikit/uiview/safeareainsets?changes=_7)

   **Resolution:** Translate every geometry family into the final content-root rectangle, including keyboard accommodation and policy shortening. The simple `local`/`region − clearance` formulas need that qualification. Test opted-out bottom clearance with wrapper accommodation, plus `resizes-content`.

3. **[major] LLP 1123.001 D6 — Observing earlier siblings still misses admitted asynchronous movement.**

   [The revised observer set](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:91) covers the reported earlier-image case, but assumes that only earlier siblings can move the arrangement.

   **Failure:** In a fixed-size owner, use a fixed-height flex column with `justify-content: center`. Its first child is a fixed-size arrangement; its second is an image whose intrinsic height increases by 100 points when it loads.

   Centering moves the arrangement **50 points upward**. The arrangement, owner and ancestor sizes remain unchanged; there are no earlier siblings, animation, scroll or application commit. Only the later image resizes, and it is outside the specified observation set. Pane placement therefore retains the old fold coordinates. This follows from flex centering and ResizeObserver’s size-based activation. [Flexbox alignment](https://www.w3.org/TR/css-flexbox-1/#justify-content-property), [ResizeObserver activation](https://drafts.csswg.org/resize-observer/#dom-resizeobservation-isactive)

   **Resolution:** Invalidate placement for intrinsic media/font changes affecting the owner regardless of sibling order, or observe the relevant layout participants without restricting them to earlier siblings. Add this centered-flex example on both web targets.

4. **[minor] Remaining passages still state superseded rules, beyond the listed amendments.**

   - [1123 C25](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:128) says a route’s keyboard overlap becomes a scroller inset and “nothing else shrinks.” That needs an explicit `resizes-visual` scope now that D7 identifies `resizes-content` as iOS’s default.
   - [1008 §9’s older viewport-facts paragraph](/Users/admin/Developer/exact-owners/llp/1008-apple-host-v1.spec.md:743) still unconditionally makes the keyboard-adjusted frame the document viewport and `exactViewport().height`. Its newer keyboard section correctly distinguishes native navigation.
   - [1086.000.000 D2, “Keyboard”](/Users/admin/Developer/exact-owners/llp/1086.000.000-improvements-from-the-first-diaries.rfc.md:136) still describes the default through the inset reading and says `resizes-content` ends the viewport, without the native-owner distinction.
   - [1080.001 §1](/Users/admin/Developer/exact-owners/llp/1080.001-native-tree-and-model-agreement.rfc.md:153) says the viewport moves into presented controllers; [1080.002 D3](/Users/admin/Developer/exact-owners/llp/1080.002-the-accessibility-tree-in-the-agents-hands.rfc.md:142) likewise describes modal controllers as holding that viewport.
   - [1115.002’s Related entry](/Users/admin/Developer/exact-owners/llp/1115.002-the-macs-structure.rfc.md:10) still calls the Mac sidebar/split work queued, despite its own built record.

   **Failure:** Following these passages yields document-level keyboard resizing, the former presentation hierarchy, or an incorrect account of available split support.

   **Resolution:** Correct claims already contradicted by built behavior; explicitly include the remaining passages in the appropriate staged amendments.

**Round-6 findings, in order:**

1. **Resolved:** Overlay selection now considers divided allowed axes first; shape breaks only a genuine tie. UIKit qualification remains explicitly owed.
2. **Resolved in D7 and the amendment:** iOS’s default is named consistently. C25’s remaining contradiction is included in finding 4.
3. **Partially resolved:** The earlier-image example is covered; finding 3 identifies movement from an unobserved later sibling.
4. **Resolved for segment extents:** Segments now follow the content root under each accommodation. Finding 2 concerns the other geometry families.
5. **Resolved for the reported passages:** 1039 §2 and 1008’s reserved-region text are corrected, and 1001’s segment paragraph is listed for stage 5. Finding 4 records additional omissions.

**Checked and found sound:**

- Arrangement intersection, empty-intersection removal, split projection, perpendicular-band handling, overlay stacking and hidden-state publication remain coherent.
- Document-wide `env()`, owner-local properties, explicit absent-segment resets and authored inset overrides retain distinct meanings.
- Container-unit selection and small-viewport fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- The expanded-bar scrolling-content basis and live container-unit exception remain consistent.
- Unchanged 1127 r23 retains coherent selection/association separation, visit-distance Back dispatch, coordinator deduplication, cancellation, visibility publication and separate ABI obligations.
- Camera occlusion uncertainty and the deferred inspector, toolbar-group and split-wide-search scope are stated clearly.

I checked the r7 snapshot hashes, revisions, deltas, prior findings, capsule evidence, relevant source and connected LLP passages. I checked the numerical counterexamples without changing files. UIKit and browser probes were not rerun.
## Loop 3, round 8

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T06:42:27Z to 2026-10-25T06:50:51Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 7.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `8d7eb1f1a4419a51abbd7da7c962b5ea67975bc3f38fa3d7a9751beae1961c74`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `cfd11ba98f37ddfe080eb00fb6b08062856b05571f00e9ab2dd154442e81860a`; brief `fcb1fb0282f3d4023e0ffa0c63d47d9818d87bf6943c259ea56f36938d9d90f7`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D3/D7 — Automatic clearance still includes a bottom obstruction the resized block no longer reaches.**

   [D3](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:328) defines `local` as the controller view’s safe-area inset and unconditionally makes it clearance. D7 shortens the editing owner’s block under `resizes-content`, while keeping that controller full size. R8 corrects D9’s published values, but leaves this clearance calculation unchanged.

   **Failure:** A controller is 800 points tall, with a 34-point bottom safe inset. Use default clearance, no authored padding, and a composer below the list. The keyboard starts at y=500. Policy shortens the block to 500; D3 then subtracts the controller’s unchanged 34-point clearance, placing the composer’s bottom at **466**, leaving an unnecessary **34-point gap** above the keyboard. D7’s residual overlap is zero, so nothing corrects it.

   This differs from constraining content above both the safe area and keyboard: the keyboard becomes the limiting boundary, without another home-indicator inset above it. [Apple’s keyboard-layout example](https://developer.apple.com/videos/play/wwdc2023/10281/?time=473)

   **Resolution:** Distinguish the controller’s raw insets from the clearance applicable to the policy-adjusted block. Translate/intersect the controller’s safe region into that block before applying automatic clearance. Preserve authored padding. Qualify a default-cleared bottom edge under `resizes-content`, alongside r8’s opted-out cases.

   The existing [NavigationCoversIOS.swift](/Users/admin/Developer/exact-owners/host/apple/Sources/ExactKit/IOS/NavigationCoversIOS.swift:95) already limits bottom clearance to what the shortened route reaches; replacing covers must preserve that behavior.

2. **[minor] LLP 1123 C25 — The corrected policy distinction still overstates when scroller accommodation applies.**

   [C25](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:128) now scopes “nothing else shrinks” to `resizes-visual`, but applies it to any route with a scroller. D7 requires the focused editor to be inside the bottom-reaching primary scroller.

   **Failure:** Under `resizes-visual`, focus a composer below the list, or an editor inside round 7’s fixed-height scroller that does not reach the bottom. D7 selects wrapper padding and shortens the content root; C25 instead requires a scroller inset and no shrinking.

   **Resolution:** Limit that cell’s inset promise to D7’s qualifying scroller, and name wrapper accommodation otherwise.

3. **[minor] Connected LLPs — Further superseded statements remain outside the amendment list.**

   - **[LLP 1021 §5.1, focus while the menu shows](/Users/admin/Developer/exact-owners/llp/1021-menus.rfc.md:909)** says the content-touch recognizer sits on the viewport, “which a presentation carries along.” Under 1123 D2 the viewport stays behind. Following that instruction leaves sheet-content touches unable to restore focus after a bar menu. Rewrite it around the actual mount: the viewport or the presentation containing the bar, as `MenuFocusIOS.watch` already does. Include this passage in stage 1’s amendments.
   - **[LLP 1039 §4](/Users/admin/Developer/exact-owners/llp/1039-viewport-facts.rfc.md:292)** says “a phone never sees the rail,” although its example selects the rail at width ≥900. The supplied Duo measurement is 951 points open, so that phone takes the rail branch. Describe the numeric predicate and include wide foldable phones.
   - **[LLP 1115.000 §3, wave-1 built record](/Users/admin/Developer/exact-owners/llp/1115.000-no-tells-on-every-host.rfc.md:859)** still calls the Mac sidebar/split half queued. Without a supersession note, this conflicts with 1115.002’s built record. Update the parenthetical or explicitly delimit it as the earlier snapshot and point to the completed work.

**Round-7 findings, in order:**

1. **Resolved:** Keyboard executor selection now uses layout before accommodation; the fixed-height-scroller qualification is explicitly owed.
2. **Resolved for the reported property values:** D9 translates all families into the final content-root rectangle. Finding 1 identifies the separate automatic-clearance defect.
3. **Resolved:** Per-frame offset sampling while segments exist covers the later-image example and the other admitted movement sources.
4. **Resolved for the requested corrections and staged obligations:** C25’s policy distinction is fixed; finding 2 concerns its remaining executor qualification. Finding 3 records additional corpus omissions.

**Checked and found sound:**

- Arrangement intersection, empty-intersection removal, perpendicular folds, overlay axis selection, stacking and hidden-state publication remain coherent. The revised web sampling removes the incomplete trigger inventory.
- Explicit `initial` resets for absent unregistered segment properties preserve fallbacks and prevent inheritance leaks; registered inset overrides retain computed-length inheritance. These distinctions agree with [CSS Variables 1](https://www.w3.org/TR/css-variables-1/#invalid-variables).
- Container-unit ancestor selection and small-viewport fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths). The expanded-bar scrolling-content basis and live-container-unit exception remain consistent.
- Unchanged 1127 r23 retains coherent selection/association separation, visit-distance Back dispatch, coordinator deduplication, cancellation, visibility and separate ABI obligations.
- Camera-occlusion uncertainty, inspector, toolbar-group and split-wide-search deferrals remain explicit.

I reviewed the revisions, deltas, prior findings, capsule evidence, connected passages and relevant source, and executed the numerical clearance counterexample. No files changed; simulator probes were not rerun.
## Loop 3, round 9

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T06:51:36Z to 2026-10-25T07:00:57Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 8.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `a5e3b41cebe5488cefa809a06937caabef107fbbaf29ae866778b385bde13067`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `cfd11ba98f37ddfe080eb00fb6b08062856b05571f00e9ab2dd154442e81860a`; brief `4fc91e7f00b203ee3feaf9953d16e7a3d6e15e8a796da772efec527007b822c0`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D6/D7 — The feedback fence omits keyboard-mediated changes to the reported allocation.**

   [D6’s bake rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:425) checks paths into projected chrome and presentation attributes. D7 also makes the reported block depend on which editor owns the keyboard.

   **Failure:** In an activated iOS session, an owner reports height 800. Its content mounts an `autofocus` field when `detail.available and detail.height >= 600`. The keyboard starts at y=500, so default `resizes-content` shortens the block to 500. The branch removes the field; the keyboard dismisses; height returns to 800; the field mounts and autofocuses again.

   The branch changes no header, tablist or presentation attribute, so the specified bake check admits this cycle. Existing [focus semantics](/Users/admin/Developer/exact-owners/docs/contract-grammar.md:2244) permit a newly mounted autofocus field to take focus when nothing holds it.

   **Resolution:** Extend the fence to paths that change keyboard ownership, including conditional editor mounting and focus/blur commands, whenever those paths can change the reported geometry. Alternatively, define a keyboard-independent reported allocation and reconcile D6 with D7. Qualify the conditional-field case before admitting the fact.

2. **[minor] LLP 1123, SwiftUI API mapping — `keyboardAvoidance="none"` does not provide the promised keyboard-safe-area opt-out.**

   [The mapping row](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:237) presents it as the counterpart of ignoring SwiftUI’s `.keyboard` region.

   **Failure:** An author sets `keyboardAvoidance="none"` to keep a full-height owner behind the keyboard. Under iOS’s default `resizes-content`, D7 still shortens that owner’s block. The attribute suppresses only additional accommodation; it does not ignore the keyboard’s safe region. Apple defines that region by the software keyboard’s extent. [Apple documentation](https://developer.apple.com/documentation/swiftui/safearearegions/keyboard)

   **Resolution:** Qualify the mapping explicitly. Describe `none` as disabling Exact’s additional accommodation, and explain the root-policy combination needed to preserve layout behind the keyboard. D7’s policy-preserving behavior can remain.

3. **[minor] Connected LLPs — The Mac’s completed sidebar and split remain described as queued.**

   Two passages retain the claim corrected elsewhere this round:

   - [LLP 1116.004 §4, D1 built-results row](/Users/admin/Developer/exact-owners/llp/1116.004-what-round-6-asks.rfc.md:213): “iPad and the Mac are queued,” because their hosts cannot lay out a route at column width.
   - [LLP 1038 D12’s amendment](/Users/admin/Developer/exact-owners/llp/1038-router.rfc.md:633): “the iPad and Mac presenters are queued.”

   **Failure:** A reader following either passage concludes that Mac column layout is unavailable, contradicting LLP 1115.002’s completed stages and the newly corrected parent record.

   **Resolution:** Correct the Mac status now and link D3/D4 of 1115.002. Keep the pending iOS work distinct. Although 1127 schedules the 1038 correction for implementation, the Mac portion is already stale independently of that implementation.

4. **[minor] Connected LLP 1102 §3.13 — The safe-area math recommendation is superseded without being marked so.**

   [The section](/Users/admin/Developer/exact-owners/llp/1102-what-the-authoring-bench-asks-for.rfc.md:345) still recommends a two-box workaround followed by a narrow `max(length, env())` kernel kind, and leaves general comparison expressions awaiting an RFC.

   **Failure:** An author avoids supported math, or an implementer follows the recommendation and adds the narrow representation, despite [1001’s implemented calculation trees](/Users/admin/Developer/exact-owners/llp/1001-kernel-v1.spec.md:1018) already covering nested comparisons, percentages and viewport lengths. LLP 1123 D5 expressly builds on that implementation.

   **Resolution:** Mark the proposal and recommendation superseded by the implemented calculation-tree surface. Point owner-local spacing to 1123 D3/D9. The historical benchmark observations can remain clearly identified as such.

**Round-8 findings, in order:**

1. **Resolved:** D3 limits clearance to the shortened block’s remaining overlap; authored padding remains additive, and stage 5 names the cleared-bottom qualification.
2. **Resolved:** C25 now limits scroller accommodation to the qualifying bottom-reaching primary scroller and names wrapper padding otherwise.
3. **Resolved for the identified passages:** The menu-focus mount amendment is listed, 1039 includes wide foldables, and 1115.000’s Mac status is corrected. Finding 3 above identifies further occurrences.

**Checked and found sound:**

- R9’s clearance arithmetic removes the reported gap: an 800-point view with bottom inset 34 has zero bottom clearance when its block ends at 500. A block ending at 780 retains 14; authored padding still adds.
- Document-wide `env()` and inherited owner properties retain coherent, distinct scopes. Explicit `initial` resets prevent absent unregistered segments from inheriting; registered inset properties retain computed-length inheritance. [CSS Environment Variables](https://drafts.csswg.org/css-env-1/), [CSS Custom Properties](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid), [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api/#computed-value-time-behavior)
- Container-unit ancestor selection and small-viewport fallback remain consistent with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Unchanged 1123.001 r8 retains coherent intersection, empty-intersection removal, perpendicular-fold handling, overlay axis selection, stacking, visibility and web position sampling.
- Unchanged 1127 r23 retains coherent selection/association separation, visit-distance Back dispatch, coordinator deduplication, cancellation, visibility publication and separate ABI obligations.
- Camera uncertainty and inspector, toolbar-group and split-wide-search deferrals remain explicit.

This was a read-only specification/source review against the capsule and connected corpus. All three targets matched `snap-r9`. I checked the numerical examples; I did not rerun UIKit or browser probes.
## Loop 3, round 10

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:02:06Z to 2026-10-25T07:09:28Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 9; the brief added the passing Duo smoke.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `660861cfa82c258d3136c5bdd50f67f7185d2eeb16b280c0fa2fd440fcb1c49c`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `a8e1ea3fff643ff3ab99242ff116909b626ed7bc3ea1f28b31c73aecf62b8e05`; brief `07863da1b76bcf2e1aa6f60c830cc923072960c3afd2ae4a185fe184449f145c`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D2/D6 — The keyboard-independent fact lacks a defined source when UIKit resizes the owner.**

   [D6](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:422) excludes all keyboard accommodation and promises that a keyboard never changes the fact. But D2 obtains a detented sheet’s allocation from its installed view’s bounds, and C26/D7 explicitly retain UIKit’s keyboard-driven sheet growth. Apple confirms that a medium sheet grows for the keyboard and contracts when it dismisses. [Apple’s sheet presentation explanation](https://developer.apple.com/videos/play/wwdc2021/10063/)

   **Failure:** A medium sheet mounts an autofocus field while its reported height is below a threshold between its resting and keyboard-expanded heights. Publishing the actual controller bounds makes keyboard expansion remove the field; dismissal contracts the sheet and mounts it again. Removing Exact’s block trim does not remove this path.

   Reading D6 literally instead requires geometry for the sheet *as though the keyboard were absent*. D2 supplies no such publication source. Merely retaining the previous value leaves initialization with an already-visible keyboard, rotation and detent changes undefined.

   **Resolution:** Define a separate keyboard-independent allocation and how the host obtains and updates it through those cases, or retain actual allocation and extend the feedback fence to keyboard-mediated dependencies. Qualify a medium sheet and an owner first installed while the keyboard is visible. The ordinary full-height route counterexample is addressed; the general independence claim remains incomplete.

2. **[minor] LLP 1123, Motivation — The comparison-function refusal remains stated in the main proposal.**

   [The local-insets bullet](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:39) still says `max(16px, env(safe-area-inset-bottom))` is refused. D5 correctly identifies these calculations as already implemented, and round 10 fixes precisely this misconception in LLP 1102.

   **Failure:** A reader concludes that ordinary document-level safe-area math requires this proposal or a workaround.

   **Resolution:** Remove the refusal claim. The missing owner-local value is sufficient motivation.

3. **[minor] Connected LLP 1115.002 D8.2 — Its future iPad instruction still prescribes root covers.**

   [“What does not change”](/Users/admin/Developer/exact-owners/llp/1115.002-the-macs-structure.rfc.md:1252) says `Edges` and `Inset` apply inside a block and that iPad columns “will use” them. This conflicts with 1123’s wrapper clearance and 1127’s requirement to land that mechanism before the split.

   **Failure:** Following the shared host-block contract adds side-cover padding to a column’s authored root. A content-box root with `width: 100%` then overflows by that padding—the failure 1127 Q3 specifically avoids.

   **Resolution:** Replace the iPad prediction with owner-wrapper clearance and explicitly include this paragraph in stage 2’s reconciliation. Preserve any cover behavior still applicable to other blocks separately.

4. **[minor] Connected LLP 1078, “As built (app half)” — The obsolete owner-coordinate recipes remain unreconciled.**

   [The Fold and Combinations description](/Users/admin/Developer/exact-owners/llp/1078-foldables-follow-the-web.rfc.md:369) still gives document `env(viewport-segment-*)` expressions inside routes, including the note sheet’s height limits. D8 now correctly uses owner properties, but this later passage supplies the previous implementation instructions without a supersession note. The staged amendment names the separate keyboard section, leaving this passage unidentified.

   **Failure:** Reusing the vertical-pane recipe under a 134-point bar places a pane using the document segment height, reproducing the hinge crossing described in 1123’s motivation.

   **Resolution:** Replace these expressions with the owner-coordinate recipe, or explicitly mark this passage superseded and direct readers to D8/D9. Include its sheet-height expressions in the migration.

Round-9 findings, in order:

1. **Partially resolved:** Exact’s block-shortening cycle is removed; UIKit-driven sheet resizing remains finding 1.
2. **Resolved:** The SwiftUI mapping now distinguishes extra accommodation from root policy and states the combination needed to preserve layout.
3. **Resolved:** Both identified Mac-status passages are corrected; 1127’s remaining amendment is scoped to iOS.
4. **Resolved:** LLP 1102 §3.13 explicitly marks its recommendation superseded. Finding 2 identifies another occurrence.

What I checked and found sound:

- All three targets match `snap-r10`; 1123.001 remains unchanged.
- Clearance against a shortened block removes the reported home-indicator gap. The numerical arrangement-intersection example also remains correct.
- Document `env()` and owner properties have distinct scopes. Registered inset inheritance, absent-segment resets and container-unit ancestor selection remain coherent with the [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api/#computed-value-time-behavior) and [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Arrangement intersection, axis selection, perpendicular folds, stacking, inactivity and per-frame web position sampling remain consistent.
- 1127 retains coherent selection/base/association rules, visit-distance Back dispatch, transition deduplication, cancellation, visibility publication and separate ABI obligations.
- Inspector, grouped-toolbar and split-wide-search support remains explicitly deferred.

This was a read-only specification and source review. I checked the numerical models; I did not rerun UIKit or browser probes. The supplied passing smoke supports its exercised paths.
## Loop 3, round 11

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:10:32Z to 2026-10-25T07:19:53Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 10.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `48b7763c39851a64ec96550a29fbb2faf0f2a6630b0f4e634ec755e7181a1949`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `a8e1ea3fff643ff3ab99242ff116909b626ed7bc3ea1f28b31c73aecf62b8e05`; brief `5d4189b08e1d815471b127b2337d9d1e2b29d40e7ce938371702d789902f382b`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D2/D6 — Keyboard independence still fails for a route pushed inside a sheet.**

   [D6’s source rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:422) uses controller bounds for a route and the resting detent for a sheet. A route pushed above the presented route is an owner too, but its controller follows the sheet’s size.

   **Failure:** Present a medium sheet, push route B inside its navigation controller, and let B mount an autofocus field when `B.height < T`, with T between the resting and keyboard-expanded heights. UIKit grows the sheet for the keyboard; B’s controller grows with it. Publishing B’s bounds removes the field; dismissal contracts the sheet and mounts it again. Resolving the presented route’s resting detent does not stabilize B. UIKit’s keyboard-driven sheet growth is documented by [Apple](https://developer.apple.com/videos/play/wwdc2021/10063/).

   **Resolution:** Define the keyboard-independent allocation through the entire containing hierarchy, including pushed routes inside sheets. Alternatively, refuse these descendant facts or fence their keyboard dependencies. Qualification must include a pushed route inside a medium sheet, including installation with the keyboard already visible.

2. **[major] LLP 1078 D8, connected to LLP 1123 D6/D9 — The amended recipe guards owner lengths with document counts.**

   [D8](/Users/admin/Developer/exact-owners/llp/1078-foldables-follow-the-web.rfc.md:71) still branches on `exactViewport().horizontalViewportSegments`, then consumes owner-local segment properties. [1123 D6](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:435) explicitly preserves document counts and defers a local segment record.

   **Failure:** Use this recipe in the detail column of a book-fold split. The document has two columns, but the cleared detail owner occupies only the trailing segment. The document count selects the two-pane branch, although that owner has no `-1-0` properties. Its second pane’s `left` and `width` declarations become invalid and take their initial values.

   This is the same mismatch D9 correctly demonstrates for a sheet moved onto one segment: translating lengths does not make the document’s count local.

   **Resolution:** Specify a local segment guard or a complete CSS fallback recipe for absent segments. Qualify a one-segment owner inside a two-segment document. If this requires D6’s deferred segment fact, name the consumer and make that prerequisite part of migration.

3. **[minor] Connected LLP 1038 D12 — The original phone presentation rule remains unqualified.**

   [D12’s first bullet](/Users/admin/Developer/exact-owners/llp/1038-router.rfc.md:601) still says every phone entry is a pushed screen. LLP 1127’s staged amendment changes only the later “iOS presenter is queued” sentence.

   **Failure:** Following the original bullet preserves a pushed presentation on the open Duo, contrary to 1127’s UIKit split. The later amendment does not explicitly replace the device-based classification.

   **Resolution:** Scope the bullet to the compact presentation and name UIKit’s split on iOS. Include this passage in 1127’s reconciliation.

4. **[minor] Connected LLP 1010 §6.9 — The collection-padding explanation retains an unlisted cover dependency.**

   [The feedback explanation](/Users/admin/Developer/exact-owners/llp/1010-scrolling-v1.spec.md:1068) says resolved padding includes a route’s cover when the list is the covered route box. Stage 2 removes that mechanism, but its amendment list does not identify this passage.

   **Failure:** After stage 2, a maintainer following §6.9 encounters a cover-based distinction that no longer describes the owner wrapper and extended scrollport.

   **Resolution:** Add §6.9 to stage 2’s obligations. Preserve the host-reported padding rule, while explaining how authored padding, owner clearance and scroll insets enter collection geometry under D3.

Round-10 findings:

- **1 — Partially resolved:** The resting-detent source addresses the directly presented owner; the descendant-route case remains finding 1.
- **2 — Resolved:** Motivation no longer claims comparison functions are refused.
- **3 — Resolved:** D8.2 distinguishes the Mac’s current covers from iPad owner-wrapper clearance, and stage 2 names the paragraph.
- **4 — Resolved:** The app-half passage explicitly identifies the old expressions as built behavior, and stage 4 names their migration.

What I checked and found sound:

- All three target hashes match the round-11 capsule; 1123.001 and 1127 are unchanged.
- D3’s shortened-block clearance removes the home-indicator gap while preserving authored padding.
- Document `env()` and owner properties retain distinct scopes; registered inset inheritance and explicit absent-segment resets remain coherent.
- The expanded-bar scrolling-content basis, live container-unit exception and container selection rules fit together.
- Arrangement intersection, axis selection, perpendicular folds, stacking, inactivity and web position sampling remain internally consistent.
- 1127’s selection/base/association rules, visit-distance dispatch, transition deduplication, cancellation, visibility and separate ABI obligations remain coherent.
- Inspector, toolbar groups and split-wide search remain explicitly deferred.

This was a read-only specification and source review, including prior findings, capsule evidence, SDK headers and connected-corpus searches. I checked numerical counterexamples; I did not rerun UIKit or browser probes.
## Loop 3, round 12

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:21:28Z to 2026-10-25T07:30:35Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 11.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `761e8290de0f11f31409a6c47ae4be9c2ca799020fbe2222d0726e2afce5e8b9`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `b38a1ce361388ac66865abf14fa22f08a3cb9de7f52277256a98e31cca323b0d`; brief `55dff334083948d6082388cc91b25fd266d9ecc13df508c933cf83876046baec`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D9 and LLP 1078 D8 — The fallbacks do not produce halves when segment `0,0` still exists.**

   [D9’s new example](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:473) promises that `var(--exact-viewport-segment-width-0-0, 50%)` lays panes in halves inside a one-segment owner. But D9 describes that owner as having an existing segment `0,0`, with only the later segment properties absent. A fallback does not replace an existing, valid value. [CSS Variables 1](https://www.w3.org/TR/css-variables-1/#using-variables)

   **Failure:** In a 400-point owner wholly on one side of the fold, segment `0,0` has width 400 and left 0; segment `1,0` is absent. [1078 D8’s recipe](/Users/admin/Developer/exact-owners/llp/1078-foldables-follow-the-web.rfc.md:71) therefore resolves the list’s width declaration to **400**, and the detail’s margin to **200 − 0 − 400 = −200**. With a nonshrinking list, the detail starts halfway across the list and overlaps it. The vertical version has the same defect.

   **Resolution:** Supply a complete recipe whose single-segment mode also changes the first pane’s extent, or use an owner-local segment guard. If the intention is instead to invalidate *every* segment property for a one-segment owner, state that explicitly and reconcile D9’s existing-segment rules. Qualification must include valid `0,0` properties alongside absent second-segment properties.

2. **[major] LLP 1123 D6 — Subtracting the sheet’s entire growth is incorrect for arbitrary descendant owners.**

   [The revised source rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:422) corrects a full-height pushed route, but applies the same subtraction to “any owner inside a sheet,” explicitly including a pane. Descendants need not grow by the sheet’s full height increase.

   **Failure:** Consider a 350-point-wide sheet growing from height 400 to 700 for the keyboard. An arrangement filling it and splitting vertically gives each pane heights **200 → 350**, following [1123.001 D6’s half-height rule](/Users/admin/Developer/exact-owners/llp/1123.001-arrangements.rfc.md:87). D6 subtracts the sheet’s growth, 300, from the pane’s current height, 350, and reports **50**. Its keyboard-free height is **200**. A fixed-height descendant would be incorrectly reduced despite not growing at all.

   These are numerical counterexamples to the stated correction; Apple documents the underlying keyboard-driven sheet growth. [Apple’s sheet presentation explanation](https://developer.apple.com/videos/play/wwdc2021/10063/)

   **Resolution:** Restrict this subtraction to descendants guaranteed to share the sheet’s entire height change. Other descendants need their allocation resolved through the keyboard-free containing hierarchy, or their facts must be refused. Pane fact access remains deferred in 1123.001 D4; that deferral can remain, without prescribing an incorrect source now. Add a partial-height descendant to qualification.

3. **[minor] Connected LLP 1116.011 D4 and LLP 1039 §4 — Device-based presentation assertions remain elsewhere.**

   [1116.011 D4](/Users/admin/Developer/exact-owners/llp/1116.011-what-round-13-asks.rfc.md:89) still instructs the starters that “a phone gets one column.” [1039 §4](/Users/admin/Developer/exact-owners/llp/1039-viewport-facts.rfc.md:292) says a phone held upright never sees the rail, then says the open Duo’s 951-point display does. LLP 1127 §1 explicitly identifies that wide pose as upright.

   **Failure:** Applying these instructions to an open, upright Duo selects a single-column composition or excludes its rail despite the available width and the platform split.

   **Resolution:** Replace the phone classification with the actual width predicate for authored responsive layouts, and UIKit’s presentation decision for the native split. Include 1116.011 D4 in the connected amendments.

Round-11 findings:

- **1 — Resolved for the reported full-height pushed route;** the generalized descendant correction introduces finding 2.
- **2 — Not resolved;** valid first-segment properties defeat the proposed fallback, finding 1.
- **3 — Resolved:** 1038 D12’s phone bullet now names compact presentation and UIKit’s split.
- **4 — Resolved as a staged obligation:** 1010 §6.9 is explicitly included in stage 2’s amendments.

What I checked and found sound:

- All three target files match `snap-r12`; 1123.001 is unchanged.
- D3’s shortened-block clearance, expanded-bar scrolling-content basis and live container-unit exception remain consistent.
- Document `env()`, owner properties, registered inset inheritance and explicit absent-segment resets retain their distinct scopes.
- Arrangement intersection, axis selection, stacking, inactivity and per-frame web position sampling remain coherent, apart from the D6 descendant-size correction above.
- 1127’s selection/base/association rules, visit-distance Back dispatch, transition deduplication, cancellation, visibility publication and separate ABI obligations remain coherent.
- Inspector, toolbar groups and split-wide search remain explicitly deferred.

This was a read-only review of the revisions, connected LLPs, relevant source and supplied evidence. I executed the numerical counterexamples; I did not rerun UIKit or browser probes.
## Loop 3, round 13

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:31:41Z to 2026-10-25T07:41:58Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 12.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `71602981a70c89d33bf280016c7c28efa16f220668efd4daa9b4465a26e7a8ed`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `b38a1ce361388ac66865abf14fa22f08a3cb9de7f52277256a98e31cca323b0d`; brief `299330814f8f804f513ce0f67aefc3d4c39e5764971760ef117d3b30292a2e05`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1078 D8, connected to LLP 1123 D9 — Document counts still select the wrong owner-local axis.**

   [D8’s recipe](/Users/admin/Developer/exact-owners/llp/1078-foldables-follow-the-web.rfc.md:71) branches on document counts but consumes local segment properties. R13 fixes an owner with one segment; it does not fix every smaller local grid.

   **Failure:** Take an admitted 1000×800 document with a 2×2 grid and 40-point gaps. An owner occupying its right column is 480×800 and has two local segments, one above the other. Under [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:467), its `width-0-0` is **480**, `left-0-0` is **0**, and `left-1-0` is absent. The document’s horizontal count nevertheless selects the across branch:

   - List width: **480**
   - Detail margin: **240 − 0 − 480 = −240**
   - Detail start: **240**, overlapping half the list.

   The owner has two positive segments, so R13’s suppression rule does not apply. Choosing the vertical branch first merely exposes the transposed counterexample with a local 2×1 owner.

   **Resolution:** Provide owner-local axis selection or fallback expressions that also handle an absent second column/row while `0,0` remains valid. Qualify 2×2 document grids containing 1×2 and 2×1 owners.

2. **[minor] LLP 1039 §4 — The turned Duo is incorrectly described as 951 points wide.**

   [The revised paragraph](/Users/admin/Developer/exact-owners/llp/1039-viewport-facts.rfc.md:296) promises the rail when open “upright or turned.” But [1127 §1’s measurements](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:32) give the turned window as **669×951**.

   **Failure:** Turning the open Duo makes `viewport.width >= 900` false. The example requires a rail while its code selects the bar.

   **Resolution:** Say the 951-point upright window gets the rail and the 669-point turned window gets the bar. Keep this authored width predicate distinct from UIKit’s split presentation.

3. **[minor] LLP 1116.011 §R and “As built” — Two copies retain the superseded phone rule.**

   D4 is corrected, but [the review table](/Users/admin/Developer/exact-owners/llp/1116.011-what-round-13-asks.rfc.md:22) and [the built-record quotation](/Users/admin/Developer/exact-owners/llp/1116.011-what-round-13-asks.rfc.md:142) still say “a phone gets one column,” without identifying that wording as superseded.

   **Failure:** Searching or reusing those starter instructions still excludes the wide Duo, contradicting amended D4.

   **Resolution:** Update both copies, or explicitly mark their device-based wording superseded and point to D4.

4. **[minor] Connected LLP 1069.001, indeterminate progress presentation — The container-unit explanation contradicts D12.**

   [The spinner description](/Users/admin/Developer/exact-owners/llp/1069.001-form-controls.rfc.md:755) says container units resolve to zero in an automatic height. Their basis is the eligible ancestor container, with the small viewport as fallback; the consuming element’s automatic height does not establish that basis. [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths)

   **Failure:** An automatically sized descendant of a 600-point owner resolves `100cqh` to **600**, not zero. Outside every owner, it uses the small viewport under D12.

   **Resolution:** Remove the blanket explanation or delimit the particular historical configuration it describes. Include this passage in stage 2’s reconciliation.

Round-12 findings:

- **1 — Resolved for the reported one-segment case:** all segment properties are invalid, producing 200-point halves and zero intervening margin in a 400-point owner. The broader axis mismatch is finding 1 above.
- **2 — Resolved:** subtraction is restricted to full-height routes; partial-height owners’ facts remain deferred.
- **3 — Partially resolved:** D4 and the numeric rail predicate are corrected; the orientation error and remaining copies are findings 2–3.

What I checked and found sound:

- All three targets match `snap-r13`; 1123.001 and 1127 are unchanged from round 12.
- Suppressing every segment property for a one-segment owner agrees with the chosen [CSS Env 1 §2.3 model](https://www.w3.org/TR/css-env-1/#viewport-segments). Explicit invalid resets preserve fallbacks and prevent ancestor leakage.
- D3’s clearance against shortened blocks, expanded-bar scrolling-content basis, and live container-unit exception remain consistent.
- Arrangement intersection, axis selection, stacking, inactivity and per-frame web position sampling remain coherent.
- 1127’s selection/base/association rules, visit-distance Back dispatch, cancellation, transition deduplication, visibility publication and separate ABI obligations remain coherent.
- Inspector, toolbar-group and split-wide-search support remain explicitly deferred.

This was read-only. I executed the numerical examples and checked the connected amendments and corpus references; I did not rerun UIKit or browser probes.
## Loop 3, round 14

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:43:11Z to 2026-10-25T07:52:53Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 13.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `1341ba6c55bdccc8d0db25ad6d3f8057e377270c41e2056712333463f4741197`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `a97243fd67e6f697e4d53906b74ab133dd393e227900bd2fc58133a6ab4d85ac`; brief `540683059d8547579c5f668e3b39e4fc800965caed595a3847a77af8091fa2d6`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D6 — The concrete feedback check omits host commands that change chrome indirectly.**

   [D6’s bake rule](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:432) follows reads through state writes to attributes affecting presentation or projected chrome. An action can instead call `scrollIntoView` directly, without writing state or changing an attribute.

   **Failure:** A periodic action reads `safeTop`. Above 140, it reveals an item deep in the primary list; otherwise, it reveals the first item. Under D3’s specified large-title behavior, this can alternate **168 → scroll down → 116 → scroll to top → 168**. The existing [iOS executor explicitly handles a programmatic reveal collapsing a large title](/Users/admin/Developer/exact-owners/host/apple/Sources/ExactKit/IntoView.swift:324).

   The general independence requirement forbids this cycle, but the stated bake check does not cover the path. D6’s runtime assertion that a reported change therefore cannot originate from a read is consequently unsupported.

   **Resolution:** Include command effects in the dependency fence, including fact-dependent scrolling and focus reveal that can change tracked chrome. Refuse paths whose independence cannot be established. Add a qualification case for an action that reads the fact and issues `scrollIntoView` without storing state.

2. **[minor] LLP 1123 “Amendments” / LLP 1069.001, progress presentation — The spinner response substitutes another incorrect explanation.**

   [The new amendment](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:637) says the spinner “is no size container.” The [actual emitter](/Users/admin/Developer/exact-owners/host/web/src/element.rs:396) writes:

   ```css
   container-type:size;contain-intrinsic-size:20px 20px;
   ```

   **Failure:** The claim contradicts the implementation. Moreover, absence of a size container would select the small viewport, rather than establish a zero basis. Inside an owner, a container unit used by the spinner’s pseudo-element would select the nearer spinner container, rather than automatically select the owner. These follow the nearest eligible ancestor rule. [CSS Conditional 5 §7](https://drafts.csswg.org/css-conditional-5/#container-lengths)

   **Resolution:** Remove the zero-resolution parenthetical from [1069.001’s presentation description](/Users/admin/Developer/exact-owners/llp/1069.001-form-controls.rfc.md:755) and correct the amendment. Describe the actual size containment and grid sizing without the unsupported container-unit explanation.

3. **[minor] Connected LLPs — The exclusive selected-route visibility rule remains unreconciled.**

   These passages still exclude a simultaneously live list route:

   - [LLP 1007 §4](/Users/admin/Developer/exact-owners/llp/1007-web-host-v1.spec.md:259): only the selected route is interactive.
   - [LLP 1008 §9, Router projection](/Users/admin/Developer/exact-owners/llp/1008-apple-host-v1.spec.md:737): AppKit shows only the selected route, apart from an inert modal underlay.
   - [LLP 1035.001 D1](/Users/admin/Developer/exact-owners/llp/1035.001-native-interaction-ownership.rfc.md:178): the same restriction for macOS.
   - [LLP 1075.003 §9.1](/Users/admin/Developer/exact-owners/llp/1075.003-native-platform-control-merged.plan.md:900): a split list answers selectors on the web, while native hosts hide the unselected route.

   **Failure:** With a list and detail displayed together, these rules hide or disable the list, or omit its matching nodes from `querySelectorAll`. That contradicts the built Mac split and proposed 1127 D8. The amendment lists do not identify these visibility corrections.

   **Resolution:** State the split exception in each passage and tie eligibility to the host’s published visibility. Preserve the modal and unselected-tab gates. Include the corresponding corrections in the implementation amendments.

Round-13 findings:

- **1 — Resolved by explicit scoping:** the fallback recipe is limited to one band; 2×2 documents require owner-local axis information or an arrangement, with both reduced owner grids added to qualification.
- **2 — Resolved:** upright 951-point Duo gets the rail; turned 669-point Duo does not.
- **3 — Resolved:** both copies identify the superseded device wording, and 1127 names the starter documentation changes.
- **4 — Not resolved:** scheduling an amendment is appropriate, but its new explanation is incorrect; finding 2 above.

Checked and found sound:

- All three target files match `snap-r14`; 1123.001 remains unchanged.
- Document-wide viewport facts, owner-local properties, wrapper clearance, expanded-bar scrolling-content sizing and the live container-unit exception remain consistent.
- One-segment suppression, complete invalid resets and the scoped segment fallbacks remain coherent.
- Arrangement intersection, axis selection, stacking, inactivity and per-frame web position sampling remain coherent for the stated supported cases.
- 1127’s selection/base/association rules, visit-distance Back dispatch, cancellation, transition deduplication and separate ABI obligations remain consistent.
- Inspector, toolbar-group and split-wide-search support remain explicitly deferred.

This was a source and specification review against the supplied evidence. I made no worktree changes and did not rerun simulators.
## Loop 3, round 15

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T07:53:56Z to 2026-10-25T08:01:38Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 14.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `782426d282a6e4d483bd0287bb75b70441bcf7434d355f0317684bbcd54bf097`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `f510eecf89030514dd275c69f135e88ce079b0afddbb4c92f8af7f916afc714e`; brief `3541dbcdea536bb54235fe007b82a168fad3e1dfdb298eadc54030eb0b2c55bb`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D6 — The fence’s stated scope permits feedback between sibling owners.**

   [The scope and bake rules](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:430) check a fact’s effects on its own owner and ancestor owners. They do not describe dependency closure through another owner’s published fact.

   **Failure:** An expanded split has two live sibling route owners, A and B, each with its own bar, as [1127 D7–D8 specify](/Users/admin/Developer/exact-owners/llp/1127-uikit-split-view-for-list-and-detail.rfc.md:144). Use each owner’s `safeTop` to distinguish its expanded large title from its inline title. Bind:

   - A’s heading to inline when B reports large, otherwise large.
   - B’s heading to large when A reports large, otherwise inline.

   A’s fact directly affects only B’s chrome; B’s fact directly affects only A’s. Neither destination is the queried owner or its ancestor, so neither satisfies the stated scoped refusal. Nevertheless, the dependencies form a cycle:

   `large/large → inline/large → inline/inline → large/inline → large/large`

   I executed this recurrence; it has no stable state. The general independence requirement forbids it, but the concrete fence does not establish that requirement. R15’s command coverage does not address this case.

   **Resolution:** Require the fence to follow dependencies through other owners’ fact publications and reject cycles across owners. A simpler conservative policy could refuse fact-dependent chrome changes in any owner. Qualify two simultaneously live sibling owners with mutually dependent heading levels.

2. **[minor] Connected LLP 1038 D6 — Another exclusive selected-route visibility rule remains.**

   [D6](/Users/admin/Developer/exact-owners/llp/1038-router.rfc.md:434) still instructs macOS and Linux to adopt the web rule that every route except the selected route and immediate modal underlay is hidden and inert. The amendments correct D12, but leave this normative passage untouched.

   **Failure:** With `[list, detail]` displayed together, following D6 hides and disables the list. That contradicts the built Mac and web splits and the visibility exceptions corrected elsewhere this round.

   **Resolution:** Add the built split exception to D6, preserving modal and unselected-tab gates. Include its iOS visibility obligation in 1127’s implementation amendments. The related present-tense account in 1038 §1 should identify its earlier implementation context.

Round-14 findings:

- **1 — Resolved for the reported command-only path:** scroll, reveal and focus effects are explicitly fenced, with the requested qualification case.
- **2 — Resolved:** 1069.001’s incorrect parenthetical and 1123’s incorrect amendment are removed.
- **3 — Resolved at all four reported sites:** the built split exceptions and future iOS selector amendment are present; finding 2 identifies another occurrence.

What I checked and found sound:

- All three target files match `snap-r15`; 1123.001 is unchanged from round 14.
- Document-wide `env()` and owner-local properties retain coherent scopes. Registered inset inheritance, invalid segment resets and one-segment suppression remain consistent with the chosen [CSS environment-variable model](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Container selection and small-viewport fallback follow [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Shortened-block clearance, stationary-content movement, expanded-bar scrolling-content sizing and the live container-unit exception remain consistent.
- Keyboard-independent full-height sheet-route facts and the deferral of partial-height descendant facts remain coherent.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent for the stated supported cases.
- 1127’s selection/base/association rules, visit-distance Back dispatch, cancellation, transition deduplication and separate ABI obligations remain consistent.
- Inspector, toolbar-group and split-wide-search deferrals remain explicit.

This was a read-only specification/source review with executable dependency-model checks. I did not rerun UIKit or browser probes.
## Loop 3, round 16

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:02:19Z to 2026-10-25T08:11:18Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 15.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `30b8c7109d1af3bc8008e15ba2b58684f531c4f357001641e8df783b6bce453d`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `3e7be75481a80e6eea1a326aeceacd79784b95a9dee87ff3ebe370acb5d6850e`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] Connected LLP 1080.001 D2 and LLP 1053.000.000 D2 — Two deferred-geometry dependencies remain outside the stage-1 amendments.**

   [1080.001’s frame-eligibility rule](/Users/admin/Developer/exact-owners/llp/1080.001-native-tree-and-model-agreement.rfc.md:288) excludes views held in modal deferred geometry, explicitly querying `ModalHost.holdsGeometry(id)` over `deferGeometry`’s saved operations. [1053.000.000 D2](/Users/admin/Developer/exact-owners/llp/1053.000.000-glass-groups.rfc.md:54) likewise explains glass-group sizing through a modal’s deferred geometry replay.

   **Failure scenario:** Adopt 1123 stage 1 and resize or update the presenting route while its sheet remains open. The route now receives live layout in its own allocation; the deferred-operation set and dismissal replay are deleted. The agreement specification still requires inspecting that removed state, and the glass specification still describes the removed replay. [1123’s amendments](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:634) name only 1080.001 §1 and omit the glass passage.

   **Resolution:** Include 1080.001 D2 and 1053.000.000 D2 in stage 1’s amendments. Remove the deferred-geometry eligibility exclusion and describe glass-group autoresizing through the owner’s live layout. This is a documentation integration defect; it requires no new mechanism.

Round-15 findings:

- **1 — Resolved:** D6’s scope and concrete bake refusal now cover every owner, including sibling owners, with the mutually dependent heading-level qualification.
- **2 — Resolved:** 1038 D6 preserves the live split list, retains modal and unselected-tab gates, and identifies §1’s earlier implementation context. 1127 §5 includes the iOS obligation.

What I checked and found sound:

- All three target files match `snap-r16`; 1123.001 is unchanged from round 15.
- The expanded fence covers indirect state writes and scroll, reveal and focus effects across owners, while unrelated chrome changes remain publishable.
- The changed route-visibility passages agree about built web/Mac splits and the proposed iOS visibility publication.
- Document-wide `env()`, inherited owner properties, segment resets and one-segment suppression remain coherent with [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments). Container selection and small-viewport fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Clearance, expanded-bar scrolling-content sizing, live container-unit lengths and keyboard-shortened content roots remain consistent.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent for the supported cases.
- 1127’s selection/base/association rules, visit-distance Back dispatch, cancellation, transition deduplication and separate ABI obligations remain consistent. Inspector, toolbar-group and split-wide-search deferrals remain explicit.

This was a read-only specification/source review, including SDK-header checks. I did not rerun UIKit or browser probes.
## Loop 3, round 17

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:11:46Z to 2026-10-25T08:21:30Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 16.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `2b6765f4ecfbd07e61e2766b042ce7c1e51662b45a268cd9355f7bb5d77b4efc`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `ead874c1a1846c92e10d763cef660b5243619e32d3a9deba6a4ec790718407ae`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] Connected keyboard passages still prescribe an unconditional keyboard inset.**

   Three passages remain inconsistent with 1123 D7/C25:

   - [LLP 1116 D2.3](/Users/admin/Developer/exact-owners/llp/1116-what-the-bakeoff-asks-of-exact2.rfc.md:297): the scroller’s bottom inset follows the keyboard.
   - [LLP 1116.008 D8](/Users/admin/Developer/exact-owners/llp/1116.008-what-round-10-asks.rfc.md:139): the scroller’s bottom inset grows by the keyboard.
   - [LLP 1086.000.000 §0](/Users/admin/Developer/exact-owners/llp/1086.000.000-improvements-from-the-first-diaries.rfc.md:17): under the default policy, keyboard overlap becomes that inset, despite the corrected D2 below.

   **Failure scenario:** An 800-point owner has a docked keyboard beginning at y=500. Default `resizes-content` ends its block at 500, leaving zero residual keyboard overlap. Following these passages additionally inserts the keyboard’s 300 points into the scroller, accommodating it twice. Under `resizes-visual`, a composer outside the primary scroller instead requires wrapper accommodation, another case their unconditional wording misses.

   **Resolution:** Apply C25’s policy and executor qualifications: resize first, accommodate only residual overlap, and distinguish scroller insetting from wrapper padding. Add 1116 D2.3 and 1116.008 D8 to stage 5’s amendments; correct or explicitly supersede 1086.000.000’s remaining summary.

**Round-16 finding — Resolved:** r17 explicitly includes 1080.001 D2 and 1053.000.000 D2 in stage 1, removing the deferred-geometry eligibility dependency and replacing glass-group dismissal replay with live owner layout.

Checked and found sound:

- The r17 delta matches the stated response; 1123.001 r8 and 1127 r28 are unchanged against the round-16 snapshot.
- Document-wide `env()` and owner-local properties remain distinct. Registered inset inheritance and guaranteed-invalid segment resets retain the intended fallback behavior. [CSS Variables](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid)
- Shortened-block clearance, stationary-content clearance, expanded-title scrolling-content sizing and the live container-unit exception remain consistent. Container selection and small-viewport fallback follow [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- D6’s general prohibition covers fact-dependent presentation, including automatic split eligibility; I found no separate defect warranting another finding there.
- Arrangement intersection, axis selection, stacking, inactivity and per-frame web position sampling remain coherent.
- Split selection, associations, visit-distance Back dispatch, cancellation, transition deduplication, visibility publication and separate ABI obligations remain consistent.
- Inspector, toolbar-group, split-wide-search and camera-occlusion limitations remain explicit.

This was a read-only specification/source review with numerical checks. I did not rerun the simulators or the reported implementation checks.
## Loop 3, round 18

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:22:02Z to 2026-10-25T08:29:04Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 17.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `830f17011cd3fb3cc5949c1c105c68b6f5ed7680bfdae460343260c65248ab7d`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `9a04ce2ff88cb3b977a50faf01555d58062414dda29301749a4d7240198e849c`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] LLP 1123, staged plan §5 — Hardware-keyboard bars are incorrectly grouped with undocked keyboards.**

   [Stage 5](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:578) treats every hardware-keyboard assistant bar as a rectangle requiring “no bottom padding or shortening.” That contradicts C28’s application of C25 and D7’s policy-dependent accommodation. A shortcuts bar can occupy the bottom edge; Apple distinguishes its collapsed state in the [keyboard layout guide guidance](https://developer.apple.com/documentation/uikit/adjusting-your-layout-with-keyboard-layout-guide).

   **Failure scenario:** An 800-point editing owner has a full-width, bottom-attached shortcuts bar beginning at y=750. Under default `resizes-content`, D7 requires the owner’s block to end at 750. Following stage 5’s prohibition leaves a bottom composer behind the bar. Revealing its caret alone does not supply the required composer clearance.

   **Resolution:** Classify the assistant bar by its actual occupancy. A docked bar follows C25/D7: policy resize first, then residual accommodation. A collapsed or floating bar uses rectangle-based reveal. Remove the unconditional assistant-bar exception and qualify both configurations.

**Round-17 finding — Resolved:** The stage-5 amendments now name LLP 1116 D2.3 and 1116.008 D8 and specify residual overlap and executor selection. LLP 1086.000.000 §0 now identifies the former default and today’s `resizes-content` behavior.

Checked and found sound:

- All three targets match `snap-r18`; 1123.001 r8 and 1127 r28 are unchanged from round 17.
- Document-wide `env()` remains distinct from inherited owner properties. Segment resets and suppression for one-segment owners agree with the chosen [CSS environment-variable model](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Container selection, inheritance of computed lengths, and small-viewport fallback remain consistent with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Shortened-block clearance, expanded-title scrolling-content sizing, stationary-content movement, and the live container-unit exception remain coherent.
- D6’s keyboard-independent sources and feedback fence retain the corrections for sheet descendants, indirect actions, commands, and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity, and web position sampling remain consistent for the admitted cases.
- Split selection, associations, visit-distance Back dispatch, cancellation, transition deduplication, visibility publication, and the two separate ABI changes remain coherent.
- The connected amendment audit found no further unreconciled obligation concerning viewport reparenting, deferred geometry, keyboard insets, or split visibility. Inspector, toolbar-group, split-wide-search, and camera-occlusion limitations remain explicit.

This was a read-only specification and source review against the capsule, SDK headers, and primary standards. I did not rerun simulator or browser probes.
## Loop 3, round 19

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:29:31Z to 2026-10-25T08:36:33Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 18.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `50eb23091f786de90cb11f867efecde495bdd15bc1550e81cd7951978a292b10`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `34090bdf8030002caea7ed206d8ca54f94b2a9a3865af416cd3728b0f2242eb2`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] LLP 1075.003 §9.11 — macOS modal routes are still described as painted overlays.**

   [The host paragraph](/Users/admin/Developer/exact-owners/llp/1075.003-native-platform-control-merged.plan.md:1539) says macOS never presents a modal route as a sheet and instead paints it over its underlay. That conflicts with the built `RouteSheetMac` behavior documented in [LLP 1115.002](/Users/admin/Developer/exact-owners/llp/1115.002-the-macs-structure.rfc.md:1392) and explicitly assumed by 1123 D3 and stage 3.

   **Failure scenario:** A modal route opens as an AppKit sheet with its own containing block. An implementer following §9.11 treats it as ordinary document content and omits the sheet’s owner, placement and local clearance.

   **Resolution:** Update the macOS paragraph to describe `RouteSheetMac`, or explicitly mark that description superseded by 1115.002 D7.4. The current description can retain its web and Linux scope. This correction concerns behavior already built; it need not wait for 1123’s implementation.

2. **[minor] LLP 1123, host matrix versus D3/D9 — macOS dialogs have conflicting ownership classifications.**

   [The host matrix](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:544) lists “a `dialog`’s sheet” among macOS owners. But [D3’s macOS paragraph](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:373) describes HTML dialog lifting as a separate top-layer mechanism, and [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:475) explicitly gives dialog roots the session root’s values.

   **Failure scenario:** A dialog sheet opens from a pane in a window whose top inset differs from the sheet’s. Following the matrix makes the dialog an owner with its own wrapper values; following D9 resets its values to the document’s. The same inset declaration consequently has two specified results.

   **Resolution:** Make the matrix agree with D3/D9 by removing HTML dialog sheets from the owner list. If native macOS dialogs are intentionally an exception, specify that exception consistently in the ownership, value and container-unit rules.

**Round-18 finding — Resolved:** Stage 5 now distinguishes docked assistant bars from collapsed or floating bars, applies policy resizing before residual accommodation, and qualifies both configurations.

Checked and found sound:

- The r19 delta matches the stated response; 1123.001 r8 and 1127 r28 are unchanged against `snap-r18`.
- Document `env()` values remain distinct from owner-local properties. Computed-length inheritance and registered-property invalidation remain consistent with the [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior).
- Shortened-block clearance, scroller extension, expanded-title content sizing and live container units retain the previous corrections.
- D6 retains its keyboard-independent sources and conservative fence covering indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent.
- Split selections, associations, visit-distance Back dispatch, cancellation, visibility and separate ABI changes remain consistent.
- The remaining camera, inspector, toolbar-group and split-wide-search limitations are explicit.

This was a read-only specification/source review with numerical checks. I did not rebuild or rerun simulators.
## Loop 3, round 20

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:37:06Z to 2026-10-25T08:42:20Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 19.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `a9ec3e39db3c2f9be16fbee5763a01aadda3bd066a1a2fbfac3169bf0eb47a15`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `bec5b71ca8e3becc1b6df0cf8f4400781e7c5b7469adeb2d73a311f79b11ddc4`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] LLP 1123, staged plan §3 — The macOS-dialog ownership contradiction remains in the implementation instructions.**

   [Stage 3](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:569) still says: “the pane and the route sheet become owners with `NSView.safeAreaInsets`; windows and dialogs with projection.” The latter clause continues the instruction to make these containers owners. That conflicts with D9 and the corrected host matrix, which explicitly exclude dialogs.

   **Failure scenario:** A dialog sheet opens from a pane. Following stage 3 gives it an owner wrapper and sheet-local properties; following D9 resets its properties to the document’s. With a document top inset of 28 and a dialog-sheet inset of zero, `padding-top="var(--exact-safe-area-inset-top)"` consequently has two specified results.

   **Resolution:** Remove dialogs from stage 3’s future-owner clause. State that windows become owners with route projection, while HTML dialogs retain the document’s top-layer value reset and receive no owner clearance.

Round-19 findings:

- **1 — Resolved:** [1075.003 §9.11](/Users/admin/Developer/exact-owners/llp/1075.003-native-platform-control-merged.plan.md:1539) now describes `RouteSheetMac`, its containing block, automatic content fitting and ignored detents, consistently with 1115.002 D7.4.
- **2 — Partially resolved:** The host matrix now agrees with D3/D9. The stage-3 instruction identified above remains contradictory.

What I checked and found sound:

- All three target files match `snap-r20`; 1123.001 r8 and 1127 r28 retain their round-19 hashes.
- Document-wide `env()` and owner-local custom properties retain distinct scopes. Registered computed-length inheritance and invalid-value handling agree with the [Properties and Values API](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior).
- Complete segment resets, one-segment suppression and the explicitly limited fallback recipes remain coherent with [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Shortened-block clearance, expanded-title scrolling-content sizing, stationary-content movement and live container units retain the previous corrections. Container selection and small-viewport fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- D6’s keyboard-independent sources and conservative feedback fence retain coverage of indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain consistent for the admitted cases.
- Split selection, associations, visit-distance Back dispatch, cancellation, visibility publication and separate ABI obligations remain consistent.
- Beyond the finding above, the connected amendment audit found no additional unreconciled obligation. Camera occlusion, inspector, toolbar-group and split-wide-search limitations remain explicit.

This was a read-only specification/source review against the capsule evidence and primary standards. I changed no files and did not rerun simulator or browser probes.
## Loop 3, round 21

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:42:43Z to 2026-10-25T08:49:28Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 20.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `1a1bd68caae94f8cd7cef31b37cd24ddd62a37753a9e2a80b19a578bd929474c`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `7f460cdc81d827100c3cbac1c76edb15b54197efc0278533c2a65b16daa5bafc`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] Connected LLP 1048.006, Summary and §1 D2/D3/D5 — Server-rendering guarantees still equate phones with narrow windows.**

   [The Summary](/Users/admin/Developer/exact-owners/llp/1048.006-the-reader-s-viewport.rfc.md:21) says the 700-point comparison produces one cached page for every phone and another for every desktop. D3’s “No `Critical-CH`” rationale and D5’s takeover statement likewise overstate agreement between the estimated server viewport and the browser’s actual viewport.

   **Failure scenario:** An open Duo with a 951-point browser viewport requests a page without viewport-width hints, identifying as an iPhone. The renderer’s `Hints::viewport` uses the default 390-point `pageViewport`. Consequently, `viewport.width <= 700` is true during server rendering and false at takeover: the composition changes. This contradicts those guarantees and the device-independent width model now stated in LLP 1039 and the reviewed proposals.

   **Resolution:** Describe cached classes by comparison results rather than phone/desktop categories. Qualify D3 and D5: the user-agent fallback estimates the viewport and can select the wrong branch on a wide foldable or a narrowed desktop window. Preserve the chosen fallback policy while stating its limits.

**Round-20 finding — Resolved:** [Stage 3](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:569) explicitly gives macOS dialog sheets the document’s top-layer values and no owner clearance, consistently with D3, D9 and the host matrix.

What I checked and found sound:

- All three targets match `snap-r21`; 1123.001 r8 and 1127 r28 retain their round-20 hashes.
- Document-wide `env()` remains distinct from owner-local properties. Registered computed-value inheritance and segment invalidation/fallback remain coherent with [Properties and Values](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior) and [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Clearance, scroller extension, expanded-title content sizing, stationary content and live container lengths retain their previous corrections. Container selection and small-viewport fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Keyboard policy resizing precedes residual accommodation; docked assistant bars, floating keyboards and wrapper-versus-scroller selection remain consistently distinguished.
- D6 retains the keyboard-independent geometry sources and the feedback prohibition covering indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent for the admitted cases.
- Split selection, associations, Back/history dispatch, cancellation, visibility publication and the separate ABI changes remain consistent.
- Beyond the finding above, the connected-text audit found no additional unreconciled obligation. Camera occlusion, inspector, toolbar-group and split-wide-search limitations remain explicit.

This was a read-only specification/source review using the capsule, SDK headers and primary standards. I changed no files and ran no builds or simulator/browser probes.
## Loop 3, round 22

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:50:03Z to 2026-10-25T08:56:08Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 21.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `1a1bd68caae94f8cd7cef31b37cd24ddd62a37753a9e2a80b19a578bd929474c`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `dd015e35b008995542a41621e2c8e59cda616f7a7abfb0ebadb4cbb95d44b18b`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] LLP 1048.006, D5 — The replacement takeover guarantee still exceeds its conditions.**

   [D5](/Users/admin/Developer/exact-owners/llp/1048.006-the-reader-s-viewport.rfc.md:47) guarantees matching layout whenever the request carried the width, or the estimate satisfies the same comparisons. Neither condition covers every viewport read admitted by D4.

   **Failure scenario:** A plan branches on `viewport.height <= 700`. An open Duo sends width 951 but no height. D2 and [the renderer](/Users/admin/Developer/exact-owners/host/render/src/viewport.rs:349) retain the phone’s estimated height, 844. The comparison is false during rendering and true at takeover with the actual height, 669. D5 nevertheless promises agreement because the width was supplied. Separately, matching comparisons cannot guarantee agreement for D4’s raw-value reads, such as displaying the width.

   **Resolution:** State the guarantee using D4’s equivalence rule: every compared fact must produce the same comparison results, and every fact read otherwise must have the same value. A supplied width alone suffices only for plans whose relevant reads depend exclusively on that width.

2. **[minor] Connected LLP 1075.003.000, Summary D5, §3.7 and §7.3 — AppKit sidebars and splits remain described as unbuilt containers.**

   [Summary D5](/Users/admin/Developer/exact-owners/llp/1075.003.000-hooks-on-every-host.rfc.md:36) says Exact must first own these containers. [§3.7](/Users/admin/Developer/exact-owners/llp/1075.003.000-hooks-on-every-host.rfc.md:225) explicitly says sidebars and split views “are not Exact’s on macOS today”; §7.3 still owes the document deciding their projection.

   **Failure scenario:** Shelf already displays Exact’s AppKit sidebar and split detail. A reader consulting this ownership specification is told their projection remains undecided, while LLP 1115.002 D3/D4 documents the built `ExactSidebarController` and `SplitHost`, and 1123 D3 explicitly builds ownership upon the existing detail pane. The connected documents give incompatible inventories of containers Exact owns.

   **Resolution:** Update these three passages to identify the built containers and LLP 1115.002. State any remaining hatch limitations separately; correcting the ownership description does not require adding new hatches.

**Round-21 finding — Resolved:** Cached classes are now described by width, and D3 acknowledges misclassification of wide foldables and narrowed desktop windows. Finding 1 concerns the broader replacement guarantee in D5.

What I checked and found sound:

- All three target hashes match round 21: 1123 r21, 1123.001 r8 and 1127 r28 are unchanged.
- Document-wide `env()` remains distinct from inherited owner properties. Registered computed lengths, segment resets and one-segment suppression remain coherent with [Properties and Values](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior) and [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Clearance, scroller extension, expanded-title sizing, stationary content and the live container-unit exception retain their previous corrections. Container selection and fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Keyboard policy resizing precedes residual accommodation; docked assistant bars, floating keyboards and wrapper-versus-scroller selection remain consistently distinguished.
- D6 retains the keyboard-independent sources and feedback fence covering indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent. Split selection, associations, Back/history dispatch, cancellation, visibility publication and separate ABI obligations remain consistent.
- The macOS dialog rules now agree across D3, D9, the host matrix and stage 3. Camera, inspector, toolbar-group and split-wide-search limitations remain explicit.

This was a read-only specification/source review with numerical counterexamples. I changed no files and ran no builds or simulator/browser probes.
## Loop 3, round 23

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T08:56:45Z to 2026-10-25T09:01:20Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 22.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `1a1bd68caae94f8cd7cef31b37cd24ddd62a37753a9e2a80b19a578bd929474c`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `c774d1b58a93c4a1eb2c746173eb9443ef7863b57eb90181fd7594ce67bd07c6`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] LLP 1116.003 D2.1 — Another unconditional keyboard inset remains outside the amendments.**

   [D2.1](/Users/admin/Developer/exact-owners/llp/1116.003-what-round-5-asks.rfc.md:104) says the nearest scroller is inset by the keyboard’s height. Neither that passage nor its document qualifies this by keyboard policy or residual overlap. LLP 1123’s stage-5 amendment list names equivalent passages in 1116 and 1116.008 but omits this one.

   **Failure scenario:** An 800-point editing owner has a docked keyboard beginning at y=500. Default `resizes-content` ends its block at 500, leaving zero residual overlap. Following D2.1 additionally inserts 300 points into the scroller, accommodating the keyboard twice. Under `resizes-visual`, its nearest-scroller rule also misses D7’s wrapper executor when the primary scroller does not reach the bottom.

   **Resolution:** Qualify or explicitly supersede D2.1: apply policy resizing first, accommodate only residual overlap, and distinguish primary-scroller insetting from wrapper padding. Add this passage to stage 5’s amendments. Preserve its requirement to reveal the field and following control.

**Round-22 finding 1 — Resolved:** LLP 1048.006 D5 now requires matching comparison results for compared facts and matching values for all other reads, covering both the missing-height and raw-value counterexamples.

**Round-22 finding 2 — Resolved:** LLP 1075.003.000 D5, §3.7 and §7.3 now identify the built AppKit sidebar and split, their owning LLP, and the remaining hook limitation.

What I checked and found sound:

- All three target hashes match round 22: 1123 r21, 1123.001 r8 and 1127 r28 are unchanged. I read the connected diff and searched the wider LLP corpus outside reviews and research.
- Document-wide `env()`, owner-local properties, segment resets and one-segment suppression remain coherent with [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Registered computed-length inheritance and container-unit selection/fallback remain consistent with [Properties and Values](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior) and [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Clearance, scroller extension, expanded-title content sizing, stationary content, keyboard-shortened geometry and the live container-unit exception retain their corrections.
- D6 retains keyboard-independent geometry sources and its feedback fence across indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent for the admitted cases.
- Split selection, associations, Back/history dispatch, cancellation, visibility publication and separate ABI obligations remain consistent. Camera, inspector, toolbar-group and split-wide-search limitations remain explicit.

This was a read-only specification/source review. I changed no files and ran no builds or simulator/browser probes.
## Loop 3, round 24

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T09:01:48Z to 2026-10-25T09:08:32Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 23.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `61e1945d08a21b79b1db975f790e9f944822b75ce31ce4ce1a74586961d538b1`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `f30066a1e978a0d6a5a5b8e667c154cb189288e7e866ebc2118a0a3d0598287a`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[minor] Connected LLP 1012 §1 — Target resolution still gives priority to the selected route.**

   [The operation introduction](/Users/admin/Developer/exact-owners/llp/1012-agent-api-v1.spec.md:208) defines a `testId` target as the first match on a selected route. A split’s list route remains live beside the selected detail, and [the driver](/Users/admin/Developer/exact-owners/scripts/agent.mjs:835) actually chooses the first matching node that is not `inactive`.

   **Failure scenario:** The list and selected detail both contain `testId="refresh"`, with the list first in preorder. Following LLP 1012 selects the detail’s control; the driver selects the list’s. This already matters on the web and Mac, and LLP 1127 D8/D11 extends it to iOS. Neither proposal’s amendment list covers this target-resolution passage.

   **Resolution:** Describe priority as the first matching **live node** in preorder, including a split’s live list column, followed by the existing inactive fallback. Update the matching driver comment. This documents the existing behavior without changing target resolution.

**Round-23 finding — Resolved:** [LLP 1123’s stage-5 amendment](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:639) now names LLP 1116.003 D2.1, preserves revealing the focused field and following control, and specifies policy resizing before residual accommodation.

What I checked and found sound:

- The round-24 delta is limited to that amendment and revision metadata; 1123.001 r8 and 1127 r28 match their round-23 snapshots.
- Document-wide `env()`, owner-local properties, absent-segment resets and one-segment suppression remain coherent with [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- Registered computed-length inheritance, invalid overrides and container-unit selection/fallback retain their corrections. [Properties and Values](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior), [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- Clearance, scroller extension, expanded-title content sizing and keyboard-shortened owner geometry remain consistently distinguished.
- D6 retains keyboard-independent geometry sources and the feedback fence across indirect actions, commands and sibling owners.
- Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent for the admitted cases.
- Split selection, associations, Back/history dispatch, cancellation, visibility publication and the separate ABI obligations remain consistent.
- The connected-text audit found no further unamended contradiction. Camera occlusion, inspector, toolbar-group and split-wide-search limitations remain explicit.

This was a read-only specification/source review using the capsule evidence and primary standards. I changed no files and ran no builds or device/browser probes.
## Loop 3, round 25

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T09:09:00Z to 2026-10-25T09:13:14Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** adversarial delta round with the connected LLPs; exact diffs against round 24.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `61e1945d08a21b79b1db975f790e9f944822b75ce31ce4ce1a74586961d538b1`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `22a9897ab63dbe7db2019c2c573fc3b81bdbf8fd72fb4229e19ccef93c6c9fc9`; 
- **Verdict:** READY.

Verdict: READY

No new findings.

**Round-24 finding — Resolved:** [LLP 1012 §1](/Users/admin/Developer/exact-owners/llp/1012-agent-api-v1.spec.md:208) now specifies the first matching live node in preorder, including a split’s list column, followed by the inactive fallback. The implementation branch’s `find` comment and selection expression agree.

What I checked and found sound:

- **Revision boundaries:** The three target hashes match round 24. Comparing the cumulative connected diffs confirms that round 25 adds only the LLP 1012 amendment.
- **Document versus owner geometry:** Viewport facts remain session-wide; allocations, clearance and placements remain owner-local. Logical inheritance and rendering ancestry are consistently distinguished.
- **Custom properties:** Registered computed-length inheritance, invalid overrides, absent-segment resets and one-segment suppression remain coherent with [Properties and Values](https://drafts.css-houdini.org/css-properties-values-api-1/#computed-value-time-behavior) and [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).
- **Clearance and scrolling:** Wrapper clearance, scroller extension, expanded-title content sizing, stationary-content movement and the live container-unit exception retain their corrections. Container selection and fallback agree with [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- **Keyboard ownership:** Policy resizing precedes residual accommodation. Scroller versus wrapper execution, shortened-block clearance, docked assistant bars and floating keyboards remain consistently specified.
- **Container facts:** Keyboard-independent geometry sources and the feedback fence still cover indirect state changes, commands and sibling-owner cycles.
- **Arrangements and splits:** Arrangement intersection, axis selection, stacking, inactivity and web position sampling remain coherent. Split selection, associations, Back/history dispatch, cancellation, visibility publication and the separate ABI changes remain consistent.
- **Connected LLPs:** I read the cumulative connected diff and searched the wider corpus outside reviews and research. I found no further unamended contradiction; implementation-dependent replacements remain explicitly assigned to stages. Camera occlusion, inspectors, toolbar groups and split-wide search retain explicit limits.

This was a read-only specification/source review using the capsule evidence and primary standards. I changed no files and ran no builds or device/browser probes.
## Loop 3, round 26 (after Astra's round)

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T09:36:31Z to 2026-10-25T09:41:05Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** a delta round on the folds of Astra's findings 1 and 2, asking also whether Sol agrees with Astra's three findings.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `ab4b0a02d924d766cc9ecb89dd926c2a202f35df7d8916af22bb0d2d96e8c327`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `4bdbe5bdd2f7dd0552453930af6b1b16d8a1c9c8540a105e243d96d0a8ce405b`; 
- **Verdict:** NOT READY.

Verdict: NOT READY

1. **[major] LLP 1123 D7/D9 and stage 6 — The fitted-sheet fix described in the brief is missing from the supplied revision.**

   The live RFC and `snap-r26` are identical (SHA-256 `ab4b0a02d924…`). The revision header mentions maximum-allocation segments, but [D9](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:470) still requires **every value** to come from the content root as laid out. It contains no content-sized-owner exception. [Stage 6](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:582) also lacks the promised qualification.

   **Failure scenario:** Astra’s counterexample therefore remains valid. A bottom-anchored sheet ends at y=800, has maximum height 500, and crosses a division at y=500–540. Its child uses `height: var(--exact-viewport-segment-height-0-0, 500px)`. At height 500, the first segment is 200 high; fitting shrinks the sheet to 200. It then occupies y=600–800, has only one segment, and activates the 500px fallback. The recurrence remains **500 → 200 → 500**. I reproduced that arithmetic; this is not a claim of an observed simulator failure.

   **Resolution:** Add the promised D9 rule using the maximum measurement allocation for content-sized owners’ segment properties, explicitly qualifying the general laid-out-root rule and its conformance expectation. Add the stage-6 cases for both fitted sizing and content-derived detents across the one-/two-segment boundary. Using a fixed maximum basis breaks this particular recurrence; the revision header alone does not specify that behavior.

My assessment of Astra’s findings:

- **1 — Agree; unresolved in the supplied files.** Neither D6’s feedback fence nor D12’s container exclusion covers a direct segment-property dependency.
- **2 — Agree; resolved in the specification and supplied implementation source.** D9/D12 now distinguish property reset from container lookup; runtime/browser qualification was not rerun here.
- **3 — Agree with the substantive scope gap.** Explicit deferral records the missing Landmarks behavior; it does not fulfill the original requirement. My earlier “resolved by explicit deferral” was too broad. This remains ide’s scope decision, as this round requests.

What I checked and found sound:

- **Top-layer ancestry:** [D12](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:521) now preserves query-container ancestry independently of D9’s custom-property reset. The WPT retains—and updates—the parent container after `showModal()`; container units use the corresponding size-query selection rules. [WPT](https://github.com/web-platform-tests/wpt/blob/master/css/css-conditional/container-queries/top-layer-dialog.html), [CSS Conditional 5](https://drafts.csswg.org/css-conditional-5/#container-lengths).
- **Implementation alignment:** [`env_for()`](/Users/admin/Developer/exact-owners-impl/kernel/src/arena/owners.rs:18) continues through a top-layer boundary and selects document values with the ancestor owner’s container. [`Env::container`](/Users/admin/Developer/exact-owners-impl/kernel/src/style/env.rs:339) is separate from owner properties. The [popover regression](/Users/admin/Developer/exact-owners-impl/kernel/tests/it/owner.rs:248) asserts document safe-area value 82 alongside owner-relative `10cqh = 81.7`.
- **Boundary composition:** A fixed-size owner inside a top-layer subtree supplies its own container. D12 continues to exclude content-sized owners, allowing lookup to proceed outward. I found no new contradiction introduced by the top-layer clarification.
- **Revision boundaries:** LLP 1123.001 r8 and LLP 1127 r28 match round 25. Comparing the connected deltas found only four amendment-header references changing r22 to r23.
- **Landmarks scope:** The inspector, toolbar groups and split-wide search remain explicitly deferred. Apple’s walkthrough confirms all three behaviors. [Apple’s Landmarks walkthrough](https://developer.apple.com/videos/play/wwdc2025/323/).

This was a read-only specification/source review. I changed no files and ran no builds, test binaries, browser probes or simulator checks.
## Loop 3, round 27

- **Family:** codex. **Provider/runtime:** OpenAI, `codex exec` (codex-cli 0.164.0), model `gpt-6.1-sol`, reasoning effort `xhigh` (from the run log), sandbox `read-only`, ephemeral session; 2026-10-25T09:41:38Z to 2026-10-25T09:45:04Z.
- **Date:** 2026-10-25. **Redacted:** no.
- **Method:** a delta round on the completed fold of Astra's finding 1; Astra's finding 3 is ide's scope decision.
- **Target revisions:** llp/1123-content-owners-and-local-safe-areas.rfc.md `d4b365fd503006aedf068de1935bed90d0a2dea8cb6cddd9f6db8c8cf35ca213`; llp/1123.001-arrangements.rfc.md `60ec566bd6fcb5f8e9fcb64b6b40cf0b5b870e44a8d19983c357d7f771d92f8f`; llp/1127-uikit-split-view-for-list-and-detail.rfc.md `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`; brief `d37bf5088c69f2a26632c0e81a962399c0c4a044ad5e8cb6a79e9a6285f9fa74`; 
- **Verdict:** READY.

Verdict: READY

No new findings within the requested scope of Astra’s findings 1 and 2. Finding 3 remains ide’s scope decision.

**Round-26 finding — Resolved:** [D9’s content-sized-owner exception](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:475) and [stage 6’s qualification](/Users/admin/Developer/exact-owners/llp/1123-content-owners-and-local-safe-areas.rfc.md:583) are now present.

What I checked and found sound:

- **The restored rule breaks the reported recurrence.** Re-evaluating Astra’s example numerically gives **500 → 200 → 500** under the old rule and **500 → 200 → 200** using maximum-allocation segments. Shrinking the sheet no longer invalidates the property that determined its height. The exception explicitly covers both fitted sizing and content-derived detents, while leaving safe areas and regions measured as laid out.

- **Qualification covers the failure boundary.** Stage 6 requires both sizing forms, content reading a segment property, changes between one and two segments, and an oracle using the maximum allocation. This makes the changed measurement basis testable.

- **Top-layer reset and container selection remain separate.** D9 resets owner properties; D12 preserves query-container ancestry through dialogs and popovers. This agrees with the [modal-dialog WPT](https://github.com/web-platform-tests/wpt/blob/master/css/css-conditional/container-queries/top-layer-dialog.html) and [CSS container-length selection rules](https://drafts.csswg.org/css-conditional-5/#container-lengths). The supplied implementation retains that separation, and its popover regression asserts document safe-area values alongside owner-relative container lengths.

- **The surrounding boundaries remain coherent.** D6 still refuses facts for content-sized owners, and D12 excludes those owners from query-container selection. Nested host blocks leave their parent’s intrinsic layout, so I found no additional sizing cycle through that mechanism. Segment resets and one-segment suppression remain consistent with [CSS Environment Variables](https://drafts.csswg.org/css-env-1/#viewport-segments).

- **The unchanged proposals retain their corrections.** I rechecked arrangement intersection, axis selection, stacking, visibility and web position sampling; and split selection, associations, Back/history dispatch, cancellation and visibility publication. I found no new contradiction.

- **Revision boundaries match the brief.** LLP 1123 matches `snap-r27`; LLP 1123.001 and LLP 1127 match `snap-r26`. The connected diff is unchanged from round 26.

This was a read-only specification and selected-source review with numerical evaluation. I changed no files and reran no builds, browser probes or simulator tests.

## Revision disclosure (2026-10-27)

- **The last verdicts bind to** LLP 1123 sha256 `d4b365fd503006aedf068de1935bed90d0a2dea8cb6cddd9f6db8c8cf35ca213` and LLP 1127 sha256 `0e46d8320a58f667f789a9d83bad2d8b25e52f418eaac55140522e4486dde6e8`: loop 3, round 27, READY within its scope, the folds of Astra's findings 1 and 2. Astra's round, NOT READY, binds to LLP 1123 sha256 `61e1945d08a21b79b1db975f790e9f944822b75ce31ce4ce1a74586961d538b1` (`1123-content-owners-and-local-safe-areas.astra.md`).
- **Later revisions exist and are unreviewed:** the LLP 1123 and LLP 1127 texts that the commit carrying this note lands were edited after round 27, and no round covered them or any revision after them. The close-outs of loops 1 and 2 above, which say that no later unreviewed revision exists, describe the document as it stood when each loop closed.
- **Proposed next step (the author decides):** a delta round on the texts as they land, or acceptance with them labelled unreviewed.
