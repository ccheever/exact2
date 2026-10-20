# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, round 3 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `bd7ff7ea31`.
- **Method:** one brief (sha256 `647f238c2ec59c230125be2121de77780f6c1b91286eacdc0a6ecc16dc3ce1bd`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX, 1 NIT).
- **Disposition:** each taken in the next amend ("Round 3").

No BLOCKING findings. Two SHOULD-FIX issues and one NIT remain.

- **SHOULD-FIX — [params.rs:364](contract/lower/src/params.rs:364): standalone validation now skips unfoldable params.** The new `continue` assumes theme lowering will diagnose the failure. The [design-server validator](contract/cli/src/tamber_ops.rs:208) also uses this function, without lowering the theme. A design-only edit setting a ranged param to `calc(space.md * 1e38)` can therefore pass validation while the next build rejects it. Preserve that refusal in shared validation; deduplicate diagnostics only in the compiler.

- **SHOULD-FIX — [residual.rs:181](tamber/src/residual.rs:181): exports still disagree on nested overflow.** `min(space.md * 1e308, 20px)` compiles and evaluates to **20px in every column**. Recursive `fold()` rejects the overflowing child before applying `min`, returning `None`. Consequently, [DTCG emits 1px](tamber/src/export_kinds.rs:62), while [tamber gen leaves unresolved base-name text](contract/cli/src/tamber_gen.rs:164). Match the kernel’s comparison semantics and report export failures instead of substituting these fallbacks.

- **NIT — [theme.rs:495](contract/lower/src/theme.rs:495): overflow gets the wrong explanation.** The f32-overflow reproducer reports the 32-value/32-level limits, although neither is exceeded. Name numeric overflow separately.

The other fixes check out: normalization and alias reduction, division/underflow handling, canceled-base bookkeeping, kind inference and non-length refusals—including the deliberate bare-base type rule. Kernel read tracking, runtime-definition precedence, bounded recursion, terminal conversion/rounding, column validation, inheritance, JS variables/SVG/release-dev handling, and fixture expectations look consistent. Emitted operators and nesting fit the [CSS math grammar](https://drafts.csswg.org/css-values-4/#calc-syntax). No schema or wire change.

**Validation:** 21 selected prebuilt tests passed; stdin compiler probes confirmed the arithmetic and refusal cases. All 43 saved plan pairs remain byte-identical. The server/export findings were traced through code; their end-to-end paths and browser/native hosts were not rerun. Tests miss both findings, live addition of an unset base, and fractional terminal-expression layout. No files changed.
