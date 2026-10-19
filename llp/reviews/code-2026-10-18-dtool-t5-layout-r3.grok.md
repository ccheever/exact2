# Code review: the source map's `layout` on a written row (LLP 1122 D14, T5), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `922498a9a9`.
- **Method:** one brief (sha256 `6dc4b92760c760ece5efca8dfa733ff4e0a6c401683e9346b09482084241507d`), shared by both reviewers, blind to the other review; commits 386762a69f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** nothing blocks (both). Three rounds, as theme2 ruled for the wire field.

Nothing blocks. Commit `386762a69f` is unchanged at this tip (`922498a9a9`, clean tree), and `scripts/tamber-inspect.mjs` still consumes `layout` exactly as `bc485dd65f` left it.

`git diff 386762a69f HEAD` is empty for `contract/cli/src/lib.rs`, `map.rs`, `tamber_rows.rs`, and `contract/cli/tests/it/source_map.rs`. `tags::rows` and the style schema are unchanged since that commit. `paint_row` is still the library function: rows from the attribute name, and the map writes `,"layout":true` only when that set is non-empty and any row `affects_layout`. The synthetic attribute is still `Expr::Str("")`. The only value fork in `tags::rows` is `resize` as an `Ident` or `Call`, which is a handler and never a written style row, so it never reaches this branch. CSS `resize` is not a layout row, so the empty string and a real keyword agree and the key stays omitted. A shorthand is classified from all of its longhands, same as `check`: `border` marks its color bindings too, because its width re-runs layout; `border-radius` does not. `Origin::State` does not take `written()`, so a state row gets no `layout` key. State parts only admit paint rows.

`git diff bc485dd65f HEAD` is empty for `scripts/tamber-inspect.mjs`, its test, and `docs/tamber-protocol.md`. `rowRung` still treats `b.layout` as `set-row`'s refusal, with the same reason string as `layout()` in `tamber_rows.rs`, and returns before a template or style rung. `editable` still offers a param whose whole value is that param (no template address, so no detach), and a token whose whole value is that token. Otherwise it surfaces the reason and does not offer `set-row`. The selection row copies `layout: true`. The inspector only edits from `editable.rung`, so a layout row shows the reason and no row edit. On a held look, `rowRung` runs only when that binding has `state`. A hover row with no place of its own is the base binding and is not offered, and a state binding never carries `layout`. `54d7784b9f` does not mention `layout`.

`a_written_row_says_its_attribute` still only asserts `padding_top` against `color`. `font-size=14` in that fixture is a layout longhand and is not asserted. It goes through the same `paint_row` call. Rust tests were not run; a build would write artifacts.
