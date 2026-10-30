# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `d12e915ec5`.
- **Method:** one brief (sha256 `e94bed2088e7382e6b4b54f5b2874bb576c0676955690ef5d2bd50875fed2eba`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the run stopped at a model-capacity error ("Selected model is at capacity") before its final message; below are its interim notes, in order, and the error, unedited.
- **Verdict:** none given. Its interim findings (lent fields keep the old order, a light-dismiss sheet takes the up, the first-click test can pass empty) were each fixed in round 1.
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

I’ll read the repository rules, inspect the diff, and trace the event paths and tests without changing files.

The tests check ordering and event counts for ordinary editor clicks, but the inactive-window test can pass without any pointer events. I’m tracing that case and whether a handler that changes the editor or opens another window can leave the early press unpaired.

The monitor is scoped to the session’s hosted windows, and the dialog-dismissal check still runs before the early press. The remaining questions are press ownership after callbacks run and AppKit’s first-click behavior for text views and field editors.

I found a concrete gap: if an editor’s `pointerdown` opens a `closedby="any"` sheet, the sheet can consume the matching mouse-up before `releaseEarlyPress` runs. I’m checking the effect on the next click and whether the weak event reference creates a separate release gap.

The ownership walk also misses fields that Exact lends to AppKit: toolbar search fields and alert prompt fields have a node delegate but no `NodeView` ancestor. Those paths retain the old ordering. I’m finishing with those findings and the vacuous test assertions; I couldn’t rerun the suite in this read-only worktree, which has no build artifacts.
ERROR: Selected model is at capacity. Please try a different model.
ERROR: Selected model is at capacity. Please try a different model.
