# Code review: The write check judges written shorthand and flex rows as lowering expands them, round 1, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `92d4088557`.
- **Method:** one brief (sha256 `4f50dd8afbe78a453be871baf9c2bfd10f2d4d4da167a3af4c990387b2d2b88e`), blind to the other reviewer; 3 rounds (theme2's ruling on the shorthand plan), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**[P2] Preserve whole shorthand choices when judging them** — [rows.rs:388](contract/lower/src/theme/rows.rs:388).

`kept()` now admits these choices, but `grouped()` still splits them into independent literal rows. That changes the first refusal’s words and location.

Repro: start with `gap = 8px`, `bad = 8px`, and:

```text
flex=(on ? "1 1 gap" : "1 1 bad")
```

In one transaction, set `gap = #444444` and `bad = 2`.

- **Build:** reports `bad` at the second arm. `check_style_value()` runs `measure_row()` across the whole basis choice before checking colour-token failures.
- **Write check:** reports `gap` at the first arm. It judges each arm separately, then sorts failures by source position.

The same mismatch occurs with `columns=(on ? "gap 2" : "bad 2")`.

**Fix:** retain expanded choices as whole `Row`s, expand each complete choice, and preserve lowering’s component typing and spans. Add a regression comparing the first diagnostic’s words and span. The existing choice test at [tamber_rows_shorthand.rs:41](contract/cli/tests/it/tamber_rows_shorthand.rs:41) uses `padding`, an `AttrTarget::Styles` row, so it never exercises the new expansion path.

Compiler behavior reproduced with the existing branch binary; three in-memory row tests passed. The read-only sandbox blocked Cargo integration testing. No files changed.
