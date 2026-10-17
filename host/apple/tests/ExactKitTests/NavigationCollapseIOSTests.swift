#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
import ExactGroupedLists

/// LLP 1075.003 Stage 3: a large title collapses with its route's content
/// scroll view (the fixture's Second tab, which names no scroller: LLP
/// 1115.000 D11.1). CSS `scrollTop` counts from the
/// scrollport's top, which under the bar is the bar's bottom whatever its
/// height — UIKit keeps the offset plus that inset fixed as the title
/// collapses — so an authored offset lands where the browser's does (0, 80,
/// the middle, the end of what shows). At rest, CSS 0, the title is
/// expanded; an authored 0 after a collapse lands at 0 under the bar as it
/// is, and the title expands when the user pulls.
/// Gestures (a finger's collapse, pull to top, a held sheet) need real
/// touches, which a unit test has none of.
final class NavigationCollapseIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
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

    /// The fixture booted in a window with its Second tab selected.
    private func second(_ label: String) throws -> (ExactSession, UINavigationController) {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"])))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        _ = tabs.delegate?.tabBarController?(tabs, shouldSelect: tabs.viewControllers![1])
        until("Second selected") { tabs.selectedIndex == 1 }
        return (session, try XCTUnwrap(tabs.selectedViewController as? UINavigationController))
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    /// `scripts/fixtures/navigation-bars.contract` booted in a window, with
    /// the route `name` pushed over its first one.
    private func pushed(_ name: String) throws -> (ExactSession, UINavigationController, RouteController) {
        let session = try booted("EXACT_NAVIGATION_BARS_PLAN", label: "scroller-\(name)")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        _ = Agent(session: session).tap(["id": Int(try node(session, "start-open-\(name)").id)])
        until("\(name) pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        return (session, nav, try XCTUnwrap(nav.topViewController as? RouteController))
    }

    /// A fixture's plan (`scripts/fixtures`, by the name build.mjs gives its
    /// path) booted in a window.
    /// `beforeItsWindow`: the plan boots before its view is in a window, as
    /// an app's does at launch.
    private func booted(_ plan: String, label: String, beforeItsWindow: Bool = false) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[plan], "build.mjs --test --ios compiles the fixtures' plans")))
        ExactGroupedLists.install() // navigation-bars' `rows` route is a grouped list
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        self.window = window
        if beforeItsWindow {
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
            view.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
            view.layoutIfNeeded()
        }
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        if !beforeItsWindow { XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error) }
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return session
    }

    /// LLP 1115.000 D11.1 over `scripts/fixtures/large-title.contract`: one
    /// stack with no tabs, the shape `exact new` makes, whose first screen
    /// names no scroller. Its large title opens expanded, its content below
    /// it, and collapses as the content scrolls. Its scroller is attached
    /// before its view has a window, and UIKit then showed the title inline.
    func testASingleStacksFirstScreenOpensWithItsLargeTitleExpanded() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "large-title", beforeItsWindow: true)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let home = try node(session, "home-scroll"), sv = try XCTUnwrap(home.scroll)
        XCTAssertTrue(route.collapseScroll === home, "the scroller it does not name is the bar's")
        let top = try node(session, "home-top"), bar = nav.navigationBar
        let expanded = bar.frame.height
        XCTAssertGreaterThan(expanded, 90, "the large title is expanded as the screen opens")
        XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5, "its content at its top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY, accuracy: 0.5, "just below the expanded title")
        XCTAssertGreaterThan(home.scrollOrigin, 0, "the expanded inset is CSS 0")
        home.pendingScrollTop = 300
        home.applyPendingScroll()
        spin(0.3)
        XCTAssertLessThan(bar.frame.height, expanded, "and the title collapses as the content scrolls")
    }

    /// LLP 1115.000 D11.1: a route that names no scroller has one all the
    /// same, its first that scrolls vertically, as a hand-built screen's
    /// scroll view is its bar's with nothing written. Right after the header
    /// the bar replaces, it goes under the bar, which insets it, and the
    /// status bar's tap scrolls that same one. (A large title's collapse
    /// with nothing written is the fixture's Second tab, above.)
    func testARoutesScrollerIsTheBarsWithNothingWritten() throws {
        let (session, nav, route) = try pushed("tall")
        let tall = try node(session, "tall-scroll")
        let sv = try XCTUnwrap(tall.scroll)
        XCTAssertNil(route.node.props["navigationScroll"].flatMap { $0.isEmpty ? nil : $0 }, "the route names nothing")
        XCTAssertTrue(route.collapseScroll === tall, "the scroller after the header is the bar's")
        XCTAssertTrue(route.contentScrollView(for: .top) === sv)
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .automatic, "UIKit insets it under the bar")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("collapses its title with its scroller"))
        XCTAssertEqual(tall.convert(tall.bounds, to: nil).minY, 0, accuracy: 0.5, "the scroller itself reaches the screen's top, under the bar")
        let top = try node(session, "tall-top"), bar = nav.navigationBar
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY, accuracy: 0.5, "its first line rests just below the bar")
        XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5, "at its top (CSS 0)")
        tall.pendingScrollTop = 300
        tall.applyPendingScroll()
        spin(0.3)
        XCTAssertLessThan(top.convert(top.bounds, to: nil).maxY, bar.convert(bar.bounds, to: nil).maxY, "scrolled, its content passes under the bar")
        XCTAssertTrue(session.presenter.scrollsToTopTarget() === sv, "the status bar's tap scrolls the same one")
        XCTAssertTrue(sv.scrollsToTop)
    }

    /// D11.1, several scrollers: the bar's is the one the status bar's tap
    /// already picked, the first that scrolls vertically, by one rule for
    /// both; `navigationScroll` is how a route names a different one.
    func testWithSeveralScrollersTheBarsIsTheStatusBarsAndARouteMayNameAnother() throws {
        let (session, _, pair) = try pushed("pair")
        let first = try node(session, "pair-first"), second = try node(session, "pair-second")
        XCTAssertTrue(pair.collapseScroll === first, "the first vertical scroller is the bar's")
        XCTAssertTrue(session.presenter.scrollsToTopTarget() === first.scroll, "and the status bar's")
        XCTAssertTrue(session.presenter.navigation.contentScroller(of: pair) === first)
        XCTAssertEqual(second.scroll?.scrollsToTop, false)
        sessions.removeLast().destroy()
        window?.isHidden = true
        let (named, _, route) = try pushed("named")
        let other = try node(named, "named-second")
        XCTAssertTrue(named.presenter.navigation.contentScroller(of: route) === other, "the one the route names")
        XCTAssertTrue(named.presenter.scrollsToTopTarget() === other.scroll, "the status bar's too")
        XCTAssertEqual(try node(named, "named-first").scroll?.scrollsToTop, false)
        XCTAssertNil(route.collapseScroll, "it is not at the route's top, so the title keeps still, as before")
    }

    /// D11.1, no header: a pushed route whose bar holds the system Back
    /// alone runs its scroller under that bar too.
    func testAHeaderlessRoutesScrollerRunsUnderTheBackOnlyBar() throws {
        let (session, nav, route) = try pushed("bare")
        let bare = try node(session, "bare-scroll")
        let sv = try XCTUnwrap(bare.scroll)
        XCTAssertFalse(nav.isNavigationBarHidden, "the bar with Back alone (D5.2)")
        XCTAssertTrue(route.collapseScroll === bare)
        XCTAssertEqual(bare.convert(bare.bounds, to: nil).minY, 0, accuracy: 0.5, "under the bar")
        let top = try node(session, "bare-top"), bar = nav.navigationBar
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY, accuracy: 0.5, "its first line just below the bar")
        XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5)
    }

    func testAnAuthoredScrollTopLandsAsTheBrowsersWhileTheTitleCollapses() throws {
        let (session, nav) = try second("collapse")
        let node = try self.node(session, "list-second")
        let sv = try XCTUnwrap(node.scroll)
        spin(0.3)
        // LLP 1115.000 D11.1: Second names no scroller, and its large title
        // collapses with the one right after its header all the same.
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertEqual(route.node.props["navigationScroll"] ?? "", "", "the route names nothing")
        XCTAssertTrue(route.collapseScroll === node)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("collapses its title with its scroller"))
        XCTAssertGreaterThan(node.scrollOrigin, 0, "the expanded inset is CSS 0")
        let css = { sv.contentOffset.y + sv.adjustedContentInset.top }
        let expanded = nav.navigationBar.frame.height
        XCTAssertEqual(css(), 0, accuracy: 0.5, "at rest, CSS 0")
        func assign(_ top: Double) {
            node.pendingScrollTop = top
            node.applyPendingScroll()
            spin(0.3)
        }
        assign(80)
        XCTAssertEqual(css(), 80, accuracy: 0.5, "80 lands at 80")
        XCTAssertLessThan(nav.navigationBar.frame.height, expanded, "and the title collapsed")
        assign(300)
        XCTAssertEqual(css(), 300, accuracy: 0.5, "the middle lands")
        assign(0)
        XCTAssertEqual(css(), 0, accuracy: 0.5, "0 lands at 0 (the title expands when the user pulls)")
        assign(100_000)
        let end = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height + sv.adjustedContentInset.top
        XCTAssertEqual(css(), end, accuracy: 0.5, "the end clamps to the scroller's own end")
        // A user's offset reads back in CSS terms, as `layout` reports it.
        sv.setContentOffset(CGPoint(x: 0, y: 30 - sv.adjustedContentInset.top), animated: false)
        spin(0.2)
        let layout = Agent(session: session).layout([:])
        let row = (layout["nodes"] as? [[String: Any]])?.first { ($0["id"] as? Int) == Int(node.id) }
        XCTAssertEqual(row?["sy"] as? Double ?? -1, 30, accuracy: 0.5)
    }

    /// A scroller replaced in place by a taller one, then a shorter one, as
    /// a quiz's results replace its question (bake-off round 3: the results
    /// opened scrolled by the bar, their score and Play Again under it).
    /// UIKit insets a new scroller from the offset it had, 0, which leaves
    /// its content's top under the bar; each opens at its top instead, just
    /// below the bar, and the title collapsed over the page before expands.
    func testAScrollerReplacedInPlaceOpensAtItsTopBelowTheBar() throws {
        let (session, nav) = try second("collapse-replaced")
        let list = try node(session, "list-second")
        spin(0.3)
        let expanded = nav.navigationBar.frame.height
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertLessThan(nav.navigationBar.frame.height, expanded, "the title collapsed over the first page's end")
        for (press, page) in [("second-taller", "taller"), ("second-shorter", "shorter")] {
            _ = Agent(session: session).tap(["id": Int(try node(session, press).id)])
            until("the \(page) scroller in place") { (try? self.node(session, "list-second-\(page)")) != nil }
            spin(0.3)
            let sv = try XCTUnwrap(try node(session, "list-second-\(page)").scroll)
            let top = try node(session, "second-\(page)-top"), bar = nav.navigationBar
            XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5, "the \(page) page at its top (CSS 0)")
            XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY + 12, accuracy: 0.5,
                           "its first line just below the bar, past its 12 of padding")
            XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5, "the title expanded, as a new page's is")
        }
    }
}
#endif
