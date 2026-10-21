#if os(macOS)
import AppKit
import XCTest
import ExactCore
@testable import ExactKit

/// LLP 1114.007: a textarea's decorations paint over its TextKit 2 layout
/// (rendering attributes, and lines the field draws), never in its storage;
/// `rangeFrame` reads the field's own layout.
final class FieldDecorationsTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession!
    override func setUp() { _ = NSApplication.shared }
    override func tearDown() { window?.close(); session?.destroy(); window = nil; session = nil }

    // Style rows by kernel style id: text_color 75, background_color 47, text_decoration_line 71.
    private let styles = ##"{"mention":[[75,"#0000ff"],[71,"underline"]],"tag":[[47,"#ffff00"]]}"##
    private func record(_ value: String, _ spans: [(String, Int, Int)]) throws -> String {
        let spans = spans.map { ["name": $0.0, "start": $0.1, "end": $0.2] as [String: Any] }
        return String(decoding: try JSONSerialization.data(withJSONObject: ["value": value, "spans": spans]), as: UTF8.self)
    }
    private func fixture(_ value: String, record: String) throws -> (Presenter, NodeView, TextArea) {
        session = ExactApp.shared.makeSession(label: "decorations-test")
        let presenter = session.presenter
        presenter.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "composer", "value": value, "decorations": record, "decorationStyles": styles], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 200],
        ]))
        let node = try XCTUnwrap(presenter.views[2]), field = try XCTUnwrap(node.textArea as? TextArea)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = presenter.viewport
        window.makeFirstResponder(field)
        node.restyleDecorations()
        return (presenter, node, field)
    }
    private func painted(_ field: TextArea, at offset: Int) -> [NSAttributedString.Key: Any] {
        guard let manager = field.textLayoutManager, let content = manager.textContentManager,
              let at = content.location(content.documentRange.location, offsetBy: offset) else { return [:] }
        var found: [NSAttributedString.Key: Any] = [:]
        manager.enumerateRenderingAttributes(from: content.documentRange.location, reverse: false) { _, attributes, range in
            if range.contains(at) { found = attributes; return false }
            return true
        }
        return found
    }

    /// A range's box is its text's (the font's content area at the
    /// baseline TextKit lays the line on), not the line's, which a tall
    /// line height grows: the grammar's, as Chrome and Linux answer
    /// (theme2's 22:27 ruling). The baseline is read here from the layout
    /// fragment's line, apart from the reader's segment, on a hard line and
    /// on a soft-wrapped one; an empty last line's caret is its own line's.
    func testRangeFrameIsTheTextsBoxUnderATallLine() throws {
        let value = "abc def\nghi and a line long enough to wrap in the field at least once\n"
        let (p, _, field) = try fixture(value, record: record(value, []))
        let storage = try XCTUnwrap(field.textStorage)
        let face = try XCTUnwrap(storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont ?? field.font)
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
            let origin = field.textContainerOrigin
            return Agent.box(CGRect(x: 0, y: baseline - ascent + origin.y, width: 1, height: 1), of: field, viewport: p.viewport).minY
        }
        for tall in [false, true] {
            if tall {
                let style = NSMutableParagraphStyle(); style.minimumLineHeight = 40; style.maximumLineHeight = 40
                storage.addAttribute(.paragraphStyle, value: style, range: NSRange(location: 0, length: value.utf16.count))
                field.layoutSubtreeIfNeeded()
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

    func testSpansPaintAsRenderingAttributesAndLeaveTheStorageAndUndoAlone() throws {
        let value = "hi @alice #art"
        let (_, node, field) = try fixture(value, record: record(value, [("mention", 3, 9), ("tag", 10, 14)]))
        let storage = try XCTUnwrap(field.textStorage)
        let ink = storage.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor
        XCTAssertEqual(storage.attribute(.foregroundColor, at: 4, effectiveRange: nil) as? NSColor, ink, "the storage keeps the field's ink")
        XCTAssertNil(storage.attribute(.underlineStyle, at: 4, effectiveRange: nil))
        let mention = painted(field, at: 4)
        XCTAssertEqual((mention[.foregroundColor] as? NSColor)?.usingColorSpace(.sRGB)?.blueComponent, 1)
        XCTAssertEqual(field.decorations?.drawsLines, true, "TextKit 2 draws no rendering-attribute underline: the field draws it")
        XCTAssertNotNil(painted(field, at: 11)[.backgroundColor])
        XCTAssertTrue(painted(field, at: 1).isEmpty)
        XCTAssertEqual(field.string, value)
        XCTAssertFalse(field.undoManager?.canUndo ?? false, "painting makes no undo step")
        XCTAssertNil(field.typingAttributes[.underlineStyle])
        let state = try XCTUnwrap((node.textArea as? TextArea)?.decorations?.state)
        XCTAssertEqual(state["provisional"] as? Bool, false)
        XCTAssertEqual((state["spans"] as? [[String: Any]])?.count, 2)
    }

    func testTypingReconcilesProvisionallyAndCompositionIsNotRestyled() throws {
        let value = "hi @alice"
        let (_, node, field) = try fixture(value, record: record(value, [("mention", 3, 9)]))
        field.setSelectedRange(NSRange(location: 0, length: 0))
        field.insertText("!", replacementRange: NSRange(location: NSNotFound, length: 0))
        node.restyleDecorations()
        let decorations = try XCTUnwrap(field.decorations)
        XCTAssertEqual(decorations.state["provisional"] as? Bool, true)
        XCTAssertEqual(((decorations.state["spans"] as? [[String: Any]])?.first)?["start"] as? Int, 4)
        XCTAssertNotNil(painted(field, at: 5)[.foregroundColor])
        field.setSelectedRange(NSRange(location: field.string.utf16.count, length: 0))
        field.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        let before = decorations.state["source"] as? String
        node.props["decorations"] = try record(field.string, [])
        node.restyleDecorations()
        XCTAssertEqual(decorations.state["source"] as? String, before, "marked text is not restyled")
        XCTAssertNil(node.presenter?.rangeFrame(2, start: 0, end: 1), "no range is read while text is marked")
        field.unmarkText(); node.restyleDecorations()
        XCTAssertEqual(decorations.state["source"] as? String, field.string)
        XCTAssertTrue(painted(field, at: 5).isEmpty)
    }

    func testRangeFrameIsTheCaretWhenCollapsedAndTheUnionOtherwise() throws {
        let value = "hi @alice and more text that wraps onto the next line of the field"
        let (presenter, _, _) = try fixture(value, record: record(value, []))
        let caret = try XCTUnwrap(presenter.rangeFrame(2, start: 3, end: 3))
        XCTAssertEqual(caret.width, 0)
        XCTAssertGreaterThan(caret.height, 8)
        let word = try XCTUnwrap(presenter.rangeFrame(2, start: 3, end: 9))
        XCTAssertGreaterThan(word.width, 20)
        XCTAssertEqual(word.minX, caret.minX, accuracy: 0.5)
        let all = try XCTUnwrap(presenter.rangeFrame(2, start: 0, end: UInt32(value.utf16.count)))
        XCTAssertGreaterThan(all.height, word.height * 1.5, "a wrapped range is the union of its lines")
        XCTAssertNil(presenter.rangeFrame(2, start: 0, end: UInt32(value.utf16.count + 1)))
        XCTAssertNil(presenter.rangeFrame(2, start: 5, end: 3))
    }
}
#endif
