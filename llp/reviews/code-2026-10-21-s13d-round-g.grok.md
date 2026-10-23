# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), narrow re-check, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `53c45e6877`.
- **Method:** one brief (sha256 `b131f7b0091e86399a2210937d240ed8cf486d9d65fdfdc1cd9b6619b43f7ca1`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check only the stage 13 D write-check fixes in those two commits, read-only, and verify they don't refuse a write the build accepts or mismatch words and spans.The two commits change how the walk treats audio `display`, grouped-list looks, and hand-written swipes. I'll read those paths against the build so a kept row or a dropped one can be checked against what `contract build` actually accepts.Nothing blocks. The two fixes match the build on the paths that refuse a write, and the new tests lock the four cases you named.

1. **Audio `display`.** `scan.rs:344` drops every `display` on an `audio`, so a class `display` cannot put a row into the check that `media::audio_rows` rewrites. `scan.rs:308-311` still sets `left` when `audio_rows` returns an error, and that `left` covers the element and its children. A `display` the build keeps (literal `controls`) is the allowed miss. `an_audios_display_with_a_class_is_the_builds` is that case.

2. **Grouped section body.** `grouped_list` (`scan.rs:129-145`) is `grouped::style` over the class rows plus the element's own. Children of that list are `Grouped::Sections`; a `section` there marks its children `Grouped::Row` (`scan.rs:379-386`). A marked node's look is dropped only in that body, and not on a `header` or `footer` (`scan.rs:302-304`). A plain `section` stays `Grouped::No`, so `a_marked_box_in_a_plain_section_keeps_its_look` judges `padding-left="gap"` at the same attribute the build does.

3. **Hand-written swipe.** A section-body element with `swipeContent` marks its descendants `Grouped::Swipe`, and that state stays on every descendant (`scan.rs:381-384`). A marked node there yields its look (`scan.rs:302-304`), including the named row in `a_hand_written_swipes_row_yields_its_look`. That is the same row `swipe_rows::written` styles.

4. **Gate.** The before-verdict half of `a_write_fixing_the_failing_row_is_accepted_before_and_after_the_verdict` starts the server with `EXACT_TAMBER_CHECK_GATE` (`tamber_rows_check.rs:1033-1034`). The worker blocks in `remove_file` until the test writes that file after the fixing write (`tamber_check.rs:197-201`).
