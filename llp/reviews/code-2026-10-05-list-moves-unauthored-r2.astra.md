# Code review r2: a list's rows moved by the list play no layout-transition (66f3b8b51..b1476ea0d), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `b1476ea0d`.
- **Method:** one brief (sha256 `73ca79fdae13a5f80110dce0bed12b83322e3de3d0cd53dab3e059063456b33a`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r3):** 1 and 2 taken: what un-settles a list no longer reads motion targets. Before the layout's observations, each changed row wrapper whose size differs from what the presenter was last told (its content or margins), and each row box whose frame in its wrapper differs (moved or resized there), takes its list out of the settled ones; only a wrapper that moved, its size kept, is the list's own. The test now grows a row with no transition (the next row, which has one, must move) and nudges a row's box in its wrapper, and fails at the growth if that check is removed. 3 taken in part: the test asserts that a body change (growth, nudge) leaves the list's data generation where it was and only a change of keys moves it, which a generation over every data change would fail; nested lists and synchronous edge actions are DEFERRED. 4 taken in part: generations are matched through a map by view; the walk stays (no measurement).

---

DO NOT LAND

1. **MATERIAL — Growth detection misses rows without transitions.** [presence.rs:205](host/apple/src/presence.rs:205) requires an existing motion target; [kernel/src/motion.rs:369](kernel/src/motion.rs:369) also requires a transition declaration. With unchanged keys, grow row A from 40px to 80px, where only following row B declares `layout-transition`: A’s resize is ignored and B now snaps. **Fix:** compare previous/current row geometry independently of motion declarations. Test mixed animated and unanimated rows.

2. **MATERIAL — Authored position changes still snap.** [presence.rs:209](host/apple/src/presence.rs:209) compares only width/height, then [presence.rs:234](host/apple/src/presence.rs:234) settles the entire row. Changing a fixed-size row’s relative `left` from 0px to 20px preserves keys and dimensions, so its declared transition disappears. Increasing its positive top margin similarly snaps following rows. **Fix:** distinguish collection corrections from authored position/margin changes; test both with intermediate presentation assertions.

3. **MINOR — The clock-update justification remains untested.** The added action cases are only insertion, reorder and growth ([collection_tests.rs:533](host/apple/src/collection_tests.rs:533)). Incrementing the generation on every data/body change would still pass these tests, reintroducing the reported Signal failure. **Fix:** combine a same-key, same-size clock update with a correction that actually moves an existing row; assert its changed target and identity presentation. Add nested-list and synchronous edge-action coverage.

4. **MINOR — Per-layout cost remains unnecessarily high.** [collection_data](runner/src/instance/collection/traversal.rs:95) adds an instance traversal, and [presence.rs:185](host/apple/src/presence.rs:185) performs quadratic generation matching across mounted lists. **Fix:** index generations by view, reuse collection traversal where practical, and measure the added layout cost.

Round-1 reorder and report-scoping fixes hold; the growth disposition remains incomplete. No additional thread/lifetime/reload defect found. `caps` and `boot` pass; changed sources meet the 1,500-line limit. Rust tests and app drives were not run: this read-only checkout has no build artifacts.