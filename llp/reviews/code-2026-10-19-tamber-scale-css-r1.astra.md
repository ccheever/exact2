# Code review: tamber scale_css held as ratios, not wall clocks (the lead's 22:29Z load flake), and the walks they found quadratic, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `63b3ba9351`.
- **Method:** one brief (sha256 `50b55d243082414cacc49e1fe0902743f44bf710b477eedd5efcea809a855eb9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking; four should-fix (alias chains scale the count, not the depth; copied's fixed prelude; the 10x + 50 ms allowance admits a quadratic, with design and vocab timed; grouped samples load-sensitive)
- **Disposition:** taken in fb8a3dc7b9 (a depth ratio, preludes grown, 8x, design and vocab built once, samples in turn, the thread's CPU time); the depth ratio found the design checker quadratic, fixed there

No blocking memo bug found. Four **should-fix** issues in the ratio tests:

- **[scale_css.rs:173](/tmp/rv-scl1/tamber/tests/it/scale_css.rs:173): Wrong scaling dimension.** Increasing chain count 20→80 while keeping length 250 makes repeated, quadratic-per-chain walks grow only 4×. Fix: hold chain count constant and scale chain length below the cap, e.g. 64→256; retain the full-size output check.

- **[scale_css.rs:103](/tmp/rv-scl1/tamber/tests/it/scale_css.rs:103): Fixed prefix masks quadratic work.** Copied-context inputs grow from 341,079 to 621,079 bytes—only 1.82×. Quadratic byte-scanning work grows 3.32× and passes. Fix: scale the prelude too, preserving the full-size fixture, or count work directly.

- **[scale_css.rs:30](/tmp/rv-scl1/tamber/tests/it/scale_css.rs:30): The allowance admits quadratic timings.** An 8 ms→128 ms quadratic path passes the 130 ms bound. Timed `design()`/`vocab()` setup further dilutes growth. Fix: prebuild those inputs and use calibrated batches with identical repetition counts, or deterministic work counts.

- **[scale_css.rs:28](/tmp/rv-scl1/tamber/tests/it/scale_css.rs:28): Grouped samples remain load-sensitive.** All small runs precede all large runs; sustained contention during the latter survives best-of-three. Fix: alternate paired samples and compare paired ratios; work counts eliminate scheduling noise.

**Coverage:** Every former fixture reaches its original full size, and every existing output assertion remains. Property braces, unmatched braces, empty rules, and resets still lack semantic output assertions; that gap predates these commits.

**Memo:** [engine.rs:244](/tmp/rv-scl1/tamber/src/import/engine.rs:244) correctly caches only the start after 257 references. Its shorter suffixes remain untouched; predecessors reaching that start also exceed 256. Keys include the full block target, and saturating addition preserves the deep marker.

Static review only; no files changed or builds/tests run.
