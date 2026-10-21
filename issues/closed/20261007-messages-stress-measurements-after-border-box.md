# messages-stress: 474 text measurements where the reuse test expects 368

**Status:** Closed
**Resolution:** SplitFacts facts are height-free under a height-free measurer: 16 rows 208 facts, 33 rows 429 of 768; layout unchanged
**Systems:** apps/messages-stress/data/tests/it/reuse/region_split.rs, vendor/taffy, kernel
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

Two `messages-stress-data` reuse tests fail on main:
- `full_messages_split_completes_all132_tail_paragraphs_and_latest_batch32`: `region_retention().accepted_facts` is 474, not 368;
- `selected_row_heights_and_live_focus_interaction_pins_survive_pending_reflow`: fails at its own count (line 48).

The gate never ran them: `cargo test --workspace` did not compile from 908ce0f23 until 0d7fa9bf7, and these crates are outside `default-members`.

**First bad commit** (git bisect on the mini, 40bef40c1..main): e962a65a8 "fix(layout): give child algorithms border-box available space" (2026-10-02, a vendored Taffy patch). Taffy now discovers about 30 scalar offers per row here, against 23. Final owners and the 768-fact cap are unchanged (474 is under it).

The test keeps an exact count "so lost reuse cannot hide in the cap". So the question is whether the extra offers are the patch's expected cost (then update both counts and the comment) or measuring work it should not do (then fix the discovery). The layout owner should decide; blessing the number would hide a 29% rise in text measurements per row.

## What it was (2026-10-07)

It was worse than the count: on main the 33-row stage no longer fit the 768-fact cap ("split scalar fact budget exhausted"), so the region could not finish that viewport; both tests failed there.

Traced by logging each accepted fact (paragraph, offer) on e962a65a8^ and on main. Patch 25's fit-content probes are needed: CSS sizes a non-stretched auto-width column-flex item (the sender line, the bubble with `max-width: 88%`, the Reply button) between its min- and max-content widths, and the browser fixtures hold that. What multiplied was the fact key: a SplitFacts fact was keyed by both axes, so the same paragraph at the same width was a separate fact under a definite, a min-content and a max-content height. Every measurer in use reads only the width (monospace; Linux and the terminal declare `TextMeasurer::height_free`; Apple's Swift answers key by width).

Fix: under SplitFacts a height-free measurer's facts match by width alone, as the ordinary measurement cache already does (`kernel/src/layout.rs`); a final owner is requested at its width under max-content height; `MonospaceMeasurer` declares `height_free`, which it always was. The default profile keeps one artifact per exact offer. The 16-row stage now takes 208 facts (13 a row; 368 before patch 25, 474 after), the 33-row stage 429 of 768. Geometry is checked against the ordinary layout in the same tests; a kernel test (`height_free_facts_answer_every_height_at_a_width`) pins both measurer kinds.
