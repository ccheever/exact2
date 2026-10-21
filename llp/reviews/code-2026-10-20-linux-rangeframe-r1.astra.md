# Code review: the Linux rangeFrame reader (LLP 1114.007 D7), round 1 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `215a31c5ed`.
- **Method:** one brief (sha256 `fed51536b77c289f1f51a66724879788845f7ecdf7bcb77bbb886b1e488022e4`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a host's geometry read, theme2's 17:27 ruling), then a narrow check of the last fix. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (4 BLOCKING, 3 SHOULD-FIX).
- **Disposition:** each taken in the next amend (the commit message's "Round 1"); the disputed points were settled by measuring Chrome's createValueRange.

**Verdict: BLOCKING.**

1. **BLOCKING — stale geometry crosses reloads.** [host/linux/src/presenter.rs:1299](host/linux/src/presenter.rs:1299) installs the existing reader without retiring its fields. New runners reuse view IDs, so an action before the replacement’s first acknowledged frame can read the previous session’s text and position. Clear or session-qualify the publication, including subsequent restoration of old display pictures. The reload test merely calls `replaced()` on the same runner and repaints, missing this case.

2. **BLOCKING — hard breaks inflate ordinary ranges.** [host/linux/src/text/field.rs:286](host/linux/src/text/field.rs:286) adds the following line’s zero-width box. For `"ab\ncd"`, range `0..3` consequently covers two lines although no second-line text is selected. Chromium skips empty rectangles when computing bounds; its result covers the first line. The newline-only test enshrines the same incorrect union. The plan’s break rule also needs correcting. [Chromium implementation](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/dom/range.cc)

3. **BLOCKING — CSS line boxes replace text bounds.** [host/linux/src/text/field.rs:280](host/linux/src/text/field.rs:280) gives every selected span the entire CSS line height. With `font-size=16` and `line-height=40px`, an ordinary word measures 40 pixels high, starting at the line top. DOM range bounds use font ascent/descent within that line, so both vertical position and height differ. Keep the explicitly defined collapsed-caret behavior separate. [CSSOM range geometry](https://drafts.csswg.org/cssom-view/#dom-range-getclientrects)

4. **BLOCKING — collapsed wrap boundaries choose the wrong line.** [host/linux/src/text/field.rs:267](host/linux/src/text/field.rs:267) forces downstream affinity. At an offset shared by adjacent wrapped lines, this returns the next line’s start. Chromium collects both collapsed fragments and returns the first, on the preceding line. A popover can therefore move down one line at an ordinary wrap boundary. [Fragment collection](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/layout/layout_text.cc), [bounding rule](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/dom/range.cc)

5. **SHOULD-FIX — the reader admits excluded targets.** [host/linux/src/presenter/range_frame.rs:61](host/linux/src/presenter/range_frame.rs:61) publishes every painted field. Single-line inputs, including passwords, now answer despite D7 explicitly requiring unavailable. Android Markdown textareas also answer: `fields::hosts()` excludes Markdown, so those fields are painted here. That contradicts the approved Android scope and amended host list.

6. **SHOULD-FIX — transformation happens after union.** [host/linux/src/paint/field.rs:131](host/linux/src/paint/field.rs:131) transforms the local bounding rectangle. For unequal multiline fragments under rotation or skew, its empty corners enlarge the result. DOM transforms the individual fragments before bounding their union. The scale-only test cannot distinguish these operations. [CSSOM fragment rules](https://drafts.csswg.org/cssom-view/#dom-range-getclientrects)

7. **SHOULD-FIX — conformance loses its numeric comparison.** [host/web-js/conformance/decorations.contract:29](host/web-js/conformance/decorations.contract:29) allows materially wrong widths or jointly shifted caret/mention positions to produce the same string. Contrary to the comment and approval premise, the web tests check thresholds such as `width > 50`, not exact dimensions. Preserve numeric wasm/JS comparison or add independent geometry assertions alongside the font-independent Linux checks.

Read-only source review; no tests or browsers run. Chrome comparisons are inferred from the cited implementation and specification.
