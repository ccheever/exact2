# Code review: The write check judges written shorthand and flex rows as lowering expands them, round 2, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `dc55f555c6`.
- **Method:** one brief (sha256 `7e8ccf33776b5c64ef5e8b1d4fa708828b6aaee4792e0a574b11c422bf72187f`), blind to the other reviewer; 3 rounds (theme2's ruling on the shorthand plan), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the write-check change against the build's whole-row judging: how shorthand and flex rows are typed, spanned, and refused. Reading the review instructions and the diff first.The review is read-only and stays on the requested range. I'll inspect the whole change and the round-1 fix directly.No findings.

Whole-row judging matches the build for a written shorthand or flex choice. `grouped` keeps the choice as one row, with `Place` at the attribute and the literal at the value span. `judge_expanded` expands each longhand with `flex_component` or `shorthands::component`, in the build's order, and types the expanded value the way `typed_code` does: `Number` when every leaf is a number, `String` when none is, `Unknown` when they mix. `check_style_value` then runs on that one longhand.

An error inside an arm keeps the build's span. That span is the arm literal, so it matches neither the attribute nor the whole value, and the relocation in `check` leaves it as `judge` returned it. For one place that relocation is the identity. Copies of the same value share that span, so they stay on it too.

Passes-before-fails-after is on the whole row. `judge` returns the first longhand error, the same one the build stops on. The row is refused only when that judgment was `Ok(true)` before the write and an error after. A choice that already failed is left to the build, as a computed or font row is.

The new test is red without the fix. With `gap` a colour and `bad` a number, `measure_row` skips the colour (colours are not measures) and refuses `bad` on the basis. The old per-arm split judged the earlier arm on its own and reported the colour first. `a_shorthands_longhand_is_refused_as_the_build_refuses_it` now uses a flex choice, so it goes through `component()` rather than the box-shorthand `sides` path.
