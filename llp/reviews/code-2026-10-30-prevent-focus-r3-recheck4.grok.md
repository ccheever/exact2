# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 4 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `f149659d14`.
- **Method:** one brief (sha256 `49ae4e283f1050339e748c42da3a1500a318ce8f9cc8e908a5f0665a6e0fbb61`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve.
- **Disposition:** none needed (both APPROVE)

## Brief

Narrow re-check 4 of one Mac state (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip f149659d14 is HEAD of this worktree; base 048bf305ca; re-check 3 read 9d324a525b. Review `git diff 9d324a525b..HEAD`, in the context of `git diff 048bf305ca..HEAD`.

Re-check 3's findings, and the fixes:
- **A forwarded left down cleared preventedContact (Astra P2, Grok P2):** a non-pressable hit (an inline atom, a plain box in a paragraph) forwards the same down to its superview's mouseDown, whose pointerPressed cleared the flag before its hold guard returned; a later left drag then extended the selection and moved the focus. pointerPressed now says the flag again only for a left down that is not the one already prevented (`mouseChain.prevented !== event`). Test: a prevented down on "box", then its parent's pointerPressed with the same event: preventedContact stays true (red before the fix).
- **A removed node defeated keepingFocus's restore (Astra P2):** a contextmenu action removing its node nils its presenter, so the deferred restore did nothing. NodeView.keepingFocus(for:) captures the MouseChain, says the value, and returns a closure that restores the outer value on that chain; mouseDown, rightMouseDown and otherMouseDown use it with defer. Not separately tested (a structural capture).
Mac PreventFocus/EditorPointerOrder/ClickFocus/NativeButtons/ReorderGroup 0 failed; caps green; full Mac and iOS suites on this tip are running.

Check: both fixes are correct and complete; nothing else regressed. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check the two fixes against the delta since re-check 3 and the full change since the base, without editing anything.The delta is four files. I'll read the pointer, focus, and test paths those fixes sit on to see whether the flag and the restore actually hold.Both fixes hold. The delta is the four Mac files since `9d324a525b`; nothing in it reopens the contract, the web glue, or iOS.

**Forwarded left down.** `pointerPressed` still clears `preventedContact` only on a left down, and only when that down is not the event already stored in `prevented`. The flag is set at the end of the same call, after the path is recorded, and the forward happens later, from `mouseDown`, with the same `NSEvent`. The parent's second `pointerPressed` therefore skips the clear and returns on the hold guard, so the contact stays prevented. A later, different left down still clears it. `mouseDragged` is the only reader of the flag, so with it left true the drag does not call `selection.drag` (the path that both extends the selection and moves the focus). Nested `mouseDown` restores `keepingFocus` in stack order around that forward.

**Removed node.** `keepingFocus(for:)` reads `MouseChain` (a class the presenter owns) while the node still has a presenter, stores the outer `Bool`, and returns a closure that writes that value back onto the captured chain. `mouseDown`, `rightMouseDown`, and `otherMouseDown` are the only setters, and each runs the closure in `defer`, including the `contextmenu` path that nils `presenter` after the flag is said. There is no leftover restore through `presenter`. A nested down saves and restores the outer value around its own. No separate test is needed for that capture; the forwarded-down test calls the real second `pointerPressed` with the same event.

No findings.

VERDICT: APPROVE
