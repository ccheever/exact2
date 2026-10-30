# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `e130732a79`.
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

- **P1 — Consuming the up can strand native tracking.** [MouseChainMac.swift:361](/tmp/iosfix2-rv-pd3-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:361): after a mid-bubble up is swallowed, `tracksPointer` clears `early` and enters AppKit tracking without an up remaining in the queue. The default-mode block cannot end that tracking. A standalone `NSTextView` probe waited until a rescue up arrived ~620 ms later; the default-mode block stayed pending. [EditorPointerOrderMacTests.swift:249](/tmp/iosfix2-rv-pd3-astra/host/apple/tests/ExactKitTests/EditorPointerOrderMacTests.swift:249) masks this by re-posting the up. Preserve the release for native tracking and test with only one up.

- **P2 — The detached-source fallback bypasses the deferral.** [ExactViewMac.swift:338](/tmp/iosfix2-rv-pd3-astra/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:338): the new interception runs **after** lines 333–335 call `source.pointerReleased(event)` when `source.window == nil`. If an inner pointerdown removes an ancestor containing the editor, then pumps the queued up, that fallback releases the captured path while down is still bubbling. Surviving outer ancestors again hear up before down. Handle the early press before this fallback; the detach test currently detaches only after bubbling finishes.

- **P2 — Later contacts’ ups are swallowed too.** [MouseChainMac.swift:532](/tmp/iosfix2-rv-pd3-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:532): every release while `earlyBubbling` is true returns `true`, including releases after `upWhileBubbling` was already set. If the handler’s nested event loop remains open for another click, that click’s up is swallowed—even in another window, because this runs before `hosts()`. Its mouse-up action or pointer release is lost. Returning `nil` stops dispatch under [AppKit’s monitor contract](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html). Consumption must be limited to the original contact.

No files changed. The repository XCTest suite was not rerun in this read-only worktree.

VERDICT: CHANGES
