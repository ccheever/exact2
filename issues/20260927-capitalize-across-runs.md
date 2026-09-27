# `text-transform: capitalize` on native hosts sees only the previous inline run, so "don't" split across spans becomes "Don'T"

**Status:** Fixed: carry Chrome's preceding-character context across empty and nested runs; run/shown_text regression tests and live Chrome verification agree. The reported apostrophe expectation was incorrect: Chrome renders Don'T Stop for those spans.
**Systems:** Kernel (`kernel/src/arena.rs`, `kernel/src/text/case.rs`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1064 D5

`kernel/src/arena.rs:571-577` passes `out.last()` as the context before a run. `kernel/src/text/case.rs:23-25` documents that context as the paragraph's text before this run.

**Failures:**
- Runs `"don"`, `"'"`, `"t stop"` give "Don'T Stop"; Chrome gives "Don't Stop".
- Runs `"foo"`, `""`, `"bar"` give "FooBar"; Chrome gives "Foobar".

**Fix:** carry word-boundary state across the paragraph's runs, skipping empty runs.

**Verified correction (2026-09-27):** Live Chrome, with `text-transform: capitalize`
on a div containing separate spans, renders `"don"`, `"'"`, `"t stop"` as
`"Don'T Stop"`, and `"foo"`, `""`, `"bar"` as `"Foobar"`. The same apostrophe
result holds with nested spans. Chrome carries the preceding source character,
not the preceding two characters: `"don'"`, `"t stop"` also gives `"Don'T Stop"`,
while `"don"`, `"'t stop"` gives `"Don't Stop"`. The fix follows these observed
boundaries, retains context through empty runs, and checks both paragraph runs
and the per-leaf `shown_text` projection. No design ruling remains.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max. Verification: confirmed by reading.
