# LLP 1128: Timed and interactive motion on Apple

**Type:** RFC (a decision brief; nothing here is built until Charlie has read it)
**Status:** Draft r19, 2026-10-17
**Systems:** Motion (`exact-motion`: played transitions, holds, cancellation), Apple host (`host/apple/src`: `presence.rs`, `svg.rs`, `svg_lower.rs`, `layout.rs`), Apple presenter (`host/apple/Sources/ExactKit`: the layers' animations, press feedback, presence, the agent), the style table (one Apple-only row)
**Author:** Claude (Opus 5.5) for ide
**Date:** 2026-10-17
**Amends:** LLP 1055 D5 (an animation and a transition on one property) and D7 (who executes on Apple), LLP 1055.000 D15 and LLP 1055.001 §2 (when an animation lowers, and `scale`'s key paths), LLP 1063 (how layout motion plays on Apple), LLP 1003.001 D8 (a played transition on Apple)
**Related:** LLP 1002 (one engine, three executors), LLP 1003 (transitions), LLP 1076 (Android plays transitions in its reader), LLP 1061 (the display link's rate), LLP 1062 D9 (`light-dark()` in keyframes), LLP 1115 (author > platform > CSS default)

## 1. The problem

On Apple the engine samples a box's transition at every display frame on its owner thread, and the presenter applies each value on the main thread, so a fade keeps both threads and the display link busy for its whole length. Handing opacity transitions to Core Animation cuts a page of 24 fading boxes from 0.23–0.25 s of CPU per 10 s to 0.04–0.05 s (iPhone 18 Pro simulator, iOS 27.0; branch `tuft/opacity-transitions-ca`).

While Core Animation plays a property, the layer's model holds where the property is going and only the screen shows the motion. Presenter code reads a view's model as what the screen shows, or as state:

| Code | Reads | Effect while Core Animation fades the view |
|---|---|---|
| `FocusSearchIOS` | `alpha < 0.01` as hidden | focus skips a view that is still fading out |
| `TextRasterIOS.textBand` | `alpha == 0` as nothing to draw | text that changes during a fade toward 0 is not painted |
| `PressFeedback.dimPress` | `layer.opacity` | the dim is computed from the target |
| `AgentCheckIOS`, `AgentSample` | `alpha`, `layer.render(in:)` | the agent reads the model |
| `BarFaceIOS`, `SwipeActionsIOS`, `SvgIsland`, `BoxFilter` | model values, `render(in:)` | the picture shows the model |

Today `svg_lower` lowers a CSS animation to Core Animation only away from filters, masks, glass groups, handlers and corner origins, so which executor runs depends on where a node is. Applying that to transitions too leaves the list of contexts open, because each new reader of the model adds one.

## 2. The principle

Core Animation separates the model from the presentation, and apps live with it. Exact does not fight that separation. It has two kinds of motion, and which kind runs is explicit and does not depend on where the node is (ide, 2026-10-16):

- **Timed motion** is played by Core Animation from its timing (`CAMediaTiming`) on the render server. It keeps going while the app's threads are busy.
- **Interactive motion** is sampled by the engine at each frame on its owner thread and applied by the presenter on the main thread. It follows input as it arrives.

Authors choose the kind on Apple platforms. Other platforms do not have this distinction and ignore the choice.

## 3. Decisions

**D1. The two kinds.**
- *Timed:* the engine hands the curve to Core Animation (`Engine::play_transition` for a transition; a lowered spec for a keyframe animation) and samples it no more. The view's model holds the property's value without the motion: a transition's target, or a keyframe animation's underlying value; a forwards fill shows on Core Animation's track (`fillMode`), not in the model. The engine keeps the curve, so it knows at any time what the screen shows (D6).
- *Interactive:* the engine samples the motion at each frame, and the model is what the screen shows.

**D2. The default kind comes from the property and the node's own style, never from where the node is.**
- *Timed:* a `transition` or `animation` of `opacity`, `translate`, `scale`, `rotate` or `background-color` on a box; an SVG element's lowered properties (LLP 1055.000 D15); layout motion (`-exact-layout-transition`, D5); an exit animation (`-exact-exit-animation`, D9); the return a released hold plays (D6).
- *Interactive:* what input drives (holds and drags while they last, drag and scroll timelines), and what Core Animation cannot play as CSS does:
  - a colour Exact paints into a picture of its own: `color` in a text raster, an SVG fill or stroke drawn by a paint server;
  - a colour outside sRGB's 0–1 (a wide-gamut or HDR colour, LLP 1100), until a lowered track carries extended values: the lowering writes 8-bit colours today (`Value::to_rgba8`);
  - a colour track whose alpha varies, because Core Animation interpolates colours unpremultiplied and CSS premultiplied, and a `light-dark()` keyframe, which LLP 1062 D9 keeps sampled because its animation plays the appearance it started under;
  - a rotation out of the screen's plane or a z translation (LLP 1077 D8), which LLP 1055.001's component key paths do not say;
  - a transform about an origin off the centre of an axis on which the box has no size, which `anchorPoint`'s unit coordinates cannot say (D5);
  - the transform motion of a box or an SVG element wherever a scale is below 0, or an underlying one is 0: the matrix of an underlying `scale` of 0 or below no longer holds the components a track keeps, and Core Animation plays a negative scale track as a mirror image (§6.8);
  - a size that lays the content out again as it changes (`height`), and a layout surface Exact draws at each size (D5);
  - a background colour the layer's `backgroundColor` does not paint, by `BoxLayerIOS.applyBoxLayer`'s conditions: a box the presenter draws (a patterned border or sides that differ, a gradient, a `background-clip` other than `border-box`, a corner shape or radii the layer cannot say), or a vibrancy view's fill;
  - the other cases `svg_lower` names by property or by the node's own style, such as an SVG element's percentage translate.

**D3. The author's choice: `-exact-apple-motion`.** One Apple-only style row, not inherited: `auto` (D2, the initial value), `timed` or `interactive`. It applies to the node's own `transition`, `animation`, `-exact-layout-transition` and `-exact-exit-animation`, and to the return of a hold on the node. What D2 keeps interactive by its property stays interactive; `timed` there is a compile warning that names the property. Changing the row while motion runs hands that motion over (D6).

**D4. One effect per Core Animation property.** The CSS cascade decides between effects on one property: a running transition is above an animation, and an animation is above the style value ([CSS Cascading 5 §6.1](https://www.w3.org/TR/css-cascade-5/#cascade-sort)), as `Engine::frame` composes now; this corrects LLP 1055 D5's last bullet, which put the animation above. A hold, which CSS does not have, is above both: while a hold owns a property, every reader of the engine's presented value (`Engine::frame` among them) gives the held value, and a hold begins from the value the screen shows, the winning animation's when one applies, not the slot's underlying value. Core Animation plays a property only while it has one effect: one animation that names it, or one running transition, and not both. Two animations that name one property, or a transition or a hold while an animation names the property, make the property interactive for as long as both are declared or running, whatever phase each is in, so a change of kind never waits on a phase boundary that nothing would wake for. The presenter's own animations stay after the effect's on the layer, because Core Animation applies a layer's animations in the order of `animationKeys`, and a non-additive one replaces what is below it ([`isAdditive`](https://developer.apple.com/documentation/quartzcore/capropertyanimation/isadditive)).

**D5. A box's transform, its press and its layout motion.**
- A layer's `anchorPoint` is its box's `transform-origin` (the centre, CSS's initial value, is UIKit's default), and the presenter places the layer by its `bounds` and its `position`. Core Animation then turns and scales the box about its origin exactly (§6.1), as an SVG scene's layers turn about theirs (LLP 1055.000 D5).
- The layer's `transform` holds the box's `translate`, `rotate` and `scale`, each played by its own component key path, which keeps the others: `transform.translation.x` and `.y`, `transform.rotation.z`, and `transform.scale.x` and `.y`. CSS `scale` scales x and y, so it plays on those two. `transform.scale`, which LLP 1055.001 used, scales z as well, which makes the transform three-dimensional: `render(in:)` then does not draw it (§6.7), and reading it back averages the three factors. An SVG element's outer layer takes the same key paths (LLP 1055.000 D5), so this and D2's rule for scales apply to it too. The host's placement (a layout offset, a sticky offset, a keyboard's lift) moves the layer's `position`, outside the transform. A flight and a context preview, which move the box by its transform, show it without its transform's timed motion: the presenter takes those animations off while they last and adds them back whole, with their start times, after.
- A press's scale and dim are factors over what shows, and the presenter's alone. It multiplies a held factor into each value it writes to the model, and into a timed track's values while one plays, making the track again when the press starts or ends. The ease between pressed and released stays additive, as now, and is added after the track.
  - For the scale this is exact: Core Animation concatenates an additive transform with what is below it ([`isAdditive`](https://developer.apple.com/documentation/quartzcore/capropertyanimation/isadditive); §6.2).
  - An opacity is clamped to 0–1 before the factor, as Core Animation clamps it ([`opacity`](https://developer.apple.com/documentation/quartzcore/calayer/opacity)). Where a track's curve leaves that range between its keys (an overshooting cubic), the presenter adds keys where it crosses 0 and 1 and at its extrema, holds the clamped value between crossings, and plays each piece between keys as its own cubic. A cubic split at a point is two cubics, each piece runs between different values, and Core Animation plays a timing function whose control points leave 0–1 exactly (§6.10), so the track stays exact.
  - A press that replaces the background (`-exact-press-background-color`, LLP 1061 D7) takes a timed background-colour animation off while it holds, and adds it back whole, with its start time, at the release.
  - An additive opacity adds rather than multiplies, so while the dim fades back after a release, an opacity that moves is off by at most its change times one minus the dim.
- Layout motion keeps LLP 1063's meaning: the box is laid out at its new frame at once, and only its surface (background, border, shadow and the clip on its children) shows the moving size. Its offset plays as an additive `position` animation, from the old place less the new to zero, over the model's position, so a sticky offset or a keyboard's lift that moves the model meanwhile adds to it ([`isAdditive`](https://developer.apple.com/documentation/quartzcore/capropertyanimation/isadditive)). Its size is timed where the surface is Core Animation's own properties (one border, one radius, no gradient: `Surface.swift`'s condition) and its radius does not depend on its size (no percentage): the surface layer's `anchorPoint` is its top-left corner, so its size plays as one `bounds` animation, beside the paths of the clip, of the shadow and of the shadow's exclusion mask (`ShadowCaster`). A surface Exact draws at each size is interactive. While the surface stands in for the box, which then paints no background of its own, a timed background-colour track plays on the surface layer with its start time, and goes back to the box's layer when the surface ends.

**D6. Handing over.** Motion changes kind when a hold takes it, when a hold is released, when an overlap starts or ends (D4), when `-exact-apple-motion` changes, and when a style of the node D2 reads changes (a border or a gradient added under a timed background colour), or its size reaches or leaves 0 on an axis its origin is off the centre of: the host decides the kind again whenever any of these change, not only when the node's animations do. The owner's batch carries the change, and the presenter applies it in one Core Animation transaction:
- *One source of truth:* the engine decides every hand-over, retarget and cancel from its own curves, and the presenter applies what the batch says. Main may apply a batch after Core Animation has played on, so a late batch can step the motion back once, by what it moved during the delay, as a late frame of interactive motion shows an older value. The engine and the screen never drift apart.
- *Timed to interactive:* the presenter removes the animation and writes the engine's sample for that frame to the model, as it writes any value, its press factors applied (D5). The animation and the engine's curve have the same start time on the same clock (`ExactEnv.t0`), so the sample is what the screen shows at that frame, to within a sampled track's sampling. A hold begins from the engine's value at the input's time (D4).
- *Interactive to timed:* the presenter writes to the model the value without the motion (D1: a transition's target, an animation's underlying value), and adds the animation whole, with the engine's start time; Core Animation shows it from the current time. A retarget's new animation is added the same way.
- *Cancelled:* when a declaration stops covering running motion (`transition: none`, a `transition-property` without the property, an animation name removed, including what the author changes for reduced motion), the presenter removes its animation in the same frame, and the model holds the value CSS gives. The engine cancels such motion for every host before Core Animation plays transitions (stage 1).
- *A followed source:* a drag or scroll timeline whose source plays timed (a released hold's return) reads the engine's curve for that source at each frame, and its followers ask for frames while the curve runs, so they follow through the release (LLP 1057.003).
- *A released hold:* `Engine::play_transition` takes a slot whose owner is `Owner::Returning` and keeps the return's token, so the hold's `end` reply and a new grab work as now. The return plays the engine's curve: a spring as keyframes of its absolute values (`Engine::spring_frames`), an easing (`PlayedCurve::Easing`) as `svg_lower` lowers a keyframe interval, a cubic, or keys at the breakpoints of `steps()` and `linear()`. `CASpringAnimation` is not used: its velocity is a fraction of the distance, which cannot say a release with velocity at zero displacement (§6.5). Nothing samples the return, so its end is a wake-up (D7).

**D7. The start and the end.** Every timed curve that ends asks for one wake-up at its end, and the batch's next wake-up is the earlier of the runner's next timer and the next such end. Its batch does what the end needs: the engine retires the curve, an exit's view goes (D9), a released hold sends its `end` and finishes what it began (a reorder's `arrange_finish`, a drag binding's last tick), a drag binding a transition suspended is published again, and a layout surface gives its background back to the box (D5). An animation that repeats forever has no end and asks for none.

 Handing a curve to Core Animation does not change when it starts (LLP 1003.001). A new curve starts at the frame that first shows it, and is handed over with that frame's time as its begin time. A release with velocity starts at its input's instant (LLP 1003.001 D6), so its animation begins slightly in the past and has already travelled at the next frame (§6.3). A cancel and a release with zero velocity keep their rules. LLP 1003.001 D8's "on Apple none exist" no longer holds.

**D8. Reading what is on screen.**
- Code that needs a value as the screen shows it reads the layer's presentation (`CALayer.presentation()`): the agent's `check`, and the text raster's choice of the band that is visible and of the text it paints at once (`TextRasterIOS`), which converts through the presentation layers' geometry. While timed motion moves a raster whose prepared band does not hold all of it, the presenter keeps preparing that raster at each frame until the motion's end, as the scroll pump does during a scroll.
- Code that needs state reads state, not opacity: focus search and the text raster read `visibility`, `display` and `inert`, as `SegmentsIOS` does since PR #698.
- A picture Exact takes of content is a still: it does not follow motion after it is taken.
  - It shows the presentation at that moment when it is taken from a copy of the layers (`Capture.paintOrderLayer`, which gives the copy each animated stored property's presentation value, D10; the GPU canvas's `Shadow.swift`), by `CARenderer` (`SvgFilterLive`), or with `drawHierarchy(in:afterScreenUpdates:)`, which `screenshot` and a shared image of a view on screen use (§6.6).
  - It shows the model, a transition's target or an animation's underlying value, when it is taken with [`render(in:)`](https://developer.apple.com/documentation/quartzcore/calayer/render(in:)) from the live layers, which ignores animations (an SVG island, a box filter, a swipe action's icon, a shared image of a view off screen), or from model values (a bar item's face). A glass group likewise ignores an opacity Core Animation plays between it and its glass.
  - An author whose picture must follow the motion makes it `interactive` (D3). This was decided over excluding those contexts (ide, 2026-10-16).
- Hit testing reads the model, as UIKit's does during an animation (§6.4).

**D9. Exits.** From the moment its exit starts, a leaving view is out of the app and only drawn (ide, 2026-10-17: "align with the underlying host platforms and implement an efficient system"). As now, it takes no input, it is hidden from accessibility (`PresenceIOS`), and the kernel neither lays it out nor sends it state. It draws what it last drew. Its timed motion keeps playing on the render server, as Core Animation keeps playing the animations inside a UIKit view that fades out before `removeFromSuperview` (§6.11). Its interactive motion stops where it was, except its own exit animation and what that composes with on the root: the exit is the last of the root's animations (LLP 1063 D5), so where it names a property another of the root's animations names, the two overlap as D4 says, and the engine composes them for the exit's length. The exit moves what is drawn on the leaving root's own layer; an inherited property it animates (`color`) does not reach the members, which keep what they last drew. When the root is text, the presenter paints and publishes its raster for the leaving root as for a live view, so its own colour moves. A picture Exact makes at each frame (a live filter's, `SvgFilterLive`) keeps its last picture: its producer and its frame requests stop when the exit starts, and nothing it finishes later is shown. So a leaving view costs no frames unless its exit animation is interactive.
- Its exit animation plays as timed by D2. The presenter still releases each member's id when the exit starts, so a new node can take it, but it moves what the member draws with (its installed animations, its SVG scene, a canvas's last bitmap) out of the id-keyed tables into the leaving record, and stops what would make new pictures for it (a canvas's replays). It tears them down when the exit ends (`endExit`), not at the release.
- The engine keeps only the exit root's motion, as LLP 1063 D5 says, for its exit animation and what that composes with.
- A leaving view whose motion is all timed asks for no frames, and its exit's end (`Engine::exit_end`) is a wake-up whose batch removes the view, as D7's rule for every timed curve says. Under the agent's clock, a seek past the end removes it in the seek's batch.

**D10. The agent.**
- Under the agent's clock every timed animation is held at the clock's time, as lowered keyframes are now (LLP 1055 D7).
- `screenshot` keeps `drawHierarchy`, which draws the view hierarchy "as visible onscreen" ([Apple](https://developer.apple.com/documentation/uikit/uiview/drawhierarchy(in:afterscreenupdates:))), the presentation included (§6.6).
- `sample` first makes the pictures a CPU render cannot see, as `screenshot` does on its platform (web and native guests' snapshots, canvases' pictures). It then renders `Capture.paintOrderLayer`'s copy into its extended-range context, instead of the live layers. The copy gives each animated stored property its whole presentation value (`transform`, not `transform.scale`), so a press over a scale track comes out composed (§6.7).
- The copy reads the presentation after `CATransaction.flush()`, as `sample` flushes now: read in the transaction that installed or seeked an animation, or mounted its layer, the presentation does not show it yet, and after the flush it does (§6.9). A layer that still has none keeps its model value in the copy.
- So `sample`, `check` and a screenshot agree at the middle of a fade.
- `clock settle` waits for the end of timed motion (`Engine::settle_time`, PR #700).

## 4. Platforms

The rule is the same on iOS, iPadOS, tvOS and macOS. What each host can play as timed differs today: the UIKit presenter (iOS, iPadOS, tvOS) lowers boxes' transforms and background colour (`box_motion`), and macOS lowers keyframe opacity and SVG properties. Until a Mac stage lowers the rest, motion on macOS that D2 would make timed is interactive there, and `timed` on it gives no warning. `interactive` applies on every Apple platform now.

## 5. Stages

1. The engine, for every host: the bookkeeping of played transitions (PR #700); a hold above animations and begun from the value the screen shows (D4); cancellation when a declaration stops covering running motion (D6); `play_transition` for a returning hold (D6).
2. The presenter's readers and the agent (D8, D10), and a press's factors over what shows (D5), with a paused capture of a split, clamped opacity track under a held dim compared with the screen.
3. Opacity: transitions timed by D2, the context checks gone for opacity, the row (D3), overlaps and hand-overs (D4, D6), the start time (D7) and exits (D9).
4. A box's transform about its `anchorPoint`, with `scale` on `transform.scale.x` and `.y`, its press and its layout motion (D5), a released hold's return (D6), and background colour; measured with a sticky header scrolled and the keyboard moving during a layout transition, and with a press held over a timed background.
5. Keyframe animations under D2: the context checks of LLP 1055.000 D15 and LLP 1055.001 §2 go. Then macOS (§4).

Each stage lands with red/green tests and the iOS, macOS and tvOS suites. Stages 3–5 also measure CPU, and frame pacing on a device with and without other work on the main thread.

## 6. Measurements

A probe app on an iPhone SE simulator with iOS 27.0, each capture read from a context of known layout and checked against a still control and against the screen:

1. A `transform.rotation.z` track over a layer whose `anchorPoint` is its top-left corner turns it about that corner and keeps the model's translation and scale.
2. An additive `transform` added after a scale track composes with it (2 × 0.5 showed 1); added before it, the track replaces it (showed 2). Of two non-additive animations, the later in `animationKeys` shows.
3. An animation added with a begin time in the past shows its progress at once.
4. Hit testing finds a view where its model is, not where a position animation shows it.
5. A `CASpringAnimation` with no displacement and an initial velocity does not move.
6. `render(in:)` drew a fade held at 0.25 at its model's 1; `drawHierarchy` drew it at 0.25, a scale at 0.5 and a colour halfway as the screen showed them.
7. `transform.scale` sets z too, and `render(in:)` then draws the layer unscaled; `transform.scale.x` and `.y` keep the transform flat, and a copy of the layers with each animated stored property at its presentation value drew a fade, a scale and a scale under a press as the screen showed them.
8. Under a scale track, 30° at an underlying scale of 0 showed 0°, and `scale: -1` under a track of 2 showed −2; scale tracks of −1 under a rotation of 30° showed a mirror image where CSS turns it 180°.
9. A fade installed at 0.25 read 1 from the presentation at once and 0.25 after `CATransaction.flush()`; seeked to 0.75, it read 0.25 and then 0.75. A layer mounted in the same transaction had no presentation at once and 0.25 after the flush.
10. An opacity eased by `(0.25, 1.6, 0.75, −0.6)` matched the cubic to four decimals at five times.
11. A child's rotation kept playing while its parent faded out before `removeFromSuperview`.
