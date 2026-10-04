// Shared Apple rank checks; the iOS runner selects *IOSTests by name.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import XCTest
@testable import ExactKit

final class PaintOrderIOSTests: XCTestCase {
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
        XCTAssertEqual(parent.paintRank, 1)
        XCTAssertGreaterThan(lift.paintZPosition, ghost.paintZPosition)
        XCTAssertGreaterThan(ghost.paintZPosition, p.views[5]!.paintZPosition)
        parent.setRank(10)
        p.apply(wireBatch([["op": "exit", "id": 4], ["op": "destroy", "id": 2]]))
        XCTAssertEqual(parent.paintRank, 1, "the second ghost keeps its parent stacking")
        p.apply(wireBatch([["op": "destroy", "id": 4]]))
        XCTAssertEqual(parent.paintRank, 10, "the latest kernel answer survives the override")
        lift.setLifted(false)
        XCTAssertEqual(lift.paintZPosition, 0)
    }
}
