# Code review: a re-held import keeps its role overrides (third pass #6), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `6a1e9f53fc`.
- **Method:** one brief (sha256 `3dacf6ea6fd9bf6ecd475738a81d702421b82d143a0b9752e8fc1135e226ba31`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `apps/design-tool/shapes.contract:178` — `kept` is documented as offers beginning on for “a recomputed import, or an edit of one.” The card now stores `startsChosen` there, and an edit-held proposal (`editedBy` set, `review.from` equal to `replaces`) begins with overrides off. The heading and the offer checkboxes follow that flag; the comment still describes the rule this change removed.
