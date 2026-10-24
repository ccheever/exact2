# Code review: inspect mode's banner legend for the dashed outlines, round 2 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `452d3c66cc`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; Astra one NIT (a view gone), taken in 4ad49dd0d6; Grok none.

I'll review HEAD's answers to the round-1 legend findings in both inspector pages, read-only. First I'll read the review instructions and the commit that answers them.The review stays read-only and scoped to HEAD’s answers to the round-1 legend notes. I’ll look at that commit and both inspector pages.HEAD only touches the two inspector pages and one test. I'll read the legend, chrome rebuild, and outline paths around those edits.Nothing blocks. No findings.

HEAD `452d3c66cc` answers all three round-1 notes on both pages.

1. A chrome rebuild with boxes still up calls `legend(marks.length)` after the marks are reattached (`host/web-js/inspect-dev.js:417`, `host/web/dev.js:761`). The new banner's first child is the text node (`textContent`, then the close button), so that write lands. An early return still means the banner never left, so its text is left as it was.

2. Both legends use `Inspecting: ${n} outlined in dashes (the selection's every copy, or what the tool outlined); click a view to select it` (`host/web-js/inspect-dev.js:446`, `host/web/dev.js:798`). The page test waits on that phrase for both the wasm page and the other one (`scripts/tamber-inspect-pages.test.mjs:64`).

3. `legend` writes only when the text node's words differ (`host/web-js/inspect-dev.js:447`, `host/web/dev.js:799`). `outlineSites` clears with `unmark(true)` (`host/web-js/inspect-dev.js:459`, `host/web/dev.js:813`) and then calls `legend(found.length)` once, so a same-count outline does not speak and a different count speaks once. A real clear still uses `unmark()` and says the plain sentence. The plan, off, and non-array guards still return before that quiet clear.
