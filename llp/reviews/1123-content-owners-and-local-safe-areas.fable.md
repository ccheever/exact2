# LLP 1123 review: Claude Fable family (Claude Fable 5.1)

Reviews of llp/1123-content-owners-and-local-safe-areas.rfc.md in a two-family consensus loop under LLP 0005 / LLP 0010.001. The author and reviser is Claude Opus 5.5, for ide. The other reviewer family is codex (`gpt-6.1-sol`, `xhigh`). Each round's body is verbatim.

## Loop 2, round 1

- **Family:** Claude Fable. **Provider/runtime:** Anthropic, Claude Code subagent (model `claude-fable-5-1`), with repository access and web fetch, instructed to edit nothing.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** fresh session, blind to all reviews and to the codex family's parallel review. Instructed not to read `llp/reviews/` or `/tmp/af/` beyond its prompt; that blindness is self-attested.
- **Target revision:** sha256 `488a8cc85fbb6fa2f4172913e16d174f418c828211aa314c96978f9129b1f1ae`.
- **Prompt:** `/tmp/af/c1-prompt.md`.
- **Verdict:** NOT READY.

## Review of LLP 1123 — Content owners: one viewport per session, local geometry and safe areas per native container

**Target reviewed:** `llp/1123-content-owners-and-local-safe-areas.rfc.md`
**sha256 (shasum -a 256):** `488a8cc85fbb6fa2f4172913e16d174f418c828211aa314c96978f9129b1f1ae` (matches the hash in the prompt)
**Reviewer:** Claude Fable 5.1, independent family; did not read `llp/reviews/` or any `/tmp/af/` file other than `c1-prompt.md`.

### How I verified

- Read the target in full, `CLAUDE.md`, `rules/RULES.md`, `rules/DEFERRED.md` (the native-platform-control entry), the `llp-super-refine` skill, and the cited passages of LLPs 1001, 1007, 1008 §9, 1031 D10, 1035.001 (D2, D4, D5, D9), 1039 (D1, D2, D5), 1069.011, 1075.003 (§3.3, §3.5, §3.6, §3.11), 1078 (D5, Q5), 1081, 1105.
- Read the code the document names: `ModalIOS.swift` (presentation styles, detents, `coordinateView`, `deferGeometry`/`settling`/`releaseSettled`, the viewport move at `presenter.navigation.move(to: controller) { controller.view.addSubview(presenter.viewport) }`), `ExactViewIOS.fit`, `NavigationIOS`/`NavigationBarIOS` (`wantsWholeView`, `reportCovers`, the 1249-wide dismissal guard, `scrollOriginProvisional`), `PresenterIOS` (`viewportFit`, `interactiveWidget`, `keyboardGuideTop`, `hasNativeKeyboardEditor`), `kernel/src/kernel/cover.rs` (`header_inset`), `kernel/src/style/covered.rs` (side-cover subtraction from authored widths), `kernel/tests/it/cover.rs` (the 44/106 assertions), `kernel/tables/schema.json` (the existing prop rows), `host/web/index.html` (`@property` registrations), `host/web-js/nav-chrome.js`, `QUEUE.md`, `apps/`.
- Fetched all 91 links. Apple documentation pages were checked through the docs JSON endpoint (the HTML site returns 200 for any path), reading abstracts and discussions for the load-bearing claims; HIG pages for Layout (tvOS), Designing for iPhone Duo, Tab bars, Sheets; the three videos; the W3C/WHATWG/CSSWG anchors and section numbers.

### Overall assessment

This is a strong, well-targeted proposal. The central move, one document viewport plus per-container owners with automatic local clearance credited against authored `env()`/`var()` padding, is the right shape, is grounded in the actual failure modes of today's code (the viewport move into the sheet, `wantsWholeView` counting presented navigations, covers only at rest), and the mapping tables against UIKit, SwiftUI and the HIG are unusually careful. The credit rule and the percentage fixed point are specified to the point of being implementable, and most Apple claims I checked are accurate (`viewIsAppearing` back to iOS 13; `.automatic` resolving to `formSheet` on iOS 18+; `prefersScrollingExpandsWhenScrolledToEdge` default `true`; `followsUndockedKeyboard` default `false`; `usesBottomSafeArea` default `true`; `edgesForExtendedLayout` default `.all` and not affecting the window's root; `extendedLayoutIncludesOpaqueBars` default `false`; `safeAreaInsets` zero for an uninstalled view; tvOS 60/80-point overscan margins; `clamp(none, …)` in CSS Values 4 §10.2; 32 terms/levels in §10.8; range checking in §10.12; CSS Variables 1 §3.1 for invalid-at-computed-value-time).

It is not ready yet, for three reasons that are decisions rather than details: the meaning of the `-exact-safe-area` edge list is self-contradictory; the D3 formula as written double-clears a shared obstruction for nested owners (route in a tab, the most common composition), with the "clear once" requirement living only in the implementation-obligations list; and C1/D1 disagree about what `viewport-fit="auto"` does when a primary container shows bars, leaving root-level content outside any owner without a defined inset. Each is fixable in a paragraph. The minor findings are citation and link corrections plus a few missing or mislabelled cases.

### Strengths

- The settled decisions are respected and not re-argued; the document is one proposal with no history narration.
- The §Cases catalogue is genuinely central and most rows name a decision and a lab screen; the "By" column makes the design traceable.
- The credit rule ("references the edge's inset in any position, selected or not; `credit = min(local, P)`") is a crisp, deterministic choice; the worked table (including rows 6, 9, 10, 14, 15) removes most of the ambiguity that a prose rule would leave.
- The percentage fixed point is handled honestly: side edges are always wrapper padding so the equation is closed-form; the invariant `c_edge + P_edge ≥ local_edge` is stated.
- "Two ancestries" (logical versus rendering) is the right framing and matches the top-layer and HTML modal-dialog references exactly.
- The "What it replaces" list is concrete and matches the code (`coordinateView`, the viewport move in `ModalHost.present`, `deferGeometry`/`settling`/`releaseSettled`, the 1249-wide dismissal guard, `header_inset`, `covered.rs` width subtraction), and the `kernel/tests/it/cover.rs` 44-to-106 consequence is correctly predicted (line 214 asserts 44 "nothing to hand over" today).
- The deviations from UIKit (margins, readable guide, size classes, `ViewThatFits`) are declared with "following CSS" reasons, consistent with `CLAUDE.md` and LLP 1001's practice.
- D7's attributes follow existing first-class rows (`navigationDetent`, `keyboardDismissMode`, `closedby`, `retainFocus` are in `schema.json`), and the DEFERRED restriction on native property names is honoured (the names are Exact's own).

### Concerns

#### MATERIAL

**M1. `-exact-safe-area`'s edge list means the opposite of its stated SwiftUI counterpart and its own example.**
Evidence: D4 says "`none` turns off Exact's local clearance on every edge; an edge list *keeps* clearance on exactly those edges", then "The SwiftUI counterpart is `ignoresSafeArea(.container, edges:)`" (where the edges listed are the ones *ignored*), and the example is a gallery route with `-exact-safe-area="top"` and a full-width hero image, which only makes sense if the top edge is the one turned off. C31 and the "Artwork under the chrome" recipe (`-exact-safe-area="none"` then pad with `var()`) are consistent with "listed edges are turned off"; the grammar sentence is the outlier. Two implementers would build opposite semantics.
Resolution: pick one and make the three places agree. Recommended: "an edge list turns clearance off on exactly those edges and keeps it elsewhere", which matches `ignoresSafeArea(edges:)` and the example. Add one table row or lab screen that exercises a single-edge list.

**M2. Nested owners clear a shared obstruction twice under the normative formula.**
Evidence: D3 defines `local` per owner as "UIKit's `safeAreaInsets` for the owner's view", and "Credit never crosses an owner boundary … a route none from the tab panel around it". D2 says "Owners nest (a route inside a tab, a sheet over either), and each is accounted for in its own coordinates"; C11 says "Each tab panel's content clears the tab bar". A route's controller view is positioned by UIKit inside the navigation controller inside the tab controller, so its `safeAreaInsets.bottom` contains the tab bar regardless of any kernel-side wrapper padding the tab panel owner added. Both owners therefore add the tab bar's height. The document knows this ("nested owners (a route in a tab) clear a shared obstruction once") but places it under "Implementation-phase obligations … not normative additions", so the normative rule and the obligation contradict each other. An implementer following D3 literally ships double clearance; one following the obligation must invent the subtraction rule.
Resolution: make the rule normative in D3. Either (a) define a nested owner's `local` as its own view's safe area less the part an ancestor owner's wrapper already cleared, expressed in the nested owner's coordinates (one sentence, same spirit as "intersect keyboard occupancy with each owner's allocation"), or (b) rule that a container whose entire content is another owner (a tab panel holding a navigation stack) contributes no clearance of its own and only a leaf owner that hosts authored content clears. State which, add it to the D3 table as a row (tab bar 83, route in tab: used 83 not 166), and keep the obligation as the test.

**M3. C1 and D1 disagree about `viewport-fit="auto"` when a primary container shows bars, and root-level content outside any owner has no defined inset.**
Evidence: C1 requires "With `viewport-fit="auto"` the document is fitted inside the safe area and `env()` is 0". D1 keeps `wantsWholeView` for primary navigation and tab containers, and today's `ExactViewIOS.fit` does `frame = whole ? container.bounds : container.bounds.inset(by: safe)` with `insets = cover ? safe : .zero`: under `auto` with a shown bar the document covers the whole screen and `env()` is 0. D9 then sets the root wrapper's `--exact-safe-area-inset-*` to `env()`, also 0. So in the most common configuration (a tabbed app, which the lab itself is: "every screen (the lab is tabbed)"), C1's required behaviour is not what D1 produces, the `root-auto` lab screen cannot test C1 as written, and any root-level box outside an owner (a toast, an overlay anchored to the session root) sits under the status bar with no way to read the inset: both `env()` and the root's custom properties are 0 while the root really extends under the chrome. Two implementers could reasonably build either "fit the root inside the safe area whenever `auto`" (breaking bar material reaching under the status bar) or "whole view with `env()` 0".
Resolution: state the root's three fittings explicitly in D1 and in C1 (auto with no primary bars: inset, `env()` 0; auto with primary bars: whole view, `env()` 0, all authored content is inside owners; cover: whole view, `env()` = the insets), and rule on root-level content outside owners under a whole-view root: either journal it as unsupported, or make the root wrapper's `--exact-safe-area-inset-*` carry the view's real safe area (deviating from "equals `env()`" and saying so). Give `root-auto` a non-tabbed lab fixture or say what it actually tests.

#### minor

**m1. Citation: "Exact propagates the colour only, declared as such in LLP 1001."** I found no such declaration in LLP 1001 (no "background" + root/canvas text; the deviation lists at lines 541 ff. are grid/alignment). The source that exists is LLP 1008 §9 ("what a phone paints behind the status bar … is the first root's `background`"). Cite 1008 §9 and let the "Amendments on acceptance" entry for 1001 add the declaration (it already lists one).

**m2. Link/section: CSS Backgrounds 3 "§2.11.2: the root's background propagates to the canvas".** The `#root-background` anchor is §2.11.1 "The Canvas Background and the Root Element"; §2.11.2 is the `<body>` rule. Fix the number.

**m3. Link: `developer.apple.com/documentation/uikit/uitraitoverrides` does not resolve** (docs JSON 404; the SPA shell returns 200 for any path). The symbols are `uikit/uitraitoverrides-swift.struct` (and `-c.protocol`). All other 90 links resolve, including the parenthesised Swift symbol URLs.

**m4. Video titles.** Tech Talk 111466 is "Design for iPhone Duo", 111463 is "Strike a pose with adaptive layouts on iPhone Duo", WWDC23 10281 is "Keep up with the keyboard". The document labels 111466 "sheets and the fold" and 111463 "adaptive layout for iPhone Duo"; cite the real titles so a reader can find them.

**m5. C13 attributes the "84-point bar strip on the trailing edge (or leading)" to the HIG Duo page.** That page gives no number and says only that "the system places toolbars and tab bars on the side"; the 84 comes from §Evidence (466 − 382). Attribute it there and drop "trailing".

**m6. C11's tab-bar placements are mislabelled against the HIG.** The HIG says the iOS tab bar "floats above content at the bottom of the screen" (Liquid Glass) and the iPadOS tab bar sits "near the top of the screen"; "floating (iPadOS 18+)" conflates the two. Also missing from the catalogue: the iOS 26 tab bar minimize behaviour (`TabBarMinimizeBehavior`, which LLP 1075.003 §3.5 explicitly leaves open: "minimize behavior, once Stage 2 has checked whether it moves the content area"). Add a row stating whether a minimized tab bar changes `local` (it should not; say so and test it).

**m7. Citation: "1007 §4 (modal routes on the web)".** §4 of LLP 1007 is "The ABI and the glue"; the "Modal routes (2026-09-09, Messages)" paragraph with `closedby` is in the as-built addendum under the second "## 10" heading (line 673). Point at that paragraph.

**m8. Credit scope is implicit.** D3 never says *whose* padding is credited; D9 ("A reference in an owner root's padding is credited") implies only the owner's authored root. State it in D3 and decide the common web idiom where a descendant (a bottom action bar inside the route) carries `padding-bottom: env(safe-area-inset-bottom)`: under `cover` that doubles with the route's wrapper clearance. Either accept it (author uses D4 or moves the padding to the root) and say so in the pitfalls doc, or credit descendants that touch the owner's edge. Not a defective decision, but an implementer will ask.

**m9. Missing or under-specified cases the APIs define.**
- A popover or anchored presentation hosting Exact content (LLP 1021's chooser, `MenusIOS.swift` "anchored to its opener by its `position-area`", QUEUE "Anchored boxes"): is it an owner? Its safe area includes the arrow. One sentence in D2 either way.
- A projected bottom toolbar (`toolbarItems` "once a toolbar is projected", 1075.003 §3.5): say it enters `local` through `safeAreaInsets` like a bar.
- `UISplitViewController` columns, which the HIG Duo page leans on: say they are app-owned containers under §3.6, or out of scope.
- The inner front-facing camera occlusion while the camera is active (HIG Duo "the UI moves aside"): the claim "Occlusion regions are already the safe area" rests on LLP 1078 D5's wording; mark it as beta-SDK evidence to confirm on the `strip` screen rather than as fact.

**m10. The lab.** `apps/safe-areas` does not exist; `apps/duo-lab` (LLP 1078 D8) already drives open/book/half/closed with a sheet-plus-keyboard-plus-pushed-route sequence. Under RULES ("agents add no apparatus"), name who creates the lab and whether it extends `duo-lab`, and note that C24, C35 and the `container:` fence tests are stage 6, so the lab lands incrementally.

**m11. Lowering detail (altitude: implementation-phase).** Author overrides are "attributes spelled exactly as the properties" (`--exact-safe-area-inset-bottom="0px"`). Nothing in `contract/lower/src/tags.rs` admits an attribute beginning with `--` today; the schema amendment should say these four rows are the first such names and how they lower, so the JS target and the native kernel agree on canonical output.

**m12. Clarity.** A few sentences carry two ideas and should be split: D3 "It is computed in the owner's coordinates, from a baseline that excludes clearance Exact added itself, so repeated layout with the same inputs is identical"; the scroller-executor bullet's "Exact compensates it by the credit and the opt-out so that credited authored padding is not cleared twice"; D9's "the native kernel consults the owner's host value when its walk reaches the owner's authored root, before continuing to the logical parent". None is ambiguous once read twice; they are the only places I had to.

### Consistency with exact2's rules and related LLPs

- `CLAUDE.md` "the web is the standard": honoured; every deviation is named with a CSS reason.
- `rules/DEFERRED.md`: "native property names in Contract" stays refused; `navigationSizing`/`keyboardAvoidance` are Exact's names, admitted by the owner. No React tier, no new check or script.
- LLP 1039 D5 ("no `@container`… a second consumer that cannot be written that way brings the design and its fixture"): D6's "lands only with a consumer" is exactly the door 1039 left open.
- LLP 1035.001 D4 ("A separate navigation controller owns the modal route and moves into the sheet with the session viewport") and D5 are correctly identified as the rules this changes.
- LLP 1078 D5 ("Occlusion regions (the camera, the status strip) stay what they already are, the safe-area insets") and Q5 are cited accurately.
- LLP 1075.003 §3.6 "Each route has its own content area. It lays out in its controller's safe area, not the session's viewport" is the seed this generalises; the "`.never` for every other scroll view" claim is a reasonable reading of §3.5's "Exact owns … `contentInset` and its adjustment behavior", though §3.5 does not literally say `.never`.

### Implementability of the staged plan

Stage 1 is the large one (viewport fitting, mount membership, rendering ancestry across mounts, keyboard ownership, agent coordinates) and is correctly gated on a proof with the existing workarounds kept. Stage 2 depends on M2 being settled before `one executor per edge` is coded. Stage 4 (D5) is consistent with QUEUE's `calc()` lease design, which today refuses `min()/max()/clamp()` and a percentage beside `env()` inside one `calc()`; D5 keeps those as separate arguments, so no conflict. Nothing in the plan looks infeasible; the order is sensible.

### Recommended next step

One revision addressing M1–M3 (three short normative edits plus a table row and a lab note), the citation and link fixes (m1–m7), and a sentence each for m8–m10. Then a delta round; I would expect READY on that revision.

VERDICT: NOT READY

## Loop 2, round 2 (delta round)

- **Family:** Claude Fable. **Provider/runtime:** Anthropic, Claude Code subagent (model `claude-fable-5-1`), repository access and web fetch, no edits.
- **Date:** 2026-10-08. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c2.diff`, and this family's own punch list `/tmp/af/c2-punch-fable.md`. Blind to the codex family; that blindness is self-attested.
- **Target revision:** sha256 `022869ec62a6e3552f16024852a8e8cc0ccc4489c6220838a1b3d346fa46ee25`.
- **Verdict:** READY. No MATERIAL concern; eleven minor concerns, ten of them IN-DELTA.

(Review body: the subagent's verbatim reply, recorded below.)

# Round 2 review — LLP 1123 (Claude Fable family)

**Reviewed revision:** `llp/1123-content-owners-and-local-safe-areas.rfc.md`, sha256 `022869ec62a6e3552f16024852a8e8cc0ccc4489c6220838a1b3d346fa46ee25` (verified on disk; matches the prompt). Delta: `/tmp/af/c2.diff`. Punch list: `/tmp/af/c2-punch-fable.md`. No other review material read.

## 1. Disposition verification (round-1 findings)

| Finding | Claimed | Verified | Notes |
|---|---|---|---|
| M1 edge-list meaning | FOLDED | Yes | D4 now: listed edges are turned off, as `ignoresSafeArea(edges:)`; the `gallery` example is consistent; a single-edge test is in Tests. |
| M2 nested double-clear | FOLDED | Yes | D2 "Nested owners clear once" (only an owner with authored content clears); D3 worked example 83-not-166; moved from obligations to Tests; the D3 "credit never crosses" sentence lost the now-moot tab-panel clause. Coherent with D7 "innermost first". |
| M3 C1 vs D1, root-level content | FOLDED | Yes | D1 three rules; C1 rewritten; D9 root wrapper carries the covered area's insets; LLP 1001 amendment and a test. See §3a for one residual. |
| m1 | FOLDED | Yes | Cites LLP 1008 §9 and the LLP 1001 amendment. Introduced a redundancy (§3f). |
| m2 | FOLDED | Yes | `#root-background` is §2.11.1 in the current TR (verified; 2.11.2 is the `<body>` section). |
| m3 | FOLDED | Yes | `uikit/uitraitoverrides-swift.struct` returns 200 via the docs JSON endpoint; the bare path returns 404. |
| m4 | FOLDED | Partly | 111463 ("Strike a pose with adaptive layouts on iPhone Duo") and 111466 ("Design for iPhone Duo") carry the real titles. 111462's real title is "Raise the bar with iPhone Duo"; the document writes "Raise the bar". Minor. |
| m5 | FOLDED | Yes | C13 attributes the 84 points to §Evidence and assumes no side. |
| m6 | FOLDED | Yes | C11 placements match HIG Tab bars ("floats above content at the bottom", "near the top of the screen" on iPadOS) and HIG Duo (side). C11a links `tabBarMinimizeBehavior` (26.0, verified) and LLP 1075.003 §3.5 does carry the minimize-behavior row with "Exact sets no `tabBarMinimizeBehavior`". C11b added. |
| m7 | FOLDED | Yes | LLP 1007's "**Modal routes** (2026-09-09…)" paragraph sits under its "## 10. The store" heading. (1007 has two "## 10" headings; a defect in 1007, not here.) |
| m8 | FOLDED | Yes | D3: only the owner root's padding is credited; descendant `env()`/`var()` padding adds; remedy is D4 or moving the padding. |
| m9 | FOLDED | Yes, with a wording gap | D2 says "Anchored boxes (`position-area`, menus, choosers)"; the disposition says "popovers". In the repo a popover is an HTML `popovertarget` box, which MenusIOS renders as a painted top-layer presentation, so the sentence's meaning covers it, but the word "popover" is absent from the document. Minor. |
| m10 | FOLDED | Yes | Lab note in Related. |
| m11 | FOLDED | Yes, but see §3b | The "Lowering" bullet exists; its claim is incomplete against the Contract lexer. |
| m12 | FOLDED | Yes | The sentences are split. |

## 2. New and changed links

All verified. Apple docs via `developer.apple.com/tutorials/data/documentation/....json`: `tabbarminimizebehavior` (200, iOS 26.0), `largestundimmeddetentidentifier` (200, 15.0), `uitraitoverrides-swift.struct` (200), `adjustedcontentinset` (200), `uiview/reservedregion` (200, 27.1). Tech Talks 111462, 111463, 111466 exist with the titles noted above. HIG `tab-bars` and `designing-for-iphone-duo` fetched via the design JSON endpoint (200). CSS Backgrounds 3 `#root-background` is §2.11.1. The 111466 transcript confirms C14's claims verbatim: "On the inside, in both landscape and portrait, sheets use standard horizontal bars" and the sheet vertical-bar opt-out; the HIG Duo page confirms the portrait exception and that the inner camera region "is only present when the camera is active" (C14a).

## 3. Concerns

**No MATERIAL concern, in-delta or pre-existing.** The settled decisions are not reopened. Every item below is minor.

**a. IN-DELTA, minor — D1 rule 3's rationale is implicit.** Under `auto` with a native primary container the document covers the view yet `env()` is 0, and D9 then has to carry the real insets at the root as a declared difference from `env()`. The text is self-consistent and the deviation is declared (LLP 1001 amendment, a test), but the reader is left to infer why `env()` stays 0: it is web-literal (`viewport-fit=auto` on the web reports 0), and the real deviation is that the document covers the screen so the bars can reach under the status bar. One clause saying so would prevent readers taking it for an oversight. No defective decision.

**b. IN-DELTA, minor — D9 "Lowering" understates the work.** It says `schema.json` declares the four rows and the targets write canonical CSS text. The Contract lexer (`contract/syntax/src/lexer.rs` ~L226–L262) admits a leading dash in an attribute name only for the prefixes `-webkit-`, `-apple-`, `-exact-` followed by a letter, and consumes an interior `-` only when the next byte is alphabetic; `--exact-safe-area-inset-top=` lexes as a negation today. No `.contract` file in `apps/`, `docs/` or `contract/` uses a `--` attribute. The bullet should name the lexer rule extension (and the Lean syntax if attribute names are constrained there) as stage-2 work. The implementer would discover this immediately; no decision changes, so it is implementation-phase material, but the document claims completeness here and is one clause short.

**c. IN-DELTA, minor — C1's lab screen vs the tabbed lab.** C1 now reads `root-auto (no routes)`, but C11 says "every screen (the lab is tabbed)", so a tabbed lab always has a native primary container and cannot exercise rule 2. `root-auto` must be a separate launch or fixture; say so.

**d. IN-DELTA, minor — the Tests bullet "D9: the root's values equal `env()`"** is now true only under `cover` (and trivially under `auto` without a container). The separate bullet covers the rule-3 case; the first should say "under `cover`".

**e. IN-DELTA, minor — C33a's "a fixed control above it".** The D3 rule is exact ("no in-flow box ... between the scroller and that edge"), but "fixed control" invites a `position: fixed` reading, under which the scroller would still reach the edge and the control would sit under the bar. "An in-flow control above it" removes the ambiguity.

**f. IN-DELTA, minor — "Backgrounds extend" now says twice** that LLP 1008 §9 paints the first root's background behind the status bar, in one three-semicolon sentence (the m1 fold). Drop the second occurrence.

**g. IN-DELTA, minor — D2's owner list.** The new sixth bullet (a split-view column, "as an app-owned container") is a sub-case of the fourth bullet (app-owned containers "including a pane an arrangement gives Exact"). Fold it into the fourth as a second example.

**h. IN-DELTA, minor — C24a's "C17–C23 apply to modal presentations only"** excludes C24, which is also a modal presentation. Say "C17–C24".

**i. IN-DELTA, minor — the LLP 1078 D5 quotation.** The document quotes it as "stay the safe-area insets"; the sentence at `llp/1078-foldables-follow-the-web.rfc.md:55` reads "stay what they already are, the safe-area insets; nothing reads them." Either quote it exactly or drop the quotation marks.

**j. IN-DELTA, minor — 111462's title** shortened to "Raise the bar" while the other two carry full titles (see m4).

**k. PRE-EXISTING, minor — D7 keyboard with the scroller taking the bottom edge but the editor outside it.** I checked the arithmetic: the wrapper pads the residual overlap (keyboard minus the excluded 34-point baseline) while the scroller keeps its 34-point content inset, which sums exactly to the keyboard height, so there is no double count. No change needed; noting that it was checked.

## 4. Diff defects

None found beyond the minor items above. The diff introduces no contradiction with the settled decisions, D3's formula, the one-executor rule, or the test list; the moved obligations are present in Tests; the removed Linux "except the root's" clause is harmless since `env()` is 0 there.

## 5. Altitude rule

Every concern above is detail that identifies no defective decision; each is one clause to fix and none blocks acceptance. No pre-existing MATERIAL concern exists, so this family meets the delta-round convergence condition.

**Reviewed sha256:** `022869ec62a6e3552f16024852a8e8cc0ccc4489c6220838a1b3d346fa46ee25`

VERDICT: READY

## Loop 2, round 3 (delta round)

- **Family:** Claude Fable. **Provider/runtime:** Anthropic, Claude Code subagent (model `claude-fable-5-1`), repository access and web fetch, no edits.
- **Date:** 2026-10-09. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c3.diff`, and this family's own punch list `/tmp/af/c3-punch-fable.md`. Blind to the codex family; repository searches excluded `llp/reviews/`. Blindness is self-attested.
- **Target revision:** sha256 `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4`.
- **Verdict:** READY. No MATERIAL concern; four IN-DELTA minor wording items and four PRE-EXISTING detail items.

(Review body: the subagent's verbatim reply, recorded below.)

# Round 3 review — LLP 1123 (Claude Fable family)

**Reviewed:** `llp/1123-content-owners-and-local-safe-areas.rfc.md`, sha256 `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4` (recomputed on disk; matches the prompt). Read in full, with `/tmp/af/c3.diff` and `/tmp/af/c3-punch-fable.md`. Blindness honored: no other `/tmp/af` file and nothing under `llp/reviews/` was opened; repository searches excluded `llp/reviews/**` and no review content was seen. Repository sources consulted to check the diff's factual claims: `contract/syntax/src/lexer.rs`, `llp/1007-web-host-v1.spec.md`, `llp/1078-foldables-follow-the-web.rfc.md`, `kernel/tables/schema.json`, `apps/`.

## 1. Disposition verification (round-2 punch list)

| Item | Disposition claimed | Verified | Notes |
|---|---|---|---|
| a | FOLDED | Yes | D1 third rule now reads "`env()` is 0, as `auto` reports on the web; covering the view is the native deviation." Accurate: under `viewport-fit=auto` a browser lays the viewport inside the safe area and `env()` resolves to 0. |
| b | FOLDED | Yes | D9 "Lowering" now states the lexer admits only `-webkit-`, `-apple-`, `-exact-` and that stage 2 extends the attribute-name rule (and the Lean syntax if it constrains names). Checked against `contract/syntax/src/lexer.rs:226-245`: the `vendor` check is exactly `["-webkit-", "-apple-", "-exact-"]`, matched only as an attribute name followed by `=`; a `--exact-…` name does not match it. The claim is correct. |
| c | FOLDED | Yes | C1's lab column: "`root-auto` (a separate fixture without routes, since the lab is tabbed)". |
| d | FOLDED | Yes | Tests, D9 bullet: "equal `env()` under `cover`, and are 0 when the root is fitted"; the `auto`-with-container case is covered by the preceding "Root fitting (D1)" bullet. Consistent with D9's three-way statement. |
| e | FOLDED | Yes | C33a and the D3 scroller rule now cover an in-flow control above, a pane beside, and a composer below. One in-delta wording defect in the new rule text; see §2.2. |
| f | FOLDED | Yes | The duplicate "LLP 1008 §9 already paints…" clause is gone; the first sentence of "Backgrounds extend" still carries the reference once. |
| g | FOLDED | Yes | The split-view column is now an example inside the app-owned-container bullet. One punctuation defect introduced; see §2.1. |
| h | FOLDED | Yes | C24a now reads "C17–C24". |
| i | FOLDED | Partially complete | The "Amendments" bullet now quotes LLP 1078 D5 exactly ("stay what they already are, the safe-area insets"; verified at `llp/1078-foldables-follow-the-web.rfc.md:55`). The same inexact quotation remains in the API table, `UIView.ReservedRegion` row (line 194): "stay the safe-area insets". See §2.3. |
| j | FOLDED | Yes | Fetched `https://developer.apple.com/videos/play/tech-talks/111462/`: the title is "Raise the bar with iPhone Duo". |
| m9 | FOLDED | Yes | "Popovers and other anchored boxes (`popovertarget`, `position-area`, menus, choosers)". `popovertarget` is a schema row (`kernel/tables/schema.json:402`). |

All eleven dispositions are correctly FOLDED. Item i is folded where the finding pointed but leaves a second copy of the inexact quotation; recorded below as a minor in-delta inconsistency, not a disposition error.

## 2. Diff review — defects the delta introduces

All of these are **IN-DELTA, minor**. None identifies a defective decision; each is a wording or punctuation correction.

**2.1 D2 owner list ends with a semicolon (line 253).** Folding the split-view bullet into the app-owned bullet removed the list's final period: "…puts around a route or sheet;" is now the last item. Should end with a period.

**2.2 The new "reaches an edge" sentence is ambiguous when read alone (line 330).** "A scroller beside a pane, or below or above another box, does not reach the edge." Per-edge, a scroller *above* a composer still reaches the *top* edge; "the edge" needs to be "the edge on that side" (or the sentence split per edge). The previous sentence's "that edge … along the edge's whole length" is precise; this summary sentence is the only loose one. Also, the clause "so no other box of the owner's content touches that edge beside or before it" is stated as a consequence but is not strictly implied (an absolutely positioned box can touch the edge without preventing the scroller from spanning it); the governing condition is the border-box-spans-the-full-length test, which is fine. The requirement in C33a is unambiguous, so this is not MATERIAL.

**2.3 Inexact LLP 1078 quotation survives in the API table (line 194).** The amendments bullet (line 541) was corrected to the exact wording; the `UIView.ReservedRegion` row still says `"stay the safe-area insets"`. Make both quote `llp/1078-foldables-follow-the-web.rfc.md:55` verbatim, or drop the quotation marks in the table row.

**2.4 D1 now names a "native deviation" that the LLP 1001 amendment does not itemize (lines 239, 536).** The new phrase "covering the view is the native deviation" is correct, and the Related line already says LLP 1001 gets a deviation declared. The LLP 1001 amendments bullet lists the automatic-clearance deviation, the D3 formula, the opt-out, colour-only propagation, the `var()` term and the D9 root-property difference, but not the D1 third rule's "covers the view while `env()` is 0". A five-word addition to that bullet closes it. Detail-only.

**2.5 C14 wording versus the cited Tech Talks — verified, no defect.** Fetched both talks. 111462 ("Raise the bar with iPhone Duo"): on the inner display sheets are centred by default with horizontal items; with `preferredPlacement`, left-placed sheets have no vertical bar and right-placed ones receive one; on the outer display a sheet with a toolbar shows it vertically. 111466 ("Design for iPhone Duo"): on the outer display sheet controls move to the side; on the inside, in both orientations, sheets use horizontal bars; the vertical bar can be disabled. The document's "a sheet placed at the side of the inner screen, and sheets on the cover screen, *can* carry a side bar" is accurately hedged (only the right-placed one does), and the normative sentence ("nothing in Exact assumes which edge carries the bars") does not depend on the particulars. Sound.

**2.6 Modality row reference (line 225) — verified.** LLP 1007 has two headings numbered "## 10." (lines 636 and 654, a numbering defect in LLP 1007, not in this document); "**Modal routes** (2026-09-09, Messages)" is a dated addendum paragraph at line 673 under the second §10. "LLP 1007 §10 addendum, "Modal routes"" locates it correctly and matches the Related line.

**2.7 Other diff hunks** (C1 lab column, C24a range, "Backgrounds extend" dedupe, D9 Lowering, Tests D9 bullet, Amendments 1078 bullet): sound, no new defects.

## 3. Whole-document concerns outside the delta

All **PRE-EXISTING, minor**; none reopens a settled decision or identifies a defective one.

- **Evidence table does not say which `viewport-fit` the unfolded bottom inset 34 was measured under (line 45).** The probe ran under both values and the text only states that the *side* insets were 0 under either. Since D1 now explicitly asserts "`env()` is 0, as `auto` reports on the web", the table should label the 34 as the `cover` value so the document's own evidence visibly agrees with its claim. Detail-only; the CSS behavior is well established.
- **Lab bullet is stale relative to the worktree (line 21).** "The implementer creates it at the owner's request, with stage 1," but an untracked `apps/safe-areas/` scaffold (app.json, app.contract, three platform crates) already exists in the worktree. Either the sentence or the scaffold is ahead of the other; not a document defect in itself, noted for the author.
- **"Owner's content box" in the scroller rule (line 330)** is inherited wording; it means the authored root's content box (inside the root's padding), which the credit rule makes consistent, but a reader could take "owner" as the wrapper. Detail-only.
- **The new full-width condition has a side effect worth a sentence (line 330):** a primary scroller that is itself narrowed (`max-width` with auto margins, D8) no longer spans the edge and so falls back to wrapper padding, losing scroll-under-the-bar. The idiom is to put the readable column inside the scroller (as C34 `columns` presumably does). Determinate and defensible; not a defective decision.

I checked the D3 table arithmetic (rows 6, 9, 10, 14, 15 and the `c = 17.5 / W = 82.5 / P = 16.5` fixed point) and the D9 "first and third rules" cross-reference against D1's rule order; all consistent.

## 4. Links

No new URLs are introduced by the delta (the 111462 link existed; only its title text changed). Verified by fetch anyway: tech-talks/111462 and tech-talks/111466 resolve to the titled videos quoted above. No Apple documentation pages were added, so the docs JSON endpoint was not needed.

## 5. Summary

Every round-2 disposition for this family is correctly folded. The delta introduces four minor wording/punctuation items (§2.1–2.4) and no MATERIAL contradiction; the one factual claim it adds about the codebase (the lexer's three prefixes) is accurate, and its C14 evidence claim matches the cited talks. Nothing pre-existing rises above detail under the altitude rule. Under the delta-round standard (punch list correctly dispositioned, diff sound, modulo detail), the document converges.

Reviewed sha256: `3078c52b0c224593fb5cab30ac891362b5fb4722479d893959d713ba1147efd4`

VERDICT: READY

## Loop 2, round 4 (delta round)

- **Family:** Claude Fable. **Provider/runtime:** Anthropic, Claude Code subagent (model `claude-fable-5-1`), repository access and web fetch, no edits.
- **Date:** 2026-10-09. **Redacted:** no.
- **Method:** delta round with the full target, the diff `/tmp/af/c4.diff`, and this family's own punch list `/tmp/af/c4-punch-fable.md`, which also describes an author-initiated factual correction (a new Safari measurement). Repository searches excluded `llp/reviews/`. Blindness is self-attested.
- **Target revision:** sha256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b`.
- **Verdict:** READY. No MATERIAL concern; two IN-DELTA minor wording items and one PRE-EXISTING minor item.

(Review body: the subagent's verbatim reply, recorded below.)

# LLP 1123 round 4 review (Claude Fable family)

**Reviewed:** `llp/1123-content-owners-and-local-safe-areas.rfc.md`, sha256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b` (verified on disk with `shasum -a 256`; 550 lines, read in full). Delta: `/tmp/af/c4.diff`. Punch list: `/tmp/af/c4-punch-fable.md`.

**Blindness:** I read only the prompt, the diff and this family's punch list under `/tmp/af`, plus the target, the repository's `.claude/skills/llp-super-refine/SKILL.md`, `llp/1001-kernel-v1.spec.md`, `llp/1008-apple-host-v1.spec.md`, `llp/1078-foldables-follow-the-web.rfc.md` and `apps/safe-areas/`. Every repository search excluded `llp/reviews/**`. I saw no review content.

## 1. Disposition verification

| Item | Disposition claimed | Verified |
|---|---|---|
| 2.1 | FOLDED at D2 | Correct. The last owner bullet (line 253) now ends with a period. |
| 2.2 | FOLDED at D3, "Primary scroller" | Correct. The summary is now per edge (beside a pane: neither edge; below a box: not top; above a box: not bottom). The unimplied "so no other box…" clause is gone, replaced by a definition: "the content box is the one inside the authored root's padding", which removes the wrapper reading. I checked the definition's consequences: with an uncredited root `padding-top` of 16 and `local` 106, the scroller's box begins at 16, its content inset is 106, and the content rests at 122, which equals the formula's `clearance + authored`; the definition is coherent with D3. |
| 2.3 | FOLDED at the API table | Correct. The row now quotes "stay what they already are, the safe-area insets", which is verbatim at `llp/1078-foldables-follow-the-web.rfc.md:55`. |
| 2.4 | FOLDED at "Amendments", LLP 1001 bullet | Correct. The bullet itemizes "the third root-fitting rule, which covers the view while `env()` is 0 (D1)". |
| §3 a | FOLDED at "Evidence" | Correct. The paragraph and the table header now state the readings were taken under both `cover` and `auto` and were identical. |
| §3 b | FOLDED at the Lab bullet | Partially. The implementer-creates clause is gone and the tense is present. But see minor finding M1 below: "one screen per case" is still ahead of the worktree, and the sentence narrates history. |
| §3 c | FOLDED with 2.2 | Correct (same sentence). |
| §3 d | FOLDED at D3 | Correct. One sentence: a `max-width`-narrowed primary scroller spans no edge, so the readable column goes inside the scroller (C34). |

## 2. The author-initiated factual correction

Treating the re-measurement as reported evidence, the corrected text is accurate and internally consistent, and changes no decision:

- "Evidence" now says Safari lets the page reach the bottom under both values and reports 34 as `env(safe-area-inset-bottom)`, so "Safari's `auto` does not fit the bottom edge. Exact's `auto` does (D1)." This is consistent with the Summary's rule ("the host lays out around its own chrome; `env()` reports what is left"): the strip is Safari's own chrome and is laid out around; the home indicator is not, and is reported.
- D1's second rule now grounds `env()` 0 in Exact's own fitting (all four edges, so no obstruction overlaps) and cites LLP 1008 §9, which does define the layout viewport as the safe area under `auto`. It notes Safari's narrower `auto` and points to C38 (the web host reports the browser's `env()`). C1, C13, C38, D3 "Per host: Web", D9's web sentence (`--exact-safe-area-inset-top: env(safe-area-inset-top)` on the root element) and the Tests bullet ("0 when the root is fitted") all remain consistent with this.
- D1's third rule no longer appeals to the web for `env()` 0; it cites the second rule and names the deviation as declared in LLP 1001. The D1/D9 decisions themselves (fitting rules, root custom properties diverging from `env()` under the third rule) are unchanged.
- The two amendment corrections quote their targets verbatim. LLP 1001 §2 (`llp/1001-kernel-v1.spec.md:780-781`): "Zero until the host says otherwise, as a browser reports the insets for a page without `viewport-fit=cover` (LLP 1008 §9)." LLP 1008 §9 (`llp/1008-apple-host-v1.spec.md:975-977`): "The layout viewport is the safe area (above) — a browser's rule for a page without `viewport-fit=cover`". Both sentences do equate zero insets or a fitted viewport with what a browser does without `cover`, which the measurement contradicts for the bottom edge, so the corrections are warranted and correctly targeted.

## 3. Diff review

The diff touches nine hunks: the Revised date, the Lab bullet, the Evidence paragraph and table header, the `ReservedRegion` quotation, D1's second and third rules, D2's terminal punctuation, the D3 scroller bullet, and two amendment bullets. I found no contradiction introduced between the new text and any unchanged section (checked against Summary, C1, C13, C38, D3 per-host, D9, Tests). No settled decision is reopened. The diff adds no new URLs: the only URLs on added lines are the two `UIView.ReservedRegion` links, which exist on the corresponding removed lines. I nevertheless checked them through Apple's docs JSON endpoint: both return 200 (`UIView.ReservedRegion`, `frame`, introduced iOS 27.1).

## 4. Concerns

**No MATERIAL concern, in-delta or pre-existing.** Minor items, none identifying a defective decision:

- **M1 (IN-DELTA, minor).** Lab bullet: "one screen per case in §"Cases"… It was created at the owner's request and gains screens with each stage." The worktree's `apps/safe-areas/app.contract` (its `caseOf` mapping, lines 87–91) has screens `root-cover`, `root-auto`, `strip`, `carousel`, `edge-art`, `columns`, `segments`, `nav-large`, `nav-inline`, `nav-search`, `composer`, `sheet-page`, `sheet-nested`, `sheet-detents`, `fullscreen`, `keyboard`. Screens the Cases tables name that do not yet exist: `status-bar` (C3), `sheet-form` (C19), `keyboard-resize` (C29), `opt-out` (C31), `padding` (C32), plus `sheet-fitted` (C24, deferred to stage 6 by design). "Gains screens with each stage" covers the gap, but "one screen per case" reads as a present fact; "has a screen per case as its stages land" would be exact. Also, "It was created at the owner's request" is narration of history, which the owner's requirements exclude. Wording only.
- **M2 (IN-DELTA, minor).** The Evidence table header singles out the bottom column as "(`cover` and `auto`)" while the preamble says every reading was identical under both. Harmless, since the preamble governs, but the asymmetry invites the question why only the bottom is labelled. Wording only.
- **M3 (PRE-EXISTING, minor).** D9's root sentence, "0 when it is fitted inside the safe area", is stated for the native second rule; on the web the root element carries `env()` directly, which under Safari's `auto` is 34 at the bottom. The web sentence later in the same bullet makes this explicit, so the text is not wrong, but a reader applying the native phrasing to the web could pause. Altitude rule: no decision is defective.

## 5. Whole-document pass

Read in full. Beyond M1–M3 I found nothing ambiguous or inconsistent; the Cases tables, the API mapping tables, D1–D9, the host matrix, the staged plan, the tests and the amendments agree with one another and with the corrected evidence. The amendments' three quotations (1001 §2, 1008 §9, 1078 D5) are each verbatim in the repository.

**Links:** no new links in the delta; the two Apple links on added lines verified (200) via the docs JSON endpoint.

**Reviewed sha256:** `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b`

VERDICT: READY

## Close-out (loop 2)

- **Converged:** both families are READY on revision sha256 `3f9e2221a36f0af5a50ab8110259ce349ef2440455e264497e5927000a41943b` (round 4); the codex family's record and the full round history are in `1123-content-owners-and-local-safe-areas.codex.md`. No later unreviewed revision exists.
- **Carried to implementation (non-blocking, detail only):** this family's round-4 M1–M3.
