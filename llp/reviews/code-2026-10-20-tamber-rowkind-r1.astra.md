# Code review: LLP 1121 D3/D7, the design server's write-time row check, round 1 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `aead237cc3`.
- **Method:** one brief (sha256 `746161055ef9f6dd885a870af6bd6e913b453dccace5cd5d0a3c115bd9136f70`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (BLOCKING findings).
- **Disposition:** each taken in the next amend (the commit message's "Round 1").

Round 1: **changes needed**.

- **BLOCKING — false refusals for unused or overridden styles.** [scan.rs:125](contract/lower/src/theme/scan.rs:125) records every style declaration. Removing `ink` used only by an unused style—or a style row overridden at every use—therefore refuses the write, although the build accepts it. Compiler probes confirmed both cases. Collect rows that lowering actually checks.

- **BLOCKING — `judged` lacks compiler preprocessing and context.** [rows.rs:240](contract/lower/src/theme/rows.rs:240) checks shorthand text directly against its longhands. For `border="1px solid ink"`, both judgments fail, so changing `ink` from colour to length escapes refusal despite breaking the build. Passing `fonts=[]` also misses `font-weight="weight"` changing `400 → 700` on a declared family without a bold face. Share shorthand projection and preserve the required font context.

- **BLOCKING — motion dependencies disappear from the map.** [scan.rs:110](contract/lower/src/theme/scan.rs:110) records attributes after loader motion substitution. `transition="opacity duration.fast ease"` becomes literal timing text, so [rows.rs:99](contract/lower/src/theme/rows.rs:99) cannot reach it when `duration.fast` becomes a colour or is removed. The build then refuses, but the write check misses this covered literal row. Retain its original dependencies and judge the candidate substitution.

- **SHOULD-FIX — duplicate diagnostics change columns.** [rows.rs:269](contract/lower/src/theme/rows.rs:269) relocates every nondefault span to the literal. Removing `ink` from repeated `box-shadow="0 0 2px ink"` rows produces attribute-position errors in the compiler; reused errors instead point at the string. Preserve whether the original refusal targeted the attribute or value.

- **SHOULD-FIX — outside-package diagnostics expose absolute paths.** [tamber_scrub.rs:228](contract/cli/src/tamber_scrub.rs:228) falls back to the absolute source path for hoisted or linked packages. Use `tamber_uses::shown`, consistent with tamber/1.15’s file naming.

- **SHOULD-FIX — incomplete thread-local cleanup.** [rows.rs:212](contract/lower/src/theme/rows.rs:212) leaves `SHOWN`, `PARAMS`, measures, motion and axes changed. More concretely, [bind.rs:243](contract/lower/src/theme/bind.rs:243) appends role records on every check, accumulating them across cache-hit writes. The kernel theme scope restores correctly, and later successful lowering resets these cells, but server checks need scoped restoration.

- **SHOULD-FIX — the budget test does not establish the promised bound.** [tamber_rows_check.rs:167](contract/cli/tests/it/tamber_rows_check.rs:167) supplies only six distinct row/value pairs after deduplication; the clone case supplies three. Candidate parsing/folding is outside the timer. The approved shared `WriteFold` is also absent: existing checks and `rows_check` fold separately. Measure distinct reached rows through the actual check entry point.

- **NIT — update the author-facing behavior description.** [tamber-protocol.md:685](docs/tamber-protocol.md:685) still says the breaking edit itself is live. Document the new refusal and “passes before, fails after” rule. [QUEUE.md:15](QUEUE.md:15) names the intended exclusions, but overstates coverage until the gaps above close.

What checks out: I found no behavior regression in the compiler binding extraction: refusal ordering/spans and axes → tokens → font-stack builder order are preserved. Lazy-before judgment, removed-name handling, ordinary reference/base reach, source-ID/path capture, Contract-edit skipping, and the verifying-scrub hook are coherent. The changed retention tests appropriately move invalid edits onto disk.

Read-only validation: **9 existing tests passed**, including both budget tests, plus stdin compiler probes using existing binaries. Full server suites were not rerun because they create files. No files changed.
