# Code review: a bar item's menu without the software keyboard (MenuFocusIOS, LLP 1021 §5.1), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the reviewed commit.
- **Method:** one brief (sha256 `9889df77a1313504663b2645d5999d7e8f2f1472961b4da47a3bd8585d93efd5`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken: a set-aside that finds nothing to set aside keeps what is held (test: two bar touches, then the next touch restores). 2 taken: the bar's recognizer is installed in `prepareRoutes`, for every stack each time it is prepared, not only when a header's projection changes. 3 taken: the restoring recognizer is on the viewport, which a presentation carries with the page. 4 taken in part: a bar touch now blurs a focused node (dispatching `blur`, as a touch on a page's chrome blurs an element on the web) and holds nothing for it; only the view's own focus (key commands, no events) waits for the next touch. Telling a menu item's touch from another bar touch needs the bar item's view, which UIKit does not expose.

---

1. **[P2] Repeated bar touches lose the saved responder.** [MenuFocusIOS.swift:38](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:38) clears `held` and `untilNextTouch` before finding a replacement. Touch a menu-bearing bar twice without touching the content between them: the second call finds no Exact responder because the first call resigned it, and returns with nothing saved. Subsequent content touches cannot restore focus. Hardware shortcuts remain unavailable, and a previously focused node never receives its eventual blur. Preserve the existing hold when no new responder replaces it.

2. **[P2] Rebuilt navigation controllers can miss the recognizer.** [NavigationBarIOS.swift:420](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:420) installs the watcher only inside `project`, which runs when `projectedSource` changes. Changing the tab configuration rebuilds navigation controllers while retaining their routes and projection cache (`NavigationTabsIOS.swift:195`). An unchanged route therefore carries its menu items into a new bar without installing `BarTouch`; opening that menu raises the keyboard again. Installation needs to follow navigation-controller ownership, independently of header changes.

3. **[P2] Modal content cannot trigger restoration.** [MenuFocusIOS.swift:55](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:55) attaches the restoring recognizer exclusively to `session.view`. [ModalIOS.swift:368](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:368) moves the viewport into the presented controller, outside that view’s ancestry. Consequently, after opening a sheet’s bar menu, touching or scrolling its content never calls `touched()`. A subsequent button press can also dispatch a new focus without blurring the quietly suspended node. The observer should follow the movable content. UIKit recognizers observe touches only in their attached view and descendants. [Apple documentation](https://developer.apple.com/documentation/uikit/uigesturerecognizer/touchesbegan%28_%3Awith%3A%29)

4. **[P2] Non-menu bar touches silently disable keyboard input too.** [MenuFocusIOS.swift:97](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:97) suspends focus whenever *any* top-item button has a menu, regardless of the touched control. Tapping a title, ordinary action, or blank bar space therefore removes the keyboard target even when no menu opens. A Back action can remove the suspended node without ever delivering its blur. The documented delay after an actual menu extends here to unrelated navigation-bar interactions.

The [new test](/private/tmp/x2-peek-review/host/apple/tests/ExactKitTests/ContextMenuIOSTests.swift:270) proves only direct helper suspension/restoration and duplicate installation on a standalone bar. Its presenter has no session view, it sends no touches, and the bar has no menu. It does not exercise installation through navigation, the bar exclusion, touch delivery, keyboard visibility, or modal ownership. After the context-menu handoff it checks silence but never checks that focus returned, so losing `held` passes.

The weak references, duplicate guard, immediate `.failed` state, and disabled touch cancellation/delays look sound. The bar exclusion is independent of recognizer callback order. I found no new tvOS defect: installation is iOS-only, and the referenced menu property is available at the tvOS deployment target.

UIKit tests were not run because this checkout is read-only. No files were edited.

**Verdict: NOT READY.**
