// A controlled value written into an editor replaces only what changed and
// carries the selection through (LLP 1045 D5), on both editors.
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

final class TextValueTests: XCTestCase {
    func testMinimalEditReplacesOnlyTheChangedMiddle() {
        XCTAssertNil(minimalTextEdit(from: "same", to: "same"))
        let e = minimalTextEdit(from: "hello world", to: "hello there world")!
        XCTAssertEqual(e.range, NSRange(location: 6, length: 0)); XCTAssertEqual(e.text, "there ")
        let d = minimalTextEdit(from: "abcdef", to: "abef")!
        XCTAssertEqual(d.range, NSRange(location: 2, length: 2)); XCTAssertEqual(d.text, "")
        let r = minimalTextEdit(from: "", to: "new")!
        XCTAssertEqual(r.range, NSRange(location: 0, length: 0)); XCTAssertEqual(r.text, "new")
        let w = minimalTextEdit(from: "gone", to: "")!
        XCTAssertEqual(w.range, NSRange(location: 0, length: 4))
    }

    func testMinimalEditNeverSplitsASurrogatePair() {
        // 🎉 is two UTF-16 units; changing it to 🎈 shares a lead surrogate.
        let e = minimalTextEdit(from: "a🎉b", to: "a🎈b")!
        XCTAssertEqual(e.range, NSRange(location: 1, length: 2)); XCTAssertEqual(e.text, "🎈")
        let f = minimalTextEdit(from: "🎉🎉", to: "🎉🎈")!
        XCTAssertEqual(f.range, NSRange(location: 2, length: 2))
    }

    func testCanonicallyEquivalentSourceStillUpdatesItsExactCodeUnits() {
        let edit = minimalTextEdit(from: "caf\u{e9}", to: "cafe\u{301}")
        XCTAssertEqual(edit?.range, NSRange(location: 3, length: 1))
        XCTAssertEqual(Array(edit!.text.utf16), [101, 769])
    }

    func testSelectionCarriesThroughAnEdit() {
        let insert = (range: NSRange(location: 2, length: 0), text: "XY")
        XCTAssertEqual(carrySelection(NSRange(location: 1, length: 0), through: insert), NSRange(location: 1, length: 0))
        XCTAssertEqual(carrySelection(NSRange(location: 5, length: 2), through: insert), NSRange(location: 7, length: 2))
        let replace = (range: NSRange(location: 2, length: 4), text: "Q")
        XCTAssertEqual(carrySelection(NSRange(location: 4, length: 0), through: replace), NSRange(location: 3, length: 0))
        XCTAssertEqual(carrySelection(NSRange(location: 1, length: 8), through: replace), NSRange(location: 1, length: 5))
    }

    #if os(macOS)
    func testTextAreaKeepsItsCaretWhenTheAppEditsElsewhere() {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 3, kind: "textarea", presenter: p)
        node.makeTextArea()
        let f = node.textArea!
        node.props["value"] = "hello world"; node.applyTextArea()
        f.setSelectedRange(NSRange(location: 5, length: 0))
        node.props["value"] = "hello world!"; node.applyTextArea()
        XCTAssertEqual(f.string, "hello world!")
        XCTAssertEqual(f.selectedRange(), NSRange(location: 5, length: 0))
        node.props["value"] = "Well, hello world!"; node.applyTextArea()
        XCTAssertEqual(f.selectedRange(), NSRange(location: 11, length: 0))
    }
    #endif
}
