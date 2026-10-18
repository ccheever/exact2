# LLP 1131: The easing the author leaves unsaid

**Type:** RFC (a decision brief; nothing here is built until Charlie has read it)
**Status:** Draft r7, 2026-10-18
**Systems:** Motion (`exact-motion`: the `transition` parse, and the curve an unsaid easing plays), the kernel's rows (LLP 1003 §1's wire and the runner's patches), the Apple hosts (Core Animation's default timing function), docs
**Author:** Claude (Opus 5.5) for ide
**Date:** 2026-10-18
**Amends:** LLP 1003 §1 (the easing codes)
**Related:** LLP 1115 (author > platform > CSS default), LLP 1115.000 D17 (the neutral look on desktop Linux and Windows), LLP 1128 (timed and interactive motion on Apple), LLP 1063 (layout motion)

## 1. The problem

When a `transition` names no easing, Exact plays CSS's initial value on every host: `ease`, which is `cubic-bezier(0.25, 0.1, 0.25, 1)` ([CSS Transitions 1 §2.3](https://www.w3.org/TR/css-transitions-1/#transition-timing-function-property); `motion/src/parse.rs`). On Apple platforms that is CSS's choice standing where the platform's belongs: LLP 1115 puts the platform's answer above CSS's default for every value the author leaves unsaid. The two curves are the same today (§6.1), so the problem is not what the screen shows now but who chooses: a curve Exact copies does not follow when the platform changes its own.

## 2. The principle

James, 2026-10-18: "The host platform chooses the easing for the current context when the application code doesn't specify it. The context could depend on the properties transitioning, the element whose properties are transitioned, or other factors the host platform uses to choose its easing curve. As such, Exact should use the host platform APIs simply and not clone the host platform's easing curve." And: "Usually the solution is to find the correct Apple API and HIG and how to configure them."

## 3. Decisions

**D1. What is unsaid.** A `transition` or `-exact-layout-transition` declaration that names no easing takes the platform's (D3). An easing the author writes is the author's, `ease`, `steps()` and `linear()` included, and plays as CSS says; so does a design's easing token. `animation` and `-exact-exit-animation` keep CSS's `ease` (§4, question 2).

**D2. Nothing new moves.** The rule changes the curve of a transition that starts, never whether one starts. An eased declaration starts a transition only when its duration plus its delay is above 0 ([CSS Transitions 1 §3](https://www.w3.org/TR/css-transitions-1/#starting): the "combined duration is greater than 0s"), and a spring always starts (`Transition::starts`). An unsaid easing is an easing for this test, so `transition="opacity"` and `"opacity 0s"` start nothing, as now (§6.6). James accepted the change to every unsaid easing on this condition (2026-10-18).

**D3. The platform's default.**
- *iOS, iPadOS, tvOS and macOS:* Core Animation's default timing function, `CAMediaTimingFunction(name: .default)`: "The system default timing function. Use this function to ensure that the timing of your animations matches that of most system animations" ([Apple](https://developer.apple.com/documentation/quartzcore/camediatimingfunctionname/default)). UIKit's default timing parameters are the same function: `UICubicTimingParameters()` gives "the system's default timing curve", "based on the default Core Animation timing function" ([Apple](https://developer.apple.com/documentation/uikit/uicubictimingparameters/init())); Core Animation's implicit animations and AppKit's animation groups use it when given no timing function (§6.3–§6.5). `UIView.animate(withDuration:)` eases by `(0.42, 0, 0.58, 1)` (§6.2), but that is the default of that call's options (`curveEaseInOut`), not the system's.
- *The web:* the browser's initial value, `ease`.
- *Desktop Linux, DRM and Windows:* these hosts show the web's neutral look (LLP 1115.000 D17), whose answer is the web's, `ease`.
- *Android:* "By default, ValueAnimator uses non-linear time interpolation, via the AccelerateDecelerateInterpolator class" ([Android](https://developer.android.com/reference/android/animation/ValueAnimator)). That is an interpolator, not a cubic; until a revision decides how the Android host plays it (§4, question 1), Android plays `ease`, which is a tell there.

The Human Interface Guidelines name no curve. They leave motion that adapts to its context to the system's components ("System components might also adjust their motion in response to factors like accessibility settings or different input methods", [Motion](https://developer.apple.com/design/human-interface-guidelines/motion)), which Exact presents through UIKit and AppKit where it has them, such as its native controls and sheets. For custom motion they ask that it be purposeful, optional, brief and cancellable, which is the author's to decide.

**D4. Taken from Core Animation, not copied.** The plan and the kernel's rows keep an unsaid easing unsaid (easing code 9 in LLP 1003 §1's grammar), on the wire and in the runner's patches (`Transitions::parse`, `kernel/build.rs`) alike. Whatever plays, lowers or writes the curve plays it as the cubic whose control points `exact-motion` holds for the platform's default: the engine, LLP 1128's Core Animation lowering, the web's CSS and the JS target's motion module (`host/web-js/motion`). The Apple host takes them from Core Animation, `CAMediaTimingFunction(name: .default)`'s `getControlPoint(at:values:)`, when ExactKit's session starts, before it creates its runtime (`ExactSession.init`), and every other host leaves them `ease`. From there the unsaid easing is a cubic like a written one, so Core Animation's tracks, a reversed or presampled track and the engine's samples need nothing new; and if Apple changes its default, Exact apps follow it with no release of Exact.

**D5. What changes now.** On iOS 27.0 and macOS 27.0.1, Core Animation's default is `(0.25, 0.1, 0.25, 1)`, CSS's `ease` (§6.1, §6.5), so no motion on screen changes today.

**D6. Getting the platform's easing back.** Contract has the `transition` shorthand alone (`transition-timing-function` is `lower-unknown-attr`), and a node's own row replaces its class's row of that name whole (`contract/lower/src/class.rs`, LLP 1017 P6). So code that wants the platform's easing over a class's writes its own shorthand with no easing, and needs no keyword. CSS does the same: "When values are omitted from a shorthand form, unless otherwise defined, each 'missing' sub-property is assigned its initial value" ([CSS Cascading 5 §3](https://www.w3.org/TR/css-cascade-5/#shorthand)).

**D7. Docs.** `docs/contract-for-agents.md`'s motion paragraph says that a `transition` or `-exact-layout-transition` with no easing plays the platform's and a written easing is CSS's, so an agent writes one only when the design asks for it.

## 4. Questions for Charlie

1. D3: Android. Playing `AccelerateDecelerateInterpolator` as Android does means the Android host evaluates the interpolator through Android's API, since it is not a cubic. Should that be a later revision of this RFC, or stay `ease`?
2. D1: keyframe animations. CSS gives `animation-timing-function` the same initial value, `ease`, so the same rule would apply. Contract composes an animation's longhands back into its shorthand's text (`compose_animation`, `contract/lower/src/svg.rs`), and a class's `animation-timing-function` outlives a node's `animation` there, so an unsaid easing needs keeping through both before the rule can reach them. Should a later revision take them on?

## 5. Stages

1. The `transition` parse, the wire's code 9, the runner's patches and the control points in `exact-motion`, `ease` until a host sets them. Red/green tests: `opacity 300ms`, on the wire and as a computed row (`transition=(open ? "opacity 300ms" : "none")`), keeps its easing unsaid and plays the process's default natively and on the JS and wasm targets; `opacity 300ms ease` keeps `ease`; `opacity` alone starts nothing; `-exact-spring(…)` and keyframe animations are unchanged.
2. The Apple hosts take the control points from Core Animation on iOS, tvOS and macOS, with a test that an unsaid easing plays them, through Core Animation and in the engine's samples, a hand-over and the agent's clock included. Then docs (D7).

Each stage lands with red/green tests and the iOS, macOS and tvOS suites.

## 6. Measurements

A probe app on an iPhone 18 Pro simulator with iOS 27.0 (SDK 27.1), and a probe on macOS 27.0.1, 2026-10-18:

1. Core Animation's named timing functions: `default` is `(0.25, 0.1, 0.25, 1)` and `easeInEaseOut` is `(0.42, 0, 0.58, 1)`.
2. `UIView.animate(withDuration: 0.3)` with no options added a `CABasicAnimation` eased by `(0.42, 0, 0.58, 1)` for `alpha` (`opacity`) and `backgroundColor`, and an additive one with the same curve for `transform`, `center` (`position`) and `bounds` (`bounds.size`).
3. `UIViewPropertyAnimator(duration: 0.3, timingParameters: UICubicTimingParameters())` added animations eased by `(0.25, 0.1, 0.25, 1)`; `UIViewPropertyAnimator(duration: 0.3, curve: .easeInOut)`, by `(0.42, 0, 0.58, 1)`.
4. A plain layer's implicit opacity animation was eased by `(0.25, 0.1, 0.25, 1)`, also when it was a sublayer of a view changed inside `UIView.animate(withDuration: 0.3)`, whose view got `(0.42, 0, 0.58, 1)`.
5. macOS: in `NSAnimationContext.runAnimationGroup` with a duration of 0.3 and no timing function, the animator's opacity and position animations were eased by `(0.25, 0.1, 0.25, 1)`, and `NSView.animation(forKey: "alphaValue")` returned that curve.
6. `Transitions::parse` then `Transition::starts` on origin/main: `""` and `none` have no declarations; `opacity` and `opacity 0s` do not start; `opacity 300ms`, `300ms` and `opacity 0s 100ms` start.
