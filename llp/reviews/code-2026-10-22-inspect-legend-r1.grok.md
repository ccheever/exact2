# Code review: inspect mode's banner legend for the dashed outlines, round 1 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `b075ff6662`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; SHOULD-FIXes (a rebuilt banner, words for Outline, live-region writes), taken in 452d3c66cc.

I'll review the inspect-banner change at HEAD without modifying anything. First I'm reading the review instructions and the commit itself.The review is read-only and scoped to the inspect-banner change. I'll look at HEAD and the two page scripts plus the test.The commit is three small edits. I'll read the banner rebuild, outline paths, and the test around them.Nothing blocks. The selection path on both pages matches the requested sentence, one copy stays on it (`n > 0` shows `(1)`), and `chrome()` still builds a text node before the close button.

1. **SHOULD-FIX** `host/web/dev.js:737` (`host/web-js/inspect-dev.js:398`). `chrome()` always creates the banner with the plain sentence, then reattaches any dashed marks (`dev.js:759`, `inspect-dev.js:413`) and never calls `legend(marks.length)`. The early return (`dev.js:731`) fails when a modal dialog is open and the banner is not inside it. The next pointer move calls `chrome()` (`dev.js:712`), so the boxes stay up and the banner goes back to "Inspecting: click a view to select it" until a later outline frame. `firstChild` is still a text node after that rebuild, so `legend()` could repair it.

2. **SHOULD-FIX** `host/web/dev.js:795` (`host/web-js/inspect-dev.js:439`). `unmark()` writes the plain sentence into the banner, and `outlineSites` calls `unmark()` before drawing the new boxes (`dev.js:807`, then `legend` at `823`). The banner is `role="status"` (`dev.js:735`), an atomic polite live region, and the close button is inside it (`dev.js:743`), so each update also carries "Stop inspecting (Escape)". A tool Outline frame has no click token, so `scrollIntoView` runs while the status text is still the unmarked sentence (`dev.js:813`). Set the final sentence once, on a text-only status node, and use the plain sentence only when the boxes are actually cleared.

The tool's Outline uses the same `outlineSites` path, so it gets this sentence too, including when `at` is several use-sites rather than copies of one source. The new test only waits for the substring after a selection, so it does not catch the rebuild, the clear path, the count, or that Outline frame.
