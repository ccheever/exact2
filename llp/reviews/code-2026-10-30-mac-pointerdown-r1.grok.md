# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `d12e915ec5`.
- **Method:** one brief (sha256 `e94bed2088e7382e6b4b54f5b2874bb576c0676955690ef5d2bd50875fed2eba`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 61bda1ee23 and 2a7268bf49, each with a test that fails before the fix

## Brief

Code review, round 1 of 3 (host change, macOS). Branch tip d12e915ec5 (HEAD of this worktree); base 4293f59072 (origin/main). Commits: c1579ba05c adds the red tests, d12e915ec5 is the fix. Review with `git diff 4293f59072..HEAD`.

Approved plan (theme2, 2026-10-30 07:06Z): ~/x2theme/iosfix2-mac-pointerdown-plan.md, if readable; summary here. On macOS, NSWindow.sendEvent makes the hit text editor first responder BEFORE its mouseDown. So for a press that moves the focus into a text area or an input, the old field's change and blur and the new field's focus came before pointerdown. Chrome's order is pointerdown, then blur/change and focus, then pointerup.
The fix:
- The session window's local monitor (ExactViewMac.swift) sees .leftMouseDown before NSWindow. In Presenter.pressEditorFirst (MouseChainMac.swift), it hit-tests the window. If the topmost view is a text area's NSTextView, an input's NSTextField, or that field's current editor; the node tracks the press (tracksPress, factored out of pressesUntracked); and AppKit will give the editor the press (key window of the active app, or the hit view acceptsFirstMouse); and it is not a control-click: it sends node.pointerPressed early and marks mouseChain.early = event.
- tracksPointer then skips its own pointerPressed for that event, tracks, and releases.
- If tracking never takes the press, the monitor's .leftMouseUp releases it (releaseEarlyPress).
- ORDER ONLY: a mousedown preventDefault stopping the focus move is out of scope, queued in QUEUE.md.

Theme2's conditions:
(1) a control-click and a double/triple click keep today's semantics apart from the order, tested;
(2) an editor covered by another view is not reported: the topmost hit decides, tested;
(3) an event for a window the session doesn't host is untouched;
(4) the full Mac suite on the tip (running).
Tests: host/apple/tests/ExactKitTests/EditorPointerOrderMacTests.swift (10 tests including EditorPointerMacTests, all pass on the tip; the order tests failed on main with focus before down). Also LLP 1005's macOS pointer paragraph and EditorPointerMacTests' header were updated.

Check hard:
- Correctness of the hit test (coordinates, TableRowCell.whatIsThere, the nearest NodeView ancestor really being the editor's owner for text areas and fields, search and password fields, inline atoms excluded).
- Can `early` leak? A press that never reaches tracking and never produces a leftMouseUp in the monitor (window closes, nested loops, drag sessions, menus, sheets opening from the pointerdown handler)? What happens to the NEXT press if `early` is stale? Note it's weak, so it is tied to the NSEvent's lifetime.
- Double reporting: a field's mouseDown handing down to its editor, the disabled/untracked node path (pressesUntracked then node.mouseDown then pointerPressed), and an up handled by both the monitor and the node path.
- The first-click rule: does `(NSApp.isActive && window.isKeyWindow) || hit.acceptsFirstMouse(for:)` match AppKit's own decision to send mouseDown? If not, what is the failure mode in each direction?
- Popovers and sheets in their own windows: is the monitor there (presenter.hosts)?
- The dialogs.pointer early return (a click behind a closedby=any sheet) still comes first.
- Tests: are they meaningful and not vacuous? Missing cases?
Do NOT edit files. Write findings with file:line and severity (blocker/major/minor/nit). End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the macOS pointer-down order change against the approved plan and the diff from `4293f59072` to `HEAD`, without editing anything.The request is a defect-first review of the macOS pointer-down order fix, read-only. I'll inspect the approved plan and the full diff from `4293f59072` to `HEAD`.The diff is truncated in the middle of the mouse-chain change. I'll read the new press path and the tests around it.The early-press path is in place. Next I’ll check hit-testing, the first-click rule, and whether `early` can stick or be reported twice.The early-press flag does not survive the cases the plan says it covers. A nested mouse-up reports the press twice, and a press that never reaches `mouseDown` leaves the hold stuck for the next click.

## Findings

### [blocker] A mouse-up inside the early `pointerdown` reports the press twice — `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:350`

`pressEditorFirst` sends `pointerdown` and sets `mouseChain.early` before `tracked` is set (`MouseChainMac.swift:490`). Sheet presentation spins the run loop until the animation finishes (`SheetsMac.swift:32`), and an alert or a file panel does the same. A mouse-up that arrives in that nested `sendEvent` hits `releaseEarlyPress` (`MouseChainMac.swift:495`), which clears `early` and sends `pointerup`. The original mouse-down is still being dispatched, so `tracksPointer` then sees `early !== event`, sends a second `pointerdown`, and a second `pointerup` after its loop. One press becomes two. `tracked` cannot stop this: it is set only after the handler returns. Opening a dialog from `pointerdown` is the case the plan called out, and none of the new tests run a nested loop between the down and the up.

### [major] `early` is a weak event, so a press that never reaches `mouseDown` never releases — `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:226`

`releaseEarlyPress` treats "this press still needs its up" as `mouseChain.early != nil` (`MouseChainMac.swift:496`). `early` does not retain the event. After `sendEvent` returns, the run loop frees that `NSEvent` and the weak ref becomes nil. The up then does nothing, while `pointerHeld` and `pointerPath` stay set.

That is fine only when something else keeps the event alive. The disable path does, accidentally: `pressesUntracked` still calls `NodeView.mouseDown`, and `receivedDown` stores the event strongly (`MouseChainMac.swift:159`). It fails when the handler removes the editor from hit-testing before AppKit delivers the down — the page goes `inert` under a sheet, or the view is hidden — and the button is still down when that returns. Nothing calls `receivedDown`, nothing starts `tracksPointer`, and `pointerSource.window` is still set, so the monitor's "view left the window" release (`ExactViewMac.swift:332`) does not run either. The next press bails out in `pressEditorFirst` (`!pointerHolds`) and `pointerPressed` drops its down because `pointerHeld` is set; the stuck hold is released as that next press's up.

`testADownThatDisablesItsEditorStillReleases` posts the up in the same call, before the pool drains, so it never sees this. `Presenter.reset` (`PresenterMac.swift:599`) also clears the hold and does not clear `early`.

### [minor] A lent field has no `NodeView` ancestor, so it keeps focus-before-down — `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:484`

The walk assumes the owner is the nearest `NodeView`. That is true for a page text area, a page field, a page search field, and a password field (`SecureField.hitTest` returns the field, which is `node.field`). It is false for a field lent out of the node: a prompt's field lives in the alert accessory (`NativeFieldsMac.swift:208`), and a toolbar search field lives in the bar, outside `contentView` entirely. `keyTarget` already resolves those through the delegate (`KeyEvents.swift:339`). `pressEditorFirst` does not, so those inputs still focus before `pointerdown`. In-page password and search fields are fine, and inline atoms stay out: `TextArea.hitTest` returns the `AtomCatcher`, which is not `node.textArea`.

## What holds up

Hit-testing matches the hover path: the point is converted into the content view's superview, which is what `hitTest` expects, and `TableRowCell.whatIsThere` reveals the row's own view. An editor in a pressable row also `keepsClick`, so the view that gets the early down is the view AppKit later sends `mouseDown`. A covered view is the topmost hit. `dialogs.pointer` still returns before the early press, so a `closedby="any"` click behind a sheet is swallowed first. Sheets and popovers are in `Presenter.hosts`, and the monitor is the session's local monitor, so their windows are included. A window this session does not host is ignored.

The first-click check matches AppKit's rule, as measured in `FocusMac.swift:122`: send the down when the window is already key, otherwise only when the hit view's `acceptsFirstMouse` is true. `NSTextField` (plain, search, password) returns true; `NSTextView` returns false unless `everyClickCounts` is set, which `TextArea` and `FieldEditor` honor. Too strict would only restore the old order. Too loose would emit a down AppKit never tracks, which is the stuck-hold case above. A control-click is excluded up front. A field that forwards `mouseDown` to its editor is reported once, because the second `tracksPointer` sees `tracked === event`.

The order tests are real assertions (they failed on main with focus first). The covered-editor, once-per-press, sheet, popover, and agent-tap tests check a full sequence. The first-click and control-click tests allow an empty log, so they do not prove a release happened.

VERDICT: CHANGES
