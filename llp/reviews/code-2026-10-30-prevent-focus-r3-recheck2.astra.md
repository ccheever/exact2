# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `43734672e2`.
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

- **P2 — A second button’s release clears prevention too early:** [MouseChainMac.swift:403](/tmp/iosfix2-rv-pvf5-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:403). With an existing paragraph selection, hold left on a selectable paragraph whose `pointerdown` prevents default, then press and release middle while keeping left held. The middle down returns at `pointerPressed`’s hold guard, but `otherMouseUp` calls `pointerReleased`, clearing the flag. The next left drag passes `MouseEventsMac.swift:284`’s selection guard; `TextSelection.drag` extends the existing selection and can refocus its anchor. Middle-up did not clear this flag at `29f2860d1f`. Preserve prevention until the owning contact ends, and cover this button sequence.

Static review only; tests were not rerun in the read-only worktree. No files changed.

VERDICT: CHANGES
