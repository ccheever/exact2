# Code review: stage 13 B, the pen importer, round 3's final budget check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `74fc5dfc38`.
- **Method:** one brief (sha256 `22a34dbeb4ea8f6f41bf5b509a7ec061c1f160d84675889f00423c152fc900c6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** None.

A hostile `.pen` under 1 MiB does not get work or memory past a constant times the budget, and it does not plant a kind. `c42412601a` and `f798abb3ab` keep each pass’s decisions: the sided-check map, the tuple index, and the read set match the old results, and an over-budget probe or fold refuses the import instead of returning a partial map. Past 32 fold candidates every candidate is refused rather than narrowed, so the mapped set only shrinks. `cargo test -p exact-tamber --test it every_pass` passed (1), and `cargo test -p exact-tamber --test it import` passed (66).

**Should-fix:** None.

**Nits:**

- `tamber/src/import/dtcg/pen.rs:759` — the parse row says depth is refused past 256. `serde_json::from_str` refuses at its own limit, 128. The depth-200 test already expects `recursion limit`. Stricter than the table, not a hole.
- `tamber/src/import/dtcg/kinds.rs:31` — `type_of` walks alias chains and never spends. Memoization makes it one walk per path per tuple, the same set the winners row charges, times a name of at most 128.
- `tamber/src/import/landing.rs:163` and `:235` — `kinds` folds the user’s design up to 16 times, and the baseline check folds it once, with no spend. Neither grows with the `.pen`. The fold row covers only `fold`.
- `tamber/src/import/landing.rs:244` — a failed fold spend is ignored and `check_design` still runs. The refusing `spend(1)` is at `:473`, after that iteration’s candidate folds. Laid-token errors do not add folds; design-line errors add at most 64 each, and only when there are 32 or fewer candidates. A `.pen` does not raise that error count.
- `tamber/src/import/landing.rs:395` — the reads row says a name is found in one step. `text` is. Matching it to a laid token, and `candidates.contains` at `:396`, still scan. For a colour chain the cycle check has already charged the square, so this stays within a small factor of that charge. The dial scan at `:458` is the same shape over the design.
- `tamber/src/import/mod.rs:150` — the ops row says one per token made. Each laid token still scans every design token. Empty design is linear; with a design the factor is the design’s token count.

`marks_of` in `contract/cli/src/tamber_import.rs:205` matches its row: a pen emits no `set-dial`, and each `set-token` is one design token times at most 64 contexts, after the import’s budget has ended. No other walk in `planned`, `pen.rs`, `resolve.rs`, `kinds.rs`, or the CLI is missing from the table.
