#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1114.007 on UIKit: a textarea's decorations paint over its TextKit 2
/// layout, never in its storage; `rangeFrame` reads the field's own layout.
///   bun host/apple/build.mjs --test --ios
final class FieldDecorationsIOSTests: XCTestCase {
    private var window: UIWindow!

    // Style rows by kernel style id: text_color 75, text_decoration_line 71.
    private func presenter(_ value: String, spans: [(String, Int, Int)]) throws -> (Presenter, NodeView, TextArea) {
        let record = String(decoding: try JSONSerialization.data(withJSONObject: ["value": value, "spans": spans.map { ["name": $0.0, "start": $0.1, "end": $0.2] as [String: Any] }]), as: UTF8.self)
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "composer", "value": value, "decorations": record, "decorationStyles": ##"{"mention":[[75,"#0000ff"],[71,"underline"]]}"##], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 120.0],
        ]))
        let node = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(node.textArea as? TextArea)
        node.restyleDecorations()
        return (p, node, field)
    }

    func testSpansAreRenderingAttributesAndTheStorageIsUntouched() throws {
        let value = "hi @alice end"
        let (p, node, field) = try presenter(value, spans: [("mention", 3, 9)])
        let manager = try XCTUnwrap(field.textLayoutManager), content = try XCTUnwrap(manager.textContentManager)
        let at = try XCTUnwrap(content.location(content.documentRange.location, offsetBy: 4))
        var painted: [NSAttributedString.Key: Any] = [:]
        manager.enumerateRenderingAttributes(from: content.documentRange.location, reverse: false) { _, attributes, range in
            if range.contains(at) { painted = attributes; return false }
            return true
        }
        XCTAssertNotNil(painted[.foregroundColor])
        XCTAssertEqual(field.decorations?.drawsLines, true)
        XCTAssertNil(field.textStorage.attribute(.underlineStyle, at: 4, effectiveRange: nil))
        XCTAssertEqual(field.text, value)
        XCTAssertFalse(field.undoManager?.canUndo ?? false)
        _ = field.becomeFirstResponder()
        field.selectedRange = NSRange(location: 0, length: 0)
        field.insertText("!")
        node.restyleDecorations()
        XCTAssertEqual(field.decorations?.state["provisional"] as? Bool, true)
        field.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        XCTAssertNil(p.rangeFrame(2, start: 0, end: 1), "no range is read while text is marked")
        field.unmarkText()
    }

    func testRangeFrameIsTheCaretWhenCollapsedAndTheUnionOtherwise() throws {
        let value = "hi @alice and more text that wraps onto the next line of the field"
        let (p, _, _) = try presenter(value, spans: [])
        let caret = try XCTUnwrap(p.rangeFrame(2, start: 3, end: 3))
        XCTAssertEqual(caret.width, 0)
        XCTAssertGreaterThan(caret.height, 8)
        let word = try XCTUnwrap(p.rangeFrame(2, start: 3, end: 9))
        XCTAssertGreaterThan(word.width, 20)
        let all = try XCTUnwrap(p.rangeFrame(2, start: 0, end: UInt32(value.utf16.count)))
        XCTAssertGreaterThan(all.height, word.height * 1.5)
        XCTAssertNil(p.rangeFrame(2, start: 0, end: UInt32(value.utf16.count + 1)))
    }
}
#endif
