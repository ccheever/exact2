# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `0bb6481812`.
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

I'll review the contract semantics and the web, macOS, and iOS host changes against the approved plan, starting with the diff and the red tests.This is a direct review of `048bf305ca..HEAD` — contract semantics, web glue, and the macOS and iOS hosts. I'll read the plan and the diff before judging the tests.The Mac early-down path is wider than editors now. I'll read that walk, the focus veto, and the iOS touch lifetime against the surrounding code.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open on a few call paths. I'll confirm those against the diff, then give findings with line numbers and a verdict.[P2] Record each prevented touch until that touch lifts — host/apple/Sources/ExactKit/IOS/PointerIOS.swift:115

A presenter keeps a single `preventedTouch`. Each pointer node has its own recognizer, and a prevented `pointerdown` assigns that shared slot (`PointerIOS.swift:115`). A second finger whose `pointerdown` also prevents replaces it. `keepsFocus(from:)` (`PointerIOS.swift:696`) then protects only the second touch's view, so the first view can still take focus: a field through `TextField.becomeFirstResponder` (`TextAreaIOS.swift:111`) or a node through `NodeView.becomeFirstResponder` (`NodeViewIOS.swift:382`). `lift` (`PointerIOS.swift:191`) clears the slot only when the lifting touch is the one stored, and UIKit delivers one touch to each view with `multipleTouchEnabled` left at its default. The contract is per press: the first finger's prevented down no longer keeps focus.

The web glue matches the Chrome rule. `rt.js` and `glue.js` call `preventDefault` on the DOM `pointerdown` the handler is running for; an inner prevent sticks for the outer handler and for the browser, and a pan or swipe listener still runs. The grammar states the rule for every host, and LLP 1005 §3 names the shared Linux presenter (Linux, Android, Windows) as not yet conforming, owned by rt1. On macOS, a click-focus node gets its down before AppKit moves focus, and a prevented editor down is kept from the window; the button's up still sends `pointerup`. An unprevented click-focus press still focuses, and `mouseUp` still fires `press` after the monitor's early `pointerup`. `takesClickFocus` is read before this press is recorded, and `mouseDown` keys off the event it was given. A touch cancel uses the same lift as a finger up. The clear is one main-queue turn after the lift, so a later tap is not held out by the previous one.

`PreventFocusIOSTests` asks the field and the text area to focus while that one touch is still down. It does not put two touches down, cancel a touch, or ask again after the lift and before `settle()`. The box path does call `focusForPress` from `touchesEnded` while the flag is still set. `glue.js` has no browser test; the line mirrors `rt.js`.

VERDICT: CHANGES
