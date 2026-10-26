# Code review: Apple pass #2: an outline brings the selection's own copy into view, check of the page test's rework (theme2's 16:25 (3)), 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `06bbd0bd53`.
- **Method:** one brief (sha256 `4db4ee94e23f2bed1405749a5616ac3c64894535ce5386268b969a6388880a20`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

SHOULD — [scripts/tamber-select-reveal.test.mjs:85](scripts/tamber-select-reveal.test.mjs:85): `click()` waits for the selection event, but its outline may still be pending. If that outline arrives after `moveUp()` clears marks, `marked()` accepts the click’s mark before the move’s outline runs. The extra 100 ms cannot guarantee completion: (a) can falsely pass, and a delayed scroll can disrupt the next case. Clear marks before clicking and await that redraw before moving, or identify the expected Card marks.

A controlled delivery probe using the actual helpers reproduced premature completion. Geometry and listed-first detection otherwise look sound for this fixture.

The full browser test could not run: `playwright-core` is missing.
