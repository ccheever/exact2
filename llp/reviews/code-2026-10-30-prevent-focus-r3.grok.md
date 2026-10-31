# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `84a001e2c4`.
- **Method:** one brief (sha256 `e2eaa09006f4f0330e52de0e5dc56dc1208a2b32c2c8fea87bb60f749476bc8d`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 29f2860d1f: the contact flag cleared at its up, no iOS commit while composing, Presenter.authoredFocus lets an action's focus() land, a control-click's pre-focus kept; the iOS caret declared in the docs

## Brief

Code review, round 3 of 3 (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 84a001e2c4 is HEAD of this worktree; base 048bf305ca. Review the whole change fresh with `git diff 048bf305ca..HEAD`, and round 2's fixes with `git diff 6ea5eefea7..HEAD`.

Round 2 findings, and the fixes:
- **Popover and panel key (Astra P2, Grok P2):** a node that a pointerdown handler hears gets no pre-focus from AppKit. Its mouseDown now takes the click focus right after pointerPressed, before every branch (SVG target, canvas input, surface control, paragraph), through takeClickFocus(): first responder, plus makeKey on a non-key window that can become key (a popover's window, a key-only-if-needed panel), as AppKit's pre-focus did. Test (Mac): "hear", a focusable box whose pointerdown is heard and not prevented, takes the focus after its pointerdown, in that order.
- **SVG and surface returns (Astra P2, Grok P2):** covered by moving the focus decision before those branches (above). Not separately tested.
- **Canvas and surface focus on a prevented press (Astra P5, Grok P2):** MouseChain.preventedContact (set by a prevented pointerdown, cleared at each button down in pressEditorFirst) makes focusSurfacePointer keep the focus on the Mac; on iOS, keepsFocus(from: self). Not separately tested.
- **A prevented Mac drag extended an existing selection (Astra P3):** mouseDragged skips selection.drag for a prevented contact. Not separately tested.
- **iOS document surface (Astra P4, Grok P2):** interactionShouldBegin also refuses when the prevented touch is on the SelectionDocument's surface. Not separately tested (the test covers a single paragraph).
- **iOS prevented button press skipped the edit commit (Grok P2):** a prevented press now commits presenter.edited (change) without moving the focus, as the Mac commits on every press. Test: text typed into a, then a prevented tap on "plain"; a keeps the focus and change-a is heard.
- **Not changed:**
  - An already-focused iOS field's caret move under a prevented touch (Grok P2). UIKit places the caret through its own tap on a first responder, with no public hook short of disabling interaction; keeping the focus is what's tested, and the caret move is a named residual.
  - SelectableText.selectWord, the agent's path: unchanged.
Results: Mac (PreventFocus, EditorPointerOrder, EditorPointer, ClickFocus) 0 failed; iOS hosted 3/3; web (the JS target, mouse and touch) pass; caps green. The full suites on this tip are running; on 6ea5eefea7, Mac was 307/0, and iOS failed IframeCaptureTests' pinch-pan clip at load 245 (a QUEUE'd flake, unrelated).

Check hard, final round:
- the moved focus decision (any focus now given where AppKit gave none: inner focus, retainFocus, disabled, inert, a first click into an inactive window);
- makeKey on canMakeKey windows (sheets, alerts' windows, and the window of an ExactView embedded in someone's panel);
- preventedContact's lifetime (a down outside any hosted window, the agent's presses, nested loops);
- the iOS commit on a prevented press (double change, an IME composing);
- whether the residual (iOS caret move in an already-focused field) is fair to name rather than fix;
- the tests.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the full change from `048bf305ca` and the round-2 fixes from `6ea5eefea7` without editing anything. First I'm loading the review skill and the diffs.The review target is the committed range `048bf305ca..HEAD`, with round-2 fixes in `6ea5eefea7..HEAD`. I'll confirm the tip and collect both diffs before reading the focus paths.The round-2 delta is small. I'll read the focus, mouse, and iOS press paths around it so the checklist is checked against the code, not the commit notes.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is ready to close. I’ll confirm the line numbers on the two candidate defects, then give the verdict.[P2] A prevented iOS press commits text an input method is still composing — host/apple/Sources/ExactKit/IOS/PointerIOS.swift:635

The new `keepsFocus` branch commits `field.text` / `textArea.text` and fires `change`. That string includes the marked syllable (`FieldEditingIOS.swift:64`). Mac's `commitPendingEdit` returns while `hasMarkedText()` (`FocusMac.swift:198`), which is the commit the comment at `PointerIOS.swift:634` says it matches. A formatting-toolbar `preventDefault` during composition therefore reports `change` for the provisional text and clears `edited` (`PresenterIOS.swift:636`). A `change` handler that writes the value back stores it as `pendingValue` while the range is still marked (`FieldEditingIOS.swift:43`), and `fieldChanged` applies that over the confirmed candidate when the composition ends (`FieldEditingIOS.swift:71`). Confirmation also sets `edited` again, so `textFieldDidEndEditing` commits a second `change` (`FieldEditingIOS.swift:102`). A press with nothing marked still commits once: this return does not end editing. `PreventFocusIOSTests` only asserts that the heard list contains `"change-a"`. The older button commit at `PointerIOS.swift:673` has the same omission and predates this diff.

[P3] A control-click on a pointerdown-heard focusable node leaves the focus where it was — host/apple/Sources/ExactKit/Mac/MouseEventsMac.swift:215

`mouseDown` returns through `rightMouseDown` before `takeClickFocus`. `rightMouseDown` (`NodeViewMac.swift:1474`) sends `pointerdown` and the context menu and never takes click focus. `needsPanelToBecomeKey` is now false for these nodes (`FocusMac.swift:123`), so AppKit no longer focuses them before `mouseDown` either. A control-click whose `pointerdown` does not call `preventDefault` leaves the previous field focused, and the menu acts on that field.

The rest of the move matches the focus AppKit used to give. `takeClickFocus` runs only when the node is `pointerdownHeard`, `takesClickFocus`, the event was not prevented, and `focusInside` is false. `takesClickFocus` is already false for `retainFocus`, and `acceptsFirstResponder` is false for `formDisabled`, inert, hidden, fields, and text areas. An inert hit returns before the focus call. A plain first click into an inactive window still does not deliver `mouseDown` unless the view clicks through, so that click still focuses nothing. `makeKey` runs only for a window that actually received `mouseDown`, is not key, and `canBecomeKey`. A sheet's parent and an alert's other windows do not get that down; there is no `canBecomeKey` override. Making an embedded view's key-only-if-needed panel key is the old `needsPanelToBecomeKey` outcome. `makeFirstResponder` before `makeKey` is valid while the window is not key.

`preventedContact` is cleared at the start of every hosted left mouse down that reaches `pressEditorFirst` (`MouseChainMac.swift:512`), including control-clicks, holds, sheets, and non-editors, and it is set only at the end of `pointerPressed` (`MouseChainMac.swift:356`). A down outside a hosted window returns at `ExactViewMac.swift:341` and leaves the flag alone, which is the previous in-app contact. Agent taps go through `NSApp.sendEvent`, so they clear it. A nested down inside the `pointerdown` handler happens before the outer set, and the outer set wins. After mouse-up the flag stays true until the next hosted down. The one reader in that gap is an agent `type` onto a surface control (`AgentMac.swift:1005`): `focusSurfacePointer` (`SurfaceInput.swift:65`) treats the stale flag as success and leaves focus put, so the key goes to the previous responder when that responder is not an editable field. Mouse drags and canvas downs see the flag for the current contact.

The iOS caret residual is fair to leave. UIKit places the caret with its own tap on a view that is already first responder; `interactionShouldBegin` (`TextSelectionIOS.swift:343`) blocks a new selection, including a prevented touch on the document surface, and `becomeFirstResponder` is already refused. Keeping the focus is what the test covers. `docs/contract-grammar.md:1562` and `llp/1005-plan-and-runner-v1.spec.md:589` still say no caret is placed, with no mention of that already-focused field. Web `preventDefault` is limited to the `pointerdown` of the handler's own depth (`glue.js:771`, `rt.js:270`); a nested non-pointer run cannot prevent it. Linux, Android, and Windows remain declared as not done.

VERDICT: CHANGES
