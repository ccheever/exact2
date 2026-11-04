// @ref LLP 1069.001: a selection a script sets as a press focuses a field.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class FocusPressSelectionMacTests: XCTestCase {
    /// A press that focuses a field whose `focus` handler selects its text
    /// leaves the text selected, as Chrome does (an address bar's `onFocus`
    /// select). The steps are a hand's press as AppKit runs it: the window
    /// makes the field first responder in the press's event, the handler's
    /// batch selects, then the editor's tracking puts the caret at the press.
    /// The test takes each step by hand, as AppKit's tracking loop does not
    /// run in a test process. A drag keeps what it selected, and a press on
    /// the field once it has the focus places the caret.
    @MainActor
    func testAPressThatFocusesKeepsTheFocusHandlersSelection() throws {
        let f = try SheetFixture(page: """
        component Proof
          action focusField(on: bool)
            if on
              selectText("address")
          view
            column width="100%" height="100%" padding=20
              input id="address" testId="address" value="http://a.test/page" focus=focusField(true) width=240 height=24 border-width=0
              input id="other" testId="other" value="" width=240 height=24 border-width=0
        """, size: CGSize(width: 420, height: 200))
        defer { f.close() }
        let node = try f.node("address"), field = try XCTUnwrap(node.field), other = try XCTUnwrap(f.node("other").field)
        let fields = f.presenter.fieldSelections, all = NSRange(location: 0, length: ("http://a.test/page" as NSString).length)
        var time = 1.0
        func press(placing range: NSRange) throws {
            time += 1
            let down = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: .zero, modifierFlags: [], timestamp: time,
                                                        windowNumber: f.window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
            if field.currentEditor() == nil {
                fields.focused(node, by: down) // as the field's becomeFirstResponder notes it, before its focus event
                XCTAssertTrue(f.window.makeFirstResponder(field))
            }
            f.settle() // the focus handler's batch: its selectText
            fields.pressing(node, down) { (field.currentEditor() as? NSTextView)?.setSelectedRange(range) }
            f.settle()
        }
        try press(placing: NSRange(location: 10, length: 0))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertEqual(editor.selectedRange(), all, "the focus handler's selection, not the press's caret")
        XCTAssertEqual(fields.current(node).range, all)
        try press(placing: NSRange(location: 10, length: 0))
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 10, length: 0), "a press in the focused field places the caret")
        XCTAssertTrue(f.window.makeFirstResponder(other))
        f.settle()
        try press(placing: NSRange(location: 7, length: 6))
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 7, length: 6), "a drag that focuses selects what it covers")
    }
}
#endif
