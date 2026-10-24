# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 4, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d6ac43e287`.
- **Target:** d6ac43e287.
- **Method:** one brief (sha256 `3c620fc1ad07d5986d2400489ca93b6509631cac1a9a55e1f8d5e81f9be7e5cd`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 6bf02edd22 (the row id only written out; renames, aliases, groups refused; interpolation comments)

VERDICT: WRONG

1. **[should-fix] [scripts/position-reads.test.mjs:169](/tmp/iosfix-rv-a4/scripts/position-reads.test.mjs:169)** — Renaming the imported variant escapes the guard:
   ```rust
   use crate::StyleId::{PositionType as Position};
   node.computed(Position);
   ```
   `directReads` returns nothing. `inUse` excludes the import, and `ALIAS` only recognizes `StyleId as`. The previous guard flagged this group import.

2. **[should-fix] [scripts/position-reads.test.mjs:131](/tmp/iosfix-rv-a4/scripts/position-reads.test.mjs:131)** — Interpolation counts parentheses inside comments. This valid Swift, accepted by the local parser, hides the subsequent read:
   ```swift
   let marker = "\(/* ) */ "/*")"
   let p = view.style["position_type"]
   ```
   The commented `)` prematurely ends interpolation; the nested quote then ends the outer string, exposing `/*` as a comment opener. `directReads` returns nothing. Handle comments before interpreting interpolation parentheses and quotes.

3. **[should-fix] [scripts/position-reads.test.mjs:28](/tmp/iosfix-rv-a4/scripts/position-reads.test.mjs:28)** — The colon exclusion also suppresses values in struct literals:
   ```rust
   use crate::StyleId::{PositionType};
   node.computed(Request { id: PositionType }.id);
   ```
   This also produces no reads: the import is excluded, and `id:` incorrectly makes the value look like a type annotation.

No false positives appeared in the current tree. The cover.rs allowlist move and audit corrections check out. Both repository checks passed; the scratch assertion passed with an in-memory filesystem. The normal scratch test was blocked by read-only `mkdtemp` permissions.
