# Code review: Linux focus on a control a commit disables, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c82b5ee79c`.
- **Method:** one brief (sha256 `1cc57928902c0cd6802de2f74dacef6931402f96b9549ad0ec09f5ede7ea97bc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **[P2] A nested disabled blur can restore the pressed button’s focus.** [events.rs:212](host/linux/src/presenter/events.rs:212) overwrites `disabled_blur` on every disabled-focus transition, while [surface_controls.rs:982](host/linux/src/surface_controls.rs:982) checks only the last recorded node.

   Concrete input: button A’s press disables A and mounts autofocus input B; A’s blur re-enables A; B’s focus handler disables B. During `after_commit`, A records `Some(A)`, then B overwrites it with `Some(B)` and leaves focus empty. A’s outer press consequently accepts its fallback and silently restores focus to A. This defeats the round-1 fix for re-enabling during blur.

   Preserve whether **this press’s target** underwent disabled blur across subsequent focus transitions. Add a regression through `p.tap` or keyboard activation; the direct-dispatch helper bypasses the faulty fallback. This finding is code-traced, not runtime-verified.

Non-blocking notes:

- The round-1 event fixes look correct: `set_focus(None)` commits edited text before blur, separates blur’s propagation flag from focusout, and uses the existing detached-path handling. Clearing focus before dispatch prevents immediate recursion on the same disabled node.
- The eligibility predicate now excludes links and both progress kinds. Disabled ancestors retain focus, and simultaneous route hiding takes the existing route path. The remote exception preserves its pinned behavior.
- Focus commands, autofocus, surface restoration and hosted-field callbacks have eligibility checks. Using `set_focus` also preserves the existing dirty, caret-reveal and ring handling. Collection-originated disabling remains untested here: this path can enter full `after_commit` refinement, unlike the removed-focus path’s `sync_commit`.
- The disabled-field IME commit rejection predates this patch. I would not block this change on that or the existing mid-blur `focus()` behavior. Android and Windows share the changed presenter; native IME and hosted-field qualification remain distinct from shared Rust coverage.
- The fixture uses controlled time and an explicit `clock +200`; I found no wall-clock dependency. The new behavioral assertions meaningfully distinguish the fix, but miss the nested case above.
- Both permitted Cargo commands failed before compilation because the sandbox denied creating `/tmp/rv-rt1-db2-target…`. I could not independently confirm the reported suite/conformance results, full-conformance baseline or caps. The Apple probe remains pending as agreed. The worktree is unchanged.

CHANGES
