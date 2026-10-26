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
    private var animationsWereEnabled = true

    override func setUp() {
        super.setUp()
        // Hostless UIKit does not finish an animated tab selection.
        animationsWereEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
    }

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        UIView.setAnimationsEnabled(animationsWereEnabled)
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    /// Whether the tab bar is hidden as the host hides it: `setTabBarHidden`
    /// on iOS 18, which on an iPad hides the floating bar and leaves
    /// `tabBar.isHidden` alone.
    private func tabBarHidden(_ tabs: UITabBarController) -> Bool {
        if #available(iOS 18.0, *) { return tabs.isTabBarHidden }
        return tabs.tabBar.isHidden
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// `wholeScreen`: the window is the device's screen, so it reaches the
    /// home indicator and the status bar and has their safe area. The
    /// default window is iPhone 17's size at the screen's top left, which on
    /// a larger phone (iPhone Air, 18 Pro Max) ends above the indicator, and
    /// UIKit gives it no bottom inset.
    private func fixture(_ label: String, module: Bool = false, wholeScreen: Bool = false, plan key: String = "EXACT_FIXTURE_PLAN") throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[key], "build.mjs --test --ios compiles \(key)")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        // In the test host's scene when it has one: UIKit presents a sheet
        // only from a window a scene holds.
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let iPhone17 = CGRect(x: 0, y: 0, width: 402, height: 874)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: iPhone17)
        // The host may have no connected scene yet; the window's own has the screen.
        let frame = wholeScreen ? (window.windowScene?.screen.bounds ?? iPhone17) : iPhone17
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: frame.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        if module {
            session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"], "build.mjs --test --ios builds the fixture's module"))
        }
        until("root tab container and selected stack are ready") {
            let navigation = session.presenter.navigation
            let container = navigation.tabOwner ?? navigation.tabController
            let selected = navigation.tabController?.selectedViewController ?? navigation.primaryNavigation
            return container?.viewIfLoaded?.window != nil && selected?.viewIfLoaded?.window != nil &&
                !Agent(session: session).nativeInFlight()
        }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func tapNode(_ session: ExactSession, _ testId: String) throws {
        _ = Agent(session: session).tap(["id": Int(try node(session, testId).id)])
    }

    /// The fixture's own switch between the tab bar and the authored
    /// tablist (`appearance: none` on it), as the app's state writes it.
    private func authoredTabs(_ session: ExactSession, _ authored: Bool) throws {
        let list = try node(session, "tabs")
        if session.presenter.navigation.adopts(tablist: list) == !authored { return }
        // Pressed as its handler is (the switch may be on a tab not shown).
        session.apply(session.runtime.press(try node(session, "tabs-authored").id, now: session.now()))
        until(authored ? "the tablist writes appearance none" : "the tablist is the tab bar's again") {
            session.presenter.navigation.adopts(tablist: list) == !authored
        }
    }

    /// A tap on the bar's item for a tab, as UIKit asks before selecting.
    private func tapTab(_ tabs: UITabBarController, _ index: Int) {
        let target = tabs.viewControllers![index]
        if tabs.delegate?.tabBarController?(tabs, shouldSelect: target) ?? true { tabs.selectedIndex = index }
    }

    func testAgentActivatesAHiddenRootTabThroughItsVisibleBar() throws {
        let session = try fixture("agent-root-tab")
        let p = session.presenter, tab = try node(session, "tab-second")
        let tabs = try XCTUnwrap(p.navigation.tabController)
        XCTAssertTrue(try node(session, "tabs").isHidden, "the native bar replaces the authored tablist")
        XCTAssertEqual(tabs.selectedIndex, 0)
        let reply = Agent(session: session).tap(["id": Int(tab.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["native"] as? String, "tab-bar-item")
        until("the visible tab bar selects the retained second stack") { tabs.selectedIndex == 1 }
        try authoredTabs(session, true)
        try authoredTabs(session, false)
        let first = try node(session, "tab-home")
        let back = Agent(session: session).tap(["id": Int(first.id)])
        XCTAssertNil(back["error"], "\(back)")
        XCTAssertEqual(back["native"] as? String, "tab-bar-item")
        until("the restored native tab bar selects the first stack") { tabs.selectedIndex == 0 }
    }

    func testAppearanceNoneKeepsRootTabsAuthoredAndTheirStacksRetained() throws {
        let session = try fixture("authored-tabs", wholeScreen: true)
        let p = session.presenter, navigation = p.navigation
        let list = try node(session, "tabs")
        let tabs = try XCTUnwrap(navigation.tabController)
        let stacks = try XCTUnwrap(tabs.viewControllers)
        try authoredTabs(session, true)
        XCTAssertFalse(navigation.adopts(tablist: list))
        XCTAssertFalse(list.isHidden)
        XCTAssertTrue(tabBarHidden(tabs))
        navigation.reportCovers()
        until("the shown authored tablist has safe-area protection") { navigation.covers[list.id] != nil }
        XCTAssertFalse(list.subviews.contains { $0 is UITabBar || $0 is UISegmentedControl })
        try tapNode(session, "tab-second")
        until("authored tab selects its retained stack") { tabs.selectedIndex == 1 }
        XCTAssertTrue(tabs.viewControllers?[0] === stacks[0])
        XCTAssertTrue(tabs.viewControllers?[1] === stacks[1])
        try authoredTabs(session, false)
        XCTAssertTrue(navigation.adopts(tablist: list))
        XCTAssertTrue(list.isHidden)
        XCTAssertFalse(tabBarHidden(tabs))
        XCTAssertEqual(tabs.selectedIndex, 1)
        XCTAssertTrue(tabs.viewControllers?[0] === stacks[0])
    }

    /// The runtime covers the root tablist the bar replaces before the first
    /// layout, and the presenter holds that cover from its first batch; a
    /// restart's boot covers it again, and its first batch looks for it again.
    func testTheBootsTablistCoverIsThePresentersAtEveryBoot() throws {
        let session = try fixture("boot-tablist")
        let navigation = session.presenter.navigation
        let covered = { () throws -> Bool in
            let root = try XCTUnwrap(session.presenter.root.subviews.first as? NodeView)
            return try XCTUnwrap(session.runtime.rootTabs(root.id)).covered
        }
        var list = try node(session, "tabs")
        XCTAssertTrue(try covered())
        XCTAssertEqual(navigation.delivered[list.id], .whole)
        XCTAssertTrue(navigation.bootCoverSeen)
        session.presenter.reset()
        XCTAssertFalse(navigation.bootCoverSeen, "a restart's first batch looks again")
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])))
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        spin(0.3)
        list = try node(session, "tabs")
        XCTAssertTrue(try covered())
        XCTAssertTrue(navigation.bootCoverSeen)
        XCTAssertEqual(navigation.delivered[list.id], .whole)
    }

    /// A style that drops the tablist's `accent-color` (here one sent by
    /// hand, which replaces the whole style) as the bar hides: the bar's
    /// tint, whose fallback was the bar's own tint, recursed until the
    /// stack ran out and the process died.
    func testABarTintWhoseAccentIsGoneStillResolves() throws {
        let session = try fixture("tint-without-accent")
        let p = session.presenter, list = try node(session, "tabs")
        let tabs = try XCTUnwrap(p.navigation.tabController)
        let tint = try XCTUnwrap(tabs.tabBar.tintColor)
        p.apply(wireBatch([["op": "style", "id": Int(list.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertTrue(tabs.tabBar.isHidden)
        XCTAssertNotNil(tint.resolvedColor(with: tabs.tabBar.traitCollection).cgColor)
    }

    /// A tablist whose accent is the app's tint (`AccentColor`) has the tint
    /// it inherits. Where the bar inherits that tint too, the bar's is left
    /// to UIKit: behind an alert UIKit dims an inherited tint once, and a
    /// bar tint read from a view's dimmed tint was dimmed again, so the
    /// selected tab went grey where a hand-built app's stays dark. Where an
    /// ancestor the bar is not inside sets the colour, the bar is still
    /// given the colour the tablist resolves.
    func testABarOnTheAppTintInheritsWhatTheTablistDoes() throws {
        let session = try fixture("tint-inherited")
        let p = session.presenter, list = try node(session, "tabs")
        let tabs = try XCTUnwrap(p.navigation.tabController)
        let traits = tabs.tabBar.traitCollection
        let rgb = { (color: UIColor) -> [Int] in
            var c: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
            color.resolvedColor(with: traits).getRed(&c.0, green: &c.1, blue: &c.2, alpha: &c.3)
            return [c.0, c.1, c.2].map { Int(($0 * 255).rounded()) }
        }
        p.apply(wireBatch([["op": "style", "id": Int(list.id), "style": ["accent_color": ["sys": "@tint"], "written": ["accent_color"]]]]))
        tabs.view.tintColor = .systemRed
        XCTAssertEqual(tabs.tabBar.tintColor.resolvedColor(with: traits), UIColor.systemRed.resolvedColor(with: traits),
                       "the bar's tint is its container's, which UIKit dims once")
        // A wrapper the bar is not inside sets the colour a nested tablist inherits.
        p.apply(wireBatch([["op": "create", "id": 9001, "kind": "view", "style": ["accent_color": [0xcc, 0, 0, 255], "written": ["accent_color"]]],
                           ["op": "create", "id": 9002, "kind": "view", "style": ["accent_color": ["sys": "@tint"], "written": ["accent_color"]]]]))
        let wrapper = try XCTUnwrap(p.views[9001]), nested = try XCTUnwrap(p.views[9002])
        try node(session, "panels").addSubview(wrapper)
        wrapper.addSubview(nested)
        XCTAssertFalse(tabs.tabBar.isDescendant(of: wrapper))
        p.navigation.tint(nested)
        XCTAssertEqual(rgb(tabs.tabBar.tintColor), rgb(nested.tintColor), "the colour the nested tablist resolves, not the bar's inherited tint")
        XCTAssertNotEqual(rgb(tabs.tabBar.tintColor), rgb(.systemRed))
        // An authored colour on the tablist is the bar's own.
        p.apply(wireBatch([["op": "style", "id": Int(list.id), "style": ["accent_color": [0x38, 0x38, 0xf5, 255], "written": ["accent_color"]]]]))
        XCTAssertEqual(rgb(tabs.tabBar.tintColor), [0x38, 0x38, 0xf5])
    }

    func testAppearanceToggleCoversSettleWithoutRepeatedDelivery() throws {
        let session = try fixture("authored-tabs-settle")
        let p = session.presenter, view = try XCTUnwrap(session.view)
        let deliver = p.onCovers
        var deliveries = 0
        p.onCovers = { changes in
            deliveries += 1
            deliver?(changes)
        }
        defer { p.onCovers = deliver }
        for appearance in ["none", "auto", "none"] {
            try authoredTabs(session, appearance == "none")
            for _ in 0..<12 {
                view.layoutIfNeeded()
                p.navigation.reportCovers()
                RunLoop.main.run(mode: .default, before: Date())
            }
            let settled = deliveries
            for _ in 0..<12 {
                view.layoutIfNeeded()
                p.navigation.reportCovers()
                RunLoop.main.run(mode: .default, before: Date())
            }
            XCTAssertEqual(deliveries, settled, "unchanged geometry must not send another cover batch")
            XCTAssertEqual(p.navigation.tabController?.tabBar.isHidden, appearance == "none")
        }
    }

    func testAuthoredTabsStayAboveTheHomeIndicatorUnlessTheViewportCoversIt() throws {
        let session = try fixture("authored-tabs-safe-area", wholeScreen: true)
        let p = session.presenter
        let list = try node(session, "tabs")
        let window = try XCTUnwrap(self.window)
        let root = try node(session, "navigation")
        XCTAssertGreaterThan(window.safeAreaInsets.bottom, 0)
        try authoredTabs(session, true)
        let bottom = window.bounds.maxY - window.safeAreaInsets.bottom
        let tab = try node(session, "tab-second")
        until("authored tab content stays above the home indicator") {
            p.navigation.covers[list.id] != nil && tab.bounds.height > 0 && tab.convert(tab.bounds, to: window).maxY <= bottom + 0.5
        }
        XCTAssertEqual(list.convert(list.bounds, to: window).maxY, window.bounds.maxY, accuracy: 0.5,
                       "the authored background reaches the window edge")
        XCTAssertEqual(list.bounds.height, 57 + window.safeAreaInsets.bottom, accuracy: 0.5,
                       "the cover protects the authored content without shrinking it")
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["viewportFit": "cover"]]]))
        session.view?.fit()
        until("cover explicitly allows tab content under the indicator") { tab.convert(tab.bounds, to: window).maxY > bottom + 0.5 }
    }

    func testAuthoredTopTabsProtectContentWithoutShrinkingTheViewport() throws {
        let session = try fixture("authored-top-tabs")
        let p = session.presenter, view = try XCTUnwrap(session.view)
        let list = try node(session, "tabs"), tab = try node(session, "tab-second")
        try authoredTabs(session, true)
        try tapNode(session, "tabs-at-top")
        let window = try XCTUnwrap(self.window)
        guard window.safeAreaInsets.top > 0 else {
            throw XCTSkip("no status bar above the content to protect it from: the iPhone Duo's is in a side column")
        }
        until("headerless top tab content clears the status bar") {
            abs(list.convert(list.bounds, to: window).minY) < 0.5 &&
                tab.convert(tab.bounds, to: window).minY >= window.safeAreaInsets.top - 0.5
        }
        XCTAssertEqual(list.convert(list.bounds, to: window).minY, 0, accuracy: 0.5)
        XCTAssertEqual(p.viewport.frame, view.bounds)
        XCTAssertNotNil(p.navigation.covers[list.id])
        let root = try node(session, "navigation")
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["viewportFit": "cover"]]]))
        view.fit()
        until("cover leaves top protection to the author") { p.navigation.covers[list.id] == nil }
    }

    func testHiddenAuthoredTabsRestoreTheWholeViewport() throws {
        let session = try fixture("hidden-authored-tabs")
        let p = session.presenter, view = try XCTUnwrap(session.view)
        let list = try node(session, "tabs")
        try authoredTabs(session, true)
        try tapNode(session, "chat")
        until("hidden tablist takes no safe-area cover") { list.isHidden && p.navigation.covers[list.id] == nil }
        view.fit()
        XCTAssertEqual(p.viewport.frame, view.bounds)
    }

    func testAuthoredTabsDoNotInsetASheetViewport() throws {
        let session = try fixture("authored-tabs-sheet", wholeScreen: true)
        let p = session.presenter
        let list = try node(session, "tabs")
        try authoredTabs(session, true)
        until("the authored tablist clears the home indicator") { p.navigation.covers[list.id] != nil }
        let protected = p.navigation.covers[list.id]
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        try tapNode(session, "menu")
        until("the menu is a sheet") { p.modals.coordinateView != nil }
        let container = try XCTUnwrap(p.modals.coordinateView)
        // This window finishes no presentation transition: the sheet's view
        // is given the frame UIKit's would have.
        container.frame = CGRect(x: 0, y: 300, width: 402, height: 574)
        p.navigation.reportCovers()
        XCTAssertEqual(p.navigation.covers[list.id], protected, "the root's tablist is not measured under a sheet")
        let view = try XCTUnwrap(session.view)
        view.fit()
        // The document stays in the session's view; the sheet lays its own
        // routes out in their blocks (LLP 1123 D1, D2)
        XCTAssertEqual(p.viewport.frame, view.bounds, "a sheet does not move or resize the document")
    }

    func testTabsSwitchedToAuthoredUnderASheetClearTheHomeIndicator() throws {
        let session = try fixture("authored-tabs-under-sheet", wholeScreen: true)
        let p = session.presenter
        let list = try node(session, "tabs")
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        try tapNode(session, "menu")
        until("the menu is a sheet") { p.modals.coordinateView != nil }
        try authoredTabs(session, true)
        until("authored tabs get their edge cover while the sheet is open") {
            if case .edges(let edges)? = p.navigation.covers[list.id] { return edges.bottom > 0 }
            return false
        }
        XCTAssertEqual(p.viewport.frame, try XCTUnwrap(session.view).bounds)
    }

    func testAuthoredTabsLeaveLandscapeNavigationBackgroundAtTheEdges() throws {
        let session = try fixture("authored-tabs-landscape")
        let p = session.presenter, view = try XCTUnwrap(session.view)
        let window = try XCTUnwrap(self.window), host = try XCTUnwrap(window.rootViewController)
        // Deterministic landscape geometry without rotating the shared simulator.
        window.frame = CGRect(x: 0, y: 0, width: 874, height: 402)
        host.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: 59, bottom: 0, right: 59)
        window.layoutIfNeeded()
        view.frame = host.view.bounds
        try authoredTabs(session, true)
        view.fit()
        window.layoutIfNeeded()
        let bar = try XCTUnwrap(p.navigation.primaryNavigation?.navigationBar)
        XCTAssertGreaterThan(view.safeAreaInsets.left, 0)
        XCTAssertEqual(p.viewport.frame, view.bounds)
        XCTAssertEqual(bar.convert(bar.bounds, to: view).minX, 0, accuracy: 0.5)
        XCTAssertEqual(bar.bounds.width, view.bounds.width, accuracy: 0.5)
    }

    func testAuthoredTabsAreNotSubstitutedForPlatformTouch() throws {
        let session = try fixture("authored-tabs-touch")
        let tab = try node(session, "tab-second")
        let agent = Agent(session: session)
        XCTAssertNotNil(agent.substituted(tab))
        try authoredTabs(session, true)
        XCTAssertNil(agent.substituted(tab), "platform touch must target the visible authored tab")
        let reply = agent.aim(["id": Int(tab.id), "aim": ["press": true]])
        if (reply["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "test host is in the background") }
        let aim = try XCTUnwrap(reply["aim"] as? [String: Any], "\(reply)")
        let point = try XCTUnwrap(aim["window"] as? [Double])
        let hit = try XCTUnwrap(window?.hitTest(CGPoint(x: point[0], y: point[1]), with: nil))
        XCTAssertTrue(hit === tab || hit.isDescendant(of: tab))
    }

    func testARIAUnavailableRootTabProjectsToItsBarItem() throws {
        let session = try fixture("aria-root-tab")
        let tab = try node(session, "tab-second")
        session.presenter.apply(wireBatch([["op": "props", "id": Int(tab.id), "set": ["accessibilityDisabled": "true"]]]))
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let item = try XCTUnwrap(tabs.viewControllers?[1].tabBarItem)
        XCTAssertTrue(item.isEnabled)
        XCTAssertTrue(item.accessibilityTraits.contains(.notEnabled))
        tapTab(tabs, 1)
        until("ARIA-unavailable tab still selects") { tabs.selectedIndex == 1 }
        session.presenter.apply(wireBatch([["op": "props", "id": Int(tab.id), "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(item.accessibilityTraits.contains(.notEnabled))
    }

    func testARIAUnavailableTabPanelReachesLiftedRouteAndRefreshesItsButton() throws {
        let session = try fixture("aria-panel-route")
        let p = session.presenter
        let panel = try node(session, "panel-home")
        let route = try node(session, "route-home")
        let button = try node(session, "show-overlay")
        XCTAssertTrue(p.navigation.logicalParent(of: route) === panel)
        XCTAssertFalse(route.isDescendant(of: panel), "UIKit lifts the route into its controller")
        XCTAssertFalse(button.accessibilityTraits.contains(.notEnabled))

        p.apply(wireBatch([["op": "props", "id": Int(panel.id), "set": ["accessibilityDisabled": "true"]]]))
        XCTAssertTrue(button.ariaUnavailable, "inherit from the authored panel")
        XCTAssertTrue(button.accessibilityTraits.contains(.notEnabled), "the ancestor change refreshes the exposed button")
        XCTAssertFalse(button.disabled, "ARIA state leaves input available")

        p.apply(wireBatch([["op": "props", "id": Int(button.id), "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(button.ariaUnavailable, "an explicit false overrides the panel")
        XCTAssertFalse(button.accessibilityTraits.contains(.notEnabled))
        p.apply(wireBatch([["op": "props", "id": Int(button.id), "clear": ["accessibilityDisabled"]]]))
        XCTAssertTrue(button.accessibilityTraits.contains(.notEnabled), "clearing the override restores inheritance")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(button.id)
        XCTAssertEqual(pressed, [button.id], "the lifted descendant still dispatches its press")
        p.apply(wireBatch([["op": "props", "id": Int(panel.id), "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(button.accessibilityTraits.contains(.notEnabled), "the ancestor becoming available refreshes the lifted route")
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

    /// A root toast after the tablist, positioned with no `z-index` (the
    /// toast of contract/corpus/tabs.contract): the tab container paints at
    /// the routes' place, so the later positioned sibling paints and takes
    /// the touch over it, as CSS paints positioned siblings in tree order
    /// (splitter rough 3: on iOS such a toast was never seen).
    func testARootToastAfterTheTablistIsOverTheNativeTabs() throws {
        let session = try fixture("tabs-toast")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        session.presenter.press(try node(session, "say").id)
        let toastNode = { session.presenter.views.values.first { $0.props["testId"] == "root-toast" } }
        until("the toast mounts") { toastNode() != nil }
        let toast = try node(session, "root-toast")
        XCTAssertTrue(tabs.view.superview === toast.superview, "the toast and the tab container are siblings")
        let siblings = try XCTUnwrap(toast.superview).subviews
        XCTAssertGreaterThan(siblings.firstIndex { $0 === toast }!, siblings.firstIndex { $0 === tabs.view }!, "the toast is after the container")
        XCTAssertGreaterThanOrEqual(toast.layer.zPosition, tabs.view.layer.zPosition, "and no lower")
        let middle = toast.convert(CGPoint(x: toast.bounds.midX, y: toast.bounds.midY), to: nil)
        let hit = try XCTUnwrap(toast.window?.hitTest(middle, with: nil))
        XCTAssertTrue(hit.isDescendant(of: toast), "it takes the touch: \(type(of: hit))")
        let reply = Agent(session: session).tap(["id": Int(toast.id)])
        XCTAssertEqual(reply["pressed"] as? Int, Int(toast.id), "\(reply)")
        until("the toast closes") { toastNode() == nil }
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
        XCTAssertEqual(second.viewControllers.count, 0, "another tab is built when first selected (LLP 1075.003 §9.14)")

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
        // A badge a hatch set on a tab that never authored one stays through
        // that tab's face changing (its selected symbol).
        home.tabBarItem.badgeValue = "hatch"
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
        XCTAssertEqual(home.tabBarItem.badgeValue, "hatch", "the hatch's badge stays")
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

    /// A warm deep link into a tab built earlier (Brooks): from Second, one
    /// update selects Home and pushes Detail onto its stack, which shows
    /// Detail, not Home's root.
    func testATabSelectedWithAPushOntoItsStackShowsThePush() throws {
        let session = try fixture("tabs-select-push")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let home = try XCTUnwrap(tabs.viewControllers?[0] as? UINavigationController)
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 && !navigation.inTransition }
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        session.presenter.press(try node(session, "open-detail").id)
        until("Home shows Detail") { tabs.selectedIndex == 0 && home.viewControllers.count == 2 && home.transitionCoordinator == nil && !navigation.inTransition }
        let top = try XCTUnwrap(home.topViewController as? RouteController)
        XCTAssertEqual(top.node.props["testId"], "route-detail")
        XCTAssertTrue(navigation.primaryNavigation === home)
        XCTAssertTrue(navigation.activeRoute === top)
        XCTAssertNotNil(top.view.window, "Detail is in the window")
        XCTAssertNil(home.viewControllers[0].viewIfLoaded?.window, "Home's root is under it")
    }

    /// A native button tab's face is the runner's (`Runtime.buttonFace`), a
    /// synchronous read on the owner thread. A batch that changes nothing a
    /// tab shows reads no tab's face; one that changes a tab's text still
    /// retitles its item.
    func testATabsFaceIsReadAgainOnlyAfterABatchThatCanChangeIt() throws {
        let session = try fixture("tab-faces", plan: "EXACT_HELD_TAB_PLAN")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let tabNodes = try ["tab-plain", "tab-titled"].map { try node(session, $0) }
        XCTAssertTrue(tabNodes.allSatisfy(\.isNativeButton), "the held-tab fixture's tabs are native buttons")
        XCTAssertEqual(tabs.viewControllers?.map { $0.tabBarItem.title }, ["Plain", "Titled"])
        var reads: [UInt32: Int] = [:]
        let read = try XCTUnwrap(session.presenter.buttonFace)
        session.presenter.buttonFace = { reads[$0, default: 0] += 1; return read($0) }
        let syncs = navigation.syncCalls
        session.presenter.press(try node(session, "bump").id)
        until("the bump shows") { (try? node(session, "bumps"))?.accessibleText == "Bumped 1" }
        until("and is projected") { navigation.syncCalls > syncs }
        XCTAssertEqual(tabNodes.map { reads[$0.id] ?? 0 }, [0, 0], "no tab's face is read for a text no tab shows")
        session.presenter.press(try node(session, "rename").id)
        until("the item's title follows its tab's text") { tabs.viewControllers?[1].tabBarItem.title == "Renamed" }
        XCTAssertEqual(tabs.viewControllers?[0].tabBarItem.title, "Plain")
    }

    /// A tab selected while a push animates: the pushing stack stops being
    /// the one navigation follows, so its `didShow` is not navigation's, and
    /// the push must still end for later navigation to apply.
    func testATabSelectedDuringAPushLeavesNavigationFree() throws {
        let session = try fixture("tabs-mid-push")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let home = try XCTUnwrap(tabs.viewControllers?[0] as? UINavigationController)
        // Second shown once already: a stack's first appearance calls its
        // `didShow`, which would end the push's transition by itself.
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        try selectDuringPush(session, home) { self.tapTab(tabs, 1) } selected: { tabs.selectedIndex == 1 }
        // Reselecting Home pops it to its root: a sync, which waits for no transition.
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        tapTab(tabs, 0)
        until("Home popped to its root") { home.viewControllers.count == 1 && home.transitionCoordinator == nil }

        // Away and back while the push runs: Home's transition is navigation's
        // again, and its own `didShow` ends it.
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try tapNode(session, "detail")
        XCTAssertNotNil(home.transitionCoordinator, "the push is animating")
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        XCTAssertNotNil(home.transitionCoordinator, "Home was selected again while its push ran")
        XCTAssertTrue(navigation.changing, "navigation waits for Home's push again")
        until("the push over and navigation free") { home.transitionCoordinator == nil && !navigation.changing }
        UIView.setAnimationsEnabled(false)
        tapTab(tabs, 0)
        until("Home popped to its root again") { home.viewControllers.count == 1 && home.transitionCoordinator == nil }
    }

    func testATabFollowedAfterItsPopStartsCompletesNavigation() throws {
        let session = try fixture("tabs-follow-mid-pop")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let home = try XCTUnwrap(tabs.viewControllers?[0] as? UINavigationController)
        try tapNode(session, "detail")
        until("Home holds its pushed route") { home.viewControllers.count == 2 }
        tapTab(tabs, 1)
        until("Exact follows the other tab") { tabs.selectedIndex == 1 && navigation.primaryNavigation !== home }
        let second = try XCTUnwrap(navigation.primaryNavigation)
        let root = try XCTUnwrap(home.viewControllers.first)
        let detail = try XCTUnwrap(home.topViewController)

        // UIKit shows the selection before Exact's delayed selection batch follows it
        tabs.selectedIndex = 0
        tabs.view.layoutIfNeeded()
        XCTAssertNotNil(home.view.window, "Home is visible before its pop starts")
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        home.popViewController(animated: true)
        let transition = try XCTUnwrap(home.transitionCoordinator)
        XCTAssertTrue(transition.isAnimated)
        XCTAssertTrue(transition.viewController(forKey: .from) === detail)
        XCTAssertTrue(transition.viewController(forKey: .to) === root)
        XCTAssertTrue(navigation.primaryNavigation === second, "the pop starts before Exact follows Home")
        try tapNode(session, "tab-home")
        XCTAssertTrue(navigation.primaryNavigation === home)
        XCTAssertNotNil(home.transitionCoordinator, "Exact follows Home before the native pop finishes")

        // didShow supplies completion when willShow ran before Exact followed Home
        until("the completed pop releases navigation and reaches the router") {
            home.transitionCoordinator == nil && !navigation.inTransition
                && home.viewControllers.count == 1
                && navigation.selectedKey == (home.topViewController as? RouteController)?.key
        }
    }

    /// The same in a container the app owns (the tabs hatch).
    func testATabSelectedDuringAPushInTheAppsContainerLeavesNavigationFree() throws {
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); NativeViews.uninstallTable() }
        let session = try fixture("tabs-owned-mid-push", module: true)
        let navigation = session.presenter.navigation
        until("the app's container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        let owner = try XCTUnwrap(navigation.tabOwner)
        let stacks = navigation.tabPanels.compactMap { navigation.tabNavigations[$0] }
        let control = try XCTUnwrap(owner.view.subviews.compactMap { $0 as? UISegmentedControl }.first)
        try selectDuringPush(session, stacks[0]) {
            control.selectedSegmentIndex = 1
            for case let target as NSObject in control.allTargets {
                for name in control.actions(forTarget: target, forControlEvent: .valueChanged) ?? [] { target.perform(Selector(name), with: control) }
            }
        } selected: { navigation.primaryNavigation === stacks[1] }
    }

    /// Pushes Detail on `home` with animation, selects another tab before the
    /// push ends, and waits for navigation to leave the transition.
    private func selectDuringPush(_ session: ExactSession, _ home: UINavigationController, select: () -> Void, selected: () -> Bool) throws {
        // The push must still be running when the selection's batch arrives.
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try tapNode(session, "detail")
        XCTAssertNotNil(home.transitionCoordinator, "the push is animating")
        select()
        var selectedMidPush = false
        until("the other tab selected and the push over") {
            if selected(), home.transitionCoordinator != nil { selectedMidPush = true }
            return selected() && home.transitionCoordinator == nil
        }
        XCTAssertTrue(selectedMidPush, "the tab was selected while the push ran")
        until("navigation left the transition") { !session.presenter.navigation.changing }
        let transition = session.presenter.navigation.observation()["transition"] as? [String: Any]
        XCTAssertEqual(transition?["phase"] as? String, "idle", "a programmatic push settles idle")
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
    /// names no content scroll view to UIKit, so a minimize behavior a hatch
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

    /// The bottom edge under a tab bar (LLP 1116 D2): Second's scroller, its
    /// route's last child, runs under the bar to the screen's edge, and UIKit
    /// insets its content by the bar and the home indicator, as a table
    /// view's under a tab bar is.
    func testARoutesLastScrollerRunsUnderTheTabBar() throws {
        let session = try fixture("tabs-edge")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        spin(0.3)
        let second = try XCTUnwrap((tabs.viewControllers?[1] as? UINavigationController)?.topViewController as? RouteController)
        let list = try node(session, "list-second"), sv = try XCTUnwrap(list.scroll)
        XCTAssertFalse(tabs.tabBar.isHidden)
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .automatic)
        XCTAssertEqual(list.convert(list.bounds, to: second.view).maxY, second.view.bounds.maxY, accuracy: 0.5, "under the bar")
        XCTAssertEqual(sv.adjustedContentInset.bottom, second.view.safeAreaInsets.bottom, accuracy: 0.5, "inset by the bar and the home indicator")
        XCTAssertGreaterThan(sv.adjustedContentInset.bottom, 40, "the bar's height and the indicator's")
        XCTAssertTrue(second.contentScrollView(for: .bottom) === sv, "the bar's edge follows it")
    }

    /// LLP 1115.000 D11.3: the selected tab, tapped again at its stack's
    /// root, scrolls that screen's content to the top, its large title
    /// expanding, as every tab bar app does. The scroller is the route's
    /// content scroller, the status bar's (D11.1), and it scrolls with
    /// UIKit's animation, which opens the title as it arrives. Another tab's
    /// tap only selects it, and above the root the router pops (the test
    /// above). The agent's frozen clock lands there unanimated
    /// (PushedTitleIOSTests).
    func testTheSelectedTabTappedAgainAtItsRootScrollsToTheTop() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "UIKit animates a scroll view's offset only in a scene's window")
        }
        let session = try fixture("tabs-retap")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        spin(0.3)
        let nav = try XCTUnwrap(tabs.viewControllers?[1] as? UINavigationController)
        // UIKit shows no large title where it places a vertical bar: the iPhone Duo upright or closed
        // holds a 28-point title in a fixed 58-point bar
        let large = !nav.traitCollection.placesVerticalBar
        let list = try node(session, "list-second"), sv = try XCTUnwrap(list.scroll)
        let css = { sv.contentOffset.y + sv.adjustedContentInset.top }
        let expanded = nav.navigationBar.frame.height
        list.pendingScrollTop = 600
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertEqual(css(), 600, accuracy: 0.5, "scrolled down")
        if large {
            XCTAssertLessThan(nav.navigationBar.frame.height, expanded, "the large title collapsed")
        } else {
            XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5, "the bar kept its height")
        }
        // The other tab, then back: selecting a tab keeps its scroll.
        tapTab(tabs, 0)
        until("Home selected") { tabs.selectedIndex == 0 }
        tapTab(tabs, 1)
        until("Second selected again") { tabs.selectedIndex == 1 }
        spin(0.3)
        XCTAssertEqual(css(), 600, accuracy: 0.5, "a tab selected from another keeps its place")
        // The selected tab, tapped again at its root.
        tapTab(tabs, 1)
        until("its content scrolled to the top") { abs(css()) < 0.5 && !sv.isDecelerating }
        spin(0.5)
        XCTAssertEqual(css(), 0, accuracy: 0.5)
        XCTAssertEqual(nav.navigationBar.frame.height, expanded, accuracy: 0.5, large ? "and the large title expanded" : "and the bar kept its height")
        XCTAssertEqual(nav.viewControllers.count, 1)
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

    /// A hatch-made item clicks its authored control while its route lives,
    /// and reaches nothing once the route ends: not the node that takes its
    /// id after the plan reloads in the same session, not a destroyed one.
    func testAHatchMadeItemDoesNothingOnceItsRouteEnds() throws {
        let session = try fixture("tabs-ended", module: true)
        defer { NativeViews.uninstallTable() }
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        let composed = { (try? self.node(session, "composed"))?.accessibleText ?? "" }
        until("hatches replayed") { log().contains("hatch route 0: built") }
        let navigation = session.presenter.navigation
        let home = try XCTUnwrap(navigation.tabNavigations[navigation.tabPanels[0]]?.viewControllers.first)
        let more = try XCTUnwrap(home.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" })
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
    /// is applied, when the route has its controller. On a tab not yet
    /// selected that is the batch that first selects it, which builds the
    /// tab (LLP 1075.003 §3.7): until then the tab has no route.
    func testAScreenMadeWithItsRouteIsContainedOnceTheRouteMounts() throws {
        let session = try fixture("tabs-screen", module: true)
        defer { NativeViews.uninstallTable() }
        until("the module connected") { session.agent(#"{"op":"logs","since":0}"#).contains("hatch route 0: built") }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])))
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        spin(0.3)
        let navigation = session.presenter.navigation
        let other = try XCTUnwrap(navigation.tabPanels.last.flatMap { navigation.tabNavigations[$0] })
        XCTAssertTrue(other.viewControllers.isEmpty, "the other tab waits for its first selection")
        let tabs = try XCTUnwrap(navigation.tabController)
        tapTab(tabs, tabs.viewControllers!.count - 1)
        until("the screen is contained") { other.viewControllers.first?.children.count == 1 }
        let route = try XCTUnwrap(other.viewControllers.first)
        XCTAssertTrue(route.children.first?.view.isDescendant(of: route.view) == true)
    }

    /// A `tabContainer` hook that keeps none of the stacks leaves them in
    /// Exact's container, its bar showing the selected tab.
    func testATabContainerHookThatKeepsNoneLeavesTheSelectedTabShown() throws {
        defer { NativeViews.uninstallTable() }
        let session = try fixture("tabs-kept", module: true)
        let navigation = session.presenter.navigation
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        until("the module connected") { log().contains("hatch tabs: built") }
        until("the hook was asked") { navigation.tabContainerAsked }
        let container = try XCTUnwrap(navigation.tabController)
        let stacks = navigation.tabPanels.compactMap { navigation.tabNavigations[$0] }
        XCTAssertEqual(container.viewControllers?.count, stacks.count)
        XCTAssertTrue(zip(container.viewControllers ?? [], stacks).allSatisfy { $0 === $1 }, "Exact's container keeps its stacks")
        XCTAssertNotNil(container.tabBar.selectedItem, "the bar shows a tab selected")
        XCTAssertEqual(container.tabBar.selectedItem, container.tabBar.items?[container.selectedIndex])
    }

    func testAnEagerTabContainerTakesTheLiveStacks() throws {
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        setenv("EXACT_FIXTURE_EAGER_TABS", "1", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); unsetenv("EXACT_FIXTURE_EAGER_TABS"); NativeViews.uninstallTable() }
        let session = try fixture("tabs-eager", module: true)
        let navigation = session.presenter.navigation
        until("the eager container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        let owner = try XCTUnwrap(navigation.tabOwner)
        let stacks = navigation.tabPanels.compactMap { navigation.tabStack($0) }
        XCTAssertEqual(stacks.count, 2)
        XCTAssertTrue(stacks.allSatisfy { $0.parent === owner })
        XCTAssertNotNil(owner.view.window)
        let selected = try XCTUnwrap(navigation.primaryNavigation?.topViewController as? RouteController)
        XCTAssertTrue(selected.node.isDescendant(of: owner.view), "the eager view holds the live selected route")
        try tapNode(session, "tab-second")
        until("the eager container shows Second") { navigation.routerTab == 1 && navigation.activeRoute?.node.props["testId"] == "route-second" }
        XCTAssertTrue(try XCTUnwrap(navigation.activeRoute).node.isDescendant(of: owner.view))
    }

    func testSharePresentsOverAnAppOwnedTabContainer() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else {
            throw missing("scene", "sharing needs a hosted window scene")
        }
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); NativeViews.uninstallTable() }
        let session = try fixture("share-owned-tabs", module: true, wholeScreen: true)
        let navigation = session.presenter.navigation
        until("the app's container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        try tapNode(session, "detail")
        until("Detail is shown") { navigation.primaryNavigation?.viewControllers.count == 2 }
        let (anchor, rect) = session.presenter.shareAnchor(nil)
        let sheet = try XCTUnwrap(ShareSheet.present(["Workout"], title: "Workout", from: XCTUnwrap(anchor), at: rect, outcome: { _ in }) as? UIActivityViewController)
        defer { sheet.dismiss(animated: false) }
        until("the share sheet is presented over the app's tabs") { sheet.presentingViewController != nil && sheet.viewIfLoaded?.window != nil }
        XCTAssertNotNil(navigation.tabOwner?.view.window)
    }

    func testTheTabsHatchAContainerTheAppOwnsAndANativeScreen() throws {
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); NativeViews.uninstallTable() }
        let session = try fixture("tabs-owned", module: true)
        let navigation = session.presenter.navigation
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        // At a cold launch Exact's container comes first; once the module
        // connects, `tabs` runs on it and `tabContainer` takes its stacks.
        until("the app's container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        XCTAssertTrue(log().contains("hatch tabs: built"))
        XCTAssertTrue(log().contains("hatch tabContainer: the app's"))
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
