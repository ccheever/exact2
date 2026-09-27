# `text-transform: capitalize` on native hosts sees only the previous inline run, so "don't" split across spans becomes "Don'T"

**Status:** Open
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

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max. Verification: confirmed by reading.
