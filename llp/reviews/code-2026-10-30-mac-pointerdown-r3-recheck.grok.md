# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3, narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `9bd2e2f84b`.
- **Method:** one brief (sha256 `3374c739dea7ddc07bb43bcfe0e8e38809ef6d3f38928dd3a0285fcf8ba294d7`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in e8cc923edf and 497de689b9, each with a test that fails before the fix

## Brief

Narrow re-check of round 3's fixes (host change, macOS: early pointerdown before focus in a text area or an input). HEAD 9bd2e2f84b; round 3 reviewed e130732a79. Read `git diff e130732a79..HEAD` first, then whatever context you need from `git diff 4293f59072..HEAD`.

Round 3 findings (Astra and Grok), and the fixes:
- P1: the monitor swallowed the press's up taken while the early down was still bubbling, so AppKit's tracking loop then waited for an up that never came. Now releaseEarlyPress stores that up in mouseChain.heldUp and returns true; the monitor returns nil. After pointerPressed returns, pressEditorFirst re-posts it with NSApp.postEvent(up, atStart: true). The window then gets the down (tracking dequeues the re-posted up, and tracksPointer releases in order), or, if no tracking runs, the up comes through the monitor again and releaseEarlyPress releases. The RunLoop.perform block is gone. Proof: testAnUpTakenWhileTheDownBubblesWaitsForTheWindow now uses ONE up (no test re-post) and asserts no up is left over. On e130732a79's product code that test HANGS: a sample shows Field.mouseDown, then tracksPointer, then -[NSControl(_NSTracking) mouseDown:], killed after 6.5 min. On the tip it passes with ["down-z","down-zo","blur-a","focus-z","up-z","up-zo"].
- P2: the detached-source fallback at the top of the monitor ran before the early check. The early check now runs first. New test: testAnUpAfterTheEditorLeftWhileTheDownBubblesWaitsToo (the inner pointerdown removes the editor's box, then delivers the up).
- P2: every up while bubbling was swallowed. Only the first is held now; later ones return false and go their way. A nil-event release (the view leaving its window) while bubbling sets releaseAfterBubbling instead. New test: testOnlyThePresssOwnUpIsHeld.
- Presenter.reset clears heldUp and releaseAfterBubbling. The LLP 1005 text is updated.
Tests: EditorPointerOrderMacTests has 17 and EditorPointerMacTests 4, all passing. The full Mac suite on the tip is running; the previous tip was 302/0.

Check only:
(a) Are these fixes correct and complete?
(b) Is NSApp.postEvent(atStart:) of a dequeued, already-monitored up safe? Consider the monitor seeing it twice, its timestamp, other local monitors, and a nested-loop depth where the outer loop dequeues it.
(c) Did they introduce a new double report, stuck hold or lost up?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check the round 3 fixes against the two diffs, starting with `e130732a79..HEAD`, then only the earlier context those changes depend on.The tip reorders the monitor and holds the first up for a re-post. Next I’ll read the press, release, and tracking paths those changes sit on.The re-post fixes the hang on the path the new tests cover. Two holes in the new branches still mis-deliver the up.

The held up is posted with `NSApp.postEvent(_:atStart:)` only after `pointerPressed` returns (`MouseChainMac.swift:509-511`). That is still inside `sendEvent` of the down, so the outer loop is not in `nextEvent` and cannot dequeue it. The editor's tracking loop is the next `nextEvent`, in `NSEventTrackingRunLoopMode`, which does not call local monitors (Apple's `addLocalMonitorForEvents` contract, and `MenusMac.swift:640-643`). The passing order `down-z, down-zo, blur-a, focus-z, up-z, up-zo` with nothing left queued is that loop taking the up and `tracksPointer` releasing once. The same `NSEvent` is posted, so its timestamp, window, click count, and location stay the original release. On the no-tracking path the monitor does see that object again; `early` is still set and `earlyBubbling` is false, so `releaseEarlyPress` reports once and returns false (`MouseChainMac.swift:539-542`). `Presenter.reset` clears both new fields (`PresenterMac.swift:600`).

## Findings

**P2. A second left-mouse-up during bubbling is dispatched inside the first down.** Once `heldUp` is set, `releaseEarlyPress` returns false (`MouseChainMac.swift:535`), and the monitor does not drop the event (`ExactViewMac.swift:334`). The up falls through to the detached-source release (`ExactViewMac.swift:336-338`) and then to the window while `pointerPressed` is still walking ancestors. If that view has already left its window, `pointerReleased` runs there. Ancestors that have not heard `pointerdown` yet hear `pointerup` first. `early` is left set, so if the editor's `mouseDown` still runs, `tracksPointer` reports the up again (`MouseChainMac.swift:363-365`). `testOnlyThePresssOwnUpIsHeld` never takes this path: it calls `releaseEarlyPress` directly and posts the up itself (`EditorPointerOrderMacTests.swift:294-296`).

**P2. A nested mouse-down during bubbling is treated as the view leaving, and the real up is then dropped.** `pressEditorFirst` calls `releaseEarlyPress(nil)` whenever `early` is set (`MouseChainMac.swift:497`). A nil event while bubbling sets `releaseAfterBubbling` (`MouseChainMac.swift:534`). After `pointerPressed`, that flag releases immediately and stores the down in `reported` (`MouseChainMac.swift:513`, `539-541`), still inside the monitor, before NSWindow focuses the editor. Tracking then runs the loop and skips `pointerup` (`MouseChainMac.swift:362`). The pointer event that remains is the nil release, taken from `currentEvent` (the down), not the button's up. `ExactView.viewWillMove` (`ExactViewMac.swift:299`) is the caller that flag is for. This supersede call is the same entry, and `pointerHolds` already makes the nested down return before it can become a new early press (`MouseChainMac.swift:498-504`).

A focus or blur handler that spins its own loop between the post and `tracksPointer` can still dequeue that up and leave `-[NSControl mouseDown:]` waiting. That window is what is left of the old hang. It is not a new one on the path these tests run.

VERDICT: CHANGES
