#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1116.005 D3 over `scripts/fixtures/tab-edge.contract`: each route
/// keeps its own scroll, as a covered route stays mounted (LLP 1038 §7) and a
/// hand-built UINavigationController keeps the controller it covers. A tab's
/// root scrolled partway, a row's press pushes a screen that opens at its
/// top; UIKit's Back returns to the root where it was left, and the next
/// push is a fresh visit at its top again. A plain `scroll` and a grouped
/// `list` (a UICollectionView) at the root.
final class RouteScrollIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_TAB_EDGE_PLAN"], "build.mjs --test --ios compiles the tab-edge fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "route-scroll")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }
            ?? session?.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    @discardableResult
    private func tap(_ testId: String) throws -> [String: Any] {
        Agent(session: try XCTUnwrap(session)).tap(["id": Int(try node(testId).id)])
    }

    /// The route `name`'s scroller: its `scroll`'s, or the grouped list's collection view.
    private func scroller(_ name: String) throws -> UIScrollView {
        let content = try node("content-\(name)")
        if let list = try XCTUnwrap(session).presenter.groupedLists?.scroller(for: content.id) { return list }
        return try XCTUnwrap(content.scroll, "\(name) scrolls")
    }

    /// The scroller's offset as the page's `scrollTop`: UIKit's, less the inset it rests at.
    private func top(_ sv: UIScrollView) -> CGFloat { sv.contentOffset.y + sv.adjustedContentInset.top }

    /// What a failure reports of a scroller: UIKit's offset, insets and sizes.
    private func seen(_ sv: UIScrollView) -> String {
        "offset \(sv.contentOffset.y), inset \(sv.adjustedContentInset.top)/\(sv.adjustedContentInset.bottom), bounds \(sv.bounds.height), content \(sv.contentSize.height), frame in window \(sv.convert(sv.bounds, to: nil).minY)"
    }

    /// Where the scroller rests, as a person sees it: a plain `scroll`'s
    /// offset as the page's `scrollTop`; for a grouped list, whose cells size
    /// themselves as they come in (UIKit moving the offset with the content
    /// above it so the rows stay put), the cell at its middle and that cell's
    /// top on the screen.
    private func rest(_ sv: UIScrollView) -> String {
        guard let list = sv as? UICollectionView else { return "scrollTop \(top(sv))" }
        list.layoutIfNeeded()
        guard let path = list.indexPathForItem(at: CGPoint(x: list.bounds.midX, y: list.bounds.midY)),
              let cell = list.layoutAttributesForItem(at: path) else { return "no cell at its middle" }
        return "item \(path.item) at \(cell.frame.minY - list.contentOffset.y)"
    }

    /// Scrolled to `y` from the top as a finger leaves it, laid out first: a
    /// grouped list's cells size themselves as they come in, and UIKit pulls
    /// an offset past a content size not yet grown back to it.
    private func scroll(_ sv: UIScrollView, to y: CGFloat) {
        for _ in 0..<6 {
            sv.layoutIfNeeded()
            if abs(top(sv) - y) < 0.5 { break }
            sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y - sv.adjustedContentInset.top), animated: false)
            spin(0.1)
        }
        try? XCTUnwrap(session).presenter.settlePump()
        spin(0.2)
    }

    /// A press on a row of the route `name` that a finger could reach where
    /// its scroller `sv` rests: the grouped list's cell at its middle, else
    /// the first row on screen (the agent refuses a row scrolled off it, and
    /// presses nothing then). Its reply.
    private func pressRow(_ name: String, in sv: UIScrollView) throws -> [String: Any] {
        if let list = sv as? UICollectionView, let path = list.indexPathForItem(at: CGPoint(x: list.bounds.midX, y: list.bounds.midY)) {
            return try tap("\(name)-\(path.item + 1)")
        }
        var reply: [String: Any] = [:]
        for r in 1...30 {
            reply = try tap("\(name)-\(r)")
            if reply["error"] == nil { return reply }
        }
        return reply
    }

    func testAPushedRouteOpensAtItsTopAndBackFindsTheListWhereItWasLeft() throws {
        let session = try fixture()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController, "the tabs are a UITabBarController")
        for (tab, root, pushed) in [("scrolls", "scrolls", "scrollsPushed"), ("grouped", "grouped", "groupedPushed")] {
            try tap("tab-\(tab)")
            spin(0.6)
            let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
            let list = try scroller(root)
            scroll(list, to: 400)
            XCTAssertEqual(top(list), 400, accuracy: 0.5, "\(root) scrolled partway")
            let resting = rest(list)
            let pressed = try pressRow(root, in: list)
            until("\(tab) pushed: \(pressed)") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
            spin(0.3)
            XCTAssertEqual(rest(list), resting, "the press left \(root) where it was")
            let detail = try scroller(pushed)
            XCTAssertEqual(top(detail), 0, accuracy: 0.5, "\(pushed) opens at its top, whatever the list beneath")
            scroll(detail, to: 300)
            // UIKit's own Back: the stack pops, and the list is where it was left.
            nav.popViewController(animated: true)
            until("back to \(root)") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
            spin(0.3)
            XCTAssertEqual(rest(list), resting, "Back finds \(root) where it was left: \(seen(list))")
            // Pushed again, the screen is a fresh visit, at its top.
            let again = try pressRow(root, in: list)
            until("\(tab) pushed again: \(again)") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
            spin(0.3)
            XCTAssertEqual(top(try scroller(pushed)), 0, accuracy: 0.5, "\(pushed) again opens at its top")
            nav.popViewController(animated: true)
            until("back to \(root) again") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
            spin(0.3)
            XCTAssertEqual(rest(list), resting, "and Back again finds \(root) where it was")
        }
    }
}
#endif
