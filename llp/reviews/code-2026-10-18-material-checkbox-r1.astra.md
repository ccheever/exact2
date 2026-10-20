# Code review: the Android checkbox as Material 3's, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed (the read-only sandbox cannot start on `expo-build-1000`), a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `a4952c5bf`.
- **Method:** one brief (sha256 `b87514ebd81ff2e3810f18b5509b99136127a2a50d74d5cc202b5a9ecbc07691`), blind to the author's reasoning. The author (Claude) is not a reviewer.
- **Answered:** 1 fixed (a mark the kernel sizes forgets what was reported for it, in the neutral look too); 2 fixed (each axis snapped on its own); 3 fixed (an empty box paints nothing, a box too small for a ring is all outline, the corner fits the box); 4 fixed (the corner, the stroke and the disabled opacity are rows, held to the source); 5 in part (a Material-look test in a child process: size, the checked fill, the unchecked inside, disabled, a forgotten report).

## Review

Read-only review; builds and tests were not run.

1. **P2 — Correctness: previously reported sizes survive a switch-to-checkbox change.** [host/linux/src/presenter/control.rs:17](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/control.rs:17) returns `None`, but `size_controls` never clears an existing intrinsic size. For `input type="checkbox" role=(switchMode ? "switch" : "checkbox")`, changing `switchMode` from true to false retains the switch’s **38 × 22** layout box and paints a 22 dp checkbox. Clear the previous report so the kernel uses 20 × 20.

2. **P2 — Correctness: horizontal snapping changes vertical geometry.** [host/linux/src/paint/material.rs:498](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:498) derives one `size` from snapped horizontal endpoints, then uses it for both axes. At density 1, a 20 dp checkbox with `scaleX(0.03)` about its top-left becomes **1 × 33.33 pixels**, although its height should remain 20. At density 2.4375 with `scaleY(1.25)`, its bottom lands at **61.25 pixels**, contradicting the promised pixel alignment. Horizontal rounding must not determine vertical extent.

3. **P2 — Correctness: an empty checkbox can still paint an outline.** [host/linux/src/paint/material.rs:532](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:532) builds a stroked path without rejecting an empty square or constraining its corners. For an unchecked checkbox with `width=0 height=20`, `size` is zero, but the path still contains nonzero segments—for example, relative to its centre, `M2 1 H-2`. It paints outside the zero-width box. Reject empty geometry and constrain the outline for small authored sizes.

4. **P2 — Other: Material constants bypass the required metric rows.** [host/linux/src/paint/material.rs:493](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:493) hardcodes the corner radius; lines 503 and 505 likewise hardcode stroke width and disabled opacity. LLP 1115.001.001 D2.6 requires these values to become rows when a component is matched. Only `CheckboxSize` is connected to the extractor: a pinned-library change to the stroke or corners can leave the checkbox unchanged while the row check passes.

5. **Tests — coverage gap.** [host/android/reference/specimens.json:15](/home/ccheever/projects/exact2-review-s2/host/android/reference/specimens.json:15) covers only enabled, default-size checked and unchecked checkboxes. No Rust test selects the Material look, leaving disabled paint, authored accents, constrained sizes, transforms and retained-control sizing untested. The failures above therefore escape the current coverage. **Existing assertions: no findings**; the 13 dp assertions still apply to their default look.

6. **Schema isolation — no findings.** [kernel/tables/schema.json:5853](/home/ccheever/projects/exact2-review-s2/kernel/tables/schema.json:5853) changes only the Material cell. Web/iOS/Mac fallbacks remain 13 × 13 for checkbox/radio and 38 × 22 for switch. Neutral remains `1.25em` square and `2.25em × 1.25em`. Material checkbox/radio fallbacks become 20 × 20, but the presenter still overrides radio to 13 × 13; switch remains 38 × 22. The unresolved 48 dp layout box is not a finding.

NOT READY