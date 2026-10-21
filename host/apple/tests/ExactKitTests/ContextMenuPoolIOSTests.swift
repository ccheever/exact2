#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A virtualized list's rows with context menus park and are lent to the
/// next rows (NodePoolIOS), their interaction and long press following the
/// new node; a row whose menu is up is destroyed as before.
final class ContextMenuPoolIOSTests: XCTestCase {
    private var window: UIWindow!
    private func collections(_ rows: [Int]) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": rows.map { ["view": $0, "root": $0, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// A row (`base`), a button with `press` and `contextmenu`, naming popover
    /// `m` when `menu`, holding a text (`base + 1`).
    private func row(_ base: Int, menu: Bool = true, handlers: [String] = ["press", "contextmenu"]) -> [[String: Any]] {
        [["op": "create", "id": base, "kind": "button", "handlers": handlers,
          "props": menu ? ["testId": "row-\(base)", "contextPopover": "m"] : ["testId": "row-\(base)"]],
         ["op": "create", "id": base + 1, "kind": "text", "props": ["text": "Chat \(base)"]],
         ["op": "children", "id": base, "ids": [base + 1]],
         ["op": "frame", "id": base, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
         ["op": "frame", "id": base + 1, "x": 8.0, "y": 8.0, "w": 200.0, "h": 20.0]]
    }
    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections([10]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Pin"]],
            ["op": "create", "id": 9, "kind": "view"]]
            + row(10)
            + [["op": "children", "id": 1, "ids": [10]], ["op": "children", "id": 2, "ids": [3]],
               ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0],
               ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 220.0, "h": 60.0],
               ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 220.0, "h": 44.0]]))
        return p
    }
    private func menus(_ v: UIView) -> [UIContextMenuInteraction] { v.interactions.compactMap { $0 as? UIContextMenuInteraction } }

    func testARowWithAContextMenuIsLentToTheNextRow() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let view = try XCTUnwrap(p.views[10])
        XCTAssertEqual(menus(view).count, 1)
        XCTAssertEqual(view.contextRecognizer?.isEnabled, false)
        let kept = try XCTUnwrap(menus(view).first)
        // Retired, and the next row of its shape takes its views.
        p.apply(wireBatch([collections([20]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11]] + row(20)
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[20] === view, "the same view, lent to the next row")
        XCTAssertEqual(p.pool.takes, 1)
        XCTAssertEqual(menus(view).count, 1, "one interaction, the new node's")
        XCTAssertTrue(menus(view).first === kept, "the same one, kept through the park: UIKit's eight recognizers are not made again")
        XCTAssertEqual(view.contextRecognizer?.isEnabled, false)
        let interaction = try XCTUnwrap(menus(view).first)
        let configuration = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero), "and it opens the new node's menu")
        XCTAssertTrue(p.menus.context.open?.source === view)
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        // Lent to a row that names no popover: no interaction, its long press on.
        p.apply(wireBatch([collections([30]), ["op": "destroy", "id": 20], ["op": "destroy", "id": 21]] + row(30, menu: false)
            + [["op": "children", "id": 1, "ids": [30]]]))
        XCTAssertTrue(p.views[30] === view)
        XCTAssertEqual(menus(view).count, 0)
        XCTAssertEqual(view.contextRecognizer?.isEnabled, true)
        XCTAssertTrue(view.interactions.isEmpty, "nothing UIKit added with the menu is left behind")
        // Lent again to a row with neither a popover nor a `contextmenu`: no long press either.
        p.apply(wireBatch([collections([40]), ["op": "destroy", "id": 30], ["op": "destroy", "id": 31]] + row(40, menu: false, handlers: ["press"])
            + [["op": "children", "id": 1, "ids": [40]]]))
        XCTAssertTrue(p.views[40] === view)
        XCTAssertNil(view.contextRecognizer)
        XCTAssertTrue(menus(view).isEmpty)
    }

    /// Only the menu's own are admitted: a recognizer anything else added
    /// keeps the row out of the pool, as before.
    func testAForeignRecognizerStillKeepsTheRowOut() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let view = try XCTUnwrap(p.views[10])
        view.addGestureRecognizer(UITapGestureRecognizer())
        p.apply(wireBatch([collections([20]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11]] + row(20)
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertFalse(p.views[20] === view, "destroyed as before, not lent")
        XCTAssertEqual(p.pool.takes, 0)
    }

    func testAParkedRowThatLeavesThePoolTakesItsInteractionWithIt() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let view = try XCTUnwrap(p.views[10])
        // Retired with nothing to take it: parked, its interaction kept.
        p.apply(wireBatch([collections([]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11],
                           ["op": "children", "id": 1, "ids": []]]))
        XCTAssertTrue(p.pool.isParked(view))
        XCTAssertEqual(menus(view).count, 1)
        p.pool.reset() // memory pressure's path
        XCTAssertTrue(view.interactions.isEmpty, "gone with the view, UIKit's beside it too")
        // The presenter's reset (a reload, the session's end): the host first.
        let q = presenter()
        let other = try XCTUnwrap(q.views[10])
        q.apply(wireBatch([collections([]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11],
                           ["op": "children", "id": 1, "ids": []]]))
        XCTAssertTrue(q.pool.isParked(other))
        q.reset()
        XCTAssertTrue(other.interactions.isEmpty, "nothing of the menu's left on a parked view")
    }

    /// A focus a menu set aside is not given to the node a parked view is
    /// lent to: row 10 holds the focus, row 20's menu sets it aside, row 10
    /// is retired and lent to row 30, and row 20's menu ends.
    func testASetAsideFocusIsNotGivenToTheNodeItsViewIsLentTo() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        p.apply(wireBatch([collections([10, 20])] + row(20) + [["op": "children", "id": 1, "ids": [10, 20]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 60.0, "w": 300.0, "h": 60.0]]))
        let first = try XCTUnwrap(p.views[10]), second = try XCTUnwrap(p.views[20])
        XCTAssertTrue(first.becomeFirstResponder())
        let interaction = try XCTUnwrap(menus(second).first)
        let configuration = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        p.menus.context.contextMenuInteraction(interaction, willDisplayMenuFor: configuration, animator: nil)
        XCTAssertFalse(first.isFirstResponder)
        p.apply(wireBatch([collections([30, 20]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11]] + row(30)
            + [["op": "children", "id": 1, "ids": [30, 20]]]))
        XCTAssertTrue(p.views[30] === first, "lent to row 30")
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertFalse(first.isFirstResponder, "row 30 never had the focus")
    }

    func testARowWhoseMenuIsUpIsDestroyedAndItsMenuEnds() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let view = try XCTUnwrap(p.views[10])
        let interaction = try XCTUnwrap(menus(view).first)
        let configuration = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        _ = p.menus.context.menu(try XCTUnwrap(p.menus.context.open))
        XCTAssertNotNil(p.menus.observation(), "showing")
        p.apply(wireBatch([collections([20]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11]] + row(20)
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertFalse(p.views[20] === view, "not parked while its menu is up")
        XCTAssertNil(p.menus.context.open, "and the menu ended with its source")
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
    }
}
#endif
