#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A native swipe row (LLP 1008 §9) costs no table at rest: batches leave its
/// content in the authored scroll; a touch landing on it projects the content
/// into a clear UIKit cell that follows the row's height, and the projection
/// goes again once nothing is swiping. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class SwipeActionsIOSTests: XCTestCase {
    private var window: UIWindow!
    private let white: BatchValue = [255, 255, 255, 255]

    private func create(_ id: UInt32, _ kind: String, props: [String: String] = [:], style: NodeStyle = [:]) -> BatchOp {
        var op = BatchOp(op: .create, nodeID: id)
        op.kind = kind; op.props = props; op.style = style
        if kind == "button" { op.handlers = ["press"] }
        return op
    }
    private func frame(_ id: UInt32, _ x: Double, _ w: Double, _ h: Double) -> BatchOp {
        var op = BatchOp(op: .frame, nodeID: id)
        op.x = x; op.w = w; op.h = h
        return op
    }
    private func children(_ id: UInt32, _ ids: [UInt32]) -> BatchOp {
        var op = BatchOp(op: .children, nodeID: id); op.ids = ids
        return op
    }
    /// The row's geometry at `height`: owner 1 scrolls a 390-wide row 2
    /// holding the 300-wide body 3 and the 90-wide Delete 4.
    private func geometry(_ height: Double) -> [BatchOp] {
        var content = BatchOp(op: .content, nodeID: 1)
        content.w = 390; content.h = height
        return [frame(1, 0, 300, height), frame(2, 0, 390, height), frame(3, 0, 300, height), frame(4, 300, 90, height), content]
    }
    private func apply(_ p: Presenter, _ ops: [BatchOp]) {
        p.apply(Batch(ops: ops, timers: false, motion: false, clock: nil, error: nil))
    }
    private func fixture() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var roots = BatchOp(op: .roots); roots.ids = [1]
        apply(p, [
            create(1, "view", props: ["swipeContent": "body", "swipeTrailing": "delete"], style: ["overflow_x": "scroll", "overflow_y": "hidden"]),
            create(2, "view"),
            create(3, "view", props: ["id": "body"], style: ["background_color": white, "border_radius_top_left": 24, "border_radius_top_right": 24]),
            create(4, "button", props: ["id": "delete", "accessibilityLabel": "Delete"]),
            children(1, [2]), children(2, [3, 4]), roots,
        ] + geometry(80))
        return p
    }
    private func turn() { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05)) }

    func testARowAtRestHasNoTableAndATouchProjectsIt() throws {
        let p = fixture()
        let owner = try XCTUnwrap(p.views[1]), body = try XCTUnwrap(p.views[3]), delete = try XCTUnwrap(p.views[4])
        let scroll = try XCTUnwrap(owner.scroll)
        XCTAssertNil(p.swipeActions.cell(of: owner), "no cell before a touch")
        XCTAssertFalse(owner.subviews.contains { $0 is UITableView })
        XCTAssertTrue(body.isDescendant(of: scroll))
        apply(p, geometry(80))
        XCTAssertTrue(body.isDescendant(of: scroll), "a batch projects nothing")

        p.swipeActions.touch(owner)
        let cell = try XCTUnwrap(p.swipeActions.cell(of: owner))
        XCTAssertTrue(body.isDescendant(of: cell.contentView))
        XCTAssertTrue(scroll.isHidden); XCTAssertTrue(delete.isHidden)
        XCTAssertEqual(cell.backgroundConfiguration?.backgroundColor, .clear, "the row paints itself")
        body.layer.displayIfNeeded()
        XCTAssertEqual(body.layer.backgroundColor?.alpha, 1)
        XCTAssertEqual(body.layer.cornerRadius, 24, "and keeps its corners")
        XCTAssertTrue(p.swipeActions.ownsAction(4))

        // No touch reached the table, so nothing can be swiping: it goes.
        turn()
        XCTAssertNil(p.swipeActions.cell(of: owner))
        XCTAssertFalse(owner.subviews.contains { $0 is UITableView })
        XCTAssertTrue(body.isDescendant(of: scroll))
        XCTAssertFalse(scroll.isHidden); XCTAssertFalse(delete.isHidden)
        XCTAssertEqual(body.convert(CGPoint.zero, to: owner), .zero)
        XCTAssertTrue(p.swipeActions.ownsAction(4), "an unprojected row's controls are still its actions")
    }

    func testAProjectedCellFollowsTheRowsHeight() throws {
        let p = fixture()
        let owner = try XCTUnwrap(p.views[1]), body = try XCTUnwrap(p.views[3])
        p.swipeActions.touch(owner)
        XCTAssertEqual(p.swipeActions.cell(of: owner)?.frame.height, 80)
        apply(p, geometry(120))
        let cell = try XCTUnwrap(p.swipeActions.cell(of: owner))
        XCTAssertEqual(cell.frame.height, 120, "the cell grows with the row")
        XCTAssertEqual(body.bounds.height, 120)
        XCTAssertTrue(body.isDescendant(of: cell.contentView))
        apply(p, geometry(64))
        XCTAssertEqual(p.swipeActions.cell(of: owner)?.frame.height, 64, "and shrinks with it")
    }
}
#endif
