# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `29f2860d1f`.
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

- **P2 — Contact cleanup remains incomplete** at [ExactViewMac.swift:336](/tmp/iosfix2-rv-pvf4-astra/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:336). `pointerPressed` sets `preventedContact` for every mouse button, but prevented right/middle clicks never clear it on their corresponding ups. Native buttons beneath a preventing ancestor also leave it set: their tracking loop consumes the left-button up, then calls `pointerReleased(nil)`, which does not clear the flag. Apple explicitly excludes tracking-loop events from [local event monitors](https://developer.apple.com/documentation/appkit/nsevent/addlocalmonitorforevents%28matching%3Ahandler%3A%29). A subsequent middle-button canvas press therefore incorrectly skips focus through `focusSurfacePointer()`. A left down outside hosted windows cannot reset it either because it returns before `pressEditorFirst`. Cleanup needs to cover these contact-completion paths. The new assertions exercise ordinary left clicks; they do not cover these cases.

No additional findings in the marked-text guard, immediate/deferred authored focus, or control-click fix, including inline-only context menus.

`git diff --check` passed. Native suites were not rerun: this read-only checkout has no build artifacts.

VERDICT: CHANGES
