# Code review: PresenterIOS.swift under its cap, and HeaderRule's early answer pinned, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `dbb8642cfeeab23a08b375ae3465184407b5587bb2d318048ec1432092d1dd3d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

No BLOCKER, MAJOR, MINOR, or NIT findings.

`HEAD~2` (`19434dcb5`) is a pure move. `FirstResponder` and the `UIResponder` extension are unchanged in `host/apple/Sources/ExactKit/IOS/FirstResponderIOS.swift`, still inside `#if os(iOS) || os(tvOS)`, still importing UIKit, still paired as `fileprivate`. `report` and `@objc exactReportFirstResponder` are in that one file, and `#selector(UIResponder.exactReportFirstResponder)` is unchanged. `FirstResponder.current` stays internal; `PresenterIOS.swift:187`, `MenuFocusIOS.swift:96`, `NavigationIOS.swift:865`, and `PressFocusIOSTests.swift:686` still resolve it. Nothing else named `report` or the selector. Caps counts `PresenterIOS.swift` at 1485 lines (`split('\n').length`), under the 1500-line cap; it was 1501.

`HEAD~1` (`63dfc3dfd`) was left as already reviewed. The new test pins the skip a nil assertion would not.

`HEAD` (`2c0d625de`) pins that skip. `HeaderRule.walked` (`HeaderRule.swift:30`) increments once per node `add` visits (`HeaderRule.swift:76`), which is the first child's subtree. The route is the words' root and is not counted; later siblings are not visited.

The counter is acceptable in production. It is one plain `Int` add on the main thread, on the walk that already reads facts and appends words. Every caller is main-thread projection: `HeaderShape.init` (`NavigationBarIOS.swift:124`), `WindowHeader.read` (`HeaderMac.swift:86`), `SheetHeader.read` (`RouteSheetMac.swift:84`). The feed path returns at `HeaderRule.swift:68` and does not touch it. `nonisolated(unsafe)` is the module's existing mark for main-thread globals.

The node counts match the tree and the author's 41-then-47 failure. Route 2's later `children(2, [30, 10, 15])` replaces `[10, 15]`, so the first child is feed 30. Rows `31..<71` are 40 text leaves: 1 + 40 = 41 if the early answer is removed. Route 3's header walks in preorder as 20, 21, 210, 22, 23, 231 — six — and not page 25. Both calls are measured from the same `before`, so a feed walk of 41 makes the second expectation 47.

`HeaderMacTests` shares one `xctest` process with the other mac-wide classes (`xctest.mjs` matches `activate(`), run one after another. `walked` is never reset, and the test samples a delta across two synchronous inits. It does not spin the run loop, so earlier classes in that process cannot land inside the window.

I did not re-run the suite.

READY
