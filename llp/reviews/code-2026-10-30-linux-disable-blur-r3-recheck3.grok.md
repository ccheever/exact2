# Code review: Linux focus on a control a commit disables, round 3-recheck3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `16adda3519`.
- **Method:** one brief (sha256 `fb554e815fcb1f17806d996eb9d0a6acf1c306f7b481095e1348443a28ffd428`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll re-check this commit read-only: the field and hover blur-between-events paths, and whether the unchanged canvas GPU batches are a blocking gap or a stated follow-up.The commit is the Linux focus-fixup change. I'll read the field, hover, and canvas batch paths and the tests that are supposed to pin the first two.The two call sites are in. Next I’ll read the tests and the canvas message loops to see whether the deferred GPU batch is the same class of bug.(1) and (2) close the gap the last re-check named. (3) stays a real hole, and it is a follow-up, not a reason to block this commit.

**(1) Field Enter.** `commit_text` applies the change through `host.dispatch_at` before it returns, so `disabled` is already on the kernel node. `blur_disabled_now()` then runs, and only then `submit_event`. `commit_text` clears `edited` first, so the blur's own focus move does not fire a second change. Without that call, the submit handler sets `off = false` before `clear_removed_focus`, the field is enabled again, and there is no blur: the log would be `cs` and focus would remain. `a_field_its_change_disables_blurs_before_its_submit_runs` expects `cbs` and `focus == None`, so it is red on that order.

**(2) Hover.** `hover_at` still walks leave, then enter, outermost first. After each `Hover` it calls `blur_disabled_now()`. The argument to `Option::or` is evaluated immediately, so the blur runs even when an earlier dispatch already stored an error. The hover test enters the outer column (`off = true`) and then the inner one (`off = false`). Blur-after-the-batch would see the button enabled and log nothing. The test expects `b` and `focus == None`, so it only passes if the blur runs while the outer hover's disable is still in effect.

**(3) Canvas GPU messages.** Both loops in `Surfaces::sync` (`gpu_messages` around lines 807 and 888) still call `host.dispatch_at` for every message before the presenter services anything. A control disabled by one message and re-enabled by a later one in that batch keeps focus and runs no blur. That is the same shape as (1) and (2). It does not block this commit:

- `sync` only has `&mut Host`. Focus and `blur_disabled_now` live on the presenter, so this is not a missed call at those loops. Closing it means a new seam (return the messages and dispatch them from `sync_surfaces`).
- A control that stays disabled is still blurred. Message dispatch sets `changed`, and `Presenter::sync_surfaces` calls `after_commit` after `sync` returns, which runs `clear_removed_focus`.
- The commit message states this limit and names a later item. It does not claim the GPU path is fixed.

The previous re-check's three sites were right. This commit fixes the two the presenter can reach, with tests that fail on the old interleaving, and leaves the host-only batch as a stated follow-up.

APPROVE
