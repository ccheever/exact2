# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `6ea5eefea7`.
- **Method:** one brief (sha256 `fd79f9cff21d6a6894504a41cdf61b48280c547b0a68bb3df16fb947929e9b0c`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 84a001e2c4: the click focus taken after pointerPressed before every branch (popover and panel key), preventedContact for surfaces and drags, the iOS document surface, the iOS commit on a prevented press; the iOS caret in an already-focused field named

## Brief

Code review, round 2 of 3 (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 6ea5eefea7 is HEAD of this worktree; base 048bf305ca. Review the whole change with `git diff 048bf305ca..HEAD`, and round 1's fixes with `git diff 0bb6481812..HEAD`.

Round 1 (Astra and Grok both CHANGES), and the fixes:
- **Astra P1 (Mac):** the widened early dispatch for click-focus nodes could let a handler move the node before NSWindow chose the recipient. Reverted; pressEditorFirst is editors-only again. Instead, needsPanelToBecomeKey (AppKit's "focus it before mouseDown") is false for a node that a pointerdown handler hears (its own, or an enabled authored ancestor's: pointerdownHeard). Its mouseDown then gives it the focus after pointerPressed, unless prevented, through the existing keepsFocus check. Nodes no pointerdown handler hears keep today's pre-focus exactly.
- **Astra P2 (Mac):** the NSApp.currentEvent test missed synthesized events. Gone, with the takesClickFocus override. Test: an agent tap on the prevented box keeps a's focus.
- **Astra P1 (iOS):** a prevented tap on a non-focusable box still ended the editing (focusForPress, endEditing). focusForPress now returns before its focus decision when keepsFocus(from: self). Test: a "plain" box that prevents; a stays first responder and its press fires. That fails without the guard.
- **Astra P2 (iOS):** selection under a prevented touch. SelectableText.interactionShouldBegin is false while keepsFocus(from: node). Test: a selectable paragraph ("para", user-select text) can begin selection before the touch, can't during the prevented touch, and can again after the lift. That fails without the guard.
- **Grok P2 (iOS):** one preventedTouch slot couldn't hold two fingers. It's now preventedTouches, a weak NSHashTable; each lift removes its own touch a turn later, and keepsFocus checks every touch. Test: two fingers on b and t, both prevented; neither view becomes first responder.
- **The iOS suite's failure on 0bb6481812:** the test helper called missing("scene"), which xctest.mjs requires inside the test method so the test runs hosted. Each test now calls it.
- **Fixture:** adds "free" (a focusable box no pointerdown hears: still focuses on the Mac), "plain" and "para".
- **LLP 1005** text updated to match.
Web (the JS target) passes, by mouse and by touch; Mac 4 classes 0 failed; iOS hosted 3/3. The full suites on this tip are running. Caps is green; tvOS compile succeeded at 0bb6481812.

Check hard:
- needsPanelToBecomeKey returning false for pointerdown-hearing nodes: any loss of focus for an unprevented click? Think of a first click into an inactive window, panels and popover windows, a node inside a canvas, and table rows.
- The authored-ancestor walk's cost and correctness.
- The iOS hooks: focusForPress's early return and what PressHandoff then does; SelectableText's other entry points (long press, the edit menu); the NSHashTable lifetime under cancellation.
- Whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the full change from `048bf305ca` to HEAD, then the round-1 fixes from `0bb6481812`, and check the focus and touch cases you called out. No files will be edited.Reviewing this clean worktree against `048bf305ca`, with the round-1 delta from `0bb6481812` checked separately. I'll read the diffs and the surrounding focus and touch paths before judging.The round-1 delta is small. Next I’ll read the Mac focus path and the iOS press handoff around the lines that changed.Auto-compacting conversation (81% full)...
Conversation compacted.
The round-1 diagnosis is in place. I'll confirm the cited lines in the focus, press, and selection paths, then write the review.Round 2 of the pointerdown `preventDefault()` work (`048bf305ca..6ea5eefea7`, with the round-1 delta `0bb6481812..HEAD`). The reverted early dispatch, the synthesized-event path, the plain-box `focusForPress` guard, the two-touch table, and `missing("scene")` inside each test method all hold. The holes below are in the cases those tests do not drive.

## Findings

**P2. An unprevented click on a click-focus node that a `pointerdown` handler hears leaves an `NSPopover` or `becomesKeyOnlyIfNeeded` panel non-key.** `FocusMac.swift:123` answers `needsPanelToBecomeKey` with `takesClickFocus && !pointerdownHeard`. AppKit makes the panel key from that flag before `mouseDown`. The replacement focus is `window?.makeFirstResponder(self)` at `MouseEventsMac.swift:236`, which sets the first responder and does not make the window key. `pointerdownHeard` (`FocusMac.swift:126-131`) walks `eventParent` (`KeyEvents.swift:137`). For the popover root, `MenusMac.swift:184-186` returns the page parent, so a `pointerdown` on a page ancestor defers every click-focus descendant in the popover. Popovers are `.transient` or `.applicationDefined` (`PopoversMac.swift:222`). Native fields keep their own answer and still key the panel. In an already-key window, that same unprevented click still focuses at `MouseEventsMac.swift:236`. A first click into an inactive ordinary `NSWindow` is unchanged: a plain box’s `acceptsFirstMouse` is false (`NodeViewMac.swift:1460`), and that click never reached `mouseDown` before either.

**P2. Two `mouseDown` returns drop that same unprevented click focus once AppKit no longer pre-focuses.** `MouseEventsMac.swift:219` returns for a surface control, and `:226` returns when the hit is an SVG element, both before `:236`. A non-button surface control (`SurfaceInput.swift:54`) with a `tabindex` or its own key handlers, and an SVG that takes click focus and is hit on an element, used to be focused by AppKit before `mouseDown`. They no longer are when `pointerdownHeard` is true, prevented or not. A pointerdown-only SVG never accepted first responder (`FocusMac.swift:16-19`). Paragraphs and a plain box still focus when the press was not prevented (`MouseEventsMac.swift:229`, `:232`, `:236`).

**P2. A prevented press still focuses a canvas when no editor is first responder.** `MouseEventsMac.swift:220` returns as soon as `canvasInput.pointer(..., "down")` returns true, so `:236` never runs. That down calls `focusSurfacePointer()` (`CanvasInputMac.swift:136-138`), which makes the canvas first responder whenever the current first responder is not an editable `NSTextView` (`SurfaceInput.swift:62-65`). `mouseChain.prevented` is not consulted. An unprevented canvas click still focuses. A prevented one on the canvas, or on an ancestor the canvas’s press hears, focuses it too.

**P2. A prevented button press on iOS no longer commits the in-progress edit.** `PointerIOS.swift:634` returns the handoff before the button branch at `:661-668`, which is what called `commitEdit` for the live field. The press still fires, and leaving `handoff.control` nil is the right keep-focus result. Mac still commits on every press, including one that keeps the editor (`PresenterMac.swift:825`). `PresenterIOS.swift:617` does not. A prevented tap on a button while a field is mid-edit now skips `change`.

**P2. The iOS selection guard looks at the paragraph while a multi-paragraph touch’s view is the selection surface.** `TextSelectionIOS.swift:343-345` refuses the interaction when `node.keepsFocus(from: node)`. For more than one selectable paragraph, `install` builds a `SelectionDocument` (`TextSelectionIOS.swift:63-69`) and `NodeViewIOS.swift:955-958` hit-tests the paragraph to that document’s surface, which lives in the owner’s container (`TextSelectionDocumentIOS.swift:57`). `preventedTouches` stores that `UITouch` (`PointerIOS.swift:115`). `keepsFocus` (`PointerIOS.swift:697-698`) matches the touch’s view or a descendant of the view it is asked about. The surface is neither the paragraph nor inside it, so the guard allows the interaction. The same delegate is what a long press and the edit menu ask. `textInputView` is the surface when a document exists (`TextSelectionIOS.swift:471`), and `keepsFocus(from: textInputView)` would see the touch. `selectWord` (`TextSelectionIOS.swift:295`) also becomes first responder with no `keepsFocus` check. That path is the agent’s. The test at `PreventFocusIOSTests.swift:107-116` installs `"para"` alone, so there is no document, `touch.view` is the paragraph, and the assertion passes.

**P2. An already-focused iOS field still moves its caret under a prevented touch.** `TextAreaIOS.swift:110` and `:306` refuse `becomeFirstResponder`. A field that is already first responder places the caret through `selectedTextRange` on the touch, which those overrides do not see. Mac swallows that down in the window monitor (`ExactViewMac.swift:362-364`): `pressEditorFirst` consumes the event, so the editor never receives it. The iOS tests call `becomeFirstResponder` on a field that does not yet have the focus.

## Hard checks that held

The ancestor walk is one `eventParent` pass per `needsPanelToBecomeKey` query, the same authored chain `pointerPressed` already walks. A disabled node is skipped and the walk continues. `stopPropagation` only makes the deferral wider. In an already-key window, `mouseDown` still focuses when it reaches line 236. That is not a cost or a correctness problem.

Table rows are unchanged. A list row clicks through, and row selection is the window monitor’s `tables.pointer` (`ExactViewMac.swift:358`). A tabindex row that reaches `MouseEventsMac.swift:236` still focuses.

`preventedTouches` is a weak `NSHashTable` (`PresenterIOS.swift:382`). `touchesCancelled` calls `lift` (`PointerIOS.swift:183-191`), which removes that touch a turn later and keeps the object alive across the turn. The table dies with the presenter. One recognizer still tracking a single touch is the previous limit.

After the early return, `PressHandoff` does not adopt the live editor, so a prevented button does not hand focus to an autofocus its press mounts. That matches keeping the focus where it is. Blocking an action’s `focus()` of the touched view and its ancestors for the rest of the touch is what `PointerIOS.swift:692-698` documents.

Web glue stays on `pointerdown` (the command path, and the `retainFocus` listener). Nothing in that glue contradicts the contract. You already have the JS target green, by mouse and by touch.

The tests prove the claims they actually make: the plain box goes through `touchesEnded` into `focusForPress` and fails without the guard; two fingers on `b` and `t` both stay out of the first responder; an agent tap on the prevented Mac box keeps `a`’s focus; `"free"` still takes AppKit’s pre-focus because no `pointerdown` hears it. They do not cover an unprevented click-focus node that does hear `pointerdown` (popover key, SVG element, surface control), a prevented canvas click, a prevented button during an open edit, a multi-paragraph selection document, or a caret move inside an already-focused editor.

VERDICT: CHANGES
