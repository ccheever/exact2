# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `9d324a525b`.
- **Method:** one brief (sha256 `5c84281272feecd41717ca63bedba4fb9ef27f5cad7946af9f49e00c93eca8a9`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in f149659d14: a forwarded down keeps its contact prevented; keepingFocus restores on the chain it was said on

## Brief

Narrow re-check 3 of one Mac state (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 9d324a525b is HEAD of this worktree; base 048bf305ca; re-check 2 read 43734672e2. Review `git diff 43734672e2..HEAD`, in the context of `git diff 048bf305ca..HEAD`.

Re-check 2's findings: Astra P2, a second button's up cleared the left contact's flag (a later left drag then extended a selection); Grok P2 x2 and P3, an up the monitor discards (a closedby="any" sheet), an up taken inside the handler's own loop (beginSheet) before the flag was set, and the native button's double release, each left MouseChain.preventedContact stale for a later surface-control `type` or surface press (focusSurfacePointer).

The redesign, so no flag outlives what reads it:
- The surface focus no longer reads a contact flag. MouseChain.keepingFocus says whether the down being handled had its pointerdown prevented (MouseChain.prevented === event), set by the view's mouseDown, rightMouseDown and otherMouseDown right after pointerPressed, and restored to its outer value when that handler returns (defer; a nested loop's down restores the outer). focusSurfacePointer reads it: a canvas's or surface control's focus from a prevented down (any button) is kept; anything outside a down's handling (an agent `type`, a later press) sees false.
- preventedContact now guards only the left drag's selection. pointerPressed says it again at every left down, before its hold guard (so a stale hold cannot keep an old value), and sets it when that left down was prevented. Nothing clears it at an up: a left drag needs a left down first, which says again. pointerReleased's clear and the button scoping are gone.
Tests: after each prevented press (left on b, box, btn, the native button in a preventing row; right on box) keepingFocus is false; a left prevented down, then a right down and up, leaves preventedContact true (the left drag stays prevented), and a later unprevented left press on "free" makes it false. Mac PreventFocus/EditorPointerOrder/ClickFocus/NativeButtons/ReorderGroup 0 failed; caps green; full Mac and iOS suites on this tip are running.

Check: the keepingFocus scope covers every path from a down to focusSurfacePointer (the canvas down in mouseDown, rightMouseDown, otherMouseDown; the surface control's control("down") in mouseDown; a down that bubbles to a superview's handler); nothing reaches focusSurfacePointer inside a down's handling for another reason; preventedContact cannot be wrong for any left drag; nothing else regressed. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

- **[P2] Bubbling clears the left contact’s prevention** — [MouseChainMac.swift:336](/tmp/iosfix2-rv-pvf6-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:336). A down on an inline image dispatches its paragraph’s preventing handler, then forwards the same event to the paragraph’s `mouseDown`. That second `pointerPressed` clears `preventedContact` before the hold guard returns. A subsequent left drag reaches `selection.drag` and extends any existing selection, potentially moving focus. Preserve the prevention result when forwarding the same down.

- **[P2] Removing the view defeats scoped cleanup** — [NodeViewMac.swift:1483](/tmp/iosfix2-rv-pvf6-astra/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1483). A prevented right down sets `keepingFocus = true`, then dispatches `contextmenu`. If that action removes its node, `forget()` sets its `presenter` to nil. The deferred restoration consequently does nothing, leaving the surviving presenter’s flag true. Later out-of-handler `focusSurfacePointer` calls, including agent typing into a held surface control, incorrectly skip focus. Capture the original `MouseChain` for restoration in all three handlers.

Static review; tests were not rerun in the read-only workspace.

VERDICT: CHANGES
