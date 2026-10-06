# LLP 1061: Press feedback, motion at the panel's rate, and the user's motion preference

**Type:** RFC
**Status:** Implemented 2026-09-26; gaps closed 2026-09-27 (AppKit press, the web's pointer rule, the scroll view's quick tap, the rate by what moves, the agent's `prefer`, `transform-origin`; see §Verified)
**Systems:** Kernel (style rows bit 147 `press_scale`, bit 120 `transform_origin`, LLP 1055.000's); Contract (`press-scale`, `transform-origin`); Motion (`Engine::spatial`); Runner (`Viewport.preferences`, `set_preferences`, two `exactViewport` fields); Apple host (`exact_set_preferences`, press feedback on UIKit and AppKit, the display link's rate policy, the batch's `spatial`); Web host (`--exact-press`, the input glue's `data-pressed`, the page's media queries through `exact_boot`/`exact_resize`); Linux host (`transform-origin` in the painter, `prefer`); agent driver (`prefer`, LLP 1012)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002 §4 and `rules/DEFERRED.md` §Motion (reduced motion is the app's choice, not the engine's); LLP 1055 D11 (animations inherit that); LLP 1039 (`exactViewport`, the fact this extends); LLP 1027.000.000 (a host fact told after boot, as the date is); LLP 1009 D4 (frames only while something moves)


**Ruled (Charlie, 2026-09-27, after the review of PR #47):** `press-scale` stays a declared non-CSS host-feedback row (LLP 1001). This amends D3 and D5:
- The press never writes CSS `transform`. On the web it composes through CSS's `scale` property (the row's scale × the press), as Apple already does, so an authored `transform` (on SVG today, and the owed HTML row) survives a press.
- The press is kept under reduced motion. A native button's highlight isn't skipped under Reduce Motion, and a shrink under the finger is feedback, not motion.

## Summary

grnl's design system asks three things of the platform a designer cannot
build in Contract: a press that gives under the finger (0.97 for buttons,
0.994 for cards), motion that runs at 120 Hz on a ProMotion iPhone, and a
Reduce Motion setting that collapses authored durations to zero while
keeping the press feedback. This adds one host-owned style row, one rate policy, and two
fields on the viewport fact:

```
keyframes rise
  from opacity=0 translate="0px 8px"

shape Media
  prefersReducedMotion: bool
  prefersReducedTransparency: bool

component Feel
  resource media = exactViewport() as shape Media
  derive still = media.prefersReducedMotion
  view
    column animation=(still ? "none" : "rise 320ms cubic-bezier(0.32, 0.72, 0, 1) both")
      button press=record press-scale=0.97 transition=(still ? "none" : "opacity 120ms cubic-bezier(0.32, 0.72, 0, 1)")
        text "Record"
```

`contract/corpus/motion-feel.contract` is this, compiled and run by the tests;
`apps/interaction-gallery` uses all three.

## Decisions

**D1 — `press-scale` is a style row, not CSS.** Bit 147, `f32`, default 1
(none), not inherited, no layout. CSS has no row for it — `:active` is a
selector, and Contract has no selectors — so the feedback is named for what it
shows. It is set like any row (literal, expression, `style`, class choice);
a literal ≤ 0 is refused (`lower-attr-value`); it is not a keyframe or
`transition` target, since the host, not the engine, animates it
(`lower-keyframes` refuses it by name). It applies to a node the host
presses: one with a `press` handler. *Rejected:* an `active` pseudo-state
block in `style` (a selector system for one property); a `transition` on the
existing `scale` row driven by a runner state (a round trip per touch-down,
and it would fight any authored `scale`).

**D2 — On Apple the host owns the press, folded into the engine's transform.**
`NodeView.pressed` (already the tap's own state) drives a `PressFeedback`: a
factor eased from where it is to the target over 120 ms on
`cubic-bezier(.16, 1, .3, 1)`, re-aimed without a jump when released mid-ease
(`PressFeedback.swift`, one file for both platforms).
`applyTransform` — the one function every writer of `UIView.transform` goes
through, the Rust engine's `present` ops and the frame op's untransformed
relayout included — multiplies the `scale` row's presentation value by the
factor. So an engine write mid-press keeps the press, a press mid-transition
keeps the engine's value, and an idle press is exactly 1 (the transform ends
where the engine left it, identity included, which the text raster relies on).
On iOS (2026-10-02) the ease is Core Animation's, with no frame of it on
the main thread: `applyTransform` folds the press's *target* into the model at
once, and an additive `transform` animation eases the difference from the
factor on screen to nothing, on the render server, on the same curve and
duration. Additive, it composes under every model write, so an engine write
mid-ease keeps the press as before; a re-aim replaces it from what shows. The
held release and the return to idle are each one timed callback. On macOS
`PressClock` is one `CADisplayLink` for every session, alive only while some
press eases. Touch-down scales on the next frame; nothing waits on the runner.
The finger leaving the box releases the feedback and re-entering presses
again, tested against the box as it stands unpressed so a finger resting
between the pressed and unpressed edges cannot flicker; the tap's acceptance
uses the same test. A pan still cancels the touch (`touchesCancelled`), which
eases back. Inside a scroll view UIKit delays a touch (`delaysContentTouches`)
and hands a quick tap its `touchesBegan` and `touchesEnded` in one turn, so
the press would never reach the screen; a `UIButton` there still flashes its
highlight, since its release fades. A release that comes before the press was
ever presented (younger than one 60 Hz frame) is held until the press has
eased in, then eases back: the quick tap shows. **AppKit** (2026-09-27) is
the same state on the mouse: `mouseDown` presses, `mouseDragged` follows
inside or out (AppKit's conversion ignores the layer's transform, so the test
is already against the unpressed box), `mouseUp` releases, and a drag a
gesture takes (layout pan, transform drag, reorder, height drag, swipe) ends
the press as a pan cancels a touch. The layer's transform is composed about
the origin explicitly, as before. Under the agent's clock (`agentFreezes`)
the press lands without easing, as UIKit's animations are skipped, so a
`layout` or screenshot between `tap X down` and `tap up` is deterministic. *Rejected:* a non-additive Core Animation animation on `transform` (it
overrides the engine's model writes while it runs); `layer.sublayerTransform`
(it does not scale the node's own background and border); a UIKit-private
layer between the view and its content (every node would pay for it).

**D3 — On the web it is CSS, composed rather than replaced.** The row emits
`--exact-press:<n>`. A pressable node's CSS `scale` multiplies two registered,
non-inherited numbers: `--exact-scale` (the authored row's presentation) and
`--exact-press-factor` (1 when idle). The input glue eases the latter through
Web Animations over 120 ms on `cubic-bezier(.16,1,.3,1)`; nothing runs per
frame in JavaScript. Authored scale transitions and lowered springs address
`--exact-scale`; keyframes supply both that number and ordinary CSS `scale`,
so non-pressable nodes keep their normal CSS declarations. On a pressable
node the composed `scale` is important so a scale keyframe cannot replace
its multiplication. No press writes `transform`, including on SVG.

Which node is pressed follows UIKit's rule, not `:active`'s: only the
innermost node with a `press` handler under a primary `pointerdown`, never
its pressable ancestors; nothing when that node has no row or is disabled.
The glue marks `data-pressed` only while the pointer stays inside its
unpressed box, and releases on up, cancel or pan takeover. Re-pressing
while release eases out measures with only the feedback effect neutralized
and restored synchronously: the browser keeps the authored transforms and
undoes the feedback about the actual `transform-origin`, including SVG's
reference box. Reduced motion keeps the feedback. Under the agent's clock
the feedback lands immediately, as on Apple.

**D4 — Motion that moves things runs at the panel's full rate.** `Frames.run`
already asked for `CAFrameRateRange(80, max, max)` for a canvas; motion asks
the same whenever what moves changes place or size — `Engine::spatial`: a
running curve or live keyframe animation of `translate`, `scale`, `rotate`,
`height`, layout or a stroke's trim — which the Apple batch says as
`"spatial":true`. A fade or a colour change reads the same at 60 Hz, so
paint-only motion (an `infinite` 4.2 s breathing opacity that runs for
minutes) asks `CAFrameRateRange(30, 60, 60)` (2026-09-27). The link exists
only while something wants frames, so an idle app gets no callbacks; on iOS
the app's one link (`FrameClock`) is paused then rather than dropped
(2026-10-05). A link kept only for a timer stays at `.default`.
`CADisableMinimumFrameDurationOnPhone` was already set. *Rejected:* a rate
from an animation's speed (the engine knows no box size, so no pixels per
frame); a rate by iteration length (a slow spin still judders).

**D5 — The preferences are `exactViewport()` fields.** `prefersReducedMotion`
and `prefersReducedTransparency`, both `bool`, beside `width` and `height`:
CSS's `@media` answers the size and the user-preference features (Media
Queries 5 §11) from the same environment, and this reuses the fact's whole
path — filled by field name, refused at bake when unknown or mistyped,
device data the bake never fixes, re-answered in one commit
(`Runner::set_preferences`; `set_viewport` keeps them). The runner has no
policy: the app writes `none`, as a stylesheet would. Hosts:

- **Web:** `matchMedia` for both, passed to `exact_boot`/`exact_boot_plan` so
  the first frame is right, and with every `exact_resize`, which the glue now
  also calls on either query's `change`. A browser that does not know
  `prefers-reduced-transparency` (Safari) answers no preference, as CSS does.
- **Apple:** `UIAccessibility.isReduceMotionEnabled` /
  `isReduceTransparencyEnabled` (macOS: `NSWorkspace`'s
  `accessibilityDisplayShouldReduce…`) and their change notifications, through
  `exact_set_preferences(rt, bits)`. It is told after every boot in the same
  main-thread turn as the boot batch, as the date is (LLP 1027.000.000), so no
  frame shows the unreduced tree; changes arrive while the app runs.
- **Linux and the build-time renderer:** no preference; neither has a
  setting to read.

Press feedback remains visible with either motion preference (the
2026-09-27 ruling); the preference is still reported to the app.

An agent sets the preferences with `prefer` (LLP 1012, 2026-09-27), by CSS's
media feature names — `prefers-reduced-motion` and
`prefers-reduced-transparency` (`reduce`/`no-preference`) and
`prefers-color-scheme` (`dark`/`light`, the system's appearance, beneath an
app's own `setScheme`). The web emulates the media (`Emulation.setEmulatedMedia`),
so the page's queries, its CSS and the glue's listeners all see it; Apple
stands the agent's values in for the accessibility settings
(`DisplayPreferences.agent`) and tells the runner, and sets the window
scene's `traitOverrides.userInterfaceStyle` (iOS) or, while the app follows
the system, `NSApp.appearance` (macOS, which has no layer beneath the app's
own); Linux re-answers the runner and keeps a system scheme that
`setScheme("system")` follows.

**D6 — `transform-origin` is CSS's, in two dimensions** (2026-09-27). Bit 120,
main's row and type (`svg::TransformOrigin`, LLP 1055.000 D5), which SVG
elements and boxes share: one or two of `left`/`center`/`right`/
`top`/`bottom`, `px` lengths and percentages (keywords in either order, a
length always x then y), and a `z` of 0; initially `50% 50%` on a box. (This
branch had its own `origin.rs` with the same grammar; the merge kept main's
and added `resolve` and `centred` to it.) The web emits it canonical
(`0% 0%`); UIKit composes the transform about the origin as an offset from
the centre, its anchor (so `frame` keeps working); AppKit and the Linux
painter move the origin to the layer's own and back. The press folds into
`scale`, so it turns about the origin too. A transform drag (LLP 1041 §8)
refuses a target whose origin is not the centre. Not animatable: no host
moves it per frame, and no app asked. The runner reads a string ending in
`%` as a percentage only when all of it is one, as the compiler does, so
`"0 100%"` reaches the row.
*Rejected:* a new `exactPreferences()` source (a second copy of the viewport's
bake, TypeScript and receipt handling for two booleans); an engine-level
reduced-motion switch (`DEFERRED`).

## What an app does with it

Every duration an app authors can collapse on one derive:
`transition=(still ? "none" : "…")`, `animation=(still ? "none" : "…")`, and
`press-scale` needs nothing (the hosts keep its feedback). A `style` that holds a
`transition` is chosen with a class choice (`class=(still ? Still : Moving)`).
There is no stylesheet-wide switch: Contract has no media blocks, so a
reduced-motion app writes the condition where the motion is.

## Verified

- Kernel/Contract/runner: `contract/cli/tests/it/motion_feel.rs` (the row from
  a literal and an expression, the preference re-answered in one commit and
  kept across a resize, a mistyped field refused at bake); two reject
  fixtures; the corpus fixture round-trips the formatter.
- Web: `css.rs` (`--exact-press` and the merged transition list),
  `tests/it/host.rs` (a preference change rides the resize batch and drops the
  `animation` declaration). In headless Chrome 153 over CDP: a held mouse
  press on a gallery button read `matrix(0.974…)` at 40 ms and `0.97` held,
  `none` after release; emulating `prefers-reduced-motion: reduce` live
  changed the app's text in one batch and the press showed no transform; a
  boot under `reduce` shipped no `animation` declaration.
- Apple: `tests/it/viewport.rs` (the host re-answers and the app's animation
  row empties), `style.rs` (`press_scale` crosses to the presenter, the
  engine's `scale` does not), `PressFeedbackIOSTests` (the easing curve, the
  press folded into and surviving an engine `present` and a relayout, no
  feedback without the row; 34 UIKit tests pass on an iOS 26.5 simulator).
  The gallery on that simulator, read through the accessibility tree (which
  reports the transformed frame): a held finger on Reset read width 0.111 of
  the screen against 0.114 unpressed, centre unmoved; sliding off released
  it, sliding back pressed it again, lifting inside fired the tap; a card in
  the scroll view read 0.875 against 0.881 (0.994) after UIKit's touch delay,
  and a vertical drag cancelled it back to 0.881 without opening the photo.
  Turning Reduce Motion on (`com.apple.Accessibility ReduceMotionEnabled`)
  changed the app's text live through the notification, a held Reset stayed
  at 0.114, and a relaunch booted reading it. macOS builds and answers
  `prefersReducedMotion: false`. The 120 Hz range is not measurable on a
  simulator (60 Hz); it is unverified on hardware.
- 2026-09-27: `PressFeedbackMacTests` (a mouse down presses, a drag out
  releases and back presses, the click still fires; the layer turns about
  the origin, whose `top` is the screen's), `PressFeedbackIOSTests` (a release
  before the press was seen waits for it; the transform about the origin);
  `host/web/tests/press.test.mjs` in headless Chrome (only the innermost
  pressable, only inside, none under reduced motion); `motion/tests/it/cadence.rs`
  and `host/apple/tests/it/animation.rs` (a fade is motion, a slide is
  spatial); `kernel/src/origin.rs`, `css.rs`, `motion_feel.rs`, a reject
  fixture and the Linux painter's boxes (`transforms_turn_about_the_transform_origin`);
  `agent.rs`'s `prefer`. Driven: the web and Linux lay a plan's rotated and
  scaled boxes out identically (`20,40 106.6×84.64`, `90,160 50×20`); on the
  web a held `tap reset down` read 44.64 of 46.02 wide and released when moved
  away; `prefer prefers-reduced-motion reduce prefers-color-scheme dark`
  changed the gallery's text in one commit on the web, Linux and an iOS 26.5
  simulator (whose screenshot is dark), and the web's press then showed
  nothing; a quick tap on a card in the simulator's scroll view still opens
  it.

## Known gaps

- **Linux now has contact feedback (2026-09-27).** Its presenter contact
  path (evdev, VNC and seekable agent contacts) eases a separate factor into
  the painter's scale about `transform-origin`, kept under reduced motion.
  Leaving and returning uses the unpressed painted box; cancel or gesture
  takeover releases without activation. The engine's scale remains its own.
  The one-shot `tap` remains an activation; phases exercise the feedback.
- **Not a gap: the agent's one-shot `tap` on iOS shows no press.** UIKit has
  no public touch synthesis, so LLP 1012 declares it an activation
  (`delivery: "activation"`), as VoiceOver's is; a press is a touch's. The
  web's and macOS's `tap` are real input and now pass through the press, and
  a held press is observable with the contact phases (`tap X down`, `layout`,
  `tap up`) on the web, macOS and the simulator carrier.
- **120 Hz is unmeasured on hardware**, and the Mac's frame link asks no
  range for motion (QUEUE).
