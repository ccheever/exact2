# Code review: LLP 1053.000.000 as built (a80e0608e), 2026-10-02 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one fresh session with `--cwd` the worktree at `a80e0608e` (`stopReason: end_turn`).
- **Method:** the shared brief (sha256 `505b2c914c86f26c362ca635570e42ca4a596b5e1c03284ea4ba6ab49331c544`) behind the fixed preamble of tool rules (whole prompt sha256 `7954d874b3b440c236afa5f8de5d722fdf51cf528e086332227bbb73423fbfae`); one round; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the joined `text` events, unedited; the first four sentences are progress narration.
- **Verdict:** DO NOT LAND.
- **Disposition:** every finding checked in the source and fixed before landing, in the commit after `a80e0608e`: focus kept through the clip-box and scroll moves (`BoxShadow.swift`, both `syncScroll`s, through `GlassGroups.moving`); a glass inside an isolated glass answers to that isolation (`glassPath` stops at the nearest container — the group, an isolated iOS slot, a macOS `GlassIsolationView` owned by its glass — and the pass repeats until nothing changes; the agent's `glassGroupOf` looks past isolation); every running opacity animation under a group is switched to sampling each receipt (`svg_lower::glass_sampling`), not only the touched ones; a canvas overlay is always a boundary on the path (its alpha changes between batches); `NSClipView` clips; the misplaced doc comment moved. Regression tests: `testFocusSurvivesAClipBoxComingAndGoing`, `testAFieldKeepsFocusThroughAClipBox`, `testAGlassInsideAnIsolatedGlassIsIsolatedByItsOwnPath` (both platforms), `a_lowered_opacity_animation_under_a_group_is_switched_to_sampling`. Leaving a group does not lower a node's animations again (declared in the code).

---

I'll audit commit `a80e0608e` against the glass-groups plan, staying inside this checkout and reading the code myself.The commit is large; I'll read `GlassGroup.swift` and the call sites it changes, then check the surrounding code it didn't touch.The plan and the new Swift file are in hand. Next I’ll check the call sites the commit didn’t rewrite—scroll, pool, navigation, and hit testing—where a group view can be left behind.The audit is already gathered. I am checking the line numbers the findings rest on, then I will write the verdict.Audit of commit a80e0608e (`glassGroup`). Read-only, against the code as built.

## 1. Blocker — a focused field is resigned by clip-box and scroll reparents, and nothing restores it

`GlassGroups.moving` (`host/apple/Sources/ExactKit/GlassGroup.swift:51-78`) is the only place that records the first responder (on macOS, the field editor's `NSTextField`) and puts it back after a move. `syncGlassGroup` uses it when it installs or removes the group view (`GlassGroup.swift:198-200`, `223-226`, `326-328`, `349-352`). Three other reparents move that same subtree with `removeFromSuperview` and never call it. Taking a view out of the hierarchy is what resigns the responder; the helper exists because that happens. After these paths, the field is back in a window and nobody calls `becomeFirstResponder`.

The clip box does it on both platforms. `BoxShadow.swift:140` and `:144` move every `NodeView` child and the `glassGroupView` into or out of the box. `syncGlassGroup` then sees `group.superview === base` (`GlassGroup.swift:207-209` on iOS, `:334-337` on macOS) and does not move again, so `moving` never runs. The group is moved out before `box.removeFromSuperview()` (`BoxShadow.swift:144-145`), so the children are not destroyed with the box. Focus is.

Scroll removal does it on iOS. `NodeViewIOS.swift:1105-1106` moves the scroll's `NodeView` children back onto the node, then `:1124` calls `syncGlassGroup`. While the node was scrolling the group was absent (a scroll is a conflict, `GlassGroup.swift:97`), so this install is the first time `moving` runs, and the responder is already gone. Scroll creation on iOS is fine: children stay inside the group until `removeGlassGroup`, which uses `moving`, and `baseContainer` is the scroll.

Both directions do it on macOS. Creation (`NodeViewMac.swift:1059`) reads `container`, which is the group content view when a group exists (`NodeViewMac.swift:525`), and pulls the children out of it. `syncGlassGroup` (`:1088`) then removes an empty group and cannot restore a responder that already resigned. Removal (`:1066`) puts the children on `overlay ?? materialContent ?? self` the same way, and the later install is too late.

Failing input, clip box: a `row glassGroup=12` containing a focused `input`, then the row gains `overflow: hidden` and a box shadow (the clip-box condition at `NodeViewIOS.swift:1122` and `NodeViewMac.swift:1083`). Removing the shadow drops focus again.

Failing input, scroll: a node whose `glassGroup` and `overflow` are bound (Contract only refuses a literal `"scroll"`), with a focused `input` inside. On macOS, overflow becoming `scroll` drops focus. On both platforms, overflow returning to `visible` drops it.

`testFocusSurvivesTheGroupComingAndGoing` (`GlassGroupIOSTests.swift:154-162`) and `testAFieldInAGlassKeepsFocusThroughItsIsolation` (`GlassGroupMacTests.swift:100`) only add and remove the prop, and isolate. They never reparent through a clip box or a scroll.

Fix: perform the moves in `syncClipBox` and in both `syncScroll` implementations inside `GlassGroups.moving(in: self)`. On macOS scroll creation, move the group view (or leave the children in it and let `removeGlassGroup` move them); do not pull them out of `container` first.

## 2. Should-fix — an opacity animation that is already lowered is not resampled when an ancestor gains `glassGroup`

Sampling is decided only for nodes in the current `MotionSync`. `svg_lower::eligibility` iterates `sync.animations` (`host/apple/src/svg_lower.rs:292`) and forces sampling when the node animates opacity and an ancestor has the prop (`:328`, `in_glass_group` at `:421-429`). The host calls that on boot (`host/apple/src/host.rs:461`) and once per receipt (`:1246`), after `motion_sync`. `motion_sync` (`kernel/src/motion.rs:602-633`) includes created and touched nodes only; descendants are added only when `display` changes. `set_node_sampled` (`motion/src/engine/animate.rs:184-192`) dirties properties only when it is actually called and the forced set changes.

A lowered opacity animation never reaches the host as a value. `glassPath` reads the model `alpha` / `alphaValue` (`GlassGroup.swift:118`, `:123`). Core Animation is playing the presentation layer, so the path looks clear, the glass is not isolated, and it draws fully opaque and merges for the whole fade.

Failing input: a glass button whose opacity animation is already running, and no group anywhere above it. Then its parent sets `glassGroup=12`. The parent is the touched node; the button is not in that receipt's `animations`, stays lowered, and row 8 of the plan fails. The reverse leaves the button sampled after the group is cleared: the pixels stay right, and it does not go back to lowering.

`opacity_inside_a_glass_group_is_sampled` (`host/apple/src/box_motion_tests.rs:250-298`) starts the animation while `glassGroup=12` is already on the parent, and it never clears the group. That case is true. It is the case the build step required beyond that one.

Fix: when `glassGroup` is set or cleared, recompute eligibility for descendants that animate opacity (or walk every live opacity animation, not only the receipt). Then `set_node_sampled` will dirty the property and the next frame will present `alpha`.

## 3. Should-fix — on macOS a nested glass stops at the ancestor's isolation container and is never isolated itself

`glassPath` returns at the first superview that is a `GlassGroupView`, and the group it reports is that view's `owner` (`GlassGroup.swift:122`). The real group sets `owner` (`:318`). The isolation container is also a `GlassGroupView`, and `reconcileGlass` never sets `owner` (`:384-395`). `isolated` is `group != nil && !reasons.isEmpty` (`:381`), so a nil owner discards whatever reasons were collected and isolates nothing.

iOS isolation is a `GlassSlot`, not a `GlassGroupView` (`GlassGroup.swift:177`, `:254-266`), so the walk continues to the real group.

On macOS the children of a glass node live in the material, and isolation moves that material into the container (`:391-393`). A glass inside that material is therefore under the isolation view.

Failing input:

```
row glassGroup=12
  box opacity=0.4
    box backgroundMaterial="glass"
      box overflow="hidden" width=40
        box backgroundMaterial="glass" width=80
```

The outer glass isolates because of the faded ancestor. The inner glass's walk stops at that isolation container, `group` is nil, and the clip between them is never given its own container. That clip sits between the nearest container and the inner glass, which is the case the platform does not honour, so the inner glass draws past the 40-point box. `layout` also omits `native.glassGroupOf` for it (`GlassGroup.swift:149-151`).

Fix: do not use `GlassGroupView` for the isolation container, or mark isolation containers so `glassPath` skips them and keeps walking to the view whose `owner` is set.

## 4. Should-fix — macOS `glassPath` does not treat `NSClipView` as clipping

The path notes a clip only for `clipsToBounds` or `layer.masksToBounds` (`GlassGroup.swift:125`). The same target's own visibility walk treats a scroll clip as clipping without those flags: `current is NSClipView || current.clipsToBounds || current.layer?.masksToBounds == true` (`VideoModule.swift:275`). A scroll between a group and a glass clips through `NSScrollView`'s `NSClipView` (`ChainingScrollView`); nothing in that type sets `clipsToBounds`. iOS is covered because `UIScrollView` clips its bounds, and the iOS arm checks `clipsToBounds` (`GlassGroup.swift:120`).

Failing input: the fixture's `scroll-in-group` row, a scroll between the group and the glass. If that clip view has neither flag, the glass is not isolated and draws past the scroll (plan row 11). Section 7 says that row was compared by eye on macOS 27 and looked clipped, so this fires only when the flags are absent. The predicate is still the wrong one for this target.

Fix: `note("clip")` when the view is an `NSClipView`, matching `VideoModule.swift:275`.

## 5. Should-fix — canvas `each` placement alpha changes outside the batch, and isolation is not recomputed

Reconcile runs once per batch, after `captureIfNeeded` (`PresenterIOS.swift:792` then `:810`; `PresenterMac.swift:955` then `:966`). A non-`each` capture sets `overlay.alpha = 0` inside that capture (`GpuIOS.swift:456`, `GpuMac.swift:341`), so the batch's reconcile sees it.

`each` mode sets the placed child's alpha from the placement outcome on every render tick and every readback: `child.alpha = outcome == 0 ? 1 : 0` (`GpuIOS.swift:380`, called from `:520` and `:667`) and `child.alphaValue` the same way (`GpuMac.swift:282`). Those calls are not followed by `glassGroups.reconcile()`. `refreshChildren` also writes those alphas when `each` flips (`GpuIOS.swift:473`).

Failing input: a `glassGroup` ancestor of a canvas that places children (`each`), and a glass inside a placed child. The child goes to alpha 0 so it is not drawn twice. No later kernel batch runs while frames continue. `glassPath` keeps the last decision, the platform ignores that alpha, and the glass keeps compositing at full opacity over the canvas texture.

Fix: reconcile at the end of `readPlacements` and when placement alphas are cleared. The emptiness test in `GlassGroups.reconcile` (`GlassGroup.swift:33-35`) already makes that free for an app that has never had a group.

## 6. Nit — `in_glass_group` still carries the box-filter comment

`host/apple/src/svg_lower.rs:415-417` is the doc comment for `under_box_filter`, sitting directly above `in_glass_group` (`:421`). The real `under_box_filter` starts at `:432`. Move the comment down.

DO NOT LAND
