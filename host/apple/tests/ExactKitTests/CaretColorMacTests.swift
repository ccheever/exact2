#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D7.9: with `caret-color` unsaid the caret is AppKit's own,
/// the colour a plain `NSTextView` and an `NSTextField`'s editor draw theirs
/// in (`textInsertionPointColor`, the person's accent), in a field, in a bare
/// field and in a textarea. A bare field's and a textarea's was the text's
/// colour, CSS's `auto`. A written `caret-color` is the author's.
final class CaretColorMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    func testAnUnsaidCaretIsAppKitsAndAWrittenOneTheAuthors() throws {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let red: [Int] = [255, 0, 0, 255], green: [Int] = [0, 128, 0, 255]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "input", "props": ["value": ""], "style": ["text_color": red]],
            ["op": "create", "id": 3, "kind": "input", "props": ["value": ""], "style": ["appearance": "none", "text_color": red]],
            ["op": "create", "id": 4, "kind": "textarea", "props": ["value": ""], "style": ["text_color": red]],
            ["op": "create", "id": 5, "kind": "textarea", "props": ["value": ""], "style": ["appearance": "none", "text_color": red]],
            ["op": "create", "id": 6, "kind": "input", "props": ["value": ""], "style": ["appearance": "none", "caret_color": green]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 24],
            ["op": "frame", "id": 3, "x": 0, "y": 30, "w": 300, "h": 24],
            ["op": "frame", "id": 4, "x": 0, "y": 60, "w": 300, "h": 60],
            ["op": "frame", "id": 5, "x": 0, "y": 130, "w": 300, "h": 60],
            ["op": "frame", "id": 6, "x": 0, "y": 200, "w": 300, "h": 24],
        ]))
        // What AppKit makes with nothing set.
        let appKit = NSTextView(frame: .zero).insertionPointColor
        XCTAssertEqual(appKit, .textInsertionPointColor)
        for id in [2, 3] as [UInt32] {
            let field = try XCTUnwrap(p.views[id]?.field)
            XCTAssertTrue(window.makeFirstResponder(field))
            let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
            XCTAssertEqual(editor.insertionPointColor, appKit, "field \(id): AppKit's caret, not its red text's colour")
        }
        for id in [4, 5] as [UInt32] {
            XCTAssertEqual(try XCTUnwrap(p.views[id]?.textArea).insertionPointColor, appKit, "textarea \(id)")
        }
        let written = try XCTUnwrap(p.views[6]?.field)
        XCTAssertTrue(window.makeFirstResponder(written))
        let editor = try XCTUnwrap(written.currentEditor() as? NSTextView)
        XCTAssertEqual(editor.insertionPointColor.usingColorSpace(.sRGB), NSColor(srgbRed: 0, green: 128 / 255, blue: 0, alpha: 1), "the author's `caret-color`")
        // The window's one field editor: the next field's caret is AppKit's again.
        let plain = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(window.makeFirstResponder(plain))
        XCTAssertEqual((plain.currentEditor() as? NSTextView)?.insertionPointColor, appKit)
    }
}
#endif
