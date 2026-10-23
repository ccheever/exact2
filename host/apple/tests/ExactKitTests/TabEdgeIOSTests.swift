#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1116.003 over `scripts/fixtures/tab-edge.contract`: a tab's scroller,
/// scrolled to its end, brings its last row above the floating tab bar, as a
/// hand-built table view in a tab bar controller does (its adjusted content
/// inset holds the bar), and its content still runs under the glass to the
/// screen's bottom. A plain `scroll`, a plain `list` and a grouped `list`,
/// each at a tab's root and pushed inside a tab.
final class TabEdgeIOSTests: XCTestCase {
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

    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_TAB_EDGE_PLAN"], "build.mjs --test --ios compiles the tab-edge fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "tab-edge")
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
        try waitForRoute("scrolls")
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }
            ?? session?.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func waitForRoute(_ name: String) throws {
        let session = try XCTUnwrap(session), agent = Agent(session: session)
        let deadline = Date().addingTimeInterval(5)
        func ready() -> Bool {
            let top = session.presenter.navigation.primaryNavigation?.topViewController as? RouteController
            return top?.node.props["testId"] == "route-\(name)" && !agent.nativeInFlight()
        }
        while !ready() && Date() < deadline {
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        XCTAssertTrue(ready(), "route \(name) is shown and settled")
    }

    private func tap(_ testId: String, route: String) throws {
        let reply = Agent(session: try XCTUnwrap(session)).tap(["id": Int(try node(testId).id)])
        XCTAssertNil(reply["error"], "\(reply)")
        try waitForRoute(route)
    }

    private func box(_ v: UIView) -> CGRect { v.convert(v.bounds, to: nil) }

    /// Scrolled to its end as a finger's scroll ends there: the offset UIKit
    /// rests at, its content size and insets as they are once laid out (a
    /// grouped list's cells size themselves as they come in).
    private func scrollToEnd(_ sv: UIScrollView) {
        for _ in 0..<6 {
            sv.layoutIfNeeded()
            let i = sv.adjustedContentInset
            let end = max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height)
            if abs(sv.contentOffset.y - end) < 0.5 { break }
            sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: end), animated: false)
            spin(0.1)
        }
        spin(0.2)
    }

    /// The route `name`'s scroller and its last row's box in the window,
    /// scrolled to its end.
    private func end(of name: String) throws -> (scroll: UIScrollView, last: CGRect) {
        let p = try XCTUnwrap(session).presenter
        let content = try node("content-\(name)")
        if let list = p.groupedLists?.scroller(for: content.id) as? UICollectionView {
            scrollToEnd(list)
            let section = list.numberOfSections - 1, item = list.numberOfItems(inSection: section) - 1
            let row = try XCTUnwrap(list.layoutAttributesForItem(at: IndexPath(item: item, section: section)), "\(name)'s last row")
            return (list, list.convert(row.frame, to: nil))
        }
        let sv = try XCTUnwrap(content.scroll, "\(name) scrolls")
        scrollToEnd(sv)
        p.settlePump()
        sv.layoutIfNeeded()
        return (sv, box(try node("\(name)-30")))
    }

    private func assertClear(_ name: String, _ tabs: UITabBarController, file: StaticString = #filePath, line: UInt = #line) throws {
        let (sv, last) = try end(of: name)
        let bar = box(tabs.tabBar)
        XCTAssertFalse(tabs.tabBar.isHidden, file: file, line: line)
        XCTAssertLessThanOrEqual(last.maxY, bar.minY + 0.5, "\(name)'s last row ends above the tab bar's top \(bar.minY): \(last), inset \(sv.adjustedContentInset.bottom)", file: file, line: line)
        XCTAssertGreaterThan(last.maxY, bar.minY - 60, "\(name)'s last row rests near the bar, not short of it: \(last)", file: file, line: line)
        XCTAssertGreaterThanOrEqual(box(sv).maxY, bar.maxY - 0.5, "\(name)'s content runs under the glass to the screen's bottom: \(box(sv))", file: file, line: line)
    }

    func testATabsLastRowScrollsClearOfTheTabBar() throws {
        let session = try fixture()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController, "the tabs are a UITabBarController")
        for (tab, root, pushed) in [("scrolls", "scrolls", "scrollsPushed"), ("plain", "plain", "plainPushed"), ("grouped", "grouped", "groupedPushed")] {
            try tap("tab-\(tab)", route: root)
            try assertClear(root, tabs)
            try tap("\(root)-30", route: pushed)
            let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
            XCTAssertEqual(nav.viewControllers.count, 2, "\(tab) pushed")
            try assertClear(pushed, tabs)
        }
    }

    func testRouteWaitIncludesDelayedPressDelivery() throws {
        let session = try fixture(), presenter = session.presenter
        let press = presenter.onPress
        let delivered = expectation(description: "delayed press delivered")
        presenter.onPress = { id in
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.75) {
                press?(id)
                delivered.fulfill()
            }
        }
        defer { presenter.onPress = press }
        try tap("tab-grouped", route: "grouped")
        XCTAssertEqual((presenter.navigation.primaryNavigation?.topViewController as? RouteController)?.node.props["testId"], "route-grouped")
        XCTAssertFalse(Agent(session: session).nativeInFlight())
        wait(for: [delivered], timeout: 5)
    }

    func testOneAgentWheelReachesTheMeasuredEndOfAGroupedList() throws {
        let session = try fixture(), agent = Agent(session: session)
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        for name in ["grouped", "plainPushed"] {
            if name == "grouped" { try tap("tab-grouped", route: "grouped") }
            else { try tap("tab-plain", route: "plain"); try tap("plain-1", route: "plainPushed") }
            let content = try node("content-\(name)")
            let sv = try XCTUnwrap(session.presenter.groupedLists?.scroller(for: content.id) as? UICollectionView)
            let reply = agent.tap(["id": Int(content.id), "wheel": [0.0, 20000.0]])
            XCTAssertNil(reply["error"], "\(reply)")
            session.presenter.settlePump()
            sv.layoutIfNeeded()
            let inset = sv.adjustedContentInset
            let end = max(-inset.top, sv.contentSize.height + inset.bottom - sv.bounds.height)
            XCTAssertEqual(sv.contentOffset.y, end, accuracy: 0.5, "one wheel reaches the measured end")
            let last = IndexPath(item: sv.numberOfItems(inSection: sv.numberOfSections - 1) - 1, section: sv.numberOfSections - 1)
            let frame = try XCTUnwrap(sv.layoutAttributesForItem(at: last)).frame
            XCTAssertNotNil(sv.cellForItem(at: last), "the final row is realized")
            XCTAssertLessThanOrEqual(sv.convert(frame, to: nil).maxY, box(tabs.tabBar).minY + 0.5, "the final row clears the tab bar")
        }
    }

    /// The `scrolls` route holds a menu between its header and its scroller
    /// and a `dialog` after it. Closed, neither is laid out, and the scroller
    /// keeps both edges. A menu given back to layout to show in Exact's views
    /// (a context preview, the agent's painted popover) is laid out out of
    /// the flow, as HTML's top layer is, so the scroller keeps its place and
    /// its edges while it shows, and after it closes.
    func testAClosedMenuAndDialogBesideTheScrollerLeaveItItsEdges() throws {
        let session = try fixture()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        try assertClear("scrolls", tabs)
        let content = try node("content-scrolls")
        let route = try XCTUnwrap(sequence(first: content.next, next: { $0?.next }).compactMap { $0 as? RouteController }.first)
        XCTAssertTrue(route.edgeScroll === content, "the scroller reaches the bottom edge")
        XCTAssertTrue(route.collapseScroll === content, "and goes under the bar")
        let menu = try node("sort-menu")
        XCTAssertFalse(NavigationHost.inFlow(menu), "a closed menu is not laid out")
        XCTAssertFalse(NavigationHost.inFlow(try node("confirm")), "nor is a closed dialog")
        let before = box(content)
        session.presenter.menus.layOut(menu)
        spin(0.2)
        XCTAssertGreaterThan(menu.bounds.height, 0, "the menu is laid out")
        // Out of the flow, as HTML's top layer is: opening it moves nothing.
        XCTAssertFalse(NavigationHost.inFlow(menu), "an open popover is out of its parent's flow")
        XCTAssertEqual(box(content), before, "so the scroller beside it stays where it was")
        session.presenter.menus.coverClosed()
        spin(0.3)
        XCTAssertFalse(NavigationHost.inFlow(menu))
        XCTAssertEqual(box(content), before)
        XCTAssertTrue(route.edgeScroll === content)
    }
}
#endif
