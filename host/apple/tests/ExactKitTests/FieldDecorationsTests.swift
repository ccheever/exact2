#if os(macOS)
import AppKit
import XCTest
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
