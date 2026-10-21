# Code review: the source map's `layout` on a written row (LLP 1122 D14, T5), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `48feaf9bf69bd883c59827c99ff86cd742600d2716d81ae9a1890bb2c774ba61`), shared by both reviewers, blind to the other review; commits 386762a69f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** nothing blocks (Grok; Astra unavailable this round, model at capacity). Smaller point (the fixture's `font-size` not asserted) not taken: the same `paint_row` call.

Nothing blocks. Commit `386762a69f` is unchanged at this tip, and the later join commits consume `layout` the way `set-row` refuses that row.

`git diff 386762a69f HEAD` is empty for `contract/cli/src/map.rs`, `lib.rs`, `tamber_rows.rs`, and `contract/cli/tests/it/source_map.rs`. `tags::rows` and the style schema are unchanged since that commit. The classification is still `paint_row`: rows from the attribute name, `layout: true` only when that set is non-empty and any row `affects_layout`.

The map still builds that attribute as `Expr::Str("")`. The only value fork in `tags::rows` is `resize` as an `Ident` or `Call`, which is a handler and never a written style row, so it never reaches this branch. CSS `resize` is not a layout row (`schema.json` field `resize`), so the empty string and a real keyword agree and the key stays omitted. A shorthand is classified from all of its longhands, same as `check` in `tamber_rows.rs`: `border` marks its color bindings too, because its width and style re-run layout; `border-radius` does not. `paint_row`'s body is the old one. History still calls it through the `tamber_rows` re-export, on the real attribute. JSON is `,"layout":true` after `serde_json` quotes `attr`. State origins do not take `written()`, so they do not get a second `at` or a duplicate key. Those parts only admit paint rows (`background-color`, `color`, `opacity`, `box-shadow`, the press rows); none of those `affects_layout`.

`e3d912aff6` and `82d98133ff` do not change the producer. `rowRung` treats `b.layout` as `set-row`'s refusal, with the same reason string as `layout()` in `tamber_rows.rs`, and returns before a template or style rung. `editable` still offers a param whose whole value is that param, and a token whose whole value is that token. Otherwise it surfaces the reason and does not offer `set-row`. The selection row copies `layout: true`. The inspector only edits from `editable.rung`, so a layout row shows the reason and no row edit. The 1.12 bullet now sits beside T5, which is where round 1 said it belonged.

`a_written_row_says_its_attribute` still only asserts `padding_top` against `color`. `font-size=14` in that fixture is a layout longhand and is not asserted. It goes through the same `paint_row` call. Rust tests were not run; a build would write artifacts.
