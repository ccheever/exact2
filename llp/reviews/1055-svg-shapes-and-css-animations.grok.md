# Review: LLP 1055 r1, 2026-09-26 (grok)

- **Family:** xAI — `grok` CLI, `--reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`; served as `grok-4.6-build` per its usage record (accepted); `--cwd` the same detached review worktree at d357a7c4; 2026-09-26T22:57:58Z to 2026-09-26T23:12:52Z; stopReason end_turn.
- **Method:** the shared brief (sha256 `c6185a771ae30f6b2ffaddea8069578336b3632d80bb640dbd7336b708d9fad5`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1055 r1.
- **Redacted:** nothing.
- **Disposition:** LLP 1055 §0 (r2).

---

## Review (verbatim)

I'll audit from the checkout only: binding rules first, then the RFC, then the kernel/motion/host code those decisions rest on. No skills, no builds, no edits.Binding docs and the RFC are in. Next I’ll locate 1002/1003/1050 and the kernel/motion/host files the decisions cite.HEAD is the RFC on `0fd40388`. I’ll read the motion/fill LLPs and the kernel seams those claims depend on.I’ll pull schema, layout exclusion, motion, tags, collection, and host seams next so D1–D12 can be checked against the code.D3’s layout-skip and Height’s wire id look like real conflicts. I’ll confirm those against txn, region, paint, CSS, and the collection.# Independent audit of LLP 1055 r1 (`llp/1055-svg-shapes-and-css-animations.rfc.md`)

**Scope.** Binding rules (`CLAUDE.md`, `rules/RULES.md`, `rules/NOT-DOING.md`), the Draft r1 RFC (commit `d357a7c4` on `origin/main` `0fd40388`; the commit is the RFC file only), LLP 1002 / 1003 / 1050.000, and the in-tree kernel, motion, contract, collection, and host seams those decisions rest on. Standards used: SVG 2, CSS Animations Level 1 (as implemented via Web Animations / CSS Animations 2 in Chrome), CSS Easing, CSS Transitions. No builds, no tests, no dependency sources outside this checkout.

**Bottom line.** The direction is right: CSS names, two executors, browser as oracle, nodes as animation targets, Core Animation only for repeating compositor work. It is not specified tightly enough to implement without inventing the hard parts. Several claims about today’s code are true; several are stale or incomplete; the CSS/CA timing story is the largest semantic hole.

---

## Claims in §1 about today’s code

These hold:

- `NodeType::Svg` is id 7 and `svgSource` is prop id 8 (`kernel/tables/schema.json:33–40`, `:107–110`). Contract has no `svg` tag (`contract/lower/src/tags.rs`).
- Web maps `NodeType::Svg` to `"div"` (`host/web/src/element.rs:98`). Unknown props become `data-<name>` (`element.rs:290–293`), so a set `svgSource` would be `data-svgsource`. Nothing in Rust/Swift/JS reads `svgSource`.
- Apple `kind_for` maps Svg to `"svg"` (`host/apple/src/host.rs:1256`) and then builds a normal node view (`host/apple/src/paragraph.rs:155–193`). Linux’s paint walk has no Svg arm; every node gets box paint (background/border) then a type match (`host/linux/src/paint.rs:830–981`). Svg is a coloured box.
- Motion is four compositor rows plus Height; the transition wire is `0 all, 1–4 compositor, Height as Property=4 → wire 5` (`motion/src/property.rs:17–28`; `kernel/src/wire/codec.rs:289–291`, `:572–573`). Schema’s `_transitions` comment still omits Height (`schema.json:1632`).
- Collection creates with `u.ids.fresh()` (`runner/src/instance.rs:648`) and destroys the wrapper (kernel DestroyView is subtree-inclusive: `kernel/src/txn.rs:46–47`, `:501–511`). iOS pools UIViews after destroy (`NodePoolIOS.swift:1–37`, `:132–148`); kinds are `view/text/image/button/scroll` (`:56`).
- Apple/Linux sample `exact-motion` while `!engine.quiescent()` (`host/apple/src/host.rs:1013`; `Session.swift:968–970`). Web emits CSS and uses the engine only for springs (`host/web/src/motion.rs:1–17`).

These are overstated or unfinished:

- “Virtualized rows are new kernel nodes” is true; “the same rule falls out on every host” is not automatic on iOS once SVG is pooled (D8).
- D9 treats LLP 1050.000’s `complete` / overscan / D3 as the collection. That RFC’s implementer is unassigned; there is no `fill-policy` in the tree. What exists is a geometric window with one viewport of overscan each side and a velocity lead (`runner/src/instance/collection/mod.rs:38–50`, `:457–590`).

---

## D1 — subset (seven/eight elements, SVG 2 names)

**Agree, with corrections.** A sparkline needs `svg`, `polyline`, `circle`, paint/stroke, `pathLength`, and `overflow`. Dropping `ellipse` and SVG `transform` (because `transform-origin` defaults to `0 0` of the view box) is the right refusal.

What is wrong or understated:

1. **The prop/row split is not “as SVG 2 defines it.”** SVG 2 geometry properties include `x`, `y`, `rx`, `ry`, `cx`, `cy`, `r`, `width`, `height`, and `d`. Chrome follows that. The RFC puts `x/y/rx/ry/d` in the “CSS cannot set” bucket and calls `d` “Chrome only.” That is false. A v1 *refusal to animate* those is fine; the justification is not.
2. **Costs say “7 node types” for 8 elements** (`svg g path polyline polygon circle line rect`). Either reuse `Svg` and add 7, or count 8; the table and §4 disagree.
3. **`rect` `width`/`height` are the existing layout rows** (`layout: true` in the schema). A rect excluded from Taffy still dirties layout if those rows change. `ellipse` does not need “two more rows” if `rx`/`ry` are already props.
4. **`viewBox` / `preserveAspectRatio` grammar is incomplete.** Four numbers is the happy path; negative `width`/`height` is invalid; default `preserveAspectRatio` is `xMidYMid meet`, which the table never states. With authored `width=96 height=32` it does not matter; with one axis omitted, SVG’s intrinsic ratio comes from `viewBox`. The RFC never gives `svg` that intrinsic ratio (HTML/CSS does).
5. **Arcs → cubics vs the Chrome oracle.** §8 wants `getTotalLength` parity with Chrome. Cubiced arcs will not match Chrome length or dash distance. Polyline sparklines do not care; the test promise is then false for the grammar they claim to implement.
6. **`pathLength` 0 / negative** is invalid in SVG 2; not named.
7. **Coordinates: unitless + `px`.** Presentation attributes allow unitless user units. CSS geometry/`stroke-width` generally need lengths. The web emitter’s default for unknown `f32` rows is to append `px` (`host/web/src/css.rs:288–308`). That is correct for `r`/`stroke-width`/`cx` and **wrong** for `fill-opacity`, `stroke-opacity`, and `stroke-miterlimit` (they would emit `fill-opacity:1px`, which Chrome drops). D2’s codec table does not classify unitless vs length.

**Intrinsic size analogy is internally inconsistent.** HTML `<svg>` is a replaced 300×150. In this tree, `<video>` is `is_replaced()` with a 300×150 measure fallback (`kernel/src/node.rs:34–36`; `layout.rs:757–764`); `<canvas>` is *not* replaced and the tag writes `width`/`height` 300/150 (`node.rs:77–79`; `tags.rs:137–142`). Svg is in neither set today. Pick one and add viewBox aspect.

---

## D2 — presentation properties as CSS rows

**Agree on inheritance and the CSS-row-vs-attribute idea; the codec story is under-specified.**

SVG initials in the table (`fill` black, `stroke` none, `stroke-width` 1, `linecap` butt, `linejoin` miter, `miterlimit` 4, `dasharray` none, `dashoffset` 0, opacities 1, `fill-rule` nonzero, `cx/cy/r` 0) match SVG 2. `opacity` is already a non-inherited row (`schema.json:1289–1294`); using it as group opacity on `g` is CSS. `cx/cy/r` must **not** be `layout: true` or a live `r` pulse relayouts the list.

`paint` / `dasharray` as “CssValue like `clip-path`” is slightly wrong: `clip-path` is a parsed `ClipPath` re-emitted as canonical CSS (`kernel/src/clip.rs`; `css.rs:279`), not opaque text. New `RowValue` arms are required or `css.rs:142–145` will skip the row (“no CSS lowering for this row’s codec”).

Odd-length `stroke-dasharray` is repeated to even — correct. **Negatives are invalid** (declaration dropped); the RFC is silent. Negative `stroke-dashoffset` is allowed and wraps.

`currentcolor` must resolve against kernel `text_color` (emitted as CSS `color`, `css.rs:230`). Native must do the same; the RFC does not say so.

---

## D3 — one kernel node per SVG element, subtree out of box layout

**Agree that kernel nodes are the right target model. Do not build “no layout” as written.**

Why nodes, not one replaced display list:

- `exact-motion` keys `(node, property)` (`motion/src/engine.rs:26–38`, `:146`). CSS animates *elements*. The sparkline’s `stroke-dashoffset` and `r`/`opacity` are different elements. A display list would need a second targeting graph, which NOT-DOING forbids.
- The web oracle needs real SVG DOM nodes (`host/web/glue.js:558` creates one element per kernel create). Unpacking a display list on web only is a third representation.
- Inheritance of `fill`/`stroke` already walks kernel ancestors (`schema.json` `_styles` inherited rule). A flattened list would bake paint.
- Four extra nodes per visible row is cheap next to the collection’s wrapper (`views.rs:23–58`).

A hybrid (nodes only for animated shapes) is not worth it for this subset: almost every sparkline child is animated.

What “outside box layout” actually hits in this kernel:

| Seam | What the code does today | What D3 implies |
|---|---|---|
| Create | Every `CreateView` gets a Taffy leaf (`txn.rs:483–485`) | Must stop creating leaves, or leave orphans |
| `can_hold_children` | Svg **cannot** hold children (`node.rs:10–20`) | Must allow `Svg`/`SvgGroup` only; shapes stay leaves. New ApplyError for “SVG under non-SVG” does not exist (`txn.rs:330–337` is only `LeafCannotHoldChildren`) |
| `sync_children` | Text gets no layout children (`layout.rs:173–176`; `txn.rs:712–714`) | Same for `Svg` — this part is the right analog |
| Publication | No Taffy → `continue`, **no frame, no descent into groups** (`layout/publication.rs:72–77`). Inline runs instead get `Frame::default()` and still descend (`:72–74`, `:110–113`) | `continue` breaks `g > circle`. The Text analog is dummy frames + descend, not skip |
| Region/hypothetical | Derived tree copies **all** non-Text children into Taffy (`region/tree.rs:28–65`) | SVG shapes would participate in content-region layout unless this is also gated. RFC never mentions regions |
| Export / agent | Every node is a 32-byte row with a frame (`export.rs:25–36`, `:91–110`) | Shapes export 0×0 or stale frames |
| Linux paint/hit | Walk is the display list; hits are painted **boxes** (`paint.rs:1–16`, `:798–805`, `:247–264`; `presenter.rs:10`, `:1222`) | 0×0 boxes at the svg origin, or missed group descendants. Path ink is not a hit target unless they add it. `overflow: hidden` clips **child boxes** (`paint.rs:982–1034`), not path ink |
| Apple host | “the view tree mirrors the kernel tree” (`host.rs:7–8`); every created key becomes a Swift view (`host.rs:1060–1068`; `paragraph.rs:155–193`) | D4’s “do not send SVG nodes one by one” **breaks that invariant** (today only inline runs skip) |
| Agent `layout` | Web: `getBoundingClientRect` on every view (`glue.js:1139–1155`). Apple: UIView boxes (`AgentIOS.swift:212–244`) | Web would report circle bboxes; Apple would not if shapes are not views |
| a11y | Apple names come from NodeViews (`Accessibility.swift:9–15`, `:191–212`) | Flattened CAShapeLayers are not in the a11y tree (fine for decorative; not if someone sets `aria-label` on a circle) |
| Collection `validate_row` | Only the **row root’s** position/transform/margin (`views.rs:136–170`) | Nested svg animation is allowed. Not a blocker |
| NodePool | Shape is preorder kinds (`NodePoolIOS.swift:113–121`). `"svg"` is not in `kinds` (`:56`) | A sparkline row is **ineligible** today. Adding `"svg"` without a reset for scene layers/CA is a leak |

**Required shape of D3:** treat SVG descendants like inline runs (a flag, dummy or user-space frames, excluded from `sync_children` and from `region/tree.rs`), **or** compute SVG user-space bounds in publication without Taffy. Declare the Apple “no per-shape UIView” exception next to the host.rs mirror rule. `svg` itself should be a replaced box (`is_replaced` + 300×150 + viewBox aspect), `display:block`, tag-default `overflow: hidden`.

Display list is the wrong call. Unspecified no-layout nodes are also the wrong call.

---

## D4 — host paint

**Agree on web = real SVG, Linux = path backend, Apple = flattened `CAShapeLayer` scene.** Several host facts are missing.

- Web **must** use `createElementNS` for inner shapes. `document.createElement('circle')` in an HTML document is an HTML unknown element, not `SVGCircleElement`. Current glue is `document.createElement(op.tag)` (`glue.js:558`). `createElement('svg')` happens to land in the SVG namespace; `polyline`/`circle` do not.
- `#exact-root * { position: relative }` (`host/web/index.html:66–68`) will hit every SVG descendant. Kernel has no `static` (LLP 1001). For inner SVG geometry this is a Chrome-behavior question the RFC never asks. `img, video { display: block }` (`index.html:69`) does **not** include `svg`; D3’s `display:block` must be in the node’s style (as they do for the kernel), not assumed from that rule.
- Apple: implicit actions **must** be off. A live `cx`/`cy` tick would otherwise CA-ease `position` (CSS snaps). RFC names this; `NodeLayer` does not disable actions today (`BoxLayerIOS.swift:19–33`). `clipsToBounds` already follows overflow hidden (`NodeViewIOS.swift:1188`); `overflow: visible` on the svg box is what lets the ring paint outside. UIKit hit-testing still uses **bounds**, so the ring outside the box is not hittable on iOS even when visible (`NodeViewIOS.swift:803–813` is CSS overflow of descendants, not path ink).
- Linux must paint the svg subtree in viewBox space inside `content()`, and **not** box-paint shape nodes. New `Backend` calls are fine; the walk’s child clip vs path clip must be specified.

---

## D5 — CSS Animations semantics

**Do not implement from this section as written.** The intent (CSS Animations 1 over the Web Animations timing model, seekable, name identity) is right. The rules are not.

What is correct:

- Start when the name appears, including on insert.
- Remove name → cancel; re-add → restart.
- Other longhands retune without creating a new animation identity (CSS Animations 2 / WA).
- Fill modes none/forwards/backwards/both.
- Per-interval easing from the **start** keyframe of the interval, else the animation’s `animation-timing-function`. The `to`/`100%` easing is unused.
- Unknown name still exists and animates nothing (not the same as `none`).
- `paused` holds local time; `running` resumes.
- Negative delay = started in the past. Freeze `breathe 1200ms ease-out -300ms infinite paused` is a real Chrome pattern.
- While an animation (including its fill) applies, transitions on that property do not (CSS Transitions).
- Closed form of clock time, so LLP 1002 D3 still holds **if** the evaluator is actually that function.

What is wrong or missing:

1. **Iteration progress is not `(t − delay) / duration`.** Directed progress is: apply delay and fill to get local time → active time → modulo duration × iteration-count (infinite has no after phase) → `animation-direction` (`normal` / `reverse` / `alternate` / `alternate-reverse`) → per-interval easing. Zero duration, `iteration-count: 0`, and `animation-name: none` vs unknown name are unspecified. The freeze example happens to equal 0.25 on the first iteration; looping does not.
2. **Matching “by name” is incomplete.** CSS Animations 2 pairs each new name with the last *unmatched* old animation of that name. Two `pulse` entries are two animations. Reorder of distinct names should not restart; the RFC’s one-name map cannot say that.
3. **Implicit 0%/100% is per property, and live.** A property missing at 0% or 100% uses the **underlying value at sample time**, not a snapshot of “style or the running transition.” Properties are keyed independently; a keyframe may omit `opacity` while setting `r`.
4. **Re-timing delay jumps.** Updating `animation-delay` on a running CSS animation updates WA `startDelay` and can jump. “Without restarting” is true; “without a visual jump” is not.
5. **Resolved keyframes on the style row** freeze `@keyframes` at bind time. CSS keyframes are live. v1 snapshot is acceptable if declared as a deviation.
6. **Two animations on the same property:** last in the list wins (CSS Animations 1 replace). Unspecified.
7. **Contract longhands win regardless of attribute order.** CSS is document order (shorthand resets). Fine as Contract, but it is not CSS.
8. **`animation-timing-function` on a keyframe:** syntax in the indented form is never shown.
9. **Engine integration.** `Engine` today is transition slots + `running` + `settle_time` = max end (`engine.rs:422–438`). Infinite animations in `running` make `quiescent()` false forever and `settle_time` infinite. D7’s “lowered mode” and D10 are not optional.

Chrome-specific: `getAnimations()` exposes these; `Animation.finish()` on `iterations === Infinity` throws. D10 is correct that settle must skip them.

---

## D6 — animatable set

**Agree for this consumer.** `translate/scale/rotate/opacity` plus `stroke-dashoffset` and `r` covers the sparkline. Refusing colour here is honest (§7). `Property::StrokeDashoffset = 5` and `R = 6` do **not** collide with Height (Height is 4; wire is `p as u8 + 1` so Height=5, dashoffset=6, r=7). You **must** extend `Property::ALL` (`property.rs:32–38`) or `Engine::remove` leaks (`engine.rs:184–186`), extend `targets()` (`kernel/src/motion.rs:71–80`, currently four), and update schema `_transitions`. Syncing `r`/`dashoffset` on every node is a silent cost; only SVG nodes that set those rows should enter the seam.

`transition: all` would then cover `r`. CSS `all` does that. Fine if intended.

PresenterIOS `present` only applies translate/scale/rotate/opacity (`PresenterIOS.swift:681–690`). Apple transitions of `r` have **no apply path** if CA is animations-only (D7). The benchmark uses `animation`, not `transition`, on those properties — still a hole.

---

## D7 — three executors + Core Animation

**Agree that Apple must not sample an infinite pulse on the display link.** `Frames` runs the link while `motion` (`Session.swift:913–970`, `:988–1007`). `batch.motion = !engine.quiescent()` (`host.rs:1013`). If the engine indexes infinite animations as running, the main thread never sleeps. Lowered mode is the right LLP 1002 §4 take **for animations**.

**The CA lowering is not faithful to CSS/Chrome.** Required changes:

1. **`ease` vs CA.** CSS `ease` is `cubic-bezier(0.25, 0.1, 0.25, 1)` (`motion/src/easing.rs:56–57`). `CAKeyframeAnimation`’s default timing is **linear** (nil). `kCAMediaTimingFunctionDefault` happens to be CSS ease; `easeInEaseOut` is CSS `ease-in-out`. The RFC’s warning is pointed at the wrong default, but “set the Bézier explicitly” is the right rule.
2. **`reverse` is not “mirror the keyframes.”** Directed progress runs the interval backwards; a cubic-bezier `(x1,y1,x2,y2)` becomes `(1-x2, 1-y2, 1-x1, 1-y1)`. Mirroring values and keeping easings makes `ease-out` look like `ease-in`.
3. **`alternate` via `autoreverses` and `repeatCount = iterations/2`.** Even counts can work. Odd counts and **`alternate-reverse`** do not (no CA equivalent; fractional `repeatCount` ≠ CSS leftover iteration). Unspecified.
4. **`steps()` / `linear()` “sampled into linear sub-keyframes as springs are.”** Spring samples are interpolable. `steps()` must be **hold pairs** (two keyTimes, same value) or it interpolates between steps. `linear()` stops can be extra linear keys.
5. **Duplicate CSS offsets.** CA `keyTimes` must be strictly increasing; CSS allows several 0% blocks (last wins per property). Merge.
6. **`lineDashPhase`.** Scaling `stroke-dashoffset` by `actualLength/pathLength` is the right *direction*. The **dash array must be scaled the same way** (D4 says the scene already does — keep it). Sign of phase vs SVG dashoffset must be pinned to Chrome, not assumed. `pathLength="1"` + `dasharray="1"` is the draw-in; both numbers are in normalized path space.
7. **`r` → `path`.** Circles about the origin, layer at `(cx,cy)`, is the right way to not fight a position animation. Path interpolation matches Chrome’s numeric `r` only if both keyframes are the same ellipse command sequence. Fine for circles; declare it.
8. **Transform properties → one `transform` key path.** Two CA animations on `transform` clobber unless additive and composed in CSS order (`translate * rotate * scale`). The sparkline does not need this; D6 still allows it.
9. **Delay / fill / pause / seek.** `beginTime` for delay, `fillMode` for CSS fill, `isRemovedOnCompletion = false` when filling forwards: right sketch. Pause as `speed = 0` + `timeOffset` is the usual recipe and is easy to get wrong with delay. **Agent-owned `session.clock` already stops the display link (`Session.swift:989–993`) but does not stop compositor CA.** Without pausing every lowered animation, screenshots and `clock` are not deterministic — the whole reason motion is in v1 (`NOT-DOING.md` clock paragraph). RFC says this; it is load-bearing, not a footnote.
10. **`Animation.finish()` / glue.** Today (`glue.js:1087–1106`): settle uses `timing.endTime` for **every** `document.getAnimations()`; seek calls `finish()` when `t >= endTime`. For infinite animations `endTime` is `Infinity`; `settle` → `Infinity`; `Infinity >= Infinity` is true; `finish()` throws. D10 must change this file, not only `Engine::settle_time`.

Parity plan (engine ↔ Chrome `getComputedStyle` at a seeked `currentTime`; CA `presentation()` under a paused layer) is the right LLP 1003 shape. It only works if the timing model in D5 is the WA one.

---

## D8 — restart on remount

**The kernel/collection half holds; the iOS pool half does not, as written.**

- New row → `ids.fresh()` (`instance.rs:648`) → new kernel nodes → CSS insert. Destroy wrapper destroys the subtree (`txn.rs:501–511`). Web new elements; Linux new engine ids. **A row that stays mounted keeps identity** (`collection/mod.rs:530–538` reuse path). Changing `points` / `cx` / `cy` does not restart `animation-name`. That is Chrome. Good.
- iOS: pool recycle is **after** kernel destroy (`PresenterIOS.swift:668–670`). Today a sparkline row never pools (`kinds` has no `svg`; `shape()` returns nil). RFC wants to extend the reset contract. `recycle()` does **not** remove CA animations or sublayers (`NodePoolIOS.swift:211–230`); `rebind` resets transform/opacity/props (`:232–248`). 1050.000 said “anything in motion” makes a row ineligible (`llp/1050.000:290–291`); the code only treats **gesture holds** as ineligible (`NodePoolIOS.swift:149–159`). An infinite pulse is “in motion” forever: either sparkline rows never pool (restart is then free, D8’s Swift work is unused) or they pool and **must** drop scene layers + `removeAllAnimations`. Name which.
- macOS has no `NodePool` (`#if os(iOS)`). Linux has none.
- Apple “re-lower dash animation with the same beginTime when length changes” is the right CSS-phase-preserving idea; replacing a `CAKeyframeAnimation` without that beginTime **restarts**.

---

## D9 — fill policy

**The product call is right (web wins: start at mount, not at first paint). The reading of 1050.000 is not a description of the code.**

- 1050.000 D1 `complete` default, D3 “don’t build a monster row mid-fling” are rulings on an **unassigned** RFC. No `fill-policy` in tree.
- Current collection already builds overscan (one viewport each side, plus 0.25s of travel, `mod.rs:38–50`) and can keep leaving rows under a limited report (`:561–590`). Those rows’ animations would already be running when they enter the port — which D9 later admits (“overscan starts early”).
- “Under complete, draw-in starts as the row appears” **contradicts** the overscan sentence. Overscan is the honest one.
- Benchmark “each time a row comes on screen”: if the row remained mounted in the buffer, it will **not** replay. That is virtualized DOM. If the spec wanted replay on every visibility, you would need Intersection Observer semantics, which this RFC correctly refuses.

Build D9 as “start at kernel create; overscan can finish the draw-in off-screen,” and do not pretend 1050.000 is shipped.

---

## D10 — clock / settle

**Agree, and it is currently a blocker for infinite animations.** Glue and `Engine::settle_time` must ignore `iterations === Infinity` (and `endTime === Infinity`). Seeking must still move them (`currentTime = t`, never `finish()`). Finite fill-forwards animations *do* settle at the end of the active interval.

`SETTLE_DEADLINE_MS = 20_000` (`glue.js:1108`) would not save you from `Infinity`.

---

## D11 — reduced motion

**Agree.** Same as LLP 1002 §4: producer emits `animation: none`; engine has no opinion.

---

## D12 — refusals

**Agree.** Gradients, SMIL, scroll timelines, `animation-composition`, events, `!important` in keyframes, nested `svg`, `use`/`defs`, SVG text, filters. The benchmark’s colour flash is correctly left to another RFC.

---

## Particular questions

### 1. Nodes vs display list; what breaks with no layout?

Nodes are the right call. “No layout” as “`sync_children` like Text” is only half of the analog. Publication `continue` without Taffy, region derived trees, Linux box hits, Apple view-mirror, agent `layout`, a11y, and NodePool all assume every non-inline node is a box. See D3 table. A display list would break animation targeting and the web oracle worse.

### 2. Are D5 CSS Animations semantics correct?

No. Start/restart-by-name, fill modes, per-keyframe interval easing, pause/resume, negative delay, animation-over-transition: yes in spirit. Timing formula, duplicate names, live implicit 0%/100%, delay retune jumps, zero duration, `none` vs unknown name, baked vs live keyframes: wrong or missing. Details under D5.

### 3. Is D7 CA lowering faithful?

No. Explicit CSS `ease` Bézier: yes. Reverse/alternate/alternate-reverse, steps sampling, odd iteration counts, agent-clock vs compositor, dasharray+phase scaling, transform composition: will not match Chrome without the list in D7.

### 4. D8/D9 vs collection and iOS pool / fill policy?

D8 restart holds on web/Linux/macOS and on iOS **until** SVG rows are pooled. Pool reset as specified is incomplete; 1050.000’s “in motion → ineligible” conflicts. D9’s web-wins rule holds; its 1050.000 mechanics are not in the tree.

### 5. Web parity pitfalls

| Pitfall | In-tree fact | Chrome |
|---|---|---|
| `r`/`cx`/`cy` as CSS | D2 rows; `css.rs` will emit `r:3px` if they go through default f32 | CSS geometry properties; units required except 0; presentation attribute `r="3"` is also valid |
| Unitless opacities / miterlimit | default f32 → `px` (`css.rs:305–308`) | invalid → property ignored → `r` pulse or fill-opacity broken |
| `createElement` vs NS | `glue.js:558` | inner SVG **must** be SVG namespace |
| `pathLength` + dashes | D4/D7 scale both pattern and phase | Chrome scales path distance; both must agree |
| `overflow` on `svg` | UA hidden; tag should set hidden; author `visible` | overflowing pulse paints; hit-testing of overflow ink is browser-specific |
| `display` of `svg` | kernel block; page CSS does not set svg | HTML svg is inline replaced; they must emit `display:block` (declared deviation, OK) |
| `#exact-root * { position: relative }` | `index.html:68` | applies to every SVG child |
| Settle + infinite | `glue.js:1087–1106` | `endTime` Infinity, `finish()` throws |
| Presentation attribute vs CSS | rows as CSS, geometry as attributes | CSS wins; do not emit both for `r` |

### 6. NOT-DOING §9 / RULES.md

**The take is real in name and weak in weight.**

Moving `@keyframes` and “a Core Animation executor” off `rules/NOT-DOING.md` §Motion is exactly the file’s procedure, and CSS `animation-iteration-count: infinite` **is** the repeat driver that list called out. LLP 1002 D2 already permitted CA. The RFC is honest that decay/sequence, colour, SMIL stay out.

The *take* is deleting dead `svgSource`. The rule is: write what it unblocks **and take something off the doing-list**. `svgSource` is not on the doing-list; it is unused schema. Unblocking `~/bench/cryptobench` is also **not** Caltrain / Weird Castle / Markdown / Fieldnotes (the bar at the top of NOT-DOING). That is Charlie’s question (§6 Q1), not something the RFC can settle by deleting a dead prop.

RULES fit:

- Not a sixth check. Fixtures belong on the existing test/async lanes (LLP 1002 §5).
- Implementer + date present → not a speculative spec under RULES “a spec needs an implementer.”
- `apps/sparkline` plus a paused-layer XCTest plus Chrome `getTotalLength` harness is **apparatus**. RULES: agents add none without a human saying so. 1050.000 already forbade a new in-repo benchmark app without Charlie.
- Path parser in **kernel** is paid by every app (LLP 1047: kernel is core). RFC §4 admits wasm growth. That is the usual schema tax, but arcs-in-core for a crypto-list consumer is a 1047 smell; keep the parser small or capability-link it.
- No `llp/current/` 16th slot: 1055 is not linked there. Fine.

### 7. D1/D2/D6 vs SVG 2

Initials and inheritance: good. Grammar: `points` odd coordinate drop: good. `d` error stop: good. `stroke-dasharray` odd repeat: good; negatives missing. `pathLength` scaling: right idea; 0/negative missing. `x/y/rx/ry/d` as non-CSS: not SVG 2. `viewBox` intrinsic ratio: missing. User units vs CSS lengths: missing. Arc cubics vs Chrome length: contradictory with §8.

---

## Verdict

**Build with named changes.** Do not build as written. The architecture (CSS vocabulary, node targets, browser oracle, CA only for repeating compositor animations, mount-not-visible start) is the one this repo would have to build anyway. The timing model, the no-layout node contract, the CA mapping, web emit/settle, and the NOT-DOING take are not ready.

### Ranked required changes

1. **Rewrite D5 to the Web Animations directed-progress model** (delay, fill, zero duration, iteration-count including 0 and infinite, all four directions, per-property implicit 0%/100% as live underlying values, name matching with duplicate pairing, `none` vs unknown). Drop `(t − delay) / duration`. Declare baked keyframes as a deviation if they stay on the row.

2. **Specify the SVG-node layout contract** (D3/D4): Text-run analog (flag, dummy or user-space frames, `sync_children` skip, **region tree skip**); `can_hold_children` + new ApplyError; `svg` as replaced 300×150 with viewBox aspect; Apple **declared** exception to “view tree mirrors kernel”; Linux paint in viewBox space and path hits vs box hits; agent `layout` parity; NodePool kinds + recycle of scene layers/CA **or** keep SVG rows ineligible.

3. **Make D7 match CSS or shrink it:** reverse easings, `alternate-reverse` and odd `alternate`, `steps()` as holds, merge duplicate offsets, scale dash **array and** phase, implicit actions off, every lowered animation paused at engine seek when `session.clock != nil` (and during screenshots). Do not put transform properties on one non-additive `transform` until that composition is specified.

4. **D10 in `glue.js` and `Engine`:** infinite animations excluded from settle; never `finish()` them; seeking still sets `currentTime`.

5. **Web emit:** `createElementNS`; unitless vs `px` per property (`fill-opacity` / `stroke-opacity` / `miterlimit` unitless; `r`/`cx`/`cy`/`stroke-width`/`dashoffset` lengths); do not emit `r` as both attribute and CSS; account for `#exact-root * { position: relative }` and `display:block`/`overflow`.

6. **Schema/seam:** bits 100–114 with `layout: false` on paint/`r`/`cx`/`cy`/dashes; `Property::ALL` + `targets()` + `_transitions` wire including Height=5; do not sync `r` on every View; `css.rs` `RowValue` arms so rows are not skipped.

7. **D8/D9:** describe the collection that exists (fresh ids, overscan, limited keep), not 1050.000. Resolve pool vs “in motion.” Start = mount.

8. **SVG 2 honesty in D1/D2:** subset split is a v1 choice, not SVG 2; name default `preserveAspectRatio`; refuse negative dasharray/pathLength/viewBox size; arc cubics ≠ Chrome `getTotalLength`.

9. **NOT-DOING:** Charlie accepts crypto-list as the consumer and a real doing-list take (dead `svgSource` is hygiene, not a trade). Human approval for `apps/sparkline` / CA XCTest apparatus. Colour stays a separate RFC.

10. **Do not expand CA to transitions in the same drop** (RFC §6 Q2). Get animation fixtures green first; Apple `present` still cannot apply `r`.
