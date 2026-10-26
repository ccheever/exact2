# Code review: the web's rangeFrame caret on an empty line and in an empty field, round 1 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `5d150c6af0`.
- **Method:** one brief (sha256 `49e4e8d9d7d8468d14f448b0637fe7b153a23d2e338a40df30f2504b764d3f98`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX, 2 NIT).
- **Disposition:** taken ("Round 1").

**SHOULD-FIX.** No blocking defect. On an ordinary field — `pre-wrap`, no zoom or transform, LF breaks — the probe puts an empty line’s caret on that line and an empty field’s at the content start, at the text’s height.

1. **SHOULD-FIX** — Authored `white-space` is ignored. `host/web/geometry-glue.js:138` always sets `pre-wrap`. The host applies `white-space` as a real row (`host/web/glue.js:1008`). `pre`, `nowrap`, and `break-spaces` change where a long line wraps, so the empty line under it is not the painted caret. Copied, and correct for the usual field: font longhands, `line-height`, letter- and word-spacing, `text-indent`, `text-align` (including `start`/`end` with `direction`), padding, `tab-size`, `overflow-wrap`, `word-break`. Borders are `clientLeft`/`clientTop`. A vertical scrollbar is `clientWidth`. Scroll is subtracted from the unscrolled mirror. `box-sizing` and `field-sizing: content` are absorbed by that used `clientWidth`.

2. **SHOULD-FIX** — Zoom and transform are not the painted caret. `host/web/geometry-glue.js:137` puts an untransformed `position: fixed` div on `body`, at the field’s axis-aligned origin, width `clientWidth`. `createValueRange` follows a scale, rotate, or CSS `zoom`, so an empty line disagrees with the carets on lines that have text. A pure translate still lands: the origin is the border box’s viewport rect.

3. **NIT** — `emptyLine` is only U+000A. `host/web/geometry-glue.js:125`. `textarea.value` already turns CR and CRLF into LF, so those are right, and a trailing LF is too: the offset is empty and the zero-width space sits after the break. U+2028, U+2029, U+000B, U+000C, and U+0085 are forced breaks in the field (CSS Text, BK/NL) and stay in the value, so those empty lines still get Chrome’s preceding-text rect. Vertical `writing-mode` is copied (`:135`) but `height: auto` (`:139`) does not constrain the inline size, so a vertical field wraps on the wrong axis. `horizontal-tb` uses width and matches.

4. **NIT** — The empty-field fact does not pin the caret. `host/web-js/conformance/decorations.contract:44` is only “inside the border box”, width 0, height > 0, so a padding miss that stays in the 40px box still reads `inside true` on every host. The empty-line facts do catch a wrong caret: rounded x against the carets on `a` and `b`, strictly between them with equal rounded gaps, and the same rounded height. That fails Chrome’s old “end of the previous line” and a probe that misses padding. Linux’s unit test pins the all-true string. Apple forces width 0 and reads the painted caret, so the same booleans hold; it is not in this drive. Android and the terminal stay `"unavailable"`, the reader-less answer.

The web test (`host/web/tests/decorations.test.mjs:172`) checks that claim against `createValueRange` on `a` and `b`, and the empty field against the content origin and a typed caret. No timers or font loads. Not flaky. `toBeCloseTo(..., 1)` is ±0.05px and the height compare is exact, so a stable subpixel split fails closed.

Appending the mirror does not change the rect: it is removed before return, and observers run as microtasks. `intersect-glue.js:79` only rechecks existing targets. Shipped CSS sits under `#exact-root` or classes; the mirror is on `body` and does not match. The mark span has no inline reset, so a global `span` rule would move it.

Grammar, LLP D7, and the QUEUE drop match the ruling. The grammar’s “as” (`docs/contract-grammar.md:1171`) means Chrome’s answers are why the web measures; LLP says that with “so”.
