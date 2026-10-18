#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Boxes drawn on the device's pixel grid (`PixelGridIOS.swift`): each edge
/// at its absolute position rounded to a device pixel, a half up, as
/// Chromium rounds a painted rectangle. Layout positions here are whole
/// layout units (1/64 point), so a sum is the absolute position exactly.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class PixelGridIOSTests: XCTestCase {
    private var window: UIWindow!

    func testEachEdgeRoundsHalfUpAndTheSizeIsBetweenThem() {
        // At 3x, x = 10.1667 is 30.5 device pixels and rounds up to 31; its
        // right edge, 90.5, to 91. A 0.5 pt height from 0 is 0 to 1.5: 0 to 2.
        let p = PixelGrid.place(CGRect(x: 30.5 / 3, y: 0, width: 20, height: 0.5), in: .zero, scale: 3)
        XCTAssertEqual(p.drawn.minX * 3, 31, accuracy: 1e-9)
        XCTAssertEqual(p.drawn.maxX * 3, 91, accuracy: 1e-9)
        XCTAssertEqual(p.drawn.height * 3, 2, accuracy: 1e-9)
        XCTAssertEqual(p.origin.pixelX, 31)
        XCTAssertEqual(PixelGrid.round(-10.25, scale: 2), -10, "a half rounds up, below zero too")
    }

    func testEdgesThatMeetInLayoutMeetDrawnAcrossParents() {
        // At 3x, 11.5 and 1.5 points are halves (34.5 and 4.5 device
        // pixels): a child's edge there and a root box's edge there meet,
        // however the parent above the child was rounded.
        for (parentY, childY, height) in [(10.1875, 0.3125, 1.0), (0.5, 1.0, 0.0)] as [(CGFloat, CGFloat, CGFloat)] {
            let parent = PixelGrid.place(CGRect(x: 0, y: parentY, width: 100, height: 4), in: .zero, scale: 3)
            let child = PixelGrid.place(CGRect(x: 0, y: childY, width: 100, height: height), in: parent.origin, scale: 3)
            let other = PixelGrid.place(CGRect(x: 0, y: parentY + childY + height, width: 100, height: 1), in: .zero, scale: 3)
            XCTAssertEqual(parent.drawn.minY + child.drawn.maxY, other.drawn.minY, accuracy: 1e-9, "parent at \(parentY)")
            XCTAssertEqual(other.drawn.minY * 3, ((parentY + childY + height) * 3 + 0.5).rounded(.down), accuracy: 1e-9)
        }
    }

    func testSiblingsWhoseEdgesMeetInLayoutShareTheirDrawnEdge() {
        // Layout's frames are not whole layout units: A's right edge and B's
        // left are one absolute position, whatever their parts round to.
        for scale in [2, 3] as [CGFloat] {
            let a = PixelGrid.place(CGRect(x: 0.084, y: 0, width: 100.084, height: 10), in: .zero, scale: scale)
            let b = PixelGrid.place(CGRect(x: 100.168, y: 0, width: 50, height: 10), in: .zero, scale: scale)
            XCTAssertEqual(a.drawn.maxX, b.drawn.minX, accuracy: 1e-9, "at \(scale)x")
        }
    }

    func testViewsAndFlatLeavesSitOnTheGridAndFollowAParentThatMoves() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "line"], "style": ["background_color": [0, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [255, 0, 0, 255], "written": ["background_color"]]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.3125, "y": 10.1875, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.3125, "w": 300.0, "h": 1.0],
            ["op": "frame", "id": 3, "x": 7.0625, "y": 1.3125, "w": 20.0, "h": 0.5],
        ])
        XCTAssertNotNil(p.flats.leaves[3], "3 is drawn as a flat leaf")
        try assertOnGrid(p, layout: [1: CGRect(x: 0.3125, y: 10.1875, width: 300, height: 40),
                                     2: CGRect(x: 0.3125, y: 10.5, width: 300, height: 1),
                                     3: CGRect(x: 7.375, y: 11.5, width: 20, height: 0.5)])
        // Only the parent moves; its children's edges move with it.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.453125, "y": 10.4375, "w": 300.0, "h": 40.0]]))
        try assertOnGrid(p, layout: [1: CGRect(x: 0.453125, y: 10.4375, width: 300, height: 40),
                                     2: CGRect(x: 0.453125, y: 10.75, width: 300, height: 1),
                                     3: CGRect(x: 7.515625, y: 11.75, width: 20, height: 0.5)])
    }

    func testABatchThatMovesAParentAndItsChildPlacesBothFromLayout() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "a"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "b"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 10.1875, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.3125, "w": 300.0, "h": 1.0],
        ])
        // The child's op comes first: it is placed again once its parent's is in.
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 0.453125, "w": 300.0, "h": 1.0],
                           ["op": "frame", "id": 2, "x": 0.0, "y": 10.4375, "w": 300.0, "h": 40.0]]))
        try assertOnGrid(p, layout: [2: CGRect(x: 0, y: 10.4375, width: 300, height: 40),
                                     3: CGRect(x: 0, y: 10.890625, width: 300, height: 1)])
    }

    func testAScrollContainersEndIsOnTheGrid() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 1000.2],
        ])
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let s = p.gridScale
        XCTAssertEqual(scroll.contentSize.height * s, (1000.2 * s + 0.5).rounded(.down), accuracy: 1e-9)
        // From a scroller placed off a whole pixel, the extent ends where its
        // last edge is drawn: 0.1 + 1000.1 points is 3000.6 device pixels at 3x, 3001 drawn, less the 0 its top is drawn at.
        let o = PixelGrid.place(CGRect(x: 0, y: 0.1, width: 300, height: 300), in: .zero, scale: 3).origin
        XCTAssertEqual(PixelGrid.length(1000.1, from: o, vertical: true, scale: 3) * 3, 3001, accuracy: 1e-9)
    }

    func testABoxKeepsLayoutsSizeForWhatIsPlacedInIt() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "a"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.1, "y": 0.1, "w": 100.1, "h": 20.1],
        ])
        let v = try XCTUnwrap(p.views[2])
        XCTAssertNotEqual(v.bounds.width, 100.1, "drawn on the grid")
        XCTAssertEqual(v.contentBox().width, 100.1, accuracy: 1e-9, "text wraps at layout's width")
        XCTAssertEqual(v.laidOutFrame, CGRect(x: 0.1, y: 0.1, width: 100.1, height: 20.1))
    }

    /// Each box drawn where its absolute layout rectangle's edges round to.
    private func assertOnGrid(_ p: Presenter, layout: [UInt32: CGRect], file: StaticString = #filePath, line: UInt = #line) throws {
        let s = p.gridScale
        func edge(_ v: CGFloat) -> CGFloat { PixelGrid.round(v, scale: s) }
        for (id, rect) in layout {
            let drawn: CGRect
            if let leaf = p.flats.leaves[id], let parent = leaf.parent.flatMap({ p.views[$0] }) {
                drawn = parent.convert(leaf.frame, to: p.viewport)
            } else {
                let v = try XCTUnwrap(p.views[id], file: file, line: line)
                drawn = v.convert(v.bounds, to: p.viewport)
            }
            for (got, want) in [(drawn.minX, rect.minX), (drawn.minY, rect.minY), (drawn.maxX, rect.maxX), (drawn.maxY, rect.maxY)] {
                XCTAssertEqual(got, edge(want), accuracy: 1e-6, "node \(id)", file: file, line: line)
            }
        }
    }

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
}
#endif
