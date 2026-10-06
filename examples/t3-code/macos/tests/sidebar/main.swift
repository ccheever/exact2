import AppKit
import XCTest

// The sidebar's native pieces (T3Sidebar.swift) with real AppKit: the thread
// action menu built from the client's ElectronMenu-shaped template, the
// selection a menu item reports, the exact-⌘ jump-hint predicate and its
// 200 ms delay, and the module requests the client makes.
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

    func testJumpHintsNeedExactlyCommandForTwoHundredMilliseconds() {
        XCTAssertTrue(T3Sidebar.jumpModifiers([.command]))
        XCTAssertTrue(T3Sidebar.jumpModifiers([.command, .capsLock, .numericPad, .function]))
        XCTAssertFalse(T3Sidebar.jumpModifiers([.command, .shift]))
        XCTAssertFalse(T3Sidebar.jumpModifiers([.command, .option]))
        XCTAssertFalse(T3Sidebar.jumpModifiers([]))
        var changes = 0
        let sidebar = T3Sidebar(agent: true) { topic in XCTAssertEqual(topic, "t3.status"); changes += 1 }
        sidebar.flags([.command])
        // The delayed check reads the live flags, which no key holds here: no hints.
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertFalse(sidebar.jumpHints)
        XCTAssertEqual(changes, 0)
        XCTAssertEqual(sidebar.status["sidebarJumpHints"] as? Bool, false)
    }

    func testModifiersAndNotifyRequests() {
        var changes = 0
        let sidebar = T3Sidebar(agent: true) { _ in changes += 1 }
        let modifiers = expectation(description: "modifiers")
        sidebar.perform(["op": "sidebarModifiers", "generation": 1]) { response in
            let value = response["value"] as? [String: Any] ?? [:]
            XCTAssertEqual(Set(value.keys), ["command", "shift", "option", "control"])
            modifiers.fulfill()
        }
        let notify = expectation(description: "notify")
        sidebar.perform(["op": "sidebarNotify", "delay": 50.0, "generation": 1]) { response in
            XCTAssertEqual(response["ok"] as? Bool, true)
            notify.fulfill()
        }
        wait(for: [modifiers, notify], timeout: 1)
        RunLoop.main.run(until: Date().addingTimeInterval(0.15))
        XCTAssertEqual(changes, 1)
        let unknown = expectation(description: "unknown")
        sidebar.perform(["op": "sidebarNope"]) { response in XCTAssertEqual(response["ok"] as? Bool, false); unknown.fulfill() }
        wait(for: [unknown], timeout: 1)
    }

    /// r8-pointer D8: a row press reads the modifiers its click carried, even
    /// after ⌘/⇧ is released; the click is read once and expires after 2 s.
    func testRowPressReadsTheClickEventsModifiers() {
        _ = NSApplication.shared
        let sidebar = T3Sidebar(agent: false) { _ in }
        func click(_ type: NSEvent.EventType, _ flags: NSEvent.ModifierFlags) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        }
        func read() -> [String: Bool] {
            var value: [String: Bool] = [:]
            let done = expectation(description: "modifiers")
            sidebar.perform(["op": "sidebarModifiers", "generation": 1]) { response in
                value = (response["value"] as? [String: Any] ?? [:]).compactMapValues { $0 as? Bool }; done.fulfill()
            }
            wait(for: [done], timeout: 1)
            return value
        }
        sidebar.record(click(.leftMouseDown, [.command]))
        sidebar.record(click(.leftMouseUp, [.command]))
        XCTAssertEqual(read()["command"], true, "⌘-click toggles though ⌘ is up by the time the client asks")
        XCTAssertEqual(read()["command"], false, "the click is read once; no key is held now")
        sidebar.record(click(.leftMouseDown, [.shift, .capsLock]))
        sidebar.record(click(.leftMouseUp, [.shift, .capsLock]))
        let shifted = read()
        XCTAssertEqual(shifted["shift"], true)
        XCTAssertEqual(shifted["command"], false)
        sidebar.record(click(.leftMouseUp, [.command]), at: ProcessInfo.processInfo.systemUptime - 3)
        XCTAssertEqual(read()["command"], false, "a click older than 2 s is not this press's")
        XCTAssertEqual(sidebar.pressModifiers(at: 0).contains(.command), false)
    }
}

let suite = XCTestSuite(forTestCaseClass: T3SidebarTests.self)
suite.run()
let run = suite.testRun!
print("T3Sidebar tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
