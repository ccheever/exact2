# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 3, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `48f9f4d96a`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 4d87dbeab6: scrolled is main's 15-row rule again and leadOlder alone anchors from 100 rows (its away gate holds through a send's settling); a restore holds the lead with leadHeld instead of clearing topKey; the lead test now pins leadOlder (mutation-checked).

The 100-row anchor change is right for the send it fixes, and the channel and placement edits are clean. One defect: `settling` can stay true, and while it does the new condition keeps the 100-row rule off.

## Findings

**[P2] A send from history leaves `settling` true on Windows, so the 100-row rule stays at 15 for the rest of the session — `apps/excord/app.contract:605`**

`post` sets `settling = true` only when a send returns the reader to the live end (`away or anchor != ""`, lines 806–810). The only write back to false is `scrollEnded`, and only when `target == ""` and `endGap <= 0.5` (lines 618–622). The painted Windows presenter never delivers `scrollend`: `tracks_scroll_end` is hard-false under `cfg!(windows)` (`host/linux/src/presenter/scroll_end.rs:45–51`), which is the fact the hover comment already states (lines 106–107). The 200 ms quiet path calls `scrollSettled` (lines 345–346, 1111–1113), which does not touch `settling`. `openChannel` does not either.

After that send, `not settling` at line 605 stays false. A later scroll up in history, where `isAway` is true and the 100-row lead should arm, still uses 15. The lead task can continue only after that 15-row edge sets `anchor`; a fling of 60–90 rows overruns it. That is the F1 case this rule exists for. Web tests do not catch it: `clock settle` waits for a `scrollend` Windows never sends. While `settling` is still honestly true during the return, keeping the threshold at 15 is the right fix for the 99-row tail (`live.test.ts` grows that tail with `history.slice(-99)`), because every index in that window is `< 100`.

## The other two edits

**openChannel is clean.** `topKey = open == id ? topKey : ""` at line 460 is before `open = id` at line 461. The right-hand side is read before that write is recorded, and neither `open` nor `topKey` was assigned earlier in the action (`closePicker` and `putMembersAway` do not write them). Later lines use the parameter `id` and the local `c`, not those slots, so this is not a read-after-assign. Action reads see the starting snapshot anyway, so the comparison is “was this channel already open?” Re-selecting keeps `topKey`; a different id clears it. The list is `for ch in [open] key=ch` (line 1351), so a re-select does not remount the transcript and the kept key is still that list’s row. The place of the channel being left is saved at line 452, before the clear.

**Clearing `topKey` on an offset placement does not undo those restores.** `afterPage` copies the row into `target` and `topOffset` into `restoreOffset` (lines 426–429) before `landJump`. The offset branch scrolls `keyOf(rows, target)` at `restoreOffset` and only then sets `topKey = ""` (lines 680–682). `leaving` saves `topKey` in its own action (line 557). `jumpTo` stores `jumpFromKey = topKey` with `targetBlock = "center"` (lines 641–646), so the outward jump does not take the clearing branch; the failed-jump return copies `jumpFromKey` into `target` (lines 693–696) and the later offset land scrolls that target. Fixture `message` / `burst` injections ignore `near`; the `*-above` actions use it only as a hint (`fixture/inject.ts:71–72`) and read `topKey` when the button runs (`injectNow`, line 1134), not inside `landJump`.
