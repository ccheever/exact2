# Code review: params scope build timings in CPU time (the lead's flake queue), quick review, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `053b268438`.
- **Method:** one brief (sha256 `ff3eff9bff87fa3fbedef5ecf6c0ccb657055425daf1d178138bbbe077f18c9e`), blind to the other reviewer; a quick review (test-only, the lead's flake queue). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Both points taken: the Unix guard, and bounds calibrated on the old code's CPU time.

**BLOCKER — [contract/cli/tests/it/tamber_params_scope.rs:23](contract/cli/tests/it/tamber_params_scope.rs:23):** `cpu()` references `libc` unconditionally, but `contract/cli/Cargo.toml` declares it only under `cfg(unix)`. Unlike `tamber_limits.rs`, this module has no Unix guard, so Windows integration-test compilation breaks. Guard the CPU measurement or provide a platform implementation while retaining the existing semantic tests.

By inspection, `compile_path` and the old quadratic folding execute synchronously on the calling thread; no worker, subprocess, or server work escapes this clock.

The **20-second threshold remains unverified against the old code**: 63 s / >120 s of wall time do not establish >20 s of CPU time under load. I couldn’t rerun that comparison in this read-only checkout, which has no build artifacts.
