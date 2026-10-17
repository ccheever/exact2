# A memoized layout's first pass is quadratic in its boxes: Recorder::repeats scans every recorded call

**Status:** Open
**Systems:** vendor/taffy (EXACT PATCH 29), kernel layout
**Author:** Claude (Opus 5.5) for Daehyeon Mun
**Date:** 2026-10-17
**Related:** issues/closed/20261016-markdown-on-macos-table-columns-do.md

## Problem

The first layout of a remembered subtree (EXACT PATCH 29) takes time quadratic in its boxes. A Markdown table drawn as one grid (`apps/markdown`, one list row) with 5 columns opens in 40 ms at 200 rows, 352 ms at 1,000 and 1.35 s at 2,000: a release build of the kernel and runner alone (no native views), from `apps/markdown/data/tests/table.rs`'s `open` with the monospace measurer. A relayout that the memo answers takes 1.4 ms at 2,000 rows. Any one list row or other remembered root with thousands of boxes pays the same.

## Cause

`Recorder::repeats` (`vendor/taffy/src/tree/memo.rs:371`) asks whether the trace already holds a `(node, LayoutInput)` call by scanning `asked`, a `Vec` of every call recorded so far. `taffy_tree.rs:551` asks it once per box, so a pass over n boxes does O(n²) comparisons. A sampler put about 93% of that main thread's time there.

## Fix

Index `asked` by node so that `repeats` compares only that node's few inputs. With a `HashMap<u32, Vec<LayoutInput>>` (the recorder's `IdHasher`) in place of the `Vec`, the same opens took 26 ms, 61 ms and 126 ms. That map allocates a vector per box, so measure a document of many small rows (`apps/markdown-stress`) before choosing it over, say, a `Vec` that builds an index once it is long. `asked` has no other reader, and its order is not used.
