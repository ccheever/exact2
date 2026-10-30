# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `2a7268bf49`.
- **Method:** one brief (sha256 `5ff76ef67836a7b22707bc260dcd67713c1d8daccbe2022e665d9da0238b4964`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 915b851e87 and e130732a79, each with a test that fails before the fix

## Brief

Code review, round 2 of 3 (host change, macOS). Tip 2a7268bf49 is HEAD of this worktree; the base is 4293f59072 (origin/main). Review the whole change with `git diff 4293f59072..HEAD`, and round 1's fixes with `git diff d12e915ec5..HEAD`.

Background: on macOS, NSWindow.sendEvent makes the hit text editor first responder BEFORE its mouseDown. So for a press that moves the focus into a text area or an input, the old field's change and blur and the new one's focus came before pointerdown. Chrome's order is pointerdown, then blur/change and focus, then pointerup.
The fix (theme2 approved the plan; ORDER ONLY):
- The session window's local monitor (ExactViewMac.swift) calls Presenter.pressEditorFirst (MouseChainMac.swift) on .leftMouseDown. It hit-tests the window's frame view and resolves the editor's node as each editor's mouseDown does (TextArea.owner, a field's delegate, the field editor's delegate's delegate).
- If the node tracks the press, AppKit will give it the press (key window of the active app, or acceptsFirstMouse), and it is not a control-click, it sends pointerdown early and keeps mouseChain.early.
- tracksPointer then sends only the up.
- The monitor releases an early press that no tracking took at the leftMouseUp, before anything else can consume the up. A press released that way is marked `reported`, so a later tracking of the same event reports nothing.

Round 1 findings, and what was done:
- Grok blocker: an up inside a nested loop that the early pointerdown runs (a sheet's animation) released the press, and tracking then reported it again. Fixed with `reported`.
- Grok major: `early` was weak, so a press the editor never tracked (the page went inert under a sheet) stayed held once AppKit freed the event. Now it is strong. A stale early is released at the next leftMouseDown, and Presenter.reset clears both.
- Grok and Astra minor: lent fields (the toolbar search, an alert's prompt) kept the old order. Fixed with delegate resolution and the frame-view hit test.
- Astra: a closedby="any" sheet opened by pointerdown consumed the up before the release. The release now runs at the top of the monitor.
- Astra and Grok: the first-click and control-click tests allowed empty logs. The first-click test now asserts exact sequences: field and password down first; the text area nothing, per LLP 1115.000 D7.9.
- New tests: testADownThatOpensASheetReportsThePressOnce and testADownThatOpensALightDismissDialogStillReleases. With the press helper now draining the down in an autoreleasepool, both FAIL on round 0's code (stuck hold, next press lost) and pass now.
Tests: host/apple/tests/ExactKitTests/EditorPointerOrderMacTests.swift has 12 tests; EditorPointerMacTests has 4. The full Mac suite on the round 0 tip was 302/0; the suite on this tip is running.

Check hard:
- Is any path still able to double-report, leave a hold, or drop a pointerup? Consider the nested loops, menus opened from pointerdown, drag sessions, window close, and a second session's monitor in another window.
- Does releasing a stale early at the next down hurt anything?
- Is hitTest on the window's frame view right in every hosted window: the main window, sheets, NSPopover windows, alert sheets, and the toolbar? Could it ever hit something in the titlebar that is not an editor yet resolves to one?
- Can a `reported` event wrongly suppress a real press?
- Is the LLP 1005 text accurate?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

1. **[P1] A nested up can precede an ancestor’s down — [MouseChainMac.swift:515](/tmp/iosfix2-rv-pd2-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:515).** If an inner `pointerdown` opens a sheet and its animation receives the up, `releaseEarlyPress` releases the entire captured path while `pointerPressed` is still bubbling. Afterward, the outer loop resumes and sends the remaining ancestors their down. An ancestor receives **up, then down**, with no further up because `reported` suppresses tracking’s release. This can leave application press state stuck. Defer release dispatch until the down finishes bubbling; cover nested listeners in the sheet regression test.

2. **[P1] Early dispatch admits clicks behind native alert sheets — [MouseChainMac.swift:491](/tmp/iosfix2-rv-pd2-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:491).** `acceptsFirstMouse` does not establish that AppKit will deliver a click through an attached sheet. Native alerts return from `DialogHost.show` before entering `dialogs.entries`, so their underlying page is not excluded by `DialogHost.blocks`. An enabled field behind that alert can therefore pass this guard and run `pointerdown`, although AppKit prohibits interaction with the parent window. Check the window’s modal eligibility before early dispatch. [Apple’s sheet documentation](https://developer.apple.com/documentation/appkit/nswindow/beginsheet(_:completionhandler:)) supports this distinction.

3. **[P2] Unmounting the session view loses the release — [ExactViewMac.swift:337](/tmp/iosfix2-rv-pd2-astra/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:337).** If early `pointerdown` causes an embedder to detach its `ExactView` while retaining the session, `viewDidMoveToWindow` removes this monitor at line 320. Tracking never starts, and neither release fallback receives the up. `early`, `pointerHeld`, and `pointerPath` remain until reset or another down after remounting. Resolve the pending press when removing its monitor, or preserve release observation. LLP 1005’s unconditional “released at the button’s up” claim is inaccurate for this case.

These are static findings. XCTest was unavailable in this read-only worktree; the standalone AppKit probe could not attach a sheet. No files changed.

VERDICT: CHANGES
