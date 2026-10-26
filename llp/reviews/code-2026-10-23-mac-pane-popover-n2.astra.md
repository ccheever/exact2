# Code review: the macOS split-pane popover, narrow re-check 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `067907ce0b`.
- **Method:** one brief (sha256 `9a0f01f79c131adbfd4c8dbbbf07ebaa3be04579f6516e925ef2b88494d1dbc9`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in a9ab83f7b2

**WRONG — reparenting can reverse popover stacking order.**

At [MenusMac.swift:548](/tmp/iosfix-rv-pane4/host/apple/Sources/ExactKit/Mac/MenusMac.swift:548), `host.addSubview(entry.layer)` appends a migrating layer above layers already there.

Concrete case:

1. Open painted popover **A** from the detail pane.
2. Open painted **manual** popover **B** from the list, declared outside the list route so it survives collapse.
3. Collapse the split, as the new test does.

A moves into the viewport after B; B stays where it is. Where they overlap, older A now covers newer B. `SheetsMac.openTopLayer` also reports this reversed order through its viewport sibling indices. Reconciliation must preserve the existing presentation order.

An in-memory AppKit probe confirmed the operation produces `[B, A]` and overlapping hit-testing selects A. The new [collapse assertions](/tmp/iosfix-rv-pane4/host/apple/tests/ExactKitTests/SplitMacTests.swift:878) check only openness and ancestry, so they miss this.

The assertions also **do not independently fail without every change**: reverting only `paint`’s owner selection is masked by the immediate `layout()` reconciliation. Escape directly exercises the expanded focus guard; collapse exercises eventual migration but does not isolate the new `SplitHost.present` call from subsequent layouts.

I found no additional blocker in the ordinary single-popover paths for light dismissal, frame conversion, accessibility, capture, restoration, presentation switching, or route retirement.

No files changed. XCTest and mutation tests were not run because the fixture writes files.
