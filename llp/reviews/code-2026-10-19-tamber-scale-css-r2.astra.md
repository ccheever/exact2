# Code review: tamber scale_css held as ratios, not wall clocks (the lead's 22:29Z load flake), and the walks they found quadratic, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fb8a3dc7b9`.
- **Method:** one brief (sha256 `1c0fb5dbfe1e77c5f79c936b7638d93f0df0aa213ead42018bc2b5f95f995d6b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking (the checker memo and the shadcn change equivalent); two should-fix (the 400 ms floor admits a short quadratic; label and empty-rule preludes fixed), one nit (tailwind's design timed)
- **Disposition:** taken in 2e491d0d0b: equal-count batches past 50 ms of CPU and no floor; the empty rules' prelude grows; a label copies its prelude by design, so it stays fixed and the sizes drop under the 8 MiB cap; tailwind's design built once

No blocking correctness defect found. Two should-fix items and one nit:

- **Should-fix — [scale_css.rs:30](/tmp/rv-scl2/tamber/tests/it/scale_css.rs:30):** The 400 ms floor still permits quadratic growth: 20 ms → 320 ms is 16× and passes. The new 40/160-link test therefore cannot reliably catch regression. **Fix:** time equal-count batches at both sizes until the smaller batch exceeds the measurement floor, then enforce 8×.

- **Should-fix — [scale_css.rs:169](/tmp/rv-scl2/tamber/tests/it/scale_css.rs:169), [445](/tmp/rv-scl2/tamber/tests/it/scale_css.rs:445):** Label and empty-rule preludes remain fixed at 4,000 bytes. Repeated context copying consequently scales only 4× in these fixtures and passes. **Fix:** grow these preludes with `n` too; `2*n/3` and `n/5`, respectively, preserve their full-size inputs.

- **Nit — [scale_css.rs:275](/tmp/rv-scl2/tamber/tests/it/scale_css.rs:275):** Tailwind still constructs `design()` inside timed closures here and at lines 303, 325 and 462. **Fix:** construct it once outside each timing call.

The checker memo appears equivalent: inference reads stable inputs, cycle-affected results remain uncached, and every activating call clears `on` on return. The shadow callback safely shares that walk. Shadcn’s observable uses of `kind` are all guarded by `Fate::NotColour`.

The timed paths are synchronous; no worker-thread work is omitted. CPU timing intentionally excludes waits. The non-Unix fallback is type-correct by inspection. All previous output assertions retain their full-size inputs.

Read-only review; no files changed, builds or tests run.
