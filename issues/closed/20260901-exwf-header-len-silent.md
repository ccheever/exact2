# Revision-1 EXWF `header_len` larger than 40 silently drops ops

**Status:** Closed
**Resolution:** EXWF revision one now requires the exact declared header length.
**Systems:** Kernel
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §4 (a malformation names a `DecodeError` and applies nothing)

Revision 1 requires `header_len >= 40` and 8-aligned, then skips `header_len - 40` bytes (`kernel/src/wire/frame.rs`). The comment claims this is how a rev-1 reader ignores a future larger header, but `revision != 1` is already `UnsupportedRevision`, so that path never serves its stated purpose.

On revision 1 it is fail-open: `header_len == frame_len` (8-aligned, ≥ 40) consumes every op as “header extension” and `decode` returns a valid empty `Frame`. `Kernel::apply_frame` then commits nothing and reports `Ok`. An inflated length is a malformation that currently applies nothing *silently*, indistinguishable from an empty batch.

Fix: on revision 1, require `header_len == HEADER_LEN`. Grow the header only by bumping the revision. Test: a rev-1 frame with `header_len = frame_len` must be `BadHeaderLength`, not an empty apply.
