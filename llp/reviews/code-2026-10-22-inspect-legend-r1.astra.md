# Code review: inspect mode's banner legend for the dashed outlines, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `b075ff6662`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; SHOULD-FIXes (a rebuilt banner, words for Outline, live-region writes), taken in 452d3c66cc.

1. **SHOULD-FIX — [host/web-js/inspect-dev.js:398](/tmp/rv-dt-legend/host/web-js/inspect-dev.js:398), [host/web/dev.js:737](/tmp/rv-dt-legend/host/web/dev.js:737):** `chrome()` recreates the plain banner but reparents existing dashed marks. After a modal/chrome rebuild, the boxes remain unexplained until another outline frame arrives. Restore the legend from retained marks.

2. **SHOULD-FIX — [host/web-js/inspect-dev.js:442](/tmp/rv-dt-legend/host/web-js/inspect-dev.js:442), [host/web/dev.js:794](/tmp/rv-dt-legend/host/web/dev.js:794):** The tool’s Outline command marks a token’s uses across multiple source locations. “Every copy of this source” incorrectly describes those frames as one source. Distinguish selection outlines or use wording valid for both.

3. **NIT — [host/web-js/inspect-dev.js:439](/tmp/rv-dt-legend/host/web-js/inspect-dev.js:439), [host/web/dev.js:795](/tmp/rv-dt-legend/host/web/dev.js:795):** Each replacement writes the plain status, then the legend—even for an unchanged count. Because [`status` is polite and atomic](https://www.w3.org/TR/wai-aria-1.2/#status), this risks redundant announcements. Update once after replacement and skip unchanged text; actual announcement behavior remains unverified.

Nothing blocks. The first-child text-node invariant holds, and the one-copy wording is accurate. No files changed; no builds or tests run.
