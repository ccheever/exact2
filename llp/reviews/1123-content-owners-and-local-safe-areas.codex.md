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
