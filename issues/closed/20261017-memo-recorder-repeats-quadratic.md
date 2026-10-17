# A memoized layout's first pass is quadratic in its boxes: Recorder::repeats scans every recorded call

**Status:** Closed
**Resolution:** Recorder::repeats now reads the asked box's calls alone: asked links each call to that box's call before it, and last_asked holds each box's last by trace index (EXACT PATCH 37). In a release build of the kernel and runner, a 5-column table of 2,000 rows drawn as one grid opens in about 124 ms instead of 1.39 s, and a resize step takes about 52 ms instead of 1.3 s. taffy's tree::memo::tests::a_recorded_call_is_looked_up_among_its_own_boxs_calls holds it.
**Systems:** vendor/taffy (EXACT PATCH 29), kernel layout
**Author:** Claude (Opus 5.5) for Daehyeon Mun
**Date:** 2026-10-17
**Related:** issues/closed/20261016-markdown-on-macos-table-columns-do.md

## Problem

A remembered subtree's layout (EXACT PATCH 29) takes time quadratic in its boxes whenever the memo cannot answer it. A Markdown table drawn as one grid (`apps/markdown`, one list row) with 5 columns opens in 40 ms at 200 rows, 352 ms at 1,000 and 1.35 s at 2,000: a release build of the kernel and runner alone (no native views), from `apps/markdown/data/tests/table.rs`'s `open` with the monospace measurer. A relayout that changes the table's width pays it again, and so does one back to a width laid out before: about 360 ms a step at 1,000 rows and 1.5 s at 2,000, so resizing the window or toggling the folder pane stalls on every step. Only a relayout at the same width is answered (1.4 ms at 2,000 rows). Any list row or other remembered root with thousands of boxes pays the same.

## Cause

`Recorder::repeats` (`vendor/taffy/src/tree/memo.rs:371`) asks whether the trace already holds a `(node, LayoutInput)` call by scanning `asked`, a `Vec` of every call recorded so far. `taffy_tree.rs:551` asks it for every answered call (each measure or cache hit, several per grid item: min-content, max-content, final), and `answered` (`taffy_tree.rs:561`) adds one entry per call, so a pass over n boxes does O(n²) comparisons. A sampler put about 93% of that main thread's time there.

## Fix

Index `asked` by node so that `repeats` compares only that node's few inputs. With a `HashMap<u32, Vec<LayoutInput>>` (the recorder's `IdHasher`) in place of the `Vec`, the same opens took 26 ms, 61 ms and 126 ms. That map allocates a vector per box. The trace's node indices are dense (`count` only grows) and `overflows` is already a `Vec` indexed by them, so a `Vec` indexed by node, holding a node's first input inline, would avoid the hashing and most of those allocations. Measure a document of many small rows (`apps/markdown-stress`) before choosing. `asked` has no other reader, and its order is not used.
