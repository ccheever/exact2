# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), D3 round 3, D1/D2 narrow re-check, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `700c41d3d0`.
- **Method:** one brief (sha256 `d1e539c15ec30edc73d4671d0cc6ca22c6dea8d1aa0340a15782e3dc5057fe24`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the stage 13 D write check read-only: the row-look yield fix, the `seen_failed` gate, and whether a write the build accepts can still be refused.This is a targeted read-only review of the write-check fix, not a branch-wide review. I'll start from the commit range and the row-check path.The round-4 diff is small. I'll read the check, the scan, and how `last_key` is set before the write checks run.Auto-compacting conversation (81% full)...
Conversation compacted.
The look-filter and the failure gate are the two places this commit can still refuse in the wrong words. I'll read those paths against the build's actual yield and first-error order.The yielded-row fix holds for a real grouped row, and the failure gate is safe on a transaction. The look skip is wider than the build's yield, and that is the one place a write is refused in different words.

1. **Major, D3.** `contract/lower/src/theme/scan.rs:237` (filter at `:254`). `yields` is every element that carries `-exact-row-look`. `row_look::yielded` runs only from `grouped::row` (`contract/lower/src/grouped.rs:789`), which `grouped::sections` applies to a section's row body. A section `header`/`footer` is `label`, which appends the author's attributes (`grouped.rs:563`) and does not yield. A nested part is `with` or cloned (`grouped.rs:1212` and the other `part` arms). Anywhere else, `swipe_rows::stray` (`contract/lower/src/swipe_rows.rs:688`) removes the marker and leaves the look. The build then style-checks `padding-left`, `background-color`, `border-color`, `box-shadow`.

   A file that builds, with the marker on a box that is not that row (standalone, or a section header, or a box nested inside a row):

   `box -exact-row-look="platform" padding-left="gap" margin-left="gap"`

   and `gap` changed from `8px` to a colour. `prepare_element` collects both failures in source order and does not return on the first, so `contract build`'s first diagnostic is `padding-left`. The check drops that row and refuses at `margin-left`: same check, different line and column. A look-only break on that node is only a missed refusal, which is allowed. The new test stays valid: that `svg` is the section row, an outer `svg` is not `is_element`, so `prepared` does not set `left`, and the filter is what lets the write through.

   **Fix:** set `yields` only for an element `grouped::sections` actually passes to `yielded` (a section row, including through `if`/`match`; not a header or footer, not a descendant `part` only restyles, not a node outside that list). Keep judging look rows on every other marked node. Extend the test with the standalone box above and assert the write diagnostic is the build's first (`padding-left`).

2. **Minor, D3.** `contract/cli/src/tamber_scrub.rs:180`. On `run`, `synthetic` sets `last_key` and `known` polls before `write_checks` (`tamber_serve.rs:433`, `:1125`, `:1160`). If this fingerprint has passed, the verdict is `Passed` before the gate, so a real row refusal is not suppressed. If it is still `Failed`, the file is already not a build the write check may accept, and the answer is `rebuild`. The gate only returns `Ok`; it cannot refuse a write the build accepts.

   A scrub verify does not call `synthetic`. It checks `base.snap` under `base.key` (`known_under`), but the gate reads `self.last_key`. A later `propose`, `uses`, or transaction can move `last_key`. If the opened texts already fail the whole build and the new key is not `Failed`, verify judges rows and can refuse at a row while the build of those texts fails first at the structural error. The pre-verdict window in `QUEUE.md` is the same limit when the key has not moved; this is the key moving out from under an open scrub.

   **Fix:** judge `seen_failed` on the retention key of the texts being checked. `run` passes the `last_key` it just set; scrub verify passes `base.key`.

**D1/D2.** `700c41d3d0` does not change font rows, `typed`, font-file staleness, or computed-row judgment. Look prefixes are not font names. The gate can only skip those refusals while the key is already `Failed`, which is a miss, not a wrong refusal.

Nothing blocks.
