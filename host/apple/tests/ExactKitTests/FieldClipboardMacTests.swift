#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// #125 on AppKit: ⌘V, ⌘C and ⌘X in a textarea or an input — the Edit
/// menu's paste:, copy: and cut: through the responder chain — are DOM's
/// `paste`, `copy` and `cut` at the nearest handler, before the editor's
/// own, which an action's `preventDefault()` cancels.
final class FieldClipboardMacTests: XCTestCase {
    private var window: NSWindow!
    private var heard: [String] = []
    private var saved: String?

    override func setUp() { saved = NSPasteboard.general.string(forType: .string) }
    override func tearDown() {
        window?.close(); window = nil
        NSPasteboard.general.clearContents()
        if let saved { NSPasteboard.general.setString(saved, forType: .string) }
    }

    private func presenter(textarea: [String], box: [String] = []) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        // An action that prevents a paste of "BLOCK" stands in for one calling preventDefault().
        p.onClipboard = { [unowned self, unowned p] id, kind, text in
            heard.append("\(id):\(kind):\(text)")
            if text == "BLOCK" { p.defaultPrevented = true }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": box],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input"] + textarea],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 24.0],
        ]))
        return p
    }

    private func pasteboard(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    func testATextareasEditActionsAreItsClipboardEventsFirst() throws {
        let p = presenter(textarea: ["paste", "copy", "cut"])
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(editor))
        editor.setSelectedRange(NSRange(location: 5, length: 0))
        pasteboard(" world")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(editor.string, "hello world", "an unprevented paste is the editor's")
        editor.setSelectedRange(NSRange(location: 0, length: 5))
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.copy(_:)), with: nil))
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "hello")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.cut(_:)), with: nil))
        XCTAssertEqual(editor.string, " world")
        XCTAssertEqual(heard, ["2:34: world", "2:32:", "2:33:"], "a cut's own copy is no second event")
        pasteboard("BLOCK")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(editor.string, " world", "a prevented paste inserts nothing")
        XCTAssertEqual(heard.last, "2:34:BLOCK")
    }

    func testAnInputsEditorFiresAtAnAncestorsHandler() throws {
        let p = presenter(textarea: [], box: ["paste"])
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertTrue(editor is FieldEditor, "the session's editor, which says what the window's does not")
        editor.setSelectedRange(NSRange(location: 5, length: 0))
        pasteboard("BLOCK")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(editor.string, "hello")
        pasteboard("!")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(editor.string, "hello!")
        XCTAssertEqual(heard, ["1:34:BLOCK", "1:34:!"])
        // No copy handler: the editor's own copy, no event.
        editor.selectAll(nil)
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.copy(_:)), with: nil))
        XCTAssertEqual(heard.count, 2)
    }

    /// A field nobody hears edits in the session's editor too (it tells
    /// its node of marked text, `FieldCompositionMacTests`), and its edit
    /// actions are the editor's own: no event, nothing prevented.
    func testAFieldNobodyHearsFiresNothingFromItsEditor() throws {
        let p = presenter(textarea: [])
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertTrue(editor is FieldEditor)
        editor.setSelectedRange(NSRange(location: 5, length: 0))
        pasteboard("BLOCK")
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(editor.string, "helloBLOCK", "the paste nobody hears is the editor's")
        editor.selectAll(nil)
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.copy(_:)), with: nil))
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "helloBLOCK")
        XCTAssertTrue(heard.isEmpty)
    }

    private func png() -> Data {
        NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 2, pixelsHigh: 2, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!.representation(using: .png, properties: [:])!
    }
    private func item(_ action: Selector) -> NSMenuItem { NSMenuItem(title: "", action: action, keyEquivalent: "") }

    /// Paste and Match Style (`pasteAsPlainText:`) is a `paste`, as Chrome
    /// fires it, which `preventDefault()` cancels as it does a paste's.
    func testPasteAndMatchStyleIsAPaste() throws {
        let p = presenter(textarea: [], box: ["paste"])
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(editor))
        editor.setSelectedRange(NSRange(location: 5, length: 0))
        pasteboard(" world")
        XCTAssertTrue(editor.tryToPerform(#selector(NSTextView.pasteAsPlainText(_:)), with: nil))
        XCTAssertEqual(editor.string, "hello world")
        pasteboard("BLOCK")
        XCTAssertTrue(editor.tryToPerform(#selector(NSTextView.pasteAsPlainText(_:)), with: nil))
        XCTAssertEqual(editor.string, "hello world")
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let line = try XCTUnwrap(field.currentEditor() as? NSTextView)
        line.setSelectedRange(NSRange(location: 5, length: 0))
        pasteboard("!")
        XCTAssertTrue(line.tryToPerform(#selector(NSTextView.pasteAsPlainText(_:)), with: nil))
        XCTAssertEqual(line.string, "hello!")
        XCTAssertEqual(heard, ["1:34: world", "1:34:BLOCK", "1:34:!"])
    }

    /// Image data alone on the pasteboard: Paste and Paste and Match Style
    /// are enabled in a field with a `paste` handler above it, which hears
    /// the paste and inserts nothing, as Chrome's Paste is enabled for
    /// `clipboardData.files`; a field nobody hears keeps AppKit's answer.
    func testImageDataAloneEnablesPasteOnlyWhereAPasteIsHeard() throws {
        let p = presenter(textarea: ["paste"])
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(editor))
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setData(png(), forType: .png)
        XCTAssertTrue(editor.validateMenuItem(item(#selector(NSText.paste(_:)))))
        XCTAssertTrue(editor.validateMenuItem(item(#selector(NSTextView.pasteAsPlainText(_:)))))
        XCTAssertTrue(editor.tryToPerform(#selector(NSText.paste(_:)), with: nil))
        XCTAssertEqual(heard, ["2:34:"], "heard (a presenter with no session mints no file)")
        XCTAssertEqual(editor.string, "hello", "and nothing inserted")
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let line = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertFalse(line.validateMenuItem(item(#selector(NSText.paste(_:)))), "no handler: AppKit's")
    }

    func testASheetWithNothingFocusedHearsTheRootsEditActions() throws {
        let page = """
        component App
          state edits = 0
          action edited
            edits = edits + 1
          view
            column copy=edited cut=edited paste=edited testId="root"
              button commandfor="sheet" command="show-modal" testId="open"
                text "Open"
              dialog id="sheet" width=300 padding=20
                input testId="field" placeholder="Title" width=240 height=24
                button commandfor="sheet" command="close" testId="close"
                  text "Close"

        """
        let f = try SheetFixture(page: page)
        defer { f.close() }
        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet)
        XCTAssertTrue(sheet.makeFirstResponder(nil))
        let chain = sequence(first: sheet as NSResponder, next: \.nextResponder)
        let edits = try XCTUnwrap(chain.compactMap { $0 as? RootEdits }.first, "the sheet joins the root responder")
        let root = try f.node("root")
        var events: [UInt32] = []
        f.presenter.onClipboard = { id, kind, _ in XCTAssertEqual(id, root.id); events.append(kind) }
        pasteboard("from the sheet")
        for action in [#selector(NSText.copy(_:)), #selector(NSText.cut(_:)), #selector(NSText.paste(_:)), #selector(NSTextView.pasteAsPlainText(_:))] {
            XCTAssertTrue(edits.validateMenuItem(item(action)))
            XCTAssertTrue(sheet.tryToPerform(action, with: nil))
        }
        XCTAssertEqual(events, [32, 33, 34, 34])
        XCTAssertTrue(sheet.makeFirstResponder(try XCTUnwrap(f.node("field").field)))
        XCTAssertFalse(edits.validateMenuItem(item(#selector(NSText.paste(_:)))), "the field editor handles its own paste")
        try f.press("close")
        XCTAssertFalse(sequence(first: sheet as NSResponder, next: \.nextResponder).contains { $0 === edits })
    }

    /// With nothing focused the DOM sends a cut, copy or paste to `<body>`:
    /// the root's handler hears it, and Edit's item is enabled while it does.
    func testWithNothingFocusedTheRootHearsAPaste() throws {
        let p = presenter(textarea: [], box: ["paste"])
        let edits = RootEdits()
        edits.presenter = p
        edits.join(window)
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertTrue(edits.validateMenuItem(item(#selector(NSText.paste(_:)))))
        XCTAssertFalse(edits.validateMenuItem(item(#selector(NSText.copy(_:)))), "no copy handler")
        pasteboard("x")
        XCTAssertTrue(window.tryToPerform(#selector(NSText.paste(_:)), with: nil), "after the window in its responder chain")
        XCTAssertEqual(heard, ["1:34:x"])
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[2]?.textArea)))
        XCTAssertFalse(edits.validateMenuItem(item(#selector(NSText.paste(_:)))), "a focused node's own")
        edits.leave(window)
        XCTAssertNil(window.nextResponder)
    }
}
#endif
