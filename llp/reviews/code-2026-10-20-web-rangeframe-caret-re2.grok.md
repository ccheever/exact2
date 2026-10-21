# Code review: the web's rangeFrame caret on an empty line and in an empty field, narrow re-check 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `e9dcbde366`.
- **Method:** one brief (sha256 `cb8b03f07839f76692e6deb68af64d13f757c090e4ab0bd913c9c553e50da95a`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 NIT).
- **Disposition:** taken: break-spaces left as it is.

**RIGHT WITH CHANGES**

(a) holds. After the computed copy, `whiteSpaceCollapse` is set to `preserve` (`geometry-glue.js:156`) and `text-wrap-mode` is left as copied. That is Blink's `AdjustStyleForEditing`: `normal` becomes `pre-wrap`, `nowrap` becomes `pre`, and `pre-line` becomes `pre-wrap`. Spaces, tabs, and breaks stay.

(b) holds. Physical and logical min and max sizes are then set to `0` and `none` (`:151–152`). A content-box field with `width:400px` and `max-width:400px`, plus `padding:10px` and `border:2px`, has a 424px border box. The twin's width is that `offsetWidth`, and the copied max no longer caps it.

The max-width case reaches that claim. Its style is applied, and offset 95 is on the short centred line `second`, which moves about 12px if the border box is capped at 400.

1. **SHOULD-FIX** — `host/web/tests/decorations.test.mjs:218`. `white-space:normal;--indent` hits a new `if` that owns the following `else`, so the style attribute is never assigned. The field stays on the textarea UA `pre-wrap`, which already keeps the inserted spaces, and the case stays green if the `preserve` override is removed. Offsets 40, 70, and 95 sit on that indented first line and would move under `white-space: normal`. Assign the style and the value together.

2. **NIT** — `geometry-glue.js:156`. Blink leaves `break-spaces` unchanged. Forcing `preserve` removes wrap opportunities inside a run of spaces, so a later caret can move by a line. Rare on a textarea.
