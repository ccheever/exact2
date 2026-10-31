# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3, narrow re-check 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `f149659d14`.
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

No new findings. Both prior P2 findings are resolved:

- [MouseChainMac.swift:337](/tmp/iosfix2-rv-pvf7-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:337) preserves `preventedContact` for the same forwarded event while resetting it for a new left down.
- [MouseChainMac.swift:371](/tmp/iosfix2-rv-pvf7-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:371) captures the original chain and restores its outer value even after node removal. All three down handlers use the closure with `defer`.

No additional regressions found in the reviewed delta and base context. Tests were not rerun in this read-only workspace; `git diff --check` passed.

VERDICT: APPROVE
