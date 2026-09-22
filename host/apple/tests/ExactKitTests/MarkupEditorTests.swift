// Native source/undo/composition behaviours that Rust edit tests cannot see.
// @ref LLP 1045 D1, D5, D6 — no claim about physical keyboard/phone gestures.
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit

final class MarkupEditorTests: XCTestCase {
    private func editor(_ source: String, selection: NSRange) -> (NodeView, TextArea, NSWindow, Presenter) {
        _ = NSApplication.shared
        let presenter = Presenter()
        let node = NodeView(id: 3, kind: "textarea", presenter: presenter)
        node.frame = NSRect(x: 0, y: 0, width: 500, height: 300)
        node.makeTextArea()
        node.props["markup"] = "markdown"
        node.props["value"] = source
        node.handlers = ["change"]
        node.applyTextArea()
        node.layoutTextArea()
        let window = NSWindow(contentRect: node.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = node
        let f = node.textArea as! TextArea
        window.makeFirstResponder(f)
        f.setSelectedRange(selection)
        f.undoManager?.removeAllActions()
        return (node, f, window, presenter)
    }

    func testFormattingIsOneSourceChangeAndNativeUndoRedo() {
        let (node, f, window, presenter) = editor("hello 🎉", selection: NSRange(location: 0, length: 5))
        defer { window.close() }
        var changes: [String] = []
        presenter.onChange = { _, value in changes.append(value) }
        XCTAssertNotNil(f.textLayoutManager)
        XCTAssertTrue(node.formatMarkup("bold"))
        XCTAssertEqual(f.string, "**hello** 🎉")
        XCTAssertEqual(changes, ["**hello** 🎉"])
        XCTAssertEqual(f.selectedRange(), NSRange(location: 2, length: 5))
        XCTAssertEqual(f.markup?.bookmark, f.selectedRange())
        XCTAssertTrue(f.undoManager?.canUndo == true)
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "hello 🎉")
        f.undoManager?.redo()
        XCTAssertEqual(f.string, "**hello** 🎉")
        XCTAssertNotNil(f.textLayoutManager)
    }

    func testListReturnAndExitAreEachOneNativeUndoGroup() {
        let (node, f, window, presenter) = editor("- item", selection: NSRange(location: 6, length: 0))
        defer { window.close(); _ = presenter }
        f.insertNewline(nil)
        XCTAssertEqual(f.string, "- item\n- ")
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "- item")
        f.undoManager?.redo()
        XCTAssertEqual(f.string, "- item\n- ")
        XCTAssertTrue(node.formatMarkup("newline"))
        XCTAssertEqual(f.string, "- item\n")
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "- item\n- ")
    }

    func testLinkCommandUsesBookmarkAfterFocusMoves() {
        let (node, f, window, presenter) = editor("first second", selection: NSRange(location: 6, length: 6))
        defer { window.close(); _ = presenter }
        window.makeFirstResponder(nil)
        f.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertTrue(node.formatMarkup("link", argument: "https://example.com"))
        XCTAssertEqual(f.string, "first [second](https://example.com)")
    }

    func testMarkedTextDefersValueAndRefusesFormatting() {
        let (node, f, window, presenter) = editor("hello", selection: NSRange(location: 5, length: 0))
        defer { window.close(); _ = presenter }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 5, length: 0))
        XCTAssertTrue(f.hasMarkedText())
        let composing = f.string
        node.props["value"] = "external"
        node.applyTextArea()
        XCTAssertEqual(f.string, composing)
        XCTAssertEqual(node.pendingValue, "external")
        XCTAssertFalse(node.formatMarkup("bold"))
        f.unmarkText()
        node.textDidChange(Notification(name: NSText.didChangeNotification, object: f))
        XCTAssertEqual(f.string, "external")
        XCTAssertNil(node.pendingValue)
    }

    func testCompositionEchoDoesNotLeaveAStaleWriteback() {
        let (node, f, window, presenter) = editor("", selection: NSRange(location: 0, length: 0))
        defer { window.close(); _ = presenter }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 0, length: 0))
        node.writeValue(f.string, into: f)
        XCTAssertNil(node.pendingValue)
        f.unmarkText()
        f.insertText("本", replacementRange: NSRange(location: 1, length: 0))
        XCTAssertEqual(f.string, "に本")
    }

    func testStylingRetainsEverySourceCharacterAndMarkerAdvance() {
        let source = "# Heading\n\n**bold** and [link](https://example.com)\n- item"
        let storage = NSTextStorage(string: source)
        let editor = MarkupEditor()
        let look = MarkupEditor.Look(font: { size, weight, _, _ in NSFont.systemFont(ofSize: size, weight: weight >= 700 ? .bold : .regular) },
                                     size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, ink: .textColor)
        editor.restyle(storage, selection: NSRange(location: 5, length: 0), look: look)
        XCTAssertEqual(storage.string, source)
        storage.enumerateAttribute(.font, in: NSRange(location: 0, length: storage.length)) { value, _, _ in
            XCTAssertGreaterThan((value as? NSFont)?.pointSize ?? 0, 1)
        }
        let linkRange = (source as NSString).range(of: "link")
        XCTAssertEqual((storage.attribute(.link, at: linkRange.location, effectiveRange: nil) as? URL)?.absoluteString, "https://example.com")
    }

    func testRemovingMarkupRestoresPlainAttributesAndCommands() {
        let (node, f, window, presenter) = editor("**bold** [link](https://example.com)", selection: NSRange(location: 3, length: 0))
        defer { window.close(); _ = presenter }
        let look = MarkupEditor.Look(font: { size, weight, _, _ in NSFont.systemFont(ofSize: size, weight: weight >= 700 ? .bold : .regular) },
                                     size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, ink: .textColor)
        f.markup!.restyle(f.textStorage!, selection: f.selectedRange(), look: look)
        XCTAssertNotNil(f.textStorage!.attribute(.link, at: 10, effectiveRange: nil))
        node.props["markup"] = "none"
        node.applyTextArea()
        XCTAssertNil(f.markup)
        XCTAssertNil(f.textStorage!.attribute(.link, at: 10, effectiveRange: nil))
        XCTAssertFalse(node.formatMarkup("italic"))
        XCTAssertEqual(f.string, "**bold** [link](https://example.com)")
        node.props["markup"] = "markdown"
        node.applyTextArea()
        XCTAssertNotNil(f.markup)
        XCTAssertTrue(node.formatMarkup("italic"))
    }

    func testCommandSeamAndStateUseUTF16AroundEmoji() throws {
        let source = "🎉 cafe\u{301}"
        let result = try XCTUnwrap(MarkupCommands.edit(source, selection: NSRange(location: 3, length: 5), command: "italic"))
        XCTAssertEqual(result.source, "🎉 *cafe\u{301}*")
        let state = try XCTUnwrap(MarkupCommands.selection(result.source, range: result.selection))
        let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(state.utf8)) as? [String: Any])
        XCTAssertTrue((object["formats"] as? String)?.split(separator: " ").contains("italic") == true)
        XCTAssertEqual(MarkupCommands.plain("**source** [link](https://example.com)"), "source link")
    }

    func testCommandSeamRejectsBrokenRangesInsteadOfSplittingSurrogates() {
        XCTAssertNil(MarkupEdit(source: "🎉", object: ["replacements": [[1, 2, "x"]], "selection": [0, 0]]))
        XCTAssertNil(MarkupEdit(source: "abcd", object: ["replacements": [[1, 3, "x"], [2, 4, "y"]], "selection": [0, 0]]))
    }
}
#endif
