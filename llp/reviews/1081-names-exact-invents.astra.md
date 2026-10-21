# Review: LLP 1081, names Exact invents are spelled `-exact-` (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one fresh session in the main checkout (219,817 tokens).
- **Date:** 2026-10-03.
- **Reviewed:** `llp/1081-names-exact-invents.rfc.md` Draft r1, untracked over `0b871f54e` (blob `09aa5a88`).
- **Redacted:** no. The document and the checkout contain no secrets the brief exposed.
- **Method:** requested by Charlie. Mutually blind to the other family (Grok 4.7), which got the identical brief (sha256 `5c57251c52c1bc5d4deb07c8ec8c0f9aa452e9b130bdd981f26bb290f453ce29`). A static source audit in a read-only sandbox: the reviewer ran nothing and changed nothing. It could not fetch WebKit and said so. The RFC's author (Claude) is not a reviewer.
- **Transcription:** the final message after the last `codex` line, unedited (codex prints it twice; one copy is kept).

---

**Overall verdict: NOT READY.** The naming policy fits Exact, but r1’s inventory and implementation plan are incomplete. Following the plan literally would leave inconsistent motion spellings and break dynamic system-fill colors on the JS target.

This is a static, read-only audit of the checkout at `0b871f54e`. I made no edits and ran no builds, tests, interpreters, remote commands, skills, or sub-agents.

The proposal has several strengths:

- Using `-exact-` for invented CSS vocabulary supports the repository’s web-first rule and avoids occupying future CSS names. Keeping actual CSS names follows [RULES.md:51](/Users/ccheever/projects/exact2/rules/RULES.md:51).
- D3 correctly distinguishes adopting CSS with documented limitations from inventing vocabulary. A supported subset of `box-shadow` should remain `box-shadow`.
- Removing old spellings with useful diagnostics fits “delete; don’t deprecate.” Separating the symbol-color behavior change from the mechanical rename is also correct.
- D7’s lexer approach fits the existing grammar: recognize the prefix before an attribute’s `=`, preserving expression negation.
- **Every bit number listed in §2 matches `schema.json`.** I found no standard CSS property mistakenly included in the property rename table. Keeping `wrap-flow`, `shape-outside`, `animation-range`, `timeline-scope`, and the other listed CSS names is appropriate.

The numbered concerns are:

1. **High — D1 does not establish a consistent test for either CSS names or browser names.**

   [D1](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:75) tests CSS names by presence in any CSSWG draft or unprefixed browser shipping. Unlike its browser-name test, it does not explicitly require the **same grammatical position and meaning**. A word’s occurrence elsewhere in CSS cannot authorize Exact to use it as a different property, keyword, or function.

   “Any maturity” is reasonable for a currently defined draft feature. “Any draft” needs clarification about superseded definitions, abandoned names, and non-normative mentions. Likewise, “ships unprefixed” needs a browser version and exposure context; implementation behind a private setting is different from ordinary author CSS.

   The browser-name rule also conflicts with the existing implementation. D1 requires native emission on the owning browser, but system colors are unconditionally converted to `light-dark()` pairs by [css.rs:502](/Users/ccheever/projects/exact2/host/web/src/css.rs:502) and [style.rs:473](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:473). The rename-only plan does not change that.

   Q3 exposes the same inconsistency: it says `-apple-visual-effect` is unavailable to Safari web content yet qualifies under D1. That contradicts §1’s claim that the prefix means “Safari ships this.”

   **Resolution:** classify an exact spelling **in its grammar and semantic role**, supported by a pinned specification or engine revision. Separate browser ownership from author exposure and native execution. Decide explicitly whether privileged WebKit vocabulary qualifies; then make D1, Q3, D3’s permitted deviations, and the existing color fallback policy agree.

2. **High — The inventory misses an invented keyword and leaves an extension function unclassified.**

   `text-decoration-line` accepts **`underline-line-through`**, declared in [schema.json:1462](/Users/ccheever/projects/exact2/kernel/tables/schema.json:1462). This is an authored spelling: the generator feeds ordinary enum rows through their exact name parser at [build.rs:1306](/Users/ccheever/projects/exact2/kernel/build.rs:1306). The web emits the enum string unchanged at [css.rs:514](/Users/ccheever/projects/exact2/host/web/src/css.rs:514). CSS expresses this combination as `underline line-through`, not the hyphenated token.

   **`spring()`** also needs a decision. [LLP 1002 D2](/Users/ccheever/projects/exact2/llp/1002-motion-v1.rfc.md:84) explicitly declares it an extension CSS lacks. Exact accepts zero arguments or `(stiffness, damping, mass)` at [parse.rs:159](/Users/ccheever/projects/exact2/motion/src/parse.rs:159). Its own documentation says it differs from WebKit’s proposed function by obtaining initial velocity from motion, [spring.rs:16](/Users/ccheever/projects/exact2/motion/src/spring.rs:16). R1 neither inventories it nor supplies evidence that this grammar qualifies under D1.

   I found no additional invented environment-variable names in the admitted `env()` parser: it recognizes safe-area insets and viewport segments, [env.rs:398](/Users/ccheever/projects/exact2/kernel/src/style/env.rs:398). The inspected media facts likewise use existing CSS feature names, [viewport.rs:78](/Users/ccheever/projects/exact2/runner/src/viewport.rs:78), [viewport.rs:159](/Users/ccheever/projects/exact2/runner/src/viewport.rs:159). Nevertheless, D1’s definition does not explicitly cover media features.

   **Resolution:** add both omissions to the inventory. Prefer adopting CSS’s two-token decoration syntax; record its existing web-rendering correction separately from the pixel-preserving rename. For `spring()`, either provide evidence satisfying the tightened D1 or adopt `-exact-spring()`, including its parsers and serializers. Explicitly cover `env()` identifiers and media-feature names.

3. **High — Renaming `tint-color` requires changing the motion vocabulary, not just Contract’s attribute table.**

   [§4 stage 1](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:139) omits `motion/src/property.rs`. That file independently names `TintColor` as `"tint-color"` and resolves motion property names through that spelling, [property.rs:167](/Users/ccheever/projects/exact2/motion/src/property.rs:167).

   Transition parsing uses that lookup through [transition.rs:50](/Users/ccheever/projects/exact2/motion/src/transition.rs:50). Keyframe lowering also calls it directly, bypassing `tags::attr`, [svg.rs:388](/Users/ccheever/projects/exact2/contract/lower/src/svg.rs:388).

   Consequently, changing `tags.rs` alone would accept `-exact-tint-color=…` on an element while rejecting it in keyframes and accepting the obsolete name inside `transition="tint-color …"`.

   There are two related diagnostic boundaries:

   - Existing `dragTimeline` guidance points to `"drag-timeline"`; it must point directly to the new accepted spelling, [tags.rs:1123](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:1123).
   - `Property::from_name` also accepts the internal `"--exact-tint"` spelling. D5 calls that namespace host-only, but the transition parser currently exposes it to authors.

   **Resolution:** enumerate author-facing motion names separately from emitted browser names. Update motion parsing and keyframe validation, preserve the internal `--exact-tint` mapping, and provide rename diagnostics in each authoring context. Explicitly decide how authored access to the reserved internal spelling is refused.

4. **High — The JS target’s dynamic color mapping will miss `-exact-system-fill`.**

   The replacement table is generated from `SYSTEM_COLORS`, but its entry condition is hard-coded:

   `typeof v === "string" && /-apple-system-/i.test(v)`

   See [style.rs:473](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:473). Renaming the kernel table entry would update the replacement pairs while leaving this condition false for a value containing only `-exact-system-fill`. The browser would receive an unsupported color.

   The keyword migration also needs both directions of serialization. `CornerShape::css()` still spells the old keyword at [corner.rs:84](/Users/ccheever/projects/exact2/kernel/src/corner.rs:84); changing only the parser and Contract’s error text is insufficient.

   Finally, replacing the JS corner mapper’s recognized token does not itself implement D2’s promised diagnostic for an old computed keyword. The current mapper simply transforms strings, [style.rs:585](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:585).

   **Resolution:** name the dynamic color guard, corner serializer, and runtime refusal paths explicitly. Test literal and computed values, mixed case, colors embedded in supported composite values, and old-keyword rejection on both web targets.

5. **High — Stage 2 starts from an incorrect description of the web renderer and underspecifies runtime behavior.**

   [Stage 2](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:148) says the web already draws symbols using inherited `color`. Both web implementations actually draw symbol masks using `--exact-tint`, registered with `inherits:false` and initial black: [glue.js:357](/Users/ccheever/projects/exact2/host/web/glue.js:357), [symbols.js:9](/Users/ccheever/projects/exact2/host/web-js/symbols.js:9). Native-button children have a specific `currentColor` override, [index.html:53](/Users/ccheever/projects/exact2/host/web/index.html:53); that is not general symbol behavior.

   Apple’s symbol configuration and cache key also explicitly read `tint_color`, [Affordances.swift:18](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Affordances.swift:18), and AppKit sets `contentTintColor` from it, [NodeSymbolMac.swift:43](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Mac/NodeSymbolMac.swift:43).

   “Refused on a symbol” needs runtime semantics because image sources can change between a symbol and a raster. The existing conformance fixture exercises exactly that case, [symbols.contract:32](/Users/ccheever/projects/exact2/host/web-js/conformance/symbols.contract:32). Shared classes and keyframes can also serve both kinds of image.

   **Resolution:** specify the changes on every renderer, including cache invalidation and presented color during animation. Define refusal after evaluating a dynamic source, and migration of symbol transitions/keyframes. Test inherited color changes, explicit overrides, hierarchical/palette/multicolor modes, native buttons, source switching, and retained raster tinting.

6. **Medium — D8 is not yet an enforceable completeness check, and it disagrees with D1.**

   [D8](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:127) proposes walking the style-name table, but the actual authority is a match function, [tags.rs:527](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:527). A manually supplied test list can omit a new match arm and still pass.

   Its other inconsistencies are substantive:

   - D1 admits an unprefixed browser implementation without a CSSWG definition; D8 requires each bare name to have a specification.
   - D1 covers functions; D8 does not clearly enumerate them.
   - The decoration omission shows why auditing only corner keywords and system colors is insufficient.
   - D1 describes browser names generally; D8’s provenance mechanism is specifically WebKit’s tables.

   Q5’s proposed single `"name"` per style row also cannot generate the current table by itself. Names and rows are many-to-many: `gap` sets two rows, animation longhands share one row, and `translate` sets both `Translate` and `TranslateZ`, [tags.rs:933](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:933), [tags.rs:869](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:869), [tags.rs:1057](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:1057).

   **Resolution:** define an enumerable authoring vocabulary consumed by both lowering and the test. Cover contextual keywords, functions, and motion names, with provenance matching D1. Keep this within the existing test check. Treat Q5 as a mapping-design change, not merely adding two fields to every row.

7. **Medium — D4’s recommendation is sound, but its claims about existing host coverage are inaccurate.**

   [D4](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:110) says `press-haptic` already has a web arm, symbol effects have web stand-ins, and scroll-edge effects have web/Linux stand-ins.

   The web explicitly omits those style rows, [css.rs:416](/Users/ccheever/projects/exact2/host/web/src/css.rs:416), [style.rs:518](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:518). The `navigator.vibrate` implementations belong to the **`haptic()` command**, [rt.js:212](/Users/ccheever/projects/exact2/host/web-js/rt.js:212), [glue.js:775](/Users/ccheever/projects/exact2/host/web/glue.js:775). LLP 1077 explicitly lists web symbol-effect stand-ins as still owed, [1077:305](/Users/ccheever/projects/exact2/llp/1077-css-visual-properties-native-draws-cheaply.rfc.md:305).

   Also, gaining another host would only force a rename if `-exact-apple-` were defined as exclusive host coverage. A provenance-based interpretation would not have that consequence.

   **Resolution:** retain the recommendation, correct planned-versus-built coverage, and reject the tier on its unnecessary distinction and ambiguity rather than claiming renaming is unavoidable.

8. **Medium — D5 and D6 need sharper boundaries.**

   [D5](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:117) should explicitly exclude author-defined animation names, timeline identifiers, font-family names, grid-line names, and Contract expression functions. These occur within style authoring without being vocabulary invented by Exact. Keyframe values already accept author palette functions, [keyframes.rs:66](/Users/ccheever/projects/exact2/contract/syntax/src/parser/keyframes.rs:66).

   Class participation is also not a reliable distinction between styles and props: `buttonStyle` is already a styleable prop, [schema.json:1276](/Users/ccheever/projects/exact2/kernel/tables/schema.json:1276), [LLP 1069.011 D12](/Users/ccheever/projects/exact2/llp/1069.011-native-buttons.rfc.md:87). Q3 should acknowledge this existing mechanism when motivating a material-property redesign.

   [D6](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:123) says graduation retires the LLP 1001 declaration. Adopting a standard name does not necessarily eliminate native approximations or supported-subset limitations. Its trigger, “CSS standardizes,” also needs to agree with D1’s admission of drafts and browser-shipped names.

   **Resolution:** define the rule over fixed vocabulary in CSS grammar, not arbitrary identifiers or Contract computations. Define graduation for properties, keywords, and functions; retire only the obsolete invention declaration, retaining any still-valid deviations.

9. **Medium — Verification and migration coverage are too narrow for the promise of unchanged behavior.**

   [§4 verification](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:146) includes useful checks, but `visual.contract` exercises corners and masks, not the complete renamed surface, [visual.contract:14](/Users/ccheever/projects/exact2/scripts/fixtures/visual.contract:14).

   The plan should explicitly verify:

   - Class overrides, conditional classes, and clearing a row absent from one branch; merging is name-based, [class.rs:21](/Users/ccheever/projects/exact2/contract/lower/src/class.rs:21).
   - Tint transitions/keyframes, finite exits, layout movement, drag timelines, press feedback, and dynamic system colors.
   - Contract embedded in Rust and JavaScript tests, not only `.contract` files. For example, [parity.rs:94](/Users/ccheever/projects/exact2/host/web/src/parity.rs:94) contains renamed attributes.
   - The affected apps launched and driven on native hosts, as required by [CLAUDE.md:43](/Users/ccheever/projects/exact2/CLAUDE.md:43).
   - Agent output boundaries. `layout` currently reports internal field names such as `press_scale`, through [agent.rs:574](/Users/ccheever/projects/exact2/runner/src/agent.rs:574). Those can remain stable; public CSS-text values and diagnostics need the new spelling.
   - External consumers beyond the two named in the migration paragraph. Bluesky is explicitly an admitted external consumer, [DEFERRED.md:198](/Users/ccheever/projects/exact2/rules/DEFERRED.md:198). Its impact cannot be established by an in-repository sweep.

   Updating only defining RFCs currently linked under `llp/current/` also misses relevant documents: LLP 1057.003 explicitly says it is not linked there, [1057.003:4](/Users/ccheever/projects/exact2/llp/1057.003-gesture-timelines.rfc.md:4).

   **Resolution:** add explicit behavioral acceptance cases and a repository-wide source/diagnostic sweep. Annotate every defining document, regardless of overlay membership. Record known external impact without claiming those applications were checked or that a blanket textual substitution is sufficient.

10. **Low — Correct the inventory count and distinguish author names from emitted declarations.**

   D2 lists **14 properties**, not 15: five earlier properties plus nine properties at bits 162–170. The two listed keyword renames are additional.

   The verified property bits are:

   | Properties | Bits |
   |---|---:|
   | `tint-color` | 86 |
   | `exit-animation`, `layout-transition`, `press-scale` | 142, 143, 144 |
   | `drag-timeline` | 147 |
   | `symbol-rendering`, `symbol-palette`, `symbol-value`, `symbol-effect` | 162–165 |
   | `press-haptic`, `content-transition`, `scroll-edge-effect`, `hover-effect`, `smart-invert` | 166–170 |

   See [schema.json:2470](/Users/ccheever/projects/exact2/kernel/tables/schema.json:2470), [schema.json:2832](/Users/ccheever/projects/exact2/kernel/tables/schema.json:2832), [schema.json:2867](/Users/ccheever/projects/exact2/kernel/tables/schema.json:2867), [schema.json:2964](/Users/ccheever/projects/exact2/kernel/tables/schema.json:2964). Several older numbers in LLP 1001 are stale—for example press-scale 147 and drag-timeline 150—and should be corrected during the declared update.

   §4’s blanket statement that the web writes no declarations for renamed rows is also inaccurate: presence and timelines emit `--exact-*` declarations, [css.rs:78](/Users/ccheever/projects/exact2/host/web/src/css.rs:78), and tint emits `--exact-tint`. These internal names need not change.

   **Resolution:** correct the count and state explicitly which author spellings change and which internal fields, ordinals, and browser custom properties remain stable.

On the **WebKit factual claims**, the asserted absence of `-apple-continuous` and bare `-apple-system-fill`, presence of the fully expanded label/separator names, and existence of gated `-apple-visual-effect` are consistent with my prior knowledge. I cannot verify the October 3 `main` contents here, and I do not identify a definite false table-membership claim from memory. “System web views do” is too broad without specifying which embedding configuration enables the setting. Replace “main, read 2026-10-03” with an immutable revision and evidence for parser exposure and meaning; table membership alone proves neither.

I suggest revising the inventory and D1 first, then completing the rename across all authoring contexts in one commit. Keep stage 2 separate. Add the focused behavioral cases above to existing tests and fixtures. For D7, extend the current vendor-prefix test to ordinary attributes, style blocks, keyframes, whitespace before `=`, equality expressions, and negation; I found no reason to broaden the lexer beyond that restriction.

My positions on the six §7 questions are:

1. **Q1 — Agree: decline `-exact-apple-`.** One Exact namespace is sufficient; host support and provenance belong in documentation. Correct D4’s factual examples and avoid treating coverage-driven renaming as inevitable.

2. **Q2 — Agree: use `-exact-system-fill`.** Preserve UIKit `systemFill`’s identity and current values. Substituting tertiary or quaternary fill would be a behavior change. Preserve its existing system-color index and update the JS mapper.

3. **Q3 — Agree to a separate comparison; disagree with automatic adoption after a value-name match.** Adoption must also establish availability, semantics, precedence, inheritance, vibrancy behavior, and fallback behavior. Compare the existing styleable-prop mechanism too. Keep this independent of the rename.

4. **Q4 — Agree with `color` for symbols and `-exact-tint-color` for rasters.** It is a sensible end state. Stage 2 needs the explicit renderer, animation, caching, and dynamic-source decisions in concern 5; the claim that the web already implements it must be removed.

5. **Q5 — Agree with schema authority; disagree that the proposed representation is merely mechanical.** Model author names and their mappings to rows, including shorthands and longhands. Keep author names distinct from internal field names and emitted browser names.

6. **Q6 — Agree for invented props, as a separate change.** CamelCase would make the distinction clearer. Preserve actual HTML/SVG/ARIA names, do not substitute a superficially similar CSS property with different semantics, and obtain Charlie’s intended decision on the bitmap-size vocabulary before changing it.

---



## Round 2: Draft r2 (2026-10-03)



- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one fresh session in the main checkout (375,029 tokens).

- **Reviewed:** `llp/1081-names-exact-invents.rfc.md` Draft r2, untracked over `0b871f54e` (blob `eec424cd`).

- **Redacted:** no.

- **Method:** mutually blind to Grok's round 2, which got the identical brief (sha256 `100cc1093eed103987a1297d409a74c85551cb10e803c4fed0930a22f52d05ca`). It had both round-1 reviews to check r2 against. Static audit in a read-only sandbox; ran nothing, changed nothing.

- **Transcription:** the final message after the last `codex` line, unedited.


**Overall verdict: NOT READY.** R2 fixes most of r1’s factual errors and establishes a sound naming policy. It still leaves concrete stage 1 failure paths and unresolved stage 2 behavior. D8 is feasible within the repository’s constraints, but its proposed representation needs adjustment.

This was a static audit of the supplied checkout. I changed nothing and ran no builds, tests, interpreters, dependency-source lookups, remote commands, skills, workflows, or sub-agents.

**R1 concerns: resolved or still open**

The numbering below refers to the [Astra review](/Users/ccheever/projects/exact2/llp/reviews/1081-names-exact-invents.astra.md:24) and [Grok review](/Users/ccheever/projects/exact2/llp/reviews/1081-names-exact-invents.grok.md:34).

| R1 concern | Assessment of r2 |
|---|---|
| Astra 1 / Grok 1: inconsistent classification | **Substantially resolved.** D1 now requires the same grammatical position and meaning, defines eligible documents and ordinary-content exposure, and removes native emission as a classification test. Two residual inconsistencies are covered below. [D1:79](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:79) |
| Astra 2 / Grok 2: missing vocabulary | **Inventory resolved.** `spring()` and the decoration correction are included; `env()` and media features are explicitly covered. Spring implementation coverage remains incomplete—concern 2 below. [§2:62](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:62) |
| Astra 3 / Grok 3: motion names, private names, hints | **Partially resolved.** The property lookup, keyframes, camelCase hints and private names are named. Author/internal parsing and computed JS transitions remain open—concerns 1, 3 and 8. [§4:175](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:175) |
| Astra 4 / Grok 3: dynamic colors, corner serialization, iOS vibrancy | **The identified mapping omissions are resolved in the plan.** R2 names the guard, serializer, computed corner mapper and Swift switch. The promised replacement diagnostics still need explicit handling. [§4:185](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:185) |
| Astra 5: stage 2 renderer facts and semantics | **Partially resolved.** Both web targets are correctly described; computed-source tint behavior is now chosen. Remaining Apple readers, shared-class rules and rendering details are covered in concerns 6–7. [Stage 2:230](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:230) |
| Astra 6 / Grok 5: enforceable classification table | **Partially resolved.** Lowering and the test would share a table, other vocabularies are added, and the many-to-many mapping is acknowledged. The stable-browser evidence mismatch remains; the proposed target representation and coverage need corrections. [D8:152](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:152) |
| Astra 7: inaccurate host-coverage argument | **Resolved.** D4 withdraws the false web-arm claims and rejects the Apple tier on ambiguity rather than inevitable renaming. [D4:125](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:125) |
| Astra 8 / Grok 6: exclusions and graduation | **Resolved.** Author identifiers, expression functions and styleable props are distinguished, and graduation retains still-valid deviations. [D5:133](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:133), [D6:148](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:148) |
| Astra 9 / Grok 7: verification and migration | **Mostly resolved.** Embedded sources, external consumers, defining documents outside the overlay and scoped migration are covered. Explicit class override/conditional-clear tests and tint motion round trips remain missing. [§4:192](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:192), [verification:215](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:215) |
| Astra 10 / Grok 4: counts, bits and emitted declarations | **Resolved.** The fourteen-property count is correct; the 35-file/158-line Contract count agrees with the expanded rename inventory. R2 preserves internal names and directs LLP 1001’s stale bits to be corrected from the schema. [§2:46](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:46), [§4:196](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:196) |

Grok’s recollection of bare `-apple-system-fill` is not sufficient grounds to reverse Q2. The asserted absence of that spelling and `-apple-continuous`, presence of the label/separator names, WebKit’s different four-argument spring grammar, and the privileged exposure of `-apple-visual-effect` are consistent with what I know. I cannot independently verify table contents or default settings at the pinned revision here. R2 appropriately distinguishes a UIKit API family from the CSS vocabulary WebKit exposes.

Applying D1 to the inspected vocabulary gives these results:

- **Exact:** all fourteen properties in §2; Exact’s spring function; the continuous-corner keyword; and the fill keyword. I found no additional invented, admitted name requiring a stage 1 rename in the requested source areas. The property inventory agrees with [tags.rs:923](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:923) and [tags.rs:1014](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:1014).
- **CSS:** the retained §2 properties; the two-token decoration; standard easing names/functions; `rgb()`, `rgba()`, `light-dark()`, `calc()`, the admitted shape/grid functions and CSS corner vocabulary. Declared subsets do not change their kind. See [parse.rs:114](/Users/ccheever/projects/exact2/motion/src/parse.rs:114), [style.rs:710](/Users/ccheever/projects/exact2/kernel/src/style.rs:710), [corner.rs:35](/Users/ccheever/projects/exact2/kernel/src/corner.rs:35).
- **Browser:** the three `-webkit-text-stroke` names and five retained Apple label/separator colors, subject to the pinned exposure evidence.
- **CSS environment/media vocabulary:** the four safe-area names, six indexed viewport-segment names, posture/segment-count features and inspected preference features. No extra invented name appeared. [env.rs:398](/Users/ccheever/projects/exact2/kernel/src/style/env.rs:398), [viewport.rs:78](/Users/ccheever/projects/exact2/runner/src/viewport.rs:78), [navigation.js:642](/Users/ccheever/projects/exact2/host/web/navigation.js:642)
- **Local vocabulary or outside D1:** `impact-light`, `numeric-countdown` and similar enumerants belonging to Exact properties stay bare. Motion’s `layout` and `box-shadow-color` are internal, not additional author properties; the lookup excludes them. [schema.json:1494](/Users/ccheever/projects/exact2/kernel/tables/schema.json:1494), [property.rs:184](/Users/ccheever/projects/exact2/motion/src/property.rs:184)

**Numbered remaining and newly identified concerns**

1. **High — Author-only rejection of `--exact-tint` needs a separate internal parsing path.**

   D5 and stage 1 correctly require authors to use `-exact-tint-color` while emitted CSS retains `--exact-tint`. However, the existing lookup accepts both specifically because serialized keyframes are parsed again. [RFC D5:145](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:145), [property.rs:184](/Users/ccheever/projects/exact2/motion/src/property.rs:184)

   Contract stores `rule.css()`, whose declarations use `p.css_name()`. The runner and JS emitter then call `Keyframes::parse` on that stored text. Their failure paths omit the rule. Simply removing the private alias from the shared lookup would therefore make newly compiled tint keyframes disappear at consumption time. [svg.rs:427](/Users/ccheever/projects/exact2/contract/lower/src/svg.rs:427), [animation/parse.rs:362](/Users/ccheever/projects/exact2/motion/src/animation/parse.rs:362), [bridge.rs:107](/Users/ccheever/projects/exact2/runner/src/bridge.rs:107), [emit.rs:708](/Users/ccheever/projects/exact2/host/web-js/src/emit.rs:708)

   **Resolution:** specify author parsing versus internal plan/CSS parsing, or reject reserved names at author-entry boundaries while preserving internal decoding. Add a compile → serialized plan → runner/JS consumption test that actually samples tint animation, alongside author rejection tests.

2. **High — Stage 1 misses a live reorder spring producer. The JS producer’s status is also determinable now.**

   The runner constructs `"translate spring(300,30,1)"` during reorder previews. It passes through the ordinary style bridge and reaches the transition parser. This is executable runtime code, not embedded Contract covered by the source-migration bullet. Rejecting bare `spring()` without changing this producer makes preview updates return an error. [reorder.rs:251](/Users/ccheever/projects/exact2/runner/src/instance/collection/reorder.rs:251), [views.rs:17](/Users/ccheever/projects/exact2/runner/src/instance/collection/views.rs:17), [build.rs:1309](/Users/ccheever/projects/exact2/kernel/build.rs:1309)

   The RFC leaves the JS reorder string conditional—rename it “if it reaches the author parser.” It does: `hooks.observe` is the observer that calls `m_transitions`. [RFC §4:184](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:184), [reorder.js:115](/Users/ccheever/projects/exact2/host/web-js/reorder.js:115), [motion.js:48](/Users/ccheever/projects/exact2/host/web-js/motion.js:48), [motion.js:155](/Users/ccheever/projects/exact2/host/web-js/motion.js:155)

   **Resolution:** explicitly rename both producers and drive reorder previews on the runner-backed hosts and JS target. Keep only the separately identified `presence-glue.js` protocol spelling unchanged.

3. **High — Computed JS transitions bypass the naming boundary.**

   The JS transition mapper filters spring-containing entries and otherwise passes author text through. The pressed-node variant additionally rewrites `scale`; neither translates the author tint property into `--exact-tint` nor rejects author access to that private name. [style.rs:572](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:572), [rows.rs:201](/Users/ccheever/projects/exact2/host/web-js/src/rows.rs:201)

   Consequently, a runtime-produced `"--exact-tint 1s"` can reach browser CSS despite D5’s prohibition. `"tint-color 1s"` remains syntactically acceptable as a transition property identifier, without a rename diagnostic. The new `"-exact-tint-color 1s"` also needs translation to animate the registered custom property. Ordinary non-spring nodes do not automatically go through the motion parser. [motion.rs:15](/Users/ccheever/projects/exact2/host/web-js/src/motion.rs:15)

   R2’s instruction that the substring filters work “unchanged” establishes only spring filtering, not author-name validation. [RFC §4:183](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:183)

   **Resolution:** specify validation and translation for computed transition text, including pressed nodes. Test strings produced from runtime data, with old, new and reserved spellings; literal-only tests will miss this path.

4. **Medium — D1’s local-vocabulary exception contradicts its treatment of shared value grammars.**

   D1 says only a keyword or function added to a CSS/browser property takes the prefix. Read literally, that permits bare `spring()` inside Exact’s own `-exact-layout-transition`. Yet §2 explicitly requires `-exact-spring()` there. The same ambiguity applies to `-exact-system-fill` inside `-exact-symbol-palette`. [D1:97](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:97), [§2:62](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:62)

   Two implementers could reasonably distinguish “the property’s own vocabulary” differently.

   **Resolution:** limit the bare-name exemption to local enumerants such as `impact-light`. State that imported shared grammars—colors, easings and transition lists—retain their canonical vocabulary inside Exact properties. With that clarification, the current inventory has an operational classification.

5. **Medium — D8 needs a richer target representation and provenance/coverage corrections.**

   The proposed `(name, Kind, &[StyleId])` table does not by itself represent all current style lowering. `flex` maps to `AttrTarget::Flex`, which produces three **different** values: authored grow, shrink `1`, basis `0%`. Treating it as an ordinary row slice changes behavior; leaving it outside the classification table loses coverage. [RFC D8:152](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:152), [tags.rs:1002](/Users/ccheever/projects/exact2/contract/lower/src/tags.rs:1002), [lib.rs:1223](/Users/ccheever/projects/exact2/contract/lower/src/lib.rs:1223)

   Two r1 gaps also remain. `Css(spec)` cannot record D1’s alternative qualification through stable, unprefixed browser shipping without a specification. And D8’s enumerated coverage omits media features and non-easing function vocabularies, although D1 covers them. Its explicit enum exception does not explain those omissions. [D1:91](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:91), [D8:154](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:154)

   **Resolution:** use a target descriptor that preserves special lowering, represent either specification or engine/version evidence for CSS names, and either cover the remaining vocabularies or accurately limit the test’s claim. Exclude internal motion entries explicitly.

   **Buildability assessment:** there is no unavoidable line-cap or generator obstacle. `tags.rs` and `kernel/build.rs` are each 1,499 lines; moving the author mapping frees space, and the other named vocabulary modules have ample room. Keep the new table solely a Contract syntax mapping onto generated IDs. Schema rows, bits and enum declarations must remain in `schema.json`; stage 1b edits that enum and handwritten parsing/generation logic, never generated output. A shared `Kind` type must also live where Motion and Kernel can use it without depending upward on Contract. [CLAUDE.md:28](/Users/ccheever/projects/exact2/CLAUDE.md:28), [LLP 1001 §1:25](/Users/ccheever/projects/exact2/llp/1001-kernel-v1.spec.md:25)

6. **High — Stage 2 omits direct iOS symbol-color readers.**

   Updating `Affordances.swift`’s configuration/cache key and macOS `contentTintColor` is insufficient. Ordinary iOS symbol views separately assign `symbolView.tintColor` from `tint_color`. That assignment must change too. [RFC stage 2:233](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:233), [NodeViewIOS.swift:423](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:423)

   Projected UI has additional readers: tab-bar faces read the image’s tint, and custom navigation badges select `tint_color` for symbols. The badge path explicitly excludes native buttons, so the RFC’s native-button exception does not settle it. [SegmentsIOS.swift:61](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:61), [NavigationBarIOS.swift:133](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:133), [NavigationBarIOS.swift:48](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:48)

   **Resolution:** include every symbol-color consumer, or explicitly document any projected-control exception in LLP 1001. Verify ordinary iOS monochrome/hierarchical symbols, selected tabs and custom navigation badges, including inherited color changes.

7. **Medium — Stage 2 still requires decisions about classes, modes and color-driven cache updates.**

   A literal symbol with tint is a compile error, but a shared class may carry both color and tint. What happens when that class is applied to a literal symbol? Class expansion currently merges rows into the element’s attributes, so the distinction is operationally significant. [RFC stage 2:239](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:239), [class.rs:14](/Users/ccheever/projects/exact2/contract/lower/src/class.rs:14)

   “Palette and multicolor unchanged” also needs a fallback rule. An empty palette currently returns the base configuration, whose eventual color comes from the view tint. Changing that tint to `color` changes this case. Both web targets currently provide a monochrome mask approximation, so their handling of these modes should be stated explicitly. [Affordances.swift:32](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Affordances.swift:32), [symbols.js:9](/Users/ccheever/projects/exact2/host/web-js/symbols.js:9)

   Finally, changing the cache key for every color update calls `showSymbol` again. With `symbol-effect: replace`, that invokes a replacement transition even when only paint changed. R2 does not say whether this is intended during `transition: color`. [NodeViewIOS.swift:410](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:410), [Affordances.swift:60](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Affordances.swift:60)

   **Resolution:** define whether the compile error applies only to direct attributes or also merged classes; define empty-palette/multicolor fallback; and distinguish paint updates from symbol replacement. Require the **presented** color during animation—the Apple host already propagates it to inheriting descendants. Shared symbol/raster keyframes also need an explicit migration choice, since globally replacing tint with color removes raster tint animation. [paragraph.rs:383](/Users/ccheever/projects/exact2/host/apple/src/paragraph.rs:383)

8. **Medium — Updating quoted error text does not deliver D2’s promised rename hints.**

   The generator discards transition parser errors when converting them to `BadTransition`. Contract then reports a generic shorthand error. `BadColor` is similarly generic; changing the system-color table does not make it explain the old fill spelling. [build.rs:1309](/Users/ccheever/projects/exact2/kernel/build.rs:1309), [values.rs:134](/Users/ccheever/projects/exact2/contract/lower/src/values.rs:134), [values.rs:146](/Users/ccheever/projects/exact2/contract/lower/src/values.rs:146)

   The JS system-color mapper substitutes recognized names; deriving its guard from the new table does not itself diagnose obsolete names. These are additional operations beyond “every error text that quotes a renamed name.” [RFC §4:179](/Users/ccheever/projects/exact2/llp/1081-names-exact-invents.rfc.md:179), [style.rs:473](/Users/ccheever/projects/exact2/host/web-js/src/style.rs:473)

   **Resolution:** specify where rename reasons survive or are reconstructed across Contract, Kernel/runner and JS runtime boundaries. Cover mixed-case keywords and colors embedded in supported composite values. Explicitly test replacement text, not merely rejection.

**Suggestions**

- Keep the three changes separate: mechanical renames, stage 1b decoration repair, then symbol-color semantics. The decoration correction is correctly specified and belongs in the schema.
- Replace §8’s claim that §4 names “every place” with a checked inventory that includes internal producers and serialized-value consumers.
- Add focused acceptance cases to existing tests: tint keyframe round trips; both reorder implementations; computed JS transitions; class overrides and conditional clears; and literal/computed keyword diagnostics.
- For stage 2, test intermediate animation frames and symbol↔raster switching, including shared classes, empty palettes and `symbol-effect: replace`, on iOS, macOS and both web targets.
- Preserve the accepted §7 direction. The remaining work is to close implementation and semantic gaps; none of these findings calls for an `-exact-apple-` tier or substituting a different fill color.

