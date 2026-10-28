# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `445212a3d6`.
- **Method:** one brief (sha256 `06ed48a670fd165025731aecf72384bf51f6374d74e734f7f8ab6bb003ea59a1`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/presenter/images.rs:45` — A no-gap `then` (or queue `next`) from an earlier load still is not settled before the next load is checked and sent, and `dispatch_at` can still run due work after that check. Android, the Linux display, and the canvas reader all install a live clock. A load handler arms its `then` at the runner's sampled arrival (`armed_at` is `clock.started` after `sample()`), and `Host::dispatch_at` never copies that sample back into `host.now()` (the live-clock test leaves the host at the event time and the runner at the sample). The next queued load reads the old `host.now()`, so `timer_due_ms() <= now` is false and the new `advance` is skipped. `dispatch_at(id, Load, now)` uses that same lagging cutoff, so it does not run the `then` either. The load goes out for a source the `then` is about to replace. This is the Grok 1 / Astra 1 case on the hosts that own a wall clock; the new test never sets one.

   The same call also widens the cutoff the other way. `self.advance(now)` is `Presenter::advance`, which publishes the live sample into `host.now()` (`Host::advanced`). `Host::dispatch_at` then does `self.now_ms = now_ms.max(self.now_ms)` and advances to that later time. Anything due in the gap — a timer that came due while the clock moved, or a `then` armed by the tail of that advance (`finish_commit`'s focusout, `deliver_scroll_ends`) — runs after the node/source check and before the handler. One `advance` is not a drain: a refusal returns early and leaves later deadlines queued for that second advance. Away-and-back done inside such a `then` still delivers the stale load; the retain in `sync_shown` only sees source changes the handler already committed.

   Fix: before the check, drain until nothing due by `host.now().max(runner.now_ms())` remains, and loop again after each `advance` so the tail cannot arm another deadline at that time. If `advance` stops on a refusal, put the event back and do not send it. Pass `dispatch_at` the `host.now()` from after that drain, so its `max` cannot move the cutoff. The check then matches what the handler will see.

**Non-blocking**

1. `host/linux/src/image/remote.rs:334` — A past `Retry-After` (an IMF-fixdate already gone becomes a zero wait) is inserted after the expiry sweep and then excluded from eviction. On a full map that drops some other URL's still-live deadline to keep one `cooling()` will ignore. Protect the new entry only when its `until` is still in the future.
