# Review: LLP 1053 r1, 2026-09-26 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort max, read-only sandbox, `-C` the fill-policy worktree at e992cefe with the RFC uncommitted; 2026-09-26T01:26:45Z to 2026-09-26T01:40:25Z.
- **Method:** the shared brief (sha256 `23ee50a38017d02202b35d77b93a01bacb2cb716254d507a7864bf33e283ed87`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1053 r1.
- **Redacted:** nothing.
- **Disposition:** LLP 1053 §0 (r2).

---

## Review (verbatim)

**Build a narrower, corrected version of this proposal.** G1–G5 and a constrained G7 are worthwhile; G8 needs end-to-end measurement, G9 needs a precise launch policy, and G6/G11 should wait.

This was a source-only audit. I made no edits and ran no builds or tests. I did not inspect the external benchmark directories or the separate rendering-fix branch. Consequently, I can assess the checked-out implementations, but cannot certify the reported benchmark measurements, historical fixes, or browser-version claims.

1. **G1 — `aspect-ratio`: agree with adding it; disagree that it is trivial or essentially risk-free.**

   The existing-row claim is correct: the schema stores an `f32`, and the kernel passes positive finite values to Taffy. The RFC’s `layout.rs:731` citation points to a measurement calculation, however; the actual style lowering is in [style.rs:979](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/style.rs:979). The row defaults to zero. [schema.json:1148](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:1148)

   There is a concrete Taffy risk: Exact’s leaf patch assumes that **a leaf with an aspect ratio is a replaced element**, and applies replaced-element constraints whenever a positive ratio exists. That assumption does not hold for a text leaf with an authored preferred ratio. The constraint function can change both dimensions when only one exceeds a maximum. This deserves a targeted browser differential before exposing the property broadly. [leaf.rs:150](/Users/ccheever/projects/exact2-wt-fill-policy/vendor/taffy/src/compute/leaf.rs:150), [leaf.rs:207](/Users/ccheever/projects/exact2-wt-fill-policy/vendor/taffy/src/compute/leaf.rs:207)

   Correct the value design:

   - CSS accepts `auto`, a ratio, or `auto` combined with a ratio in either order.
   - A ratio has nonnegative numeric components and an optional denominator, defaulting to one. Degenerate ratios need CSS’s defined treatment, not division producing infinity/NaN.
   - A preferred ratio does not override two definite dimensions.
   - `auto <ratio>` matters particularly for photos: the supplied ratio is a fallback until an intrinsic ratio is available.

   **Include `auto` now.** Currently, merely setting the row suppresses intrinsic-ratio substitution, even when its numeric value produces no usable ratio. Mapping `auto` to zero without changing that logic would be wrong. [style.rs:1073](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/style.rs:1073)

2. **G2 — per-side border colours: agree, but the Apple support claim is incomplete and the cost is understated.**

   The four rows already exist and default to `currentcolor`; Contract exposes only the shared `border-color` expansion. Web emission already constructs the correct side-specific CSS property names. [schema.json:1293](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:1293), [tags.rs:448](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/tags.rs:448), [css.rs:234](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/src/css.rs:234)

   **“iOS already can” is not generally true.** `BoxLayerIOS` correctly detects differing colours and selects custom drawing. But that drawing path uses the top colour for the entire rounded stroke whenever the four widths match; it does not also require matching colours. macOS has the same mistake. [BoxLayerIOS.swift:50](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/IOS/BoxLayerIOS.swift:50), [NodeViewIOS.swift:1276](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1276), [NodeViewMac.swift:1265](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1265)

   Linux carries the individual colours, but its nonuniform path paints four rectangles. That is not sufficient for rounded, differently coloured borders and their corner joins. [paint.rs:201](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/paint.rs:201)

   Preserve the existing `currentcolor` mechanism: border colour is not inherited; its initial value resolves against the element’s computed `color`. Do not replace it with a fixed black default or resolve it permanently during compilation. The kernel already represents that distinction. [style.rs:436](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/style.rs:436), [style.rs:903](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/style.rs:903)

   Also record that today’s `border-color` expansion assigns one value to all four rows; it is not the complete CSS one-to-four-value shorthand. [lib.rs:1284](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/lib.rs:1284)

3. **G3 — `flex-grow`: agree. `flex-basis` is already exposed.**

   The table correctly identifies the missing longhand; the summary’s proposal to add `flex-basis` is redundant. `flex-shrink` and `flex-basis` are already named. [tags.rs:460](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/tags.rs:460)

   The distinction from `flex=1` is substantive. The existing shorthand writes **grow = 1, shrink = 1, basis = 0%**. Setting only `flex-grow=1` should preserve the other declarations, including the initial `flex-basis:auto`. The compiler already resolves overlapping row bindings with the last binding winning. [lib.rs:1242](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/lib.rs:1242), [lib.rs:887](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/lib.rs:887), [schema.json:1074](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:1074)

   Add nonnegative validation for grow and shrink. Their current generic float conversion checks finiteness but accepts negative numbers. [style.rs:346](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/style.rs:346)

   Test both declaration orders, class versus local overrides, shrink under pressure, and `0%` versus zero length in an indefinite main axis. “Complete longhand names” should not imply complete CSS `flex` grammar: the existing shorthand implementation handles the numeric form.

4. **G4 — `font-variant-numeric`: agree with `normal` and `tabular-nums`; disagree with the existing-support claims and “trivial names” categorization.**

   The kernel row exists and reaches `TextStyle`, but that is where usable support largely stops. [text.rs:103](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/text.rs:103)

   The RFC’s wording about host support is materially wrong:

   - **Web explicitly skips the row** as “not lowered in v1.” [css.rs:121](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/src/css.rs:121)
   - **Apple’s measurement ABI does not carry it.** Its font lookup also has no numeric-feature parameter. [measure.rs:29](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/src/measure.rs:29), [Text.swift:387](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:387)
   - **Linux drops it when constructing a text run**, and its attributes builder sets no numeric feature. [text.rs:89](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text.rs:89), [catalog.rs:236](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text/catalog.rs:236)

   Linux does have a useful implementation building block: the vendored shaper accepts font features and passes them into shaping. A dependency replacement is not indicated. [attrs.rs:167](/Users/ccheever/projects/exact2-wt-fill-policy/vendor/cosmic-text/src/attrs.rs:167), [shape.rs:155](/Users/ccheever/projects/exact2-wt-fill-policy/vendor/cosmic-text/src/shape.rs:155)

   CSS semantics matter here: `normal` restores normal feature selection; it does **not** mean “force proportional digits.” `tabular-nums` requests a feature of the selected font, not replacement with a monospaced system font. Unsupported font features should not invent glyphs.

   The full property combines mutually exclusive figure, spacing and fraction choices with independent features such as `ordinal` and `slashed-zero`. Supporting only two values initially is reasonable if other values are rejected explicitly. A raw integer row still needs a keyword parser; the generated `u8` conversion currently accepts integers. [build.rs:1315](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/build.rs:1315)

5. **G5 — `nowrap` and `pre-line`: agree with the feature, but expand the work beyond adding enum cases.**

   The existing enum really contains only `normal` and `pre-wrap`. [schema.json:819](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:819)

   The required semantics are:

   | Value | Whitespace handling | Soft wrapping |
   |---|---|---|
   | `normal` | Collapse spaces and segment breaks | Allowed |
   | `nowrap` | Same collapsing as `normal` | Suppressed |
   | `pre` | Preserve spaces and segment breaks | Suppressed |
   | `pre-wrap` | Preserve spaces and segment breaks | Allowed |
   | `pre-line` | Collapse spaces; preserve segment breaks | Allowed |
   | `break-spaces` | Preserve spaces with additional breaking and sizing rules | Allowed |

   In the newer model, these are combinations of **`white-space-collapse` and `text-wrap-mode`**. For example, `nowrap` is `collapse nowrap`; `pre-line` is `preserve-breaks wrap`. Separate those dimensions internally even if the first authored interface remains the familiar shorthand.

   The RFC’s min-content = max-content statement applies to uniformly nonwrapping content, accounting for forced breaks. **It does not apply to `pre-line`.** Likewise, “a mode that does not break lines” must distinguish soft opportunities from forced breaks.

   There are two existing omissions:

   - Native ordinary text passes raw strings into CoreText/cosmic-text without a CSS collapsing preparation step. Adding another enum case alone will not produce correct `normal`, `nowrap`, or `pre-line` behavior. [Text.swift:443](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:443), [shaping.rs:131](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text/shaping.rs:131)
   - `white_space` is paragraph-level in the measurement interface, whereas CSS allows differently styled inline descendants. The run style does not carry it. Mixed inline whitespace policies therefore need an explicit treatment. [text.rs:90](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/text.rs:90), [text.rs:145](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/text.rs:145)

   Include genuine inline-overflow ellipsis in this work. Apple’s `Spec` and Linux’s `Spec` omit `text-overflow`; their truncation paths are driven by `line-clamp`. That does not implement the RFC’s unclamped `nowrap` + hidden overflow + ellipsis example. [Text.swift:70](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:70), [Text.swift:623](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:623), [text.rs:106](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text.rs:106), [shaping.rs:237](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text/shaping.rs:237)

6. **G6 — `text-transform`: disagree with “add now” as designed. Keep it as a real feature with a larger, explicit design.**

   The missing capability is real. The current text interface carries source strings and font metrics, without a transform or language field. [text.rs:90](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/text.rs:90), [text.rs:181](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/src/text.rs:181)

   The proposed implementation is insufficient:

   - Full case mapping can expand strings; `ß → SS` is only one example.
   - Lowercasing needs contextual handling, including Greek final sigma.
   - Language affects Turkish/Azeri casing, Lithuanian behavior and Dutch capitalization. “Locale-insensitive initially” must not silently override an authored language. Contract already has `lang`, and the web emits it. [tags.rs:348](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/tags.rs:348), [element.rs:237](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/src/element.rs:237)
   - `capitalize` titlecases the relevant initial letter unit; it does not lowercase the rest of each word. UAX #29 is a useful boundary basis, not a complete implementation specification.
   - Adjacent inline runs can split a word or contextual case sequence.

   **Preserve source text and maintain source-to-rendered offsets.** Transforming the sole stored string is incompatible with dependable selection, links, accessibility and copy. Apple currently assigns inline ranges using original UTF-16 lengths, while ordinary macOS copy slices the paragraph’s run strings. A length-changing transform must update both sides deliberately. [InlineText.swift:79](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/InlineText.swift:79), [TextSelectionMac.swift:203](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift:203)

   CSS transformation is presentational. Preserve source content for plain-text copying; do not make the RFC’s unverified Chrome/Safari comparison the data-model contract.

   Prefer browser CSS for ordinary web text and shared, source-preserving preparation for native text. A universal kernel rewrite would also require reconciling the existing separate measurement and presentation paths, not merely handing every host a transformed string. For one uppercased site label, this is poor immediate return on implementation risk.

7. **G7 — expression-valued `font-family`: agree with a constrained first implementation.**

   The literal-only claim is correct. The compiler registers generic and declared stacks, then lowers a literal family name to a numeric stack ID. The runner already validates numeric family IDs against the plan’s stack count. [fonts.rs:13](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/fonts.rs:13), [lib.rs:1261](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/lib.rs:1261), [bridge.rs:60](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/bridge.rs:60)

   **Prefer compiling finite choices to existing IDs**, such as a conditional choosing between two known families. That solves the benchmark without introducing runtime string-based font discovery. Arbitrary resource-provided names need an explicit unknown-name policy.

   Preserve declared-face validation. Today the compiler rejects certain requests for missing real italic/bold faces; dynamic family selection must not accidentally bypass that policy. [values.rs:162](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/values.rs:162)

   Two corrections to the RFC:

   - “Doubles row nodes” overstates the runtime cost of `when`: the runner destroys the old arm and realizes only the selected arm. It duplicates authored/compiled alternatives and causes switching churn, not necessarily twice as many live nodes. [instance.rs:1073](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/instance.rs:1073)
   - Font-family fallback lists are a separate existing limitation: comma-separated families are explicitly rejected. Do not imply that expression support completes CSS font-family support. [fonts.rs:188](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/fonts.rs:188)

8. **G8 — keyed partial answers: agree with measuring first; disagree with the proposed binary decision and runner-only measurement.**

   “Shares unchanged records” needs qualification. Ordinary keyed rows retain an equivalent previous row item, but this happens **after** processing the replacement list. Resource settlement validates fresh values and can retain the entire previous answer when equivalent; it does not generally rebuild a changed answer using all previous record allocations. [instance.rs:993](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/instance.rs:993), [settlement.rs:493](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/settlement.rs:493)

   The current collection path has useful optimizations: it can reuse prior keys for pointer-identical items and avoid rebuilding key storage when keys remain unchanged. It nevertheless iterates the input items. The older list-window path also processes the replacement list and reconstructs its height data. [collection/mod.rs:261](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/instance/collection/mod.rs:261), [window.rs:316](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/instance/window.rs:316)

   Fresh-answer shape checking recursively traverses lists and records. Serialized answers also decode into new allocations, so in-process Rust sharing is not representative of all execution paths. [value.rs:108](/Users/ccheever/projects/exact2-wt-fill-policy/plan/src/value.rs:108), [value.rs:212](/Users/ccheever/projects/exact2-wt-fill-policy/plan/src/value.rs:212), [envelope.rs:100](/Users/ccheever/projects/exact2-wt-fill-policy/data/src/envelope.rs:100)

   Measure **producer work → serialization/transfer → decoding → validation → settlement → collection update → layout/publication**. Report allocations, bytes and retained memory alongside p50/p95/p99 latency and scroll-frame interference. Cover unchanged refreshes, first/last/offscreen-row updates, inserts, removals and reorders. A single “under 1 ms” runner result cannot decide this.

   My alternative order is:

   - Keep presentation-only selection/toggle state out of a giant data answer where appropriate.
   - Reuse immutable records in an in-process producer.
   - Return bounded windows or pages before designing a generic delta protocol. Messages already returns a bounded slice and has a test exercising 25,000-row histories. [app.ts:336](/Users/ccheever/projects/exact2-wt-fill-policy/apps/messages/app.ts:336), [window.rs:217](/Users/ccheever/projects/exact2-wt-fill-policy/apps/messages/apple/tests/window.rs:217)
   - Consider keyed deltas only if an actual full-history workload still needs them.

   A patch protocol needs more than insert/update/remove: base revision, resource arguments, ordered insertion/movement, duplicate-key policy, reset/resynchronization, schema validation, atomic rejection and stale-result handling. A resource’s stable key must be declared independently of an arbitrary UI `each` key. Existing values are ordered lists and positional records, not keyed maps. [value.rs:27](/Users/ccheever/projects/exact2-wt-fill-policy/plan/src/value.rs:27)

   Applying a patch to an `Rc<Vec<Value>>` can still copy an O(N) outer vector, and downstream derivations may still scan it. Do not promise O(changes) without measuring the whole path. Preserve existing transactional rollback and stale-ticket rejection. [commit.rs:30](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/commit.rs:30), [commit.rs:583](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/commit.rs:583)

9. **G9 — launch-time values: agree with declarative resources over a mount action; disagree that this is a generally missing lifecycle capability.**

   **“Resources are baked at build time” is too broad.** Bake marks device-store readers specially and omits compiled answers for resources that remain pending. At runtime, placeholders can supply the first presentation and `data_ready()` re-asks deferred resources. [lib.rs:532](/Users/ccheever/projects/exact2-wt-fill-policy/contract/cli/src/lib.rs:532), [settlement.rs:416](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/settlement.rs:416), [kept.rs:127](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/kept.rs:127)

   The narrower missing capability is **declaring that an otherwise synchronous source must be evaluated on the device rather than accepting its baked answer**. That is a plausible addition.

   Separate the examples:

   | Input | Preferred treatment |
   |---|---|
   | First-run marker, saved preference | Existing device-store resource behavior |
   | Cheap host fact such as launch locale | Host-provided session snapshot, exposed declaratively |
   | App-defined runtime computation | Runtime-only resource with an explicit placeholder |
   | Benchmark live flag already known to the launcher | Supply it as launch configuration; do not wait for a timer |

   A `not-bakeable` flag needs to answer two different questions: **must bake never call the source**, or **may bake call it for a placeholder but must launch refresh it**? Those are different contracts. Merely discarding a baked result does not prevent build-time evaluation.

   Also define “once”: process, session, scene, app-root mount, or resource activation. Decide how reload carry, document checkpoints, argument changes and explicit refresh interact. The runner currently carries matching named/source answers across reloads, so “not baked” alone does not ensure a new launch read. [runner.rs:621](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner.rs:621)

   Do not promise a device-dependent value in a baked first frame when it requires app JavaScript or storage I/O. The existing source contract defers storage opening until after first pixel, and the binding rules prohibit app JS before first pixel. [source.rs:65](/Users/ccheever/projects/exact2-wt-fill-policy/runner/src/runner/source.rs:65), [RULES.md:54](/Users/ccheever/projects/exact2-wt-fill-policy/rules/RULES.md:54)

   A general `mount` action adds lifecycle and duplicate-execution questions without solving those timing constraints. I would drop that candidate.

10. **G10 — outside-app bake inputs: agree with retaining the current boundary; correct the suggested workaround.**

    The refusal is real for the TypeScript source-capture pipeline: it captures app-local files and rejects imports outside that captured graph. [lib.rs:415](/Users/ccheever/projects/exact2-wt-fill-policy/js/bake/src/lib.rs:415), [lib.rs:717](/Users/ccheever/projects/exact2-wt-fill-policy/js/bake/src/lib.rs:717)

    **“Copying or linking” is wrong for symlinks.** Source capture explicitly rejects them; declared fonts also canonicalize paths and refuse escape from the app root. [lib.rs:440](/Users/ccheever/projects/exact2-wt-fill-policy/js/bake/src/lib.rs:440), [fonts.rs:142](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/fonts.rs:142)

    Say “copy or materialize the input into the captured app.” An explicit external-input capture mechanism could also be hermetic if it hashed and snapshotted everything, but this benchmark does not justify adding one.

    Narrow “bake refuses them” to the relevant input pipeline; `bake/` itself is the delivery/compatibility crate, not the sole authority for every possible build input. [bake/src/lib.rs:1](/Users/ccheever/projects/exact2-wt-fill-policy/bake/src/lib.rs:1)

11. **G11 — native switch: disagree with building it in this tranche. First specify a functional checkbox contract.**

    The missing authored switch is real, but the table overlooks existing scaffolding: the kernel has `Toggle` and `toggleValue`, and the web maps them to an input and `checked`. Contract’s `input`, however, always lowers to `TextInput`. [schema.json:37](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:37), [schema.json:87](/Users/ccheever/projects/exact2-wt-fill-policy/kernel/tables/schema.json:87), [element.rs:275](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/src/element.rs:275), [tags.rs:131](/Users/ccheever/projects/exact2-wt-fill-policy/contract/lower/src/tags.rs:131)

    This is more than choosing `UISwitch`/`NSSwitch`. The current web change handler sends `el.value`, not checkbox checked state. Apple’s input handling is text-field handling. [glue.js:522](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/glue.js:522), [NodeViewIOS.swift:1064](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1064), [NodeViewMac.swift:1003](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1003)

    Define checked-state binding, boolean event payloads, keyboard activation, disabled/inert behavior, focus, accessibility, intrinsic size, programmatic changes and view recycling. Reuse or replace the existing toggle scaffolding deliberately.

    Treat the `switch` spelling as an enhancement requiring browser testing; the RFC’s Safari-version assertion does not establish an interoperable contract. A visual fallback is acceptable only if the control’s behavior remains correct.

The RFC should additionally record these gaps and corrections:

- **Native inline background support is not merely an iOS issue.** Apple’s shared run type lacks a background field and its inline decoder skips unrecognized style fields. Linux skips independently painting inline nodes, while its run paint data contains only colour and source identity. Extend the rendering-bug scope to macOS and Linux. [Text.swift:28](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:28), [BatchShapes.swift:104](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/BatchShapes.swift:104), [paint.rs:784](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/paint.rs:784), [text.rs:174](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text.rs:174)
- **Intrinsic text measurement needs attention alongside G5.** Apple’s min-content calculation splits each run independently on Unicode whitespace. That can split an unbreakable word at a style boundary and treats whitespace more broadly than CSS collapsible spaces. Test cross-run words, NBSP and CJK opportunities. [Text.swift:744](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/Text.swift:744)
- **The shared text-flow implementation is another affected subsystem.** It has its own two-value whitespace enum and preparation logic. Web and native adapters also explicitly distinguish only normal/pre-wrap. “Three text engines” understates the number of paths that must change together. [walker.rs:42](/Users/ccheever/projects/exact2-wt-fill-policy/textflow/src/walker.rs:42), [textflow-glue.js:126](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/textflow-glue.js:126), [textflow.rs:157](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/src/textflow.rs:157), [flow.rs:180](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text/flow.rs:180)
- **Language-sensitive layout is incompletely represented.** `lang` exists, but the shared measurement interface lacks language. G6 should close that gap rather than add a separate global-locale assumption.
- **Photo cropping has a separate limitation from sizing.** Linux’s `object-fit` implementation always centres the image; noncentral focal positioning is a distinct potential gap. List it only if the photo benchmark actually needs it. [paint.rs:1037](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/paint.rs:1037)

For host costs and parity coverage, I would require the following before calling the selected features complete:

| Host | Understated work | Required parity coverage |
|---|---|---|
| **Web** | Ordinary CSS emission is only one path. Flow text uses canvas measurement and generated fragments; its font key currently omits numeric variants and its preparation reads source text. [textflow-glue.js:65](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/textflow-glue.js:65), [textflow-glue.js:179](/Users/ccheever/projects/exact2-wt-fill-policy/host/web/textflow-glue.js:179) | Compare ordinary DOM and flowed text. Check computed properties, intrinsic/used sizes, dynamic updates and baked-page adoption. Use actual Chromium and WebKit fixtures for behavior where the RFC claims divergence. |
| **Apple, both platforms** | Extend the C measurement ABI, Swift runs, inline decoding, font selection and cache identities together. The borrowed-request identity code currently enumerates the existing metric fields explicitly. [TextResidency.swift:16](/Users/ccheever/projects/exact2-wt-fill-policy/host/apple/Sources/ExactKit/TextResidency.swift:16) | Check measurement against painted geometry, cold/warm caches, dynamic font-feature changes, fallback fonts, recycled rows and delayed raster publication. Test rounded multicolour borders separately on iOS and macOS. Selection/link offsets and copy must remain correct after any text preparation. |
| **Linux** | Extend run attributes and every relevant cache comparison; current fingerprint/equality code explicitly lists the old fields. Both ordinary and flow layout must agree, and border fixes must reach CPU/GPU painting. [cache.rs:818](/Users/ccheever/projects/exact2-wt-fill-policy/host/linux/src/text/cache.rs:818) | Use a bundled font with known numeric features and a font lacking them. Check intrinsic widths, wrapped geometry, clipping/ellipsis, mixed-script text, cache invalidation and both paint backends. |

The shared fixture set should include:

- **G1/G3:** image versus non-replaced text; one/both/no definite dimensions; min/max constraints; padding/borders under both box-sizing modes; flex row/column; shorthand/longhand ordering; image metadata arriving later.
- **G2:** four colours, unequal widths, rounded corners, transparency, omitted colours, explicit `currentcolor`, inherited colour changes and light/dark changes.
- **G4/G7:** inherited and per-run feature/family changes; `normal` reset; bold/italic declared faces; fallback glyphs; font changes that alter line breaks.
- **G5:** spaces/tabs/CRLF, leading/trailing whitespace, blank lines, NBSP, cross-run words, nested whitespace modes, RTL/CJK, intrinsic widths, hidden versus visible overflow, and ellipsis with **no** line clamp.
- **G8/G9:** stale replies, refused commits, reordered/deleted anchors, reload carry, placeholders, first-paint timing, and multiple sessions. These are architectural correctness tests, not just performance samples.

My priority order would be:

1. **Fix claimed rendering semantics first**, including the broader inline-background issue and the prerequisite whitespace/ellipsis defects.
2. **G3**, including negative-value validation and shorthand interaction.
3. **G2**, including native corner painting.
4. **G1**, with `auto` and targeted Taffy constraint checks.
5. **G4**, implemented through the complete host paths.
6. **G5**, as coherent whitespace preparation plus wrapping/overflow behavior.
7. **G7**, starting with compiler-resolved finite family choices.
8. **Measure G8 end to end**, then prefer bounded answers unless evidence specifically demands deltas.
9. **Resolve G9’s narrow policy**, using existing deferred/store resources wherever sufficient.

**Build now:** G3, G2, corrected G1, G4, coherent G5, and constrained G7. **Defer:** G6 and G11. **Do not build yet:** a generic keyed-patch protocol, a mount action, or external-input capture infrastructure. Keep G10’s boundary and fix its documentation.

That selection follows the repository’s existing preference for bounded answers over generic arrival-time reconciliation, while preserving the first-pixel constraint. The relevant scope rule already names that trade; it should remain explicit if the eventual G8 design changes it. [NOT-DOING.md:213](/Users/ccheever/projects/exact2-wt-fill-policy/rules/NOT-DOING.md:213)