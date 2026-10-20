# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 4, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and rounds 1-3.
- **Method:** one brief (sha256 `de602afe4019d98450689613bcaf7877440bb84fadd7023e12abefa543b69c7d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the same two).
- **Disposition:** fixed in round 4's commit.

---

The release re-ink is wrong when the lending press also authors its text colour. That is blocking. The other round-3 checks hold.

1. **Blocking** — `host/apple/Sources/ExactKit/PressPaint.swift:269`
   - **Scenario:** A parent authors `color` (the style carries `written: ["text_color"]`, which `wrote` reads) and is down, or release-held, with `press_color`. A child press releases. `liveLender` hits `wrote("text_color")` and returns nil before it considers that press, so `inkAgain` never runs. The child snaps to its engine colour instead of the parent's press colour. The same thing happens for a background-only press between them: that node is not a text boundary, but the walk still dies on the parent's `written` flag. Initial paint is fine (`inkBelow` never asks `liveLender`). The nested-release tests never set `written` on the lender, so they miss it. Linux checks the press colour before stopping at the colour source.
   - **Fix:** Return a down or release-held ancestor that has `press_color` first; only then stop on `wrote("text_color")`.

2. **Should-fix** — `host/apple/Sources/ExactKit/PressPaint.swift:412`
   - **Scenario:** An ancestor press is down. A child is pressed with only a background colour, so it is showing the ancestor's text colour and the grandchild labels are on the ancestor's `inked` list (`inkedBy` is the ancestor). A style write sets `written: ["text_color"]` on that child. The down branch drops the lent colour on the child (line 410) but only rewinds its own `inked` list, which is just itself. The not-down branch (line 421) calls `endInk` for this. The labels stay on the ancestor's colour for the rest of that press.
   - **Fix:** When `writes(s, "text_color")` while down, call `endInk(below:by:)` for `inkedBy` and clear `inkedBy`, as the not-down branch does.

Round 3's other pieces check out. `inkBelow`'s third clause no longer requires `!down`, and `showsOwnTextColor` still stops a press that has its own `press_color` (line 217). `pressPainted` keeps a lent text colour on a pressed node whose own press writes none (lines 407–410). `inkAgain` runs only from a real release (`endInked(_:released: true)` at line 196), not from a style rework (line 419) or `dropPressPaint` (line 456). `inked` appends a view only when it is not already there (line 219).

Removing a node's own `press_color` while it is down still does not re-ink from a lender. It does not have to be fixed in this round: that path is a style rework, and this change deliberately does not call `inkAgain` there. The release path will re-ink once finding 1 is fixed. A recycled view also does not re-ink from a grandparent (`dropPressPaint` clears the bookkeeping and the next style is the engine style). That matches a view created under a press that is already down, and it is the same "no re-ink on drop" rule.
