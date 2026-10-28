# Code review: Excord warm address-following, round 6, the narrow check of its fix, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6d8550eff6`.
- **Method:** a delta brief (not kept), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review only the HEAD fix against `c72dff7b95` for that encoded-address case, in a throwaway worktree, and report whether the canonical-visit change is correct and complete.The diff is small. Next I’ll read how `go`, `replace`, and stack URLs actually behave so the canonical-visit claim can be checked against the encoded-address case.No BLOCKER or MAJOR. The encoded address still arrives as its own visit: `askRoute` keeps the percent-encoding in the stack URL and records that raw segment as the request, while the route param is the decoded id, so `routeDecoded` still runs. For the open channel, `takeDecoded` then `go`s to the existing `/c/<id>` entry (the one `openChannel` wrote) and drops the pending visit above it, so one Back lands on the list. For any other channel, `replace` rewrites this visit to `path("channel", id)` and the request keeps that decoded id, so the later open still replaces this same visit. The new test drives that stack (`#general`, then `#help`, then `#general` with every digit encoded) and would still show `#help` on Back if the pop were removed.

Lesser: the canonical check is the literal `` `/c/${id}` ``, which matches `path("channel", id)` for these decimal ids.

RIGHT
