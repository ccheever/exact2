# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `9d324a525b`.
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

I'll re-check the redesign against the earlier findings: whether `keepingFocus` covers every path from a mouse-down to `focusSurfacePointer`, and whether `preventedContact` can still be wrong for a left drag. No files will be edited.The tip matches `9d324a525b`. Next I’ll trace every path from a mouse-down into `focusSurfacePointer`, and every read and write of `preventedContact`.The flag is set after `pointerPressed`, so I’m checking whether a nested loop or a bubbled down can still reach `focusSurfacePointer` outside that window.The focus flag now lives only for the down on the stack, so the old stale-contact failures are gone. One left-drag hole is still open.

**P2.** A forwarded left down clears `preventedContact` and never puts it back, so that contact's drag extends the selection and moves the focus.

`pointerPressed` clears the flag before the hold guard, and sets it only if the guard passes:

```336:337:host/apple/Sources/ExactKit/Mac/MouseChainMac.swift
        if event?.type == .leftMouseDown { presenter?.mouseChain.preventedContact = false }
        guard let presenter, presenter.pointerHeld == nil, presenter.pointerPath.isEmpty else { return }
```

```361:364:host/apple/Sources/ExactKit/Mac/MouseChainMac.swift
        if presenter.defaultPrevented, let event {
            presenter.mouseChain.prevented = event
            if event.type == .leftMouseDown { presenter.mouseChain.preventedContact = true }
        }
```

The hit's `mouseDown` runs that fully (the handler prevents, the flag becomes true), then a non-pressable node forwards the same left event:

```244:244:host/apple/Sources/ExactKit/Mac/MouseEventsMac.swift
        if pressable { pressed = true } else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseDown(with: event) }
```

The superview's `pointerPressed` clears the flag and returns, because the hit already holds the pointer. A selectable paragraph ancestor is in that position whenever the hit is a non-selecting descendant (an image or a plain box; both are real children of a text node). Its drag then reads the cleared flag:

```287:287:host/apple/Sources/ExactKit/Mac/MouseEventsMac.swift
        if selectsText, presenter?.mouseChain.preventedContact != true { presenter?.selection.drag(event) } // a prevented contact selects nothing
```

`drag` extends whatever selection is already anchored, and makes that paragraph first responder:

```513:518:host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift
    func drag(_ event: NSEvent) {
        if pendingBegin { deferredDrag = event; return }
        guard anchor != nil || list != nil else { return }
        if let anchor {
            holding = true
            anchor.window?.makeFirstResponder(anchor)
```

The down was prevented. The drag still changes the selection and the focus. A click on the paragraph itself does not forward, so that path stays correct. `keepingFocus` on this same forward is fine: the superview sets it from `prevented === event`, which the first `pointerPressed` already recorded, and `focusSurfacePointer` sees it.

The other checks hold. Canvas down in `mouseDown`, `rightMouseDown`, and `otherMouseDown`, and a surface control's `control("down")`, all run after `keepingFocus` is set and before its `defer` restores the outer value. A bubbled superview either sets it again or is still inside the hit's scope. Nothing else calls `focusSurfacePointer` on that path. A right or middle up no longer clears the left flag, and a later left down says it again when that call is the one that passes the guard. This commit does not touch the web or iOS paths.

VERDICT: CHANGES
