---
name: 20261005-x18-svg-path-animation
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-composer-fidelity]
upstream_url: https://github.com/ccheever/exact2/issues/123
reproduced_on: null
---

# X18: An SVG path's `d` cannot be animated, so icons cross-fade where T3 Code morphs them

## Summary

T3 Code morphs an icon into the next one (copy into check, send into queue, chevron into chevron) on a short
spring, and snaps instantly when the user prefers reduced motion. Exact2 cannot interpolate one SVG path into
another, so the clone cross-fades the two glyphs (and turns chevrons, which are the same shape rotated). The
end state is the same; the frames in between are not. Needed: a transitionable `d` on SVG `path`.

## Why this issue arose

### The T3 Code behavior
- `MorphIcon` wraps the `morphicons` package (`apps/web/package.json:54`, `^1.7.1`;
  `apps/web/src/components/MorphIcon.tsx:4`), which "renders lucide icon data and morphs between shapes when
  `icon` changes", with `reducedMotion="user"`. Introduced by commit `f2cc80a7a4` ("morph composer and panel
  action icons", 2026-10-03). 20 files, 68 references.
- Examples: copy to check for 2 s after a copy (`components/chat/MessageCopyButton.tsx:52-55`; also code-block
  and diff-path copy buttons); the Send button's icon between "queue" (`ListPlus`) and "steer"
  (`CornerUpRight`) while a turn runs (`components/chat/ComposerPrimaryActions.tsx:314`); panel and tab
  controls (`PanelLayoutControls.tsx`, `RightPanelTabs.tsx`); disclosure chevrons; lock and unlock.
- Timing, as the clone records it from the package: a "snappy" spring (stiffness 420, damping 30, mass 1;
  slight overshoot; settled in about 0.35 s), interruptible mid-flight, instant under reduced motion
  (`shell-morph.contract:1-7`). The package source was not read for this plan.

### What exact2 does today
- GAPS summary row X18 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked
  against `main` `d2cb661eb`): "SVG path `d` animation" / "Morphing icons" / host / workaround "cross-fade".
  GAPS has no detail section.
- Bundled library, topics `motion` and `capabilities` (revision `fd1c879072aa836bcd2002a1e41aa83c7607e836`):
  the supported mechanisms are explicit property transitions, keyframes, spring timing and preference-driven
  policy; `capabilities` lists "General width/layout-property interpolation: Unsupported". SVG path `d` is not
  covered: unknown.
- Observed in the clone (mc-orch, 2026-10-05): the cross-fade code in `shell-morph.contract`.

### Where the clone hits it
- `shell-morph.contract`: `MorphPair` (the outgoing glyph scales to 0.5 and fades while the incoming grows
  in, both on `spring(420, 30, 1)`, no motion when `still`) and `MorphChevron` (one chevron rotated 90° or
  180° on the same spring). Used by the toast copy button and chevron (`shell-toast.contract:98,132`), the maximize/minimize
  toggle (`r4-surfaces.contract:109`, `shell-panels.contract:66`), the telemetry and appearance-editor
  disclosures (`settings-a-telemetry.contract:244`, `settings-appearance-editor.contract:39`) and the provider
  lock (`providers.contract:441`, "drawn as a quick cross-fade of the two glyphs").
- What a user sees differently: two overlapping glyphs for about 0.3 s instead of one shape flowing into the
  other. The final frame and the reduced-motion behavior match. Not every reference call site uses a pair
  yet: the Send icon swaps `list-plus` and `corner-up-right` by name (`composer-controls.contract:299-300`);
  the other reference call sites (tab controls, worktree card, device rail) were not checked.
- Cost: each pair needs hand-written scale and opacity values, and congruent shapes (chevrons) are special
  cases, which will not extend to unrelated shapes such as `ListPlus` to `CornerUpRight`.

## Why it must be resolved

The goal is a complete clone, and a declared difference is not an end state. Icon morphs are the most
repeated micro-interaction in the composer and panel controls (68 references): every send, copy, collapse and
mode change. A cross-fade reads as a different, heavier effect and cannot reproduce the shape flow, so it
shows in any motion capture compared against the oracle. No plan ticket waits for it; integrated acceptance
carries the difference, and the reference call sites not yet converted stay unconverted until the support
exists.

## Requested support

The web way: SVG 2 / CSS Shapes make `d` a CSS property on `path` (`d: path("…")`), and Chrome interpolates it
between two `path()` values that have the same command structure (otherwise it flips at the midpoint). Request,
on the macOS host first:
- **A (recommended).** `d` animatable on SVG `path` through the `transition` property with spring timing, for
  example `transition="d spring(420, 30, 1)"`: the host interpolates the two paths command by command when
  their structure matches (lucide icons used by `morphicons` are authored to match; to confirm), and flips
  otherwise. Honors the app's reduced-motion policy (`exactViewport().prefersReducedMotion`, library topic
  `motion`). Interruptible: a new target starts from the current shape.
- **B.** An app-side native drawer: a hook that draws the icon with a Core Animation shape layer and animates
  `path` between two strings. Works only for call sites the app owns and needs a hook per icon site; no
  framework change, but it duplicates what an SVG row would give.
A also benefits other hosts; B stays on macOS.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Minimal app: an `svg` with one `path`; a button flips a state that switches `d` between the lucide `Copy` and
   `Check` data with `transition="d 350ms ease"`. Expected (Chrome): intermediate shapes. Actual: the path
   jumps (to confirm; GAPS says it cannot animate).
2. Clone: copy a toast error text, or copy a code block; capture
   `screenshot copy.apng over 400 every 40` and compare with the oracle capture of the same copy button.
   Expected: one flowing shape. Actual: two glyphs overlapping.

## Acceptance for the fix
- A conformance case against Chrome: `Copy` to `Check` at 0, 25, 50, 75 and 100 % of a linear transition;
  host crops equal Chrome's within a fixed threshold; a case with different command structure (flip); a
  reversal at 50 %.
- Reduced motion: with the preference on, the end state appears in one frame.
- Clone: the toast copy button and the Send icon film frame-by-frame at 40 ms steps within the oracle's
  tolerance; no stuck intermediate state after rapid taps (agent: tap twice with `clock +60`).

## App adoption after resolution
- Replace `MorphPair` with a single `path` and `d` transition where both shapes share structure; keep
  `MorphChevron` only if the reference also rotates (verify against the package).
- Convert the call sites that still swap (the Send icon first, then any found in the call-site check) and add their rows to the
  tickets that own them; remove the "cross-fade" notes in `shell-morph.contract`, `providers.contract` and the
  README. `issue-close` verifies the conformance case and the film comparison.

**Adoption owner.** `20261005-composer-fidelity` owns the Send / Stop icon. Other call sites
found by the call-site check go into a follow-up ticket `20261005-adopt-x18-svg-morph` that
`issue-close` creates when the fix lands (planned plan revision).

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval;
publication only after approval).

## Resolved upstream and adopted (2026-10-07)

Filed as [#123](https://github.com/ccheever/exact2/issues/123); fixed by main PR #188 (`74affc099`: a path's `d` transitions on macOS, iOS and Linux when both ends have the same command list), merged into the feature branch with main `cff90b364` by task [20261007-adopt-main-fixes-r3](../tasks/20261007-adopt-main-fixes-r3.md).

How the reference morphs: `MorphIcon` (`apps/web/src/components/MorphIcon.tsx`) is morphicons 1.7.1 `MorphIcon` with `reducedMotion="user"`, the default "snappy" spring (k 420, c 30, m 1), over lucide-react 0.564.0 icon nodes. morphicons resamples both icons to 64 points per subpath, pairs the subpaths (`buildPlan`) and interpolates each pair in polar form (`interpPolar`): the centroid drifts, the subpath turns by θ about its pivot and scales by σ^t, while its centred shape moves linearly to the target's. At rest it snaps to the target's real curves.

What the clone draws now (`shell-morph.contract`, generated from morphicons' own plan for each pair): one box per plan item that drifts (`translate`) and turns (`rotate`, about the item's pivot as `transform-origin`) on `spring(420, 30, 1)`, holding a `path` whose `d` moves on the same spring from the source polyline to the target's σ-scaled, unturned polyline (64 points, same `M L…` list at both ends, so #188 interpolates it). The end shape equals morphicons' t = 1 output (checked to 0.0001 units in the generator). Differences: the scale moves linearly in `d` where morphicons scales by σ^t (largest mid-flight for copy → check, σ 0.53); at rest the glyph stays the 64-point polyline where morphicons snaps to the real curves (sub-pixel at 12–16 pt).

| Site | Before | After |
| --- | --- | --- |
| Toast copy (`shell-toast.contract` CopyErrorButton, reference `ui/toast.tsx` Copy → Check, `text-success`) | `MorphPair`: two glyphs crossing over (opacity + scale) | `MorphCopyCheck` (2 subpaths); colour switches at once, as the reference's class does |
| Right panel maximize (`r4-surfaces.contract`, `shell-panels.contract`; reference `PanelLayoutControls.tsx` Maximize2 ↔ Minimize2) | `MorphPair` cross-over | `MorphMaximize` (4 subpaths; two arrowheads turn 180°) |
| Provider env variable lock (`providers.contract`; reference `ProviderInstanceCard.tsx` LockOpen ↔ Lock) | 180 ms opacity cross-fade | `MorphLock` (shackle closes, 27° turn) |
| Composer steer ↔ queue (`composer-controls.contract`; reference `ComposerPrimaryActions.tsx` CornerUpRight ↔ ListPlus) | 220 ms opacity + scale cross-fade | `MorphQueueSteer` (5 subpaths) |
| Markdown table expand (`markdown.contract` TableIconButton; reference `ChatMarkdown.tsx` Maximize2 ↔ Minimize2) | instant icon swap | `MorphMaximize` at 12 pt |
| Chevrons (`MorphChevron`: toast details, appearance editor, telemetry) | rotation on the spring | unchanged: for a chevron pair morphicons' plan is a pure turn (θ 180° or 90°, σ 1), which the rotation already draws |

Not converted: the code block copy (`markdown.contract` mdCheck/mdCopy keyframes) and the welcome command copy (`pages-welcome.contract`), whose Check → Copy return after 1.2 s / 1.5 s runs as a keyframed animation (data sources have no timers, X19), and keyframed `d` is still refused on main (#188 builds `d` under `transition` only). The provider lock and the composer steer/queue morph run under Reduce Motion too (the component has no `viewport`; the old cross-fades did the same); the toast, panel and chevron morphs honour it.
