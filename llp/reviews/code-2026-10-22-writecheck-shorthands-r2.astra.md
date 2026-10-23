# Code review: The write check judges written shorthand and flex rows as lowering expands them, round 2, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `dc55f555c6`.
- **Method:** one brief (sha256 `7e8ccf33776b5c64ef5e8b1d4fa708828b6aaee4792e0a574b11c422bf72187f`), blind to the other reviewer; 3 rounds (theme2's ruling on the shorthand plan), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**P3 — Nested-choice typing still differs from the build** at [rows.rs:407](contract/lower/src/theme/rows.rs:407).

For `columns=(a ? (b ? "8px" : "auto") : "auto")`, the width component has number/string/string leaves. This code assigns `Unknown`, but `typed_code` recursively unifies the arms and produces `String`: `Unknown.unify(String)` returns `String`.

Fix: recursively unify each choice’s arm types as `expr::compile` does, and add a nested-choice typing comparison. I found no resulting refusal difference for currently admitted longhands, so this is P3.

No P1/P2 findings. Arm spans and whole-row passes-before/fails-after handling look correct.

Tests could not run: the read-only sandbox blocked Cargo’s target creation. No files edited.
