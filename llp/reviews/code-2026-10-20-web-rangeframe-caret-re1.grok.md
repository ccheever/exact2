# Code review: the web's rangeFrame caret on an empty line and in an empty field, narrow re-check 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `1c673f0e13`.
- **Method:** one brief (sha256 `749dd330073158fa71236153aacd383b4a14029ad02d26ccd1255953cc537625`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**RIGHT.** No findings.

Round 2 holds in `caretProbe` and in the Chrome-agreement test.

- Line breaks: `collapse` (`normal`, `nowrap`) is rewritten to `preserve-breaks` after the computed copy (`geometry-glue.js:154`). `text-wrap-mode` stays, so `nowrap` still does not wrap. `pre`, `pre-wrap`, and `pre-line` already keep breaks. Those are the five values on the white-space row.
- Last empty line: a trailing break gets a zero-width space only when the caret is before the end (`:160`), so the scroll height includes the line the textarea's placeholder break keeps. A caret at the end already sits on that line via the mark.
- Motion: `transition: none`, `animation: none`, and `scroll-behavior: auto` are set before insertion (`:147`), so the left/top fit and `scrollTop` apply in the same turn.
- Fixed fields are probed (`:138`). A fixed field gets a fixed twin (`:144`); the border-box fit places it.
- Border box: `box-sizing: border-box` with `offsetWidth` and `offsetHeight` (`:150`) is the field's border box, so a classic scrollbar is laid out once inside that box.

The new cases reach their claims. `white-space: normal` and `nowrap` are sampled at offset 95, on `second`; a collapsed newline moves that caret. `transition: all .2s` is read in the same turn. Smooth scroll is set only after an instant `scrollTop = 60`. `position: fixed` is compared. `trailing` appends a newline, scrolls to `scrollHeight`, and still compares earlier carets. The first line is 93 characters; offset 70 is inside `hundred`, past a 400px wrap, so `white-space: pre` disagrees with a wrapping twin there. Offsets 61 and 80 follow a space and are skipped as wrap-ambiguous.
