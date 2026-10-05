import AppKit
import XCTest

// Lane r11-upstream (95edeb753b): a draft row's context menu as T3Sidebar builds it
// from the client's template (r11-upstream-drafts.ts draftMenuItems through
// nativeTemplate, the shape r11-upstream.test.ts pins): Copy ▸ Path / Branch,
// Project settings, a separator, then the destructive Discard draft with the trash glyph.
final class R11UpstreamDraftMenuTests: XCTestCase {
    let template: [[String: Any]] = [
        ["type": "submenu", "id": "copy", "label": "Copy", "enabled": true, "destructive": false, "children": [
            ["type": "item", "id": "copy-path", "label": "Path", "enabled": true, "destructive": false],
            ["type": "item", "id": "copy-branch", "label": "Branch", "enabled": true, "destructive": false],
        ]],
        ["type": "item", "id": "project-settings", "label": "Project settings", "enabled": true, "destructive": false],
        ["type": "separator"],
        ["type": "item", "id": "discard", "label": "Discard draft", "enabled": true, "destructive": true],
    ]

    func testDraftMenuMirrorsTheReferenceItems() {
        _ = NSApplication.shared
        let sidebar = T3Sidebar(agent: true) { _ in }
        let menu = T3Sidebar.menu(template, target: sidebar)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Copy", "Project settings", "—", "Discard draft"])
        XCTAssertEqual(menu.items[0].submenu?.items.map(\.title), ["Path", "Branch"])
        XCTAssertNil(menu.items[0].representedObject, "Copy is a submenu, not a choice")
        XCTAssertNotNil(menu.items[3].image, "Discard draft carries the destructive trash glyph")
        XCTAssertNil(menu.items[1].image)
    }

    func testChoosingReportsTheDraftActionIds() {
        _ = NSApplication.shared
        let sidebar = T3Sidebar(agent: true) { _ in }
        let menu = T3Sidebar.menu(template, target: sidebar)
        menu.items[0].submenu!.performActionForItem(at: 1)
        XCTAssertEqual(sidebar.picked, "copy-branch")
        menu.performActionForItem(at: 3)
        XCTAssertEqual(sidebar.picked, "discard")
    }

    func testCopyWithNothingToCopyIsDisabled() {
        _ = NSApplication.shared
        let menu = T3Sidebar.menu([["type": "item", "id": "copy", "label": "Copy", "enabled": false, "destructive": false],
                                   ["type": "separator"], ["type": "item", "id": "discard", "label": "Discard draft", "enabled": true, "destructive": true]], target: nil)
        XCTAssertFalse(menu.items[0].isEnabled)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Copy", "—", "Discard draft"])
    }
}

let suite = XCTestSuite(forTestCaseClass: R11UpstreamDraftMenuTests.self)
suite.run()
let run = suite.testRun!
print("R11 upstream draft menu tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
