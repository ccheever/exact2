import XCTest
@testable import ExactKit

/// CSS Scroll Anchoring's selection and adjustment (`ScrollAnchoring.swift`,
/// #138 X23d), on a tree of rects.
final class ScrollAnchoringTests: XCTestCase {
    private final class Box {
        let name: String, rect: CGRect, children: [Box], excluded: Bool
        init(_ name: String, _ y: CGFloat, _ h: CGFloat, excluded: Bool = false, _ children: [Box] = []) {
            self.name = name; rect = CGRect(x: 0, y: y, width: 100, height: h); self.children = children; self.excluded = excluded
        }
    }
    private func select(_ boxes: [Box], port: CGRect) -> String? {
        ScrollAnchoring.select(boxes, port: port, children: { $0.children }, excluded: { $0.excluded }, rect: { $0.rect })?.name
    }
    private let port = CGRect(x: 0, y: 200, width: 100, height: 200)

    func testExamination() {
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 0, width: 100, height: 48), port: port), .skip)
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 250, width: 100, height: 0), port: port), .skip, "no area")
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 250, width: 100, height: 30), port: port), .select)
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 180, width: 100, height: 30), port: port), .descend)
    }

    func testTheFirstVisibleBoxInTreeOrderIsTheAnchor() {
        let rows = [Box("header", 0, 48), Box("a", 48, 140), Box("b", 188, 30), Box("c", 218, 30)]
        XCTAssertEqual(select(rows, port: port), "b", "a partly visible leaf is the anchor")
        XCTAssertEqual(select([Box("header", 0, 48), Box("c", 218, 30)], port: port), "c")
    }

    func testAPartlyVisibleBoxIsDescendedInto() {
        let section = Box("section", 100, 400, [Box("x", 100, 90), Box("y", 190, 30), Box("z", 220, 30)])
        XCTAssertEqual(select([section], port: port), "y")
        // Nothing inside fully or partly visible: the section itself.
        XCTAssertEqual(select([Box("section", 100, 400, [Box("x", 100, 20)])], port: port), "section")
    }

    func testExcludedSubtreesAreNotCandidates() {
        let rows = [Box("sticky", 200, 30, excluded: true), Box("row", 230, 30)]
        XCTAssertEqual(select(rows, port: port), "row")
        XCTAssertNil(select([Box("fixed", 200, 30, excluded: true)], port: port))
    }

    func testTheAdjustmentFollowsTheAnchorAndClamps() {
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 200, before: 198, after: 150, minimum: 0, maximum: 1000), 152)
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 200, before: 198, after: 400, minimum: 0, maximum: 300), 300)
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 20, before: 100, after: 0, minimum: 0, maximum: 300), 0)
    }

    func testOnlyLayoutPropertiesSuppress() {
        var changes = ScrollAnchoring.Changes()
        changes.note(1, from: ["background_color": "#fff"], to: ["background_color": "#000"])
        XCTAssertTrue(changes.layout.isEmpty)
        changes.note(2, from: ["height": 48], to: ["height": 0])
        changes.note(3, from: [:], to: ["position_type": "absolute"])
        XCTAssertEqual(changes.layout, [2, 3])
        XCTAssertEqual(changes.positioned, [3])
        changes.reset()
        XCTAssertTrue(changes.layout.isEmpty && changes.positioned.isEmpty)
    }
}
