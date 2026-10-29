# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 4 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `1421c1821b`.
- **Method:** one brief (sha256 `993b9134f5562eee072f5bdbf8fe01f748f4f3100d4c8043af17ea8d94c0ed7a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/presenter/images.rs:51` and `host/linux/src/image.rs:413` — A source that changes away and back inside one drain `advance` still gets its old `load`. `Presenter::advance` runs every callback due by `T` in one `advance_timed`, then `finish_commit` syncs images once against the final tree. `assignment` bumps and the queued load is dropped only when `view.source != source` at that sync. Two commits at the same time (a `then` sets another source, and the `then` it arms sets the original back; both are due at the parent's `armed_at`, so both run before `advance` returns) leave the string unchanged, so the event stays queued and is sent. Keeping it queued does not help if sync never sees the middle source. The new test does not cover this: its away and back are two load handlers, and each `dispatch_at` already syncs in `after_commit` while later loads are still queued. Fix: drain one due commit at a time and sync after each, so the away commit bumps `assignment` and drops the load before the back commit restores the string. The round cap has to count those commits.

2. `host/linux/src/presenter/images.rs:58` — A load is held back for good while `timer_due_ms()` stays `<= now` and `advance` cannot clear it. After eight advances the function `break`s and leaves every load queued; the next turn does the same, and `image_turn_due_in_ms` stays `0`. That happens when a wake is only queued and a call is running: `wake_due` is `now` (`runner/src/runner/modules/wakes.rs:496`) and `step_wakes` returns immediately (`wakes.rs:264`), so `dispatch_at`'s `advance_timed` would not run that wake before the handler either. The picture is ready and `load` never fires until the call ends. Same class: a running call whose `complete_call` stays false and whose invocation owns nothing (`call_due` is `now`). Fix: withhold the load only when another advance would still run a commit before the handler. If an advance leaves the same due instant in place, send the load.

**Non-blocking**

The live-clock case (a `then` armed at the runner sample, which `host.now()` can lag) is resolved. `T` is `max(host.now(), runner.now_ms())`, the drain runs until nothing is due at `T` or it gives up, and `dispatch_at(id, Load, T)` only fires pre-handler work with `due <= T`, not a newer sample.

`cool` is resolved. A wait already past is an eviction candidate and is the soonest, so a live `Retry-After` is not dropped to keep it. A wait still in the future stays protected. With one entry per URL and a 256-entry cap, the empty-candidate `expect` does not run.

The away-and-back test never enters the drain's single sync. A load pushed during the drain can still be sent in this pass when `retain` has removed the event in front of it; the comment says those wait for the next turn. There is no test that an already-past `Retry-After` is the entry that gets evicted.
