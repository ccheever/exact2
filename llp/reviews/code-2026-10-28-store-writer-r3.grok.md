# Code review: Linux and Android store writes on a thread of their own, round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `a6e4ba1244`.
- **Method:** one brief (sha256 `391b56ac01a6aecb86ae36e763b376d13e57c74ca450048ff5ef8c9e64d0e178`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `ExactActivity.kt:472` — `onStop` posts `hidden` and then `drainStore`, and only after that wait returns does it release pads (`ExactActivity.kt:473`) and post `view.running(false)` (`ExactActivity.kt:474`). `hidden` runs `Native.hidden` and then `produce` (`Owner.kt:348`, `Owner.kt:597`). `produce` calls `schedule`, which posts `again` or a due `timer` on that same handler (`Owner.kt:452`, `Owner.kt:459`). The two `onStop` posts land back to back, so the queue is the hide task, then the drain task, then that follow-up. The drain waits for commits already handed to the writer. The follow-up then runs, `Native.frame` can commit, and `submit` returns while those files are still queued. `running(false)` is posted after the wait, so it sits behind that follow-up and its `removeCallbacks` (`Owner.kt:224-225`) does not cancel it. `Pads.background` posts `Exact.pad` onto the same thread after the drain has returned. `onStop` then returns. A kill after `onStop` — the case this drain is there for — drops those writes.

   Fix: make the exact-handler order hide, then pad release, then stop turns, then drain. Post `view.running(false)` before `drainStore`, so its body sets `running = false`, drops the choreographer callback, and `removeCallbacks` on `again`, `timer`, `refiner`, and `settle` before `storeDrain` runs. That removes the follow-up `schedule` just queued. Call `pads.background()` before the drain as well, so its pad task is ahead of the wait. Commits the hide and the pad release already made stay in front of the drain and are written. With `running` false, `schedule` posts nothing further.

**Non-blocking**

1. `store_writer_tests.rs:361` — `a_drain_that_already_retried_does_not_retry_again` passes a preset flag to `drain_retrying`. The boot path is `settle`, which is its own loop around 5 s rounds (`store_writer.rs:217-220`). Point a test at `settle` where the first round times out while a retry is still owed, and assert a later round leaves the key unwritten.
