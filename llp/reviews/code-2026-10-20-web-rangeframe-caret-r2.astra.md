# Code review: the web's rangeFrame caret on an empty line and in an empty field, round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `be8fe39ee6`.
- **Method:** one brief (sha256 `0b2218d8e982d3d33aec4106cc2fa243a2ffb61c30932bad65735e10a5f662dd`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (5 SHOULD-FIX).
- **Disposition:** each taken ("Round 2").

**Verdict: SHOULD-FIX — five concrete defects remain.**

1. **SHOULD-FIX — [host/web/geometry-glue.js:150](host/web/geometry-glue.js:150): Whitespace parity remains incomplete.** With `white-space:normal` or `nowrap`, the twin collapses literal newlines, while Chrome’s textarea preserves them through internal `<br>` elements and editing-style adjustments. For `a\n\nb`, offset 2 therefore measures on the wrong line. Round 1’s `pre`/`wrap=off` cases are addressed, but these modes are absent from the new test. Reproduce the editor’s effective whitespace handling. [Blink implementation](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/html/forms/text_control_element.cc)

2. **SHOULD-FIX — [host/web/geometry-glue.js:150](host/web/geometry-glue.js:150): A trailing newline shortens the twin’s scroll range.** When measuring an earlier empty line, the suffix ends in a bare newline; Chrome additionally creates a placeholder break to retain the final empty line. Consequently, the twin’s `scrollTop` can clamp below the field’s. Example: `a\n\nb\n`, three visible lines, scrolled to the bottom, measuring offset 2. The result shifts by one line. Preserve the final line independently of the probe. The tests exercise scrolling and trailing newlines separately. [Chrome’s placeholder handling](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/html/forms/text_control_element.cc)

3. **SHOULD-FIX — [host/web/geometry-glue.js:158](host/web/geometry-glue.js:158): Copied transitions break calibration.** A field with `transition:all .2s` gives the twin that transition. Its calibration writes to `left`/`top` then animate instead of moving immediately, producing zero/intermediate displacement and an unavailable or incorrect result. Disable transitions before insertion and calibration; copied animations also need suppression. The same file already disables transitions for `measure`. [Transition rules](https://drafts.csswg.org/css-transitions-1/#starting)

4. **SHOULD-FIX — [host/web/geometry-glue.js:168](host/web/geometry-glue.js:168): Smooth scrolling prevents synchronous restoration.** Copied `scroll-behavior:smooth` makes these assignments animate. The marker is measured and removed before reaching the field’s scroll position. Use an explicit instant scroll; the existing scroll test uses default behavior. [CSSOM scrolling rules](https://drafts.csswg.org/cssom-view/#dom-element-scrolltop)

5. **SHOULD-FIX — [host/web/geometry-glue.js:138](host/web/geometry-glue.js:138): Fixed-position textareas are rejected outright.** Their populated-line ranges still work, but empty-line and empty-field ranges become `unavailable`. Fixed positioning is ordinary supported CSS, not a grammar refusal. Handle its positioning context instead.

The other round-1 changes address typography, vertical dimensions, static transforms/zoom, and bare div/span styling. The strengthened assertions catch the original wrong carets; Linux pins the fixture’s facts. No timing flake is apparent. Insertion still queues observers and forces layouts. Grammar/LLP preserve the ruling, but QUEUE closure is premature.

Static review only; no browser or tests run. Tree unchanged.
