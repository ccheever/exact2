# Code review: the 24 dp marks and the 48 dp touch target, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed (the read-only sandbox cannot start on `expo-build-1000`), a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `b66ee376d`.
- **Method:** one brief (sha256 `ea06c22aecc10e6e3d2deb7c6d4f6b3793b718c3437bcc0d7bfd2ec8ccc72982`), blind to the author's reasoning. The author (Claude) is not a reviewer.
- **Answered:** 1 and 5 fixed (a candidate must be hittable at its own middle: pointer-eligible, shown, not transformed away); 2 fixed (the touch target is the press's alone, after `hit`: hover, wheels and gestures keep the painted hit); 3 fixed (a link run under the press takes it); 4 fixed (the touch target decides on release, where the press is resolved, so a press below a control reaches it whatever its feedback); 6 and 7 answered by not widening the accessibility bounds (Charlie's "if it fits D13": it does not, §5 Q5); 8 fixed (an authored size draws across the box); 9 fixed (a disabled control keeps an authored accent at 38 %); 10 fixed (a finger's press and release below a checkbox, in a neighbouring button, by a hidden checkbox, and an authored size).

## Review

Read-only static review; no files changed or tests run.

1. **[P1] Hidden and pointer-ineligible controls become actionable.** [addressed_tap.rs:210](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:210) never checks `PaintedBox::pointer_hit`. Those boxes include controls with `visibility: hidden` or `pointer-events: none`. The normal hit test rejects them, but the fallback selects them again—even at their centres—and activation can toggle them. Preserve the normal eligibility checks, including the release path’s current CSS eligibility.

2. **[P1] Touch expansion changes which container scrolls.** [presenter.rs:1156](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter.rs:1156) applies the fallback to every `hit` caller, including `wheel_at`, `scrollers_at`, hover, and gesture discovery. Put a checkbox immediately beside a scroller: its 12 dp extension overlaps the scroller’s edge. A wheel or drag starting there now follows the checkbox’s ancestors, skipping the scroller and potentially scrolling the page. Press-target expansion needs separation from geometric hit testing.

3. **[P2] Checkbox targets steal presses from inline links.** [addressed_tap.rs:233](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:233) checks node handlers and ancestors, but never checks `run_link(hit, x, y)`. Markdown links and inline `href` runs are hit through their containing paragraph. A link beside a checkbox, within its expanded target, therefore toggles the checkbox instead of following the link.

4. **[P2] Expanded hits do not survive the touch release path with press feedback.** [addressed_tap.rs:228](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:228) returns the expanded target, but [contact.rs:1032](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/contact.rs:1032) still requires release inside the original painted box when `contact.press` exists. A checkbox with a `press` handler and `-exact-press-scale=0.9`, touched and released 10 dp below its box, is selected on down but never activated. The release eligibility must retain the accepted touch region.

5. **[P2] Transformed-away controls acquire invisible targets.** [addressed_tap.rs:222](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:222) expands the transformed bounding rectangle without checking transform validity or projective clipping. A checkbox under `transform="scale(0)"` has a zero-area painted box that ordinary `contains` rejects; `touch_box` turns it into an actionable 48 × 48 region. Preserve transform and clipping eligibility before expanding.

6. **[P2] Accessibility exploration selects the wrong neighbouring control.** [access.rs:473](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/access.rs:473) expands bounds without updating [Access.kt:470](/home/ccheever/projects/exact2-review-s2/host/android/canvas/src/main/java/dev/exact/canvas/Access.kt:470), which chooses overlapping rectangles in reverse sibling order. For adjacent 24 dp checkboxes, the second checkbox’s bounds cover the first checkbox’s right half. Exploring that painted half announces the second checkbox; double-tapping activates it. Ordinary pointer hit testing correctly keeps that point on the first checkbox.

7. **[P2] Accessibility expansion escapes clipping ancestors.** [access.rs:475](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/access.rs:475) intersects only `scope.clip`, ignoring `painted.clip`. An ordinary `overflow: hidden` ancestor does not update that scope. A checkbox fitting exactly inside a 24 × 24 clipping parent consequently reports 48 × 48 accessibility bounds, including space where `touch_target` explicitly rejects it. Apply the same effective clip to both bounds.

8. **[P2] Explicit larger dimensions cannot enlarge the painted marks.** [material.rs:573](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:573) unconditionally caps both painters at 20 dp. An authored `width=40 height=40` now enlarges only the layout box; the mark remains 20 dp. This loses the previous resizing behaviour and conflicts with D9’s authored-size precedence. Distinguish an authored size from an automatically stretched box.

9. **[P2] Disabled radios discard authored accent colours.** [material.rs:603](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:603) selects `CanvasText` before consulting `accent-color`. A selected, disabled red radio now paints identically to one without that accent. Previously it retained a dimmed accent; the schema’s Android `radio:disabled` capability still promises that behaviour.

10. **Tests:** [material_tests.rs:198](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/material_tests.rs:198) checks `hit` but never sends a down/up pair or asserts activation. It therefore misses finding 4. The added coverage also omits eligibility, links, scrolling, transforms, clipped accessibility bounds, and exploration overlap. No existing test assertion was found to become false.

11. **Schema outside Material: no findings.** [schema.json:5865](/home/ccheever/projects/exact2-review-s2/kernel/tables/schema.json:5865) preserves the other four metric cells; the new rows are unset outside Material.

12. **Other changes, including the oracle: no findings.**

NOT READY