#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class GroupedPressIOSTests: GroupedListFixture {
    func testActiveInkUsesTheLeadingSymbolsColorOwnership() throws {
        for ownsLeading in [false, true] {
            let p = presenter { self.model() }
            let red: [String: Any] = ["text_color": [255, 0, 0, 255], "written": ["text_color"]]
            p.apply(wireBatch([
                ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:sf/xmark"], "style": red.merging(["display": "none"]) { _, new in new }],
                ["op": "create", "id": 31, "kind": "image", "props": ["imageSource": "symbol:sf/person.circle", "symbolColorWritten": ownsLeading ? "1" : "0"], "style": ownsLeading ? red : [:]],
                ["op": "create", "id": 33, "kind": "text", "props": ["text": "Profile"], "style": [:]],
                ["op": "create", "id": 32, "kind": "image", "props": ["imageSource": "symbol:forward-chevron"], "style": ownsLeading ? [:] : red],
                ["op": "children", "id": 10, "ids": [30, 31, 33, 32]],
                ["op": "style", "id": 10, "style": ["press_color": [0, 0, 255, 255]]],
            ]))
            let native = try cell(p, 10)
            native.isHighlighted = true
            native.setNeedsUpdateConfiguration()
            waitForMainQueue()
            let content = try XCTUnwrap(native.contentConfiguration as? UIListContentConfiguration)
            XCTAssertEqual(content.image?.renderingMode, ownsLeading ? .alwaysOriginal : .alwaysTemplate)
            XCTAssertEqual(content.textProperties.color.resolvedColor(with: native.traitCollection), .blue)
            if ownsLeading {
                XCTAssertEqual((native as? GroupedCell)?.imageInk?.resolvedColor(with: native.traitCollection), .red)
            } else {
                XCTAssertEqual(content.imageProperties.tintColor?.resolvedColor(with: native.traitCollection), .blue)
            }
            XCTAssertFalse(native.accessories.isEmpty, "the native disclosure accessory stays")
            native.isHighlighted = false
            window.isHidden = true
        }
    }

    func testAStandardCellShowsAuthoredActivePaintWithoutReplacingItsContent() throws {
        let p = presenter { self.model() }
        let native = try cell(p, 10)
        native.isHighlighted = true
        native.setNeedsUpdateConfiguration()
        waitForMainQueue()
        let style: [String: Any] = ["press_background_color": [255, 0, 0, 255], "press_color": [0, 0, 255, 255], "press_opacity_to": 0.4, "press_scale": 0.95]
        p.apply(wireBatch([["op": "style", "id": 10, "style": style]]))
        waitForMainQueue()
        native.setNeedsUpdateConfiguration()
        waitForMainQueue()
        XCTAssertEqual(native.backgroundConfiguration?.backgroundColor?.resolvedColor(with: native.traitCollection), .red)
        XCTAssertEqual((native.contentConfiguration as? UIListContentConfiguration)?.text, "Profile")
        XCTAssertEqual((native.contentConfiguration as? UIListContentConfiguration)?.textProperties.color.resolvedColor(with: native.traitCollection), .blue)
        waitFor("the cell's active presentation", late: 1, cap: 2) {
            guard let shown = native.layer.presentation() else { return false }
            return abs(shown.opacity - 0.4) < 0.01 && abs(shown.sublayerTransform.m11 - 0.95) < 0.01
        }
        XCTAssertEqual(native.alpha, 1, "feedback leaves the model opacity and hit testing alone")
        p.apply(wireBatch([["op": "style", "id": 10, "style": [:]]]))
        waitForMainQueue()
        native.setNeedsUpdateConfiguration()
        waitForMainQueue()
        XCTAssertNotEqual(native.backgroundConfiguration?.backgroundColor?.resolvedColor(with: native.traitCollection), .red)
        XCTAssertEqual(native.alpha, 1, accuracy: 0.001)
        XCTAssertEqual(native.transform.a, 1, accuracy: 0.001)
        native.isHighlighted = false
    }

    func testAHeldStateUsesTheStandardCellsHighlight() throws {
        let p = presenter { self.model() }
        let row = try XCTUnwrap(p.views[10]), native = try cell(p, 10)
        XCTAssertEqual(row.hold("active", dropped: nil).held, "active")
        defer { _ = row.hold(nil, dropped: nil) }
        p.apply(wireBatch([["op": "style", "id": 10, "style": ["press_background_color": [255, 0, 0, 255]]]]))
        waitFor("the held native cell", late: 1, cap: 2) {
            native.backgroundConfiguration?.backgroundColor?.resolvedColor(with: native.traitCollection) == .red
        }
    }

    func testInkWrittenBelowTheRowStaysOwnedUntilItStartsInheriting() throws {
        var m = model()
        m.sections[0].rows[0].titleLook.color = .array([.number(0), .number(255), .number(0), .number(255)])
        let p = presenter { m }
        let native = try cell(p, 10)
        let font = (native.contentConfiguration as? UIListContentConfiguration)?.textProperties.font
        p.apply(wireBatch([
            ["op": "create", "id": 101, "kind": "text", "props": ["text": "Profile"], "style": ["text_color": [0, 255, 0, 255], "written": ["text_color"]]],
            ["op": "children", "id": 10, "ids": [101]],
            ["op": "style", "id": 10, "style": ["text_color": [0, 255, 0, 255], "written": ["text_color"], "press_background_color": [255, 0, 0, 255], "press_color": [0, 0, 255, 255]]],
        ]))
        native.isHighlighted = true
        native.setNeedsUpdateConfiguration()
        waitFor("the row's authored active ground", late: 1, cap: 2) {
            native.backgroundConfiguration?.backgroundColor?.resolvedColor(with: native.traitCollection) == .red
        }
        XCTAssertEqual((native.contentConfiguration as? UIListContentConfiguration)?.textProperties.color.resolvedColor(with: native.traitCollection), .green)
        p.apply(wireBatch([["op": "style", "id": 101, "style": ["text_color": [0, 255, 0, 255]]]]))
        waitFor("the title inheriting the active ink", late: 1, cap: 2) {
            (native.contentConfiguration as? UIListContentConfiguration)?.textProperties.color.resolvedColor(with: native.traitCollection) == .blue
        }
        XCTAssertEqual((native.contentConfiguration as? UIListContentConfiguration)?.textProperties.font, font)
        native.isHighlighted = false
    }
}
#endif
