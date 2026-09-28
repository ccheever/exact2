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
        node.handlers = ["input"] // each source change; `change` is the commit (LLP 1069.001)
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
        presenter.onInput = { _, value in changes.append(value) }
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

    func testLinkAttributesGateNavigationWithoutChangingSource() {
        let storage = NSTextStorage(string: "")
        let editor = MarkupEditor()
        let look = MarkupEditor.Look(font: { size, _, _, _ in NSFont.systemFont(ofSize: size) },
                                     size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, ink: .textColor)
        for href in ["https://example.test/path", "javascript:probe", "data:text/plain,probe", "file:///tmp/probe", "/relative"] {
            let source = "[visible](\(href))"
            storage.mutableString.setString(source)
            editor.restyle(storage, selection: NSRange(location: 0, length: 0), look: look)
            XCTAssertEqual(storage.string, source)
            var links: [URL] = []
            storage.enumerateAttribute(.link, in: NSRange(location: 0, length: storage.length)) { value, _, _ in
                if let url = value as? URL { links.append(url) }
            }
            XCTAssertEqual(links.isEmpty, !href.hasPrefix("https:"), href)
            XCTAssertTrue(links.allSatisfy { $0.absoluteString == href })
        }
    }

    func testControlledValueAndRestylingKeepTheNativeUndoTransaction() {
        let (node, f, window, presenter) = editor("hello", selection: NSRange(location: 0, length: 5))
        let session = ExactApp.shared.makeSession(label: "markup-undo")
        presenter.session = session
        defer { window.close(); session.destroy() }
        var reported: [String] = []
        presenter.onInput = { _, value in
            reported.append(value)
            node.props["value"] = value
            node.applyTextArea()
            node.styleTextArea()
        }
        XCTAssertTrue(node.formatMarkup("bold"))
        XCTAssertEqual(f.string, "**hello**")
        XCTAssertTrue(f.undoManager?.canUndo == true)
        let undo = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
                                    windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        XCTAssertTrue(f.performKeyEquivalent(with: undo))
        XCTAssertEqual(f.string, "hello")
        let redo = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command, .shift], timestamp: 0,
                                    windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        XCTAssertTrue(f.performKeyEquivalent(with: redo))
        XCTAssertEqual(f.string, "**hello**")
        XCTAssertEqual(reported, ["**hello**", "hello", "**hello**"], "undo and redo each publish one source change")
    }

    func testExternalSourceWriteResetsOnlyItsEditorsNativeHistory() {
        let (node, f, window, presenter) = editor("hello world", selection: NSRange(location: 0, length: 5))
        defer { window.close(); _ = presenter }
        let other = TextArea(usingTextLayoutManager: true)
        other.allowsUndo = true
        other.insertText("other", replacementRange: NSRange(location: 0, length: 0))
        XCTAssertTrue(other.undoManager?.canUndo == true)
        XCTAssertTrue(node.formatMarkup("bold"))
        node.writeValue(f.string, into: f)
        XCTAssertTrue(f.undoManager?.canUndo == true, "a controlled echo must retain undo")
        node.writeValue("PREFIX **hello** world", into: f)
        XCTAssertFalse(f.undoManager?.canUndo == true)
        XCTAssertFalse(f.undoManager?.canRedo == true)
        XCTAssertEqual(f.string, "PREFIX **hello** world")
        XCTAssertTrue(other.undoManager?.canUndo == true, "another editor retains its history")
        other.undoManager?.undo()
        XCTAssertEqual(other.string, "")
        f.insertText("!", replacementRange: NSRange(location: (f.string as NSString).length, length: 0))
        XCTAssertTrue(f.undoManager?.canUndo == true)
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "PREFIX **hello** world")
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
        let linkField = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 30))
        node.addSubview(linkField)
        window.makeFirstResponder(linkField)
        f.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertTrue(node.formatMarkup("link", argument: "https://example.com"))
        XCTAssertEqual(f.string, "first [second](https://example.com)")
        XCTAssertTrue(window.firstResponder === f)
        XCTAssertTrue(f.undoManager?.canUndo == true, "a bookmarked format must restore editing ownership for native undo")
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "first second")
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
