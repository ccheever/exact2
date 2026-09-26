# Footnote numbering performs a linear label search for every reference

**Status:** Open
**Systems:** Markdown, Reader, Editor
**Severity:** P2
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24

`markdown/src/style.rs:199–207` stores footnotes in a vector and uses
`.position()` to find each newly encountered label. A document with n distinct
references makes roughly n²/2 label comparisons. The `ordinal` closure at
lines 240–244 repeats the linear lookup while building the styled output.
Definitions need not exist for this work to occur.

Reproduced against the optimized `exact_markdown` library:

```rust
let source: String = (0..16_000).map(|i| format!("[^f{i}] ")).collect();
let _ = exact_markdown::style(&source, None);
```

The 148,890-byte document took about 4.82 seconds in the review run; 8,000
references took about 2.72 seconds. Concurrent machine load makes exact
timings noisy. The source-level repeated full-vector search is the quadratic
cost, and styling runs synchronously in the document path.

Keep first-reference order in the existing vector, but index labels to their
ordinals with a map and reuse that index during rendering. Verify ordering
with repeated labels, definitions preceding/following references, and unused
definitions. Add a growth/work-count regression for many distinct labels.
This is separate from the unclosed-destination scanner issue.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Reproduction source and
measurements: `/tmp/exact2-review-20260924/markdown-probe.rs` and
`/tmp/exact2-review-20260924/markdown-timings.log`.
