# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `9bd2e2f84b`.
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

- **P1 — The replay can be consumed before tracking starts.** [MouseChainMac.swift:508](/tmp/iosfix2-rv-pd4-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:508). The code clears `earlyBubbling` and queues the up while still handling the original down in the monitor. If a subsequent local down monitor runs a nested loop and dequeues/sends that up, `releaseEarlyPress` clears `early`, records the down in `reported`, and passes the up through before the window receives the down. Later, `tracksPointer`’s `reported` branch still calls `track()` at line 362, with no up remaining. The original hang can recur. The up needs to survive until the tracking handoff; the current test covers only one intervening loop.

- **P2 — A later up still releases a detached early press during bubbling.** [MouseChainMac.swift:535](/tmp/iosfix2-rv-pd4-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:535). Once `heldUp` is occupied, another up returns `false`. If the inner down handler detached the editor, the monitor immediately reaches [ExactViewMac.swift:338](/tmp/iosfix2-rv-pd4-astra/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:338) and releases the original pointer path despite `earlyBubbling` remaining true. This permits `down-z, up-z, up-zo, down-zo`. The second up must pass without releasing the suspended first press. `testOnlyThePresssOwnUpIsHeld` calls the helper directly, so it misses this fallback.

Reposting the existing event is valid, and retaining its original [timestamp](https://developer.apple.com/documentation/appkit/nsevent/timestamp) is appropriate. However, [`atStart`](https://developer.apple.com/documentation/appkit/nsapplication/postevent(_:atstart:)) orders the shared queue; it does not reserve the event for a particular loop. Earlier local monitors can observe it twice when replay goes through `sendEvent`; native tracking instead [bypasses local monitors](https://developer.apple.com/documentation/appkit/nsevent/addlocalmonitorforevents(matching:handler:)). The ordinary replay paths avoid duplicate Exact reports, but that does not close the two paths above.

Static review only: no built test bundle was available, and AppKit initialization failed in the read-only sandbox. No files changed.

VERDICT: CHANGES.
