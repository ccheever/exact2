# LLP 1061: Press feedback, motion at the panel's rate, and the user's motion preference

**Type:** RFC
**Status:** Implemented 2026-09-26 (web and iOS verified; see §Verified)
**Systems:** Kernel (style row bit 105, `press_scale`); Contract (`press-scale`); Runner (`Viewport.preferences`, `set_preferences`, two `exactViewport` fields); Apple host (`exact_set_preferences`, press feedback on UIKit, the display link's rate policy); Web host (`--exact-press`, the shell's `:active` rule, the page's media queries through `exact_boot`/`exact_resize`)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002 §4 and `rules/NOT-DOING.md` §Motion (reduced motion is the app's choice, not the engine's); LLP 1057 D7 (animations inherit that); LLP 1039 (`exactViewport`, the fact this extends); LLP 1027.000.000 (a host fact told after boot, as the date is); LLP 1009 D4 (frames only while something moves)

## Summary

grnl's design system asks three things of the platform a designer cannot
build in Contract: a press that gives under the finger (0.97 for buttons,
0.994 for cards), motion that runs at 120 Hz on a ProMotion iPhone, and a
Reduce Motion setting that collapses every duration to zero and turns the
press off. This adds one host-owned style row, one rate policy, and two
fields on the viewport fact:

```
keyframes rise
  from
    opacity=0
    translate="0px 8px"

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

**D1 — `press-scale` is a style row, not CSS.** Bit 105, `f32`, default 1
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

**D2 — On UIKit the host owns the press, folded into the engine's transform.**
`NodeView.pressed` (already the tap's own state) drives a `PressFeedback`: a
factor eased from where it is to the target over 120 ms on
`cubic-bezier(.16, 1, .3, 1)`, re-aimed without a jump when released mid-ease.
`applyTransform` — the one function every writer of `UIView.transform` goes
through, the Rust engine's `present` ops and the frame op's untransformed
relayout included — multiplies the `scale` row's presentation value by the
factor. So an engine write mid-press keeps the press, a press mid-transition
keeps the engine's value, and an idle press is exactly 1 (the transform ends
where the engine left it, identity included, which the text raster relies on).
`PressClock` is one `CADisplayLink` for every session, alive only while some
press eases. Touch-down scales on the next frame; nothing waits on the runner.
The finger leaving the box releases the feedback and re-entering presses
again, tested against the box as it stands unpressed so a finger resting
between the pressed and unpressed edges cannot flicker; the tap's acceptance
uses the same test. A pan still cancels the touch (`touchesCancelled`), which
eases back. *Rejected:* a Core Animation animation on `transform` (it
overrides the engine's model writes while it runs); `layer.sublayerTransform`
(it does not scale the node's own background and border); a UIKit-private
layer between the view and its content (every node would pay for it).

**D3 — On the web it is CSS, composed rather than replaced.** The row emits
`--exact-press:<n>` and a `transform 0.12s cubic-bezier(0.16,1,0.3,1)` entry
appended to the node's own `transition` list (last, so it beats an authored
`all`); the shell's stylesheet holds one rule,
`[style*="--exact-press"]:active:not(:disabled) { transform: scale(var(--exact-press)) }`,
inside `@media (prefers-reduced-motion: no-preference)`. `transform` is no
row's — the motion rows are CSS's individual `translate`/`scale`/`rotate`
properties — so it composes with them and with the engine's animations of
them.

**D4 — Motion runs at the panel's full rate while it runs.** `Frames.run`
already asked for `CAFrameRateRange(80, max, max)` for a canvas; motion now
asks the same whenever `batch.motion` is true (a transition, an animation, a
held or springing value). The link exists only while something wants frames,
so an idle app drops to no link at all; a link kept only for a timer stays at
`.default`. `CADisableMinimumFrameDurationOnPhone` was already set.

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

UIKit's press feedback reads `UIAccessibility.isReduceMotionEnabled` itself
at touch-down: it is host-owned feedback, so the host honours the setting.
*Rejected:* a new `exactPreferences()` source (a second copy of the viewport's
bake, TypeScript and receipt handling for two booleans); an engine-level
reduced-motion switch (`NOT-DOING`).

## What an app does with it

Every duration an app authors can collapse on one derive:
`transition=(still ? "none" : "…")`, `animation=(still ? "none" : "…")`, and
`press-scale` needs nothing (the hosts drop it). A `style` that holds a
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

## Known gaps

- **macOS and Linux show no press.** AppKit's `NodeView` has no press state to
  drive it (a click is `mouseDown`/`mouseUp` there); Linux is headless.
- **Web `:active` is not "still inside".** A browser keeps `:active` while the
  button is held even after the pointer leaves; UIKit releases the feedback.
  Nested nodes that both carry the row both scale on the web (`:active`
  matches ancestors); on UIKit only the node that took the press does.
- **Inside a scroll view, UIKit delays `touchesBegan`** (~150 ms,
  `delaysContentTouches`) so a pan never flashes a press, as it does for
  `UIButton`; a quick tap there shows almost no feedback.
- **An `infinite` animation keeps the display link at 120 Hz** for as long as
  it runs; the batch says "motion", not which kind.
- **The agent's `tap`** activates without touches (LLP 1012), so it never
  shows a press; only a real touch does.
- **No agent operation sets a preference** (the eight operations are closed,
  `NOT-DOING`); tests set it through the runner, and a device through its
  accessibility settings.
