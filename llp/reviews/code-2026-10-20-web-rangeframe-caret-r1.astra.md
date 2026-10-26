# Code review: the web's rangeFrame caret on an empty line and in an empty field, round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `5d150c6af0`.
- **Method:** one brief (sha256 `49e4e8d9d7d8468d14f448b0637fe7b153a23d2e338a40df30f2504b764d3f98`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 4 SHOULD-FIX, 1 NIT).
- **Disposition:** taken: the copy became a twin of the field (the commit message's "Round 1").

**Verdict: BLOCKING — the mirror can place the caret incorrectly under ordinary field styling.**

1. **BLOCKING — [host/web/geometry-glue.js:139](host/web/geometry-glue.js:139): Mixed coordinate systems.** `getBoundingClientRect()` includes transforms, while client dimensions, borders and scroll offsets are unscaled. A field with `transform:scale(2)` gets an unscaled caret height, padding and line displacement; rotation also invalidates using the bounding box’s corner as the origin. CSS `zoom` has the same missing conversion. Transform the locally measured caret into viewport coordinates. [CSSOM View](https://drafts.csswg.org/cssom-view/#dom-element-clientwidth)

2. **BLOCKING — [host/web/geometry-glue.js:138](host/web/geometry-glue.js:138): Wrapping is forced.** A textarea using `white-space:pre`, `nowrap`, or `wrap=off` can contain a long unwrapped paragraph followed by an empty line. The mirror wraps that paragraph, moving the reported caret down several lines. Reproduce the textarea’s effective whitespace behavior, including its editing adjustments. [Blink’s rules](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/css/resolver/style_adjuster.cc)

3. **SHOULD-FIX — [host/web/geometry-glue.js:134](host/web/geometry-glue.js:134): Incomplete typography.** Missing `font-variation-settings`, `font-feature-settings`, and `hyphens`/language can change preceding text’s wrapping. Missing `text-align-last` directly changes an empty line’s alignment. Copying the listed font properties and `text-align` does not cover these.

4. **SHOULD-FIX — [host/web/geometry-glue.js:139](host/web/geometry-glue.js:139): Vertical writing uses the wrong constraint.** With vertical writing, the field’s height constrains line length. The mirror instead uses `height:auto`, so wrapping and start/end alignment use a different box. Preserve the appropriate inline dimension.

5. **SHOULD-FIX — [host/web/geometry-glue.js:142](host/web/geometry-glue.js:142): The probe is exposed to page CSS.** For example, `span { font-size:24px }` overrides the marker’s inherited field font. Global div rules, pseudo-elements and min/max dimensions can also alter measurement. Isolate and reset both elements.

6. **SHOULD-FIX — [decorations.contract:44](host/web-js/conformance/decorations.contract:44), [decorations.test.mjs:181](host/web/tests/decorations.test.mjs:181): Assertions leave incorrect answers passing.** The fixture accepts an empty-field caret anywhere vertically inside the field, with any positive fitting height; it lacks even a right-edge bound. Check the actual start and matching text height. The trailing-newline test checks neither x nor zero width nor exactly one line’s displacement. Existing assertions appear deterministic.

7. **NIT — [host/web/geometry-glue.js:145](host/web/geometry-glue.js:145): Observable measurement overhead.** Append/read/remove forces fresh layout per probe and queues document-wide MutationObservers, including `intersect-glue.js:79`. Removal does not cancel those records.

CR/CRLF normalize to LF, and trailing LF is recognized; no U+2028 defect established. [HTML](https://html.spec.whatwg.org/multipage/form-elements.html#the-textarea-element) Basic horizontal padding, borders, scrollbar subtraction, scrolling, box-sizing and content sizing look consistent. Grammar/LLP preserve the ruling, but QUEUE closure is premature given the failures above.

Static review only; no browser or tests run. Tree unchanged.
