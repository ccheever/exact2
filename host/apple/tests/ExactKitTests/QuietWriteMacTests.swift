#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1045 D5: the app's write into the focused textarea lands while the
/// text view is no accessibility element, so AppKit posts nothing VoiceOver
/// would read; it is one again, with the new text, once the write is in.
final class QuietWriteMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    func testAWriteIntoTheFocusedTextareaLeavesItAnElementWithTheNewText() throws {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": "Alpha draft about the parser", "accessibilityLabel": "Message"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 120],
        ]))
        let area = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(area))
        for value in ["Bravo draft for the second thread", "", "Alpha draft about the parser"] {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": value]]]))
            XCTAssertEqual(area.string, value)
            XCTAssertEqual(area.accessibilityValue(), value)
            XCTAssertTrue(area.isAccessibilityElement(), "an element again after the write of \"\(value)\"")
            XCTAssertTrue(window.firstResponder === area, "the write keeps the focus")
        }
    }
}
#endif
