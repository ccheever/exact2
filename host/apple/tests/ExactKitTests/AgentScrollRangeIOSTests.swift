#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// The agent's `layout` reports each scroller's range beside its offset (LLP 1012 `rx`/`ry`): how far the offset
/// travels on each axis between the ends the wheel routes by, whether or not a wheel or a drag may move it that way.
/// The Duo smoke reads it to check that UIKit gives a route's scroller no sideways range (#704), which a list that
/// hides `overflow-x` would not show through the wheel.
final class AgentScrollRangeIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    /// What a person sees of a port past the bars over it: a bar across the
    /// top or the foot shortens it, and one down a whole side (the iPhone
    /// Duo's strip of bar items) narrows it and keeps its height.
    func testBarsShortenThePortAndASideStripNarrowsIt() {
        let port = CGRect(x: 0, y: 0, width: 400, height: 800)
        XCTAssertEqual(Agent.port(port, clearOf: [CGRect(x: 0, y: -20, width: 400, height: 120)]), CGRect(x: 0, y: 100, width: 400, height: 700))
        XCTAssertEqual(Agent.port(port, clearOf: [CGRect(x: 0, y: 700, width: 400, height: 120)]), CGRect(x: 0, y: 0, width: 400, height: 700))
        XCTAssertEqual(Agent.port(port, clearOf: [CGRect(x: 316, y: -10, width: 84, height: 820)]), CGRect(x: 0, y: 0, width: 316, height: 800), "the right side")
        XCTAssertEqual(Agent.port(port, clearOf: [CGRect(x: -10, y: 0, width: 94, height: 800)]), CGRect(x: 84, y: 0, width: 316, height: 800), "the left side")
    }

    private func mounted(_ ops: [[String: Any]]) -> (Agent, Presenter) {
        let session = ExactApp.shared.makeSession(label: "agent-scroll-range")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        p.viewport.layoutIfNeeded()
        return (Agent(session: session), p)
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func node(_ reply: [String: Any], _ id: Int) throws -> [String: Any] {
        try XCTUnwrap((reply["nodes"] as? [[String: Any]])?.first { $0["id"] as? Int == id }, "\(reply)")
    }

    /// A scroller as wide as its content, which no wheel moves sideways (`overflow-x` is not set), still has the
    /// sideways range its side insets give it, 42 points each: `rx` reports it, where the wheel's `sx` cannot.
    func testASideInsetIsARangeTheWheelCannotShow() throws {
        let (agent, p) = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "row"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 600.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 1200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 44.0],
        ])
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        sv.contentInset = UIEdgeInsets(top: 0, left: 42, bottom: 0, right: 42)
        XCTAssertEqual(sv.contentSize.width, sv.bounds.width, "the content fits the scroller's width")
        let i = sv.adjustedContentInset
        XCTAssertGreaterThanOrEqual(i.left + i.right, 84)
        var scroller = try node(agent.layout(), 2)
        let rx = try XCTUnwrap(scroller["rx"] as? Double), ry = try XCTUnwrap(scroller["ry"] as? Double)
        XCTAssertEqual(rx, Double(i.left + i.right), accuracy: 0.01, "the side insets are its sideways range: \(scroller)")
        XCTAssertEqual(ry, Double(sv.contentSize.height + i.top + i.bottom - sv.bounds.height), accuracy: 0.01, "\(scroller)")
        let sx = scroller["sx"] as? Double
        Agent.scroll(from: sv, dx: 400, dy: 0)
        scroller = try node(agent.layout(), 2)
        XCTAssertEqual(scroller["sx"] as? Double, sx, "the wheel takes no sideways step here")
        XCTAssertEqual(scroller["rx"] as? Double, rx, "and the range is as it was")
    }

    /// A scroller UIKit insets, as its bar does at the top, reports its scrollport (`port`) inside those insets: where
    /// its content shows, which the Duo smoke reads as the list's top edge under UIKit's bar.
    func testAnInsetScrollerReportsItsPortInsideTheInsets() throws {
        let (agent, p) = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 600.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 1200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 1200.0],
        ])
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        sv.contentInset = UIEdgeInsets(top: 60, left: 10, bottom: 20, right: 30)
        let i = sv.adjustedContentInset
        XCTAssertGreaterThanOrEqual(i.top, 60)
        let scroller = try node(agent.layout(), 2)
        let port = try XCTUnwrap(scroller["port"] as? [Double], "\(scroller)")
        let box = [scroller["x"], scroller["y"], scroller["w"], scroller["h"]].compactMap { $0 as? Double }
        XCTAssertEqual(port.count, 4)
        XCTAssertEqual(port[0], box[0] + Double(i.left), accuracy: 0.01, "\(scroller)")
        XCTAssertEqual(port[1], box[1] + Double(i.top), accuracy: 0.01, "the port begins below the top inset: \(scroller)")
        XCTAssertEqual(port[2], box[2] - Double(i.left + i.right), accuracy: 0.01, "\(scroller)")
        XCTAssertEqual(port[3], box[3] - Double(i.top + i.bottom), accuracy: 0.01, "and ends above the bottom one: \(scroller)")
        // Scaled to half, as a sheet behind another is, its port is carried through the transform as its box is
        try XCTUnwrap(p.views[2]).transform = CGAffineTransform(scaleX: 0.5, y: 0.5)
        let scaled = try node(agent.layout(), 2)
        let half = try XCTUnwrap(scaled["port"] as? [Double], "\(scaled)")
        let scaledBox = [scaled["x"], scaled["y"], scaled["w"], scaled["h"]].compactMap { $0 as? Double }
        XCTAssertEqual(half[1], scaledBox[1] + Double(i.top) / 2, accuracy: 0.01, "\(scaled)")
        XCTAssertEqual(half[3], (box[3] - Double(i.top + i.bottom)) / 2, accuracy: 0.01, "\(scaled)")
        // Insets taller than the scroller leave an empty port, as `clientHeight` is 0, not a negative one
        try XCTUnwrap(p.views[2]).transform = .identity
        sv.contentInset = UIEdgeInsets(top: 700, left: 10, bottom: 300, right: 30)
        let shut = try node(agent.layout(), 2)
        let empty = try XCTUnwrap(shut["port"] as? [Double], "\(shut)")
        XCTAssertEqual(empty[3], 0, "\(shut)")
        XCTAssertGreaterThanOrEqual(empty[1], box[1], "\(shut)")
        XCTAssertLessThanOrEqual(empty[1], box[1] + box[3], "inside its box: \(shut)")
    }

    /// A scroller a canvas's surface has placed reports its port where it is drawn, as its box is, and none of it once
    /// the placement hides it.
    func testACanvasPlacedScrollersPortIsWhereItIsDrawn() throws {
        let (agent, p) = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
            ["op": "content", "id": 3, "w": 100.0, "h": 400.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 100.0, "h": 400.0],
        ])
        let placed = try XCTUnwrap(p.views[3]), sv = try XCTUnwrap(placed.scroll)
        placed.placement = [1, 0, 50, 0, 1, 50, 0, 0, 1, 0]
        sv.contentInset = UIEdgeInsets(top: 20, left: 0, bottom: 10, right: 0)
        let i = sv.adjustedContentInset
        let scroller = try node(agent.layout(), 3)
        let port = try XCTUnwrap(scroller["port"] as? [Double], "\(scroller)")
        XCTAssertEqual(scroller["x"] as? Double, 50, "drawn 50 points in: \(scroller)")
        XCTAssertEqual(port[0], 50, accuracy: 0.01, "\(scroller)")
        XCTAssertEqual(port[1], 50 + Double(i.top), accuracy: 0.01, "\(scroller)")
        XCTAssertEqual(port[3], 100 - Double(i.top + i.bottom), accuracy: 0.01, "\(scroller)")
        placed.placementHidden = true
        let hidden = try node(agent.layout(), 3)
        XCTAssertEqual(hidden["w"] as? Double, 0, "\(hidden)")
        XCTAssertEqual(hidden["port"] as? [Double], [0, 0, 0, 0], "\(hidden)")
    }

    /// `layout <target>`'s scroll chain carries each scroller's range as `layout` does, the viewport's included.
    func testTheScrollChainCarriesTheRangeAndTheViewports() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a node's explanation is the runner's, of a page booted in a scene's window")
        }
        let session = ExactApp.shared.makeSession(label: "agent-scroll-range-chain")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try fixturePlan("PAGE_EDGES"), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        let named = { (testId: String) in session.presenter.views.values.first { $0.props["testId"] == testId } }
        until("the page is laid out") { named("list")?.scroll != nil }
        let list = try XCTUnwrap(named("list")), top = try XCTUnwrap(named("list-top"))
        let agent = Agent(session: session)
        let flat = try node(agent.layout(), Int(list.id))
        let target = try XCTUnwrap(agent.layout(["id": Int(top.id)])["node"] as? [String: Any])
        let chain = try XCTUnwrap(target["scroll"] as? [[String: Any]], "\(target)")
        let entry = try XCTUnwrap(chain.first { $0["id"] as? Int == Int(list.id) }, "\(chain)")
        XCTAssertEqual(entry["rx"] as? Double, flat["rx"] as? Double, "\(entry) \(flat)")
        XCTAssertEqual(entry["ry"] as? Double, flat["ry"] as? Double, "\(entry) \(flat)")
        XCTAssertGreaterThan(entry["ry"] as? Double ?? 0, 0, "the list scrolls its 1600-point filler: \(entry)")
        XCTAssertEqual(entry["port"] as? [Double], flat["port"] as? [Double], "and its port: \(entry) \(flat)")
        XCTAssertNotNil(entry["port"], "\(entry)")
        let page = try XCTUnwrap(chain.first, "\(chain)")
        XCTAssertEqual(page["viewport"] as? Bool, true, "\(chain)")
        XCTAssertNotNil(page["rx"], "the viewport has its range too: \(page)")
        XCTAssertNotNil(page["ry"], "\(page)")
    }

    /// A scroller whose native scroll view stands in for it (a grouped list's collection view) reports that view's
    /// range, not its own box's.
    func testANativeScrollViewsRangeIsTheNodes() throws {
        let (agent, p) = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "row"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 600.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 600.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 44.0],
        ])
        let owner = try XCTUnwrap(p.views[2])
        let native = UIScrollView(frame: owner.bounds)
        native.contentSize = CGSize(width: 400, height: 1500)
        owner.addSubview(native)
        // Its own padding (16 above, 24 below) is content; the 50 points above it stand for a bar's room
        owner.mountNativeScrollView(native, contentInsets: UIEdgeInsets(top: 16, left: 0, bottom: 24, right: 0))
        native.contentInset = UIEdgeInsets(top: 66, left: 0, bottom: 24, right: 0)
        XCTAssertTrue(owner.scrollView === native)
        let scroller = try node(agent.layout(), 2)
        let i = native.adjustedContentInset
        XCTAssertEqual(scroller["ry"] as? Double, Double(1500 + i.top + i.bottom - native.bounds.height), "\(scroller)")
        XCTAssertEqual(scroller["port"] as? [Double], [0, 50, 400, 550], "the port leaves out the bar's room, not the padding: \(scroller)")
    }

    /// A swipe row's scroll, not yet made, reports no range, and reading it makes none.
    func testADormantScrollHasNoRangeAndStaysDormant() throws {
        let (agent, p) = mounted([
            ["op": "create", "id": 1, "kind": "view", "props": ["swipeContent": "body", "swipeTrailing": "delete"], "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "body"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["id": "delete", "accessibilityLabel": "Delete"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3, 4]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 390.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 4, "x": 300.0, "y": 0.0, "w": 90.0, "h": 80.0],
            ["op": "content", "id": 1, "w": 390.0, "h": 80.0],
        ])
        let owner = try XCTUnwrap(p.views[1])
        XCTAssertTrue(owner.scrollDormant)
        let row = try node(agent.layout(), 1)
        XCTAssertEqual(row["rx"] as? Double, 0, "\(row)")
        XCTAssertEqual(row["ry"] as? Double, 0, "\(row)")
        XCTAssertNil(row["port"], "and no port: \(row)")
        let action = try node(agent.layout(), 4)
        XCTAssertEqual(action["w"] as? Double, 0, "the closed native action has no box")
        XCTAssertNil(action["radii"], "the authored fallback is not the native action's geometry")
        XCTAssertTrue(owner.scrollDormant, "reading the range made no scroll view")
        XCTAssertNil(owner.scroll)
        p.swipeActions.touch(owner)
        XCTAssertNotNil(p.swipeActions.cell(of: owner))
        XCTAssertNil(try node(agent.layout(), 4)["radii"], "projection still owns the action's corners")
    }
}
#endif
