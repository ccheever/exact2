# Code review: a stale slice frees what waited for it, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `cfa73c2762daca7023fa03c2e49572df596aa337f36f9bf570c6b04063e4d638`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocker (Astra P3; Grok P2).
- **Disposition:** Astra's P3 applied without re-review; Grok's P2 not taken, with the reason in the third-round commit

---

I'll review the last three commits as a read-only pass, starting with the review guidance and the commit diffs.This is a read-only review of the last three commits. I'll inspect the diffs and the surrounding session, executor, and test code against the checklist.The three commits are in. Next I’ll read the landing, idle, and reset paths those diffs touch, including every place the presenter’s views are replaced.[P2] A stale landing applies owner batches already queued behind the slice before the work that was waiting on it — `host/apple/Sources/ExactKit/Session.swift:667`

`landFill` keeps draining `publishedQueue` after a stale fill (`Session.swift:644`). A tick or canvas the owner published after that fill, with the generation from after the commit, takes the current-generation arm and `apply` runs in that same call (`Session.swift:647`). The pump, timer advance, and other `afterFill` work is only dispatched at `Session.swift:667`, so it runs on the next main turn, after those batches. A current-generation fill still runs that work inline at `Session.swift:666` before the loop continues. The owner queue is serial, so the usual shape is real: the in-flight fill is posted first, a post-commit tick is posted behind it, both are in `publishedQueue` before main drains, and the tick commits before the pump that was held for the fill. The same call also runs `collections.landed` (`Session.swift:665`) before scheduling the deferred work, so a held report flush is queued ahead of the pump as well.

Nothing else in this round is blocking. The lost pump is not dropped, and bound work from the old views cannot run on the new ones.

The deferral still runs while the session is alive. `idle` (`Session.swift:573`) keeps each item's `views` value, re-queues it if a slice is in flight, and checks the epoch immediately before running. A destroyed session drops the deferred block (`Session.swift:667`) and drops anything still in `afterFill` (`Session.swift:664`). `drainFill` returns before that turn; `IdleTickTests` locks that. `clock settle` does not depend on it: `waitForReplies` pumps the executor itself and turns the run loop, so the fixed point is taken after the deferred work has had a turn. Nothing else reads `afterFill`; it is empty once `landedFill` returns because the closures live in the deferred block.

Bound work cannot slip through. `whenIdle(bound:)` stores `viewsEpoch` (`Session.swift:572`). The stale path passes that same tuple to `idle`, and both the inline and deferred arms require `item.views ?? viewsEpoch == viewsEpoch`. A later reset bumps the epoch, so an item queued for the old views fails that check. Unbound work (`views == nil`) is the pump and the timer advance; it stays eligible.

`viewsEpoch` is bumped after `presenter.reset()` on both paths that install a new tree: the reboot in `finishBoot` (`Session.swift:833`) and `presentCommitted` (`Session.swift:921`). iOS navigation reports cleared covers synchronously inside `reset`, before that bump, so those clears are bound to the old epoch. Mac cover reset does not report. `destroy` (`Session.swift:1491`) resets without bumping the epoch; `state` is already `.destroyed`, so a queued landing drops the work and a synchronous report hits `apply`, which returns immediately. Collections, navigation, and controls reset only from `presenter.reset()`.

The test fails on the old drop (`free` stays 0 through the run-loop wait) and on the inline landing from the first fix (`free == 0` fails inside `drainFill`). It also checks that work queued before `commitForTests` is dropped and work queued after it runs. It does not bump the epoch again during the deferred turn, and it never calls `presenter.reset()`, so a binding stripped inside the deferral, or a bump placed before `reset()`, would still pass.
