#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// x2apps codeedit #2 on UIKit: a field's `input` reports the selection its
/// edit left; `setSelectionRange` fires `select`, never focuses, and waits
/// for an input's focus when it has none; a person's non-collapsed selection
/// is a `select`, typing is not. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class FieldSelectionIOSTests: XCTestCase {
    private var window: UIWindow!
    private var selects: [String] = []

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.fieldSelections.onSelect = { [unowned self] id, value, s in selects.append("\(id) \(value) \(s.start)-\(s.end) \(s.direction)") }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 30.0],
        ]))
        return p
    }

    func testATextViewTakesTheScriptsRangeAndTypingReportsTheCaret() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects, ["2 hello 1-3 backward"])
        XCTAssertFalse(editor.isFirstResponder, "setSelectionRange does not focus")
        XCTAssertTrue(editor.becomeFirstResponder())
        XCTAssertEqual(editor.selectedRange, NSRange(location: 1, length: 2))
        editor.insertText("x")
        XCTAssertEqual(editor.text, "hxlo")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.text), FieldSelection(start: 2, end: 2))
        XCTAssertEqual(selects.count, 1, "typing is no select")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.last, "2 hxlo 2-2 none")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.count, 2, "the same selection again: nothing")
        editor.selectedRange = NSRange(location: 2, length: 1)
        XCTAssertEqual(selects.last, "2 hxlo 2-3 forward", "a person's extension is a select")
    }

    func testAnInputWaitsForItsFocus() throws {
        let p = presenter()
        let field = try XCTUnwrap(p.views[3]?.field)
        p.fieldSelections.setSelectionRange(["line", 1, 4, "forward"])
        XCTAssertEqual(selects, ["3 hello 1-4 forward"])
        XCTAssertTrue(field.becomeFirstResponder())
        let range = try XCTUnwrap(field.selectedTextRange)
        XCTAssertEqual(field.offset(from: field.beginningOfDocument, to: range.start), 1)
        XCTAssertEqual(field.offset(from: field.beginningOfDocument, to: range.end), 4)
        field.insertText("y")
        XCTAssertEqual(field.text, "hyo")
        XCTAssertEqual(p.fieldSelections.reported(3, field.text ?? ""), FieldSelection(start: 2, end: 2))
        XCTAssertEqual(selects.count, 1)
    }
}
#endif
