# Code review: the Linux rangeFrame reader (LLP 1114.007 D7), narrow re-check after round 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `c7fac77cde`.
- **Method:** one brief (sha256 `8d67ae3f12d474a1dc0f537f6d8cb15b4e811e3a284971676ac5c3714ca4c211`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a host's geometry read, theme2's 17:27 ruling), then a narrow check of the last fix. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**RIGHT.**

No BLOCKING, SHOULD-FIX, or NIT findings.

[Geometry::range](host/linux/src/text/field.rs:298) now compares the candidates’ physical left edges and uses the selected span’s logical endpoint. Tracing offsets 0–7 in both RTL examples gives the endpoint choices in every supplied measurement. The LTR `abc אבג` results at 4 and 7 remain correct, accounting for Linux’s declared base-direction difference and font-dependent advances.

The [new test](host/linux/src/presenter/range_frame_tests.rs:427) checks both separated-boundary cases against painted glyph geometry. Its offsets are correct: UTF-16 offset 4 corresponds to byte 7 in `אבג abc`. Both assertions distinguish the amendment from the previous implementation.

The diff contains only the caret-selection change, its explanatory comment, and the new test. No unrelated changes.

Validation limitation: the existing test executable predates the amendment and lacks the new test. This verdict is from source inspection; I did not rebuild or modify files.
