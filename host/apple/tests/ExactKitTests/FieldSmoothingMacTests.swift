#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1019 D12, Fields: `-webkit-font-smoothing: antialiased` reaches the
/// text AppKit draws for a field. An input's cell and a textarea's text view
/// draw with less ink than an `auto` one, its editor too; the window's one
/// field editor draws the next `auto` field as AppKit does.
final class FieldSmoothingMacTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession?
    override func tearDown() { window?.close(); window = nil; session?.destroy(); session = nil }

    private func ink(_ view: NSView) throws -> Int {
        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: rep)
        var sum = 0
        for y in 0..<rep.pixelsHigh { for x in 0..<rep.pixelsWide {
            guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
            sum += Int(((1 - c.brightnessComponent) * c.alphaComponent * 255).rounded())
        } }
        XCTAssertGreaterThan(sum, 0, "the field draws ink")
        return sum
    }

    func testACaptureWithoutInkIsRejected() throws {
        _ = NSApplication.shared
        let view = NSView(frame: NSRect(x: 0, y: 0, width: 8, height: 8))
        _ = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        XCTExpectFailure("zero ink cannot prove that antialiasing reduces a field's ink") { _ = try? ink(view) }
    }

    func testAnAntialiasedFieldDrawsWithoutFontSmoothing() throws {
        _ = NSApplication.shared
        let p = Presenter()
        // A session for the text engine a field styles with.
        session = ExactApp.shared.makeSession(label: "field-smoothing")
        p.session = session
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let black: [Int] = [0, 0, 0, 255]
        func style(_ smoothing: String) -> [String: Any] {
            ["appearance": "none", "text_color": black, "font_size": 14.0, "font_weight": 500.0, "font_smoothing": smoothing]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "input", "props": ["value": "Settings General"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["value": "Settings General"]],
            ["op": "create", "id": 4, "kind": "textarea", "props": ["value": "Settings General"]],
            ["op": "create", "id": 5, "kind": "textarea", "props": ["value": "Settings General"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 24],
            ["op": "frame", "id": 3, "x": 0, "y": 30, "w": 300, "h": 24],
            ["op": "frame", "id": 4, "x": 0, "y": 60, "w": 300, "h": 40],
            ["op": "frame", "id": 5, "x": 0, "y": 110, "w": 300, "h": 40],
            ["op": "style", "id": 2, "style": style("auto")],
            ["op": "style", "id": 3, "style": style("antialiased")],
            ["op": "style", "id": 4, "style": style("auto")],
            ["op": "style", "id": 5, "style": style("antialiased")],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let auto = try XCTUnwrap(p.views[2]?.field), antialiased = try XCTUnwrap(p.views[3]?.field)
        let cells = (try ink(auto), try ink(antialiased))
        XCTAssertLessThan(Double(cells.1), Double(cells.0) * 0.97, "an input's cell: \(cells)")

        let plainArea = try XCTUnwrap(p.views[4]?.textArea), area = try XCTUnwrap(p.views[5]?.textArea)
        XCTAssertNil(plainArea.textLayoutManager?.delegate, "an `auto` textarea keeps AppKit's layout")
        XCTAssertNotNil(area.textLayoutManager?.delegate)
        let areas = (try ink(plainArea), try ink(area))
        XCTAssertLessThan(Double(areas.1), Double(areas.0) * 0.97, "a textarea: \(areas)")

        XCTAssertTrue(window.makeFirstResponder(antialiased))
        let editor = try XCTUnwrap(antialiased.currentEditor() as? NSTextView)
        XCTAssertNotNil(editor.textLayoutManager?.delegate, "the editor of an `antialiased` input")
        XCTAssertTrue(window.makeFirstResponder(auto))
        XCTAssertTrue(auto.currentEditor() === editor)
        XCTAssertNil(editor.textLayoutManager?.delegate, "the same editor, for an `auto` input, is AppKit's again")

        // A change of the row lays the text out again in the other fragments.
        p.apply(wireBatch([["op": "style", "id": 5, "style": style("auto")]]))
        XCTAssertNil(area.textLayoutManager?.delegate)
        XCTAssertEqual(try ink(area), areas.0, "drawn as an `auto` textarea again")
    }
}
#endif
