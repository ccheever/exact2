import AppKit
import XCTest

// Compile with T3ContextMenu.swift alone (see README, settings-a). Real AppKit menus:
// the archive row's menu layout and its choice, and the theme export / import file
// helpers under an isolated directory (the agent's exports/ and imports/).

final class ContextMenuTests: XCTestCase {
    func testArchiveMenuSeparatesTheDestructiveDeleteWithItsTrashSymbol() {
        _ = NSApplication.shared
        let owner = T3ContextMenu()
        let menu = owner.menu(for: [["id": "unarchive", "label": "Unarchive"], ["id": "delete", "label": "Delete", "destructive": true]])
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Unarchive", "—", "Delete"])
        XCTAssertNil(menu.items[0].image)
        XCTAssertNotNil(menu.items[2].image)
        XCTAssertTrue(menu.items[2].image?.isTemplate ?? false)
        XCTAssertEqual(menu.items[2].representedObject as? String, "delete")
        XCTAssertTrue(menu.items.allSatisfy { $0.isSeparatorItem || $0.isEnabled })
    }

    func testPanelTabMenuKeepsPlainOrderAndDisabledCloseActions() {
        let menu = T3ContextMenu().menu(for: [["id": "rename", "label": "Rename"], ["id": "close", "label": "Close"],
            ["id": "close-others", "label": "Close others", "disabled": true],
            ["id": "close-to-right", "label": "Close to the right", "disabled": true], ["id": "close-all", "label": "Close all"]])
        XCTAssertEqual(menu.items.map(\.title), ["Rename", "Close", "Close others", "Close to the right", "Close all"])
        XCTAssertFalse(menu.items.contains(where: \.isSeparatorItem))
        XCTAssertTrue(menu.items.allSatisfy { $0.image == nil })
        XCTAssertFalse(menu.items[2].isEnabled)
        XCTAssertFalse(menu.items[3].isEnabled)
        XCTAssertTrue(menu.items[4].isEnabled)
    }

    func testUnderTheAgentTheMenuAnswersDismissedWithoutTracking() {
        // right-panel-tab-menu repro: popping the menu in the agent's never-key window tracked
        // until real input, leaving the request pending past `clock settle`'s 20 s bound.
        _ = NSApplication.shared
        var reply: [String: Any]?
        T3ContextMenu.perform(["op": "contextMenu", "generation": 7, "items": [["id": "close", "label": "Close"]]], agent: true) { reply = $0 }
        XCTAssertEqual(reply?["ok"] as? Bool, true)
        XCTAssertEqual(reply?["generation"] as? Int, 7)
        let value = reply?["value"] as? [String: Any]
        XCTAssertTrue(value?["clicked"] is NSNull)
        XCTAssertEqual(value?["shown"] as? Bool, false)
    }

    func testChoosingAnItemReportsItsId() {
        _ = NSApplication.shared
        let owner = T3ContextMenu()
        let menu = owner.menu(for: [["id": "unarchive", "label": "Unarchive"], ["id": "delete", "label": "Delete", "destructive": true]])
        menu.performActionForItem(at: 0)
        XCTAssertEqual(owner.picked, "unarchive")
        menu.performActionForItem(at: 2)
        XCTAssertEqual(owner.picked, "delete")
    }

    func testDeclaredSectionsAndDisabledItemsAreKept() {
        _ = NSApplication.shared
        let menu = T3ContextMenu().menu(for: [["id": "a", "label": "A"], ["id": "b", "label": "B", "separatorBefore": true, "disabled": true],
                                               ["id": "c", "label": "C", "destructive": true], ["label": "missing id"]])
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["A", "—", "B", "C"])
        XCTAssertFalse(menu.items[2].isEnabled)
    }

    func testExportWritesTheThemeFileIntoTheIsolatedDirectory() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-contextmenu-\(UUID().uuidString)/exports", isDirectory: true)
        let done = expectation(description: "saved")
        var reply: [String: Any] = [:]
        T3ContextMenu.saveText(["generation": 3, "suggestedName": "aurora/test.json", "text": "{\"version\":1}\n"], exportsRoot: root) { reply = $0; done.fulfill() }
        wait(for: [done], timeout: 2)
        XCTAssertEqual(reply["ok"] as? Bool, true)
        XCTAssertEqual((reply["value"] as? [String: Any])?["name"] as? String, "aurora-test.json")
        XCTAssertEqual(try String(contentsOf: root.appendingPathComponent("aurora-test.json"), encoding: .utf8), "{\"version\":1}\n")
        let refused = expectation(description: "refused")
        T3ContextMenu.saveText(["generation": 4, "suggestedName": "x.json"], exportsRoot: root) { reply = $0; refused.fulfill() }
        wait(for: [refused], timeout: 2)
        XCTAssertEqual(reply["ok"] as? Bool, false)
    }

    func testImportReadsJsonFilesFromTheIsolatedDirectory() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-contextmenu-\(UUID().uuidString)/imports", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try "{\"name\":\"B\"}".write(to: root.appendingPathComponent("b.json"), atomically: true, encoding: .utf8)
        try "{\"name\":\"A\"}".write(to: root.appendingPathComponent("a.json"), atomically: true, encoding: .utf8)
        try "ignored".write(to: root.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
        let done = expectation(description: "read")
        var reply: [String: Any] = [:]
        T3ContextMenu.openText(["generation": 1, "multiple": true], importsRoot: root) { reply = $0; done.fulfill() }
        wait(for: [done], timeout: 2)
        let files = (reply["value"] as? [String: Any])?["files"] as? [[String: Any]] ?? []
        XCTAssertEqual(files.map { $0["name"] as? String }, ["a.json", "b.json"])
        XCTAssertEqual(files.first?["text"] as? String, "{\"name\":\"A\"}")
    }

    /// pickThemeFiles: a 10 KB theme is read, a 300 KB one comes back unread with its size, an
    /// unreadable one with size 0; nothing picked answers cancelled.
    func testThemeFilesOverTheLimitOrUnreadableComeBackEmpty() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-themes-\(UUID().uuidString)/imports", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
        let small = "{\"name\":\"Small\",\"pad\":\"" + String(repeating: "x", count: 10_000) + "\"}"
        try small.write(to: root.appendingPathComponent("a-small.json"), atomically: true, encoding: .utf8)
        try String(repeating: " ", count: 300 * 1024).write(to: root.appendingPathComponent("b-big.json"), atomically: true, encoding: .utf8)
        let locked = root.appendingPathComponent("c-locked.json")
        try "{}".write(to: locked, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o000], ofItemAtPath: locked.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: locked.path) }
        let done = expectation(description: "read")
        var reply: [String: Any] = [:]
        T3ContextMenu.openText(["generation": 1], importsRoot: root) { reply = $0; done.fulfill() }
        wait(for: [done], timeout: 2)
        let value = reply["value"] as? [String: Any] ?? [:]
        let files = value["files"] as? [[String: Any]] ?? []
        XCTAssertEqual(files.map { $0["name"] as? String }, ["a-small.json", "b-big.json", "c-locked.json"])
        XCTAssertEqual(files[0]["text"] as? String, small)
        XCTAssertEqual(files[0]["size"] as? Int, small.utf8.count)
        XCTAssertEqual(files[1]["size"] as? Int, 300 * 1024)
        XCTAssertEqual(files[1]["text"] as? String, "")
        XCTAssertEqual(files[2]["size"] as? Int, 0)
        XCTAssertEqual(files[2]["text"] as? String, "")
        XCTAssertEqual(value["cancelled"] as? Bool, false)
        let empty = root.deletingLastPathComponent().appendingPathComponent("none", isDirectory: true)
        let none = expectation(description: "none")
        T3ContextMenu.openText(["generation": 2], importsRoot: empty) { reply = $0; none.fulfill() }
        wait(for: [none], timeout: 2)
        XCTAssertEqual((reply["value"] as? [String: Any])?["cancelled"] as? Bool, true)
    }
    /// The panel opens in ~/.vscode/extensions when it exists, and anywhere otherwise (a scratch HOME).
    func testThemePanelStartsInTheVsCodeExtensionsFolder() throws {
        let home = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-home-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: home) }
        XCTAssertNil(T3ContextMenu.themeStartDirectory(home: home.path))
        try FileManager.default.createDirectory(at: home.appendingPathComponent(".vscode/extensions"), withIntermediateDirectories: true)
        XCTAssertEqual(T3ContextMenu.themeStartDirectory(home: home.path)?.standardizedFileURL.path, home.appendingPathComponent(".vscode/extensions").standardizedFileURL.path)
        let panel = T3ContextMenu.themePanel()
        XCTAssertTrue(panel.allowsMultipleSelection)
        XCTAssertFalse(panel.canChooseDirectories)
        XCTAssertEqual(panel.allowedContentTypes, [.json])
    }

    func testOpenVsxReadsRefuseOtherHosts() {
        let done = expectation(description: "refused")
        var reply: [String: Any] = [:]
        T3ContextMenu.fetchText(["generation": 2, "url": "https://example.com/api"]) { reply = $0; done.fulfill() }
        wait(for: [done], timeout: 2)
        XCTAssertEqual((reply["value"] as? [String: Any])?["message"] as? String, "Only Open VSX can be searched.")
    }
}

let suite = XCTestSuite(name: "Context menus, tab input, file menus and the shell's menu")
suite.addTest(XCTestSuite(forTestCaseClass: ContextMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: TabInputTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: FileMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: TextContextMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ShellPageMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ShellTextMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ShellTemplateTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ShellWebMenuTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: RealInput1010gTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: RealInput1010gWebTests.self))
suite.run()
let run = suite.testRun!
print("context menu tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
