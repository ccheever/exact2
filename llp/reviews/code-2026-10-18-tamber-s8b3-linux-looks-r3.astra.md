# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `9760771d8..57f577fd1`, the commit and its rounds 1 and 2.
- **Method:** one brief (sha256 `ca9635e1545f642d2e6227781a303360dc6fc5473361e1ece00f5401f475c767`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: hover stale after an asynchronous page flip; should-fix: currentcolor on Android, text-clipped backgrounds, focus looks and flow damage; nit: coverage).
- **Disposition:** post-round fixes, with a focus-look test (`looks_tests.rs`). 1: `follow_pointer` after `display_complete`. 2: a colour transition stays sampled where a look reads `currentcolor`. 3: the text clip takes the states' background. 4: a look change and a focus with a look take the full painter. 5: partly (a focus pixel; the layer and display seams not added).

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Resting-pointer hover remains stale on the physical Linux display.** [events.rs:521](/tmp/rv-s8b3-r3/host/linux/src/presenter/events.rs:521)  
   `follow_pointer()` now refreshes held contacts, but [display.rs:425](/tmp/rv-s8b3-r3/host/linux/src/display.rs:425) calls it immediately after submission. For asynchronous page flips, hit testing still uses the previous frame’s boxes. `display_complete()` installs the new boxes without refreshing hover. A one-time scroll or layout change can therefore leave the old node highlighted indefinitely, including during a settled hold. Move the refresh to successful acknowledgement, after installing the picture and clearing pending submission. Add a pending-frame regression with a stationary pointer, both held and released.

2. **SHOULD-FIX — Android still loses animated `currentcolor` in looks.** [press.rs:52](/tmp/rv-s8b3-r3/host/linux/src/press.rs:52)  
   `paints_over(Color)` checks `PressColor` and a look’s `TextColor`; looks instead reference color through their background or border. Consequently, a leaf text’s color transition still delegates to Android. Its glyphs animate, while the look’s `currentcolor` uses the committed target because Rust no longer samples that transition. Keep color transitions engine-sampled when a look depends on `currentcolor`, including cleared border colors. Test a focused text’s border against its intermediate glyph color.

3. **SHOULD-FIX — Text-clipped backgrounds bypass state composition.** [paint.rs:1074](/tmp/rv-s8b3-r3/host/linux/src/paint.rs:1074)  
   The composed background is discarded for `background-clip="text"`: ordinary box painting skips that background, and [text_clip.rs:36](/tmp/rv-s8b3-r3/host/linux/src/paint/text_clip.rs:36) captures the base again. A standalone text with transparent glyph color and a red background remains red when its hover background becomes blue or `unset`. Pass the composed paint into the clipping path. This affects standalone paragraphs, beyond the queued inline-run limitation.

4. **SHOULD-FIX — Focus looks are missing from CPU damage invalidation.** [feedback.rs:119](/tmp/rv-s8b3-r3/host/linux/src/presenter/feedback.rs:119)  
   Look changes invalidate kept rows but not `flow_damage`. That optimization permits non-input focus and checks only the base shadow at [paint.rs:971](/tmp/rv-s8b3-r3/host/linux/src/paint.rs:971). During an otherwise eligible text-flow update, a focus look outside the damage rectangles can remain unpainted; a moving exclusion with a focus shadow can leave trails beyond its padded box. Request a full repaint when look membership changes and account for effective look shadows when deciding damage eligibility.

5. **NIT — The previously requested integration coverage remains absent.** [looks_tests.rs:40](/tmp/rv-s8b3-r3/host/linux/src/presenter/looks_tests.rs:40)  
   Every fixture uses the CPU painter and immediate frames. None exercises reader-layer alpha, display acknowledgement, focus precedence, or disabling during a hold. The added clear test checks only a plain background. Add focused regressions at those seams; the existing backend doubles and pending-display harness provide suitable starting points.

Previous-review audit, by source inspection:

| Recorded findings | Status |
|---|---|
| R1 Astra 1 / Grok 2: cache contamination | Fixed |
| R1 Astra 2 / Grok 1; R2 Astra 2: stationary hover | Incomplete; finding 1 |
| R1 Astra 3 / Grok 3; R2 Astra 1 / Grok 1: opacity delegation and double alpha | Fixed for overlays present when delegation is decided |
| R1 Astra 4: ignored `currentcolor` | Fixed |
| R1 Grok 4: focus membership | Fixed |
| R2 Astra 4 / Grok 2: explicit clears | Fixed in ordinary box painting |
| R2 Astra 5 / Grok 3: animated `currentcolor` | Incomplete on Android; finding 2 |
| R2 Astra 6–7: disabled press and unclamped base | Fixed |
| R1 Grok 5 / R2 Grok 4: coverage | Still incomplete; finding 5 |

[QUEUE.md:24](/tmp/rv-s8b3-r3/QUEUE.md:24) accurately records R2 Astra 3’s already-delegated transition limitation; acceptable as a non-blocking follow-up, not a fix. The inline-run entry at [QUEUE.md:25](/tmp/rv-s8b3-r3/QUEUE.md:25) is acceptable as a recorded limitation awaiting the named ruling; it does not cover finding 3.

Tests could not start: the read-only sandbox denied creating the specified Cargo target directory. No files were changed.
