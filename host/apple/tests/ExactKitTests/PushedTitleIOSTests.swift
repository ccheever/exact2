#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
import ExactGroupedLists

/// LLP 1115.000 D11.1 over `scripts/fixtures/pushed-title.contract`: a pushed
/// screen whose title is large arrives with it expanded, as UIKit shows a
/// pushed controller whose item says `.always` in a bar that prefers large
/// titles. It collapses as its scroller scrolls and expands again at the top,
/// exactly as a stack's first screen does; a level-2 heading stays inline;
/// and the screen underneath is as it was left.
///
/// UIKit opens a pushed large title only when the scroll view it follows is
/// at its controller's top, under the bar, as the transition's first layout
/// runs (measured on iOS 27 in UIKit alone: one 10 points lower arrives
/// collapsed over content at its top, and moving it up a turn later does not
/// open it). Exact's scroller was still below its header's height then: the
/// header the bar stands for kept its room in the layout until after the push
/// had begun.
///
/// A push inside an open sheet, a finger's pull and a finger's swipe back
/// need a presentation this host's window never finishes and real touches;
/// they are driven in the app.
final class PushedTitleIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The fixture booted in a window: its tab controller and Home's stack.
    private func fixture() throws -> (ExactSession, UITabBarController, UINavigationController) {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_PUSHED_TITLE_PLAN"], "build.mjs --test --ios compiles the pushed-title fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "pushed-title")
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
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        until("Home shown") { rests(nav, 1) }
        settle()
        return (session, tabs, nav)
    }

    private func find(_ testId: String) -> NodeView? {
        session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }
    }
    private func node(_ testId: String) throws -> NodeView { try XCTUnwrap(find(testId), "no \(testId) on screen") }

    /// A stack at rest with `depth` routes: no transition of UIKit's or
    /// Exact's in flight, its top route in the window.
    private func rests(_ nav: UINavigationController, _ depth: Int) -> Bool {
        nav.viewControllers.count == depth && nav.transitionCoordinator == nil && session?.presenter.navigation.transitioning == false
            && nav.topViewController?.viewIfLoaded?.window != nil
    }

    /// An authored button pressed (directly: it may have scrolled out of sight).
    private func press(_ testId: String) throws {
        let node = try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
        session?.presenter.press(node.id)
    }

    /// A button pressed, and the stack at rest with `depth` routes.
    private func tap(_ testId: String, in nav: UINavigationController, depth: Int) throws {
        try press(testId)
        until("\(testId): the stack at rest, \(depth) deep") { rests(nav, depth) }
        settle()
    }

    /// One turn of the main queue after a transition: what `didShow` left
    /// for it (the tablist, the insets sampled at rest) has run.
    private func settle() {
        let turned = expectation(description: "a turn of the main queue")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 5)
        window?.layoutIfNeeded()
    }

    /// Whether UIKit shows `title` as a large title: a label holding it at a
    /// large-title size, opaque, and at least half of it inside the window
    /// and every view that clips it. UIKit keeps a pushed route's label in
    /// the scroll view the bar follows, which clips it when it is left where
    /// that scroller no longer reaches.
    private func largeTitleShows(_ title: String) -> Bool {
        guard let window else { return false }
        var found = false
        func visit(_ view: UIView, _ alpha: CGFloat, _ clip: CGRect) {
            guard !view.isHidden, !found else { return }
            let a = alpha * view.alpha
            let inside = view.clipsToBounds ? clip.intersection(view.convert(view.bounds, to: window)) : clip
            if let label = view as? UILabel, label.text == title.capitalized, label.font.pointSize >= 26, a > 0.5 {
                let frame = label.convert(label.bounds, to: window), shown = frame.intersection(inside)
                if frame.height > 1, !shown.isNull, shown.height >= frame.height / 2 { found = true }
            }
            for sub in view.subviews { visit(sub, a, inside) }
        }
        visit(window, 1, window.bounds)
        return found
    }

    /// A route's bar and scroller: the bar's height, whether its large title
    /// shows, how far the content is scrolled (CSS scrollTop), and how far
    /// below the bar its first line (`top`, else `<route>-top`) is.
    private func state(_ nav: UINavigationController, _ route: String, _ scroller: String, top: String? = nil) throws -> (bar: CGFloat, large: Bool, scrolled: CGFloat, below: CGFloat) {
        let node = try node(scroller)
        let sv = try XCTUnwrap(session?.presenter.navigation.shownScroll(node))
        let bar = nav.navigationBar
        let first = try top.map { try self.node($0) } ?? find("\(route)-top")
        let below = first.map { $0.convert($0.bounds, to: nil).minY - bar.convert(bar.bounds, to: nil).maxY } ?? 0
        return (bar.frame.height, largeTitleShows(route), sv.contentOffset.y + sv.adjustedContentInset.top, below)
    }

    private func scroll(_ scroller: String, to top: Double) throws {
        let node = try node(scroller)
        node.pendingScrollTop = top
        node.applyPendingScroll()
        settle()
    }

    /// The bar's heights on this device: Home's large title at rest, and an
    /// inline title's.
    private func heights(_ nav: UINavigationController) throws -> (expanded: CGFloat, inline: CGFloat) {
        let expanded = nav.navigationBar.frame.height
        XCTAssertTrue(largeTitleShows("home"), "the stack's first screen opens with its large title")
        try tap("home-open-inline", in: nav, depth: 2)
        let inline = nav.navigationBar.frame.height
        XCTAssertLessThan(inline, expanded - 20, "a level-2 heading's bar is the inline one")
        try back(nav, depth: 1)
        return (expanded, inline)
    }

    /// UIKit's Back, as its button pops: the stack one shorter, at rest.
    private func back(_ nav: UINavigationController, depth: Int) throws {
        nav.popViewController(animated: false)
        until("popped to \(depth)") { rests(nav, depth) }
        settle()
    }

    func testAPushedLevelOneRouteArrivesExpandedCollapsesOnScrollAndExpandsAtTheTop() throws {
        let (session, _, nav) = try fixture()
        let (expanded, inline) = try heights(nav)

        // As the push begins, UIKit's transition still running: the title is
        // open from its first frame, never opened afterwards.
        try press("home-open-long")
        XCTAssertEqual(nav.viewControllers.count, 2, "UIKit's push began with the press")
        XCTAssertNotNil(nav.transitionCoordinator, "and is animated")
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let scroller = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "long-scroll" })
        XCTAssertEqual(scroller.convert(scroller.bounds, to: route.view).minY, 0, accuracy: 0.5, "its scroller is at its controller's top as UIKit lays the push out: the header the bar stands for takes no room")
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5, "the bar is the large title's as the push begins")
        until("the push at rest") { rests(nav, 2) }
        settle()

        var long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, expanded, accuracy: 0.5, "a pushed level-1 route arrives with its bar as tall as a first screen's")
        XCTAssertTrue(long.large, "and its large title showing")
        XCTAssertEqual(long.scrolled, 0, accuracy: 0.5, "its content at its top")
        XCTAssertEqual(long.below, 0, accuracy: 0.5, "its first line just below the expanded title")
        XCTAssertEqual(scroller.scrollOrigin, try XCTUnwrap(scroller.scroll).adjustedContentInset.top, accuracy: 0.5, "CSS scrollTop 0 is the expanded title's inset, sampled once the push is over")

        try scroll("long-scroll", to: 300)
        long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, inline, accuracy: 0.5, "it collapses as its scroller scrolls")
        XCTAssertFalse(long.large)
        XCTAssertEqual(long.scrolled, 300, accuracy: 0.5, "an authored scrollTop lands where the browser's does")

        // Back at the top as a reselected tab takes a screen there under the
        // agent's frozen clock: unanimated (`restAtTop`).
        session.presenter.navigation.scrollsToTopAnimated = false
        session.presenter.navigation.scrollToTop(route)
        settle()
        long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, expanded, accuracy: 0.5, "and expands again at the top")
        XCTAssertTrue(long.large)
        XCTAssertEqual(long.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(long.below, 0, accuracy: 0.5)

        // At its top under a collapsed title (a scroller replaced in place
        // leaves one): the reselected tab opens it, as UIKit's status-bar
        // tap goes on to the open title's inset.
        try scroll("long-scroll", to: 300)
        let sv = try XCTUnwrap(scroller.scroll)
        sv.contentOffset.y = -sv.adjustedContentInset.top
        settle()
        XCTAssertFalse(try state(nav, "long", "long-scroll").large, "collapsed, at its top")
        session.presenter.navigation.scrollToTop(route)
        settle()
        long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, expanded, accuracy: 0.5, "the title open again")
        XCTAssertTrue(long.large)
    }

    /// Pushed without UIKit's animation, as a context menu's commit pushes
    /// and as every push is under the agent's frozen clock.
    func testAPushWithoutAnimationArrivesExpandedToo() throws {
        let (session, _, nav) = try fixture()
        let (expanded, _) = try heights(nav)
        session.presenter.navigation.unanimated = true
        try tap("home-open-long", in: nav, depth: 2)
        let long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, expanded, accuracy: 0.5, "expanded, and still so once `didShow` has run")
        XCTAssertTrue(long.large)
        XCTAssertEqual(long.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(long.below, 0, accuracy: 0.5)
    }

    func testALevelOneRouteWithoutAnAuthoredScrollerArrivesExpanded() throws {
        let (_, _, nav) = try fixture()
        let (expanded, _) = try heights(nav)
        try tap("home-open-edit-visual", in: nav, depth: 2)
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5)
        XCTAssertTrue(largeTitleShows("edit"))
    }

    func testEveryKindOfLevelOneRouteArrivesExpandedAndALevelTwoRouteIsInline() throws {
        let (_, _, nav) = try fixture()
        let (expanded, inline) = try heights(nav)

        try tap("home-open-short", in: nav, depth: 2)
        let short = try state(nav, "short", "short-scroll")
        XCTAssertEqual(short.bar, expanded, accuracy: 0.5, "content shorter than the screen: expanded all the same")
        XCTAssertTrue(short.large)
        XCTAssertEqual(short.below, 0, accuracy: 0.5, "its first line just below the title")
        try back(nav, depth: 1)

        for (route, what) in [("rows", "a grouped list"), ("plain", "a plain list, its rows built in slices")] {
            try tap("home-open-\(route)", in: nav, depth: 2)
            let list = try state(nav, route, "\(route)-scroll")
            XCTAssertEqual(list.bar, expanded, accuracy: 0.5, "a level-1 heading over \(what) arrives expanded")
            XCTAssertTrue(list.large, route)
            XCTAssertEqual(list.scrolled, 0, accuracy: 0.5, route)
            try back(nav, depth: 1)
        }

        // A header's search field is the bar's search controller, which UIKit
        // places: under the title here, so the bar is taller by it.
        try tap("home-open-find", in: nav, depth: 2)
        XCTAssertNotNil(nav.topViewController?.navigationItem.searchController, "the header's search field is the item's search controller")
        let search = try state(nav, "find", "find-scroll")
        XCTAssertGreaterThanOrEqual(search.bar, expanded - 0.5, "a level-1 heading with a search field arrives expanded")
        XCTAssertTrue(search.large)
        XCTAssertEqual(search.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(search.below, 0, accuracy: 0.5)
        try back(nav, depth: 1)

        try tap("home-open-inline", in: nav, depth: 2)
        var plain = try state(nav, "inline", "inline-scroll")
        XCTAssertEqual(plain.bar, inline, accuracy: 0.5, "a pushed level-2 route is inline")
        XCTAssertFalse(plain.large)
        XCTAssertEqual(plain.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(plain.below, 0, accuracy: 0.5, "its first line just below the bar")
        // A pull past the top, as a finger's: an inline title never expands.
        let sv = try XCTUnwrap(try node("inline-scroll").scroll)
        sv.contentOffset.y = -sv.adjustedContentInset.top - 80
        settle()
        plain = try state(nav, "inline", "inline-scroll")
        XCTAssertEqual(plain.bar, inline, accuracy: 0.5, "and never expands")
        XCTAssertFalse(plain.large)
    }

    func testTheScreenUnderneathIsAsItWasLeft() throws {
        let (_, _, nav) = try fixture()
        let (expanded, inline) = try heights(nav)

        // Left at its top: expanded on return.
        try tap("home-open-long", in: nav, depth: 2)
        try back(nav, depth: 1)
        var home = try state(nav, "home", "home-scroll")
        XCTAssertEqual(home.bar, expanded, accuracy: 0.5, "left at its top, the first screen returns expanded")
        XCTAssertTrue(home.large)
        XCTAssertEqual(home.scrolled, 0, accuracy: 0.5)

        // Left scrolled: collapsed on return, where it was.
        try scroll("home-scroll", to: 200)
        try tap("home-open-long", in: nav, depth: 2)
        XCTAssertEqual(try state(nav, "long", "long-scroll").bar, expanded, accuracy: 0.5, "pushed over a collapsed title, it arrives expanded")
        XCTAssertTrue(largeTitleShows("long"))
        // A pushed screen left scrolled, under another.
        try scroll("long-scroll", to: 240)
        try tap("long-open-short", in: nav, depth: 3)
        XCTAssertEqual(try state(nav, "short", "short-scroll").bar, expanded, accuracy: 0.5, "pushed over a pushed screen's collapsed title")
        try back(nav, depth: 2)
        let long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, inline, accuracy: 0.5, "a pushed screen left scrolled returns collapsed")
        XCTAssertEqual(long.scrolled, 240, accuracy: 0.5, "where it was")
        XCTAssertFalse(long.large)
        try back(nav, depth: 1)
        home = try state(nav, "home", "home-scroll")
        XCTAssertEqual(home.bar, inline, accuracy: 0.5, "the first screen left scrolled returns collapsed")
        XCTAssertEqual(home.scrolled, 200, accuracy: 0.5, "where it was")
        XCTAssertFalse(home.large)
    }

    func testATabSwitchAContentSwapAndAReplaceKeepIt() throws {
        let (session, tabs, nav) = try fixture()
        let (expanded, inline) = try heights(nav)
        func select(_ index: Int) {
            let controller = tabs.viewControllers![index]
            if tabs.delegate?.tabBarController?(tabs, shouldSelect: controller) ?? true { tabs.selectedIndex = index }
            until("tab \(index) selected") { tabs.selectedIndex == index && session.presenter.navigation.transitioning == false }
            settle()
        }

        // A tab switch away and back: the pushed screen as it was, expanded.
        try tap("home-open-long", in: nav, depth: 2)
        select(1)
        XCTAssertTrue(largeTitleShows("other"), "the other tab's first screen has its large title")
        select(0)
        var long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, expanded, accuracy: 0.5, "back on its tab, the pushed screen is still expanded")
        XCTAssertTrue(long.large)
        XCTAssertEqual(long.scrolled, 0, accuracy: 0.5)
        // …and collapsed if it was left scrolled.
        try scroll("long-scroll", to: 260)
        select(1)
        select(0)
        long = try state(nav, "long", "long-scroll")
        XCTAssertEqual(long.bar, inline, accuracy: 0.5, "left scrolled, it is still collapsed")
        XCTAssertEqual(long.scrolled, 260, accuracy: 0.5)

        // A `replace`: the route keeps its place and its controller, its
        // header and scroller replaced under the collapsed bar. The bar stays
        // as it was left, as UIKit keeps it for a controller whose scroll view
        // is replaced in place; the new screen opens at its top.
        try tap("long-replace-short", in: nav, depth: 2)
        let short = try state(nav, "short", "short-scroll")
        XCTAssertEqual(short.bar, inline, accuracy: 0.5, "a screen that replaces a scrolled one finds the bar collapsed")
        XCTAssertFalse(short.large)
        XCTAssertEqual(short.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(short.below, 0, accuracy: 0.5, "its first line just below the bar")
        try back(nav, depth: 1)

        // A content swap under a pushed, expanded bar (LLP 1116.005 D4): the
        // scroller replaced in place, then a reason between the header and it.
        try tap("home-open-swap", in: nav, depth: 2)
        var swap = try state(nav, "swap", "swap-first", top: "swap-first-top")
        XCTAssertEqual(swap.bar, expanded, accuracy: 0.5)
        XCTAssertTrue(swap.large)
        try tap("swap-swap", in: nav, depth: 2)
        swap = try state(nav, "swap", "swap-second", top: "swap-second-top")
        XCTAssertEqual(swap.bar, expanded, accuracy: 0.5, "a scroller replaced in place under an open title leaves it open")
        XCTAssertTrue(swap.large)
        XCTAssertEqual(swap.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(swap.below, 0, accuracy: 0.5, "its first line just below the title")
        // …and under a collapsed one leaves it collapsed, as UIKit does.
        try scroll("swap-second", to: 300)
        XCTAssertEqual(nav.navigationBar.frame.height, inline, accuracy: 0.5)
        try tap("swap-back", in: nav, depth: 2)
        swap = try state(nav, "swap", "swap-first", top: "swap-first-top")
        XCTAssertEqual(swap.bar, inline, accuracy: 0.5, "a scroller replaced in place under a collapsed title leaves it collapsed")
        XCTAssertFalse(swap.large)
        XCTAssertEqual(swap.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(swap.below, 0, accuracy: 0.5, "its first line just below the bar")
        // Pushed again, open: a reason between the header and the scroller,
        // which then collapses nothing; the title stays open and in sight.
        try back(nav, depth: 1)
        try tap("home-open-swap", in: nav, depth: 2)
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5)
        try tap("swap-fail", in: nav, depth: 2)
        XCTAssertNotNil(find("swap-reason"), "the reason shows between the header and the scroller")
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5, "nothing collapses the title now, and it stays")
        XCTAssertTrue(largeTitleShows("swap"), "in sight, not left in the scroller below the reason")
        // The reason goes: the scroller, the route's bottom-edge scroller all
        // along and so inset by UIKit throughout, gets the bar back at its top.
        try tap("swap-fail", in: nav, depth: 2)
        XCTAssertNil(find("swap-reason"))
        swap = try state(nav, "swap", "swap-first", top: "swap-first-top")
        XCTAssertEqual(swap.scrolled, 0, accuracy: 0.5, "back at its top, not scrolled by the bar's inset")
        XCTAssertEqual(swap.bar, expanded, accuracy: 0.5, "the title stays open")
        XCTAssertEqual(swap.below, 0, accuracy: 0.5, "its first line just below the title")
    }
    func testAReasonAboveABottomScrollerDoesNotChangeItsPositionWhenRemoved() throws {
        let (_, _, nav) = try fixture()
        let (expanded, _) = try heights(nav)
        try tap("home-open-swap", in: nav, depth: 2)
        try tap("swap-fail", in: nav, depth: 2)
        try tap("swap-fail", in: nav, depth: 2)
        let swap = try state(nav, "swap", "swap-first", top: "swap-first-top")
        XCTAssertEqual(swap.scrolled, 0, accuracy: 0.5)
        XCTAssertEqual(swap.bar, expanded, accuracy: 0.5)
        XCTAssertTrue(swap.large)
    }

    func testReselectingATabOpensACollapsedTitleWhoseContentIsAlreadyAtTheTop() throws {
        let (session, _, nav) = try fixture()
        let (expanded, _) = try heights(nav)
        try tap("home-open-long", in: nav, depth: 2)
        try scroll("long-scroll", to: 300)
        try tap("long-replace-short", in: nav, depth: 2)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        session.presenter.navigation.scrollsToTopAnimated = false
        session.presenter.navigation.scrollToTop(route)
        settle()
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5)
        XCTAssertTrue(largeTitleShows("short"))
    }

    /// Baseline coverage: this drive also passes on the starting revision.
    func testAgentDragsScrollAGroupedListAndPullItsTitleOpen() throws {
        let (session, _, nav) = try fixture()
        let (expanded, inline) = try heights(nav)
        try tap("home-open-rows", in: nav, depth: 2)
        let rows = try node("rows-scroll")
        let agent = Agent(session: session)
        let down = agent.tap(["id": Int(rows.id), "scrollDrag": [0.0, -300.0]])
        XCTAssertNil(down["error"], "\(down)")
        settle()
        XCTAssertGreaterThan(try state(nav, "rows", "rows-scroll").scrolled, 200)
        XCTAssertEqual(nav.navigationBar.frame.height, inline, accuracy: 0.5)
        let up = agent.tap(["id": Int(rows.id), "scrollDrag": [0.0, 600.0]])
        XCTAssertNil(up["error"], "\(up)")
        settle()
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5)
        XCTAssertEqual(try state(nav, "rows", "rows-scroll").scrolled, 0, accuracy: 0.5)
    }

    /// LLP 1141.001 D6: a header's `autofocus` search field, mounted by a
    /// press's push, is the bar's search field, which takes the focus once
    /// the push settles, though under an inline title UIKit has collapsed
    /// it to no height.
    func testAHeadersAutofocusSearchFieldIsFocusedOnceThePushSettles() throws {
        let (_, _, nav) = try fixture()
        try tap("home-open-seek", in: nav, depth: 2)
        let search = try XCTUnwrap(nav.topViewController?.navigationItem.searchController, "the header's search field is the item's search controller")
        until("the bar's search field takes the autofocus") { search.searchBar.searchTextField.isFirstResponder }
        try back(nav, depth: 1)
        try tap("home-open-find", in: nav, depth: 2)
        XCTAssertFalse(try XCTUnwrap(nav.topViewController?.navigationItem.searchController).searchBar.searchTextField.isFirstResponder,
                       "a search field without autofocus waits for a tap")
    }

}
#endif
