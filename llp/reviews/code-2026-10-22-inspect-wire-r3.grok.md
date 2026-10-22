# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 3 of 3 (Astra run twice: the first stopped at model capacity) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 3 of 3 (Astra run twice: the first stopped at model capacity); HEAD `c700c1d7a2`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; SHOULD-FIXes and NITs taken post-round in fc0ec2c006 and 618908d5a1.

I'll review the wire round against the round-2 answers and the full change since round 1, read-only. First I'll load the review instructions and the diffs.The review is read-only and scoped to the wire round: HEAD’s answer to round 2, plus the full change since round 1. I’ll pull those diffs and the tamber/1.16 notes next.HEAD’s frame and landing changes are in. I’ll read the select path, `fromAt`, and both pages against the round-2 claims.Nothing blocks.

1. **SHOULD-FIX** `apps/design-tool/app.contract:948` — A page that refuses the move still leaves the "landed higher" expectation set. `selectRequest` returns HTTP 200 once the frame is sent. The page then answers `unsupported` on its own when the plan, site, or (on wasm) generation does not match, when no level has a node above the selection, and on Apple, which always refuses `select`. `upRefused` runs only when `selectedUp` is not ok, so `upExpect` stays set, and `selection-up-error` (`app.contract:1847`) only renders that HTTP error. The next selection is treated as the move's landing: if its chain is shorter, `landUp` shows "The page's tree had fewer above it" for a later click. The page's words are a separate `unsupported` event on the build and do not clear `upExpect`.

2. **NIT** `scripts/tamber-builds.mjs:778` — No level at all (a selection whose enclosing copies own no node) is a 409, as round 2 asked, but the text is the no-selection message: "has no selection with a source map to move up from: select a node first". That string is what the tool shows under the selection.

3. **NIT** `host/web/dev.js:902` — The comments on both pages' `selectUp` still describe `{select: {up, sites}}` and one level (`host/web-js/inspect-dev.js:297` is the same). The code walks `levels` and starts above the selection. The 1.16 summary at `docs/tamber-protocol.md:151` still says the page selects the node `n` call sites up; the frame bullet at line 1425 matches the code.

Round 2 holds on the path the tests cover. The frame is `{select: {up, levels, plan, from}}`; both pages refuse unless plan and site match, and wasm also checks generation. `levelsUp` lists every owning level, nearest first, and the page takes the first with a node above the selection, outermost in that level. An HTTP refusal shows `selection-up-error`, and a move the server accepts keys the note to the landing selection. `fromAt` is joined from the page's `fromSite`, and `fromSite` is removed before the selection is published.
