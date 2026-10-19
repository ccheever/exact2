# Code review: the 24 dp marks and the 48 dp touch target, round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed, a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `f108e1171`, the third form; shown rounds 1 and 2 to check their findings.
- **Method:** one brief (sha256 `d116da630c57a7e81ade8c2a75389f7c245e1e36989376d0906a50c9039cd9a2`). The author (Claude) is not a reviewer.
- **Answered (the last of three rounds):** 1 left, in QUEUE (the target is chosen at the release; keeping it from the down is the fix, and a larger change than a fix round); 2 fixed (a press on a control's own box that hit something else is on a cover there, and does not reach it; the half-covered checkbox is tested); 3 fixed (a box drawn rotated or skewed, its own or an ancestor's, as its painted matrix holds them, is no candidate; the rotated clip is tested); 4 and 5 fixed by removing the no-hit fallback, which leaves round 2's finding 4 open, in QUEUE with 1; 6 in part (partial occlusion and rotated clipping; not sliding taps, overlapping targets, links or press feedback).

## Review

Static review of `f108e1171`. No files changed; no tests run.

1. **[P1] Expanded presses do not retain their activation target.** [surface_controls.rs:637](host/linux/src/surface_controls.rs:637) chooses the control on release, while [contact.rs:1077](host/linux/src/presenter/contact.rs:1077) retains only the geometric hit. A page’s `pointerdown` can reveal a checkbox beside the finger; releasing on the same page then activates that newly appeared checkbox. The mismatch also drops ordinary taps: moving four dp from inside a checkbox into its expanded target changes the geometric hit to its parent, so release does nothing despite remaining inside the intended target and below touch slop.

2. **[P1] Partially covered controls remain actionable through their cover.** [addressed_tap.rs:227](host/linux/src/presenter/addressed_tap.rs:227) checks occlusion only at the control’s centre. Put an opaque, nonpressable overlay over a checkbox’s left half, leaving its centre uncovered. A stationary tap on the covered half geometrically hits the overlay, but expansion redirects it to the checkbox underneath. Round 2’s fully covered example is fixed; occlusion at the actual press remains unchecked.

3. **[P2] Expanded targets still escape rotated clipping ancestors.** [addressed_tap.rs:222](host/linux/src/presenter/addressed_tap.rs:222) excludes projective transforms but accepts affine rotation, then checks only the axis-aligned `b.clip`. Put a checkbox inside a matching 24 × 24 `overflow="hidden"` parent rotated 45°. A point 15 dp right and down from its centre lies inside that bounding rectangle but outside the rotated clipping square. Ordinary hit testing misses the checkbox; expansion activates it. Round 2’s perspective case is excluded, but transformed clipping remains incomplete.

4. **[P2] The no-hit fix reintroduces the press-feedback failure.** [contact.rs:539](host/linux/src/presenter/contact.rs:539) creates a contact on the expanded control when geometric hit testing returns nothing. With a `press` handler and `-exact-press-scale=0.9`, this produces `contact.press`; release then requires the original box to contain the point at [contact.rs:1036](host/linux/src/presenter/contact.rs:1036). For a 24 × 24 root containing that checkbox, down/up at `(30,12)` never activates it. Round 1 finding 4 therefore survives where round 2 finding 4’s fix applies.

5. **[P2] The no-hit fallback also expands drag discovery.** [contact.rs:548](host/linux/src/presenter/contact.rs:548) passes the expanded control into `candidates`. A 24 × 24 root with a `pan` handler and a checkbox now starts that pan from `(30,12)`, outside every geometric box, once the finger moves beyond slop. Meanwhile `scrollers_at` reports no hit or gesture claims there. Press expansion needs its own retained target rather than replacing the hit used for gesture discovery.

6. **Tests remain incomplete.** [material_tests.rs:248](host/linux/src/presenter/material_tests.rs:248) still asserts only that the point misses the checkbox, not that it hits nothing. All added taps are stationary. Consequently the tests miss the no-hit feedback/drag paths, boundary-crossing taps, release retargeting, partial occlusion, and rotated clipping above. Regression coverage also remains absent for overlapping targets, inline links, explicit `auto`, and disabled radio accents. No existing assertion was identified as newly false.

7. **Sizes and drawing: no findings.** The Material marks use 24 dp layout and centred 20 dp drawing; numeric authored dimensions and explicit `auto` are distinguished. The other looks retain their sizing and painting paths.

8. **Accessibility bounds: no findings introduced by this commit.** [access.rs:464](host/linux/src/presenter/access.rs:464) remains unchanged and reports layout bounds. HEAD does **not** introduce the 48 dp accessibility rectangles described in the brief. This answers round 1’s accessibility expansion findings.

9. **Schema outside Material: no findings.** [schema.json:5865](kernel/tables/schema.json:5865) preserves the other columns; the new metrics are unset outside Material.

10. **Other changes, including the oracle: no findings.**

Earlier-review disposition: R1 findings **1, 3, 6–9** are addressed; **2, 4, 5, 10** remain incomplete as detailed above; **11–12** remain no findings. R2 findings **2, 5, 6** are addressed; **1, 3, 4** fix their original examples but leave the related failures above; **7** remains incomplete; **8–10** remain no findings.

NOT READY