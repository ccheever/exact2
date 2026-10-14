# [Bug] macOS: ⌘A highlights and ⌘C copies visibility:hidden text

**Status:** Closed
**Resolution:** fixed by #525
**Systems:** host/apple macOS, text selection, runner
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-13
**Related:** LLP 1115 D8; LLP 1033; X81 (TextSelectionMacTests); T3 Code clone STATUS F12-2, KD-select-all-hidden

## Summary

On macOS, ⌘A (and Edit › Select All) highlights text that computes `visibility: hidden`, and ⌘C copies it. The
T3 Code clone keeps a pull request's Summary and Timeline tabs both mounted, the inactive one under
`visibility: hidden`, as the reference app does: ⌘A on either tab paints selection boxes where the hidden tab's
texts lie, and ⌘C copies the hidden tab's texts with the shown ones (T3 `realinput-1011c` F12-2,
KD-select-all-hidden). Chrome selects and copies only the shown tab, and so does AppKit's own text system, which
neither shows nor selects hidden text.

`TextSelection` (`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift`) collects the paragraphs a selection
covers with `texts(in:)`, which leaves out `isHidden` views (`display: none`) but not `visibility: hidden`
ones: CSS `visibility` keeps the view and only stops its own painting (`CssVisibility.swift`). Which paragraphs
take part is `selectable(_:)`, that is `textSelectable` (`user-select` and the Mac's `auto`), with no visibility
or `inert` test. `selectAll()` filters `inert` itself, but a drag, or an `inert` paragraph between ⌘A's two ends,
is still highlighted and copied. A virtualized list's copy is the runner's (`runner/src/instance/text.rs`,
`list_text`, X81): it leaves out `display: none` and `user-select: none` text, not `visibility: hidden` or
`inert` text, in mounted and unmounted rows alike.

## Reproduction

Two scratch apps made with `bun scripts/exact.mjs new <dir>` (`app.ts` exports no sources), built with
`bun exact.mjs mac` at `origin/main` `beaee003d` and driven with `bun exact.mjs agent macos --size 520x420`:

```text
component Hiddentabs
  state tab = "summary"
  action show(name: string)
    tab = name
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" gap=12
      row role="tablist" aria-label="Pull request" testId="tabs" gap=8
        button role="tab" aria-selected=(tab == "summary") press=show("summary") testId="tab-summary"
          text "Summary"
        button role="tab" aria-selected=(tab == "timeline") press=show("timeline") testId="tab-timeline"
          text "Timeline"
      view position="relative" flex=1 min-height=0
        column testId="summary" position="absolute" inset=0 gap=10 visibility=(tab == "summary" ? "visible" : "hidden")
          text "Reviewers: Ada and Lin" testId="summary-reviewers"
          text "Moves the parser onto the new grammar and drops the old tokenizer." testId="summary-description"
          text "Lin: looks good once the fixtures are regenerated." testId="summary-comment"
        column testId="timeline" position="absolute" inset=0 gap=10 visibility=(tab == "timeline" ? "visible" : "hidden")
          text "Ada opened this pull request" testId="timeline-opened"
          text "Lin approved these changes" testId="timeline-approved"
          text "Ada merged commit 1a2b3c4 into main" testId="timeline-merged"
          text "CI passed on 14 checks" testId="timeline-ci"
          text "Ada deleted the branch" testId="timeline-deleted"
```

```text
component Hiddenlist
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" gap=8
      list virtualized=true flex=1 min-height=0 estimated-item-height=32 testId="commits"
        each n in [1, 2, 3, …, 30] key=n
          row height=32 align-items="center" gap=12
            text `Commit ${n}: tidy the parser` testId=`commit-${n}`
            text `Hover card ${n}` visibility="hidden" testId=`card-${n}`
```

Drive: `"type summary-reviewers key Meta+a" "type summary-reviewers key Meta+c" "screenshot <png> window"`, then
`pbpaste` (and the same after `"tap tab-timeline"` on `timeline-opened`; on `commit-1` in the list app). On main
after LLP 1116.015 D1 the list repeats with `for`, not `each`.

| Scenario | Platform | Revision | Actual | Expected (Chrome 155, ⌘A ⌘C on the web build) |
|---|---|---|---|---|
| Summary shown, ⌘A ⌘C | macOS 26.6.2 (25G83), Apple Silicon | `beaee003d` | the hidden Timeline's five texts are highlighted over the Summary (their fill hides its glyphs) and copied after it: 8 paragraphs | the Summary's 3 paragraphs |
| Timeline shown, ⌘A ⌘C | macOS | `beaee003d` | boxes where the hidden Summary's texts lie; the copy holds all 8 paragraphs | the Timeline's 5 paragraphs |
| Virtualized list, ⌘A ⌘C | macOS | `beaee003d` | each mounted row's hidden card is a highlighted box; the copy holds all 30 `Hover card n` (18 rows unmounted) | the commits only, no card. Chrome copies the 25 rows in its DOM; the runner's copy on macOS covers all 30 (X81) |

Chrome on the same cases as a static page (`visibility: visible` inside a hidden box, an `inert` box, a
`display: none` box, a hidden paragraph): it copies the shown text and the visible descendant only.

## Current scope

Skip text that computes `visibility: hidden`, or is `inert` or under `display: none`, in `TextSelection`'s
paragraph set (⌘A, a drag, the highlight, `selectedText`) and in the runner's `list_text` (the visibility a row's
text takes from above, mounted or not). Inline runs keep their paragraph's part: a visible run inside a hidden
paragraph, and a hidden run inside a visible one, are left for later (Chrome copies the first and omits the
second).
