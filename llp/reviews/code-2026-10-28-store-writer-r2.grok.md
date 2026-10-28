# Code review: Linux and Android store writes on a thread of their own, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `84728e0afa`.
- **Method:** one brief (sha256 `8560dc046a6258fb89b5d487e02ded19ff37142798aa40b6780b6cfb47800abc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `host/linux/src/store_writer.rs:122` — Collapsing an empty commit leaves the queued retry's `StoreRoots` as they were when that retry was first queued. A later empty commit, after the environment was corrected, does not run and does not update those roots, so the retry can keep failing against the old location until some later commit or a `drain` resolves roots again. On the collapse path, set that queued commit's `roots` to `store_roots(app)` from now.

2. `host/windows/src/window.rs:720` — Linux teardown logs an app's leftover warnings after its drain (`host/linux/src/teardown.rs:41`). Windows drains and then exits without `take_store_warnings`, so a failure that nothing committed after is never logged. After the drain, take that app's warnings and print them the same way.
