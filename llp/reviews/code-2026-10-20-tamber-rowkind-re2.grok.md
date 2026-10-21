# Code review: LLP 1121 D3/D7, the design server's write-time row check, narrow re-check 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `8f1bba2ba3`.
- **Method:** one brief (sha256 `3bf9ae549bcbbbe638544373098e5a65e4f73c08167b8a3ae837ef461047d4cf`); the commit as amended after the narrow re-check and rebased onto main with LLP 1139 stage 12 C; the reviewer who raised the last SHOULD-FIX. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve with changes (2 NIT).
- **Disposition:** both taken (wording in QUEUE, scan.rs, the test comment and the commit message).

**RIGHT WITH CHANGES**

1. **NIT** — `QUEUE.md:17` still has a stray `)` after the `let` rewrite: "as the row) and a literal a derive binds". The claim matches `written` in `rows.rs`: a `let` is not a map row; `leaves` substitutes the binding and `measure_row` kind-checks that literal as the row. A derive's literal stays "checked where it is bound".

2. **NIT** — "coerces the rest" (`scan.rs:125`, the test comment, `QUEUE.md`, the commit) overshoots. `svg::coerce` renames a few attributes (`fePointLight` `x` → `lightX`, SVG `text` `x` → `textX`, `scale` → `feScale`) and rewrites unitless geometry numbers and non-string length props. `rect x="pos"` is neither: `coerce` returns `None`, `x` stays `StyleId::X`, and a colour is `tamber-kind` at the build. The operational line is right: the scan records no svg-subtree rows, and none of the four texts say the build accepts that colour.

What checks out: (b) `rows_the_build_judges_otherwise_are_left_to_it` asserts `w.get("ok").is_some()` on each of the three writes; success replies carry `ok`, errors carry `error`. (d) The module list keeps main's `pub(crate) mod scan` (`anatomy` calls `theme::scan::words`) and adds `bind` and `rows`. C3's `theme.rs` hunks are intact: `pub(crate) fn role_named`, and `anatomy::param_lines` before `entry_errors` and `written_ranges`.
