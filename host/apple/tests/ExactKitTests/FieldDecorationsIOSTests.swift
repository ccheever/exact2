#if os(iOS)
import UIKit
import XCTest
import ExactCore
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

    /// A range's box is its text's (the font's content area at the
    /// baseline TextKit lays the line on), not the line's, which a tall
    /// line height grows: the grammar's, as Chrome and Linux answer
    /// (theme2's 22:27 ruling). The baseline is read here from the layout
    /// fragment's line, apart from the reader's segment, on a hard line and
    /// on a soft-wrapped one; an empty last line's caret is its own line's.
    func testRangeFrameIsTheTextsBoxUnderATallLine() throws {
        let value = "abc def\nghi and a line long enough to wrap in the field at least once\n"
        let (p, _, field) = try presenter(value, spans: [])
        // The field's font as a styled one has it (this fixture sets none).
        field.font = UIFont.systemFont(ofSize: 16)
        let storage = field.textStorage
        let face = try XCTUnwrap(storage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont ?? field.font)
        let (ascent, descent) = CSSLineBox.content(face as CTFont)
        let manager = try XCTUnwrap(field.textLayoutManager), content = try XCTUnwrap(manager.textContentManager)
        // The viewport top of the text box on the line holding `offset`.
        func expected(_ offset: Int) throws -> CGFloat {
            let at = try XCTUnwrap(content.location(content.documentRange.location, offsetBy: offset))
            manager.ensureLayout(for: content.documentRange)
            let fragment = try XCTUnwrap(manager.textLayoutFragment(for: at))
            let local = content.offset(from: try XCTUnwrap(fragment.rangeInElement.location as NSTextLocation?), to: at)
            let line = try XCTUnwrap(fragment.textLineFragments.first { NSLocationInRange(local, $0.characterRange) || local == NSMaxRange($0.characterRange) } ?? fragment.textLineFragments.last)
            let baseline = fragment.layoutFragmentFrame.minY + line.typographicBounds.minY + line.glyphOrigin.y
            let origin = CGPoint(x: field.textContainerInset.left, y: field.textContainerInset.top)
            return Agent.box(CGRect(x: 0, y: baseline - ascent + origin.y, width: 1, height: 1), of: field, viewport: p.viewport).minY
        }
        for tall in [false, true] {
            if tall {
                let style = NSMutableParagraphStyle(); style.minimumLineHeight = 40; style.maximumLineHeight = 40
                storage.addAttribute(.paragraphStyle, value: style, range: NSRange(location: 0, length: value.utf16.count))
                field.layoutIfNeeded()
            }
            let caret = try XCTUnwrap(p.rangeFrame(2, start: 1, end: 1)), word = try XCTUnwrap(p.rangeFrame(2, start: 0, end: 3))
            XCTAssertEqual(caret.width, 0)
            XCTAssertEqual(caret.height, ascent + descent, accuracy: 0.01, "tall \(tall): \(caret)")
            XCTAssertEqual(word.height, ascent + descent, accuracy: 0.01, "tall \(tall): \(word)")
            XCTAssertEqual(caret.minY, try expected(1), accuracy: 0.5, "tall \(tall): on line 1's baseline")
            XCTAssertEqual(word.minY, caret.minY, accuracy: 0.01)
            // The last word of the wrapped line, and the last empty line.
            let wrapped = value.utf16.count - 6, end = value.utf16.count
            let late = try XCTUnwrap(p.rangeFrame(2, start: UInt32(wrapped), end: UInt32(wrapped)))
            XCTAssertEqual(late.minY, try expected(wrapped), accuracy: 0.5, "tall \(tall): a wrapped line's baseline")
            XCTAssertGreaterThan(late.minY, caret.minY + (tall ? 70 : 25), "below the hard break and the wrap")
            let last = try XCTUnwrap(p.rangeFrame(2, start: UInt32(end), end: UInt32(end)))
            XCTAssertGreaterThan(last.minY, late.minY, "the empty last line")
            XCTAssertEqual(last.height, ascent + descent, accuracy: 0.01)
        }
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
