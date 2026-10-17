# A memoized layout's first pass is quadratic in its boxes: Recorder::repeats scans every recorded call

**Status:** Closed
**Resolution:** Recorder::repeats now reads the asked box's calls alone (EXACT PATCH 42). Measured again for the fix in interleaved runs of one harness (the Problem's figures are an earlier run), #716's one-grid table of 5 columns and 2,000 rows opens in about 124 ms instead of 1.37–1.39 s, and a resize step takes about 52 ms instead of 1.29–1.30 s, in a release build of the kernel and runner. taffy's tree::memo::tests::a_recorded_call_is_looked_up_among_its_own_boxs_calls and a_lookup_answers_as_a_scan_of_every_recorded_call_does hold it.
**Systems:** vendor/taffy (EXACT PATCH 29), kernel layout
**Author:** Claude (Opus 5.5) for Daehyeon Mun
**Date:** 2026-10-17
**Related:** issues/20261016-markdown-on-macos-table-columns-do.md

## Problem

A remembered subtree's layout (EXACT PATCH 29) takes time quadratic in its boxes whenever the memo cannot answer it. A Markdown table drawn as one grid (`apps/markdown`, one list row) with 5 columns opens in 40 ms at 200 rows, 352 ms at 1,000 and 1.35 s at 2,000: a release build of the kernel and runner alone (no native views), from `apps/markdown/data/tests/table.rs`'s `open` with the monospace measurer. A relayout that changes the table's width pays it again, and so does one back to a width laid out before: about 360 ms a step at 1,000 rows and 1.5 s at 2,000, so resizing the window or toggling the folder pane stalls on every step. Only a relayout at the same width is answered (1.4 ms at 2,000 rows). Any list row or other remembered root with thousands of boxes pays the same.

## Cause

`Recorder::repeats` (`vendor/taffy/src/tree/memo.rs:371`) asks whether the trace already holds a `(node, LayoutInput)` call by scanning `asked`, a `Vec` of every call recorded so far. `taffy_tree.rs:551` asks it for every answered call (each measure or cache hit, several per grid item: min-content, max-content, final), and `answered` (`taffy_tree.rs:561`) adds one entry per call, so a pass over n boxes does O(n²) comparisons. A sampler put about 93% of that main thread's time there.

## Fix

Index `asked` by box so that `repeats` compares only that box's few inputs. EXACT PATCH 42 does it without hashing or a vector per box: each recorded call links to its box's call before it, and a `Vec` indexed by the trace's box indices (dense, as `overflows` uses them) holds each box's last call. A `HashMap<u32, Vec<LayoutInput>>` tried first gave 26 ms, 61 ms and 126 ms for the same opens, but allocates a vector per box. A document of many small rows (3,000 paragraphs and quotes, scrolled through) costs the same with the patch as without.
