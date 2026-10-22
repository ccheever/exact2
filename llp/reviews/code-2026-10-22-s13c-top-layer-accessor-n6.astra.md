# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 6, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a946d58d4f`.
- **Target:** a946d58d4f.
- **Method:** one brief (sha256 `79cf239a4673d8762791d0c5b518109ef575a4da63d8619a0c837b15842b13c3`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 32d361f7eb (a glob in any module's scope); theme2 12:59: the bar is met once this round is in, an ordinary form found added with its test and Grok noting the fix

VERDICT: WRONG

1. **[should-fix] [scripts/position-reads.test.mjs:37](/tmp/iosfix-rv-a6/scripts/position-reads.test.mjs:37)** — The module-wide refusal only matches `use` at column zero. An ordinary nested module escapes:

   ```rust
   mod layout {
       use crate::StyleId::*;
       mod child {
           fn read(s: &crate::StyleProps) {
               let _ = s.get(super::PositionType);
           }
       }
   }
   ```

   `directReads()` returns `[]`: the import is indented, and `BARE` excludes qualified names. A separate child file using `super::*` also escapes. This is ordinary Rust, so it fails the ruling. Detect module scope independently of indentation.

2. **[nit] [scripts/position-reads.test.mjs:99](/tmp/iosfix-rv-a6/scripts/position-reads.test.mjs:99)** — Swift interpolation comments are skipped when finding the string boundary, but copied unchanged into the scanned text. Consequently, `let text = "\( /* style["position_type"] */ 1)"` falsely reports a read. Boundary handling works; comment removal remains incomplete.

3. **[nit] [scripts/position-reads.test.mjs:266](/tmp/iosfix-rv-a6/scripts/position-reads.test.mjs:266)** — The grouped-glob fixture shares its file with a plain `StyleId::*` import, masking regressions in grouped-glob recognition. Removing that recognition in memory still passes all three tests. Isolate this case from plain globs.

The real-tree and allowlist checks pass. `cover.rs` preserves the same four ids and predicate. The unchanged scratch fixture passes with an in-memory filesystem; its normal run stops at `mkdtemp` under the read-only sandbox. No files were modified.
