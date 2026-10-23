import AppKit
import XCTest

// The sidebar's native pieces (T3Sidebar.swift) with real AppKit: a menu built
// from the client's ElectronMenu-shaped template (the legacy members menu's builder,
// shown here with the richest template, a thread menu's), the selection a menu item
// reports, and the module requests the client makes.
final class T3SidebarTests: XCTestCase {
    let template: [[String: Any]] = [
        ["type": "item", "id": "pin", "label": "Pin thread", "enabled": true, "destructive": false],
        ["type": "submenu", "id": "snooze", "label": "Snooze", "enabled": true, "destructive": false, "children": [
            ["type": "item", "id": "snooze:hour", "label": "In 1 hour (13:00)", "enabled": true, "destructive": false],
            ["type": "separator"],
            ["type": "item", "id": "snooze:custom", "label": "Custom…", "enabled": true, "destructive": false],
        ]],
        ["type": "separator"],
        ["type": "item", "id": "regenerate-title", "label": "Regenerating…", "enabled": false, "destructive": false],
        ["type": "submenu", "id": "auto-settle", "label": "Auto-settle behavior", "enabled": true, "destructive": false, "children": [
            ["type": "item", "id": "auto-settle:enabled", "label": "Enabled", "enabled": true, "checked": false, "destructive": false],
            ["type": "item", "id": "auto-settle:disabled", "label": "Disabled", "enabled": true, "checked": true, "destructive": false],
        ]],
        ["type": "separator"],
        ["type": "item", "id": "archive", "label": "Archive thread", "enabled": true, "destructive": false],
        ["type": "item", "id": "delete", "label": "Delete", "enabled": true, "destructive": true],
    ]

    func testMenuMirrorsTheTemplate() {
        _ = NSApplication.shared
        let sidebar = T3Sidebar(agent: true) { _ in }
        let menu = T3Sidebar.menu(template, target: sidebar)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title },
                       ["Pin thread", "Snooze", "—", "Regenerating…", "Auto-settle behavior", "—", "Archive thread", "Delete"])
        XCTAssertFalse(menu.autoenablesItems)
        XCTAssertEqual(menu.items[1].submenu?.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["In 1 hour (13:00)", "—", "Custom…"])
        XCTAssertFalse(menu.items[3].isEnabled)
        XCTAssertEqual(menu.items[4].submenu?.items.map(\.state), [.off, .on])
        XCTAssertNotNil(menu.items[7].image, "the destructive row carries the trash glyph")
        XCTAssertNil(menu.items[6].image)
        XCTAssertNil(menu.items[1].representedObject, "a submenu row is not itself a choice")
    }

    func testPickingAnItemReportsItsId() {
        _ = NSApplication.shared
        let sidebar = T3Sidebar(agent: true) { _ in }
        let menu = T3Sidebar.menu(template, target: sidebar)
        let custom = menu.items[1].submenu!.items[2]
        XCTAssertEqual(custom.representedObject as? String, "snooze:custom")
        menu.items[1].submenu!.performActionForItem(at: 2)
        XCTAssertEqual(sidebar.picked, "snooze:custom")
        let reply = expectation(description: "menu reply")
        // The agent never enters menu tracking: it answers dismissed.
        sidebar.perform(["op": "sidebarMenu", "items": template, "generation": 3]) { response in
            XCTAssertEqual(response["generation"] as? Int, 3)
            XCTAssertEqual((response["value"] as? [String: Any])?["shown"] as? Bool, false)
            reply.fulfill()
        }
        wait(for: [reply], timeout: 1)
    }

    /// The delayed notification the provider pill and snooze refresh ask for. The click-modifier read is gone (a row
    /// press carries its own MouseEvent: sidebar.test.ts "a row press carries its own
    /// click modifiers" holds the cases this suite had), and so is the speed-mode read (the window's key events).
    func testNotifyRequestsAndNoModifierRead() {
        var changes = 0
        let sidebar = T3Sidebar(agent: true) { _ in changes += 1 }
        let notify = expectation(description: "notify")
        sidebar.perform(["op": "sidebarNotify", "delay": 50.0, "generation": 1]) { response in
            XCTAssertEqual(response["ok"] as? Bool, true)
            notify.fulfill()
        }
        wait(for: [notify], timeout: 1)
        RunLoop.main.run(until: Date().addingTimeInterval(0.15))
        XCTAssertEqual(changes, 1)
        for op in ["sidebarNope", "sidebarModifiers", "sidebarSpeedMode"] {
            let unknown = expectation(description: op)
            sidebar.perform(["op": op]) { response in XCTAssertEqual(response["ok"] as? Bool, false); unknown.fulfill() }
            wait(for: [unknown], timeout: 1)
        }
    }
}

let suite = XCTestSuite(forTestCaseClass: T3SidebarTests.self)
suite.run()
let run = suite.testRun!
print("T3Sidebar tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
