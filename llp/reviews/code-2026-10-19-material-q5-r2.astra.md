# Code review: the 24 dp marks and the 48 dp touch target, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed, a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `ba937d88c`, the second form; shown round 1's review to check its findings.
- **Method:** one brief (sha256 `1e46739e8c66e1208b93a5f648a3a183ff110190091436d2ac9cec34fca6f76f`). The author (Claude) is not a reviewer.
- **Answered:** 1 fixed (a candidate is what a press at its middle hits, or inside it); 2 fixed (no disabled ancestor: `input_live`); 3 fixed (no expansion for a box drawn through a perspective); 4 fixed (a finger on nothing at all may still be in a touch box, on down and on release); 5 fixed (only a finger's release expands; an agent's named tap does not); 6 fixed (`auto` is no authored size); 7 in part (a covered checkbox, a disabled column, a named wrapper; not transformed clipping, links or press feedback).

## Review

Read-only static review of `ba937d88c`; no files changed or tests run.

1. **[P1] Touch targets can activate covered controls.** [addressed_tap.rs:223](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:223) checks the candidate’s own geometry, not whether it wins hit testing at its centre. Put an opaque, nonpressable overlay over a checkbox: a finger tap on the overlay now toggles the checkbox behind it. `b.contains` and `paint::hits` do not check occlusion, contrary to the function’s stated eligibility rule.

2. **[P2] Expansion bypasses disabled ancestors.** [addressed_tap.rs:214](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:214) checks only the candidate’s `disabled` property. Ordinary touch contacts use `input_live`, which also rejects disabled ancestors. Put an unbound checkbox inside a disabled 24 × 24 container and tap six dp outside that container, on the surrounding page. The fallback selects and toggles the checkbox although a touch directly on it is rejected.

3. **[P2] Expanded targets escape clipping inside a transformed subtree.** [addressed_tap.rs:220](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/addressed_tap.rs:220) checks only `b.clip` at the touch point. For a 3D-transformed subtree, [space.rs:323](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/space.rs:323) stores internal clips in `b.projective`; `b.contains` checks those only at the candidate’s centre here. A checkbox filling an `overflow="hidden"` parent with `rotateY(60deg)` therefore accepts taps outside the parent’s projected clipping boundary. The first review’s `scale(0)` case is fixed, but transform eligibility remains incomplete.

4. **[P2] The target fails where the finger hits no layout box.** Expansion runs only at release in [surface_controls.rs:634](/home/ccheever/projects/exact2-review-s2/host/linux/src/surface_controls.rs:634), but [contact.rs:535](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/contact.rs:535) creates no contact when geometric `hit` returns `None`. With a 24 × 24 root containing a checkbox, a touch at `(30,12)` is inside its intended 48 dp target but never activates it. The new test succeeds because another layout box occupies its test point.

5. **[P2] Named agent taps can activate an unnamed control.** [surface_controls.rs:634](/home/ccheever/projects/exact2-review-s2/host/linux/src/surface_controls.rs:634) changes activation after `tap_at` has validated the geometric target. A passive 60 × 24 wrapper containing a left-aligned 24 dp checkbox has its centre six dp beyond the checkbox. `tap wrapper` passes the “never press a control inside it” checks, then the fallback toggles the checkbox. Target validation and activation must agree about expansion.

6. **[P2] Explicit `auto` dimensions incorrectly enlarge the drawing.** [material.rs:577](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/material.rs:577) treats any width/height mask bit as an authored size. Adding `width="auto"` to an otherwise default checkbox or radio leaves its intrinsic layout at 24 × 24 but changes its drawing from 20 × 20 to 24 × 24, removing the intended inset. A stretched control with an explicit automatic dimension can grow still further. The sizing fix must distinguish automatic dimensions from an actual size override.

7. **Tests remain incomplete.** [material_tests.rs:238](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/material_tests.rs:238) asserts only that the point misses the checkbox, not that it hits nothing. Coverage now includes actual down/up activation, a neighbouring button, hidden controls, and one explicit checkbox size. It omits the failures above, overlapping expanded targets, disabled ancestors, transformed clipping, named taps, and regression cases for links and press feedback. No existing assertion was identified as newly false.

8. **Accessibility bounds: no new findings.** [access.rs:463](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/access.rs:463) retains the laid-out bounds; HEAD no longer introduces 48 dp accessibility rectangles. This answers the first review’s overlap and expanded-clipping findings without changing consumers of `rect`.

9. **Schema outside Material: no findings.** [schema.json:5865](/home/ccheever/projects/exact2-review-s2/kernel/tables/schema.json:5865) preserves the iOS, Mac, web, and neutral values. The added metrics are unset outside Material.

10. **Other changes, including the oracle: no findings.** [oracle.mjs:182](/home/ccheever/projects/exact2-review-s2/host/android/reference/oracle.mjs:182) retains size comparisons and compares internal geometry relative to each control’s own origin.

The first review’s stated cases **1–4, 6–7, and 9** are addressed. **5** fixes collapsed transforms but misses transformed clipping; **8** fixes explicit numeric sizes but introduces the `auto` problem; **10** is only partially addressed. Its **11–12** remain no findings.

NOT READY