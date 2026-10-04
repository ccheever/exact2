#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A native swipe row (LLP 1008 §9) costs no table and no scroll view at rest:
/// batches leave its content in the owner, clipped, where the authored scroll
/// would hold it at its start; a touch landing on it projects the content into
/// a clear UIKit cell that follows the row's height, and the projection goes
/// again once nothing is swiping. A refused row gets its scroll: then the
/// scroll is the swipe, as on the web. UIKit, so a simulator runs it:
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
        let owner = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[2])
        let body = try XCTUnwrap(p.views[3]), delete = try XCTUnwrap(p.views[4])
        XCTAssertNil(owner.scroll, "no scroll view at rest")
        XCTAssertTrue(owner.scrollDormant); XCTAssertTrue(owner.clipsToBounds, "clipped as the scroll would clip")
        XCTAssertTrue(owner.gestureRecognizers?.isEmpty ?? true)
        XCTAssertNil(p.swipeActions.cell(of: owner), "no cell before a touch")
        XCTAssertFalse(owner.subviews.contains { $0 is UITableView })
        XCTAssertTrue(row.superview === owner)
        apply(p, geometry(80))
        XCTAssertTrue(row.superview === owner, "a batch projects nothing")
        XCTAssertNil(owner.scroll)

        p.swipeActions.touch(owner)
        let cell = try XCTUnwrap(p.swipeActions.cell(of: owner))
        XCTAssertTrue(body.isDescendant(of: cell.contentView))
        XCTAssertTrue(delete.isHidden)
        XCTAssertNil(owner.scroll, "the cell swipes, not a scroll")
        XCTAssertEqual(cell.backgroundConfiguration?.backgroundColor, .clear, "the row paints itself")
        body.layer.displayIfNeeded()
        XCTAssertEqual(body.layer.backgroundColor?.alpha, 1)
        XCTAssertEqual(body.layer.cornerRadius, 24, "and keeps its corners")
        XCTAssertTrue(p.swipeActions.ownsAction(4))

        // No touch reached the table, so nothing can be swiping: it goes.
        turn()
        XCTAssertNil(p.swipeActions.cell(of: owner))
        XCTAssertFalse(owner.subviews.contains { $0 is UITableView })
        XCTAssertTrue(row.superview === owner)
        XCTAssertFalse(row.isHidden); XCTAssertFalse(delete.isHidden)
        XCTAssertEqual(body.convert(CGPoint.zero, to: owner), .zero)
        XCTAssertTrue(p.swipeActions.ownsAction(4), "an unprojected row's controls are still its actions")
    }

    /// The projection restores what the host had said of a control, not
    /// CSS's `display: none` (review B1): shown again, it is visible.
    func testAProjectionRestoresTheHostsWordNotDisplay() throws {
        let p = fixture()
        let owner = try XCTUnwrap(p.views[1]), delete = try XCTUnwrap(p.views[4])
        delete.applyStyle(["display": "none"])
        XCTAssertTrue(delete.isHidden)
        p.swipeActions.touch(owner)
        XCTAssertNotNil(p.swipeActions.cell(of: owner), "projected")
        turn()
        XCTAssertNil(p.swipeActions.cell(of: owner), "released")
        delete.applyStyle([:])
        XCTAssertFalse(delete.isHidden, "displayed again, nothing the host said hides it")
    }

    func testARefusedRowSwipesByItsScroll() throws {
        let p = fixture()
        let owner = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[2])
        // The trailing control loses its name: the native swipe is refused.
        var rename = BatchOp(op: .props, nodeID: 4); rename.clear = ["id"]
        apply(p, [rename])
        let scroll = try XCTUnwrap(owner.scroll, "the web's swipe: the scroll itself")
        XCTAssertTrue(row.superview === scroll)
        XCTAssertEqual(scroll.contentSize.width, 390)
        XCTAssertFalse(p.swipeActions.ownsAction(4))
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
