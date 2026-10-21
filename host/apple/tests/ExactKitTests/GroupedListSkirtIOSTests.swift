#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// A grouped list's background past a viewport short of its container's
/// edge (moved whole from GroupedListIOSTests.swift for its line cap).
final class GroupedListSkirtIOSTests: GroupedListFixture {
    /// LLP 1116.004 D2 (chat-rooms, round 6): a page with no navigation
    /// container keeps its viewport inside the safe area, and a grouped list
    /// at its bottom left the root's white under the home indicator. Where
    /// the list's box meets an edge of a viewport short of its container's,
    /// the list's background goes on to the container's edge, behind the
    /// viewport; not while the keyboard ends the viewport.
    func testAListAtTheViewportsEdgeDrawsItsBackgroundToTheScreensEdge() throws {
        let p = presenter(height: 840) { self.model() }
        let grey: [[Int]] = [[242, 242, 247, 255], [0, 0, 0, 255]]
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": grey]]]))
        // The page's area ends at the home indicator's 34 points, which the
        // viewport is drawn past (LLP 1130 D1).
        p.viewport.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        p.pageMargins = UIEdgeInsets(top: 0, left: 0, bottom: 34, right: 0)
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 840.0]]))
        let skirts = { p.viewport.subviews.filter { $0 !== p.root && !$0.isHidden && $0.frame.maxY == 874 } }
        let skirt = try XCTUnwrap(skirts().first, "the list's background reaches the screen's bottom edge")
        XCTAssertEqual(skirt.frame, CGRect(x: 0, y: 0, width: 402, height: 874))
        XCTAssertLessThan(try XCTUnwrap(p.viewport.subviews.firstIndex(of: skirt)), try XCTUnwrap(p.viewport.subviews.firstIndex(of: p.root)),
                          "behind the document: only what lies past its edge shows")
        XCTAssertFalse(skirt.isUserInteractionEnabled)
        func rgb(_ style: UIUserInterfaceStyle) throws -> [CGFloat] {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            try XCTUnwrap(skirt.backgroundColor).resolvedColor(with: UITraitCollection(userInterfaceStyle: style)).getRed(&r, green: &g, blue: &b, alpha: &a)
            return [r, g, b, a].map { ($0 * 255).rounded() }
        }
        XCTAssertEqual(try rgb(.light), [242, 242, 247, 255], "the list's grey, in light")
        XCTAssertEqual(try rgb(.dark), [0, 0, 0, 255], "and in dark")
        // Only past the list's box, where the list paints its own: a translucent colour is not laid over itself
        let mask = try XCTUnwrap((skirt.layer.mask as? CAShapeLayer)?.path)
        XCTAssertTrue(mask.contains(CGPoint(x: 200, y: 860), using: .evenOdd), "the strip under the home indicator")
        XCTAssertFalse(mask.contains(CGPoint(x: 200, y: 400), using: .evenOdd), "not the list's box")
        // The keyboard ends the viewport: the keys cover that edge.
        p.keyboardInset = 300
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 840.0]]))
        XCTAssertTrue(skirts().isEmpty)
        p.keyboardInset = 0
        // A list short of the viewport's edge draws nothing past it.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 600.0]]))
        XCTAssertTrue(skirts().isEmpty)
    }
}
#endif
