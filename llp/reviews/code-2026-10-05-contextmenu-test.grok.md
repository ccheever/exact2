# Round 1
I'll review only commit `9b5534371` and the menu press path it depends on, without changing any files.The request is a single-commit review, so I'll read that diff and the menu press path it depends on.No defects.

The diagnosis is right. `pick` records the item and presses on a later main-queue turn:

```423:433:host/apple/Sources/ExactKit/Mac/MenusMac.swift
@objc private func pick(_ sender: NSMenuItem) {
    guard let pick = sender.representedObject as? Pick, !pick.once.taken else { return }
    pick.once.taken = true
    picking.append(pick)
    DispatchQueue.main.async { [weak self, pick] in
        guard let self else { return }
        self.picking.removeAll { $0 === pick }
        guard self.valid(pick), let row = pick.row else { return }
        pick.cancelled = true
        self.presenter?.press(row.id, fromNativeMenu: true)
    }
}
```

`NSApp.sendAction` invokes that synchronously, so the press is still queued when line 53 checks `pressed`. The old `RunLoop.main.run(until:)` of 50 ms can return before that block runs. The reported mismatch was `[]` against `[5]`, which is the assertion at line 57; the check at line 53 carries the message `"on the next turn"` and had already passed. A loaded machine taking 2.2 s for the whole test fits that.

The test is deterministic, bounded by the 10 s timeout. `wait(for:)` runs the main run loop until `onPress` fulfils the expectation, so the queued block is waited on instead of sampled for 50 ms. The same wait is what `ChooserMacTests.turn()` relies on.

It still proves the original contract. Line 53 still requires no press inside the action. Line 57 then requires `pressed == [5]`. That array gets its only append in the same closure that fulfils, and `press` calls `onPress` once:

```765:774:host/apple/Sources/ExactKit/Mac/PresenterMac.swift
func press(_ id: UInt32, fromNativeMenu: Bool = false, held: String = "") {
    pressHeld = held; defer { pressHeld = "" }
    guard let node = textHost(id), !node.inert, !node.disabled,
          fromNativeMenu || (segments.shown(node) ?? !node.isHiddenOrHasHiddenAncestor) || toolbar.contains(node) else { return }
    let command = dialogs.command(node, fromNativeMenu: fromNativeMenu)
    let popover = menus.command(node, fromNativeMenu: fromNativeMenu)
    if command == nil && popover == nil, node.id == id, let url = node.defaultLink, node.activateLink(url) { return }
    if (command == nil && popover == nil) || node.handlers.contains("press") { onPress?(id) }
    command?()
    popover?()
}
```

Row 5 has a `press` handler, so `onPress(5)` runs once. The hide command that follows closes a popover this test never opened (`menu(of:)` does not enter `entries`), and that `close` does not press. `pick` also sets `once.taken` before returning, so a second action on this menu is dropped.

`valid(pick)` accepts this pick. The test builds the menu with no invoker, so `Pick.invoked` is false and `valid` returns at the `guard pick.invoked else { return true }` once the row checks pass. Those checks stay true here: row 5 remains live, parented under popover 2, enabled, and titled `"Delete"`, which line 48 already reads back, and nothing in this test bumps `presentation`. A rejection would drop the press for good and time out at 10 s; the recorded failure was a press that had not arrived after 50 ms.

A second `fulfill()` would fail the test on purpose. `expectation(description:)` is the `XCTestCase` API, which sets `assertForOverFulfill`, so a second call raises. This path calls `onPress` once, before `wait` returns, so the line 57 check sees the only press.
