#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1105: the status bar's style from `status-bar-style`. The declaration
/// painted on top of what covers the bar wins, and a flip reaches the
/// controllers in the batch that made it.
final class StatusBarIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        return p
    }
    /// A route (1) holding a banner (2) whose header (3) sits inside it, and
    /// a later sibling overlay (4) of the route.
    private func screen(_ p: Presenter, route: String?, header: String?, overlay: String? = nil, headerStyle: [String: Any] = [:]) {
        func props(_ s: String?) -> [String: Any] { s.map { ["statusBarStyle": $0] } ?? [:] }
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view"],
            ["op": "create", "id": 1, "kind": "view", "props": props(route)],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": props(header), "style": headerStyle],
            ["op": "create", "id": 4, "kind": "view", "props": props(overlay)],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 10, "ids": [1, 4]], ["op": "roots", "ids": [10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
        ]))
    }

    func testNoDeclarationIsTheDefault() {
        let p = presenter()
        screen(p, route: nil, header: nil)
        XCTAssertEqual(p.statusBar, StatusBarChoice())
    }

    func testTheDeclarationPaintedOnTopWins() {
        var p = presenter()
        screen(p, route: "light-content", header: nil)
        XCTAssertEqual(p.statusBar.style, .lightContent)
        XCTAssertEqual(p.statusBar.source, 1)
        // A descendant over its ancestor's declaration.
        p = presenter()
        screen(p, route: "light-content", header: "dark-content")
        XCTAssertEqual(p.statusBar.style, .darkContent)
        // A later sibling branch over a deeper declaration in an earlier one.
        p = presenter()
        screen(p, route: nil, header: "dark-content", overlay: "light-content")
        XCTAssertEqual(p.statusBar.style, .lightContent, "painted later, though shallower")
        // A hidden or visibility: hidden header does not count.
        for style: [String: Any] in [["display": "none"], ["visibility": "hidden"]] {
            p = presenter()
            screen(p, route: "light-content", header: "dark-content", headerStyle: style)
            XCTAssertEqual(p.statusBar.style, .lightContent, "\(style)")
        }
    }

    func testAutoIsTheSchemeWhereItSits() {
        var p = presenter()
        screen(p, route: nil, header: "auto")
        XCTAssertEqual(p.statusBar.style, .default, "unforced: the system's")
        p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["color_scheme": "dark"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["statusBarStyle": "auto"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 60.0],
        ]))
        XCTAssertEqual(p.statusBar.style, .lightContent, "a dark subtree: light text")
    }

    /// Bluesky's compact header: a scroll past a threshold flips the state,
    /// and the batch that shows the header changes the bar in the same
    /// `apply`, before it returns — no frame of the old style.
    func testAScrollThresholdFlipChangesTheBarInItsOwnBatch() {
        let p = presenter()
        screen(p, route: "light-content", header: nil)
        XCTAssertEqual(p.statusBar.style, .lightContent, "over the dark banner")
        var told: [StatusBarChoice] = []
        p.onStatusBar = { told.append($0) }
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["statusBarStyle": "dark-content", "statusBarAnimation": "fade"]]]))
        XCTAssertEqual(told.map(\.style), [.darkContent], "told once, inside the batch's apply")
        XCTAssertEqual(p.statusBar.style, .darkContent)
        XCTAssertTrue(p.statusBar.fade, "the winner's animation")
        // A batch that changes nothing else does not tell again.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 310.0]]))
        XCTAssertEqual(told.count, 1)
        // Scrolled back: the route's own style returns in that batch.
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["statusBarStyle", "statusBarAnimation"]]]))
        XCTAssertEqual(told.map(\.style), [.darkContent, .lightContent])
    }
}
#endif
