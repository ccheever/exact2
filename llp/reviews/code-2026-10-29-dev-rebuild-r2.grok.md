# Code review: the dev loop's -dev bake driver rebuild, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `74680b6845`.
- **Method:** one brief (sha256 `e8303bc151fbf897760d21892f632eb17ee8e282cc0c6d385cf2270a4fc12ae8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `host/web/dev-bin-run.test.mjs:36` — A hard kill leaves both `sparkline/target/.dev-bin-run-test.lock` and a marked `deck/`. The next run waits 25 minutes and throws on line 36, before the `try` that deletes a marked `deck/` (line 40), and that throw is outside `finally`, so the dead lock is never removed. Every later run does the same, and the leftover `deck/` stays an asset root for sparkline. Record the holder pid in the lock directory and, if that pid is dead, remove the lock and fall through to the marked-`deck/` cleanup already on line 40.
