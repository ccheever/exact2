# Code review: the checker limits' build timings in CPU time (the lead's flake ask), quick review, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `f0006ac98e`.
- **Method:** one brief (sha256 `0f5cdb163e0774149fe9ae297554c28fc74d5e23bd6cac3d9c66d92325816226`), blind to the other reviewer; a quick review (test-only, the lead's flake fixes). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Astra's calibration point taken: the old code measured 9.2 s of CPU, so the bound went from 8 s to 4 s (new 0.9 s).

CLEAN on code inspection.

`compile_path` runs loading, checking, analysis, and lowering synchronously on the calling thread. Neither timed fixture invokes a worker, Rayon, subprocess, or server. The old quadratic scans therefore remain counted.

The **8-second calibration is unverified**: 15.6 seconds of wall time does not prove more than 8 seconds of CPU. Your 0.88-second measurement gives current-code headroom, but confirming regression detection requires timing the old implementation. I couldn’t rerun it in this read-only tree, which has no built tests.

No other issues found.
