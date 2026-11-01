// Native source/undo/composition behaviours that Rust edit tests cannot see.
// @ref LLP 1045 D1, D5, D6 — no claim about physical keyboard/phone gestures.
import XCTest
@testable import ExactKit
@testable import ExactMarkdown
#if os(macOS)
import AppKit

final class MarkupEditorTests: XCTestCase {
    override func setUp() {
        super.setUp()
        ExactMarkdown.install() // LLP 1047.001 D4: the capability this tests
    }

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

    func testPlainTextAreaRoutesUndoRedoActionsAndChordsToItsOwnHistory() {
        let (node, field, window, presenter) = editor("", selection: NSRange(location: 0, length: 0))
        let session = ExactApp.shared.makeSession(label: "plain-undo")
        presenter.session = session
        defer { window.close(); session.destroy() }
        node.props["markup"] = "none"
        node.applyTextArea()
        XCTAssertNil(field.markup)
        var changes: [String] = []
        presenter.onInput = { _, value in changes.append(value) }
        let undo = NSMenuItem(title: "Undo", action: #selector(TextArea.undo(_:)), keyEquivalent: "z")
        let redo = NSMenuItem(title: "Redo", action: #selector(TextArea.redo(_:)), keyEquivalent: "z")
        XCTAssertFalse(field.validateMenuItem(undo))
        XCTAssertFalse(field.validateMenuItem(redo))
        field.insertText("hello", replacementRange: NSRange(location: 0, length: 0))
        field.breakUndoCoalescing()
        XCTAssertTrue(field.validateMenuItem(undo))
        XCTAssertFalse(field.validateMenuItem(redo))
        XCTAssertTrue(field.tryToPerform(undo.action!, with: undo), "Edit menu responder action")
        XCTAssertEqual(field.string, "")
        XCTAssertFalse(field.validateMenuItem(undo))
        XCTAssertTrue(field.validateMenuItem(redo))
        XCTAssertTrue(field.tryToPerform(redo.action!, with: redo))
        XCTAssertEqual(field.string, "hello")
        for (flags, expected) in [(NSEvent.ModifierFlags.command, ""), ([.command, .shift], "hello")] {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0,
                windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
            XCTAssertTrue(field.performKeyEquivalent(with: event))
            XCTAssertEqual(field.string, expected)
        }
        XCTAssertEqual(changes, ["hello", "", "hello", "", "hello"])
        node.writeValue("authoritative", into: field)
        XCTAssertFalse(field.validateMenuItem(undo))
        XCTAssertFalse(field.validateMenuItem(redo))
    }

    func testUnfocusedPlainTextAreaDoesNotConsumeAnotherEditorsUndoOrRedo() {
        let (node, first, window, presenter) = editor("", selection: NSRange(location: 0, length: 0))
        defer { window.close(); _ = presenter }
        node.props["markup"] = "none"
        node.applyTextArea()
        first.insertText("first", replacementRange: NSRange(location: 0, length: 0))
        first.breakUndoCoalescing()
        let second = TextArea(usingTextLayoutManager: true)
        second.frame = NSRect(x: 0, y: 0, width: 200, height: 50)
        second.isEditable = true
        node.addSubview(second)
        XCTAssertTrue(window.makeFirstResponder(second))
        let undo = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        let redo = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command, .shift], timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        XCTAssertFalse(first.performKeyEquivalent(with: undo))
        XCTAssertEqual(first.string, "first")
        XCTAssertTrue(first.undoManager?.canUndo == true)
        XCTAssertFalse(first.performKeyEquivalent(with: redo))
        XCTAssertEqual(first.string, "first")
        XCTAssertTrue(window.firstResponder === second)
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

    /// While text is marked a formatting command is refused, and the app's
    /// value waits (LLP 1045 D5): the value the editor shows leaves the
    /// composition alone and holds nothing; a different one is held, and
    /// applied when the composition ends, which the editor now does itself
    /// as it unmarks.
    func testMarkedTextDefersValueAndRefusesFormatting() {
        let (node, f, window, presenter) = editor("hello", selection: NSRange(location: 5, length: 0))
        defer { window.close(); _ = presenter }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 5, length: 0))
        XCTAssertTrue(f.hasMarkedText())
        let composing = f.string
        node.props["value"] = "external"
        node.applyTextArea()
        XCTAssertEqual(f.string, composing)
        XCTAssertEqual(f.markedRange(), NSRange(location: 5, length: 1), "the composition stands")
        XCTAssertEqual(node.pendingValue, "external")
        XCTAssertFalse(node.formatMarkup("bold"))
        f.unmarkText()
        XCTAssertEqual(f.string, "external")
        XCTAssertNil(node.pendingValue)
    }

    func testCompositionEchoDoesNotLeaveAStaleWriteback() {
        let (node, f, window, presenter) = editor("", selection: NSRange(location: 0, length: 0))
        defer { window.close(); _ = presenter }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 0, length: 0))
        node.writeValue(f.string, into: f)
        XCTAssertNil(node.pendingValue)
        XCTAssertTrue(f.hasMarkedText(), "the echo leaves the composition alone")
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

    func testThematicBreakDrawsInTheSecondaryLabelsOpacityWhateverTheSelection() {
        let source = "Above\n\n---\n\n> ***"
        let (node, f, window, presenter) = editor(source, selection: NSRange(location: 0, length: 0))
        let session = ExactApp.shared.makeSession(label: "markup-rule")
        presenter.session = session
        defer { window.close(); session.destroy() }
        let text = source as NSString, rule = text.range(of: "---"), quoted = text.range(of: "***"), quote = text.range(of: ">")
        let alpha = { (at: Int) in (f.textStorage!.attribute(.foregroundColor, at: at, effectiveRange: nil) as? NSColor)?.alphaComponent ?? -1 }
        var opacities: [CGFloat] = []
        for name in [NSAppearance.Name.aqua, .darkAqua] {
            window.appearance = NSAppearance(named: name)
            var secondary: CGFloat = 0
            NSAppearance(named: name)!.performAsCurrentDrawingAppearance { secondary = NSColor.secondaryLabelColor.alphaComponent }
            opacities.append(secondary)
            for caret in [0, rule.location + 1, quoted.location + 1] {
                f.setSelectedRange(NSRange(location: caret, length: 0))
                node.markdownRestyle()
                for at in [rule.location, NSMaxRange(rule) - 1, quoted.location] {
                    XCTAssertEqual(alpha(at), secondary, accuracy: 0.002, "\(name.rawValue) caret \(caret) at \(at)")
                }
                // A quote's `>` beside a break stays the quieter syntax.
                XCTAssertLessThan(alpha(quote.location), 0.45)
            }
        }
        XCTAssertNotEqual(opacities[0], opacities[1], "the appearance's own opacity, not one value")
    }

    func testRemovingMarkupRestoresPlainAttributesAndCommands() {
        let (node, f, window, presenter) = editor("**bold** [link](https://example.com)", selection: NSRange(location: 3, length: 0))
        defer { window.close(); _ = presenter }
        let look = MarkupEditor.Look(font: { size, weight, _, _ in NSFont.systemFont(ofSize: size, weight: weight >= 700 ? .bold : .regular) },
                                     size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, ink: .textColor)
        (f.markup as! MarkupEditor).restyle(f.textStorage!, selection: f.selectedRange(), look: look)
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

    /// A Markdown textarea built as a batch builds it, so the commands find it by `id`.
    private func markdownField(_ source: String) -> (Presenter, NodeView, TextArea, NSWindow) {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": source, "markup": "markdown"], "handlers": ["input", "select", "selectionchange", "beforeinput"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 120.0],
        ]))
        let node = p.views[2]!, f = node.textArea as! TextArea
        XCTAssertNotNil(f.markup)
        return (p, node, f, window)
    }

    /// LLP 1045 D6: the editor's storage is its source, so its native
    /// selection is the source selection its `input` and `selectionchange`
    /// report and `setSelectionRange` sets; its `select` stays its formats.
    func testSourceSelectionIsReportedAndSetLikeAPlainFields() throws {
        let (p, node, f, window) = markdownField("a **b** c")
        defer { window.close() }
        var selects = 0, changes: [FieldSelection] = []
        p.fieldSelections.onSelect = { _, _, _ in selects += 1 }
        p.fieldSelections.onChange = { _, _, s in changes.append(s) }
        window.makeFirstResponder(f)
        p.fieldSelections.setSelectionRange(["editor", 2, 7, "backward"])
        XCTAssertEqual(f.selectedRange(), NSRange(location: 2, length: 5))
        let reported = try XCTUnwrap(p.fieldSelections.reported(2, f.string))
        XCTAssertEqual([reported.start, reported.end], [2, 7])
        XCTAssertEqual(reported.direction, "backward")
        XCTAssertEqual(f.markup?.bookmark, NSRange(location: 2, length: 5), "a format applies to it")
        let deadline = Date().addingTimeInterval(2)
        while changes.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
        XCTAssertEqual(changes.last.map { [$0.start, $0.end] }, [2, 7])
        // The person's own range selects natively; HTML's `select` (kind 42) is never the editor's.
        f.setSelectedRange(NSRange(location: 0, length: 3))
        XCTAssertEqual(selects, 0)
        // A caret anywhere in the source, hidden syntax included: AppKit draws every character.
        p.fieldSelections.setSelectionRange(["editor", 3, 3])
        XCTAssertEqual(f.selectedRange(), NSRange(location: 3, length: 0))
        p.fieldSelections.setSelectionRange(["editor", 99, 99])
        XCTAssertEqual(f.selectedRange(), NSRange(location: 9, length: 0), "clamped as HTML clamps it")
    }

    /// Each `setRangeText` is one native undo step of its own (LLP 1045 D1,
    /// issue #792): two in one action are two, and the typing before them,
    /// whose event group is still open, stays a step of its own.
    func testEachRangeEditIsOneUndoStep() throws {
        let (p, node, f, window) = markdownField("")
        defer { window.close() }
        var replaced: [String] = []
        p.fieldSelections.onReplacement = { id, value, s in
            replaced.append("\(value) \(s.start)-\(s.end) \(p.fieldSelections.input(id)?.data ?? "nil")")
            node.props["value"] = value
            node.applyTextArea()
        }
        window.makeFirstResponder(f)
        let manager = try XCTUnwrap(f.undoManager)
        XCTAssertTrue(manager.groupsByEvent)
        f.insertText("hello", replacementRange: NSRange(location: 0, length: 0))
        XCTAssertEqual(manager.groupingLevel, 1, "the typing's event group is open")
        p.fieldSelections.setRangeText(["editor", "!", 5, 5, "end"])
        p.fieldSelections.setRangeText(["editor", "?", 6, 6, "end"])
        XCTAssertEqual(f.string, "hello!?")
        XCTAssertEqual(f.selectedRange(), NSRange(location: 7, length: 0))
        XCTAssertEqual(replaced, ["hello! 6-6 !", "hello!? 7-7 ?"])
        XCTAssertTrue(manager.groupsByEvent, "the manager groups by event again")
        // The end of the event closes the group it opened, and keeps no empty step.
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertEqual(manager.groupingLevel, 0)
        f.undo(nil)
        XCTAssertEqual(f.string, "hello!")
        f.undo(nil)
        XCTAssertEqual(f.string, "hello")
        f.redo(nil)
        XCTAssertEqual(f.string, "hello!")
        f.undo(nil)
        f.undo(nil)
        XCTAssertEqual(f.string, "", "the typing is a step of its own")
        // One call after typing in the same event: its own step, and no empty one.
        f.redo(nil)
        f.setSelectedRange(NSRange(location: 5, length: 0))
        f.insertText("X", replacementRange: NSRange(location: 5, length: 0))
        p.fieldSelections.setRangeText(["editor", "@", 0, 0, "end"])
        XCTAssertEqual(f.string, "@helloX")
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertEqual(manager.groupingLevel, 0)
        f.undo(nil)
        XCTAssertEqual(f.string, "helloX")
        f.undo(nil)
        XCTAssertEqual(f.string, "hello")
        // One call in an event of its own: one step.
        p.fieldSelections.setRangeText(["editor", "!", 5, 5, "end"])
        XCTAssertEqual(manager.groupingLevel, 0)
        f.undo(nil)
        XCTAssertEqual(f.string, "hello")
        // A replacement and a selection mode, its history intact for typing after it.
        p.fieldSelections.setRangeText(["editor", "[a.ts]", 0, 5, "select"])
        XCTAssertEqual(f.string, "[a.ts]")
        XCTAssertEqual(f.selectedRange(), NSRange(location: 0, length: 6))
        f.undo(nil)
        XCTAssertEqual(f.string, "hello")
        // While an input method composes, refused: nothing changes.
        f.setSelectedRange(NSRange(location: 5, length: 0))
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 5, length: 0))
        let composing = f.string
        replaced.removeAll()
        p.fieldSelections.setRangeText(["editor", "x", 0, 0])
        p.fieldSelections.setSelectionRange(["editor", 0, 0])
        XCTAssertEqual(f.string, composing)
        XCTAssertTrue(f.hasMarkedText())
        XCTAssertEqual(f.markedRange(), NSRange(location: 5, length: 1))
        XCTAssertEqual(replaced, [])
        f.unmarkText()
    }

    /// The app's `beforeinput` comes before the editor's own handling: a
    /// cancelled Return continues no list, an allowed one is heard first
    /// with the source as it was, then as one `input`. A format is the
    /// app's own command, not an edit it is asked about.
    func testReturnAsksTheAppBeforeTheListContinues() throws {
        let (p, node, f, window) = markdownField("- one")
        defer { window.close() }
        var heard: [String] = [], prevent = true
        p.fieldSelections.onBeforeInput = { _, value, s, edit in
            heard.append("before \(value) \(s.start) \(edit.inputType) \(edit.cancelable)")
            p.defaultPrevented = prevent
        }
        p.onInput = { id, value in
            let s = p.fieldSelections.reported(id, value)
            heard.append("input \(value) \(s?.start ?? -1) \(p.fieldSelections.input(id)?.inputType ?? "")")
            node.props["value"] = value
            node.applyTextArea()
        }
        window.makeFirstResponder(f)
        f.setSelectedRange(NSRange(location: 5, length: 0))
        f.insertNewline(nil)
        XCTAssertEqual(f.string, "- one")
        prevent = false
        f.insertNewline(nil)
        XCTAssertEqual(f.string, "- one\n- ")
        XCTAssertEqual(heard, ["before - one 5 insertLineBreak true", "before - one 5 insertLineBreak true", "input - one\n-  8 insertLineBreak"])
        f.undoManager?.undo()
        XCTAssertEqual(f.string, "- one")
        heard.removeAll()
        f.setSelectedRange(NSRange(location: 2, length: 3))
        XCTAssertTrue(node.formatMarkup("bold"))
        XCTAssertEqual(f.string, "- **one**")
        XCTAssertEqual(heard, ["input - **one** 4 "])
    }

    /// Tab and Shift-Tab leave the editor along the key-view loop with its
    /// source as it was, as a plain textarea's do; an app that prevents the
    /// `key` keeps the focus and makes its own edit.
    func testTabMovesTheFocusUnlessTheAppTakesTheKey() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "input", "props": ["value": "before"]],
            ["op": "create", "id": 3, "kind": "textarea", "props": ["id": "editor", "value": "- one", "markup": "markdown"], "handlers": ["input", "key"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "after"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 300.0, "h": 120.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 180.0, "w": 200.0, "h": 24.0],
        ]))
        let f = try XCTUnwrap(p.views[3]?.textArea as? TextArea)
        let before = try XCTUnwrap(p.views[2]?.field), after = try XCTUnwrap(p.views[4]?.field)
        XCTAssertNotNil(f.markup)
        var heard: [String] = [], prevent = false
        p.onKey = { _, press in
            heard.append(press.chord)
            guard prevent else { return }
            p.defaultPrevented = true
            p.fieldSelections.setRangeText(["editor", "\t"])
        }
        p.fieldSelections.onReplacement = { _, value, _ in heard.append("input \(value)") }
        // As the session's monitor routes a key, then AppKit when it was not taken.
        func tab(shift: Bool) throws {
            let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: shift ? .shift : [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                       windowNumber: window.windowNumber, context: nil, characters: shift ? "\u{19}" : "\t",
                                                       charactersIgnoringModifiers: shift ? "\u{19}" : "\t", isARepeat: false, keyCode: 48))
            p.flushKeyViewLoop()
            if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
        }
        let caret = NSRange(location: 2, length: 0)
        for (shift, target) in [(false, after), (true, before)] {
            XCTAssertTrue(window.makeFirstResponder(f))
            f.setSelectedRange(caret)
            try tab(shift: shift)
            XCTAssertTrue(window.firstResponder === target.currentEditor(), shift ? "Shift-Tab reaches the field before" : "Tab reaches the field after")
            XCTAssertEqual(f.string, "- one")
        }
        XCTAssertEqual(heard, ["Tab", "Shift+Tab"])
        heard.removeAll()
        prevent = true
        XCTAssertTrue(window.makeFirstResponder(f))
        f.setSelectedRange(caret)
        try tab(shift: false)
        XCTAssertTrue(window.firstResponder === f, "the app took the key")
        XCTAssertEqual(f.string, "- \tone")
        XCTAssertEqual(heard, ["Tab", "input - \tone"])
    }

    func testCommandSeamRejectsBrokenRangesInsteadOfSplittingSurrogates() {
        XCTAssertNil(MarkupEdit(source: "🎉", object: ["replacements": [[1, 2, "x"]], "selection": [0, 0]]))
        XCTAssertNil(MarkupEdit(source: "abcd", object: ["replacements": [[1, 3, "x"], [2, 4, "y"]], "selection": [0, 0]]))
    }

    /// A Markdown editor styled by a session's text engine, which gives it
    /// its fonts: without one it styles nothing. The caller keeps the
    /// presenter, which its node holds weakly.
    private func styled(_ source: String, caret: Int? = nil) -> (NodeView, TextArea, NSWindow, Presenter, ExactSession) {
        let (node, field, window, presenter) = editor(source, selection: NSRange(location: caret ?? (source as NSString).length, length: 0))
        let session = ExactApp.shared.makeSession(label: "paragraph-dirs")
        presenter.session = session
        node.restyleMarkup()
        return (node, field, window, presenter, session)
    }

    /// Each line's base direction, read at the given line starts.
    private func directions(_ field: TextArea, _ starts: [Int]) -> [NSWritingDirection?] {
        starts.map { (field.textStorage?.attribute(.paragraphStyle, at: $0, effectiveRange: nil) as? NSParagraphStyle)?.baseWritingDirection }
    }

    /// LLP 1045 D11: each line's paragraph takes the app's word as its base
    /// direction, a line past the last word the last, and a line without one
    /// the field's; a new `paragraphDirs` alone restyles, and the source never
    /// changes.
    func testParagraphDirsGiveEachLineItsBaseWritingDirection() throws {
        let source = "One\nReact هو مكتبة\n- عنصر\n"
        let (node, field, window, presenter, session) = styled(source)
        defer { window.close(); session.destroy(); _ = presenter }
        let starts = [0, 4, 19]
        // Where "React" starts, from the field's left edge.
        let react = { () -> CGFloat in
            var actual = NSRange()
            return field.firstRect(forCharacterRange: NSRange(location: 4, length: 1), actualRange: &actual).minX - window.frame.minX
        }
        // Without words every line takes the field's direction: no first strong letter selects one.
        XCTAssertEqual(directions(field, starts), [.leftToRight, .leftToRight, .leftToRight])
        XCTAssertLessThan(react(), 100)
        node.props["paragraphDirs"] = "ltr rtl"
        node.applyTextArea()
        XCTAssertEqual(directions(field, starts), [.leftToRight, .rightToLeft, .rightToLeft])
        XCTAssertGreaterThan(react(), 250, "a right-to-left line starts at the right")
        // The caret on the empty last line types, and draws a placeholder, in its line's direction.
        XCTAssertEqual((field.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .rightToLeft)
        // The list item keeps its paragraph row's indent, which TextKit measures from the leading edge.
        let item = try XCTUnwrap(field.textStorage?.attribute(.paragraphStyle, at: 19, effectiveRange: nil) as? NSParagraphStyle)
        XCTAssertEqual(item.headIndent, 20)
        // The most common direction is every paragraph's base; only the line that differs is set apart.
        let drawn = try XCTUnwrap(field.markup as? MarkupEditor).directions
        XCTAssertEqual(drawn.base, .rightToLeft)
        XCTAssertEqual(drawn.runs.map(\.range), [NSRange(location: 0, length: 4)])
        node.props["paragraphDirs"] = "rtl"
        node.applyTextArea()
        XCTAssertEqual(directions(field, starts), [.rightToLeft, .rightToLeft, .rightToLeft])
        XCTAssertTrue(try XCTUnwrap(field.markup as? MarkupEditor).directions.runs.isEmpty, "one word is one pass")
        node.props["paragraphDirs"] = "rtl sideways"
        node.applyTextArea()
        XCTAssertEqual(directions(field, starts), [.rightToLeft, .leftToRight, .leftToRight])
        node.props["paragraphDirs"] = nil
        node.applyTextArea()
        XCTAssertEqual(directions(field, starts), [.leftToRight, .leftToRight, .leftToRight])
        XCTAssertEqual(field.string, source)
    }

    /// LLP 1045 D11: a field whose own `direction` is right to left gives it
    /// to every line without a word, the author's over the first strong letter.
    func testALineWithoutAWordTakesTheFieldsDirection() {
        let (node, field, window, presenter, session) = styled("Hello\nمرحبا")
        defer { window.close(); session.destroy(); _ = presenter }
        node.applyStyle(["direction": .string("rtl")])
        XCTAssertEqual(directions(field, [0, 6]), [.rightToLeft, .rightToLeft])
        node.props["paragraphDirs"] = "ltr"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 6]), [.leftToRight, .leftToRight])
        node.props["paragraphDirs"] = "ltr x"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 6]), [.leftToRight, .rightToLeft])
    }

    /// LLP 1045 D11: an empty editor's placeholder is drawn in its first
    /// line's direction, as UIKit's and the web's are.
    func testAnEmptyEditorsPlaceholderTakesItsFirstLinesDirection() throws {
        let (node, field, window, presenter, session) = styled("")
        defer { window.close(); session.destroy(); _ = presenter }
        let editor = try XCTUnwrap(field.markup)
        XCTAssertEqual(editor.direction(at: 0), .leftToRight)
        node.props["paragraphDirs"] = "rtl"
        node.applyTextArea()
        XCTAssertEqual(editor.direction(at: 0), .rightToLeft)
    }

    /// LLP 1045 D11: `auto` is a line's first strong letter outside its
    /// syntax and atoms, as the web editor finds it; with none, the field's.
    func testAutoTakesTheFirstStrongLetterOutsideSyntaxAndAtoms() {
        let source = "@src/a.ts اشرح هذا \n@x اكتب\n[1](https://example.test) مرحبا\n42"
        let (node, field, window, presenter, session) = styled(source)
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["atoms"] = #"[{"name":"mention","prefix":"@"}]"#
        node.props["atomFaces"] = #"[{"source":"@src/a.ts","label":"a.ts","icon":"","title":""}]"#
        node.props["paragraphDirs"] = "auto"
        node.applyTextArea()
        let starts = [0, 20, 28, 60]
        XCTAssertEqual(directions(field, starts), [.rightToLeft, .leftToRight, .rightToLeft, .leftToRight])
    }

    /// LLP 1045 D11: undo and redo give back the words of the lines they
    /// restore. Each step restyles, as the next keystroke would.
    func testUndoAndRedoGiveBackTheWordsOfTheLinesTheyRestore() throws {
        let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 1)
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["paragraphDirs"] = "ltr rtl ltr"
        node.applyTextArea()
        let manager = try XCTUnwrap(field.undoManager)
        manager.removeAllActions()
        field.setSelectedRange(NSRange(location: 1, length: 4))
        field.deleteBackward(nil)
        node.restyleMarkup()
        XCTAssertEqual(field.string, "A")
        XCTAssertEqual(directions(field, [0]), [.leftToRight])
        field.breakUndoCoalescing()
        manager.undo()
        node.restyleMarkup()
        XCTAssertEqual(field.string, "A\nب\nC")
        XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        manager.redo()
        node.restyleMarkup()
        XCTAssertEqual(field.string, "A")
        manager.undo()
        node.restyleMarkup()
        XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
    }

    /// LLP 1045 D11: an undo step keeps the words of the source it returns
    /// to, however many keystrokes it holds, and typing still coalesces; a
    /// removed line typed back in takes the word above it, and the app's next
    /// words forget what undo would have given back.
    func testUndoStepsKeepTheirWordsWhichTypingAndTheAppsNextWordsDoNotBringBack() throws {
        let typed = NSRange(location: NSNotFound, length: 0)
        do {
            let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 1)
            defer { window.close(); session.destroy(); _ = presenter }
            node.props["paragraphDirs"] = "ltr rtl ltr"
            node.applyTextArea()
            let manager = try XCTUnwrap(field.undoManager)
            manager.removeAllActions()
            field.setSelectedRange(NSRange(location: 1, length: 4))
            turn(manager) { field.deleteBackward(nil) }
            for _ in 0..<64 { turn(manager) { field.insertText("y", replacementRange: typed) } }
            XCTAssertEqual(field.string, "A" + String(repeating: "y", count: 64))
            // AppKit coalesces the keystrokes, one a turn, into the deletion's step, as a bare NSTextView does.
            manager.undo()
            XCTAssertEqual(field.string, "A\nب\nC")
            XCTAssertFalse(manager.canUndo)
            XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
        do {
            let (node, field, window, presenter, session) = styled("A\nQ", caret: 1)
            defer { window.close(); session.destroy(); _ = presenter }
            node.props["paragraphDirs"] = "ltr rtl"
            node.applyTextArea()
            field.setSelectedRange(NSRange(location: 1, length: 2))
            field.deleteBackward(nil)
            field.insertNewline(nil)
            field.insertText("Q", replacementRange: typed)
            XCTAssertEqual(field.string, "A\nQ")
            XCTAssertEqual(directions(field, [0, 2]), [.leftToRight, .leftToRight])
        }
        do {
            let (node, field, window, presenter, session) = styled("A\nB", caret: 3)
            defer { window.close(); session.destroy(); _ = presenter }
            node.props["paragraphDirs"] = "ltr"
            node.applyTextArea()
            let manager = try XCTUnwrap(field.undoManager)
            manager.removeAllActions()
            field.insertText("C", replacementRange: typed)
            node.props["value"] = field.string
            node.props["paragraphDirs"] = "rtl"
            node.applyTextArea()
            field.breakUndoCoalescing()
            manager.undo()
            XCTAssertEqual(field.string, "A\nB")
            XCTAssertEqual(directions(field, [0, 2]), [.rightToLeft, .rightToLeft])
        }
    }

    /// LLP 1045 D11: a line ends at CR and CRLF as at LF, where TextKit ends
    /// a paragraph; text the app rewrites from NFC to NFD finds its lines again.
    func testALineEndsAtCROrCRLFAndCanonicallyEquivalentTextIsAnotherSource() {
        let (node, field, window, presenter, session) = styled("A\rب\r\nC")
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["paragraphDirs"] = "ltr rtl ltr"
        node.applyTextArea()
        XCTAssertEqual(field.string, "A\rب\r\nC")
        XCTAssertEqual(directions(field, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        node.props["paragraphDirs"] = "auto"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        node.props["paragraphDirs"] = "ltr rtl"
        node.props["value"] = "\u{E9}\nب"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 2]), [.leftToRight, .rightToLeft])
        node.props["value"] = "e\u{301}\nب"
        node.applyTextArea()
        XCTAssertEqual(field.string.utf16.count, 4)
        XCTAssertEqual(directions(field, [0, 3]), [.leftToRight, .rightToLeft])
    }

    /// LLP 1045 D11: a composition's step, which its first marked text opens,
    /// and `setRangeText`'s keep the words of the source they return to.
    func testACompositionsStepAndSetRangeTextsKeepTheirWords() throws {
        do {
            let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 1)
            defer { window.close(); session.destroy(); _ = presenter }
            node.props["paragraphDirs"] = "ltr rtl ltr"
            node.applyTextArea()
            let manager = try XCTUnwrap(field.undoManager)
            manager.removeAllActions()
            let none = NSRange(location: NSNotFound, length: 0)
            field.setSelectedRange(NSRange(location: 1, length: 2))
            turn(manager) { field.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: none) }
            turn(manager) { field.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: none) }
            turn(manager) { field.unmarkText() }
            XCTAssertEqual(field.string, "A한\nC")
            manager.undo()
            XCTAssertEqual(field.string, "A\nب\nC")
            XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
        do {
            let (p, _, f, window) = markdownField("A\nب\nC")
            let session = ExactApp.shared.makeSession(label: "paragraph-dirs")
            p.session = session
            defer { window.close(); session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
            window.makeFirstResponder(f)
            let manager = try XCTUnwrap(f.undoManager)
            manager.removeAllActions()
            p.fieldSelections.setRangeText(["editor", "", 1, 3, "end"])
            XCTAssertEqual(f.string, "A\nC")
            XCTAssertEqual(directions(f, [0, 2]), [.leftToRight, .leftToRight])
            closeGroup(manager)
            manager.undo()
            XCTAssertEqual(f.string, "A\nب\nC")
            XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
    }

    /// LLP 1045 D11: the app's new words, which a runner can apply inside an
    /// undo's `input` and so before its restyle, still forget the words redo
    /// would give back.
    func testNewWordsWithAnUndoForgetTheWordsRedoWouldGiveBack() throws {
        let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 1)
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["paragraphDirs"] = "ltr rtl ltr"
        node.applyTextArea()
        let manager = try XCTUnwrap(field.undoManager)
        manager.removeAllActions()
        field.setSelectedRange(NSRange(location: 1, length: 2))
        turn(manager) { field.deleteBackward(nil) }
        XCTAssertEqual(field.string, "A\nC")
        node.props["paragraphDirs"] = "rtl"
        manager.undo()
        XCTAssertEqual(field.string, "A\nب\nC")
        XCTAssertEqual(directions(field, [0, 2, 4]), [.rightToLeft, .rightToLeft, .rightToLeft])
        manager.redo()
        XCTAssertEqual(field.string, "A\nC")
        XCTAssertEqual(directions(field, [0, 2]), [.rightToLeft, .rightToLeft])
    }

    /// LLP 1045 D11: each undo and redo moves one set of words, as the
    /// manager moves one step, where the app's new words left the stacks
    /// fewer steps than the manager's.
    func testEachUndoAndRedoMovesOneSetOfWords() throws {
        let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 3)
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["paragraphDirs"] = "ltr rtl ltr"
        node.applyTextArea()
        let manager = try XCTUnwrap(field.undoManager)
        manager.removeAllActions()
        field.setSelectedRange(NSRange(location: 3, length: 0))
        turn(manager) { field.insertNewline(nil) }
        XCTAssertEqual(field.string, "A\nب\n\nC")
        node.props["value"] = field.string
        node.props["paragraphDirs"] = "ltr ltr rtl ltr"
        node.applyTextArea()
        field.setSelectedRange(NSRange(location: 2, length: 2))
        turn(manager) { field.deleteBackward(nil) }
        XCTAssertEqual(field.string, "A\n\nC")
        let starts = [0, 2, 4, 5], drawn: [NSWritingDirection?] = [.leftToRight, .leftToRight, .rightToLeft, .leftToRight]
        manager.undo()
        XCTAssertEqual(directions(field, starts), drawn)
        manager.undo()
        XCTAssertEqual(field.string, "A\nب\nC")
        manager.redo()
        XCTAssertEqual(field.string, "A\nب\n\nC")
        XCTAssertEqual(directions(field, starts), drawn)
        manager.redo()
        XCTAssertEqual(field.string, "A\n\nC")
        manager.undo()
        XCTAssertEqual(field.string, "A\nب\n\nC")
        XCTAssertEqual(directions(field, starts), drawn)
    }

    /// The text view's open undo group closes, as it does at the end of a run loop turn.
    private func closeGroup(_ manager: UndoManager) {
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    /// `body` in a run loop turn of its own, as an event arrives, and the
    /// undo group it opened closed after it.
    private func turn(_ manager: UndoManager, _ body: @escaping () -> Void) {
        var done = false
        RunLoop.main.perform { body(); done = true }
        while !done { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        closeGroup(manager)
    }

    /// LLP 1045 D11: `auto` reads a footnote mark's label as text, as the web
    /// shows it, wherever the caret is (a caret on a line reveals its syntax);
    /// in a right-to-left field, a line with no strong letter takes the
    /// field's direction.
    func testAutoReadsAFootnoteMarksLabelAsText() {
        let (node, field, window, presenter, session) = styled("[^n] مرحبا\n\n[^note]: ملاحظة")
        defer { window.close(); session.destroy(); _ = presenter }
        node.applyStyle(["direction": .string("rtl")])
        node.props["paragraphDirs"] = "auto"
        node.applyTextArea()
        for caret in [20, 2] {
            field.setSelectedRange(NSRange(location: caret, length: 0))
            node.restyleMarkup()
            XCTAssertEqual(directions(field, [0, 11, 12]), [.leftToRight, .rightToLeft, .leftToRight], "caret at \(caret)")
        }
    }

    /// LLP 1045 D11: a word stays with its line through typing, Return, undo
    /// and a composition, until the app's next words.
    func testAWordStaysWithItsLineThroughTypingReturnUndoAndComposition() throws {
        let (node, field, window, presenter, session) = styled("A\nب\nC", caret: 1)
        defer { window.close(); session.destroy(); _ = presenter }
        node.props["paragraphDirs"] = "ltr rtl ltr"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        // Return at the end of the first line: the new line takes the word above it, and ب keeps its own.
        field.setSelectedRange(NSRange(location: 1, length: 0))
        field.insertNewline(nil)
        XCTAssertEqual(field.string, "A\n\nب\nC")
        XCTAssertEqual(directions(field, [0, 2, 3, 5]), [.leftToRight, .leftToRight, .rightToLeft, .leftToRight])
        field.breakUndoCoalescing()
        field.undoManager?.undo()
        XCTAssertEqual(field.string, "A\nب\nC")
        XCTAssertEqual(directions(field, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        // Typing in a line keeps its word.
        field.setSelectedRange(NSRange(location: 3, length: 0))
        field.insertText("x", replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertEqual(field.string, "A\nبx\nC")
        XCTAssertEqual(directions(field, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        // A composition is drawn in its line's direction and committed with it.
        field.setMarkedText("ج", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(field.hasMarkedText())
        field.unmarkText()
        XCTAssertEqual(field.string, "A\nبxج\nC")
        XCTAssertEqual(directions(field, [0, 2, 6]), [.leftToRight, .rightToLeft, .leftToRight])
        // The app's next words go by line number again.
        node.props["value"] = field.string
        node.props["paragraphDirs"] = "rtl"
        node.applyTextArea()
        XCTAssertEqual(directions(field, [0, 2, 6]), [.rightToLeft, .rightToLeft, .rightToLeft])
    }
}
#endif

#if os(iOS)
import UIKit

/// LLP 1045 D6 on UIKit: the Markdown editor's source selection and range
/// edits, each `setRangeText` one undo step of its own, and Return asking the
/// app before the list continues. A simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class MarkupEditorIOSTests: XCTestCase {
    private var window: UIWindow!

    override func setUp() {
        super.setUp()
        ExactMarkdown.install()
    }

    private func markdownField(_ source: String) -> (Presenter, NodeView, TextArea) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": source, "markup": "markdown"], "handlers": ["input", "select", "selectionchange", "beforeinput"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 120.0],
        ]))
        let node = p.views[2]!, f = node.textArea as! TextArea
        XCTAssertNotNil(f.markup)
        return (p, node, f)
    }

    func testSourceSelectionAndEachRangeEditIsOneUndoStep() throws {
        let (p, node, f) = markdownField("")
        var selects = 0, replaced: [String] = []
        p.fieldSelections.onSelect = { _, _, _ in selects += 1 }
        p.fieldSelections.onReplacement = { id, value, s in
            replaced.append("\(value) \(s.start)-\(s.end) \(p.fieldSelections.input(id)?.data ?? "nil")")
            node.props["value"] = value
            node.applyTextArea()
        }
        XCTAssertTrue(f.becomeFirstResponder())
        let manager = try XCTUnwrap(f.undoManager)
        manager.removeAllActions()
        f.insertText("hello")
        p.fieldSelections.setRangeText(["editor", "!", 5, 5, "end"])
        p.fieldSelections.setRangeText(["editor", "?", 6, 6, "end"])
        XCTAssertEqual(f.text, "hello!?")
        XCTAssertEqual(f.selectedRange, NSRange(location: 7, length: 0))
        XCTAssertEqual(replaced, ["hello! 6-6 !", "hello!? 7-7 ?"])
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertEqual(manager.groupingLevel, 0)
        manager.undo()
        XCTAssertEqual(f.text, "hello!")
        manager.undo()
        XCTAssertEqual(f.text, "hello")
        manager.redo()
        XCTAssertEqual(f.text, "hello!")
        manager.undo()
        manager.undo()
        XCTAssertEqual(f.text, "", "the typing is a step of its own")
        manager.redo()
        // One call after typing in the same event: its own step, and no empty one.
        f.selectedRange = NSRange(location: 5, length: 0)
        f.insertText("X")
        p.fieldSelections.setRangeText(["editor", "@", 0, 0, "end"])
        XCTAssertEqual(f.text, "@helloX")
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        manager.undo()
        XCTAssertEqual(f.text, "helloX")
        manager.undo()
        XCTAssertEqual(f.text, "hello")
        p.fieldSelections.setSelectionRange(["editor", 1, 4, "backward"])
        XCTAssertEqual(f.selectedRange, NSRange(location: 1, length: 3))
        let reported = try XCTUnwrap(p.fieldSelections.reported(2, f.text))
        XCTAssertEqual([reported.start, reported.end], [1, 4])
        XCTAssertEqual(reported.direction, "backward")
        XCTAssertEqual(selects, 0, "the editor's `select` is its formats, never HTML's")
        // Refused while an input method composes.
        f.selectedRange = NSRange(location: 5, length: 0)
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        let composing = f.text
        replaced.removeAll()
        p.fieldSelections.setRangeText(["editor", "x", 0, 0])
        XCTAssertEqual(f.text, composing)
        XCTAssertNotNil(f.markedTextRange)
        XCTAssertEqual(replaced, [])
        f.unmarkText()
    }

    func testReturnAsksTheAppBeforeTheListContinues() throws {
        let (p, node, f) = markdownField("- one")
        var heard: [String] = [], prevent = true
        p.fieldSelections.onBeforeInput = { _, value, s, edit in
            heard.append("before \(value) \(s.start) \(edit.inputType)")
            p.defaultPrevented = prevent
        }
        p.onInput = { id, value in
            heard.append("input \(value) \(p.fieldSelections.reported(id, value)?.start ?? -1) \(p.fieldSelections.input(id)?.inputType ?? "")")
            node.props["value"] = value
            node.applyTextArea()
        }
        XCTAssertTrue(f.becomeFirstResponder())
        f.selectedRange = NSRange(location: 5, length: 0)
        f.insertText("\n")
        XCTAssertEqual(f.text, "- one")
        prevent = false
        f.insertText("\n")
        XCTAssertEqual(f.text, "- one\n- ")
        XCTAssertEqual(heard, ["before - one 5 insertLineBreak", "before - one 5 insertLineBreak", "input - one\n-  8 insertLineBreak"])
    }

    /// A Markdown field styled by a session's text engine, which gives it
    /// its fonts: without one it styles nothing.
    private func styledField(_ source: String) -> (Presenter, NodeView, TextArea, ExactSession) {
        let (p, node, f) = markdownField(source)
        let session = ExactApp.shared.makeSession(label: "paragraph-dirs")
        p.session = session
        node.restyleMarkup()
        return (p, node, f, session)
    }

    private func directions(_ f: TextArea, _ starts: [Int]) -> [NSWritingDirection?] {
        starts.map { (f.textStorage.attribute(.paragraphStyle, at: $0, effectiveRange: nil) as? NSParagraphStyle)?.baseWritingDirection }
    }

    /// LLP 1045 D11 on UIKit: a `paragraphDirs` the app sends alone restyles
    /// each line's base direction, the empty last line types (and draws its
    /// placeholder) in its line's, and a line without a word takes the field's.
    func testParagraphDirsGiveEachLineItsBaseWritingDirection() {
        let source = "One\nReact هو مكتبة\n"
        let (p, _, f, session) = styledField(source)
        defer { session.destroy() }
        XCTAssertEqual(directions(f, [0, 4]), [.leftToRight, .leftToRight])
        f.selectedRange = NSRange(location: (source as NSString).length, length: 0)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl"]]]))
        XCTAssertEqual(directions(f, [0, 4]), [.leftToRight, .rightToLeft])
        XCTAssertEqual((f.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .rightToLeft)
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["paragraphDirs"]]]))
        XCTAssertEqual(directions(f, [0, 4]), [.leftToRight, .leftToRight])
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
        XCTAssertEqual(directions(f, [0, 4]), [.rightToLeft, .rightToLeft])
        XCTAssertEqual(f.text, source)
    }

    /// LLP 1045 D11 on UIKit: undo and redo give back the words of the lines
    /// they restore. Each step restyles, as the next keystroke would.
    func testUndoAndRedoGiveBackTheWordsOfTheLinesTheyRestore() throws {
        let (p, node, f, session) = styledField("A\nب\nC")
        defer { session.destroy() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
        XCTAssertTrue(f.becomeFirstResponder())
        let manager = try XCTUnwrap(f.undoManager)
        manager.removeAllActions()
        f.selectedRange = NSRange(location: 1, length: 4)
        f.deleteBackward()
        node.restyleMarkup()
        XCTAssertEqual(f.text, "A")
        XCTAssertEqual(directions(f, [0]), [.leftToRight])
        // UIKit closes the edit's undo group at the end of the run loop turn.
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        manager.undo()
        node.restyleMarkup()
        XCTAssertEqual(f.text, "A\nب\nC")
        XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        manager.redo()
        node.restyleMarkup()
        XCTAssertEqual(f.text, "A")
        manager.undo()
        node.restyleMarkup()
        XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
    }

    /// LLP 1045 D11 on UIKit: an undo step keeps the words of the source it
    /// returns to, however many keystrokes it holds; a removed line typed
    /// back in takes the word above it, and the app's next words forget what
    /// undo would have given back.
    func testUndoStepsKeepTheirWordsWhichTypingAndTheAppsNextWordsDoNotBringBack() throws {
        do {
            let (p, _, f, session) = styledField("A\nب\nC")
            defer { session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
            XCTAssertTrue(f.becomeFirstResponder())
            let manager = try XCTUnwrap(f.undoManager)
            manager.removeAllActions()
            f.selectedRange = NSRange(location: 1, length: 4)
            turn(manager) { f.deleteBackward() }
            for _ in 0..<64 { turn(manager) { f.insertText("y") } }
            XCTAssertEqual(f.text, "A" + String(repeating: "y", count: 64))
            // UIKit coalesces the keystrokes, one a turn, into the deletion's step.
            manager.undo()
            XCTAssertEqual(f.text, "A\nب\nC")
            XCTAssertFalse(manager.canUndo)
            XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
        do {
            let (p, _, f, session) = styledField("A\nQ")
            defer { session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl"]]]))
            XCTAssertTrue(f.becomeFirstResponder())
            f.selectedRange = NSRange(location: 1, length: 2)
            f.deleteBackward()
            f.insertText("\n")
            f.insertText("Q")
            XCTAssertEqual(f.text, "A\nQ")
            XCTAssertEqual(directions(f, [0, 2]), [.leftToRight, .leftToRight])
        }
        do {
            let (p, _, f, session) = styledField("A\nB")
            defer { session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr"]]]))
            XCTAssertTrue(f.becomeFirstResponder())
            let manager = try XCTUnwrap(f.undoManager)
            manager.removeAllActions()
            f.selectedRange = NSRange(location: 3, length: 0)
            f.insertText("C")
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": f.text ?? "", "paragraphDirs": "rtl"]]]))
            closeGroup(manager)
            manager.undo()
            XCTAssertEqual(f.text, "A\nB")
            XCTAssertEqual(directions(f, [0, 2]), [.rightToLeft, .rightToLeft])
        }
    }

    /// LLP 1045 D11 on UIKit: a line ends at CR and CRLF as at LF, where
    /// TextKit ends a paragraph; text the app rewrites from NFC to NFD finds
    /// its lines again.
    func testALineEndsAtCROrCRLFAndCanonicallyEquivalentTextIsAnotherSource() {
        let (p, _, f, session) = styledField("A\rب\r\nC")
        defer { session.destroy() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
        XCTAssertEqual(f.text, "A\rب\r\nC")
        XCTAssertEqual(directions(f, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "auto"]]]))
        XCTAssertEqual(directions(f, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": "\u{E9}\nب", "paragraphDirs": "ltr rtl"]]]))
        XCTAssertEqual(directions(f, [0, 2]), [.leftToRight, .rightToLeft])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": "e\u{301}\nب"]]]))
        XCTAssertEqual((f.text ?? "").utf16.count, 4)
        XCTAssertEqual(directions(f, [0, 3]), [.leftToRight, .rightToLeft])
    }

    /// LLP 1045 D11 on UIKit: a composition's step and `setRangeText`'s keep
    /// the words of the source they return to.
    func testACompositionsStepAndSetRangeTextsKeepTheirWords() throws {
        do {
            let (p, _, f, session) = styledField("A\nب\nC")
            defer { session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
            XCTAssertTrue(f.becomeFirstResponder())
            let manager = try XCTUnwrap(f.undoManager)
            manager.removeAllActions()
            f.selectedRange = NSRange(location: 1, length: 2)
            turn(manager) { f.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0)) }
            turn(manager) { f.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0)) }
            turn(manager) { f.unmarkText() }
            XCTAssertEqual(f.text, "A한\nC")
            manager.undo()
            XCTAssertEqual(f.text, "A\nب\nC")
            XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
        do {
            let (p, _, f, session) = styledField("A\nب\nC")
            defer { session.destroy() }
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
            XCTAssertTrue(f.becomeFirstResponder())
            let manager = try XCTUnwrap(f.undoManager)
            manager.removeAllActions()
            p.fieldSelections.setRangeText(["editor", "", 1, 3, "end"])
            XCTAssertEqual(f.text, "A\nC")
            XCTAssertEqual(directions(f, [0, 2]), [.leftToRight, .leftToRight])
            closeGroup(manager)
            manager.undo()
            XCTAssertEqual(f.text, "A\nب\nC")
            XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        }
    }

    /// LLP 1045 D11 on UIKit: each undo and redo moves one set of words, as
    /// the manager moves one step, where the app's new words left the stacks
    /// fewer steps than the manager's.
    func testEachUndoAndRedoMovesOneSetOfWords() throws {
        let (p, _, f, session) = styledField("A\nب\nC")
        defer { session.destroy() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
        XCTAssertTrue(f.becomeFirstResponder())
        let manager = try XCTUnwrap(f.undoManager)
        manager.removeAllActions()
        f.selectedRange = NSRange(location: 3, length: 0)
        turn(manager) { f.insertText("\n") }
        XCTAssertEqual(f.text, "A\nب\n\nC")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": f.text ?? "", "paragraphDirs": "ltr ltr rtl ltr"]]]))
        f.selectedRange = NSRange(location: 2, length: 2)
        turn(manager) { f.deleteBackward() }
        XCTAssertEqual(f.text, "A\n\nC")
        let starts = [0, 2, 4, 5], drawn: [NSWritingDirection?] = [.leftToRight, .leftToRight, .rightToLeft, .leftToRight]
        manager.undo()
        XCTAssertEqual(directions(f, starts), drawn)
        manager.undo()
        XCTAssertEqual(f.text, "A\nب\nC")
        manager.redo()
        XCTAssertEqual(f.text, "A\nب\n\nC")
        XCTAssertEqual(directions(f, starts), drawn)
        manager.redo()
        XCTAssertEqual(f.text, "A\n\nC")
        manager.undo()
        XCTAssertEqual(f.text, "A\nب\n\nC")
        XCTAssertEqual(directions(f, starts), drawn)
    }

    /// UIKit closes an edit's undo group at the end of the run loop turn.
    private func closeGroup(_ manager: UndoManager) {
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    /// `body` in a run loop turn of its own, as an event arrives, and the
    /// undo group it opened closed after it.
    private func turn(_ manager: UndoManager, _ body: @escaping () -> Void) {
        var done = false
        RunLoop.main.perform { body(); done = true }
        while !done { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        closeGroup(manager)
    }

    /// LLP 1045 D11 on UIKit: `auto` is a line's first strong letter outside
    /// its syntax and atoms, a footnote mark's label being text; with none,
    /// the field's direction.
    func testAutoTakesTheFirstStrongLetterOutsideSyntaxAndAtoms() {
        let (p, node, f, session) = styledField("@src/a.ts اشرح هذا \n@x اكتب\n[1](https://example.test) مرحبا\n42\n[^n] مرحبا\n\n[^note]: ملاحظة")
        defer { session.destroy() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": [
            "atoms": #"[{"name":"mention","prefix":"@"}]"#,
            "atomFaces": #"[{"source":"@src/a.ts","label":"a.ts","icon":"","title":""}]"#,
            "paragraphDirs": "auto",
        ]]]))
        XCTAssertEqual(directions(f, [0, 20, 28, 60, 63, 74, 75]), [.rightToLeft, .leftToRight, .rightToLeft, .leftToRight, .leftToRight, .leftToRight, .leftToRight])
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
        XCTAssertEqual(directions(f, [0, 20, 28, 60, 63, 74, 75]), [.rightToLeft, .leftToRight, .rightToLeft, .rightToLeft, .leftToRight, .rightToLeft, .leftToRight])
        // A caret on a footnote's line reveals its syntax; the answer stays.
        for caret in [65, 80] {
            f.selectedRange = NSRange(location: caret, length: 0)
            node.restyleMarkup()
            XCTAssertEqual(directions(f, [63, 75]), [.leftToRight, .leftToRight], "caret at \(caret)")
        }
    }

    /// LLP 1045 D11 on UIKit: a word stays with its line through typing,
    /// Return, undo and a composition, until the app's next words.
    func testAWordStaysWithItsLineThroughTypingReturnUndoAndComposition() throws {
        let (p, node, f, session) = styledField("A\nب\nC")
        defer { session.destroy() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["paragraphDirs": "ltr rtl ltr"]]]))
        XCTAssertTrue(f.becomeFirstResponder())
        f.undoManager?.removeAllActions()
        f.selectedRange = NSRange(location: 1, length: 0)
        f.insertText("\n")
        XCTAssertEqual(f.text, "A\n\nب\nC")
        XCTAssertEqual(directions(f, [0, 2, 3, 5]), [.leftToRight, .leftToRight, .rightToLeft, .leftToRight])
        let manager = try XCTUnwrap(f.undoManager)
        // UIKit closes the typing's undo group at the end of the run loop turn.
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        manager.undo()
        XCTAssertEqual(f.text, "A\nب\nC")
        XCTAssertEqual(directions(f, [0, 2, 4]), [.leftToRight, .rightToLeft, .leftToRight])
        f.selectedRange = NSRange(location: 3, length: 0)
        f.insertText("x")
        XCTAssertEqual(directions(f, [0, 2, 5]), [.leftToRight, .rightToLeft, .leftToRight])
        f.setMarkedText("ج", selectedRange: NSRange(location: 1, length: 0))
        XCTAssertNotNil(f.markedTextRange)
        f.unmarkText()
        XCTAssertEqual(f.text, "A\nبxج\nC")
        XCTAssertEqual(directions(f, [0, 2, 6]), [.leftToRight, .rightToLeft, .leftToRight])
        node.props["value"] = f.text
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": f.text ?? "", "paragraphDirs": "rtl"]]]))
        XCTAssertEqual(directions(f, [0, 2, 6]), [.rightToLeft, .rightToLeft, .rightToLeft])
    }
}
#endif
