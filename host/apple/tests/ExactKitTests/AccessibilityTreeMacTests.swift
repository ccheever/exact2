#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// `tree --ax` on AppKit (LLP 1080.002 §3 stage 3): the unignored tree in
/// navigation order, AppKit's own roles and names, each element joined to
/// its view. Run by `bun host/apple/build.mjs --test`.
final class AccessibilityTreeMacTests: XCTestCase {
    private var window: NSWindow!

    private func fixture() throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["testId": "named", "accessibilityLabel": "Play"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["testId": "bare"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 40.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        return p
    }
    private func elements(_ ax: [String: Any]) -> [[String: Any]] { ax["elements"] as? [[String: Any]] ?? [] }

    func testElementsAreAppKitsUnignoredTreeJoinedToTheirViews() throws {
        let p = try fixture()
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertEqual(ax["source"] as? String, "appkit")
        XCTAssertEqual(ax["order"] as? String, "navigation")
        let named = try XCTUnwrap(elements(ax).first { $0["testId"] as? String == "named" })
        XCTAssertEqual(named["role"] as? String, "button")
        XCTAssertEqual((named["native"] as? [String: Any])?["role"] as? String, "AXButton")
        XCTAssertEqual(named["name"] as? String, "Play")
        XCTAssertEqual(named["via"] as? String, "self")
        XCTAssertNotNil(named["frame"])
        let order = elements(ax).compactMap { $0["testId"] as? String }
        XCTAssertEqual(order.firstIndex(of: "named").map { $0 < (order.firstIndex(of: "bare") ?? 0) }, true)
    }
}
#endif
