# Unclosed Markdown link destinations repeatedly scan the remaining paragraph

**Status:** Closed
**Resolution:** Fixed: balanced destination parentheses are indexed once per inline scan; 4,000/16,000 failed destinations and escaped/balanced link regressions pass.
**Systems:** Markdown, Reader, Editor
**Severity:** P2
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24

`markdown/src/inline.rs:371–384` scans from each `[label](` through the rest of
the inline range while looking for its closing parenthesis. On failure it
returns `None`, and the outer scan subsequently tries the next opener. With
`"[x](".repeat(n)`, every opener rescans a slightly shorter suffix: quadratic
work for a document that should render as ordinary text.

Reproduced with `exact_markdown::style(&"[x](".repeat(n), None)` against an
optimized build of the actual library. Observed times were approximately
1.28 s for 16 KB, 4.02 s for 32 KB, and 15.87 s for 64 KB. The machine was
under concurrent load, so these are observations, not stable performance
budgets; the nested suffix scans establish the asymptotic problem independently.
Styling is synchronous, so pasting or opening this text can block the reader
or editor for seconds.

Avoid repeating unsuccessful destination searches, for example by indexing
matching parentheses or retaining failed-search information. Preserve escape
handling and valid balanced destinations. Add a growth/work-count regression
for repeated unclosed destinations; the existing unmatched-bracket case does
not cover this path. This is distinct from recursive link-label stack overflow
and footnote label lookup, which have separate causes and fixes.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Reproduction source and
measurements: `/tmp/exact2-review-20260924/markdown-probe.rs` and
`/tmp/exact2-review-20260924/markdown-timings.log`.
