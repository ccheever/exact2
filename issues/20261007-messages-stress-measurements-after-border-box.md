# messages-stress: 474 text measurements where the reuse test expects 368

**Status:** Open
**Systems:** apps/messages-stress/data/tests/it/reuse/region_split.rs, vendor/taffy, kernel
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

Two `messages-stress-data` reuse tests fail on main:
- `full_messages_split_completes_all132_tail_paragraphs_and_latest_batch32`: `region_retention().accepted_facts` is 474, not 368;
- `selected_row_heights_and_live_focus_interaction_pins_survive_pending_reflow`: fails at its own count (line 48).

The gate never ran them: `cargo test --workspace` did not compile from 908ce0f23 until 0d7fa9bf7, and these crates are outside `default-members`.

**First bad commit** (git bisect on the mini, 40bef40c1..main): e962a65a8 "fix(layout): give child algorithms border-box available space" (2026-10-02, a vendored Taffy patch). Taffy now discovers about 30 scalar offers per row here, against 23. Final owners and the 768-fact cap are unchanged (474 is under it).

The test keeps an exact count "so lost reuse cannot hide in the cap". So the question is whether the extra offers are the patch's expected cost (then update both counts and the comment) or measuring work it should not do (then fix the discovery). The layout owner should decide; blessing the number would hide a 29% rise in text measurements per row.
