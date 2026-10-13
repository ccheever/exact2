// @ref LLP 1115.002 D6 — the Mac's grouped form, where a row's look is a
// control's: a field with no bezel, AppKit's mini switch, and a push button
// round a `destructive` row's title. Each is what the sheet's metrics say
// in the Mac's column, stated here as the batch the kernel then sends.
#if os(macOS)
import AppKit
import CExact
import XCTest
@testable import ExactKit

final class FormRowsMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func form(_ ops: [[String: Any]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"], ["op": "roots", "ids": [1]],
                           ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 560.0, "h": 200.0]] + ops))
        return p
    }

    /// A field in a row has no bezel, no ground and no ring, and is measured
    /// as that: its chrome is nothing round one line, so the row stays 36
    /// points where a bezelled field made it 42. Out of a row it is the
    /// platform's field again. A text area in a row keeps its box.
    func testAFieldInAFormRowIsBorderlessAndMeasuredSo() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "form-row-field")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 16)
        node.applyProps(set: ["value": "Middlemarch"], clear: [])
        node.applyStyle(["font_size": .number(13), "text_align": .string("right")])
        node.applyFieldContent(["rect": [0.0, 0.0, 240.0, 16.0], "cell": true])
        let field = try XCTUnwrap(node.field)
        XCTAssertFalse(field.isBezeled)
        XCTAssertFalse(field.drawsBackground)
        XCTAssertEqual(field.focusRingType, .none)
        XCTAssertEqual(field.alignment, .right)
        node.applyFieldContent(["rect": [4.0, 3.0, 232.0, 16.0]])
        XCTAssertTrue(field.isBezeled)
        XCTAssertEqual(field.focusRingType, .default)

        let cache = FieldChromeCache()
        cache.configure(NSAppearance(named: .aqua)!, scale: 2)
        let engine = TextEngine(resolve: { _ in nil })
        let family = engine.registerControlFont(cache.body), weight = UInt16(TextEngine.controlWeight(cache.body))
        cache.prefill(family: family, font: cache.body, weight: weight, italic: false)
        func chrome(_ kind: UInt8) -> ExactFieldChrome {
            cache.answer(ExactFieldChromeRequest(kind: kind, family_id: family, size: Float(cache.body.pointSize), weight: weight, italic: 0), font: cache.body)
        }
        for kind in [UInt8(4), 5] {
            let bare = kind == 5 ? NSSecureTextField() : NSTextField()
            bare.isBezeled = false; bare.isBordered = false; bare.font = cache.body; bare.stringValue = "Hg"
            let cell = chrome(kind), bezelled = chrome(kind & 3)
            XCTAssertEqual(cell.provisional, 0, "measured with the others, before a first frame")
            XCTAssertEqual([cell.top, cell.right, cell.bottom, cell.left], [0, 0, 0, 0])
            XCTAssertEqual(cell.minimum_height, Float(bare.fittingSize.height))
            XCTAssertLessThan(cell.minimum_height, bezelled.minimum_height)
        }
        let area = chrome(3), areaInRow = chrome(7)
        XCTAssertEqual([areaInRow.top, areaInRow.left, areaInRow.minimum_height], [area.top, area.left, area.minimum_height])
    }

    /// A row's switch is the size its sheet names: AppKit's mini, 36 by 16,
    /// which is the one SwiftUI's grouped `Form` shows and leaves the row at
    /// 36 points. Unsaid, it is the regular one.
    func testAFormRowsSwitchIsTheSizeItsSheetNames() throws {
        let p = form([
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "checkbox", "accessibilityRole": "switch", "checked": "true"], "handlers": ["change"], "style": ["control_size": "mini"]],
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "checkbox", "accessibilityRole": "switch"], "handlers": ["change"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 36.0, "h": 16.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 54.0, "h": 24.0],
        ])
        let mini = try XCTUnwrap(p.controls.controls[2] as? NSSwitch), regular = try XCTUnwrap(p.controls.controls[3] as? NSSwitch)
        XCTAssertEqual(mini.controlSize, .mini)
        XCTAssertEqual(regular.controlSize, .regular)
        XCTAssertEqual(mini.state, .on)
        XCTAssertLessThan(mini.intrinsicContentSize.height, regular.intrinsicContentSize.height)
        XCTAssertEqual(mini.frame.size, mini.intrinsicContentSize, "the size it reports is the size it is drawn at")
    }

    /// A pressable row whose sheet says `appearance: auto` is a push button
    /// round its parts: from the row's padding edge to as far past its title
    /// as the title is in, as high as the row's content, under the title and
    /// taking no click of its own. A row that says nothing has none, and the
    /// bezel follows its title's width.
    func testADestructiveFormRowIsAPushButtonRoundItsTitle() throws {
        let row: [String: Any] = ["appearance": "auto", "padding_left": 12.0, "padding_top": 10.0, "padding_bottom": 10.0]
        let p = form([
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["testId": "remove"], "style": row],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Remove from Library"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["testId": "plain"], "style": ["padding_left": 12.0]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Sign Out"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "frame", "id": 2, "x": 10.0, "y": 0.0, "w": 500.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 12.0, "y": 14.0, "w": 126.0, "h": 16.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 50.0, "w": 500.0, "h": 36.0],
            ["op": "frame", "id": 5, "x": 12.0, "y": 10.0, "w": 60.0, "h": 16.0],
        ])
        let remove = try XCTUnwrap(p.views[2]), plain = try XCTUnwrap(p.views[4])
        remove.layoutSubtreeIfNeeded(); plain.layoutSubtreeIfNeeded()
        func bezel(_ node: NodeView) -> NSButton? { node.subviews.compactMap { $0 as? NSButton }.first }
        let button = try XCTUnwrap(bezel(remove))
        XCTAssertNil(bezel(plain))
        XCTAssertEqual(button.bezelStyle, .push)
        XCTAssertEqual(button.alignmentRect(forFrame: button.frame), NSRect(x: 0, y: 10, width: 150, height: 24))
        XCTAssertTrue(remove.subviews.first === button, "under the row's parts")
        XCTAssertNil(button.hitTest(NSPoint(x: 20, y: 20)), "the row takes the press")
        XCTAssertFalse(button.acceptsFirstResponder)
        XCTAssertEqual((remove.rowBezelObservation?["frame"] as? [Double]) ?? [], [0, 10, 150, 24])
        // A longer title, and the bezel is as much longer.
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 12.0, "y": 14.0, "w": 160.0, "h": 16.0]]))
        remove.layoutSubtreeIfNeeded()
        XCTAssertEqual(button.alignmentRect(forFrame: button.frame).width, 184)
        // The sheet's row gone (an author's `appearance`), the bezel goes.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["appearance": "none", "padding_left": 12.0]]]))
        remove.layoutSubtreeIfNeeded()
        XCTAssertNil(bezel(remove))
    }
}
#endif
