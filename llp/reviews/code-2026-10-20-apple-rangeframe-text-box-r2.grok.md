# Code review: Apple's rangeFrame box is its text's, round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `66a704f13a`.
- **Method:** one brief (sha256 `d8a105183a929072b661d782855513c3f806680591d901e9f336536df77d64c0`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (an Apple host read, theme2's 22:27 ruling). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

I'll review HEAD read-only against the rangeFrame contract, the theme2 ruling, and the round-1 notes. First I'll read the review rules and the commit itself.Round 1 is in. I’ll read the geometry, the tests, and the contract next, then check whether each round-1 fix actually holds.**No BLOCKING or SHOULD-FIX findings.** Round 1 is fixed in the code the tests actually exercise, and nothing new in this commit is a real defect.

Round 1, checked:

1. **Content area.** `area` uses `CSSLineBox.content` (`InlineGeometry.swift:76`). macOS rounds ascent and descent to whole pixels; iOS stays unrounded. That is this host's Chrome content area.
2. **Font runs and `baselineOffset`.** `extent` walks storage runs (`InlineGeometry.swift:96`). A positive offset moves the box up in this y-down space (`baseline - offset`). Decorations never set a rendering font (`FieldDecorations.swift:169`). Emoji staying on the field face is the stated choice.
3. **No segment.** The caret uses the layout fragment's glyph baseline (`InlineGeometry.swift:118`), then the same content box, not the line fragment.
4. **Tests pin that baseline.** `expected` is `glyphOrigin` on the fragment, not the segment callback (`FieldDecorationsTests.swift:70`, `FieldDecorationsIOSTests.swift:81`). Under a 40 px line, a box hung from the line top misses by about 10 px; the tolerance is 0.5. The `> 70` check forces the tall pitch. Both sides call `ensureLayout` in-process, so this is not flaky.

`baselinePosition` is the distance from the segment frame's top. `rect.minY + baseline` (`InlineGeometry.swift:112`) matches the fragment baseline on a wrapped line; a container-absolute value would miss by a line height and fail that assertion. iOS moves the container by `textContainerInset` only; macOS uses `textContainerOrigin` (including `nativeOrigin`). Scroll stays in `Agent.box` (`convert`, then the viewport origin). With no paragraph line height the box is the content area on that baseline, which is the grammar, and `tall == false` checks it. The caret width is forced to 0 at the segment's visual `minX`, so RTL keeps the old horizontal answer. The QUEUE edit removes only the Apple caret item.
