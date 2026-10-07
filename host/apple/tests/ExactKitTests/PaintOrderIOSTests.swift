// Shared Apple rank checks; the iOS runner selects *IOSTests by name.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import XCTest
@testable import ExactKit

final class PaintOrderIOSTests: XCTestCase {
    #if os(macOS)
    private final class HitHolder: NSView {
        var traversals = 0
        override func hitTest(_ point: NSPoint) -> NSView? {
            traversals += 1
            return raisedHit(super.hitTest(point), point)
        }
    }

    func testNestedMacHoldersVisitEachHitBranchOnce() {
        _ = NSApplication.shared
        for depth in [8, 16] {
            let holders = (0..<depth).map { _ in HitHolder(frame: NSRect(x: 0, y: 0, width: 100, height: 100)) }
            for i in 1..<depth { holders[i - 1].addSubview(holders[i]) }
            XCTAssertTrue(holders[0].hitTest(NSPoint(x: 20, y: 20)) === holders.last)
            let traversals = holders.reduce(0) { $0 + $1.traversals }
            print("paint-order: depth \(depth), \(traversals) hit traversals")
            XCTAssertEqual(traversals, depth, "AppKit's successful branch is reused at every holder")
        }
    }
    #endif

    private func fixture(_ style: [String: Any] = [:]) -> Presenter {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        var ops: [[String: Any]] = (1...5).map { ["op": "create", "id": $0, "kind": "view", "props": ["testId": "n\($0)"]] }
        ops += [["op": "style", "id": 1, "style": style],
                ["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]]]
        ops += (1...5).map { ["op": "frame", "id": $0, "x": 0, "y": 0, "w": 100, "h": 100] }
        p.apply(wireBatch(ops))
        return p
    }

    func testTenThousandEqualRankChildrenMountInOneBatch() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        let parent = NodeView(id: 1, kind: "view", presenter: p)
        let children = (2...10_001).map { NodeView(id: UInt32($0), kind: "view", presenter: p) }
        let start = CFAbsoluteTimeGetCurrent()
        PaintOrder.begin()
        for child in children {
            parent.addSubview(child)
            child.setRank(1)
        }
        // Nothing scans the growing list during the transaction.
        XCTAssertTrue(children.allSatisfy { $0.paintZPosition == 0 })
        PaintOrder.end()
        let ms = (CFAbsoluteTimeGetCurrent() - start) * 1000
        print("paint-order: mounted 10000 rank-1 children in one batch: \(ms) ms")
        XCTAssertTrue(children.allSatisfy { $0.paintZPosition == 0.001 })
        XCTAssertTrue(NodeView.hitOrder(parent.subviews).first === children.last)
    }

    func testDenseRanksAreLosslessAndRebaseAfterRemoval() throws {
        let p = fixture()
        p.apply(wireBatch([["op": "rank", "id": 2, "rank": -4_294_967_290],
                          ["op": "rank", "id": 3, "rank": 1],
                          ["op": "rank", "id": 4, "rank": 4_294_967_288],
                          ["op": "rank", "id": 5, "rank": 4_294_967_290]]))
        XCTAssertEqual((2...5).map { p.views[UInt32($0)]!.paintZPosition }, [-0.001, 0.001, 0.002, 0.003])
        XCTAssertEqual(p.views[5]?.rank, 4_294_967_290)
        p.apply(wireBatch([["op": "destroy", "id": 4]]))
        XCTAssertEqual(p.views[5]?.paintZPosition, 0.002)
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 0]]))
        XCTAssertEqual(p.views[5]?.paintZPosition, 0.001)
        XCTAssertEqual(p.views[3]?.paintZPosition, 0)
    }

    #if os(iOS)
    /// A native navigation container takes rank ½, the positioned route
    /// holders' rank: over its in-flow siblings, among positioned ones where
    /// its subview place (the routes' place in the tree) puts it, and under
    /// a positive `z-index` overlay. A top-layer view (a popover, a
    /// snapshot) stays over every ranked sibling. Chat2 diary: a root sheet
    /// after the routes, positioned with no `z-index`, lost its taps to the
    /// route's collection view.
    func testANavigationContainerPaintsAtTheRoutesPlace() throws {
        let p = fixture()
        let parent = try XCTUnwrap(p.views[1])
        // Twice the rank: 3 is positioned (½), 5 is `z-index: 30`.
        p.apply(wireBatch([["op": "rank", "id": 3, "rank": 1], ["op": "rank", "id": 5, "rank": 60]]))
        let container = UIView(), top = UIView()
        parent.container.addSubview(container)
        container.setPaintForeground(aboveAuthored: false)
        parent.container.sendSubviewToBack(container)
        p.views[2]?.setRank(1); p.views[2]?.setRank(0)
        XCTAssertEqual(container.layer.zPosition, p.views[3]!.paintZPosition, accuracy: 1e-9)
        XCTAssertGreaterThan(container.layer.zPosition, p.views[4]!.paintZPosition, "over an in-flow sibling after it")
        let order = { NodeView.hitOrder(parent.container.subviews) }
        XCTAssertTrue(order()[1] === p.views[3], "under a positioned sibling after it: the root sheet")
        XCTAssertTrue(order()[2] === container)
        parent.container.insertSubview(container, aboveSubview: p.views[3]!)
        XCTAssertTrue(order()[1] === container, "over a positioned sibling before it")
        XCTAssertGreaterThan(p.views[5]!.paintZPosition, container.layer.zPosition, "the overlay paints over the container")
        XCTAssertTrue(order().first === p.views[5], "and takes the touch first")
        parent.container.addSubview(top)
        top.setPaintForeground()
        XCTAssertGreaterThan(top.layer.zPosition, p.views[5]!.paintZPosition)
        container.setPaintForeground(false)
        XCTAssertEqual(container.layer.zPosition, 0)
    }
    #endif

    func testHitsUseRankAndLaterFirstAmongEqualsAcrossHolders() throws {
        let styles: [[String: Any]] = [[:], ["overflow_y": "scroll"], ["overflow_x": "hidden", "overflow_y": "hidden", "border_radius_top_left": 8]]
        for style in styles {
            let p = fixture(style)
            let parent = try XCTUnwrap(p.views[1]), top = try XCTUnwrap(p.views[2])
            p.apply(wireBatch([["op": "rank", "id": 2, "rank": 1]]))
            let children = parent.container.subviews
            XCTAssertTrue(NodeView.hitOrder(children).first === top)
            #if os(macOS)
            XCTAssertTrue(parent.hitTest(NSPoint(x: 20, y: 20)) === top)
            #else
            XCTAssertTrue(parent.hitTest(CGPoint(x: 20, y: 20), with: nil) === top)
            #endif
            for id in 3...5 { p.views[UInt32(id)]?.setRank(1) }
            XCTAssertEqual(NodeView.hitOrder(parent.container.subviews).compactMap { ($0 as? NodeView)?.id }, [5, 4, 3, 2])
            XCTAssertEqual(parent.container.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3, 4, 5])
        }
    }

    func testFilterPicturesFollowRanksAndTransientsImmediately() throws {
        let sub: [Double] = [-10, -10, 20, 20]
        let program = sub + [1] + [0, -1, -3] + sub + [0, 2, 2]
        try XCTSkipUnless(SvgFilterGPU.runs(program.map(Float.init)), "no GPU filter path")
        let p = fixture()
        let node = try XCTUnwrap(p.views[2])
        node.applyStyle(["background_color": [255, 0, 0, 255], "filter": .object(["p": .array(program.map { .number($0) })])])
        let picture = try XCTUnwrap(node.boxFilter?.picture)
        node.setRank(1)
        XCTAssertEqual(picture.zPosition, node.paintZPosition)
        p.views[5]?.setRank(10)
        node.setLifted(true)
        XCTAssertGreaterThan(picture.zPosition, p.views[5]!.paintZPosition)
        node.setLifted(false)
        XCTAssertEqual(picture.zPosition, 0.001)
        node.setGhost(true)
        XCTAssertGreaterThan(picture.zPosition, p.views[5]!.paintZPosition)
        node.setGhost(false)
        XCTAssertEqual(picture.zPosition, 0.001)
    }

    func testGhostsKeepSlotsLiftWinsAndParentRestoresLatestRank() throws {
        let p = fixture()
        let parent = try XCTUnwrap(p.views[1]), ghost = try XCTUnwrap(p.views[2]), lift = try XCTUnwrap(p.views[3])
        p.views[5]?.setRank(4_294_967_290)
        parent.setRank(-4)
        lift.setLifted(true)
        p.apply(wireBatch([["op": "exit", "id": 2], ["op": "children", "id": 1, "ids": [3, 4, 5]]]))
        XCTAssertTrue(parent.container.subviews.first === ghost)
        XCTAssertEqual(parent.paintRank, -4)
        XCTAssertGreaterThan(lift.paintZPosition, ghost.paintZPosition)
        XCTAssertGreaterThan(ghost.paintZPosition, p.views[5]!.paintZPosition)
        parent.setRank(10)
        p.apply(wireBatch([["op": "exit", "id": 4], ["op": "destroy", "id": 2]]))
        XCTAssertEqual(parent.paintRank, 10, "ghost containment preserves the parent's nonzero rank")
        parent.setRank(0)
        XCTAssertEqual(parent.paintRank, 1, "only an in-flow parent needs promotion for containment")
        parent.setRank(10)
        p.apply(wireBatch([["op": "destroy", "id": 4]]))
        XCTAssertEqual(parent.paintRank, 10, "the latest kernel answer survives the override")
        lift.setLifted(false)
        XCTAssertEqual(lift.paintZPosition, 0)
    }
}
