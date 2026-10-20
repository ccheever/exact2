# Code review: Stage 11 C2: the Android reader held to the Material column, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `365e628374`.
- **Method:** one brief (sha256 `63efeca56f5fe246d304765705c27690c28c8cd28956d26ca5b91d6ec6fba216`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (theme2's condition: assert the reader runs the pushed plan), one P2 (a right-only inset passed)
- **Disposition:** both taken in 21410de336: every host must name the compiled plan in its perf before it is measured; the topmost box's right edge held too

1. **P2 — Missing running-plan identity check.** [drive()](/tmp/rv-s11c2r-2/scripts/scale-check.mjs:278) never compares the reader’s plan ID/hash with the compiled plan. The reader can fall back to its bake after a read failure. Theme2’s [16:55 ruling](/Users/admin/x2theme/to-iosfix.md:166) explicitly requires this assertion before measurement. **This blocks sign-off.** Pushing to Unsaid preserves the original ruling’s intent, and that newer ruling accepts the changed approach conditionally.

2. **P2 — Horizontal-inset rejection remains incomplete.** [pageTop()](/tmp/rv-s11c2r-2/scripts/scale-check.mjs:199) checks only the left edge. With the fixture’s calibration box at x=0, a right-only bar/cutout leaves that edge unchanged while inflating `image.width / viewport.w`. The guard therefore accepts an incorrect scale; the height shortcut can also bypass calibration entirely. Validate the content width or actual density, including right-only insets.

The fractional formula correctly handles partial coverage in either boundary pixel, including a two-pixel ramp straddling the threshold. Returning zero outside the image handles x=0 safely; the one-pixel tolerance accommodates clipped antialiasing. The pushed-plan path works, Hello’s data remains unused, and this fixture needs no missing optional capabilities.

Read-only review; no builds or tests run.
