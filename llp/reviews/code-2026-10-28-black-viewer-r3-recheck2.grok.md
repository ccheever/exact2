# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0b1913511b`.
- **Method:** one brief (sha256 `fa65a1f132dc9e0bf77d370eb33d92da71aa3d72c84d532cd4aa32e64e3db36d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/presenter/images.rs:51` — A queued `load` is checked, then sent with `dispatch_at`, and that call runs everything already due at `host.now()` before the handler (`runner/src/runner/advance.rs:48-50`). An `after` / `every` cannot join that set: the cutoff is not resampled, and the soonest arming is `now + 1`. A mutation answered in an earlier load's commit can. Its `then` (and a queue `next`) is stored at the arming time, with no 1 ms gap (`runner/src/runner/commit.rs:278-282`). On the seek clock — the agent and `EXACT_AGENT=1`, where there is no live sample — that time is `host.now()`. The next load in the same pass has already matched the node, the key, and the source; the `then` then changes or replaces that node, and the `load` handler still runs. The same-timestamp case exists on a live clock when the sample has not moved past `host.now()`. Run the work already due at `host.now()` before the check, and deliver the event with `Runner::dispatch` so nothing the clock owes runs between the check and the handler.

**Non-blocking**

1. `host/linux/src/image/remote.rs:332-336` — `cool` inserts the URL, then drops the soonest entry while the map is over 256. If the wait just recorded is the soonest, it is the one removed, so a later source of that URL fetches immediately. A map already full of longer waits does this to every new, shorter `Retry-After`. When trimming, drop some other entry, and keep the URL just inserted.
