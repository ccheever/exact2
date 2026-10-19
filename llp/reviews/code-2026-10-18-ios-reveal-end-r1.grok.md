# Code review: iOS scrollIntoView past the range's end under a collapsing large title (the reveal test's flake), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `5ea27cf46`.
- **Method:** one brief (sha256 `afe870206a83c8521df0d311ee26cbdcbd8a0757712cb60cd03c8d999d5e271f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings.
- **Disposition:** nothing to take.

No findings.

Verdict: The second alignment is the right CSSOM clamp (content-space `area` is still valid after the inset change, so the new port and range are what get applied), and it does not disturb outer, non-grouped, page, keyboard, held-request, or Mac scrolling; the rewritten tests remove the timer race and lock both the in-range row and the end clamp.
