# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), round 3, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7272245fe7`.
- **Method:** one brief (sha256 `6666a9434fd01f6a00f1458279c9c56a013d0f1180f883ceb88cda2923d466bb`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the full style-row write check against the build's refusal rules, including round 2's fixes, and look only for remaining false refusals or mismatched words, place, or element.The tree is clean and the review target is the style-row write check from `4f214a52b9` through `HEAD`. I'll read that diff and the build's refusal rules before judging anything.The diff is the style-row write check plus its tests. Next I’ll compare how the check collects and refuses those rows with the order and words the build uses.Two places still report a refusal that is not the build's first. Both show up when one token is named by more than one row.

**[P1] A class row the check skips can be the build's first failure — `contract/lower/src/theme/scan.rs:557`**

`kept` drops `flex` and any shorthand other than `font` (`rows.rs` `style_row`), and the `continue` at `scan.rs:557` does not put that span in `unjudged`. The build still value-checks those rows, in style order, before the longhands. `flex="gap"` becomes a `flex-basis` of the word `gap` (`values::flex_component`), and `columns="gap"` keeps `gap` as the column width (`shorthands::columns`); with `gap = 8px` both build, and so does a later `padding="gap"`. Setting `gap` to `#444444` makes the build refuse the `flex` or `columns` row first. The check never stored that row, so it refuses `padding` instead: different line, different words.

Judge those rows the way `attr` does (`flex_component` / `shorthand::component`, then `check_style_value`), so the earlier row is in the map and can be the one refusal. Add a `same_words` case for `flex="gap"` then `padding="gap"` on one plain element.

**[P1] Style refusals are emitted after own rows, and in source order rather than lowering order — `contract/lower/src/theme/rows.rs:602`**

`check` pushes every element-0 refusal into `out`, then appends style refusals from a `BTreeMap` keyed by span (`rows.rs:565`, `rows.rs:602`). The server's first diagnostic is that vector's first item (`tamber_scrub.rs`). The build's first is whatever it reaches first: elements in `order()`, and within an element the class rows before the element's own rows.

Two cases the tests do not lock, because the two-element test uses one row so both elements say the same thing:

- `style Pad / padding="gap"` on a column that also has `margin="gap"`. The build refuses `padding` (the class row). The check refuses `margin` first.
- `style Pad / padding="gap"` declared above `style Wide / margin="gap"`, with `Wide` on the earlier element. The map emits `padding` (earlier span). The build refuses `margin`.

Give own rows the same element index `order()` already computes (they are stored as `0` today, which is also the "not a style" flag). Emit one list sorted by `(element, class row before the element's own row, span)`. Keep one refusal per style span, still the smallest element index.
