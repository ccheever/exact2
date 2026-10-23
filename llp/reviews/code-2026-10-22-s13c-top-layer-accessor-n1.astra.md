# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 1, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6a110261a3`.
- **Target:** 7c2e2e0d10 (worktree 6a110261a3).
- **Method:** one brief (sha256 `09aa5e3304b39112151f326fce88cafc0966d9d77696578ca518fa2436202585`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in a8a6e5ae12 (the author-only sticky; the guard lexes code)

VERDICT: WRONG

1. **[blocker] [kernel/src/style/top_layer.rs:15](/tmp/iosfix-rv-a/kernel/src/style/top_layer.rs:15)** — The sticky exception changed semantics: `computed_source` includes platform defaults; the former `top_layer()` read `arena.style`. A neutral route header receives default `sticky` positioning without the author writing it ([schema.json:5818](/tmp/iosfix-rv-a/kernel/tables/schema.json:5818)). Give that header `popover`, and the old code makes it absolute; the new accessor returns sticky and `apply` leaves it in flow. This violates the stated **authored-sticky** exception. `NodeRef` forwards this behavior. The Rust and Swift formulas otherwise agree for equivalent supplied rows.

2. **[blocker] [scripts/position-reads.test.mjs:22](/tmp/iosfix-rv-a/scripts/position-reads.test.mjs:22)** — Ordinary direct reads escape the guard. Confirmed with its scanner, using an in-memory filesystem:
   - `let StyleProps { position_type: p, .. } = s;`
   - Multiline shorthand destructuring with `position_type,` on its own line.
   - `s . position_type`.

   Each produced zero findings; normal `s.position_type` was detected. The regex needs to recognize renamed patterns and reads across whitespace/lines.

3. **[blocker] [scripts/position-reads.test.mjs:63](/tmp/iosfix-rv-a/scripts/position-reads.test.mjs:63)** — Test skipping counts braces inside strings and comments. An unmatched `"{"` inside a `#[cfg(test)] mod tests` makes it erase subsequent production code through EOF. An ordinary production `s.position_type` after that module consequently escapes detection. Confirmed in memory for both string and comment cases. Brace matching must respect Rust strings and comments.

4. **[nit] [host/apple/Sources/ExactKit/LayoutPosition.swift:15](/tmp/iosfix-rv-a/host/apple/Sources/ExactKit/LayoutPosition.swift:15)** — This new authored-row read is named in the commit’s allowlist explanation but missing from its “every site” audit table. Add an accessor/A row. The other scoped production occurrences found by `grep -rn position_type` are accounted for.

The existing allowlist exceptions are justified for this mapping, including the sticky-only predicates and flat-leaf eligibility restriction. Excluding browser hosts is appropriate for this kernel-host accessor. The QUEUE update accurately describes the deferred scroll-anchoring work.

The guard’s first two tests passed. Its scratch-tree test failed with sandbox `EPERM`; the in-memory probes confirmed findings 2–3. No files changed.
