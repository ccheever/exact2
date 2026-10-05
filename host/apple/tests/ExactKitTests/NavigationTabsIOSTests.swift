#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 Stage 2 over the native fixture's tabs: Exact's
/// `UITabBarController`, one navigation controller a tab, items from the
/// authored tablist, a tab the bar selects pressing its authored tab, every
/// tab's stack kept — its pushed screen, its scroll offset, its draft — and
/// reselecting a tab popping it to its root. The bar is driven as a tap on
/// it drives it, through the delegate Exact keeps.
final class NavigationTabsIOSTests: XCTestCase {
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
        while !done(), Date() < deadline { spin(0.02) }
        XCTAssertTrue(done(), what)
    }

    private func fixture(_ label: String, module: Bool = false) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        // In the test host's scene when it has one: UIKit presents a sheet
        // only from a window a scene holds.
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
        if module {
            session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"], "build.mjs --test --ios builds the fixture's module"))
        }
        spin(0.3)
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func tapNode(_ session: ExactSession, _ testId: String) throws {
        _ = Agent(session: session).tap(["id": Int(try node(session, testId).id)])
    }

    /// A tap on the bar's item for a tab, as UIKit asks before selecting.
    private func tapTab(_ tabs: UITabBarController, _ index: Int) {
        let target = tabs.viewControllers![index]
        if tabs.delegate?.tabBarController?(tabs, shouldSelect: target) ?? true { tabs.selectedIndex = index }
    }

    /// Regression (590d73531): a root overlay with a z-index after the
    /// tablist, the pattern docs/agent-pitfalls.md gives for full-screen
    /// overlays, paints and takes touches over the native tab container. Dense
    /// ranks had put the container over every authored sibling, so the
    /// overlay was laid out, in the tree, and invisible (LLP 1083.000 D4).
    func testARootOverlayWithAZIndexIsOverTheNativeTabs() throws {
        let session = try fixture("tabs-overlay")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        // Pressed directly: the button may sit below the fold of the home tab.
        session.presenter.press(try node(session, "show-overlay").id)
        let overlayNode = { session.presenter.views.values.first { $0.props["testId"] == "root-overlay" } }
        until("the overlay mounts") { overlayNode() != nil }
        let overlay = try node(session, "root-overlay")
        let parent = try XCTUnwrap(overlay.superview)
        XCTAssertTrue(tabs.view.superview === parent, "the overlay and the tab container are siblings")
        XCTAssertGreaterThan(overlay.layer.zPosition, tabs.view.layer.zPosition, "the overlay paints over the tabs")
        let middle = overlay.convert(CGPoint(x: overlay.bounds.midX, y: overlay.bounds.midY), to: nil)
        let hit = try XCTUnwrap(overlay.window?.hitTest(middle, with: nil))
        XCTAssertTrue(hit.isDescendant(of: overlay), "and takes the touch: \(type(of: hit))")
        let close = try node(session, "hide-overlay")
        let reply = Agent(session: session).tap(["id": Int(close.id)])
        XCTAssertEqual(reply["pressed"] as? Int, Int(close.id), "\(reply)")
        until("the overlay closes") { overlayNode() == nil }
    }

    func testEveryTabKeepsItsStackItsScrollAndItsDraftAndReselectPopsToRoot() throws {
        let session = try fixture("tabs")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController, "the root's tablist names its panels")
        XCTAssertEqual(tabs.viewControllers?.count, 2)
        XCTAssertEqual(tabs.selectedIndex, 0)
        XCTAssertFalse(tabs.tabBar.isHidden)
        XCTAssertEqual(tabs.viewControllers?.map { $0.tabBarItem.title }, ["Home", "Second"])
        XCTAssertTrue(try node(session, "tabs").isHidden, "the bar takes the authored tablist's place")
        // The container paints where the panels are among the root's
        // children: under the tablist after them, as CSS paints a later
        // sibling over an earlier one (shop F21, recipes F23).
        let root = try node(session, "navigation"), tablist = try node(session, "tabs")
        let order = root.subviews.map { ObjectIdentifier($0) }
        XCTAssertLessThan(try XCTUnwrap(order.firstIndex(of: ObjectIdentifier(tabs.view))), try XCTUnwrap(order.firstIndex(of: ObjectIdentifier(tablist))), "the tablist after the panels paints over the container")
        XCTAssertGreaterThan(try XCTUnwrap(order.firstIndex(of: ObjectIdentifier(tabs.view))), try XCTUnwrap(order.firstIndex(of: ObjectIdentifier(try node(session, "panels")))), "the container paints over the panels' box")
        // The tablist's `accent-color` is the bar's tint (recipes F20, shop F28).
        let tint = try XCTUnwrap(tabs.tabBar.tintColor).resolvedColor(with: UITraitCollection(userInterfaceStyle: .light))
        var rgb: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        tint.getRed(&rgb.0, green: &rgb.1, blue: &rgb.2, alpha: &rgb.3)
        XCTAssertEqual([rgb.0, rgb.1, rgb.2].map { Int(($0 * 255).rounded()) }, [0x38, 0x38, 0xf5])
        let home = try XCTUnwrap(tabs.viewControllers?[0] as? UINavigationController)
        let second = try XCTUnwrap(tabs.viewControllers?[1] as? UINavigationController)
        XCTAssertEqual(second.viewControllers.count, 1, "another tab's stack is built too")

        // A pushed screen in Home.
        try tapNode(session, "detail")
        until("detail pushed in Home") { home.viewControllers.count == 2 && home.transitionCoordinator == nil }
        // The bar selects Second through the authored tab.
        tapTab(tabs, 1)
        until("Second selected by the router") { tabs.selectedIndex == 1 }
        XCTAssertEqual(home.viewControllers.count, 2, "Home's stack is kept")
        // A change made while Home is hidden shows when it is selected.
        XCTAssertNil(second.tabBarItem.badgeValue, "no badge box, no badge")
        try tapNode(session, "bump-second")
        // The count's filled box on the Second tab is its item's badge (§9.9).
        until("the badge follows the authored box") { second.tabBarItem.badgeValue == "1" }
        // The box goes, the badge goes; it comes back with the box.
        try tapNode(session, "unbump-second")
        until("the badge leaves with its box") { second.tabBarItem.badgeValue == nil }
        try tapNode(session, "bump-second")
        until("and comes back with it") { second.tabBarItem.badgeValue == "1" }
        XCTAssertNil(home.tabBarItem.badgeValue, "a transparent box is no badge; Second's \"1\" beside its pill shows the pill is none either")
        // A badge a hook set on a tab that never authored one stays through
        // that tab's face changing (its selected symbol).
        home.tabBarItem.badgeValue = "hook"
        // A draft and a scroll in Second.
        let draft = try node(session, "draft")
        _ = Agent(session: session).type(["id": Int(draft.id), "text": "kept"])
        let list = try node(session, "list-second")
        let scroll = try XCTUnwrap(list.scroll)
        scroll.setContentOffset(CGPoint(x: 0, y: 300), animated: false)
        spin(0.2)
        // In CSS terms: under a collapsing title UIKit moves the offset by
        // what the title gives up, and keeps the offset plus its inset.
        let css = { scroll.contentOffset.y + scroll.adjustedContentInset.top }
        let scrolled = css()
        // Away and back.
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        XCTAssertEqual(home.tabBarItem.badgeValue, "hook", "the hook's badge stays")
        XCTAssertEqual(home.topViewController?.navigationItem.title, "Detail", "the pushed screen survived")
        XCTAssertTrue(try node(session, "counts").accessibleText.contains("count 1"), "the hidden tab's update shows")
        tapTab(tabs, 1)
        until("Second selected again") { tabs.selectedIndex == 1 }
        XCTAssertEqual(css(), scrolled, accuracy: 0.5, "the scroll offset survived")
        XCTAssertTrue(try node(session, "list-second") === list, "the same views, retained")
        XCTAssertEqual(try node(session, "draft").field?.text, "kept", "the draft survived")
        // Reselecting a tab pops it to its root.
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        tapTab(tabs, 0)
        until("Home popped to its root") { home.viewControllers.count == 1 && home.transitionCoordinator == nil }
    }

    /// The edge swipe on a tab's stack: Exact stays the delegate of both pop
    /// recognizers once the tab controller has loaded every stack (UIKit sets
    /// its own when a navigation controller's view loads), and a pushed
    /// screen with its back control lets the pop begin, a root does not.
    func testEveryTabsPopGesturesAskExactAndAPushedScreenMayPop() throws {
        let session = try fixture("tabs-pop")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let navs = try XCTUnwrap(tabs.viewControllers as? [UINavigationController])
        func pops(_ nav: UINavigationController) -> [UIGestureRecognizer] {
            var out = [nav.interactivePopGestureRecognizer].compactMap { $0 }
            if #available(iOS 26.0, *), let content = nav.interactiveContentPopGestureRecognizer { out.append(content) }
            return out
        }
        // Every stack loads, as selecting its tab does.
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        for nav in navs {
            XCTAssertTrue(nav.isViewLoaded)
            for pop in pops(nav) { XCTAssertTrue(pop.delegate === navigation, "\(pop) asks Exact") }
        }
        let home = navs[0], second = navs[1]
        // A swipe from the left edge, finger moving right, as UIKit hands it over.
        let edge = CGPoint(x: 4, y: 400), right = CGPoint(x: 600, y: 20)
        func mayPop(_ nav: UINavigationController, velocity: CGPoint = right) -> [Bool] {
            pops(nav).map { navigation.popMayBegin($0, from: edge, in: nav.view, velocity: velocity) }
        }
        XCTAssertFalse(mayPop(home).contains(true), "a root does not pop")
        try tapNode(session, "detail")
        until("detail pushed in Home") { home.viewControllers.count == 2 && home.transitionCoordinator == nil }
        XCTAssertEqual(mayPop(home), pops(home).map { _ in true }, "the pushed screen with its back control pops")
        // Over a `swiperight` row the edge decides: a finger that landed at
        // x = 1 pops, and the start is where it landed, not the pan's
        // translation origin (which leaves out the travel before recognition).
        let detail = try node(session, "route-detail")
        detail.handlers.insert("swiperight")
        defer { detail.handlers.remove("swiperight") }
        // (A test cannot place a `UITouch`, so `shouldReceive`'s first-finger
        // gate is proved by the live drive, LLP 1080.000 §11.)
        for pop in pops(home).compactMap({ $0 as? UIPanGestureRecognizer }) {
            navigation.notePopTouchDown(pop, at: CGPoint(x: 30, y: 400))
            XCTAssertFalse(navigation.popShouldBegin(pop, in: home.view, velocity: right), "landed past the edge: the row's swipe")
            navigation.notePopTouchDown(pop, at: CGPoint(x: 1, y: 400))
            XCTAssertTrue(navigation.popShouldBegin(pop, in: home.view, velocity: right), "landed at the edge: the pop")
        }
        XCTAssertFalse(mayPop(home, velocity: CGPoint(x: 20, y: 600)).contains(true), "a vertical pan is the content's")
        XCTAssertFalse(mayPop(second).contains(true), "a hidden tab's stack does not pop")
        // Its own depth decides once it shows: Second is a root.
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        XCTAssertFalse(mayPop(second).contains(true), "Second is a root")
        XCTAssertFalse(mayPop(home).contains(true), "Home's pushed screen is hidden now")
    }

    func testASheetOverTheTabsIsPresentedByTheContainersParent() throws {
        let session = try fixture("tabs-sheet")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        try tapNode(session, "sheet")
        until("the tab controller's parent presents the sheet") { tabs.parent?.presentedViewController != nil }
        let sheet = try XCTUnwrap(tabs.parent?.presentedViewController?.sheetPresentationController)
        XCTAssertEqual(sheet.detents.count, 2, "two detents, as authored")
        XCTAssertTrue(sheet.prefersGrabberVisible, "a resizable sheet shows its grabber")
        XCTAssertTrue(tabs.presentedViewController == nil || tabs.presentedViewController === tabs.parent?.presentedViewController,
                      "never a tab's own stack")
        // The sheet's header is its bar: Close, at its stack's root, is an
        // item there (UIKit gives a root no back button). Closing runs in
        // the app, under the native smoke: this window finishes no
        // presentation transition, and a dismissal waits for one.
        let close = try XCTUnwrap(navigation.presentedNavigations.last?.topViewController?.navigationItem.leftBarButtonItems?.first)
        XCTAssertEqual(close.accessibilityLabel, "Close")
        XCTAssertEqual(tabs.selectedIndex, 0)
    }

    /// Stage 2's check of `tabBarMinimizeBehavior` (iOS 26), as far as a
    /// unit test reaches: a route whose title stays still (Detail's, inline)
    /// names no content scroll view to UIKit, so a minimize behavior a hook
    /// sets has nothing to follow and the content area holds under scrolling.
    /// A finger's scroll (UIKit's own heuristics) is owed to a device check
    /// (LLP 1075.003 §6).
    func testAMinimizeBehaviorFollowsNoScrollWhileExactNamesNone() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("tabBarMinimizeBehavior is iOS 26") }
        let session = try fixture("tabs-minimize")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        tabs.tabBarMinimizeBehavior = .onScrollDown
        try tapNode(session, "detail")
        until("detail pushed") { session.presenter.navigation.primaryNavigation?.viewControllers.count == 2 }
        let route = try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController)
        XCTAssertNil(route.contentScrollView(for: .top), "a still title names no scroller")
        let scroll = try XCTUnwrap(try node(session, "list-detail").scroll)
        spin(0.3)
        let before = route.view.safeAreaInsets.bottom, frame = tabs.tabBar.frame
        for y in stride(from: 0, through: 600, by: 20) {
            scroll.setContentOffset(CGPoint(x: 0, y: y), animated: false)
            spin(0.016)
        }
        spin(0.6)
        XCTAssertEqual(tabs.tabBar.frame, frame, "the bar did not minimize")
        XCTAssertEqual(route.view.safeAreaInsets.bottom, before, "the content area held")
    }

    func testUnmountingAndRemountingTheViewKeepsEveryTabsStack() throws {
        let session = try fixture("tabs-remount")
        try tapNode(session, "detail")
        let view = try XCTUnwrap(session.view), host = try XCTUnwrap(view.superview)
        until("detail pushed") { session.presenter.navigation.primaryNavigation?.viewControllers.count == 2 }
        view.removeFromSuperview()
        spin(0.2)
        XCTAssertNil(session.presenter.navigation.tabController, "unmounted, the containers go")
        host.addSubview(view)
        until("remounted, the containers come back") { session.presenter.navigation.tabController != nil }
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        XCTAssertEqual((tabs.viewControllers?[0] as? UINavigationController)?.viewControllers.count, 2, "Home's pushed screen is still there")
        XCTAssertEqual(tabs.viewControllers?.count, 2)
    }

    /// A hook-made item clicks its authored control while its route lives,
    /// and reaches nothing once the route ends: not the node that takes its
    /// id after the plan reloads in the same session, not a destroyed one.
    func testAHookMadeItemDoesNothingOnceItsRouteEnds() throws {
        let session = try fixture("tabs-ended", module: true)
        defer { NativeViews.uninstallTable() }
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        let composed = { (try? self.node(session, "composed"))?.accessibleText ?? "" }
        until("hooks replayed") { log().contains("hook route 0: built") }
        let navigation = session.presenter.navigation
        let home = try XCTUnwrap(navigation.tabNavigations[navigation.tabPanels[0]]?.viewControllers.first)
        let more = try XCTUnwrap(home.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hook-more" })
        let target = try XCTUnwrap(more.target as? NSObject), action = try XCTUnwrap(more.action)
        _ = target.perform(action, with: more)
        until("the live item clicks Compose") { composed() == "composed 1" }
        // The plan boots again in this session: its routes end, ids restart.
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])))
        let before = try node(session, "composed")
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        until("the plan booted again") { (try? self.node(session, "composed")).map { $0 !== before } == true }
        spin(0.3)
        let after = composed()
        _ = target.perform(action, with: more)
        spin(0.3)
        XCTAssertEqual(composed(), after, "an ended route's item presses nothing")
        sessions.removeAll { $0 === session }
        session.destroy()
        _ = target.perform(action, with: more)
        spin(0.1)
    }

    /// With the module loaded, a module view is made as its node is, in the
    /// batch that mounts its route: a screen is contained once that batch
    /// is applied, when the route has its controller.
    func testAScreenMadeWithItsRouteIsContainedOnceTheRouteMounts() throws {
        let session = try fixture("tabs-screen", module: true)
        defer { NativeViews.uninstallTable() }
        until("the module connected") { session.agent(#"{"op":"logs","since":0}"#).contains("hook route 0: built") }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])))
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        spin(0.3)
        let navigation = session.presenter.navigation
        let route = try XCTUnwrap(navigation.tabPanels.last.flatMap { navigation.tabNavigations[$0] }?.viewControllers.first)
        until("the screen is contained") { route.children.count == 1 }
        XCTAssertTrue(route.children.first?.view.isDescendant(of: route.view) == true)
    }

    func testTheTabsHookAContainerTheAppOwnsAndANativeScreen() throws {
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); NativeViews.uninstallTable() }
        let session = try fixture("tabs-owned", module: true)
        let navigation = session.presenter.navigation
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        // At a cold launch Exact's container comes first; once the module
        // connects, `tabs` runs on it and `tabContainer` takes its stacks.
        until("the app's container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        XCTAssertTrue(log().contains("hook tabs: built"))
        XCTAssertTrue(log().contains("hook tabContainer: the app's"))
        let owner = try XCTUnwrap(navigation.tabOwner)
        let stacks = navigation.tabPanels.compactMap { navigation.tabNavigations[$0] }
        XCTAssertEqual(stacks.count, 2)
        XCTAssertTrue(stacks.allSatisfy { $0.parent === owner }, "the app's container holds Exact's stacks")
        XCTAssertTrue(try node(session, "tabs").isHidden, "and takes the authored tablist's place")
        // Its own control selects through the router, which says what it chose.
        let control = try XCTUnwrap(owner.view.subviews.compactMap { $0 as? UISegmentedControl }.first)
        control.selectedSegmentIndex = 1
        // What a change does: its actions, sent to their targets (this test
        // host delivers nothing through UIApplication's sendAction).
        for case let target as NSObject in control.allTargets {
            for name in control.actions(forTarget: target, forControlEvent: .valueChanged) ?? [] { target.perform(Selector(name), with: control) }
        }
        until("the router selected Second") { navigation.primaryNavigation === stacks[1] }
        if navigation.primaryNavigation !== stacks[1] { XCTFail("journal: \(log().suffix(2000))") }
        XCTAssertEqual(control.selectedSegmentIndex, 1)
        XCTAssertFalse(stacks[1].view.isHidden)
        // A native screen is a child of its route's controller.
        let route = try XCTUnwrap(stacks[1].topViewController)
        until("the screen is contained") { route.children.count == 1 }
        XCTAssertTrue(route.children.first?.view.isDescendant(of: route.view) == true)
    }
}
#endif
