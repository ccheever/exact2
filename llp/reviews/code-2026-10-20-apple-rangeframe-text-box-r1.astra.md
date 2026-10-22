# Code review: Apple's rangeFrame box is its text's, round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `dafb360281`.
- **Method:** one brief (sha256 `4d51fe650878316f97e8e4bca21f8099845cce0f25e98c77635514119088c727`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (an Apple host read, theme2's 22:27 ruling). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX, 1 NIT).
- **Disposition:** taken (the commit message's "Round 1").

**Verdict: SHOULD-FIX. No confirmed BLOCKING finding from this read.**

1. **SHOULD-FIX — [InlineGeometry.swift:83](host/apple/Sources/ExactKit/InlineGeometry.swift:83).** A TextKit segment is a contiguous visual/logical range, not necessarily one font run. Using only its first character’s font can understate a range spanning smaller Markdown inline code and larger heading text on the same line. This also affects normal line height. Storage’s `.font` does not identify the actual emoji fallback face either. Compute the selected runs’ content-area union instead of assigning one font’s metrics to the whole segment. [Apple’s segment definition](https://developer.apple.com/documentation/uikit/nstextlayoutmanager/enumeratetextsegments%28in%3Atype%3Aoptions%3Ausing%3A%29).

2. **SHOULD-FIX — [FieldDecorationsTests.swift:64](host/apple/tests/ExactKitTests/FieldDecorationsTests.swift:64), [FieldDecorationsIOSTests.swift:72](host/apple/tests/ExactKitTests/FieldDecorationsIOSTests.swift:72).** Neither test proves placement at the painted baseline. Returning font-height rectangles at each line’s **top** would satisfy every new assertion: equal heights, equal tops, zero caret width and 40-point pitch. Independently compare the returned top with the layout fragment’s line baseline minus ascent, including a soft-wrapped line. The callback’s baseline origin is not explained in either SDK header or the public method documentation, so this read cannot certify the `rect.minY + baseline` assumption on either platform. The tests use synchronous layout and introduce no obvious timing flake.

3. **NIT — [InlineGeometry.swift:90](host/apple/Sources/ExactKit/InlineGeometry.swift:90), [InlineGeometry.swift:95](host/apple/Sources/ExactKit/InlineGeometry.swift:95).** Both fallback paths retain the native caret rectangle’s height, recreating the original line-box answer whenever enumeration supplies no segment. Neither empty fields nor empty/trailing lines are tested. Establish their path and normalize fallback geometry where needed; reachability was not demonstrated in this read.

The coordinate conversion looks consistent: iOS adds `textContainerInset`, with scrolling handled by `Agent.box`’s view conversion; macOS uses `textContainerOrigin`. RTL positioning remains TextKit’s. Markdown source offsets remain aligned, and decorations here change colours, not fonts. The QUEUE deletion matches the ruling’s requested closure once the geometry is qualified.

Read-only review; no files changed, builds, tests or simulators run.
