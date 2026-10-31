# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `29f2860d1f`.
- **Method:** one brief (sha256 `8f4b96b37d67c7b1ea411fd17063db04e2184c83c0f71f6ebb131e19e39ffa8d`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 43734672e2: the flag's lifetime is the pointer hold (cleared in pointerPressed and on every pointerReleased path)

## Brief

Narrow re-check after round 3 of 3 (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 29f2860d1f is HEAD of this worktree; base 048bf305ca; round 3 reviewed 84a001e2c4. Review the round-3 fixes with `git diff 84a001e2c4..HEAD`, in the context of `git diff 048bf305ca..HEAD`.

Round 3 findings, and the fixes:
- **Mac preventedContact outlived its contact (Astra, Grok):** the ExactView monitor clears it at the left button's up (after the early release) and at a right button's down, so a later agent `type` or surface press no longer reads a stale flag. Test: after each prevented press, preventedContact is false.
- **iOS commit on a prevented press published marked (IME) text (Astra):** the commit is skipped while the edited field has a markedTextRange, as the Mac waits for composition. Test: insertText, setMarkedText, a prevented tap commits nothing; after unmarkText, the next prevented tap commits once (exactly one change-a).
- **iOS declined an action's explicit focus() of the touched view during the touch (Grok):** Presenter.authoredFocus is set around the private focus(_:selectText:) (which focusElement, focusNode and the deferred flush all go through); keepsFocus(from:) answers false while it is set. Test: during a prevented touch on b, presenter.focusNode(b) makes b first responder and focus-b is heard.
- **Mac control-click on a pointerdown-heard focusable node lost its pre-focus (Grok):** before the context-click branch, mouseDown takes the click focus as AppKit's pre-focus did (pointerdownHeard, takesClickFocus, not inside). Test: fixture node "ctx" (tabindex, pointerdown heard, contextmenu); a control-click hears press-ctx and focus-ctx. (The Mac test window grew to 480 tall so ctx is on screen.)
- **Docs:** contract-grammar and LLP 1005 §3 now say an action's own focus() still moves the focus, and declare that on iOS a field that already has the focus still places its caret where the touch lifts (UIKit's own text gesture on its first responder); the focus stays and no selection starts.
Each new test was red with its fix reverted and green with it: Mac PreventFocus/EditorPointerOrder/ClickFocus 0 failed; iOS hosted PreventFocusIOSTests 4/4; web test pass; caps green. Full Mac and iOS suites on this tip are running.

Check: each fix is correct and complete (the monitor sees every up the flag needs: a down outside hosted windows, nested tracking loops, the agent's presses; authoredFocus with a deferred/pending focus; markedTextRange on the text area; the control-click focus when a contextmenu is inline-only); nothing else regressed; the tests prove what they claim. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the round-3 fixes against the full branch diff, checking each claimed fix and whether the tests actually prove it. No files will be edited.Round 3 is one commit on top of `84a001e2c4`. I'll read that delta against the surrounding focus and pointer code, then check whether the new tests actually cover the cases they name.The round-3 diff is small. Next I’ll read the pointer, focus, and monitor paths those four fixes touch, including the cases the tests may not cover.The round-3 fixes for an action's `focus()`, iOS composition, and a control-click's pre-focus are in place. The Mac contact flag still outlives some contacts, so a later surface press can read it.

What holds:

- An up outside a hosted window still clears the flag. The clear in the session monitor runs before the `hosts()` check, so a `leftMouseUp` or `rightMouseDown` that `sendEvent` delivers is cleared even when the window is not this session's.
- An agent press whose up is `sendEvent`'d (a box, the fixture `button`) is seen by that monitor. `PreventFocusMacTests` only asserts the flag after `press()`, which is that path.
- `authoredFocus` wraps `focus(_:selectText:)`, and the deferred flush calls that same function, so a pending focus still becomes first responder while the flag is set.
- The iOS commit skips while `field.markedTextRange` or `textArea.markedTextRange` is set. The new test covers the input; the text area is the same check.
- A control-click focuses before `rightMouseDown` whenever `contextClick` is true. That includes an inline-only `contextmenu` (`inlineContextTarget`), and the focus predicate does not require the node's own menu.

## Findings

### 1. Bug — a prevented right or middle click leaves `preventedContact` set

`host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:336`

`pointerPressed` sets the flag for every button (`MouseChainMac.swift:356`), including `rightMouseDown` (`NodeViewMac.swift:1480`) and `otherMouseDown` (`MouseEventsMac.swift:485`). The monitor clears only on `leftMouseUp` and `rightMouseDown`. `rightMouseUp` and `otherMouseUp` are in the mask (`ExactViewMac.swift:326`) and do not clear it. `otherMouseDown` is not in the mask at all.

A `pointerdown` handler that calls `preventDefault()` with no button check (the fixture's `keep`) therefore sets the flag on a right or middle click, the monitor sees the up, and the flag stays true. The next agent surface press or surface-control `type` hits `focusSurfacePointer` (`SurfaceInput.swift:65`) and keeps the focus where it is. The Mac test never clicks those buttons, so it stays green.

### 2. Bug — a left-button up taken by a tracking loop never reaches the monitor

`host/apple/Sources/ExactKit/Mac/NativeButtonsMac.swift:149`

A local monitor runs only for `NSApplication.sendEvent`. A tracking loop takes the up with `nextEvent(matching:)` and the monitor does not see it (same fact as the popover light-dismiss fix). Two paths set the flag and then enter such a loop:

- A native button calls `pointerPressed` and then `super.mouseDown`, which tracks (`NativeButtonsMac.swift:149-157`). The agent's tap posts that up for the loop and does not `sendEvent` it if the loop already took it (`AgentMac.swift:862-874`).
- `tracksPointer` calls `pointerPressed` and then `track()` (`MouseChainMac.swift:376-378`). That is the editor when `pressEditorFirst` did not swallow the down (already tracking, control-click, window not eligible), and any `pointerdown` handler that runs a menu, alert, or sheet loop.

A prevented editor down that the monitor swallows is fine: no loop starts, and the later up is `sendEvent`'d and cleared. The fixture `button` is a plain node, not a native button, and `press()` `sendEvent`s its up, so `PreventFocusMacTests.swift:61` does not catch either hole. Afterward the flag is still what a surface press reads.

VERDICT: CHANGES
