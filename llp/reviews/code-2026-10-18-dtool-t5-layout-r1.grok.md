# Code review: the source map's `layout` on a written row (LLP 1122 D14, T5), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `2b7265ddf78e70b9655cee0e8c97febd8c3a38191933b8f043d8d0245af8f86f`), shared by both reviewers, blind to the other review; commits 386762a69f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** nothing blocks (both). Smaller points: the 1.12 bullet's splice of `layout` into stage 9 E's clause was already moved beside T5 in `e3d912aff6`; the untested `resize`/`flex`/`font-size` cases go through the same `paint_row` call (not taken: the predicate is unchanged and shared with set-row's refusal).

** This is the only value fork in `tags::rows`: an `Ident` or `Call` is the handler, anything else is CSS `resize` (`contract/lower/src/tags.rs:384-389`). The map always builds `Expr::Str("")` (`map.rs:270-275`). That does not diverge. A handler never pushes a style row, so the map never classifies it. A compiling CSS value (`resize="none"`; other keywords are `lower-css-resize`) takes `Styles(&[Resize])` for both the empty string and the real string. `resize` has no `layout` flag (`schema.json` field `resize`, default false), so both sides treat it as paint and the key is omitted.

## JSON and the rest of the map

`"layout":true` is inserted immediately after `quote` of `attr` (`map.rs:267-277`). `quote` is `serde_json`, so the name is a finished string before the comma. The boolean is unquoted. `Origin::State` does not take this branch, so the key is not duplicated and `state` / `at` are not shifted. Paint rows omit the key rather than sending `false`, which is what the optional 1.12 field requires. No other map field moves; the diff in `map.rs` is only this insertion.

## The move of `paint_row`

The body is unchanged. `tamber_rows` re-exports it (`tamber_rows.rs:105`). `check` still does its own `tags::rows` call on the real attribute, state prefix stripped. History still passes that real attribute (`contract/cli/src/tamber_history.rs:263`), so a handler `resize={…}` stays non-paint there, and a layout row on its own line stays code.

## Test

`a_written_row_says_its_attribute` checks the two sides of the predicate the ruling asked for: `padding_top` is `layout: true`, `color` has no key (`source_map.rs:516-518`). `color` is `text_color`, which is not a layout row.

Smaller points, not blocking. The 1.12 bullet splices "a written row's `layout` (D14)" into the stage 9 E `:active` clause (`docs/tamber-protocol.md:83` at this commit) instead of next to T5. The normative sentence is the right one (`docs/tamber-protocol.md:1127-1128`): `layout: true` where that attribute writes a layout row, by this test. The fixture also contains `font-size=14`, a single layout longhand, and does not assert it; flex, `resize="none"`, and a state row are untested. They go through the same `paint_row` call.
