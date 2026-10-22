# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 3, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9540ee1a92`.
- **Target:** ab4c231328..9540ee1a92.
- **Method:** one brief (sha256 `14251b2c54216059712a67f01b245600d0dd189c7ea56e3fdff3a2d28f3f87c0`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in d6ac43e287 (Swift interpolation and raw escapes; a bare row id after a group import)

VERDICT: WRONG

1. **should-fix** — [scripts/position-reads.test.mjs:86](/tmp/iosfix-rv-a3/scripts/position-reads.test.mjs:86): Swift interpolation still breaks string scanning. Valid Swift:
   ```swift
   let marker = "\(String("/*"))"
   let p = view.style["position_type"]
   ```
   produces **zero reads**: the inner quote prematurely ends the string, and `/*` consumes the remainder. Raw strings with escaped delimiters also fail because `\#` escapes are ignored. Both failures were reproduced in memory; Swift’s parser accepted the fixtures.

2. **should-fix** — [scripts/position-reads.test.mjs:26](/tmp/iosfix-rv-a3/scripts/position-reads.test.mjs:26), [allowlist:43](/tmp/iosfix-rv-a3/scripts/position-reads.test.mjs:43): The grouped-import pattern detects the import but misses subsequent unqualified reads. Adding `let _ = arena.style(slot).get(PositionType);` after the existing allowlisted import in [cover.rs:348](/tmp/iosfix-rv-a3/kernel/src/kernel/cover.rs:348) leaves **both guard assertions passing**. The current import is legitimately a row list, but its exemption permits new layout reads to escape.

The targeted runtime fixes and combined audit table otherwise look correct by inspection, including preservation of author-sticky and non-popover behavior.

Verification: two repository scan checks passed; the scratch test passed with an in-memory filesystem. Normal scratch execution and Cargo were blocked by read-only filesystem permissions. No files changed.
