# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `6ea5eefea7`.
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

1. **P2 — Unprevented clicks can no longer activate a panel.** [FocusMac.swift:123](/tmp/iosfix2-rv-pvf2-astra/host/apple/Sources/ExactKit/Mac/FocusMac.swift:123)  
   In an `NSPanel` with `becomesKeyOnlyIfNeeded = true`, a focusable node now declines panel activation merely because a `pointerdown` handler exists. Even when that handler allows the default, the fallback only calls `makeFirstResponder`; it never makes the panel key, so keyboard input stays elsewhere. This property controls both behaviors ([Apple documentation](https://developer.apple.com/documentation/appkit/nsview/needspaneltobecomekey)). Neither `free` nor `c` tests an unpreventing handler.

2. **P2 — Unprevented SVG clicks can lose focus.** [FocusMac.swift:123](/tmp/iosfix2-rv-pvf2-astra/host/apple/Sources/ExactKit/Mac/FocusMac.swift:123)  
   A focusable SVG containing a pressable shape loses AppKit’s automatic focus when it or an ancestor hears `pointerdown`. Its `mouseDown` then returns at `svg.target` in `MouseEventsMac.swift:226`, before the replacement focus decision. Adding a handler that never prevents therefore changes whether clicking the shape focuses the SVG.

3. **P2 — A prevented Mac drag still extends an existing selection.** [MouseEventsMac.swift:232](/tmp/iosfix2-rv-pvf2-astra/host/apple/Sources/ExactKit/Mac/MouseEventsMac.swift:232)  
   Skipping `selection.press` suppresses only the down. Subsequent `mouseDragged` still calls `selection.drag`, which accepts an existing anchor, focuses it and extends its range. Select some text, then drag on a paragraph whose down prevents: the old selection changes. Prevention needs to govern the whole contact. The new Mac tests exercise no selection drag.

4. **P2 — Shared iOS selection surfaces bypass the guard.** [TextSelectionIOS.swift:345](/tmp/iosfix2-rv-pvf2-astra/host/apple/Sources/ExactKit/IOS/TextSelectionIOS.swift:345)  
   Multi-paragraph selectable containers install the interaction on `SelectionDocument.surface`, a sibling of their paragraphs. The prevented touch’s `view` is that surface, but this guard checks whether it descends from `SelectableText.node`, a paragraph. It therefore permits selection despite prevention. Check the actual interaction surface or resolved touch source. The single-paragraph `Touch(para)` test cannot expose this.

5. **P2 — Mac canvas controls still move focus after prevention.** [MouseEventsMac.swift:218](/tmp/iosfix2-rv-pvf2-astra/host/apple/Sources/ExactKit/Mac/MouseEventsMac.swift:218)  
   Both early canvas branches ignore `keepsFocus`. `SurfaceGlue.control` and `CanvasInputMac.pointer` call `focusSurfacePointer`, which can call `makeFirstResponder` directly. With a non-editor node focused, a prevented click on a canvas or surface control still moves focus. Preserve input delivery while suppressing these focus calls; the current editor-focused fixture misses this.

Static review only; I did not rerun suites in this read-only worktree. The iOS tests directly invoke focus and delegate methods rather than exercising native selection gestures.

VERDICT: CHANGES
