# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `e130732a79`.
- **Method:** one brief (sha256 `a99b84efbcf3cacd76ebb5f57482753bbd5fb876f29909e7118b9175948b9f81`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 9bd2e2f84b; the single-up test hangs on the reviewed code (sampled in tracksPointer under AppKit tracking) and passes after

## Brief

Code review, round 3 of 3 (host change, macOS). Tip e130732a79 is HEAD of this worktree; the base is 4293f59072 (origin/main). Review the whole change with `git diff 4293f59072..HEAD`, and round 2's fixes with `git diff 2a7268bf49..HEAD`.

The change (theme2 approved; ORDER ONLY): on macOS, a press in a text area or an input reports pointerdown before NSWindow focuses the editor, as Chrome orders them.
- The session window's local monitor (ExactViewMac.swift) calls Presenter.pressEditorFirst (MouseChainMac.swift) on .leftMouseDown: frame-view hit test, owner resolution as each editor's mouseDown does, tracksPress, first-click rule, no control-click, and no window under a sheet or another window's modal session.
- It sends pointerdown early and keeps mouseChain.early.
- tracksPointer sends only the up.
- The monitor releases an early press no tracking took at the leftMouseUp, at the top, before anything else can take it.

Round 2 findings (Astra and Grok both CHANGES), and what was done:
- An up taken by a loop that the early pointerdown runs (a sheet's animation) while the down is still bubbling was released mid-bubble: outer ancestors heard up before down, and the up came before the focus. Now releaseEarlyPress returns true, the monitor returns nil (the window has not had the down, so the up is no one else's), and the release is scheduled with RunLoop.main.perform(inModes: [.default]) for after the window's turn with the down. A default-mode block waits out a tracking loop, so if tracking runs it releases, and the block finds `early` changed and does nothing. Test: testAnUpTakenWhileTheDownBubblesWaitsForTheWindow (the inner node's onPointer delivers the queued up mid-bubble). It failed on 2a7268bf49 with ["down-z","up-z","up-zo","down-zo","blur-a","focus-z"]; now it is ["down-z","down-zo","blur-a","focus-z","up-z","up-zo"].
- Clicks behind a native alert sheet passed the first-click guard. Now there is `window.attachedSheet == nil` and the NSApp.modalWindow check. Test: testNoEarlyDownBehindAnAlert, red on 2a7268bf49.
- An ExactView detached during an early press lost the release with its monitor. viewWillMove(toWindow:) now releases. Test: testAnEarlyPressIsReleasedWhenTheViewLeavesItsWindow, red on 2a7268bf49.
- `reported` held one event and could be overwritten. It is now a list of the last 4, each removed when its tracking comes, and Presenter.reset clears it.
- LLP 1005's text was rewritten to match.
- Window close: the monitor stays with the view, and the release runs before the hosts() guard, so an up for a closed window still releases.
Tests: 15 in EditorPointerOrderMacTests plus EditorPointerMacTests' 4, all passing. The full Mac suite on 2a7268bf49 was 302/0; the suite on this tip is running.

Check hard, final round:
- Any remaining double report, stuck hold, or dropped pointerup.
- Is the monitor returning nil for a mid-bubble up safe? Think of other monitors, AppKit's own state, a mouseUp that AppKit needs for a sheet or animation, and drags.
- Is the RunLoop.main.perform block safe (retain cycles, the session destroyed before it runs, a reset in between)?
- Is the attachedSheet/modalWindow guard right for sheets on a sheet, popovers, and the agent's never-key window?
- Do the tests prove what they claim?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the macOS pointer-order change against the base and against round 2, focusing on double reports, stuck holds, and dropped pointerups. No files will be edited.This is a direct defect review of the pointer-order change, not a posted PR review. I'll read both diffs and the surrounding host code before judging the round-2 fixes.The round-2 diff is small. Next I’ll read the monitor, the early-press path, and the tests that claim to cover the new order.[P1] A mouseUp swallowed while the early down is still bubbling never comes back, so the editor's tracking loop waits until some later click — host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:338

`releaseEarlyPress` during `earlyBubbling` only sets `upWhileBubbling` and returns true (`MouseChainMac.swift:532`). The monitor then returns nil, so `sendEvent` drops that mouseUp. The event is not kept and not posted again. After the bubble, `pressEditorFirst` schedules the semantic `pointerup` with `RunLoop.main.perform(inModes: [.default])` (`MouseChainMac.swift:511`). That block does not run in the event-tracking mode the editor's `mouseDown` uses, and `tracksPointer` still starts that loop once this monitor returns the down. The loop waits for the mouseUp that was discarded. `testAnUpTakenWhileTheDownBubblesWaitsForTheWindow` only finishes because it posts that same up again (`EditorPointerOrderMacTests.swift:249`). A `pointerdown` loop that dispatches the queued up — the sheet animation this path is for — leaves the button held until a later mouseUp, and that later up is what ends the original tracking.

[P2] An up after the source view has left, while the down is still bubbling, is released immediately — host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:333

The deferral lives only in `releaseEarlyPress`. The monitor still calls `pointerReleased` first whenever `pointerSource.window == nil`. A `pointerdown` that removes that node, and whose loop then delivers the up, sends `pointerup` along the whole path while outer ancestors have not heard `pointerdown`. `releaseEarlyPress` then returns nil as well. `viewWillMove` cannot cover this: it only runs for the session view, and during the bubble it only sets the flag.

The ordinary press is one down and one up. `tracksPointer` skips a second down when `early` or `reported` matches, and a release before tracking is what puts the event in `reported`. The run-loop block holds the presenter weakly, so there is no cycle. `reset` clears `early`, and the block then does nothing. If the session is gone, `self` is nil. The block is not the leak. It cannot unstick the tracking loop in P1, because it waits for that loop to finish.

`window.attachedSheet == nil` is the right sheet check: the event's window is the sheet that was hit, a sheet on a sheet leaves `attachedSheet` set on the covered one and on the parent, and the top sheet has none. `NSApp.modalWindow` being nil allows the click. A click on the modal window itself passes. A popover is not a sheet, and a click outside one still reaches the editor, so an early down there is the order this change is for. The agent window is never key. A text area and a field editor take that click when `everyClickCounts` is set, and an `NSTextField` accepts the first mouse on its own. Returning nil does not start or end a drag: the window has not had this down yet. It does steal the up from every earlier local monitor, and from the tracking loop that is about to run.

`testAnUpTakenWhileTheDownBubblesWaitsForTheWindow` proves the order only when the caller puts the up back. `testNoEarlyDownBehindAnAlert` proves a parent-window event is not reported early. It does not click a lower sheet under a second sheet. `testAnEarlyPressIsReleasedWhenTheViewLeavesItsWindow` calls `pressEditorFirst` and then replaces the content view. It does not remove the node while the down is still bubbling.

VERDICT: CHANGES
