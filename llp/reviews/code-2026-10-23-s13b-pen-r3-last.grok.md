# Code review: stage 13 B, the pen importer, round 3's last re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `efaa47f5ca`.
- **Method:** one brief (sha256 `e628bf6654d5fb769c8afb8de6265d9942c5aaffd69b20e77d56edaa3f7f044f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** None.

A hostile `.pen` under 1 MiB no longer reaches the checker's residual or nested-`calc` work, and the passes it does reach stay within a constant of the budget. `cargo test -p exact-tamber --test it import` passed (66), including `every_pass_over_a_hostile_file_is_charged_to_the_budget`.

- A Pencil number that is not a JSON number is reported and stored as null (`tamber/src/import/dtcg/pen.rs:670`). `raw` stops on that refusal, so `clamp(...)` never becomes a length the checker folds. The checker's own leaf and substring bounds are the new `QUEUE.md:3` item.
- Types are compared on their first 32 characters (`pen.rs:426`). Real Pencil types are shorter than that, so alias and kind decisions are unchanged. A long unknown type is still reported.
- Once a fold cannot be paid for, `landing` runs no further fold and `spend(1)` refuses the import (`tamber/src/import/landing.rs:241`, `:354`, `:478`). A skipped fold returns no diagnostics, and that result is not treated as success. The 32-candidate cutoff is gone, so narrowing matches the old culprit walk until the budget refuses the whole import.
- Reached names use `laid_at` (`landing.rs:408`) and a dial's readers are collected once (`landing.rs:459`). Both keep the old first-match rule. A diagnostic copied into a token's why is capped at 300 characters (`landing.rs:361`, `tamber/src/import/kinds.rs:328` and `:346`).
- `reconcile` finds each design token by name, keeping the first, which is what the scan returned (`tamber/src/import/mod.rs:153`).

**Should-fix:** None.

**Nits:**

- `tamber/tests/it/import_pen.rs:959` — the family file's per-tuple keep is about 115M against a budget of about 50M (`tamber/src/import/dtcg.rs:1203`), so the import returns during resolution. The probe and landing never run, though the comment says they do. The small-budget failure still requires that keep.
- `tamber/tests/it/import_pen.rs:1052` — the comment still describes unread strings as the cheap case. The case allowed to finish inside three times its size is `expression`.
