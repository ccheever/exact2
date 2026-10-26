# Code review: Apple's rangeFrame box is its text's, round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `66a704f13a`.
- **Method:** one brief (sha256 `d8a105183a929072b661d782855513c3f806680591d901e9f336536df77d64c0`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (an Apple host read, theme2's 22:27 ruling). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**Verdict: No confirmed BLOCKING or SHOULD-FIX findings.**

Round 1 fixes checked:

1. [InlineGeometry.swift:76](host/apple/Sources/ExactKit/InlineGeometry.swift:76): `CSSLineBox.content` supplies the host’s rounded metrics. The extent calculation now unions every storage attribute run and applies `baselineOffset`, addressing mixed Markdown fonts and raised footnotes. Markdown’s substitution preserves source offsets; decorations override only colours. Emoji fallback retains the stated storage-font limitation.

2. [FieldDecorationsTests.swift:64](host/apple/tests/ExactKitTests/FieldDecorationsTests.swift:64) and [FieldDecorationsIOSTests.swift:75](host/apple/tests/ExactKitTests/FieldDecorationsIOSTests.swift:75): the tests now independently derive the baseline from layout and line fragments. They would reject content-height boxes placed at line tops, including on wrapped text. This checks the `rect.minY + baseline` interpretation on both platforms; [Apple’s method documentation](https://developer.apple.com/documentation/uikit/nstextlayoutmanager/enumeratetextsegments%28in%3Atype%3Aoptions%3Ausing%3A%29) does not explicitly define that callback coordinate. Saved logs record both revised tests passing. No obvious timing flake appears.

3. [InlineGeometry.swift:118](host/apple/Sources/ExactKit/InlineGeometry.swift:118): the fallback now constructs a font-content-height box at the line baseline instead of retaining native caret height. Entirely empty fields without a usable layout fragment remain outside the added tests’ coverage; I could not establish a concrete failure read-only.

The iOS inset plus view conversion accounts for scrolling; macOS retains `textContainerOrigin`. Horizontal and RTL placement remains TextKit-derived. No normal-line-height regression was established. The QUEUE deletion matches the ruling’s requested closure.

No files changed, builds, tests, or simulators run.
