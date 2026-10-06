# Code review: a bar item's menu without the software keyboard (MenuFocusIOS, LLP 1021 §5.1), 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at the reviewed commit; read-only by instruction.
- **Method:** one brief (sha256 `9889df77a1313504663b2645d5999d7e8f2f1472961b4da47a3bd8585d93efd5`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken (astra's 1). 2 taken (astra's 3). 3 taken (astra's 2). 4 taken: the tests now cover a second bar touch keeping the hold, the view's focus returning at the next touch and after a context menu, a node blurred by a bar touch, and the recognizers' flags on a bar and on the viewport. Touch delivery itself is shown on the simulator, not in XCTest (UIKit makes no public `UITouch`).

---

[P1] A second bar touch forgets the responder the next content touch was supposed to restore — `host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:38`

`setAside` treats `untilNextTouch` as “someone else is taking over” and clears `held` before it has a new first responder. A bar touch always calls `setAside(untilTouch: true)` (`MenuFocusIOS.swift:97`), and the session-view recognizer ignores that touch (`MenuFocusIOS.swift:94-96`), so it does not restore first. After the first touch, `ExactView` or the pressed node is already resigned, so the second call finds nothing to hold and leaves `untilNextTouch` false. Tapping the same bar item again to dismiss the pull-down, or tapping Back, the title, or another item, hits this path. `touched()` then no-ops (`MenuFocusIOS.swift:60`). Key commands stay dead: they are delivered only while `ExactView` is first responder (`ExactViewIOS.swift:167-170`), and the resign was quiet, so the app never gets `blur` either. The same clear runs if a context menu or invoker pull-down calls `setAside()` while a bar set-aside is still outstanding and nothing has become first responder (`MenuFocusIOS.swift:111`, `ContextMenusIOS.swift:306`); `willEnd` restores nothing.

[P1] Content touches never restore focus while a sheet or cover is up — `host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:55`

The recognizer that calls `touched()` is added to `session.view` (`ExactView`). Presenting a route reparents the viewport, which holds the navigation bar and the page, onto the modal controller (`ModalIOS.swift:368` via `NavigationIOS.swift:535-543`). Touches on that page and its bar no longer hit `ExactView`, so `untilNextTouch` stays set for the whole presentation. A later bar touch then takes the path in the finding above and the focus does not return after the cover dismisses either. On the root stack the bar is still inside `ExactView`, and the superview walk that skips those touches (`MenuFocusIOS.swift:95`) is right; it does not cover a presented stack.

[P2] A rebuilt navigation controller never gets `BarTouch` — `host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:420`

`watch` runs only from `project`, and only when `projectedSource` changes (`NavigationBarIOS.swift:378-380`). `RouteController`s are reused by node id (`NavigationIOS.swift:239-241`). `retireTabs` moves those controllers into new navigation controllers without clearing `projectedSource` (`NavigationTabsIOS.swift:287-292`). The new bar has no recognizer, so a bar menu there takes the focus UIKit saw and the software keyboard comes back. The first install is fine: `setViewControllers` runs before the async `watch`, so `navigationController` is set. tvOS correctly does not call `watch` (`#if os(iOS)`). The remote does not deliver `touchesBegan`, and this keyboard path is iOS-only.

[P2] The new test does not prove `BarTouch`, and its context-menu leg leaves focus unset — `host/apple/tests/ExactKitTests/ContextMenuIOSTests.swift:270`

`testABarsTouchSetsTheFocusAsideUntilTheNextTouchOnTheView` calls `setAside(untilTouch:)` and `touched()` directly. It never builds a touch, never calls `BarTouch.touchesBegan`, and the presenter has no session view, so `watch` cannot install the restore recognizer. The recognizer check is only “one `BarTouch` on a free-standing `UINavigationBar`, and `cancelsTouchesInView` is false.” It does not check `holdsMenu`, the on-bar exclusion, `delaysTouchesBegan`, or a failed state. The context-menu leg (`ContextMenuIOSTests.swift:284-289`) calls `setAside()` while `untilNextTouch` is still true, which is the clear at line 38, then asserts only that no focus events fired and that `untilNextTouch` is false. It does not assert the source is first responder again, which is what `testTheFocusIsSetAsideQuietlyWhileTheMenuShows` asserts and what this leg loses.

The recognizer flags that keep it from taking the bar’s touches are set: `cancelsTouchesInView`, both delay flags, an immediate `.failed`, and simultaneous recognition (`MenuFocusIOS.swift:85-100`). A successful set-aside still resigns and restores inside `quiet`, so a node does not dispatch `blur` or `focus`. tvOS is left on the old path on purpose. The gap the RFC accepts — shortcuts stay quiet until the next content touch, because `GCKeyboard` cannot see a hardware keyboard apart from the software one — is real on the root stack’s first bar touch only. The findings above make that gap permanent.

**NOT READY**
