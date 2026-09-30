# Nested Markdown links overflow the stack and abort the process

**Status:** Closed
**Resolution:** Fixed: link-label nesting stops at 32 and preserves deeper text literally; the 20,000-link public API subprocess and ordinary nested formatting pass.
**Systems:** Markdown, Reader
**Severity:** P1
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24

`markdown/src/inline.rs:402–404` recursively calls `Scanner::scan` on each
link's label. No nesting bound limits this recursion. A small document supplied
to the normal public styling API can abort the host process, rather than
produce literal text or a recoverable parse refusal.

Reproduced using an optimized build of the real `exact_markdown` library:

```rust
let n = 20_000;
let source = format!("{}x{}", "[".repeat(n), "](u)".repeat(n));
let _ = exact_markdown::style(&source, None);
```

This is 100,001 bytes. A separate probe process exited with signal 6 and
`fatal runtime error: stack overflow, aborting`. The reader/editor accepts
document text as data, so this is not confined to a developer compiling an
invalid Contract. Existing adversarial tests for unmatched brackets do not
exercise nested link labels. This is separate from the compiler view-depth
issue in `20260924-deep-view-stack-overflow.md`.

Bound nesting with a defined literal-text fallback, or make scanning iterative.
Use a subprocess regression so a recurrence cannot abort the entire test
runner. Cover both the failing document and ordinary nested inline formatting,
and ensure the bound applies wherever link-label scanning re-enters the parser.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Probe and recorded exit:
`/tmp/exact2-review-20260924/markdown-probe.rs` and
`/tmp/exact2-review-20260924/markdown-nesting.log`.
