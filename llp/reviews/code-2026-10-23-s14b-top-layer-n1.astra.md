# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), narrow re-check 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `7df0ba99f8`.
- **Method:** one brief (sha256 `ab283a0c480b6332f9d9ef87e4263ad75ecf17da6b96712c1bf557baea3be3e3`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 894220519e (laid-out end margins)

WRONG — [kernel/src/kernel/block.rs:479](/tmp/iosfix-rv-s14b4/kernel/src/kernel/block.rs:479): the new RTL branch reconstructs the right margin from a clamped block size, so negative margins can still misplace the popover.

Concrete case: an 800-point viewport, root `direction: rtl`, and a popover with `width: 80`, `left: 0`, `right: 0`, `margin-left: -100`, `margin-right: 0`.

Its block-space `frame.x` is −100. `used_size()` clamps the margin-box width, −20, to zero. Consequently, `end_margin = used - at - size` becomes **20**, although the actual right margin is **0**. The new branch returns **x = 700**; the required position is **720**, honoring the right inset and right margin. [CSS 2 §10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width). This affects the runner’s `shown_in` fallback and terminal placement. Use the resolved margin or an unclamped extent.

The scroll-anchor and region-validation fixes look correct. By source tracing, each new test detects its original regression: the row jumps 48 points, former-owner registration fails, or placement returns `y = 0` / RTL `x = 0`. The placement test misses the negative-margin case above.

No files changed. Rust tests and mutation tests were not executed: this read-only checkout has no compiled target.
