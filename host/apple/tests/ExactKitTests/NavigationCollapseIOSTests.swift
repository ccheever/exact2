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

    private final class InsetWrites: UIScrollView {
        var writes = 0
        override var contentInsetAdjustmentBehavior: UIScrollView.ContentInsetAdjustmentBehavior {
            didSet { writes += 1 }
        }
    }

    func testUnchangedProjectionLeavesTheNativeInsetPolicyAlone() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "inset-policy")
        let navigation = session.presenter.navigation
        let nav = try XCTUnwrap(navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let home = try node(session, "home-scroll")
        let scroll = InsetWrites(frame: try XCTUnwrap(home.scrollView).frame)
        home.addSubview(scroll)
        home.more.nativeScrollView = scroll
        navigation.prepareRoutes([route], in: nav)
        XCTAssertTrue(route.contentScrollView(for: .top) === scroll)
        for transparent in [false, true, false] {
            route.node.props["navigationBar"] = transparent ? "transparent" : nil
            navigation.prepareRoutes([route], in: nav)
            XCTAssertEqual(scroll.contentInsetAdjustmentBehavior, transparent ? .never : .automatic)
            scroll.writes = 0
            navigation.prepareRoutes([route], in: nav)
            navigation.prepareRoutes([route], in: nav)
            XCTAssertEqual(scroll.writes, 0, "unchanged projection leaves UIKit's scroll configuration alone")
        }
    }

    /// `navigationBar="transparent"` over `scripts/fixtures/transparent-bar.contract`:
    /// the bar shows, clear at the top of the route's scroller, whose content
    /// starts at the top edge under it (UIKit insets it nothing), as under a
    /// fixed transparent header on the web; the route is covered nothing
    /// at the top, so the hero picture bleeds to the edge.
    func testATransparentBarInsetsNothingOfItsScroller() throws {
        let session = try booted("EXACT_TRANSPARENT_BAR_PLAN", label: "transparent-bar", beforeItsWindow: true)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertFalse(nav.isNavigationBarHidden, "the bar shows")
        XCTAssertEqual(route.navigationItem.scrollEdgeAppearance, NavigationHost.clearScrollEdge, "clear at the scroller's top")
        let home = try node(session, "home-scroll"), sv = try XCTUnwrap(home.scroll)
        XCTAssertTrue(route.collapseScroll === home, "the bar follows the route's scroller")
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(sv.adjustedContentInset.top, 0, "UIKit insets it nothing")
        let hero = try node(session, "hero")
        XCTAssertEqual(hero.convert(hero.bounds, to: nil).minY, 0, accuracy: 0.5, "the picture starts at the top edge, under the bar")
        XCTAssertEqual(home.scrollOrigin, 0, "CSS scrollTop 0 is the top edge")
    }

    /// LLP 1115.000 D11.1 over `scripts/fixtures/large-title.contract`: one
    /// stack with no tabs, the shape `exact new` makes, whose first screen
    /// names no scroller. Its large title opens expanded, its content below
    /// it, and collapses as the content scrolls. UIKit shows the title
    /// collapsed when the stack is laid out in the window before it has its
    /// root (`installPrimary`).
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

    /// The agent's `tap <scroller> drag 0 -900` to the end under an expanded
    /// large title rests at the end, as a finger's drag does once its bounce
    /// settles: UIKit adds the collapsing title's height to the offset, which
    /// left the content that far past its end (euxy's Settings, 52 points).
    func testAnAgentDragToTheEndUnderALargeTitleRestsAtTheEnd() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "large-title-drag", beforeItsWindow: true)
        let home = try node(session, "home-scroll"), sv = try XCTUnwrap(home.scroll)
        let bar = try XCTUnwrap(session.presenter.navigation.primaryNavigation).navigationBar
        let expanded = bar.frame.height
        let reply = Agent(session: session).tap(["id": Int(home.id), "scrollDrag": [0.0, -100_000.0]])
        XCTAssertNil(reply["error"], "\(reply)")
        spin(0.3)
        XCTAssertLessThan(bar.frame.height, expanded, "the title collapsed")
        let end = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height
        XCTAssertEqual(sv.contentOffset.y, end, accuracy: 0.5, "at the end, not past it")
    }

    /// The agent's wheel under a large title leaves the scroller and the bar
    /// where a person's scroll by as much leaves a hand-built list's (measured
    /// on iOS 27): the content moved by the wheel's distance exactly while the
    /// title collapses, a wheel back to the top opening the title again, and
    /// none resting with the content partway under the title, which eases to
    /// the nearer of its open and closed rests.
    func testTheWheelUnderALargeTitleRestsAsAPersonsScrollDoes() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "large-title-wheel", beforeItsWindow: true)
        let home = try node(session, "home-scroll"), sv = try XCTUnwrap(home.scroll)
        let top = try node(session, "home-top"), bar = try XCTUnwrap(session.presenter.navigation.primaryNavigation).navigationBar
        let expanded = bar.frame.height, open = sv.contentOffset.y
        let onScreen = { top.convert(top.bounds, to: nil).minY }, opened = onScreen()
        Agent.scroll(from: sv, dx: 0, dy: 300)
        XCTAssertLessThan(bar.frame.height, expanded, "the title collapsed")
        XCTAssertEqual(onScreen(), opened - 300, accuracy: 0.5, "the content moved by the wheel's 300, not the title's height besides")
        let closed = -sv.adjustedContentInset.top, band = closed - open
        XCTAssertGreaterThan(band, 40, "the title's height")
        Agent.scroll(from: sv, dx: 0, dy: -1_000)
        XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5, "the title opened at the top")
        XCTAssertEqual(sv.contentOffset.y, open, accuracy: 0.5, "at the top, not past it")
        XCTAssertEqual(onScreen(), opened, accuracy: 0.5, "the content just below the title")
        // A third of the title down eases back open; two thirds, closed
        Agent.scroll(from: sv, dx: 0, dy: (band / 3).rounded())
        XCTAssertEqual(sv.contentOffset.y, open, accuracy: 0.5, "a third of the way in eases back open")
        XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5)
        Agent.scroll(from: sv, dx: 0, dy: (band * 2 / 3).rounded())
        XCTAssertEqual(sv.contentOffset.y, closed, accuracy: 0.5, "two thirds of the way in eases closed")
        XCTAssertLessThan(bar.frame.height, expanded)
        // From the closed title's top, the same up
        Agent.scroll(from: sv, dx: 0, dy: -(band / 3).rounded())
        XCTAssertEqual(sv.contentOffset.y, closed, accuracy: 0.5, "a third of a pull eases back closed")
        Agent.scroll(from: sv, dx: 0, dy: -(band * 2 / 3).rounded())
        XCTAssertEqual(sv.contentOffset.y, open, accuracy: 0.5, "two thirds of one opens it")
        XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5)
    }

    /// The agent's drag past the top under a collapsed large title opens it,
    /// as a finger's pull does, and rests at the top once the bounce settles.
    func testAnAgentDragPastTheTopOpensACollapsedLargeTitle() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "large-title-pull", beforeItsWindow: true)
        let home = try node(session, "home-scroll"), sv = try XCTUnwrap(home.scroll)
        let bar = try XCTUnwrap(session.presenter.navigation.primaryNavigation).navigationBar
        let expanded = bar.frame.height, start = sv.contentOffset.y
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(home.id), "scrollDrag": [0.0, -300.0]])["error"])
        XCTAssertLessThan(bar.frame.height, expanded, "the title collapsed")
        let pull = agent.tap(["id": Int(home.id), "scrollDrag": [0.0, 1_000.0]])
        XCTAssertNil(pull["error"], "\(pull)")
        XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5, "the pull opened the title")
        XCTAssertEqual(sv.contentOffset.y, start, accuracy: 0.5, "at the top")
        XCTAssertEqual(try XCTUnwrap(pull["scrolledBy"] as? [Double])[1], -300, accuracy: 0.5, "the content moved back by the 300 it went")
    }

    /// LLP 1054 A4: the agent's drag pulls to refresh as a finger does. A
    /// real touch's 200 points past the top refreshed, 180 did not (iOS 27).
    func testTheAgentsDragPullsToRefreshAsAFingerDoes() throws {
        let session = try booted("EXACT_PULL_REFRESH_PLAN", label: "pull-refresh")
        let agent = Agent(session: session)
        let feed = try node(session, "feed"), control = try XCTUnwrap(feed.scroll?.refreshControl)
        let short = agent.tap(["id": Int(feed.id), "scrollDrag": [0.0, 150.0]])
        XCTAssertNil(short["error"], "\(short)")
        XCTAssertEqual(short["refreshed"] as? Bool, false, "short of the pull")
        XCTAssertFalse(control.isRefreshing)
        let pull = agent.tap(["id": Int(feed.id), "scrollDrag": [0.0, 300.0]])
        XCTAssertNil(pull["error"], "\(pull)")
        XCTAssertEqual(pull["refreshed"] as? Bool, true)
        XCTAssertTrue(control.isRefreshing, "the spinner shows until the app's `refreshing` says otherwise")
        // Target-action goes through UIApplication, which a bare test run has
        // none of: the control's own actions, sent as UIKit would.
        for target in control.allTargets {
            for action in control.actions(forTarget: target, forControlEvent: .valueChanged) ?? [] {
                _ = (target.base as AnyObject).perform(NSSelectorFromString(action), with: control)
            }
        }
        until("the refresh ran the app's action") { (try? self.node(session, "pulls"))?.props["text"] == "1" }
    }

    /// A diagonal agent drag moves a scroller along the axes it scrolls
    /// alone, though its content overflows the other (the page is 800 wide,
    /// the strip's row 100 tall), and reports the drag's own distance while
    /// the collapsing title adds its height to the offset.
    func testADiagonalAgentDragMovesAScrollerAlongItsOwnAxisAlone() throws {
        let session = try booted("EXACT_LARGE_TITLE_PLAN", label: "large-title-diagonal", beforeItsWindow: true)
        let agent = Agent(session: session)
        let page = try node(session, "home-scroll"), sv = try XCTUnwrap(page.scroll)
        let strip = try node(session, "home-strip"), row = try XCTUnwrap(strip.scroll)
        XCTAssertEqual(page.scroll?.scrollsX, false)
        XCTAssertEqual(strip.scroll?.scrollsY, false)
        let across = agent.tap(["id": Int(strip.id), "scrollDrag": [-240.0, -30.0]])
        XCTAssertNil(across["error"], "\(across)")
        XCTAssertEqual(row.contentOffset.x, 240, accuracy: 0.5, "the strip scrolls sideways")
        XCTAssertEqual(row.contentOffset.y, 0, accuracy: 0.5, "and not down")
        let x = sv.contentOffset.x, y = sv.contentOffset.y
        let down = agent.tap(["id": Int(page.id), "scrollDrag": [-40.0, -300.0]])
        XCTAssertNil(down["error"], "\(down)")
        XCTAssertEqual(sv.contentOffset.x, x, accuracy: 0.5, "the page scrolls down and not sideways")
        XCTAssertGreaterThan(sv.contentOffset.y, y + 299)
        let by = try XCTUnwrap(down["scrolledBy"] as? [Double])
        XCTAssertEqual(by[0], 0, accuracy: 0.5)
        XCTAssertEqual(by[1], 300, accuracy: 0.5, "the drag's 300, not the title's height besides")
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

    /// Under a `viewport-fit="cover"` root a route's scroller spans the
    /// window, side insets included. UIKit insets it at its top and bottom
    /// only, so its content, as wide as it, has no sideways range (LLP 1123
    /// D3); `.always` added the side inset too, and it scrolled sideways by it.
    func testUnderACoverRootTheBarsScrollerHasNoSidewaysRange() throws {
        let session = try booted("EXACT_COVER_SCROLL_PLAN", label: "cover-scroll")
        let sv = try XCTUnwrap(try node(session, "home-scroll").scroll)
        let host = try XCTUnwrap(window?.rootViewController), before = sv.safeAreaInsets.right
        host.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 84)
        host.view.layoutIfNeeded()
        until("the scroller lies under the side inset") { sv.safeAreaInsets.right >= before + 83.5 }
        XCTAssertEqual(sv.adjustedContentInset.left, 0, accuracy: 0.5)
        XCTAssertEqual(sv.adjustedContentInset.right, 0, accuracy: 0.5, "UIKit adds no side inset")
        XCTAssertLessThanOrEqual(sv.contentSize.width + sv.adjustedContentInset.left + sv.adjustedContentInset.right, sv.bounds.width + 0.5, "no sideways range")
        XCTAssertGreaterThan(sv.adjustedContentInset.top, 0, "inset under the bar")
    }

    func testAPoppingRouteKeepsItsSideClearanceUntilItLeavesTheWindow() throws {
        for left in [false, true] {
            let (session, nav, route) = try pushed("tall")
            let navigation = session.presenter.navigation
            let host = try XCTUnwrap(window?.rootViewController)
            host.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: left ? 84 : 0, bottom: 0, right: left ? 0 : 84)
            host.view.layoutIfNeeded()
            func side() -> CGFloat? {
                guard case .edges(let edges)? = navigation.covers[route.node.id] else { return nil }
                return left ? edges.left : edges.right
            }
            until("the route clears the side safe area") { (side() ?? 0) >= 83.5 }
            let clearance = try XCTUnwrap(side())
            let content = try node(session, "tall-scroll")
            let contentWidth = content.bounds.width
            XCTAssertTrue(nav.popViewController(animated: true) === route)
            let transition = try XCTUnwrap(nav.transitionCoordinator)
            XCTAssertTrue(transition.viewController(forKey: .from) === route)
            XCTAssertFalse(nav.viewControllers.contains(route), "UIKit removes the source before its visible transition ends")
            XCTAssertNotNil(route.view.window)
            navigation.reportCovers()
            XCTAssertEqual(side() ?? 0, clearance, accuracy: 0.5, "the departing screen retains its side clearance")
            XCTAssertEqual(content.bounds.width, contentWidth, accuracy: 0.5, "its content does not reflow as Back begins")
            until("the pop ends") { nav.transitionCoordinator == nil && route.view.window == nil }
            navigation.reportCovers()
            XCTAssertNil(navigation.covers[route.node.id], "the departed screen releases its clearance")
            session.destroy()
            sessions.removeLast()
            window?.isHidden = true
        }
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
    /// below the bar. The bar stays as the page before left it, its title
    /// collapsed, as UIKit keeps the bar of a hand-built screen whose scroll
    /// view is replaced in place (measured on iOS 27); a pull opens it.
    func testAScrollerReplacedInPlaceOpensAtItsTopBelowTheBarAsItWasLeft() throws {
        let (session, nav) = try second("collapse-replaced")
        let list = try node(session, "list-second")
        spin(0.3)
        let expanded = nav.navigationBar.frame.height
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        spin(0.3)
        let collapsed = nav.navigationBar.frame.height
        XCTAssertLessThan(collapsed, expanded, "the title collapsed over the first page's end")
        for (press, page) in [("second-taller", "taller"), ("second-shorter", "shorter")] {
            _ = Agent(session: session).tap(["id": Int(try node(session, press).id)])
            until("the \(page) scroller in place") { (try? self.node(session, "list-second-\(page)")) != nil }
            spin(0.3)
            let sv = try XCTUnwrap(try node(session, "list-second-\(page)").scroll)
            let top = try node(session, "second-\(page)-top"), bar = nav.navigationBar
            XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5, "the \(page) page at its top (CSS 0)")
            XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY + 12, accuracy: 0.5,
                           "its first line just below the bar, past its 12 of padding")
            XCTAssertEqual(bar.frame.height, collapsed, accuracy: 0.5, "the title still collapsed, as UIKit leaves it")
        }
    }
}
#endif
