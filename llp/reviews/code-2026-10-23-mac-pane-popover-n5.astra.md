# Code review: the macOS split-pane popover, narrow re-check 5, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `58652aca8a`.
- **Method:** one brief (sha256 `abd9b403bcbaf9930dbab61f5219f02374f0517e12e288ed0749d972d8b8f467`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 6bac86f58e

**WRONG.**

[P2] [MenusMac.swift:553](/tmp/iosfix-rv-pane7/host/apple/Sources/ExactKit/Mac/MenusMac.swift:553) excludes source-less popovers from the owner’s ordered layers. `layerHost(entry, in: entry.source?.window)` returns `nil` for a supported `showPopover(id)` invocation without a source.

Concrete failure:

1. Open painted manual popover **A** from the detail pane.
2. Open manual popover **B** using `showPopover("B")` without a source; its layer attaches to the viewport.
3. Collapse the split.

The new grouping includes A but omits B, then appends A after B. Both receive the same foreground rank, so older A paints above—and receives overlapping clicks ahead of—newer B. An isolated, in-memory AppKit reproduction confirmed this reversal; the previous insertion rule preserved B above A. Source-less entries need an owner included in the ordering pass.

**The tests do not distinguish every fix.** The new ordering scenarios supply sources for both popovers, missing this case. Also, [SplitMacTests.swift:899](/tmp/iosfix-rv-pane7/host/apple/tests/ExactKitTests/SplitMacTests.swift:899) does not independently guard the re-ranking fix: commit `339fdffe37` explicitly records that it passes without `setPaintForeground` because another page re-rank masks the omission.

No files changed. I did not run repository XCTests, whose fixtures write files; verification was source tracing and the isolated AppKit reproduction.
