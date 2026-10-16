# Code review: the Apple dev client waiting for its program instead of stopping, round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `9c697349e031f17c5d4d3e7313c775f0b1a2e7e8230cd0f5e9552cdd8bfa2ee6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

Nothing blocking.

Once a program has been adopted, a generation for another program is not fetched or applied. `acceptProgram` still adopts only the first program it sees (`Session.swift:232-236`); after that, a mismatch returns before any payload fetch. `{rebuilt}` and a foreign revision both go through `waitForProgram` (`PlanURL.swift:830-836`), which sets `terminal` and bumps `resolution` so in-flight fetches cannot commit (`495-499`, `690`, `707`). The envelope path returns before `fetchGeneration` when the program does not match (`648-653`). `observe` only clears `terminal` after `acceptProgram` succeeds (`549-557`). The first program is still booted, same as before this commit; the old `terminal` stop only ran when `acceptProgram` had already returned false.

A server that keeps announcing the other program does not spin. Each announcement calls `beginResolution` and leaves the stream up; it does not `resolveAndBoot`, resubscribe, or re-register. Reconnects happen when the stream actually dies (`816-823`, `838-846`), with the existing 1s–5s backoff. `beginResolution` clears `syncing` and drops the snapshot fetch, so a design sync is not left stuck; the next batch starts another.

Design frames are handled before the program check (`867-869`) and still go through the shape check. Hello still registers before `observe` (`875-878`), and `beginResolution` does not cancel the report post, so the build stays on the channel. `reload()` clears `terminal` and rediscovers (`442-445`); a foreign envelope sets it again. `close()` still stops retries. `connectionStatus` stays "(rebuild the host)" while `terminal` is set (`Session.swift:252`).

The commit's claims hold for the restart case it describes: refuse the foreign generation, keep the stream and the registration, reconnect, and take a later generation whose program matches.

No test covers this path. The diff is only `PlanURL.swift`, and the commit message's check was a manual loop restart.
