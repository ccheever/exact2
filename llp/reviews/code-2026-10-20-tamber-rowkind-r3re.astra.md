# Code review: LLP 1121 D3/D7, the design server's write-time row check, narrow re-check after round 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `65a83bf6d0`.
- **Method:** one brief (sha256 `47d90e15705f17002ea05d860d1de64d4215dca620aa4143a1f25d92b9700ace`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (3 NIT).
- **Disposition:** taken ("Narrow re-check after round 3").

**RIGHT WITH CHANGES**

1. **NIT — QUEUE’s exclusions are correct, but two explanations need correction.** [QUEUE.md:15](QUEUE.md:15) says every SVG attribute is a prop. Compiler probes confirm that `rect x="pos"` remains a style row and rejects a colour token; `fePointLight x` becomes a prop. Describe SVG rows as conservatively excluded because classification depends on context. The blanket claim also appears in [scan.rs:125](contract/lower/src/theme/scan.rs:125) and the new test’s comment. Also, `padding={let c = "gap" in c}` is invalid Contract syntax; label it as AST pseudocode or replace it.

2. **NIT — The acceptance test can accept a missing reply.** [tamber_rows_check.rs:234](contract/cli/tests/it/tamber_rows_check.rs:234) checks only that diagnostics are absent and `error` is null. `Serve::ask` returns `Value::Null` on EOF, which satisfies both assertions. Assert an `ok` response for each transaction. The fixture otherwise exercises the intended exclusions.

3. **NIT — The budget’s documentation still says “best of three.”** [tamber_rows_check.rs:368](contract/cli/tests/it/tamber_rows_check.rs:368) should say ten.

No BLOCKING or SHOULD-FIX findings. Prior failures are keyed by `Place.row`; computed-arm choices and attribute-level `Expr::Let` are excluded; grouping includes `(name, plain, literal)`; SVG ancestry propagates through every traversed node form. The plain/arm test compares the complete diagnostic locations and messages against the build. I found no new false refusal or misplaced/misworded diagnostic.

The prebuilt budget and table-restoration tests passed. The budget takes the minimum of ten samples; no additional flakiness was found in its construction. Compiler probes confirmed the relevant distinctions. File-writing integration tests were inspected, not rerun. No files changed.
