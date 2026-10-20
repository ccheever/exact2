# Code review: LLP 1121 D3/D7, the design server's write-time row check, round 3 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `d05f9aec6c`.
- **Method:** one brief (sha256 `38cf8b90a8e87ad5abc218cc3b781f5652c7fc00e49568637ba2cee745f490f7`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX).
- **Disposition:** taken ("Round 3": an attribute-level `let` is no row, and the text says so).

I'll review HEAD read-only: first the plan, theme2's ruling, and the prior-round findings, then the commit itself.The ruling is in hand. Next I’ll read the row check, the bind refactor, and the server path, and check the round-1 and round-2 fixes against the code.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open on two points: whether the commit claims to judge `let` leaves, and whether the `.contract` skip can fire on a design-only write. I'll confirm those against the code, then grade.The `.contract` skip only sees files the plan actually edits. Next I’ll check binding, the `let` disclaimer against the build, and whether the row map and spans match.No BLOCKING findings. One SHOULD-FIX: an attribute-level `let` is a row this commit still says it judges, and a kind change there is a write the build refuses and the check accepts.

## SHOULD-FIX

**An attribute-level `let` of a literal is dropped, so a kind change the build reports at that row is accepted** — `contract/lower/src/theme/rows.rs:118`

`written` records a string or a number, and each arm of a `?:` or `match`, and then stops. `Expr::Let` falls through and records nothing, so `padding={let c = "gap" in c}` never enters the map. The build does judge that row: `measure_row` (`contract/lower/src/theme.rs:977`) walks `leaves` (`theme.rs:940`), which resolves the `let` and kind-checks the bound string. Changing `gap` from `8px` to a colour is `tamber-kind` at the row. The write check accepts it.

Round 2's derive case is a different row and is right to accept. `color=chosen` is an `Ident`; `leaves` does not turn it into `"mute"`, and `style_value` on a `let` only pushes the body (`contract/lower/src/values.rs:370`), which is why substituting the bound string was a false refusal. Keeping the original `let` expression and calling `check_style_value` on it would kind-check through `leaves` and still not show the bound string to the codec.

The close contradicts itself. `QUEUE.md:15` says the check judges "each literal of a `?:`, `match` or `let`" and, later in the same bullet, that "a literal a `let` or a derive binds" is left to the build because it is "checked where it is bound". The same "through `let`" claim is in `literal_row`'s comment (`rows.rs:89`) and in `Said.rows` (`contract/lower/src/theme/scan.rs:93`). For this row the bound literal is kind-checked as the row.

## What checks out

Round 1 and round 2 fixes are in the code: `font-weight` and `font-style` are out with `font` and `font-family`; `flex` is out with shorthands; motion rows match by `transition` or `animation` in the name, so `-exact-layout-transition` and `-exact-exit-animation` are out; a literal arm next to a computed arm is kept; a choice arm is stored at the literal's span; reach walks `.names` and `.bases`; diagnostics go through `tamber_uses::shown`.

The binding move matches the old `declare_theme` order. `fold` drops a loose synthetic and returns `None` on a design-line error. `profiles` and `admitted` are the same checks. `bind` fills entries, then motion, scale, measures, and axes, then `sink.plan` and `sink.stacks`. `Lowering::plan` writes axes, then kept tokens. `Lowering::stacks` sets `self.theme`, then `theme_stacks()`, and `declare_theme` assigns `self.theme` again after `bind`. `Quiet::push` drops bind refusals. `Quiet::said` is always false, and the unfolded-length entry is still skipped, so the table matches. I did not re-run the 46-app byte compare.

`check` refuses only a row that fails after and passed before. An empty or unfoldable before refuses nothing. Turning one failure into another is left alone. `judged` reuses the caller's fold only when nothing synthetic and no profile colour joins it; otherwise it refolds. `before` is the closure in `rows_check`, so it lays only after a failure.

Thread-locals: `Kept` restores `SHOWN`, `PARAMS`, `MEASURES`, `MOTION`, `AXES`, `ROLES`, and the scale's declared names, and the kernel `ThemeScope` drops first and puts the previous theme table back. The check does not write `ROWS`, `CANDIDATES`, `RETENTION`, or accent. A later retention build clears those tables at the start of `declare_theme`. The unit test covers measures, roles, and the scale names.

The map is the expanded root's element attributes only. `file.styles` feed words and synthetics, not rows. Places keep the parser spans. `keep_paths(sources.paths())` runs in that same compile (`contract/cli/src/lib.rs:815`), and `retention_build` returns those paths with the rows.

`planned.files` is only files whose bytes changed (`tamber/src/session.rs:190`), so a design-only write is checked and a write that also edits a `.contract` is skipped. `touched` is the design's changed names plus names the old parse had and the new one lacks. The design passed to `check` is `folded.design` from `fold_with` (packages joined, params laid). Package-fold errors return before the row check. Rows are stored under the Contract sources key and are not refreshed on a retention-key hit, which matches a map that comes from Contract. `transaction` calls `synthetic` before `write_checks`. A scrub runs the check when `verify` is true. The first diagnostic's file, line, and col come from the remapped span and `paths[source_id]`.

The protocol paragraph (`docs/tamber-protocol.md:686`) matches that rule and points at `QUEUE` for the rows left to the build.

The tests lock same words for a kind change, a press scale through an alias and through a value, a removed name, a reference chain, a base name, a choice arm, and a mixed arm; alike rows against the build's full list; an unused style and a shorthand accepted; a derive accepted; an already-failing row accepted; the fresh-versus-reused map. The budget test times `check_design` plus `rows::check` over 102 distinct rows, best of three, passing and breaking, under 10 ms. That bound is the whole check it claims to time. I did not re-run it, and nothing in the test makes it flaky by construction.

Not covered, and already named in `QUEUE` or in Round 2: state-part rows, font rows, shorthands and `flex`, motion rows, a `style`'s rows, a Contract source edited in the same write, a verifying scrub frame, a second source file. `same_words` compares diagnostics[0] only.
