# Code review: LLP 1053.000.000 as built (a80e0608e), 2026-10-02 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort max, read-only sandbox, `-C` the worktree `exact2-wt-glassgroup` at `a80e0608e`.
- **Method:** the shared brief (sha256 `505b2c914c86f26c362ca635570e42ca4a596b5e1c03284ea4ba6ab49331c544`) verbatim; one round; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** every finding checked in the source and fixed before landing, in the commit after `a80e0608e`: focus kept through the clip-box and scroll moves (`BoxShadow.swift`, both `syncScroll`s, through `GlassGroups.moving`); a glass inside an isolated glass answers to that isolation (`glassPath` stops at the nearest container — the group, an isolated iOS slot, a macOS `GlassIsolationView` owned by its glass — and the pass repeats until nothing changes; the agent's `glassGroupOf` looks past isolation); every running opacity animation under a group is switched to sampling each receipt (`svg_lower::glass_sampling`), not only the touched ones; a canvas overlay is always a boundary on the path (its alpha changes between batches); `NSClipView` clips; the misplaced doc comment moved. Regression tests: `testFocusSurvivesAClipBoxComingAndGoing`, `testAFieldKeepsFocusThroughAClipBox`, `testAGlassInsideAnIsolatedGlassIsIsolatedByItsOwnPath` (both platforms), `a_lowered_opacity_animation_under_a_group_is_switched_to_sampling`. Leaving a group does not lower a node's animations again (declared in the code).

---

1. **Should-fix — Creating or removing a clip box drops the focused editor.**

   **Evidence:** [BoxShadow.swift:140](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/BoxShadow.swift:140) explicitly removes `glassGroupView` from its superview before moving it; the reverse operation at line 144 does likewise. Neither uses `GlassGroups.moving`. The subsequent group synchronization finds the group already correctly placed and cannot recover the lost focus.

   **Failing scenario:** Focus an input inside `box glassGroup=12 box-shadow="0px 2px 4px #000000"`, then change its overflow from `visible` to `hidden`, for example on the first input event. Typing loses its editor. I reproduced the same move sequence in AppKit: the field had an editor before the move and none afterward. The new clip-box test checks placement but contains no focused input.

   **Fix:** Wrap the entire clip-box installation/removal in the focus-preserving helper, including attachment of the destination container. Test focus and selection through both transitions.

2. **Should-fix — An outer glass’s isolation disables required isolation for nested glass on macOS.**

   **Evidence:** [GlassGroup.swift:122](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/GlassGroup.swift:122) stops at any `GlassGroupView` and returns its owner. However, the isolation containers created at [GlassGroup.swift:384](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/GlassGroup.swift:384) have no owner. A nested glass therefore gets `group == nil`, and lines 381–383 remove its isolation even when its own path requires one.

   **Failing scenario:** `box glassGroup=12` contains a glass box at `opacity=0.5`, which contains another glass box at `opacity=0.2`. Once the outer isolation exists, the inner glass loses its isolation—at latest on the next reconciliation—and joins the outer container. Its own opacity is ignored and it can fuse with the outer glass. This Contract input compiles.

   **Fix:** Represent the encountered container separately from its authored group owner. An internal isolation container must remain a valid boundary for evaluating descendant opacity and clipping. Test nested glass across repeated reconciliations and both isolation/rejoin transitions.

3. **Should-fix — Changing group membership leaves existing opacity animations in the wrong executor.**

   **Evidence:** [svg_lower.rs:292](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/src/svg_lower.rs:292) evaluates eligibility only for `sync.animations`. [kernel/src/motion.rs:602](/Users/ccheever/projects/exact2-wt-glassgroup/kernel/src/motion.rs:602) populates that list from created/touched nodes; setting a group prop touches the parent, not its unchanged animated descendants.

   **Failing scenario:** Start with parent `P` containing glass child `C`, whose opacity keyframes run in Core Animation. A producer then sends only `SetProp(P, GlassGroup, 12)`. Swift installs the group, but `C` remains lowered. Its model alpha stays at 1, so reconciliation leaves it joined and the grouped glass ignores its animated fade. Removing a group similarly fails to reconsider unchanged descendants.

   **Fix:** Reevaluate affected animated descendants when group props or ancestry change, and invalidate their emitted animation specifications when switching executors. The current [opacity test](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/src/box_motion_tests.rs:263) changes the child’s animation under an already-existing group, so it misses this case.

4. **Should-fix — Canvas changes outside a presenter batch leave isolation stale.**

   **Evidence:** Reconciliation runs at [PresenterIOS.swift:810](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:810) and [PresenterMac.swift:966](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:966). Canvas loading can happen afterward at [Session.swift:901](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/Session.swift:901). Capture then sets the overlay’s alpha to zero at [GpuIOS.swift:456](/Users/ccheever/projects/exact2-wt-glassgroup/host/apple/Sources/ExactKit/IOS/GpuIOS.swift:456), without another reconciliation. Canvas placement updates likewise change alpha outside batches.

   **Failing scenario:** Put a `canvas surface=aurora("1")` containing a glass box beneath `box glassGroup=12`, with visible overflow. This compiles. The initial pass joins the glass; subsequent module loading captures and hides the overlay. The glass remains joined, so the outer group ignores the overlay’s zero alpha and continues drawing it until another presenter batch arrives.

   **Fix:** Reconcile affected glass after canvas capture and placement changes, including initial module loading, before presenting the resulting frame. Test this without an intervening runtime batch.

The five existing macOS glass tests, Contract test, and Rust opacity test passed. No files were changed.

LAND WITH FIXES
