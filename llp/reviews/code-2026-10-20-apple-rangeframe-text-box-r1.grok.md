# Code review: Apple's rangeFrame box is its text's, round 1 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `dafb360281`.
- **Method:** one brief (sha256 `4d51fe650878316f97e8e4bca21f8099845cce0f25e98c77635514119088c727`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (an Apple host read, theme2's 22:27 ruling). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (4 SHOULD-FIX, 1 NIT).
- **Disposition:** taken; the emoji NIT stated ("Round 1").

**No blocking findings.** The main path matches theme2's option (a) for one face. `baselinePosition` is the distance from the segment frame's top to the baseline (TextKit 2, y-down, both platforms), so `rect.minY + baseline - ascender` is the content box on that baseline. iOS adds `textContainerInset` and macOS `textContainerOrigin`; scroll is applied in `Agent.box` (the view's `bounds.origin` / the clip view), not by subtracting `contentOffset`. RTL keeps the segment's visual span. A field with no paragraph line height only shrinks from the line fragment (leading included) to the content area, which is the grammar, and the new test checks that height. The QUEUE edit removes the 18:18 caret item and nothing else.

1. **SHOULD-FIX** — macOS uses raw font metrics, not this host's Chrome content area. `InlineGeometry.swift:85` sets the height to `ascender - descender`. `CSSLineBox.content` (`host/apple/Sources/ExactCore/Text.swift:305`) rounds ascent and descent on macOS to match Chrome and is what inline `frame` uses; iOS stays unrounded, so only macOS is off, by up to about a pixel. The top is `baseline - rawAscender`, so the edge moves with the height.

2. **SHOULD-FIX** — one storage font for the whole line fragment, and `baselineOffset` is ignored. `InlineGeometry.swift:82`. This commit's own measurement is that `.standard` segments are line fragments, not font runs, and the face is the storage font at the segment location only. A caret is right: the character after it, else before, else `field.font`. A wider Markdown range on one line is not. Inline code is 0.92em and a footnote mark is 0.75em plus `.baselineOffset` (`MarkupEditor.swift:109`, `MarkupEditor.swift:135`), so the union is the first face only and the raised footnote is not the box. Decorations do not affect this: rendering attributes are only colours (`FieldDecorations.swift:169`).

3. **SHOULD-FIX** — if enumeration yields no segment, the caret is still the line box. `InlineGeometry.swift:89`. macOS `firstRect(forCharacterRange:)` and iOS `caretRect(for:)` are the measured 40 and 41.7. Width is then forced to 0, height is not. An empty line that does get a `.selection` segment is fine (the newline keeps the paragraph font). An empty field is on this path only when TextKit returns no segment; when it does, the main path uses `field.font`.

4. **SHOULD-FIX** — the new tests do not lock "at the baseline". `FieldDecorationsTests.swift:66` and `FieldDecorationsIOSTests.swift:74`. Both platforms check content height, a shared top, caret width 0, and a 40pt step. A content-height box hung from the line top would pass. Linux centres the box in the 40pt line; these do not. The oracle is `ascender - descender`, so finding 1 cannot fail the test. They are not flaky: metrics are fixed, `rangeFrame` calls `ensureLayout`, and the newline makes the second caret a hard line.

5. **NIT** — emoji fallback is not a storage font, so a caret beside an emoji stays the field face (`InlineGeometry.swift:78`). That matches a textarea's own font rather than Apple Color Emoji.
