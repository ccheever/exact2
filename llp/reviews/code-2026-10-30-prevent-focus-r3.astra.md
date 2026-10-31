# Code review: a pointerdown action's preventDefault() keeps the focus where it is (web, macOS, iOS), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `84a001e2c4`.
- **Method:** one brief (sha256 `e2eaa09006f4f0330e52de0e5dc56dc1208a2b32c2c8fea87bb60f749476bc8d`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 29f2860d1f: the contact flag cleared at its up, no iOS commit while composing, Presenter.authoredFocus lets an action's focus() land, a control-click's pre-focus kept; the iOS caret declared in the docs

## Brief

Code review, round 3 of 3 (contract semantics plus web glue, macOS and iOS: a pointerdown action's preventDefault() keeps the focus where it is, places no caret and starts no selection; the press still fires). Tip 84a001e2c4 is HEAD of this worktree; base 048bf305ca. Review the whole change fresh with `git diff 048bf305ca..HEAD`, and round 2's fixes with `git diff 6ea5eefea7..HEAD`.

Round 2 findings, and the fixes:
- **Popover and panel key (Astra P2, Grok P2):** a node that a pointerdown handler hears gets no pre-focus from AppKit. Its mouseDown now takes the click focus right after pointerPressed, before every branch (SVG target, canvas input, surface control, paragraph), through takeClickFocus(): first responder, plus makeKey on a non-key window that can become key (a popover's window, a key-only-if-needed panel), as AppKit's pre-focus did. Test (Mac): "hear", a focusable box whose pointerdown is heard and not prevented, takes the focus after its pointerdown, in that order.
- **SVG and surface returns (Astra P2, Grok P2):** covered by moving the focus decision before those branches (above). Not separately tested.
- **Canvas and surface focus on a prevented press (Astra P5, Grok P2):** MouseChain.preventedContact (set by a prevented pointerdown, cleared at each button down in pressEditorFirst) makes focusSurfacePointer keep the focus on the Mac; on iOS, keepsFocus(from: self). Not separately tested.
- **A prevented Mac drag extended an existing selection (Astra P3):** mouseDragged skips selection.drag for a prevented contact. Not separately tested.
- **iOS document surface (Astra P4, Grok P2):** interactionShouldBegin also refuses when the prevented touch is on the SelectionDocument's surface. Not separately tested (the test covers a single paragraph).
- **iOS prevented button press skipped the edit commit (Grok P2):** a prevented press now commits presenter.edited (change) without moving the focus, as the Mac commits on every press. Test: text typed into a, then a prevented tap on "plain"; a keeps the focus and change-a is heard.
- **Not changed:**
  - An already-focused iOS field's caret move under a prevented touch (Grok P2). UIKit places the caret through its own tap on a first responder, with no public hook short of disabling interaction; keeping the focus is what's tested, and the caret move is a named residual.
  - SelectableText.selectWord, the agent's path: unchanged.
Results: Mac (PreventFocus, EditorPointerOrder, EditorPointer, ClickFocus) 0 failed; iOS hosted 3/3; web (the JS target, mouse and touch) pass; caps green. The full suites on this tip are running; on 6ea5eefea7, Mac was 307/0, and iOS failed IframeCaptureTests' pinch-pan clip at load 245 (a QUEUE'd flake, unrelated).

Check hard, final round:
- the moved focus decision (any focus now given where AppKit gave none: inner focus, retainFocus, disabled, inert, a first click into an inactive window);
- makeKey on canMakeKey windows (sheets, alerts' windows, and the window of an ExactView embedded in someone's panel);
- preventedContact's lifetime (a down outside any hosted window, the agent's presses, nested loops);
- the iOS commit on a prevented press (double change, an IME composing);
- whether the residual (iOS caret move in an already-focused field) is fair to name rather than fix;
- the tests.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

- **[P2] Prevention leaks into later Mac contacts** — [MouseChainMac.swift:510](/tmp/iosfix2-rv-pvf3-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:510). `preventedContact` survives mouse-up and resets only for `.leftMouseDown`. After a prevented click, an unprevented right/middle click on an input canvas therefore still skips focus through `focusSurfacePointer()`. The agent’s surface-control keyboard path also reads this stale flag. Scope prevention to the contact and test a prevented press followed by an unprevented secondary click.

- **[P2] The iOS commit publishes unfinished IME text** — [PointerIOS.swift:636](/tmp/iosfix2-rv-pvf3-astra/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:636). This branch calls `commitEdit` without checking `markedTextRange` or ending composition. If text is still marked, it emits `change` with provisional text and clears `edited` while the IME remains active. Resolve composition before committing, or leave that edit pending. The added test covers ordinary insertion only; its `contains("change-a")` assertion also does not establish exactly-once delivery.

- **[P2] iOS cancels explicit authored focus too** — [NodeViewIOS.swift:382](/tmp/iosfix2-rv-pvf3-astra/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:382), [TextAreaIOS.swift:111](/tmp/iosfix2-rv-pvf3-astra/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:111). A prevented touch remains recorded throughout `pointerup` and `press`. Consequently, an explicit `focus()` targeting the touched box or field from either handler is rejected by `becomeFirstResponder`. Canceling the pointer’s default should still allow that authored focus command, as the web and Mac paths do. The current tests only request explicit focus after the touch record expires.

- **[P2] The documented contract conceals the known iOS caret exception** — [contract-grammar.md:1562](/tmp/iosfix2-rv-pvf3-astra/docs/contract-grammar.md:1562). The guide promises no caret placement or selection, but an already-focused iOS editor still processes UIKit’s text gestures. Naming that residual in the review request does not qualify the shipped promise; LLP 1005 declares only the Linux exception. Either fix and test real editor touches, or explicitly narrow the documented iOS guarantee. The tests’ direct `becomeFirstResponder()` calls cannot establish caret preservation.

Both requested diffs reviewed; both `git diff --check` runs passed. Runtime suites were not rerun: this read-only checkout has no build artifacts. No files changed.

VERDICT: CHANGES
