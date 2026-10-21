# Code review: the web's rangeFrame caret on an empty line and in an empty field, narrow re-check 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1c673f0e13`.
- **Method:** one brief (sha256 `749dd330073158fa71236153aacd383b4a14029ad02d26ccd1255953cc537625`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX).
- **Disposition:** both taken ("Narrow re-check").

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — [geometry-glue.js:154](host/web/geometry-glue.js:154): whitespace handling remains incomplete.** `preserve-breaks` collapses spaces and tabs, while Blink changes an editable textarea’s `normal` to `pre-wrap` and `nowrap` to `pre`. An indented line that wraps only when its indentation is preserved therefore moves the following empty-line caret by a line. For example, `width:20ch; font-family:monospace; white-space:normal` with `"    const value = foo\n\nx"`. The new cases contain only single interior spaces and miss this difference. Match the editor’s effective whitespace rules. [Blink implementation](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/css/resolver/style_adjuster.cc)

2. **SHOULD-FIX — [geometry-glue.js:150](host/web/geometry-glue.js:150): changing box sizing reinterprets copied maximum sizes.** A content-box field with `width:400px;max-width:400px;padding:10px;border:2px solid` has a 424px border box. The twin receives `width:424px`, but its retained `max-width:400px` now caps the **border box** at 400px. A centered empty caret shifts 12px; wrapping can also change. Clear or convert the copied size constraints, including their logical equivalents. This follows directly from the [CSS sizing rules](https://drafts.csswg.org/css-ui-3/#box-sizing).

The other round-2 changes look sound. The test retains offset 70 beyond the wrap, skips offsets 61/80 after spaces, and scrolls the trailing-newline case to the end.

Source review and an executed offset-selection check only; browser tests were not run because they create build/profile files. Tree unchanged.
