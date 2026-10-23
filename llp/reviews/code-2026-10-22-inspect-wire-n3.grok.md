# Code review: tamber/1.16 inherited rows' fromAt and inspect select, a third narrow check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a third narrow check; HEAD `eb6c933479`. The author (Claude) is not a reviewer.
- **Verdict:** closed; nothing blocks.

I'll verify that one refusal-tracking fix against `eb6c933479` and only report whether the old drop still happens or a new one showed up.The diff is two files. I'll read how the per-build count is snapshotted and who still reads the old sum.Closed. Nothing blocks.

A refusal, a refusal-clear, or the removal of any other build no longer changes the watched value. `refusalCounts` is one `refusalSeq` per build (`refusals.ts:30`), and `refusalsKey` is only the row for `inspected.build` (`app.contract:959`). `selectUp` still copies that into `upRefusalsAt` before the wait (`app.contract:1230`), so a later short landing still reaches `landUp`.
