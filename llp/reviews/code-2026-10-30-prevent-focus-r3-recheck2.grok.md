# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `43734672e2`.
- **Method:** one brief (sha256 `dd25ae029374dcc85ed684d2974968c595abf7ef9755578cf30a912ac4fc935c`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** redesigned in 9d324a525b: the surface focus reads MouseChain.keepingFocus, scoped to the prevented down's own handler; preventedContact only guards the left drag and is said again at each left down

## Brief

Narrow re-check of one fix (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 43734672e2 is HEAD of this worktree; base 048bf305ca; the last re-check read 29f2860d1f. Review `git diff 29f2860d1f..HEAD`, in the context of `git diff 048bf305ca..HEAD`.

The re-check's finding (Astra P2 and Grok, the same): Mac MouseChain.preventedContact outlived its contact. The ExactView monitor cleared it only at a left up and a right down, so a prevented right or middle click left it set, and a left up a tracking loop took (a native button under a preventing ancestor; tracksPointer's loop) never reached the monitor; a later surface press or surface-control `type` then read the stale flag in focusSurfacePointer.

The fix: the flag's lifetime is the pointer hold. pointerPressed clears it for each new contact of any button (after its hold guard, before propagating) and sets it if the pointerdown was prevented; pointerReleased clears it on every path (an up, a tracking loop's pointerReleased(nil), releaseEarlyPress), before its hold guard. The monitor's clear and pressEditorFirst's clear are gone.
Tests (each red on 29f2860d1f, green here): a native button (appearance="auto") inside a row whose pointerdown prevents: the press fires, the focus stays in a, and the flag is false after its loop; a prevented right press on "box": the flag is false after its up and the focus stays. Mac PreventFocus/EditorPointerOrder/ClickFocus/NativeButtons 0 failed; web pass; caps green; full Mac and iOS suites on this tip are running.

Check: every way a contact ends reaches pointerReleased (left, right, middle; native buttons; editors' tracksPointer; early release; a down outside hosted windows; agent presses; a nested loop in a handler); nothing reads the flag during a contact after it is cleared too early (mouseDragged's selection guard, the canvas/surface focus at the down); pointerPressed's guard returning early (a second button while one is held) leaves a correct flag. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check the pointer-hold lifetime of `preventedContact` against the tip diff and the full fix, without editing anything.The tip only moves when the flag is cleared. Next I’ll trace every contact-end path and every read of `preventedContact`.The flag now follows `pointerReleased` for a normal left, right, and middle up, a native button’s loop, an editor’s `tracksPointer`, an early release, an agent press, and a second button that returns at the hold guard. Two ends still leave it set, and one extra release clears it while a later contact is still down.

**P2.** A left up the session monitor discards never reaches `pointerReleased`, so `preventedContact` stays set. `ExactViewMac.swift:361` returns nil from `dialogs.pointer` for every left up on the page window while a `closedby="any"` sheet is up (`SheetsMac.swift:305`). That runs before the view’s `mouseUp`. The monitor clear that used to run on every left up, ahead of this discard, is gone. The only clear is `MouseChainMac.swift:403`. An editor is still released first (`ExactViewMac.swift:334`). A box, paragraph, or custom button is not an early press: its `pointerdown` can open that sheet (`SheetsMac.swift:262` returns only after the animation), the up is dropped, and the flag stays true. The hold stays too, so the next `pointerPressed` returns at `MouseChainMac.swift:333` and does not clear it. A later surface-control `type` then reads the stale flag in `focusSurfacePointer` (`SurfaceInput.swift:65`, from `AgentMac.swift:1005`).

**P2.** The flag is recorded after the handler, so a release inside the handler does not stick. `MouseChainMac.swift:347` clears it, the handler runs, and `MouseChainMac.swift:358` sets it again only after `propagating` returns. `beginSheet` spins the run loop until the animation finishes (`SheetsMac.swift:32`, `SheetsMac.swift:262`), which is the loop that can take the button’s up while `pointerPressed` is still on the stack. For a non-editor there is no `earlyBubbling` hold, so that up calls `pointerReleased` (`MouseChainMac.swift:403`) and then line 358 sets the flag on a contact whose hold is already clear. Until the next real down, `focusSurfacePointer` (`SurfaceInput.swift:65`) keeps a surface `type` from taking focus. A drag in that same loop reads the flag while it is still clear (`MouseEventsMac.swift:284`), which is before line 358.

**P3.** The native button releases twice, and the second release is not that contact. `NativeButtonsMac.swift:180` clears the flag in `sendAction` before the action. `NativeButtonsMac.swift:159` calls `pointerReleased(nil)` again after `super.mouseDown` returns. A contact that starts during the action (the sheet animation above) is still down when that second call clears `preventedContact` and the hold.

Left, right, and middle ups on the view, `tracksPointer` (`MouseChainMac.swift:380`), `releaseEarlyPress` (`MouseChainMac.swift:563`), an up whose source has left its window (`ExactViewMac.swift:337`), and an agent `sendEvent` up all reach `pointerReleased`. A second button while one is held returns at `MouseChainMac.swift:333` and leaves the held contact’s flag as it was. On a down that is not re-entered, the selection guard and the canvas or surface focus run only after `pointerPressed` returns (`MouseEventsMac.swift:217` then `223`–`224`), so they see the value from line 358.

VERDICT: CHANGES
