# Code review: a bar item's menu without the software keyboard (MenuFocusIOS, LLP 1021 §5.1), round 2, 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at `eeb012e20`; read-only by instruction.
- **Method:** one brief (sha256 `56639575ecb41f842681ed088b15d519355519b70a7c2c73d9b4a7913dd0c197`), shared with astra. Blind to astra's round-2 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** the one finding taken (astra's 1). Round 2 of at most two; the fix was verified by the UIKit tests and on the simulator, not by a third review. Fixing it found a second case, also taken: a node that resigns hands the focus to `ExactView` (UIKit's fallback to its nearest ancestor that takes it, measured in the test), so every set-aside now releases the view too, or the menu's input would take it from the view (test: a context menu over a focused node leaves neither with the focus, and the node has it back after).

---

[P1] A bar touch that blurs a focused node never gives key commands back — `host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:51`**

A pressable takes the focus on touch-up (`NodeViewIOS.swift:1452`), so the usual case is a node as first responder, not `ExactView`. `setAside(untilTouch:)` then resigns that node for real (`quiet` is false, so `blur` is delivered) and clears both `held` and `untilNextTouch`. Nothing else becomes first responder. `ExactView` is what hears `aria-keyshortcuts`, and only while it is first responder (`ShortcutsIOS.swift:7`, `ExactViewIOS.swift:167-170`). After this blur it is not, and the next touch on the page does not make it so: `touched()` restores only when `untilNextTouch` is set (`MenuFocusIOS.swift:71`). A tap on empty page only calls `endEditing`, which resigns text fields (`ScrollViewIOS.swift:123`). Shortcuts stay dead for the rest of the session — through a later context menu as well, because that `setAside()` finds nobody to hold and `willEnd` restores nothing.

That is a regression against the view path this commit does get right. There, the same bar touch holds `ExactView` and the next page touch returns it. `QUEUE.md` and `llp/1021-menus.rfc.md` describe that return as the behavior. The node branch should blur the node and still arm the session view for that next touch, without making it first responder now (doing it now is what raises the keyboard over the menu). `testABarsTouchBlursAFocusedNode` (`ContextMenuIOSTests.swift:300`) asserts `untilNextTouch` is false on a presenter that has no session view, so it cannot see this.

## Round 1

1. **Second bar touch drops the hold.** Fixed. A `setAside` that finds nothing to resign leaves `held` in place when `untilTouch` is true (`MenuFocusIOS.swift:42-46`). `testABarsTouchSetsTheViewsFocusAsideUntilTheNextTouch` resigns twice and checks the following `touched()` makes the view first responder again (`ContextMenuIOSTests.swift:112-117`).
2. **A rebuilt navigation controller never gets `BarTouch`.** Fixed. `watch` runs from `prepareRoutes` on every stack, every time it is prepared (`NavigationBarIOS.swift:363-365`), including `installTabs` after `retireTabs` / `reshape`, `sync`, `syncTabs`, modal stacks, and `replayHooks`. It is inside `#if os(iOS)`.
3. **A sheet never restores.** Fixed. The restoring recognizer is on `presenter.viewport` (`MenuFocusIOS.swift:66-67`). A presentation moves that viewport into the presented controller (`ModalIOS.swift:368`), and the bar stays inside it, so the on-bar walk still applies.
4. **Any touch on a menu-bearing bar suspends focus.** Taken in part, and that part is the P1 above. A focused node is blurred and not put back. The view's own focus still waits — but only when the view itself was first responder.

The recognizer still does not take the bar's or the page's touches: `cancelsTouchesInView` and both delay flags are false, it fails inside `touchesBegan`, and it allows simultaneous recognition. iOS 26 bar buttons remain inside `UINavigationBar`, so both the bar recognizer and the on-bar exclusion see them. tvOS does not install `BarTouch`. Context-menu and invoker pull-downs still set aside and restore quietly; a `setAside()` that finds nothing while a bar hold is outstanding keeps that hold and lets `willEnd` return it.

The new tests prove that state machine. They never build a touch, never call `BarTouch.touchesBegan`, and never install through `prepareRoutes`. `holdsMenu` and the on-bar exclusion are untested. That gap was accepted last round because UIKit has no public `UITouch`; it does not hide the P1.

**Verdict: NOT READY.**
