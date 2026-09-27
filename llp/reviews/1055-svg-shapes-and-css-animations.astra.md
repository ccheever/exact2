# Review: LLP 1055 r1, 2026-09-26 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort max, read-only sandbox, `-C` a detached review worktree at d357a7c4 (r1 committed on origin/main 0fd40388); 2026-09-26T22:57:56Z to 2026-09-26T23:12:40Z.
- **Method:** the shared brief (sha256 `c6185a771ae30f6b2ffaddea8069578336b3632d80bb640dbd7336b708d9fad5`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1055 r1.
- **Redacted:** nothing.
- **Disposition:** LLP 1055 §0 (r2).

---

## Review (verbatim)

Draft r1 is **not ready to build unchanged**. The overall direction is sound, but D5 contains incorrect CSS semantics, D7 is not a complete faithful lowering, and D3 understates the consequences of introducing nodes without boxes.

This is a static audit of `d357a7c4` over `0fd40388`. I used no skills or agents, made no edits, and ran no builds, tests, interpreters, or browser checks. Standards conclusions below are not claims of measured Chrome or Core Animation conformance.

1. **D1 — Agree with a bounded SVG subset; correct the property classification and specify the grammar more precisely.**

   The chosen elements cover the stated consumer. Omitting `ellipse`, nested SVG, paint servers, and transforms is reasonable when explicitly refused. There are **eight element types**, including `svg`, rather than seven.

   Several statements are wrong or incomplete:

   - **`x`, `y`, `rx`, and `ry` are SVG 2 geometry properties.** They are CSS properties as well as presentation attributes, like `cx`, `cy`, and `r`. Assigning them exclusively to props contradicts D2’s stated rule.
   - **`d` is defined as a CSS property by SVG 2.** Chrome’s implementation is not what makes it a property. An attribute-only, non-animatable subset is defensible, but must be described as a limitation. CSS `d: path("…")` and the `d="…"` attribute also have different value syntax.
   - The blanket acceptance of a **`px` suffix on coordinates** cannot apply to path data, `points`, or `viewBox`: those grammars contain numbers, not CSS lengths. Basic-shape length attributes and CSS geometry properties need their own parsing and serialization rules.
   - Specify `viewBox`’s comma/whitespace grammar, four-number requirement, negative-size errors, and zero-size rendering suppression. Specify the initial `preserveAspectRatio` value, **`xMidYMid meet`**, and its behavior without a usable `viewBox`.
   - Rectangles need SVG’s `rx`/`ry` **`auto` behavior**, copying the specified radius to the unspecified one, and limiting radii to half the corresponding dimension. Their `width`/`height: auto` behavior must not become ordinary container auto-sizing.
   - The full path grammar includes implicit command repetition, additional `moveto` pairs becoming lines, compact number separators, reflected control points, and special arc-flag syntax. Arc handling must include radius correction, zero-radius lines, coincident endpoints, and negative radii handled by the arc rules.
   - “Last good segment” must mean the last complete segment, including complete repetitions within a command. Dropping an odd trailing `points` coordinate is correct.

   D1’s statement about SVG transform origins is also too broad: the outer SVG participating in CSS layout has different default-origin treatment from ordinary SVG descendants. That distinction still matters if transforms remain refused.

2. **D2 — Agree with presentation properties as inherited style rows; the schema table needs domain rules and complete lowering.**

   The listed initial values and inheritance flags are substantially correct: black fill, no stroke, unit stroke width, butt caps, miter joins, miter limit 4, nonzero fill rule, zero dash offset, and non-inherited circle geometry. Group `opacity` must composite the group once; it is not inherited paint opacity.

   Required details:

   - Reject negative stroke widths, radii, and dash-array entries according to their property grammars. A negative dash entry invalidates the declaration; it is not clamped individually.
   - `stroke-miterlimit` must be at least 1.
   - Odd dash lists repeat correctly; **an all-zero list acts as `none`**. Define comma/whitespace parsing and dash-offset normalization, including negative offsets.
   - Follow CSS clamping for opacity properties. A generic finite `f32` codec supplies neither these domains nor the promised `px` input handling. The existing numeric conversion generally accepts finite numbers, with specific exceptions implemented separately. [kernel/src/style.rs:419](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/style.rs:419)
   - Preserve `currentColor` until it can be resolved against the element’s computed color. Resolve inherited paint through logical ancestry, including ancestors outside the SVG. The existing computed-style machinery supports that ancestry; reading the shape’s own `style` alone does not. [kernel/src/kernel.rs:95](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/kernel.rs:95)

   There is a concrete web-lowering trap: the generic numeric serializer appends `px` except for an explicit list. Without additions, the proposed rows would produce invalid declarations such as `fill-opacity:0.5px`, `stroke-opacity:1px`, and `stroke-miterlimit:4px`. Conversely, geometry CSS and keyframe values must get appropriate length serialization rather than blindly copying attribute text. [host/web/src/css.rs:288](/Users/ccheever/projects/exact2-wt-svg-review/host/web/src/css.rs:288)

   The `CssValue` generator mechanism does exist. It parses text into typed values and serializes `.css()`, however; “exactly what was authored” should mean equivalent CSS, not preservation of the original text. [kernel/build/codec.rs:195](/Users/ccheever/projects/exact2-wt-svg-review/kernel/build/codec.rs:195)

3. **D3 — Agree conditionally with logical nodes per element; disagree that the implementation is essentially a `sync_children` exception.**

   I would retain element identity in the kernel for this proposal. It fits bindings, keyed lifetime, inheritance, inspection, and animation targets. The outer SVG should own the only CSS layout box.

   The alternative is understated, though. A replaced node can contain a retained scene with stable element IDs and incremental geometry updates; it need not rewrite a monolithic display list for every point change. Real DOM elements also do not require corresponding kernel nodes. D4 already chooses a whole-scene transport to Apple, so the native transport advantage is not inherent in D3.

   The node choice requires an explicit contract for nodes without boxes:

   - **Allocation and rebuild:** creation currently allocates a Taffy leaf for every node. `sync_children` only disconnects text children from their parent’s layout children; it does not eliminate their allocations. Full rebuild independently allocates every live node and skips only text parents when connecting children. Changing just `sync_children` leaves these paths inconsistent. [kernel/src/txn.rs:483](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/txn.rs:483), [kernel/src/layout.rs:173](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/layout.rs:173), [kernel/src/layout.rs:891](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/layout.rs:891)
   - **Publication and export:** publication special-cases inline text, otherwise reads a Taffy layout; a missing Taffy node causes an early `continue`. Export provides a frame for every row and distinguishes inline text, but has no general “no independent box” representation. SVG descendants need defined geometry absence or SVG bounds, dirty-flag consumption, and traversal behavior—not accidental zero or stale frames. [kernel/src/layout/publication.rs:69](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/layout/publication.rs:69), [kernel/src/export.rs:89](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/export.rs:89)
   - **Hit testing:** Linux hits painted boxes; UIKit walks views and their bounds. Neither implements SVG’s default painted-fill/stroke hit semantics. A circle’s bounding rectangle is not its hit region, and `fill="none"` matters. CALayers do not supply UIView event targets. Either implement SVG geometry hit testing and event routing, or explicitly restrict descendants to decorative content and consistently refuse their handlers/focus semantics on every host. [host/linux/src/paint.rs:798](/Users/ccheever/projects/exact2-wt-svg-review/host/linux/src/paint.rs:798), [host/linux/src/presenter.rs:1220](/Users/ccheever/projects/exact2-wt-svg-review/host/linux/src/presenter.rs:1220), [host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:788](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:788)
   - **Accessibility and inspection:** decide whether the SVG is one accessible image or supports accessible descendants. Shape IDs without native views currently cannot participate through the existing presenter lookup. Native geometry reporting must distinguish layout boxes from SVG geometric bounds. [host/apple/Sources/ExactKit/Accessibility.swift:192](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/Accessibility.swift:192)
   - **Apple projection:** create, update, children, frame, and presentation ops currently assume ordinary native views except for explicitly handled projections such as inline text. Suppressing shape `create` ops alone leaves children and presentation ops addressing absent views. The owning SVG must absorb the complete descendant lifecycle and route shape presentation updates. [host/apple/src/paragraph.rs:155](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/src/paragraph.rs:155), [host/apple/src/paragraph.rs:249](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/src/paragraph.rs:249), [host/apple/src/layout.rs:123](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/src/layout.rs:123), [host/apple/src/host.rs:1160](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/src/host.rs:1160)
   - **Regions:** the alternate region tree builds its own layout nodes and skips only text when connecting children. Its geometry walk directly indexes those nodes. It must share the SVG exclusion rule rather than independently rediscover it. [kernel/src/region/tree.rs:28](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/region/tree.rs:28), [kernel/src/region/tree.rs:152](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/region/tree.rs:152)
   - **Collection validation:** the existing validator checks the row root’s authored style bindings, not every descendant’s geometry. SVG children therefore need not break row validation, provided the outer SVG remains in ordinary flow. But a new transform animation on the row root can bypass the existing static zero-transform restrictions. Validate that animation case too. [runner/src/instance/collection/views.rs:136](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/views.rs:136), [contract/lower/src/collection.rs:109](/Users/ccheever/projects/exact2-wt-svg-review/contract/lower/src/collection.rs:109)

   **Sizing is another missing part.** A 300×150 default does not describe all SVG replaced-element sizing. `viewBox` can supply an intrinsic ratio, which affects auto dimensions and constraints. Today only image and video are classified as replaced; intrinsic-ratio lowering reads the arena’s intrinsic dimensions. SVG needs its own integration and invalidation when `viewBox` changes. [kernel/src/node.rs:33](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/node.rs:33), [kernel/src/layout.rs:757](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/layout.rs:757), [kernel/src/style.rs:1223](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/style.rs:1223)

   The declared `display:block` deviation is reasonable. Default clipping matches inline SVG’s UA treatment; it should not be presented as an inherent SVG initial-value rule. Both must apply consistently outside Contract’s tag defaults too.

   Finally, “noise” is unmeasured. The arena stores full `StyleProps` per node, so adding rows also enlarges ordinary nodes, and resolved keyframe payloads can add copying costs. [kernel/src/arena.rs:28](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/arena.rs:28)

4. **D4 — Agree with the renderer choices; require complete scene invalidation, web adoption, and capture support.**

   Real SVG DOM, CAShapeLayer, and Linux path painting are appropriate.

   On the web, namespace creation and exact attribute names are essential. Current creation uses `document.createElement`, while unrecognized props become lowercase `data-*` attributes. `viewBox`, `preserveAspectRatio`, and `pathLength` must not fall through that path. The static-document and adoption paths need the same SVG semantics as live creation. [host/web/glue.js:556](/Users/ccheever/projects/exact2-wt-svg-review/host/web/glue.js:556), [host/web/src/element.rs:289](/Users/ccheever/projects/exact2-wt-svg-review/host/web/src/element.rs:289)

   The actual page stylesheet matters: it currently makes images and videos block-level but not SVG, and applies `position:relative` to every descendant. Root host CSS can also append `display:flow-root`. SVG defaults and applicable properties must be checked in that page, not only in an isolated SVG fixture. [host/web/index.html:66](/Users/ccheever/projects/exact2-wt-svg-review/host/web/index.html:66), [host/web/src/element.rs:28](/Users/ccheever/projects/exact2-wt-svg-review/host/web/src/element.rs:28)

   “Parsed once in Rust for every host” conflicts with “geometry props as attributes” on the web. Chrome will parse the authored path and render its arcs. Choose explicitly between original browser geometry and canonicalized geometry; retain an independent comparison against the original SVG. Cubic arc approximation needs a stated tolerance, especially for path length and dashing.

   A scene must be invalidated by inherited paint/color changes, topology changes, viewport-size changes, `viewBox` changes, and presentation changes—not merely direct prop writes to the SVG. Group opacity, fill/stroke opacity, fill-before-stroke ordering, viewport clipping, and non-uniform viewBox scaling all need consistent treatment.

   Two existing capture paths would otherwise lose the feature:

   - Linux retained-region payloads support only empty, image, and text content; other node types become empty. Ordinary painter support alone therefore does not add SVG to retained regions. [host/linux/src/paint/region.rs:84](/Users/ccheever/projects/exact2-wt-svg-review/host/linux/src/paint/region.rs:84), [host/linux/src/paint/region.rs:247](/Users/ccheever/projects/exact2-wt-svg-review/host/linux/src/paint/region.rs:247)
   - iOS’s shadow capture copies **model layers**, not their animated presentation. Its CAShapeLayer copy omits both `lineDashPhase` and `miterLimit`, and does not transfer animations. The CPU fallback also renders the model layer tree. A correct live sparkline could consequently be wrong inside a captured canvas. [host/apple/Sources/ExactKit/IOS/Shadow.swift:202](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/Shadow.swift:202), [host/apple/Sources/ExactKit/IOS/Shadow.swift:259](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/Shadow.swift:259), [host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1050](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1050)

   Support these routes or explicitly refuse the unsupported combinations; silent disappearance is not an acceptable subset.

5. **D5 — Disagree as written. Several central CSS Animations semantics are incorrect or underspecified.**

   The style-row representation is appropriate, but CSS Animations Level 1 and the Web Animations timing model require the following corrections:

   - **Unknown names do not start a timing-only CSS animation.** An animation cannot start until its matching keyframes rule resolves. Installing the rule later must establish the corresponding start semantics.
   - **Matching “by name” is insufficient.** Process the new animation-name list from the end, matching the last unmatched old animation of that name. Duplicate names are separate instances. Reordering changes composite order without necessarily restarting matched instances.
   - **Start is not raw node allocation.** It depends on applicable resolved animation style on a rendered element. Applying an animation to an existing element can start it. `display:none` on an element or ancestor cancels CSS animations; becoming displayed starts them again. Offscreen position and `visibility:hidden` do not mean the same thing.
   - **Retiming preserves animation identity and playback history, not normalized progress.** Changing duration or delay can jump the current phase, end an animation, or make a previously finished animation active again. Finished instances cannot simply be forgotten while their names remain applied.
   - **Remove/add requires an observable style change.** Removing and restoring a name before the browser observes the intermediate state does not necessarily restart it. Native per-op behavior must agree with the web’s committed style-change boundary.
   - **Iteration progress needs the full boundary algorithm.** `(t − delay) / duration` is overall progress, not final iteration progress. Define modulo iteration progress, direction parity, exact integer endpoints, fractional counts, zero iterations, zero duration, and negative delays beyond the active duration.
   - **Fill values depend on direction and the actual terminal progress.** Backwards fill is not invariably the 0% value; forwards fill is not invariably the 100% value. Fractional iteration counts can finish between keyframes. Filling must not overwrite the underlying authored value.
   - **Build property-specific keyframe tracks.** A keyframe omitting opacity does not insert an underlying-opacity value at that offset. Duplicate offsets cascade declarations; missing endpoints are synthesized per property. A keyframe’s easing governs its outgoing interval for that property, and easing on the terminal keyframe has no following interval to affect.
   - **Implicit endpoints are live underlying values.** They are not a one-time snapshot taken at animation creation. Underlying style changes and lower-composite-order effects can change them while the animation runs or fills.
   - **Pause freezes local time, including delay.** Resume retains that time without including time spent paused. The negative-delay frozen benchmark is valid: at creation it samples the phase implied by the negative delay.
   - **The transition-precedence statement is reversed.** Active CSS transitions have higher cascade precedence than CSS animations. Do not treat a running transition as a simple lower-priority value that animations always cover. Conversely, animation samples must not continuously generate new transitions; CSS’s before/after-change style rules still apply.
   - **Multiple animations need an effect stack.** When several contributing animations affect a property, list/composite order matters. An animation outside its active interval with no applicable fill contributes nothing, exposing a lower effect.

   The current engine has one transition slot per `(node, property)` and removes completed running curves. It does not already provide animation-instance identity, duplicate-name matching, or an effect stack. This is a substantive extension. [motion/src/engine.rs:143](/Users/ccheever/projects/exact2-wt-svg-review/motion/src/engine.rs:143), [motion/src/engine.rs:334](/Users/ccheever/projects/exact2-wt-svg-review/motion/src/engine.rs:334)

   The current kernel seam only processes created/touched nodes and reads their own style targets. Ancestor display changes and animated inherited values therefore need additional reconciliation. [kernel/src/motion.rs:226](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/motion.rs:226)

   The easing implementation also explicitly evaluates steps with the **before flag unset**. Animations need the correct side of discontinuities during fill and reverse playback. Existing `linear()` code holds endpoint stops outside their range; CSS easing requires checking extrapolation and coincident-stop behavior too. Reusing it unchanged does not establish animation parity. [motion/src/easing.rs:236](/Users/ccheever/projects/exact2-wt-svg-review/motion/src/easing.rs:236)

   Finally, CSS list repetition/truncation follows the animation-name list. A shorter duration list cycles; `none` still occupies a list position. “Longhands always override shorthand regardless of attribute order” can be a Contract authoring rule, but it is not CSS declaration-order semantics and should be identified accordingly.

6. **D6 — Agree with the small numeric animation surface, with applicability restrictions.**

   Adding `stroke-dashoffset` and `r` without relayout is appropriate. The proposed property discriminants 5 and 6 follow the existing `Height = 4`; there is no enum collision. [motion/src/property.rs:17](/Users/ccheever/projects/exact2-wt-svg-review/motion/src/property.rs:17)

   Required clarifications:

   - D1 refuses SVG transforms while D6 admits transform keyframes. Specify target applicability and refuse transform animations/transitions on SVG elements where transforms are unsupported, including dynamically resolved animation names.
   - Radius interpolation must respect its nonnegative domain, including easing overshoot and zero-radius rendering.
   - `stroke-dashoffset` inherits. Native presentation must account for inherited animated values where applicable, not only authored inheritance.
   - New transition properties must reach SVG scene presentation on Apple; their current `.present` handling recognizes only translate, scale, rotate, and opacity. [host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:681](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:681)

   Omitting color animation is a reasonable scope choice, but “color interpolation needs text re-rendering” is not a general explanation for refusing SVG fill/stroke color animation. Those are shape paint properties. The price-flash consumer already exists in the benchmark; §7 correctly records that this proposal does not deliver complete benchmark visual parity.

7. **D7 — Agree with a CA executor in principle; reject the proposed mapping as sufficient for the admitted semantics.**

   The benchmark’s simple explicit-endpoint animations are plausible CA candidates. The complete D5 surface is substantially harder.

   - **Key times and timing functions:** generate each property’s own offsets and values. Set the overall animation timing explicitly so it does not warp the entire sequence in addition to per-segment easing. D7 correctly distinguishes CSS `ease` from CA’s default curve.
   - **Reverse:** reverse offsets, values, interval association, and easing direction. Swapping values alone is wrong for asymmetric easing. Reversing a cubic interval requires the time-reversed curve; an ease-in forward segment becomes ease-out when traversed backwards. `alternate-reverse` also needs an explicit lowering.
   - **`steps()` cannot be faithfully converted to ordinary sampled linear interpolation.** Sampling introduces ramps where CSS has jumps. `linear()` should preserve its actual breakpoints, including coincident positions, rather than use a generic sampling grid. Exact boundary behavior needs a representable lowering or a declared fallback/refusal.
   - **Autoreverse/count conversion is not a complete algorithm.** `iterations / 2` can describe positive alternate cycles when CA’s cycle accounting is correctly configured, but zero iterations cannot be encoded as ordinary CA repeat count zero, which means no repetition rather than no initial execution. Fractional counts, zero duration, terminal direction, and forwards fill require explicit handling.
   - **Fill modes are not sufficient by themselves.** CA’s retained presentation and CSS’s participation in an effect stack are different concerns. Underlying changes, cancellation, competing animations, and transitions must reveal the correct current lower value.
   - **`speed = 0` and `timeOffset` describe a paused sample, not the entire pause/resume protocol.** Convert between session time, layer-local time, animation delay, and held local time. Rebase begin time on resume. A paused animation before a positive delay must remain before that delay. A negative-delay animation must immediately show its advanced phase.
   - **Radius-to-path interpolation is conditional.** It works for circles represented by identical command topology whose control points vary linearly with radius. It is not guaranteed for arbitrary ellipse-building output, optimized zero-radius paths, or out-of-range radius samples. “Exactly” also ignores the circle approximation used by the path representation.
   - **Dash scaling is directionally correct for a static path:** native dash lengths and phase use `actual_length / pathLength`, in user coordinates. Do not scale stroke width, geometry, or the web values again. Handle absent, zero, negative, and degenerate path lengths explicitly.
   - **Animated radius plus normalized dashes exposes a missing dependency.** A circle’s actual length changes during an `r` animation. Its calibrated dash pattern and phase must change with that length. A once-scaled independent `lineDashPhase` animation cannot generally reproduce this combination.
   - **Composing individual transforms into matrix keyframes is not faithful.** A CSS `rotate` from 0 to 360 degrees has identical endpoint matrices but must perform a full turn. Independent transform properties can also have different timings and competing effects. A single matrix animation per property either conflicts on the same key path or loses those semantics.
   - **CA animations and sampled transitions need one presentation owner per property.** Current transition presentation writes UIView transform components and alpha. Those model writes do not automatically override an explicit CA animation as CSS transition precedence requires. [host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:681](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:681), [host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1251](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1251)

   “No display-link wakeups for the pulse” is a good target, but requires separating **needs frame sampling**, **has animation state**, and **has a finite settling deadline**. Currently Apple’s `batch.motion` comes directly from `!engine.quiescent()`, Swift uses it to tick, and the engine derives settlement from the same running set. Removing lowered animations from that set alone would lose other responsibilities. [host/apple/src/host.rs:1010](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/src/host.rs:1010), [host/apple/Sources/ExactKit/Session.swift:946](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/Session.swift:946), [motion/src/engine.rs:421](/Users/ccheever/projects/exact2-wt-svg-review/motion/src/engine.rs:421)

   Paused-layer presentation checks must include intermediate times and both sides of discontinuities, not only keyframe times. Transform hit testing, capture, and screenshot paths also need presentation-state coverage.

   I would initially lower only cases whose equivalence is established and use the existing evaluator for the remaining admitted cases, rather than claim this mapping covers all of D5.

8. **D8 — Agree with restart after actual destruction/remount; the current lifecycle description is too broad.**

   Collection rows receive fresh wrappers when constructed, and motion identity includes the kernel generation. Those are sound foundations for restarting after destruction. [runner/src/instance/collection/views.rs:23](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/views.rs:23), [kernel/src/motion.rs:26](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/motion.rs:26)

   But rows are not always destroyed immediately upon leaving the window:

   - Existing keyed rows are retained and updated.
   - Focus/interaction pins keep rows mounted.
   - Limited reports retain some rows outside the target window while retirement catches up.

   Such rows can leave and return without restarting. That is consistent with mount-based semantics, but contradicts §1’s simplified destruction claim. [runner/src/instance/collection/mod.rs:473](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/mod.rs:473), [runner/src/instance/collection/mod.rs:528](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/mod.rs:528), [runner/src/instance/collection/mod.rs:561](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/mod.rs:561)

   On iOS, **SVG rows are currently ineligible for pooling**: allowed kinds omit `svg`, and both retired-tree and new-tree shape matching use that allowlist. Extending reset alone does not enable reuse. [host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:56](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:56), [host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:113](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:113), [host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:187](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:187)

   If reuse is admitted, reset must cancel animations throughout the SVG layer subtree, discard scene-ID mappings, and reset timing and presentation state **when parking**, not only when taking a view again. Hidden parked views remain in the native tree. Current recycle/rebind does not remove CA animations or reset layer timing. [host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:130](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:130), [host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:208](/Users/ccheever/projects/exact2-wt-svg-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:208)

   Preserving pulse phase during data updates is correct. Re-lowering a changed-length path must preserve matched animation identity, current local time, pause state, direction, and iteration state—not merely its original begin time.

9. **D9 — Agree with mount-based animation lifetime; qualify the fill-policy claims.**

   CSS animations do not wait for intersection with the viewport. Starting overscan animations early is therefore appropriate, and fill policy should not introduce a second visibility-triggered animation system.

   Three corrections are required:

   - `complete` does **not** unconditionally promise every visible row is already built. LLP 1050.000 explicitly has the D3 overload exception, a bounded first-mount bootstrap, and a web user-scroll deviation. [llp/1050.000-choosing-the-fill-tradeoff.rfc.md:72](/Users/ccheever/projects/exact2-wt-svg-review/llp/1050.000-choosing-the-fill-tradeoff.rfc.md:72), [llp/1050.000-choosing-the-fill-tradeoff.rfc.md:119](/Users/ccheever/projects/exact2-wt-svg-review/llp/1050.000-choosing-the-fill-tradeoff.rfc.md:119)
   - Being built before paint does not guarantee phase zero at first pixel. Specify the common animation activation timestamp relative to a committed, attached presentation. Kernel allocation, DOM insertion/style resolution, and CA installation are different moments.
   - D3’s expensive-row deferral is a policy obligation, not something established by the current realization code: that code puts all visible rows in the owed set. Distinguish implemented behavior from planned fill-policy stages. [runner/src/instance/collection/mod.rs:490](/Users/ccheever/projects/exact2-wt-svg-review/runner/src/instance/collection/mod.rs:490)

   “Each time mounted” is a defensible interpretation of the benchmark requirement, but it is a deliberate qualification of “each time on screen,” not an equivalence. Pins and deferred retirement add differences beyond overscan.

   Also, SVG work changes row cost and can affect whether the overload policy defers a row. The accurate statement is that animations do not change the **policy**, not that adding them cannot change when rows are built.

10. **D10 — Agree with excluding infinite iterations from settlement; include paused animations and preserve timing state.**

    The current web implementation takes every animation’s `endTime` into the settle maximum and calls `finish()` once a seek reaches it. D10 correctly identifies the infinity failure. [host/web/glue.js:1084](/Users/ccheever/projects/exact2-wt-svg-review/host/web/glue.js:1084)

    Merely filtering infinity is insufficient:

    - An author-paused finite animation does not become finished because the agent clock advances.
    - The agent’s own pause used for deterministic seeking must remain distinct from authored `animation-play-state:paused`.
    - Registration currently records one timestamp per animation object; seeking then overwrites current time with `to − start`. That is insufficient after authored pause/resume or adoption of an already-progressing animation.
    - `finish()` is not a general substitute for seeking: it changes playback state and disregards the intended held local time.
    - Finite lowered CA effects must still contribute settling deadlines even when they do not request display-link ticks.
    - “Settled” should mean finite runnable work has settled; an infinite decorative animation still exists and can change on a later seek.

    For the frozen benchmark, the expected ring sample is unambiguous: local time zero with delay −300 ms and duration 1200 ms yields directed progress 0.25. Thus `r = 3 + 6 × ease-out(0.25)` and opacity `= 0.5 × (1 − ease-out(0.25))`, remaining fixed until resumed.

11. **D11 — Agree with producer-owned reduced-motion policy; this does not establish the preference plumbing.**

    Keeping the policy outside the engine matches LLP 1002 and NOT-DOING. Switching to `animation:none` should cancel effects and reveal the authored static values.

    The RFC should name the actual preference input and update path. I found the policy statements, but no reduced-motion reporting implementation in the inspected kernel, motion, runner, Contract, and host paths. Treat that as an integration requirement or explicitly unimplemented behavior, rather than implying the producer can already receive it.

12. **D12 — Agree with named refusals; correct their classification and enforce them beyond literal authoring.**

    The exclusions keep the proposal bounded. Stable diagnostic IDs can use the existing error mechanism; they do not justify a new refusal registry.

    Two classification corrections:

    - Animation events are part of CSS Animations Level 1. Refusing them makes this a subset of Level 1; they are not “outside” it.
    - CSS ignores `!important` declarations inside keyframes. Compile-time refusal is an intentional stricter authoring rule.

    Refusals must cover computed animation names, structured kernel operations, plan/wire decoding, and applicable target types. Compile-time checking alone cannot protect dynamic values or non-Contract producers. The existing transition decoder’s validate-before-apply behavior is the appropriate precedent. [kernel/src/wire/codec.rs:278](/Users/ccheever/projects/exact2-wt-svg-review/kernel/src/wire/codec.rs:278)

**The §9 take is not substantive.** The dead `svgSource` claim is supported: the schema declares it, web fallback would expose it as inert data, and there is no corresponding rendering consumer in the inspected paths. Deleting it is good cleanup. It does not retire an implemented feature or active commitment, and the purported “Svg-as-markup path” supplies no functioning path to remove. [kernel/tables/schema.json:107](/Users/ccheever/projects/exact2-wt-svg-review/kernel/tables/schema.json:107), [host/web/src/element.rs:98](/Users/ccheever/projects/exact2-wt-svg-review/host/web/src/element.rs:98)

The binding rule requires taking something off the doing-list in the same PR. Keeping already-refused decay, sequence, color motion, and SMIL off the list is not another take. The proposal also needs to admit the new benchmark consumer against the existing restriction on additional benchmark/showcase apps. [rules/NOT-DOING.md:136](/Users/ccheever/projects/exact2-wt-svg-review/rules/NOT-DOING.md:136), [rules/NOT-DOING.md:338](/Users/ccheever/projects/exact2-wt-svg-review/rules/NOT-DOING.md:338)

The remaining rules fit is mixed:

- An implementer and date are present, so this is **not ownerless speculative specification** merely because it is a draft.
- Unit fixtures and host tests can fit existing checks. Nothing in §8 inherently requires a sixth check. Live Chrome, pixel comparisons, and XCTests must remain in the existing asynchronous arrangement when they exceed the blocking budget; the current parity recorder explicitly follows that pattern. [host/web/parity.mjs:1](/Users/ccheever/projects/exact2-wt-svg-review/host/web/parity.mjs:1)
- The separate `apps/sparkline` fixture needs explicit scope approval. It should not silently become another benchmark app or harness.
- New scripts, checks, registries, or governance documents require the explicit human approval specified by the rules. The audit request does not provide that implementation authorization. [rules/RULES.md:64](/Users/ccheever/projects/exact2-wt-svg-review/rules/RULES.md:64)
- Implementation must account for the 1,500-line cap: `glue.js` already has 1,493 lines and Linux `paint.rs` has 1,471. Small extensions will require ordinary source-file decomposition, not an exception.

**Verdict: build with named changes.** I would require these, ranked:

1. Correct D5’s transition precedence, animation matching/start rules, effect stacking, implicit endpoints, and timing boundaries.
2. Replace D7’s blanket CA mapping with proven lowerings plus an explicit fallback or narrower admitted surface.
3. Define and implement the complete contract for SVG nodes without boxes: allocation, rebuild, export, host projection, input, accessibility, and regions.
4. Correct SVG property classification, value domains, intrinsic sizing, path-length behavior, and web CSS/namespace lowering.
5. Make clock seeking, authored pause, actual remount lifetime, and iOS pooling agree across hosts.
6. Cover retained-region and canvas-capture paths; add parity cases at intermediate times and discontinuity boundaries.
7. Obtain a real NOT-DOING trade and explicit approval for the new consumer/fixture, while keeping verification within the existing five-check and async structure.
