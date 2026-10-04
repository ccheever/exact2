#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// An inert leaf box is a bare layer in its parent's (LLP 1068 §6.1): its
/// place among the parent's views, its promotion to a view when it needs
/// one, and hits falling to its parent. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class FlatLeavesIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    private func bar(_ id: Int, x: Double, color: [Int] = [0, 122, 255, 255]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view",
          "style": ["width": 3.0, "height": 20.0, "background_color": color,
                    "border_radius_top_left": 1.5, "border_radius_top_right": 1.5,
                    "border_radius_bottom_right": 1.5, "border_radius_bottom_left": 1.5, "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": 3.0, "h": 20.0]]
    }
    /// A pressable row (1) holding bars 2 and 4 around a labelled view 3.
    private func row() -> [[String: Any]] {
        [["op": "create", "id": 1, "kind": "view", "handlers": ["press"]]]
            + bar(2, x: 0)
            + [["op": "create", "id": 3, "kind": "view", "props": ["testId": "middle"]],
               ["op": "frame", "id": 3, "x": 5.0, "y": 0.0, "w": 3.0, "h": 20.0]]
            + bar(4, x: 10)
            + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 20.0]]
    }

    func testAnInertLeafIsALayerInItsPlaceAmongTheViews() throws {
        let p = presenter(row())
        XCTAssertNil(p.views[2]); XCTAssertNil(p.views[4])
        let parent = try XCTUnwrap(p.views[1]), middle = try XCTUnwrap(p.views[3])
        XCTAssertEqual(parent.subviews.compactMap { ($0 as? NodeView)?.id }, [3])
        let sub = try XCTUnwrap(parent.layer.sublayers)
        let a = try XCTUnwrap(sub.firstIndex { $0.frame == CGRect(x: 0, y: 0, width: 3, height: 20) })
        let m = try XCTUnwrap(sub.firstIndex { $0 === middle.layer })
        let b = try XCTUnwrap(sub.firstIndex { $0.frame == CGRect(x: 10, y: 0, width: 3, height: 20) })
        XCTAssertTrue(a < m && m < b, "tree order: \(a) \(m) \(b)")
        let leaf = sub[a]
        XCTAssertEqual(leaf.cornerRadius, 1.5)
        XCTAssertEqual(leaf.backgroundColor, UIColor(red: 0, green: 122 / 255, blue: 1, alpha: 1).cgColor)
        XCTAssertEqual(p.flats.observation["flatLeaves"] as? Int, 2)
    }

    func testALeafThatNeedsAViewBecomesOneInItsPlace() throws {
        let p = presenter(row())
        // A border is paint a flat leaf does not carry: it is promoted.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["width": 3.0, "height": 20.0, "border_width": 1.0,
                                                              "border_color_top": [0, 0, 0, 255], "text_color": [0, 0, 0, 255]]]]))
        let promoted = try XCTUnwrap(p.views[2])
        let parent = try XCTUnwrap(p.views[1])
        XCTAssertEqual(parent.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3], "before the middle view")
        XCTAssertEqual(promoted.frame, CGRect(x: 0, y: 0, width: 3, height: 20))
        XCTAssertTrue(parent.layer.sublayers?.contains { $0.frame == CGRect(x: 0, y: 0, width: 3, height: 20) && $0 !== promoted.layer } == false,
                      "the leaf's layer is gone")
        // A prop promotes too, and the prop lands on the view.
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["testId": "last"]]]))
        XCTAssertEqual(p.views[4]?.props["testId"], "last")
        XCTAssertEqual(parent.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3, 4])
        XCTAssertEqual(p.flats.observation["flatLeaves"] as? Int, 0)
        // A destroyed leaf leaves nothing behind.
        let q = presenter(row())
        let layers = q.views[1]?.layer.sublayers?.count ?? 0
        q.apply(wireBatch([["op": "destroy", "id": 4], ["op": "children", "id": 1, "ids": [2, 3]]]))
        XCTAssertEqual(q.views[1]?.layer.sublayers?.count, layers - 1)
    }

    func testATouchOnALeafIsItsParents() throws {
        let p = presenter(row())
        let parent = try XCTUnwrap(p.views[1])
        window.layoutIfNeeded()
        let hit = window.hitTest(parent.convert(CGPoint(x: 1, y: 10), to: window), with: nil)
        XCTAssertTrue(hit === parent, "the row, as the web bubbles a press from a box with no handler: \(String(describing: hit))")
    }

    func testRanksPromoteDemoteAndSplitRuns() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view", "handlers": ["press"]]]
            + bar(2, x: 0) + bar(3, x: 5) + bar(4, x: 10)
            + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]])
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 1)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 1]]))
        XCTAssertEqual(p.views[3]?.paintZPosition, 0.001)
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 2)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0]]))
        XCTAssertNil(p.views[3])
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).first?.leaves, [2, 3, 4])
        // Position is no longer an inert layout row, even before rank arrives.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["position_type": "absolute"]],
                          ["op": "rank", "id": 3, "rank": 1]]))
        XCTAssertNotNil(p.views[3])
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0],
                          ["op": "style", "id": 3, "style": ["position_type": "static"]]]))
        XCTAssertNil(p.views[3])
    }

    func testFlatLeavesOccludeViewsByRankAndTreeOrderIncludingRunGaps() throws {
        let p = presenter(row())
        let parent = try XCTUnwrap(p.views[1]), middle = try XCTUnwrap(p.views[3])
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0, "y": 0, "w": 20, "h": 20],
                          ["op": "rank", "id": 3, "rank": -2]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === parent)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 11, y: 10), with: nil) === parent)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 7, y: 10), with: nil) === middle)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === middle)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 11, y: 10), with: nil) === parent)
        // Adjacent leaves merge into a run, but its gaps do not occlude.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3, 2, 4]]]))
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 1)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 11, y: 10), with: nil) === parent)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 7, y: 10), with: nil) === middle)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 1]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 11, y: 10), with: nil) === middle)
    }

    func testALeafUnderAListOrWithMotionIsAView() throws {
        // A row root sits in a list's scroll view: it is a view.
        let p = presenter([["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]] + bar(2, x: 0)
            + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        XCTAssertNotNil(p.views[2])
        // Opacity stays on the layer; a translate promotes.
        let q = presenter(row())
        q.apply(wireBatch([["op": "present", "id": 2, "property": "opacity", "x": 0.5, "y": 0.0, "w": 0.0, "h": 0.0]]))
        XCTAssertNil(q.views[2])
        q.apply(wireBatch([["op": "present", "id": 4, "property": "translate", "x": 3.0, "y": 0.0, "w": 0.0, "h": 0.0]]))
        XCTAssertNotNil(q.views[4])
    }
    func testAListRowWithLeavesParksAndIsTakenWithFreshLeaves() throws {
        func rowOps(_ base: Int) -> [[String: Any]] {
            [["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"]]]
                + bar(base + 1, x: 0) + bar(base + 2, x: 5)
                + [["op": "children", "id": base, "ids": [base + 1, base + 2]],
                   ["op": "frame", "id": base, "x": 0.0, "y": 0.0, "w": 300.0, "h": 20.0]]
        }
        func collections(_ view: Int) -> [String: Any] {
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
                "rows": [["view": view, "root": view, "epoch": 1]], "correction": NSNull()]]]
        }
        let p = presenter([collections(10), ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rowOps(10) + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
                            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
                            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]])
        let row = try XCTUnwrap(p.views[10])
        XCTAssertNil(p.views[11])
        let before = row.layer.sublayers?.count ?? 0
        p.apply(wireBatch([collections(20), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11], ["op": "destroy", "id": 12]]
            + rowOps(20) + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[20] === row, "the row's view is reused around its leaves")
        XCTAssertNil(p.views[21]); XCTAssertNil(p.views[22])
        XCTAssertEqual(row.layer.sublayers?.count ?? 0, before, "the old leaves' layers went, the new ones came")
        XCTAssertEqual(p.flats.observation["flatLeaves"] as? Int, 2)
    }
    func testAlikeAdjacentLeavesAreOneShapeLayerUntilOneDiffers() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view"]]
            + bar(2, x: 0) + bar(3, x: 5) + bar(4, x: 10) + bar(5, x: 15, color: [199, 199, 204, 255])
            + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 20.0]])
        let parent = try XCTUnwrap(p.views[1])
        var shapes = parent.layer.sublayers?.compactMap { $0 as? CAShapeLayer } ?? []
        XCTAssertEqual(shapes.count, 1, "2, 3 and 4 are one run")
        XCTAssertEqual(shapes.first?.path?.boundingBoxOfPath, CGRect(x: 0, y: 0, width: 13, height: 20))
        XCTAssertEqual(parent.layer.sublayers?.count, 2, "the run, and 5 on its own")
        let last = try XCTUnwrap(parent.layer.sublayers?.last)
        XCTAssertFalse(last is CAShapeLayer); XCTAssertEqual(last.frame, CGRect(x: 15, y: 0, width: 3, height: 20))
        // The middle one fades: it stands alone, and the run splits around it.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "opacity", "x": 0.5, "y": 0.0, "w": 0.0, "h": 0.0]]))
        shapes = parent.layer.sublayers?.compactMap { $0 as? CAShapeLayer } ?? []
        XCTAssertEqual(shapes.count, 0)
        XCTAssertEqual(parent.layer.sublayers?.count, 4)
        XCTAssertEqual(parent.layer.sublayers?.map(\.frame.minX), [0, 5, 10, 15], "in tree order")
    }
    func testRanksPromoteDemoteAndSplitRunsAfterTheWholeBatch() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view", "handlers": ["press"]]]
            + bar(2, x: 0) + bar(3, x: 5) + bar(4, x: 10)
            + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]])
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 1)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 1]]))
        XCTAssertNotNil(p.views[3]); XCTAssertFalse(p.flats.isFlat(3))
        XCTAssertEqual(p.views[3]?.paintZPosition, 0.001)
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 2, "no run crosses the ranked view")
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0]]))
        XCTAssertNil(p.views[3]); XCTAssertTrue(p.flats.isFlat(3))
        XCTAssertEqual(p.flats.inspectionLayers(under: 1).count, 1)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 1],
                          ["op": "style", "id": 3, "style": ["position_type": "relative"]]]))
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0],
                          ["op": "style", "id": 3, "style": ["background_color": [0, 122, 255, 255]]],
                          ["op": "props", "id": 3, "set": ["testId": "keep-view"]]]))
        XCTAssertNotNil(p.views[3], "the later prop prevents demotion")
    }

    func testPositionedOrInitiallyRankedLeavesStayViews() throws {
        let p = presenter(row() + [["op": "rank", "id": 2, "rank": -2]])
        XCTAssertNotNil(p.views[2]); XCTAssertFalse(p.flats.isFlat(2))
        XCTAssertNil(FlatPaint(["position_type": "relative"]))
        XCTAssertNil(FlatPaint(["position_type": "absolute"]))
        XCTAssertNotNil(FlatPaint(["position_type": "static"]))
    }

    func testFlatRunOccludesNegativeControlButNotItsGapsOrHigherRanks() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
                           ["op": "create", "id": 5, "kind": "view", "handlers": ["press"]],
                           ["op": "frame", "id": 5, "x": 0, "y": 0, "w": 100, "h": 20],
                           ["op": "rank", "id": 5, "rank": -2]]
            + bar(2, x: 0) + bar(3, x: 5)
            + [["op": "children", "id": 1, "ids": [2, 3, 5]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]])
        let parent = try XCTUnwrap(p.views[1]), control = try XCTUnwrap(p.views[5])
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === parent)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 4, y: 10), with: nil) === control)
        p.apply(wireBatch([["op": "rank", "id": 5, "rank": 1]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === control)
        p.apply(wireBatch([["op": "rank", "id": 5, "rank": 0]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === control, "later equal rank wins")
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [5, 2, 3]]]))
        XCTAssertTrue(parent.hitTest(CGPoint(x: 1, y: 10), with: nil) === parent)
    }

}
#endif
