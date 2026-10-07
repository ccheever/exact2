#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// x2apps codeedit #2 on AppKit: a text field's `input` reports the
/// selection its edit left; its `select` fires for the person's
/// non-collapsed selection and for a script's `setSelectionRange`, which
/// never focuses the field, waits for its focus when it has none, and is
/// clamped as HTML clamps it.
final class FieldSelectionMacTests: XCTestCase {
    private var window: NSWindow!
    private var selects: [String] = []

    override func tearDown() { window?.close(); window = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.fieldSelections.onSelect = { [unowned self] id, value, s in selects.append("\(id) \(value) \(s.start)-\(s.end) \(s.direction)") }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "box"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 140.0, "w": 30.0, "h": 24.0],
        ]))
        return p
    }

    func testTypingReportsTheCaretAndAScriptsRangeWaitsForTheFocus() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects, ["2 hello 1-3 backward"])
        XCTAssertFalse(window.firstResponder === editor, "setSelectionRange does not focus")
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects.count, 1, "the same selection set again fires nothing")
        window.makeFirstResponder(editor)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 1, length: 2))
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string), FieldSelection(start: 1, end: 3, direction: "backward"))
        editor.insertText("x", replacementRange: editor.selectedRange())
        XCTAssertEqual(editor.string, "hxlo")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string), FieldSelection(start: 2, end: 2), "typing leaves a caret, direction none")
        XCTAssertEqual(selects.count, 1, "typing is no select")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.last, "2 hxlo 2-2 none", "a script's caret is a select, even where the caret was")
        // A person's Shift-extension: right is forward, then back past the anchor is backward.
        editor.moveRightAndModifySelection(nil)
        XCTAssertEqual(selects.last, "2 hxlo 2-3 forward")
        editor.moveLeftAndModifySelection(nil)
        XCTAssertEqual(selects.count, 3, "back to a caret: no select")
        editor.moveLeftAndModifySelection(nil)
        XCTAssertEqual(selects.last, "2 hxlo 1-2 backward")
        editor.moveRight(nil)
        XCTAssertEqual(selects.count, 4, "a caret move is no select")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string)?.direction, "none")
    }

    func testAnInputsFieldEditorTakesTheScriptsRangeNotItsSelectAll() throws {
        let p = presenter()
        let field = try XCTUnwrap(p.views[3]?.field)
        // ToUint32: -1 is past the end, clamped to the length.
        p.fieldSelections.setSelectionRange(["line", 1, -1])
        XCTAssertEqual(selects, ["3 hello 1-5 none"])
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 1, length: 4))
        XCTAssertEqual(selects.count, 1, "the focus's own selection is no select")
        p.fieldSelections.setSelectionRange(["line", 9, 2, "forward"])
        XCTAssertEqual(selects.last, "3 hello 2-2 forward", "an end before the start moves the start to it")
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 2, length: 0))
        editor.insertText("y", replacementRange: editor.selectedRange())
        XCTAssertEqual(p.fieldSelections.reported(3, field.stringValue), FieldSelection(start: 3, end: 3))
        p.fieldSelections.selectAll(try XCTUnwrap(p.views[3]))
        XCTAssertEqual(selects.last, "3 heyllo 0-6 none", "selectText is the DOM's select()")
        // Refusals change nothing.
        p.fieldSelections.setSelectionRange(["box", 0, 1])
        p.fieldSelections.setSelectionRange(["missing", 0, 1])
        p.fieldSelections.setSelectionRange(["line"])
        XCTAssertEqual(selects.count, 3)
        XCTAssertNil(p.fieldSelections.reported(4, ""), "a box is no text field")
    }

    func testClampingIsHTMLs() {
        XCTAssertEqual(FieldSelection.clamped(-1, -1, nil, length: 4), FieldSelection(start: 4, end: 4))
        XCTAssertEqual(FieldSelection.clamped(.nan, .infinity, "sideways", length: 4), FieldSelection(start: 0, end: 0))
        XCTAssertEqual(FieldSelection.clamped(1.9, 3.2, "backward", length: 4), FieldSelection(start: 1, end: 3, direction: "backward"))
        XCTAssertEqual(FieldSelection(start: 1, end: 3, direction: "forward").payload("a,b\nc"), "1,3,forward,a,b\nc")
    }
}
#endif
