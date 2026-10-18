# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `23fd7820f` and its round-1 fixes `feeca37c0` (now `2a2fdabc1`, `29023ccd8` after a reorder; same tree).
- **Method:** one brief (sha256 `776daeb72681500d758410e928ae35de65443c7b0f879538c80cab858dadd1cd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: Android's layer alpha doubling a drawn opacity, hover stale while a contact is held; should-fix: played tracks not relinquished, explicit clears, currentcolor in flight, a disabled held node, an unclamped base).
- **Disposition:** in the round-2 commit, with a test of a cleared look (`looks_tests.rs`). 1: the layer's alpha is the opacity only when its reader plays it (`paint/layer.rs`). 2: `follow_pointer` refreshes the looks while held. 4: clears are CSS's initial; `paints_over` counts them. 5: currentcolor from the shown colour. 6: a disabled node's press progress is 0. 7: the base is clamped first. 3: QUEUE (reclaiming a played track at its curve's position, as a hold does).

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Android can apply the composed opacity twice.** [host/linux/src/lower.rs:539](/tmp/rv-s8b3-r2/host/linux/src/lower.rs:539), [paint/layer.rs:84](/tmp/rv-s8b3-r2/host/linux/src/paint/layer.rs:84)  
   With `:active opacity=0.5` and a concurrent scale transition, scale still creates a reader layer. Its base alpha is `0.5`, and the painter also records an opacity group because `LOWER_OPACITY` is absent. Android applies both, producing approximately `0.25`. Give opacity exactly one owner: use layer alpha `1` when opacity remains in the recorded drawing. Add a Canvas-reader regression covering mixed transitions and a held press.

2. **BLOCKING — Hover still becomes stale while a contact is held.** [host/linux/src/presenter/feedback.rs:80](/tmp/rv-s8b3-r2/host/linux/src/presenter/feedback.rs:80)  
   The per-frame path retains previous hover IDs. Round 1’s replacement path, `follow_pointer()`, returns immediately while a contact or pointer button is held. If a timer moves the hovered card away during a stationary press, its hover border/shadow remains and the newly covered node gets no hover. Refresh look membership from current hit geometry during held input too, independently of hover-event dispatch; invalidate departing and arriving rows.

3. **SHOULD-FIX — Already-lowered transitions do not relinquish ownership when looks change.** [host/linux/src/lower.rs:177](/tmp/rv-s8b3-r2/host/linux/src/lower.rs:177)  
   `playable()` checks only transitions being handed over. A `SetLooks` commit adding an opacity overlay during an existing reader opacity transition leaves that transition in `self.played`; an unchanged base target preserves `is_played`. Android therefore continues overwriting the new look until the transition ends. Recheck existing tracks on relevant commits, resume engine sampling at their current curve position, and publish their removal to the reader.

4. **SHOULD-FIX — Explicit clears remain ignored outside opacity.** [host/linux/src/paint/presented.rs:268](/tmp/rv-s8b3-r2/host/linux/src/paint/presented.rs:268), [press.rs:56](/tmp/rv-s8b3-r2/host/linux/src/press.rs:56)  
   `looked()` checks only `mask`, although `unset` arrives in `written` without a mask bit. Consequently, a focus look clearing background, border color, or shadow leaves the base/hover value showing. Separately, `paints_over()` misses cleared opacity, allowing Android’s animation to override the newly implemented opacity reset. Use `mask.union(written)` to detect declarations and apply each cleared row’s initial value.

5. **SHOULD-FIX — `currentcolor` still ignores the presented color.** [host/linux/src/paint/presented.rs:244](/tmp/rv-s8b3-r2/host/linux/src/paint/presented.rs:244)  
   The fix reads committed `TextColor`, bypassing `shown.colors`. During a red-to-blue color transition, a hovered `border-color=currentcolor` jumps to blue while the text is still transitioning. Resolve against the presented color, falling back to the computed row. Cover both background and border, including inherited animated color.

6. **SHOULD-FIX — Disabling a held node leaves its press paint visible.** [host/linux/src/press.rs:93](/tmp/rv-s8b3-r2/host/linux/src/press.rs:93)  
   `disabled` suppresses only hover/focus. Contact cancellation starts the normal release ease, so scale, pressed colors, and `press_opacity_to` continue affecting the disabled node for up to 120 ms. Clear feedback on disabling and gate all press outputs by disabled state. Test disabling during a hold, before advancing the clock.

7. **SHOULD-FIX — Absolute opacity interpolates from an unclamped base.** [host/linux/src/press.rs:97](/tmp/rv-s8b3-r2/host/linux/src/press.rs:97)  
   Base/look opacity is clamped only later by the painter. For authored opacity `2`—visibly `1`—and target `0.5`, half press progress computes `1.25`, displayed as `1`, instead of `0.75`. Clamp the underlying shown opacity before composing feedback. Cover out-of-range values and animation overshoot.

Round-1 audit: cache contamination **A1/G2** and focus membership **G4** are fixed by inspection. Hover **A2/G1**, Android lowering **A3/G3**, and `currentcolor` **A4** are partially fixed. **G5** gained the scale-under-hover regression, but still lacks focus, stationary-pointer scrolling, and Canvas-reader coverage.

Cargo tests could not start: the read-only sandbox denied creating the requested target directory. No files were changed.
