# Code review: exact-tamber color-mix and Rec. 2100 follow-up, round 1, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `a7512b613c4482a0c79399fd1cb643217607c961f24c5f715cd773f8e1ea1817`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 2 MAJOR, 1 MINOR; Grok 1 MAJOR, 1 MINOR, the same findings).
- **Disposition:** all fixed in round 2: the running mix keeps its hue, HLG signed both ways, all-0% weights stepped evenly

---

Found three issues:

1. **MAJOR — Intermediate hue is lost** at [tamber/src/color_fn.rs:661](/private/tmp/rv-mix/tamber/src/color_fn.rs:661).  
   `color-mix(in oklch, oklch(.7 .1 30 / 0), oklch(.7 .1 90 / 0), oklch(.7 .1 120))` produces hue **120° instead of 80°**. The first pair retains hue 60° despite zero alpha/chroma; the next `interpolate` treats that accumulator as achromatic and overwrites its hue. Preserve the accumulated hue: it is not premultiplied, and no colour-space conversion is required between these steps. [CSS Color 4 interpolation](https://drafts.csswg.org/css-color-4/#interpolation)

2. **MAJOR — HLG conversion clips negative channels** at [color/src/lib.rs:653](/private/tmp/rv-mix/color/src/lib.rs:653).  
   `color(from color(rec2100-linear -.01 .1 .1) rec2100-hlg r g b)` gets red **0**, rather than approximately **−0.08916**. The `.max(0.0)` destroys valid extended colour information. HLG requires a signed transfer for negative values. [CSS Color HDR](https://drafts.csswg.org/css-color-hdr/)

3. **MINOR — Three or more all-zero weights mix incorrectly** at [tamber/src/color_fn.rs:703](/private/tmp/rv-mix/tamber/src/color_fn.rs:703).  
   `color-mix(in srgb, red 0%, lime 0%, blue 0%)` gives equal thirds; the specified result is **`color(srgb .25 .25 .5 / 0)`**. Normalization preserves zero weights, then each zero-total step uses progress **0.5**. Remove the equal-weight substitution and change line 659’s zero-total fallback accordingly. Relative colours that replace alpha expose the difference. [Normalization](https://drafts.csswg.org/css-values-5/#normalize-mix-percentages), [mixing algorithm](https://drafts.csswg.org/css-color-5/#color-mix-result)

Ordinary normalization, sequential order, and the final alpha multiplier look correct. PQ’s constants/reference white and positive HLG round trips check out numerically. I found no incorrect numeric expectations in `expected.rs`.

I ran in-memory numeric reproductions. `cargo test -p exact-tamber -p exact-color --no-fail-fast` was blocked by the read-only sandbox creating Cargo’s build directory. No files were edited.

VERDICT: fix (MAJOR: tamber/src/color_fn.rs:661, color/src/lib.rs:653; MINOR: tamber/src/color_fn.rs:703)
