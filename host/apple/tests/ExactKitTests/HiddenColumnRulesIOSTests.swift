import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A hidden multi-column container paints no column rules. Linux skips them
/// when the container itself does not paint. Showing it again puts them back.
final class HiddenColumnRulesIOSTests: XCTestCase {
    #if os(macOS)
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil; super.tearDown() }
    #else
    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }
    #endif

    private func rules(_ node: NodeView) -> CALayer? {
        #if os(macOS)
        node.layer?.sublayers?.first { $0.name == "exact-column-rules" }
        #else
        node.layer.sublayers?.first { $0.name == "exact-column-rules" }
        #endif
    }

    func testAHiddenMulticolumnContainerDropsItsColumnRules() throws {
        let p = Presenter()
        #if os(macOS)
        _ = NSApplication.shared
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 120)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        #else
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 120))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        #endif
        let shown: [String: Any] = ["column_rule_style": "solid", "column_rule_width": 4]
        let columns: [[Double]] = [[0, 0, 40, 80, 1], [52, 0, 40, 80, 1]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": shown],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 120.0, "h": 80.0],
            ["op": "fragments", "id": 1, "columns": columns],
        ]))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertNotNil(rules(node), "two holding columns install a rule layer")
        var hidden = shown
        hidden["visibility"] = "hidden"
        p.apply(wireBatch([["op": "style", "id": 1, "style": hidden]]))
        XCTAssertTrue(node.cssVisibilityHidden)
        XCTAssertEqual(node.style["column_rule_style"]?.string, "solid")
        XCTAssertNil(rules(node), "a hidden container still shows its column rules")
        var back = shown
        back["visibility"] = "visible"
        p.apply(wireBatch([["op": "style", "id": 1, "style": back]]))
        XCTAssertNotNil(rules(node), "showing the container installs the rules again")
    }
}
