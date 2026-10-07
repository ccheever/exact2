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
        XCTAssertTrue(editor is FieldEditor, "a field heard by a clipboard handler takes its own editor")
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

    func testAFieldNobodyHearsKeepsTheWindowsEditor() throws {
        let p = presenter(textarea: [])
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        XCTAssertFalse(field.currentEditor() is FieldEditor)
        XCTAssertTrue(heard.isEmpty)
    }
}
#endif
