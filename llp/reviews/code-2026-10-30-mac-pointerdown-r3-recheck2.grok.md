# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3, second narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `497de689b9`.
- **Method:** one brief (sha256 `72b39d28326de5ed5fb63e06ebc2b0b9dced5bacfb9d8b1c373ddf361f85c875`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. The full Mac suite reruns on the final tip.

## Brief

Final narrow re-check (host change, macOS: early pointerdown before focus in a text area or an input). HEAD 497de689b9; the last re-check reviewed 9bd2e2f84b. Read `git diff 9bd2e2f84b..HEAD` (2 commits, small), with context from `git diff 4293f59072..HEAD` as needed.

Findings from the last re-check, and the fixes:
- Astra P1: the re-posted up could be consumed before tracking starts (another local monitor's nested loop), leaving tracking waiting. A press released before its tracking now runs no tracking at all: in tracksPointer, the `reported` branch returns without track(), because its up is gone and AppKit's loop would wait for another.
- Astra P2 and Grok P2: a second up while the first is held fell through to the detached-source fallback and released mid-bubble. The fallback now requires mouseChain.early == nil. Test: testOnlyThePresssOwnUpIsHeld goes through NSApp.sendEvent, with and without the editor detached. On 9bd2e2f84b the detached case gave ["down-z","up-z","up-zo","down-zo","down-zo","up-zo"]; it passes now.
- Grok P2: a nested mouse-down during bubbling hit the stale release at the top of pressEditorFirst and released the press early. The stale release now skips while earlyBubbling. Test: testAClickWhileTheDownBubblesLeavesThePressAlone, red on e8cc923edf (up before focus) and passing now.
- Named residual, not fixed: a focus or blur handler that spins its own event loop between the re-post and the editor's tracking could still take the up, and AppKit's tracking would then wait for the next up. This is the general AppKit hazard of any loop eating a button's up, and it exists without this change too. It will be stated to theme2 at sign-off.
Tests: EditorPointerOrderMacTests has 18 and EditorPointerMacTests 4, all passing. The full Mac suite on 9bd2e2f84b was 302/0; the suite on 497de689b9 is running.

Check only:
(a) Are these three fixes correct?
(b) Did they introduce any new double report, stuck hold or lost up?
(c) Is the residual fairly characterised?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check only those three fixes against the two-commit diff, then look at the surrounding pointer-order code for a new double report, stuck hold, or lost up.The two commits are small. I'll read the press, hold, and release paths around those lines before judging the three fixes.The three edits sit on the early-press path. I'll trace what happens to a down that is released, re-posted, or clicked again before tracking starts.The three fixes are correct, and they do not add a double report, a stuck hold, or a lost up. The residual is fair.

**(a)** Each fix does what the last re-check asked.

- A press already released is not tracked. `tracksPointer` drops it from `reported` and returns before `track()` (`MouseChainMac.swift:362-363`). The up was already emitted in `releaseEarlyPress` (`MouseChainMac.swift:541-543`), and this return is also before `pointerReleased` (`MouseChainMac.swift:366`), so that up is not sent again. Calling `super.mouseDown` here would wait: that up is already dequeued. Skipping it also avoids the field-editor re-entry, where `chain.tracked === event` would call `track()` anyway (`MouseChainMac.swift:358`).
- A second up while the first is held no longer releases mid-bubble. During `earlyBubbling`, only the first up is stored and the monitor swallows it (`MouseChainMac.swift:534-539`, `ExactViewMac.swift:334`). A later up returns false and used to fall through to the detached-source `pointerReleased`. That path now requires `mouseChain.early == nil` (`ExactViewMac.swift:337-339`). After the bubble, the held up is posted again (`MouseChainMac.swift:511-513`) and `releaseEarlyPress` emits the one pointerup. That call nils `pointerSource`, so the fallback does not fire a second time.
- A mouse-down nested in the bubble does not end the press. The stale `releaseEarlyPress(nil)` runs only when `early != nil` and the down is not still bubbling (`MouseChainMac.swift:497-499`). A click in that handler therefore does not set `releaseAfterBubbling`, which is what used to emit the pointerup before focus. The nested down cannot replace `early`: `pointerHolds` is already set, so `pressEditorFirst` returns before assigning it (`MouseChainMac.swift:500-508`).

**(b)** No new double report, stuck hold, or lost up on these paths. The normal press still clears `early` and tracks once (`MouseChainMac.swift:364-366`). The detached fallback still runs for a non-early press whose view has left the window. A left up for an early press is still taken by `releaseEarlyPress` once the bubble has finished, including when the editor has been removed.

**(c)** The residual is fair. Between the re-post (`MouseChainMac.swift:511-513`) and `track()`, NSWindow still runs the old field's change and blur and the new field's focus (`MouseChainMac.swift:351-354`). A loop there that eats the up without `releaseEarlyPress` recording the down leaves `early` set and the down out of `reported`, so `track()` waits for a later up. A loop that sends the up through the monitor does not wait: the press is recorded and fix (a) skips tracking. The wait is the ordinary AppKit hazard of a loop consuming a button's up before `mouseDown`'s tracking loop, and it exists for a text view without this change.

No findings.

VERDICT: APPROVE
