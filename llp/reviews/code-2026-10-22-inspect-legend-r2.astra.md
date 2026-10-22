# Code review: inspect mode's banner legend for the dashed outlines, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `452d3c66cc`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; Astra one NIT (a view gone), taken in 4ad49dd0d6; Grok none.

1. **NIT** — [host/web-js/inspect-dev.js:452](/tmp/rv-dt-legend/host/web-js/inspect-dev.js:452), [host/web/dev.js:805](/tmp/rv-dt-legend/host/web/dev.js:805): `place()` hides marks for disconnected elements without updating the legend. If outlined views unmount, the banner can still say “N outlined” after their dashes disappear. Update the count when marks become disconnected.

Nothing blocks. All three round-1 fixes are implemented correctly on both pages. No changes, builds, or tests performed.
