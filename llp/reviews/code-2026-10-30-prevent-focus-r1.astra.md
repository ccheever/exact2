# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `0bb6481812`.
- **Method:** one brief (sha256 `f57d7f82557d4c97284fad61ca5d8a0ddbc4a88610077275d547b6156782a168`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** replaced: the widened early dispatch and the currentEvent read are reverted; AppKit's pre-focus is declined through needsPanelToBecomeKey plus pointerdownHeard (6ea5eefea7)

## Brief

Code review, round 1 of 3 (contract semantics plus web glue, macOS and iOS hosts). Tip 0bb6481812 is HEAD of this worktree; base 048bf305ca. Review `git diff 048bf305ca..HEAD`: 6 commits, the red tests first per host.

Approved plan (theme2, 2026-10-30 17:01Z): ~/x2theme/iosfix2-pointerdown-prevent-focus-plan.md, if readable.
Definition, measured in Chrome 156 by real CDP mouse and touch input: an action handling `pointerdown` that calls `preventDefault()` keeps the focus where it was, places no caret and starts no selection; the click (`press`) still fires. Chrome does this for a prevented pointerdown, not only a prevented mousedown.

The change:
- **Docs:** docs/contract-grammar.md (Events) and LLP 1005 §3 say it for every host. The shared Linux presenter (Linux, Android, Windows) is DECLARED not yet conforming, owned by rt1 (the lead routed it, theme2 17:02).
- **Web:** host/web-js/rt.js (the JS target, Hosts.preventDefault) and host/web/glue.js (the wasm web host) now also call preventDefault on the DOM pointerdown a handler is running for: `Propagating` / `pointerEvent`, the same events stopPropagation already reaches.
- **macOS:**
  - pointerPressed resets presenter.defaultPrevented before the bubbling walk and records `mouseChain.prevented = event` if a handler prevented it.
  - pressEditorFirst (the session window's monitor) now also sends the early down for a hit NodeView that takes the click focus, because AppKit makes such a view first responder before its mouseDown. It returns true for a prevented editor down, and the monitor then keeps the leftMouseDown from NSWindow; the early release path sends the pointerup at the button's up.
  - takesClickFocus answers false while NSApp.currentEvent is the prevented press, so NSWindow moves no focus.
  - NodeView.mouseDown skips its makeFirstResponder and a paragraph's selection begin for a prevented press; the press goes on.
- **iOS:**
  - PointerRecognizer resets defaultPrevented around the down dispatch and records presenter.preventedTouch, cleared a turn after the lift.
  - TextField, TextArea and NodeView becomeFirstResponder decline while keepsFocus(from:) holds (the view the touch went down on, or one it's in).
  - Accepted edge: an action's own focus() of that same view during that touch is declined too.
- **Fixture:** scripts/fixtures/prevent-focus.contract (in compileFixtures).
- **Tests:**
  - host/web/tests/prevent-focus.test.mjs (JS target; real CDP mouse and touch);
  - PreventFocusMacTests (NSApp.sendEvent presses);
  - PreventFocusIOSTests (hosted; UITouches fed to the PointerRecognizer and the node, with the field's own focus asked directly, as UIKit's tap would).
  Each fails on main; for iOS, without the becomeFirstResponder vetoes. EditorPointerOrderMacTests, EditorPointerMacTests and ClickFocusMacTests stay green. The wasm web host (glue.js) has no browser test: its build needs an app crate and Hermes, and the line mirrors rt.js's. A tvOS compile of ExactKit (xcodebuild, generic tvOS Simulator) succeeds. The full Mac and iOS suites are running; caps is green.

Check hard:
- **Mac, the widened early down for click-focus nodes:** any change in what they do when not prevented (focus order, press, double clicks, first clicks into inactive windows, nested loops, the ExactView monitor across sessions);
- the takesClickFocus override reading NSApp.currentEvent (agent taps, synthesized events, and the needsPanelToBecomeKey consumers);
- the editor swallow path (the up's routing when the window never had the down);
- **iOS:** the preventedTouch lifetime, multiple touches, a touch cancelled, the async clear racing a quick second tap;
- **web:** preventDefault on a pointerdown that a recognizer (pan, swipe) also handles; an inner handler preventing while an outer doesn't;
- the docs' accuracy;
- whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

1. **[P1] Early macOS dispatch can redirect an unprevented click.** [MouseChainMac.swift:516](/tmp/iosfix2-rv-pvf1-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:516) now runs a focusable node’s `pointerdown` before NSWindow chooses its mouse-down recipient. If that handler moves or hides the node, the monitor returns the event and NSWindow hits the newly exposed control. That control can receive the down and fire its `press`. Previously, the handler ran after AppKit had captured the original recipient. Preserve that recipient across early dispatch; add a test with a focusable overlay that moves during an unprevented down.

2. **[P2] The macOS focus veto misses synthesized events.** [FocusMac.swift:109](/tmp/iosfix2-rv-pvf1-astra/host/apple/Sources/ExactKit/Mac/FocusMac.swift:109) compares against `NSApp.currentEvent`, which is the last event retrieved from the queue. The agent sends its down directly at `AgentMac.swift:866`. An AppKit probe confirmed that `currentEvent` differs from that down during `needsPanelToBecomeKey`, so AppKit can move focus before `NodeView.mouseDown` checks prevention. The new test masks this by dequeuing its down first. Track the event being dispatched explicitly. [Apple documentation](https://developer.apple.com/documentation/appkit/nsapplication/currentevent)

3. **[P1] A prevented iOS background tap still blurs the editor.** [NodeViewIOS.swift:382](/tmp/iosfix2-rv-pvf1-astra/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:382) only blocks acquiring first responder. With an input focused, tap a nonfocusable box whose `pointerdown` prevents default: `touchesEnded` still calls `focusForPress`, which reaches `presenter.endEditing()` at `PointerIOS.swift:670`. No `becomeFirstResponder` call occurs, so the veto cannot retain focus. Apply prevention to the default focus decision, including resignation. The fixture’s focusable box misses this case.

4. **[P2] iOS selection remains enabled for prevented contacts.** [PointerIOS.swift:115](/tmp/iosfix2-rv-pvf1-astra/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:115) records prevention, but its consumers only veto responder acquisition on three view classes. Selectable paragraphs use a separate `SelectableText` responder: `interactionShouldBegin` still returns true, and its responder eligibility ignores prevention (`TextSelectionIOS.swift:329–345`). A prevented long press can therefore select text and take focus, contrary to the new definition. Gate that interaction and test actual selection; directly asking a field to become first responder does not establish this behavior.

Native suites were not rerun: this checkout is read-only and has no build artifacts. No files were edited.

VERDICT: CHANGES
