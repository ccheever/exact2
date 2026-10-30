# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `2a7268bf49`.
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

I'll review the host-change diff against both bases and check the pointer-order paths for double reports, stuck holds, and dropped ups.The range is four commits on a detached HEAD. I'll read the full diff and the pointer-order code those commits touch.The round-1 delta is small. Next I’ll read the monitor, the press chain, and the tests that pin the order.The early-down path is right for an ordinary click: one `pointerdown` before focus, one `pointerup` from tracking, and a second session's monitor cannot report that press. Two holes remain when the early `pointerdown` runs a nested loop (the sheet animation this round is about).

## Findings

### 1. Major — a nested up is delivered inside the down walk, before focus
`host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:337`, `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:333`, `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:511`

`releaseEarlyPress` calls `pointerReleased` synchronously. Sheet animation (`DialogHost.begin` → `beginSheet`) runs that up from inside `pointerPressed`'s handler, before the walk finishes and before `NSWindow` makes the hit editor first responder.

`pointerPath` already lists every ancestor. The up goes to all of them, then the walk continues and sends `pointerdown` to ancestors that just received `pointerup`. Those later downs are not held and get no up. On a single listener the press's own node is paired, which is all the new sheet test checks, but the order is `pointerdown`, then `pointerup`, then the clicked field's `change`/`blur`/`focus`. Chrome's order, and the order this change exists to produce, is the reverse of that last pair.

A menu opened from `pointerdown` does not hit this: `MenuHost.show` defers `popUp` with `RunLoop.main.perform` until the click has unwound. A text drag does not either: `tracksPointer` clears `early` before `super.mouseDown`, so the drag session's up finds nothing to release and the single `pointerup` is the one after the loop.

### 2. Major — `reported` remembers one event, so the original press is reported again
`host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:231`, `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:355`, `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:514`

`reported` is one strong reference, replaced on every release. The animation loop that consumes the first up can also dispatch a second editor click before the original `mouseDown` reaches `tracksPointer` (a double-click fits the sheet animation; `beginSheet` spins the run loop). The second release overwrites `reported`. Tracking the first event then takes the `else` branch: another `pointerdown` and another `pointerup`. If that second press had already set the hold, the extra `pointerReleased` ends the second hold as well.

`reported === event` does not suppress a different press. The event is retained, so AppKit cannot recycle that object. The failure is the opposite: the event that should stay suppressed is forgotten.

Releasing a stale `early` at the next down (`MouseChainMac.swift:490`) is safe for that next press. It runs before the `pointerHolds` guard, ends the old gesture, and arms the new one. The old up's point is the new down's point (`pointerReleased(nil)` samples `NSApp.currentEvent`). It does not drop the new up, unless that stale up's own handler nests and consumes the new up before `early` is set to the new down — the same re-entrant release as finding 1.

### 3. Minor — LLP 1005 overstates both the order and "tracking never starts"
`llp/1005-plan-and-runner-v1.spec.md:609`

The key-window sentence (down, then `change`/`blur`/`focus`, then up; tracking reports only the up) matches a press whose up arrives after `tracksPointer` has cleared `early`. It does not match finding 1: the up is sent from the monitor, and `tracksPointer` still runs `super.mouseDown` — it reports nothing because of `reported`, it does not skip tracking. The control-click, first-click, and covered-editor sentences match the code. `Presenter.reset` clearing `early` and `reported` without an up matches the existing "a reset clears the hold" line (`PresenterMac.swift:600`).

### 4. Minor — closing the window from the early down drops the up until a later press
`host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:320`, `host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:337`

The monitor is removed in `viewDidMoveToWindow` while `early` and the hold are still set. The up is never seen. Nothing releases the hold until `reset` or the next `leftMouseDown` after this view has a window again. A second session does not do it: each presenter releases only its own `early`.

## Hit testing

`window.contentView?.superview` is the frame view this host already uses for clicks. It sees the page, a sheet, an alert sheet, a popover, and the toolbar, which is why a lent search field is found. `editorOwner` (`MouseChainMac.swift:502`) resolves only a `TextArea` whose `owner.textArea` is that area, or an `NSTextField` whose delegate is the `NodeView` that still has that field, or that field's current editor. A title-bar text field fails that test. So does a pill: `TextArea.hitTest` returns an `AtomCatcher`, and `editorOwner` returns nil, so a pill never takes the early path. Table rows are not a false press: `whatIsThere` reveals the cell, and an editor is an `NSControl`/`NSText`, which `keepsClick` already gives the click to.

VERDICT: CHANGES
