# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 5, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6bf02edd22`.
- **Target:** 6bf02edd22.
- **Method:** one brief (sha256 `a4bac0f0766392c6f1dce1e26fd36780c8fd27fd99aa16278255f18639de586c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in a946d58d4f (Self::, grouped globs, renames after a glob, re-exports, module-wide globs, split aliases, Swift """ keys)

VERDICT: WRONG

1. **[should-fix] [scripts/position-reads.test.mjs:37](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:37)** — Grouped globs escape. `use crate::StyleId::{self, *};` followed by `s.get(PositionType)` produces no findings. This ordinary, rustfmt-preserved import matches neither `GLOB` nor `REFUSED`.

2. **[should-fix] [scripts/position-reads.test.mjs:187](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:187)** — Renaming an already imported variant escapes: `use crate::StyleId::*; use PositionType as P;` followed by `s.get(P)` produces no findings. `inUse` discards the variant’s name, and the rename rule requires `StyleId` in that same declaration.

3. **[should-fix] [scripts/position-reads.test.mjs:189](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:189)** — Re-exports defeat the file-local glob check. A parent importing `StyleId::*` and a child importing `super::*` lets the child call `s.get(PositionType)` undetected. Likewise, `mod ids { pub use crate::StyleId::*; }` permits an undetected `s.get(ids::PositionType)`. These are ordinary module/import forms.

4. **[should-fix] [scripts/position-reads.test.mjs:35](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:35)** — Type-alias rejection misses whitespace within the qualified path. `type Id = crate::` followed on the next line by `StyleId;`, then `s.get(Id::PositionType)`, produces no findings. The path matcher needs to tolerate token-separating whitespace.

5. **[should-fix] [scripts/position-reads.test.mjs:25](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:25)** — Inside an ordinary `impl StyleId`, `s.get(Self::PositionType)` escapes both read detection and alias refusal. The row remains reachable without spelling `StyleId::PositionType`.

6. **[should-fix] [scripts/position-reads.test.mjs:44](/tmp/iosfix-rv-a5/scripts/position-reads.test.mjs:44)** — Swift multiline keys escape:
   ```swift
   let p = view.style["""
   position_type
   """]
   ```
   Swift parses this key as `position_type`, but the scan returns no findings because it requires adjacent quotes.

The tree scan and allowlist checks pass. The scratch fixture passes with an in-memory filesystem; its normal run encounters `EPERM`. The interpolation comment fix works, and `cover.rs` preserves the same IDs, order, and condition. No files were modified.
