#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
import ExactGroupedLists

/// LLP 1075.003 §9.10 over the native fixture's Chat and Photo routes: a
/// pressable group around the heading (an avatar, a subtitle) is the item's
/// title, drawn and tapped; Chat, pushed while the authored tablist is
/// hidden, hides the tab bar; its inline title's scroller goes under the
/// bar; Photo, which says it has no bar, has none, and the bar comes back
/// with Chat. LLP 1115.000 D5.2 and D5.3 over
/// `scripts/fixtures/navigation-bars.contract`: the bar is each route's, a
/// pushed route with none of its own gets the system Back alone, and no bar
/// is something a route says.
/// UIKit synthesizes no touches for a unit test: the title is driven by its
/// control's action, an interactive pop by the `willShow` UIKit sends.
final class NavigationBasicsIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    func testNotificationClickSetsColdLaunchLocationOnlyOnce() {
        let session = ExactApp.shared.makeSession(label: "notification-\(UUID().uuidString)")
        sessions.append(session)
        let presenter = NotificationPresenter()
        presenter.app = ExactApp.shared
        let date = Date()
        let info: [AnyHashable: Any] = ["navigate": "/thread/42", "session": session.label,
                                        "owner": session.notificationOwnerID]
        presenter.deliver(id: "thread-42", date: date, info: info)
        XCTAssertEqual(session.launchLocation, "/thread/42")
        presenter.deliver(id: "thread-42", date: date, info: ["navigate": "/other", "session": session.label,
                                                                           "owner": session.notificationOwnerID])
        XCTAssertEqual(session.launchLocation, "/thread/42")
        XCTAssertTrue(session.pendingLaunchLocations.isEmpty, "a duplicate click must not queue another navigation")
        presenter.deliver(id: "thread-43", date: date, info: ["navigate": "/thread/43", "session": session.label,
                                                                           "owner": session.notificationOwnerID])
        XCTAssertEqual(session.launchLocation, "/thread/42")
        XCTAssertEqual(session.pendingLaunchLocations, ["/thread/43"])
    }

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

    /// A session over the native fixture's plan, or over the one `plan`
    /// names: `EXACT_NAVIGATION_BARS_PLAN` is `scripts/fixtures/navigation-bars.contract`.
    private func fixture(_ label: String, plan: String = "EXACT_FIXTURE_PLAN", launch: String? = nil) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[plan], "build.mjs --test --ios compiles the fixtures' plans")))
        ExactGroupedLists.install() // navigation-bars.contract's `rows` route is a grouped list
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        if let launch {
            session.protocolHandlers = [["protocol": "exacttest", "url": "/detail?u=%s"]]
            XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: launch))))
        }
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = window.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func tapNode(_ session: ExactSession, _ testId: String) throws {
        _ = Agent(session: session).tap(["id": Int(try node(session, testId).id)])
    }

    private func journal(_ session: ExactSession) -> String { session.agent(#"{"op":"logs","since":0}"#) }

    private func text(_ session: ExactSession, _ testId: String) -> String? {
        (try? node(session, testId))?.accessibleText
    }

    func testStationaryNavigationKeepsItsBarsOutsideThePageScroller() throws {
        let session = try fixture("stationary-navigation")
        let presenter = session.presenter
        let owner = try XCTUnwrap(presenter.navigation.primaryOwner?.view)
        var ancestor = owner.superview
        while let view = ancestor {
            XCTAssertFalse(view is UIScrollView, "Duo's native Back must not sit inside a page scroller")
            ancestor = view.superview
        }
        XCTAssertTrue(presenter.mounts(owner))
        XCTAssertFalse(presenter.viewport.isScrollEnabled)
    }

    func testNavigationDocumentsKeepScrollingWhenTheirContentGrows() throws {
        let session = try fixture("scrolling-navigation-document")
        let presenter = session.presenter
        let root = try XCTUnwrap(presenter.root.subviews.first)
        let original = root.frame
        root.frame.size.height = presenter.viewportSize.height + 300
        presenter.fitDocument()
        XCTAssertTrue(root.isDescendant(of: presenter.viewport))
        XCTAssertTrue(presenter.viewport.isScrollEnabled)
        let before = root.convert(CGPoint.zero, to: session.view)
        presenter.viewport.setContentOffset(CGPoint(x: 0, y: 100), animated: false)
        XCTAssertEqual(root.convert(CGPoint.zero, to: session.view).y, before.y - 100, accuracy: 0.5)
        presenter.viewport.setContentOffset(.zero, animated: false)
        root.frame = original
        presenter.fitDocument()
        XCTAssertFalse(root.isDescendant(of: presenter.viewport))
        XCTAssertTrue(presenter.mounts(root))
    }

    func testDeclaredProtocolCarriesColdAndWarmURLsThroughTheRouter() throws {
        let session = try fixture("protocol", plan: "EXACT_NAVIGATION_BARS_PLAN", launch: "exacttest://auth/start#cold")
        func state() -> String { session.agent(#"{"op":"state"}"#) }
        XCTAssertTrue(state().contains("/detail?u=exacttest%3A%2F%2Fauth%2Fstart%23cold"))
        for (path, fragment) in [("first", "one"), ("second", "two")] {
            XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: "exacttest://auth/\(path)#\(fragment)"))))
            XCTAssertTrue(state().contains("/detail?u=exacttest%3A%2F%2Fauth%2F\(path)%23\(fragment)"))
        }
        let batch = ["exacttest://auth/batch-first#one", "exacttest://auth/batch-second#two"].compactMap(URL.init(string:))
        var states: [String] = []
        IncomingURLBatch.deliver(batch, development: { _ in false }, document: { _ in }, location: { url in
            XCTAssertTrue(session.openURL(url))
            states.append(state())
        })
        XCTAssertEqual(states.count, 2)
        XCTAssertTrue(states[0].contains("batch-first%23one"))
        XCTAssertTrue(states[1].contains("batch-second%23two"))
        let long = "exacttest://auth/x?request=\(String(repeating: "x", count: 16_384))#frag"
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: long))))
        XCTAssertTrue(state().contains("request%3D\(String(repeating: "x", count: 16_384))%23frag"))
    }

    /// Home's stack with Chat pushed on it.
    private func chat(_ session: ExactSession) throws -> (UITabBarController, UINavigationController, RouteController) {
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        try tapNode(session, "chat")
        until("Chat pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        return (tabs, nav, try XCTUnwrap(nav.topViewController as? RouteController))
    }

    func testAPressableGroupAroundTheHeadingIsTheItemsTitleDrawnAndTapped() throws {
        let session = try fixture("basics-title")
        let (_, _, chat) = try chat(session)
        let item = chat.navigationItem
        XCTAssertEqual(item.title, "Chat", "the heading stays the title, which Back reads")
        let view = try XCTUnwrap(item.titleView as? HeaderTitleView, "an avatar and a press make a drawn title view")
        XCTAssertEqual(view.title.text, "Chat")
        XCTAssertEqual(view.subtitle.text, "Online")
        XCTAssertFalse(view.subtitle.isHidden)
        XCTAssertEqual(view.avatar.image?.size, CGSize(width: 32, height: 32), "the avatar at the size the author gave its box (headerBoxSize)")
        XCTAssertFalse(view.avatar.isHidden)
        // Dark mode keeps the box: the asset's dark variant once drew at 3x.
        view.window?.overrideUserInterfaceStyle = .dark
        view.layoutIfNeeded()
        XCTAssertEqual(view.avatar.bounds.size, CGSize(width: 32, height: 32), "the avatar's box in dark mode")
        view.window?.overrideUserInterfaceStyle = .unspecified
        XCTAssertEqual(view.accessibilityIdentifier, "title-group")
        XCTAssertTrue(view.accessibilityTraits.contains(.button))
        XCTAssertEqual(view.accessibilityValue, "Online")
        if #available(iOS 26.0, *) { XCTAssertNil(item.subtitle, "the drawn view holds the subtitle") }
        XCTAssertTrue(try node(session, "header-chat").isHidden, "the header itself is not painted")
        // A tap on the title presses the group, as a tap on it would.
        XCTAssertEqual(text(session, "title-presses"), "title presses 0")
        // (The registered action, sent as a touch up sends it: the test
        // host does not deliver `sendActions`, §9.8.)
        for target in view.allTargets {
            for action in view.actions(forTarget: target, forControlEvent: .touchUpInside) ?? [] {
                _ = (target as NSObject).perform(Selector(action))
            }
        }
        until("the title pressed its group") { self.text(session, "title-presses") == "title presses 1" }
        // The subtitle follows the Contract, in the same view.
        try tapNode(session, "toggle-subtitle")
        until("the subtitle left") { view.subtitle.isHidden }
        XCTAssertNil(view.accessibilityValue)
        XCTAssertTrue(item.titleView === view)
        // A subtitle that is a line of symbols and texts draws them inline.
        try tapNode(session, "toggle-muted")
        until("the glyph line") { view.accessibilityValue == "Muted, disappearing messages after 1 week" }
        let line = try XCTUnwrap(view.subtitle.attributedText)
        var attachments: [NSTextAttachment] = []
        line.enumerateAttribute(.attachment, in: NSRange(location: 0, length: line.length)) { value, _, _ in
            if let a = value as? NSTextAttachment { attachments.append(a) }
        }
        XCTAssertEqual(attachments.count, 2, "bell.slash and timer, inline")
        // Template images, tinted by the run's dynamic colour: they follow
        // the appearance with no rebuild.
        XCTAssertEqual(attachments.map { $0.image?.renderingMode }, [.alwaysTemplate, .alwaysTemplate])
        XCTAssertEqual(line.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor, view.subtitle.textColor)
        XCTAssertTrue(line.string.contains("Muted") && line.string.contains("1w"))
        try tapNode(session, "toggle-muted")
        until("back to no subtitle (it was toggled off)") { view.accessibilityValue == nil && view.subtitle.isHidden }
        // A title view a hatch sets stays: Exact draws only its own.
        let hatches = UILabel()
        item.titleView = hatches
        try tapNode(session, "toggle-subtitle")
        until("the subtitle came back") { !view.subtitle.isHidden }
        XCTAssertTrue(item.titleView === hatches, "the hatch's title view is left alone")
    }

    func testARIAUnavailablePressableTitleKeepsItsAction() throws {
        let session = try fixture("aria-title")
        let (_, _, chat) = try chat(session)
        let view = try XCTUnwrap(chat.navigationItem.titleView as? HeaderTitleView)
        let group = try node(session, "title-group")
        let header = try node(session, "header-chat")
        let heading = try node(session, "title-chat")
        group.props["accessibilityDisabled"] = "true"
        view.update(try XCTUnwrap(HeaderTitle(header: header, heading: heading, tap: group, apart: [])), title: "Chat")
        XCTAssertTrue(view.isEnabled, "ARIA does not disable the title action")
        XCTAssertTrue(view.accessibilityTraits.contains(.notEnabled))
        group.props["accessibilityDisabled"] = "false"
        view.update(try XCTUnwrap(HeaderTitle(header: header, heading: heading, tap: group, apart: [])), title: "Chat")
        XCTAssertFalse(view.accessibilityTraits.contains(.notEnabled))
    }

    /// LLP 1116 D2 (amending LLP 1075.003 §9.10): a heading the header holds
    /// directly takes the first text after it as its subtitle, UIKit's own
    /// on iOS 26 (a drawn title view before). A second text is not the
    /// bar's: the agent marks it in its tree and screenshot and refuses a
    /// tap on it, by the rule the bar follows (`HeaderShape.presents`).
    func testAHeadingTheHeaderHoldsDirectlyTakesTheFirstTextAfterItAsItsSubtitle() throws {
        let session = try fixture("basics-direct-subtitle")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        try tapNode(session, "open-plain")
        until("Plain pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        let plain = try XCTUnwrap(nav.topViewController as? RouteController)
        let item = plain.navigationItem
        XCTAssertEqual(item.title, "Plain")
        if #available(iOS 26.0, *) {
            XCTAssertEqual(item.subtitle, "No Back control", "UIKit's own subtitle")
            XCTAssertNil(item.titleView, "nothing drawn: no avatar, no press")
        } else {
            let view = try XCTUnwrap(item.titleView as? HeaderTitleView, "a subtitle before iOS 26 is a drawn title view")
            XCTAssertEqual(view.subtitle.text, "No Back control")
        }
        XCTAssertTrue(try node(session, "header-plain").isHidden, "the bar shows the header; iOS does not paint it")
        // What the bar leaves out, as the agent reports it.
        let unshown = session.presenter.navigation.unshownByBar()
        let aside = try node(session, "aside-plain")
        XCTAssertEqual(unshown[aside.id], NavigationHost.unshownReason)
        for shown in ["header-plain", "title-plain", "subtitle-plain", "compose-plain"] {
            XCTAssertNil(unshown[try node(session, shown).id], "\(shown) is the bar's")
        }
        let agent = Agent(session: session)
        let tap = agent.tap(["id": Int(aside.id)])
        XCTAssertTrue((tap["error"] as? String)?.contains("the iOS navigation bar does not show this node") == true, "\(tap)")
        let rows = agent.decorateTree(["nodes": [["id": Int(aside.id)], ["id": Int(try node(session, "subtitle-plain").id)]]])["nodes"] as? [[String: Any]]
        XCTAssertEqual(rows?.first?["unshown"] as? String, NavigationHost.unshownReason)
        XCTAssertNil(rows?.last?["unshown"])
        // A heading in a wrapper with a bar item (Chat's press group aside)
        // and a box before a direct heading are not avatars: Home's badge
        // button is an item, and its title stays UIKit's.
        nav.popViewController(animated: false)
        spin(0.3)
        let home = try XCTUnwrap(nav.topViewController)
        XCTAssertFalse(home.navigationItem.titleView is HeaderTitleView, "Home's segments keep the title view")
        if #available(iOS 26.0, *) { XCTAssertNil(home.navigationItem.subtitle, "Home has no text after its heading") }
    }

    /// LLP 1075.003 §9.15: a readout (`role="status"`) after the header's
    /// buttons is a trailing item with a custom view, as a hand-built bar's
    /// "● Clock" is: its dot and monospaced text drawn as written, read by
    /// VoiceOver as static text, the bar's and not the page's; it follows
    /// the Contract in place, the same item and view.
    func testAReadoutIsATrailingItemWhoseViewFollowsItInPlace() throws {
        let session = try fixture("basics-readout")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        try tapNode(session, "open-plain")
        until("Plain pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        let plain = try XCTUnwrap(nav.topViewController as? RouteController)
        let right = plain.navigationItem.rightBarButtonItems ?? []
        let item = try XCTUnwrap(right.first, "the readout is after Compose: the rightmost item")
        XCTAssertEqual(item.accessibilityIdentifier, "readout-plain")
        XCTAssertTrue(right.contains { $0.accessibilityLabel == "Compose" }, "Compose is still an item")
        let view = try XCTUnwrap(item.customView as? ReadoutView, "a custom view, not a title or an image")
        XCTAssertNil(item.action, "nothing presses a readout")
        func labels() -> [UILabel] { view.subviews.flatMap(\.subviews).compactMap { $0 as? UILabel } }
        let label = try XCTUnwrap(labels().first)
        XCTAssertEqual(label.text, "0 sent")
        XCTAssertEqual(label.font.pointSize, 12)
        XCTAssertTrue(label.font.fontName.lowercased().contains("menlo"), "the author's font: \(label.font.fontName)")
        let dot = try XCTUnwrap(view.subviews.flatMap(\.subviews).first { !($0 is UILabel) })
        view.layoutIfNeeded()
        XCTAssertEqual(dot.bounds.size, CGSize(width: 7, height: 7), "the dot at its authored size")
        XCTAssertGreaterThan(view.intrinsicContentSize.width, 7 + 6)
        XCTAssertTrue(view.isAccessibilityElement)
        XCTAssertTrue(view.accessibilityTraits.contains(.staticText))
        XCTAssertEqual(view.accessibilityLabel, "0 composed", "its `aria-label`")
        // The bar's, not the page's: the agent does not call it unshown.
        let unshown = session.presenter.navigation.unshownByBar()
        for shown in ["readout-plain", "readout-text"] { XCTAssertNil(unshown[try node(session, shown).id], "\(shown) is the bar's") }
        // Nor the subtitle's: the first text after the heading still is.
        if #available(iOS 26.0, *) { XCTAssertEqual(plain.navigationItem.subtitle, "No Back control") }
        // The agent's tap on it is a person's: it lands, and nothing is pressed.
        let tap = Agent(session: session).tap(["id": Int(try node(session, "readout-text").id)])
        XCTAssertNil(tap["error"], "\(tap)")
        XCTAssertEqual(tap["native"] as? String, "bar-readout")
        try tapNode(session, "compose-plain")
        until("the readout follows the count") { labels().first?.text == "1 sent" }
        XCTAssertTrue(plain.navigationItem.rightBarButtonItems?.first === item, "the same item")
        XCTAssertTrue(item.customView === view, "the same view")
        XCTAssertEqual(view.accessibilityLabel, "1 composed")
        // `aria-hidden` on the tabpanel, which UIKit holds the route apart
        // from, takes the readout off the accessibility tree.
        let panel = try node(session, "panel-home")
        session.presenter.apply(wireBatch([["op": "props", "id": Int(panel.id), "set": ["accessibilityElementsHidden": "true"], "clear": []]]))
        until("hidden by its logical ancestor") { !view.isAccessibilityElement }
        session.presenter.apply(wireBatch([["op": "props", "id": Int(panel.id), "set": [:], "clear": ["accessibilityElementsHidden"]]]))
        until("shown again") { view.isAccessibilityElement }
        // A sibling button kept authored lifts the header only in part; the
        // agent's tap on the readout still lands on the bar's.
        let compose = try node(session, "compose-plain")
        session.presenter.apply(wireBatch([["op": "style", "id": Int(compose.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        spin(0.2)
        XCTAssertTrue(plain.navigationItem.rightBarButtonItems?.contains { $0.customView === view } == true, "still the bar's")
        let partial = Agent(session: session).tap(["id": Int(try node(session, "readout-text").id)])
        XCTAssertEqual(partial["native"] as? String, "bar-readout", "\(partial)")
    }

    /// Whether the tab bar insets a tab's content from below: a phone's (and
    /// a compact width's) does; iPadOS's floating bar, at the top in a
    /// regular width, insets nothing, so there the bar's own state is read.
    private func barInsets(_ tabs: UITabBarController) -> Bool { tabs.tabBar.window != nil }
    /// Whether the tab bar shows on screen: the bar, or an iPad's floating
    /// list of its items, in the window, unhidden, opaque enough, on screen.
    private func barShows(_ tabs: UITabBarController) -> Bool {
        guard let bar: UIView = barInsets(tabs) ? tabs.tabBar : tabs.floatingTabList, let window = bar.window else { return false }
        var at: UIView? = bar
        while let v = at { if v.isHidden || v.alpha < 0.01 { return false }; at = v.superview }
        let box = bar.convert(bar.bounds, to: window)
        return box.height > 1 && box.intersects(window.bounds)
    }

    func testARoutePushedWhileTheTablistIsHiddenHidesTheTabBar() throws {
        let session = try fixture("basics-tabbar")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let home = try XCTUnwrap((tabs.selectedViewController as? UINavigationController)?.topViewController)
        let insets = barInsets(tabs)
        if !insets { XCTAssertNotNil(tabs.floatingTabList, "no UITabBar in the window: an iPad's floating bar") }
        XCTAssertTrue(barShows(tabs), "the tab bar shows over Home")
        let shown = home.view.safeAreaInsets.bottom
        if insets { XCTAssertGreaterThan(shown, 40, "the tab bar insets the root") }
        try tapNode(session, "detail")
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        until("Detail pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController)
        XCTAssertFalse(detail.hidesBottomBarWhenPushed, "the tablist shows over Detail")
        if insets { XCTAssertEqual(detail.view.safeAreaInsets.bottom, shown, accuracy: 0.5) }
        XCTAssertTrue(barShows(tabs), "and over Detail")
        nav.popViewController(animated: true)
        until("back to Home") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        let (_, _, chat) = try chat(session)
        XCTAssertTrue(chat.hidesBottomBarWhenPushed, "the authored tablist is hidden over Chat")
        // (On an iPad the floating bar, which does not take
        // `hidesBottomBarWhenPushed`, is hidden by `setTabBarHidden`.)
        until("the tab bar left") { !barShows(tabs) && (!insets || chat.view.safeAreaInsets.bottom < shown - 20) }
        // Another tab, its root showing the tablist, and back to Chat (selected by the app, as a link would).
        tapTab(tabs, 1)
        until("Second shows the bar") { tabs.selectedIndex == 1 && barShows(tabs) }
        tapTab(tabs, 0)
        until("back on Chat, the bar hidden again") { tabs.selectedIndex == 0 && nav.topViewController === chat && !barShows(tabs) }
        nav.popViewController(animated: true)
        until("back to Home") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        until("the tab bar came back") { barShows(tabs) && (!insets || abs(home.view.safeAreaInsets.bottom - shown) < 0.5) }
    }

    /// LLP 1075.003 §3.7, amended 2026-10-05: the root's tablist hidden
    /// hides the bar too, with UIKit's animated `setTabBarHidden`, and shown
    /// brings it back (Signal's chat-list multi-select). Driven through the
    /// fixture's own batches: a hide and a show at the root, a pop back to a
    /// root still hidden (UIKit brings the bar back for its root; Exact
    /// hides it again once the pop settles), another tab's root with its
    /// tablist shown, and a hatch's own value standing between those moments.
    /// A tap on the bar's item for a tab, as UIKit asks before selecting.
    private func tapTab(_ tabs: UITabBarController, _ index: Int) {
        let target = tabs.viewControllers![index]
        if tabs.delegate?.tabBarController?(tabs, shouldSelect: target) ?? true { tabs.selectedIndex = index }
    }

    func testTheRootsTablistHiddenHidesTheTabBarAndShownBringsItBack() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("setTabBarHidden is iOS 18") }
        let session = try fixture("basics-roottabbar")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let home = try XCTUnwrap(nav.topViewController as? RouteController)
        let insets = barInsets(tabs)
        let shown = home.view.safeAreaInsets.bottom
        if insets { XCTAssertGreaterThan(shown, 40, "the tab bar insets the root") }
        let settled = { nav.transitionCoordinator == nil }
        // Hidden over the root: the bar goes.
        try tapNode(session, "toggle-choosing")
        until("the bar left") { tabs.isTabBarHidden && (!insets || home.view.safeAreaInsets.bottom < shown - 20) }
        XCTAssertFalse(home.hidesBottomBarWhenPushed, "the root is never pushed: no flag")
        // Shown again: it comes back.
        try tapNode(session, "toggle-choosing")
        until("the bar came back") { !tabs.isTabBarHidden && (!insets || abs(home.view.safeAreaInsets.bottom - shown) < 0.5) }
        // A hatch's hide, with the tablist shown and unchanged, stands; and
        // the tablist hidden and shown again over it leaves it standing too:
        // Exact did not make that hide.
        tabs.setTabBarHidden(true, animated: false)
        try tapNode(session, "bump")
        spin(0.3)
        XCTAssertTrue(tabs.isTabBarHidden, "an unchanged tablist writes nothing")
        try tapNode(session, "toggle-choosing")
        spin(0.3)
        try tapNode(session, "toggle-choosing")
        spin(0.3)
        XCTAssertTrue(tabs.isTabBarHidden, "the hatch's hide stands")
        tabs.setTabBarHidden(false, animated: false)
        // Hidden at the root, Detail pushed (its tablist shown: the fixture
        // hides it at Home only), then a pop back: UIKit brings the bar back
        // for its root; settled there, Exact hides it again.
        try tapNode(session, "toggle-choosing")
        until("the bar left") { tabs.isTabBarHidden }
        try tapNode(session, "detail")
        until("Detail pushed") { nav.viewControllers.count == 2 && settled() }
        until("Detail's tablist shows: so does the bar") { !tabs.isTabBarHidden }
        nav.popViewController(animated: true)
        until("back to Home (1)") { nav.viewControllers.count == 1 && settled() }
        until("hidden again over the root") { tabs.isTabBarHidden }
        // A hatch's show between those moments stands.
        tabs.setTabBarHidden(false, animated: false)
        try tapNode(session, "bump")
        spin(0.3)
        XCTAssertFalse(tabs.isTabBarHidden, "an unchanged tablist writes nothing")
        tabs.setTabBarHidden(true, animated: false)
        // Another tab's root, its tablist shown: the bar Exact hid comes back.
        tapTab(tabs, 1)
        until("the second tab's root shows the bar") { !tabs.isTabBarHidden }
        tapTab(tabs, 0)
        until("Home, still hidden, hides it") { tabs.isTabBarHidden }
        try tapNode(session, "toggle-choosing")
        until("shown") { !tabs.isTabBarHidden }
        // A pushed route's tablist hidden in place hides the bar
        // (`setTabBarHidden`); a pop to a root whose tablist shows (the tab
        // reselected) brings it back (a pop alone does not clear a stored
        // hide).
        let list = try node(session, "tabs")
        try tapNode(session, "detail")
        until("Detail pushed") { nav.viewControllers.count == 2 && settled() }
        list.style["display"] = .string("none")
        session.presenter.navigation.followTablist(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        until("hidden over Detail") { tabs.isTabBarHidden }
        list.style["display"] = .string("flex")
        // Reselecting the tab pops it to its root: Exact's own pop.
        tapTab(tabs, 0)
        until("back to Home (2)") { nav.viewControllers.count == 1 && settled() }
        until("the bar is back over Home") { !tabs.isTabBarHidden }
        if insets { until("Home inset by it again") { abs(home.view.safeAreaInsets.bottom - shown) < 0.5 } }
        // The pushed-route rule stands.
        let (_, _, chat) = try chat(session)
        XCTAssertTrue(chat.hidesBottomBarWhenPushed, "the authored tablist is hidden over Chat")
    }

    func testTheNavigationFixtureReachesTheScreensBottomEdge() throws {
        _ = try fixture("basics-screen")
        let window = try XCTUnwrap(window)
        XCTAssertEqual(window.frame, window.screen.bounds)
        XCTAssertGreaterThan(window.safeAreaInsets.bottom, 0)
    }

    func testAnInlineTitlesScrollerGoesUnderTheBar() throws {
        let session = try fixture("basics-under")
        let (_, nav, chat) = try chat(session)
        let list = try node(session, "list-chat")
        let sv = try XCTUnwrap(list.scroll)
        XCTAssertTrue(journal(session).contains("scrolls its content under the bar"))
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .automatic, "UIKit insets it")
        XCTAssertEqual(list.convert(list.bounds, to: chat.view).minY, 0, accuracy: 0.5, "it starts under the bar")
        XCTAssertEqual(sv.adjustedContentInset.top, chat.view.safeAreaInsets.top, accuracy: 0.5, "inset to the bar's bottom")
        XCTAssertFalse(nav.isNavigationBarHidden)
        // Its route's last child, it runs to the screen's bottom edge too, and
        // UIKit insets it by the home indicator (LLP 1116 D2): no band of the
        // route's background under it.
        XCTAssertTrue(journal(session).contains("scrolls its content to the bottom edge"))
        XCTAssertGreaterThan(chat.view.safeAreaInsets.bottom, 0, "a home indicator to go under")
        XCTAssertEqual(list.convert(list.bounds, to: chat.view).maxY, chat.view.bounds.maxY, accuracy: 0.5, "it ends at the screen's edge")
        XCTAssertEqual(sv.adjustedContentInset.bottom, chat.view.safeAreaInsets.bottom, accuracy: 0.5, "inset by the home indicator")
        XCTAssertTrue(chat.contentScrollView(for: .bottom) === sv, "the bottom edge follows it")
        let css = { sv.contentOffset.y + sv.adjustedContentInset.top }
        XCTAssertEqual(css(), 0, accuracy: 0.5, "at rest, CSS 0")
        list.pendingScrollTop = 80
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertEqual(css(), 80, accuracy: 0.5, "an authored offset lands as the browser's")
    }

    /// The bottom edge (LLP 1116 D2): Detail's scroller has the "Open in"
    /// row under it, which keeps its place above the tab bar, the scroller
    /// no inset of UIKit's.
    func testAFooterUnderTheScrollerKeepsTheBottomCover() throws {
        let session = try fixture("basics-footer")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        try tapNode(session, "detail")
        until("Detail pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        let detail = try XCTUnwrap(nav.topViewController as? RouteController)
        let list = try node(session, "list-detail"), footer = try node(session, "open-above-card")
        let sv = try XCTUnwrap(list.scroll)
        if #available(iOS 18.0, *) { XCTAssertFalse(tabs.isTabBarHidden, "Detail shows the tab bar") }
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(sv.adjustedContentInset.bottom, 0)
        XCTAssertEqual(footer.convert(footer.bounds, to: detail.view).maxY, detail.view.bounds.maxY - detail.view.safeAreaInsets.bottom,
                       accuracy: 0.5, "the footer stays above the tab bar")
        XCTAssertNil(detail.contentScrollView(for: .bottom))
    }

    /// The bottom edge's scroller may sit deeper than the route's last child
    /// (LLP 1116 D2, amended): a sheet whose component's root column holds
    /// its header and then its scroller reaches the edge, as a hand-built
    /// sheet's scroll view pinned to its view's bottom does. A footer, a
    /// padding under it, or a sideways scroller does not.
    func testANestedLastScrollerReachesTheBottomEdge() {
        let p = Presenter()
        var next: UInt32 = 0
        func box(_ parent: NodeView?, y: CGFloat, height: CGFloat, overflow: String? = nil) -> NodeView {
            next += 1
            let n = NodeView(id: next, kind: "view", presenter: p)
            n.frame = CGRect(x: 0, y: y, width: 400, height: height)
            if let overflow { n.style[overflow == "x" ? "overflow_x" : "overflow_y"] = .string("scroll"); n.syncScroll() }
            parent?.container.addSubview(n)
            return n
        }
        // The route's cover pads 34 under its child, which stands at the foot
        // of the route's content box less that cover: the box the kernel
        // sends is the authored one, the cover said apart (`cover:`).
        let route = box(nil, y: 0, height: 600)
        let sheet = box(route, y: 0, height: 566)
        _ = box(sheet, y: 0, height: 60)
        let scroller = box(sheet, y: 60, height: 506, overflow: "y")
        XCTAssertTrue(NavigationHost.bottomScroller(in: route, cover: 34) === scroller, "the component's scroller reaches the edge")
        XCTAssertNil(NavigationHost.bottomScroller(in: route), "not with the cover unsaid: it ends 34 above the content box")
        sheet.applyKernelContentBox(CGRect(x: 0, y: 0, width: 400, height: 546), padding: [0, 0, 20, 0])
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a padding under it keeps the cover")
        sheet.applyKernelContentBox(nil, padding: nil)
        scroller.frame.size.height = 446
        let footer = box(sheet, y: 506, height: 60)
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a footer under it keeps the cover")
        footer.removeFromSuperview()
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "nor does a scroller that ends short of its parent")
        scroller.frame.size.height = 506
        scroller.style["overflow_y"] = nil
        scroller.style["overflow_x"] = .string("scroll")
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a sideways scroller takes no bottom inset")
        scroller.style["overflow_x"] = nil
        scroller.style["overflow_y"] = .string("scroll")
        // The route's child sized to its content, 200 points of a 600-point
        // route: its scroller ends far above the home indicator.
        sheet.frame.size.height = 200
        scroller.frame.size.height = 140
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a route's child short of its content box keeps the cover")
        sheet.frame.size.height = 566
        scroller.frame.size.height = 506
        XCTAssertTrue(NavigationHost.bottomScroller(in: route, cover: 34) === scroller)
        // The component's column ends in a row filling it: a side column
        // beside the scroller reaches the edge too.
        scroller.removeFromSuperview()
        let split = box(sheet, y: 60, height: 506)
        _ = box(split, y: 0, height: 506)
        let right = box(split, y: 0, height: 506, overflow: "y")
        right.frame.origin.x = 200
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a row's scroller beside a column keeps the cover")
        // A full-height row: a column with Save at its foot, and a scroller
        // beside it. The Save button reaches the edge too: the cover stays.
        sheet.removeFromSuperview()
        let row = box(route, y: 0, height: 566)
        let side = box(row, y: 0, height: 566)
        _ = box(side, y: 522, height: 44)
        let beside = box(row, y: 0, height: 566, overflow: "y")
        beside.frame.origin.x = 200
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a scroller with a sibling beside it in a row keeps the cover")
        side.removeFromSuperview()
        XCTAssertTrue(NavigationHost.bottomScroller(in: route, cover: 34) === beside, "the row's only child spans its bottom edge")
        // A reversed column: its last child is at its top, above the scroller.
        _ = box(row, y: 0, height: 60)
        beside.frame.origin = CGPoint(x: 0, y: 60); beside.frame.size.height = 506
        XCTAssertNil(NavigationHost.bottomScroller(in: route, cover: 34), "a last child above its sibling is not at the edge")
    }

    /// A route's closed menus and confirmations take no place in its flow, as
    /// a browser lays out neither: one written after its scroller leaves the
    /// scroller at the bottom edge (euxy's Patterns tab, clipped at the tab
    /// bar by its sort menu and its "Restore factory presets?" dialog).
    func testTopLayerBoxesAfterAScrollerLeaveItAtTheBottomEdge() {
        let p = Presenter()
        var next: UInt32 = 0
        func box(_ parent: NodeView?, y: CGFloat, height: CGFloat, props: [String: String] = [:], scrolls: Bool = false) -> NodeView {
            next += 1
            let n = NodeView(id: next, kind: "view", presenter: p)
            n.props = props
            n.frame = CGRect(x: 0, y: y, width: 400, height: height)
            if scrolls { n.style["overflow_y"] = .string("scroll"); n.syncScroll() }
            parent?.container.addSubview(n)
            return n
        }
        let route = box(nil, y: 0, height: 600)
        _ = box(route, y: 0, height: 100)
        // Closed, a top-layer box is covered whole: laid out at no size.
        _ = box(route, y: 0, height: 0, props: ["popover": "auto"])
        let scroller = box(route, y: 100, height: 500, scrolls: true)
        let menu = box(route, y: 0, height: 0, props: ["popover": "auto"])
        let dialog = box(route, y: 0, height: 0, props: ["semanticTag": "dialog"])
        XCTAssertTrue(NavigationHost.bottomScroller(in: route) === scroller)
        XCTAssertFalse(NavigationHost.inFlow(menu))
        XCTAssertFalse(NavigationHost.inFlow(dialog))
        _ = box(route, y: 0, height: 0)
        XCTAssertNil(NavigationHost.bottomScroller(in: route), "a box in the flow after it still keeps the cover")
    }

    /// A route that paints no background shows what the web shows through
    /// it — the root's (LLP 1116 D2: `-exact-grouped-background` on the root
    /// under every screen, and under the home indicator) — light and dark,
    /// written again when a batch changes it; its own background wins.
    func testARouteWithoutABackgroundShowsTheRootsUnderIt() throws {
        let session = try fixture("basics-backdrop")
        let (_, _, chat) = try chat(session)
        let root = try node(session, "navigation")
        let rgb = { (style: UIUserInterfaceStyle) -> [Int] in
            let c = chat.view.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: style)) ?? .clear
            var v: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
            c.getRed(&v.0, green: &v.1, blue: &v.2, alpha: &v.3)
            return [v.0, v.1, v.2, v.3].map { Int(($0 * 255).rounded()) }
        }
        let colour = { (v: [Double]) in BatchValue.array(v.map { .number($0) }) }
        XCTAssertEqual(rgb(.light), [255, 255, 255, 255], "its own white")
        chat.node.style["background_color"] = nil
        root.style["background_color"] = .array([colour([242, 242, 247, 255]), colour([28, 28, 30, 255])])
        chat.paintBackdrop()
        XCTAssertEqual(rgb(.light), [242, 242, 247, 255], "the root's, light")
        XCTAssertEqual(rgb(.dark), [28, 28, 30, 255], "the root's, dark")
        XCTAssertEqual(chat.view.convert(chat.view.bounds, to: nil).maxY, try XCTUnwrap(chat.view.window).bounds.maxY,
                       "under the home indicator too")
        chat.node.style["background_color"] = colour([0, 0, 255, 255])
        chat.paintBackdrop()
        XCTAssertEqual(rgb(.light), [0, 0, 255, 255], "the route's own wins")
    }

    /// LLP 1115.000 D5.2: no bar is something a route says. Photo, a
    /// full-bleed viewer that draws its own Close, is `navigationBar="none"`.
    func testARouteThatAsksForNoBarHasNoneAndTheBarFollowsAPop() throws {
        let session = try fixture("basics-nobar")
        let (_, nav, chat) = try chat(session)
        let chatTop = chat.view.safeAreaInsets.top
        try tapNode(session, "photo")
        until("Photo pushed") { nav.viewControllers.count == 3 && nav.transitionCoordinator == nil }
        spin(0.3)
        let photo = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertEqual(photo.node.props["navigationBar"], "none")
        XCTAssertTrue(nav.isNavigationBarHidden, "a route that says it has no bar has none")
        XCTAssertLessThan(photo.view.safeAreaInsets.top, chatTop, "only the status bar is left above it")
        let view = try node(session, "photo-view")
        XCTAssertEqual(view.convert(view.bounds, to: photo.view).minY, photo.view.safeAreaInsets.top, accuracy: 0.5,
                       "its content starts below the status bar, the page not covering it")
        XCTAssertFalse(journal(session).contains("navigation bar visibility changed outside Exact"))
        // With no bar the edge swipe still pops it.
        let swipe = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(session.presenter.navigation.popMayBegin(swipe, from: CGPoint(x: 4, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20)))
        // An interactive pop's start, and its cancel: UIKit asks for the
        // destination, then for the source again.
        let proxy = try XCTUnwrap(nav.delegate)
        proxy.navigationController?(nav, willShow: chat, animated: false)
        XCTAssertFalse(nav.isNavigationBarHidden, "the bar comes in with Chat")
        proxy.navigationController?(nav, willShow: photo, animated: false)
        XCTAssertTrue(nav.isNavigationBarHidden, "and leaves again when the pop is cancelled")
        // Its own Back control pops it; Chat's bar and content area are as before.
        try tapNode(session, "photo-back")
        until("back to Chat") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertEqual(chat.view.safeAreaInsets.top, chatTop, accuracy: 0.5)
        XCTAssertTrue(chat.navigationItem.titleView is HeaderTitleView)
    }

    /// LLP 1115.000 D5.3, over `scripts/fixtures/navigation-bars.contract`:
    /// the bar is each route's, never decided once for its stack. A stack
    /// that opens on a route with no header has no bar there, shows the bar
    /// for a pushed route whose header is one, and hides it again when that
    /// route is popped.
    func testAStackWhoseFirstRouteHasNoHeaderShowsTheBarForALaterRouteThatHasOne() throws {
        let session = try fixture("bars-per-route", plan: "EXACT_NAVIGATION_BARS_PLAN")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let start = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertTrue(nav.isNavigationBarHidden, "the stack's first route has no header: no bar")
        let bare = start.view.safeAreaInsets.top
        let content = try node(session, "content-start")
        XCTAssertEqual(content.convert(content.bounds, to: start.view).minY, bare, accuracy: 0.5, "its content starts below the status bar")
        try tapNode(session, "start-open-detail")
        until("Detail pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertFalse(nav.isNavigationBarHidden, "a later route's header is the bar, whatever the first route had")
        XCTAssertEqual(detail.navigationItem.title, "Detail")
        XCTAssertFalse(detail.navigationItem.hidesBackButton, "with UIKit's Back button")
        XCTAssertTrue(try node(session, "header-detail").isHidden, "the bar shows the header; iOS does not paint it")
        until("Detail's content starts below the bar") { detail.view.safeAreaInsets.top > bare + 20 }
        // UIKit's Back button pops it: the bar leaves with Detail.
        nav.popViewController(animated: true)
        until("back to Start") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        until("no bar on the first route again") { nav.isNavigationBarHidden && abs(start.view.safeAreaInsets.top - bare) < 0.5 }
        until("its content below the status bar, as before") { abs(content.convert(content.bounds, to: start.view).minY - bare) < 0.5 }
    }

    /// The navigation-bars fixture's one stack, at its first route.
    private func bars(_ label: String) throws -> (ExactSession, UINavigationController, RouteController) {
        let session = try fixture(label, plan: "EXACT_NAVIGATION_BARS_PLAN")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        return (session, nav, try XCTUnwrap(nav.topViewController as? RouteController))
    }

    /// `from`'s button to `name`, pressed: the route now on top of `nav`,
    /// `depth` deep, once UIKit's push has ended.
    private func push(_ session: ExactSession, _ nav: UINavigationController, from: String, _ name: String, depth: Int) throws -> RouteController {
        let tap = Agent(session: session).tap(["id": Int(try node(session, "\(from)-open-\(name)").id)])
        XCTAssertNil(tap["error"], "\(tap)")
        until("\(name) pushed") { nav.viewControllers.count == depth && nav.transitionCoordinator == nil }
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertEqual(route.node.props["testId"], "route-\(name)")
        return route
    }

    /// LLP 1115.000 D5.2: every pushed route with no bar of its own gets a
    /// bar that holds the system Back alone, with no title; the button's
    /// title is the route beneath's, as UIKit reads it. A header that is no
    /// bar's (two headings) still paints in the page as written, under it.
    func testAPushedRouteWithNoBarOfItsOwnGetsTheSystemBackAlone() throws {
        let (session, nav, start) = try bars("bars-back-alone")
        let bare = start.view.safeAreaInsets.top
        // No header at all.
        let plain = try push(session, nav, from: "start", "plain", depth: 2)
        XCTAssertFalse(nav.isNavigationBarHidden, "a pushed route with no header still has the bar")
        let item = plain.navigationItem
        XCTAssertNil(item.title, "no title")
        XCTAssertNil(item.titleView)
        XCTAssertFalse(item.hidesBackButton, "the system Back")
        XCTAssertTrue((item.leftBarButtonItems ?? []).isEmpty && (item.rightBarButtonItems ?? []).isEmpty, "alone")
        XCTAssertEqual(item.largeTitleDisplayMode, .never)
        XCTAssertNil(start.navigationItem.backButtonTitle, "Back's title is UIKit's, read from the route beneath")
        XCTAssertEqual(start.navigationItem.backButtonDisplayMode, .default)
        until("Plain's content starts below the bar") { plain.view.safeAreaInsets.top > bare + 20 }
        let content = try node(session, "content-plain")
        until("the page is laid out under the bar") { abs(content.convert(content.bounds, to: plain.view).minY - plain.view.safeAreaInsets.top) < 0.5 }
        // A header of two headings is no bar's: Back alone, and the header in the page.
        let twin = try push(session, nav, from: "plain", "twin", depth: 3)
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertNil(twin.navigationItem.title, "two headings give the bar no title")
        XCTAssertFalse(twin.navigationItem.hidesBackButton)
        XCTAssertTrue((twin.navigationItem.leftBarButtonItems ?? []).isEmpty && (twin.navigationItem.rightBarButtonItems ?? []).isEmpty)
        let header = try node(session, "header-twin")
        XCTAssertFalse(header.isHidden, "its header paints in the page as the author wrote it")
        XCTAssertNil(twin.lifted)
        until("the header sits under the bar") { abs(header.convert(header.bounds, to: twin.view).minY - twin.view.safeAreaInsets.top) < 0.5 }
        XCTAssertGreaterThan(header.bounds.height, 0)
        // UIKit's Back button pops each, by the root's `navigate`: neither names a control.
        nav.popViewController(animated: true)
        until("back to Plain") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil && nav.topViewController === plain }
        XCTAssertFalse(nav.isNavigationBarHidden, "Plain's bar, as it was")
        nav.popViewController(animated: true)
        until("back to Start") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        until("the first route, with nothing to go back to, has no bar") { nav.isNavigationBarHidden }
        XCTAssertEqual(text(session, "start-backs"), "backs 0", "no Back control was pressed: none is declared")
    }

    /// LLP 1115.000 D5.2: no bar is something the author says, not
    /// something inferred. Own draws its own Back control and says nothing:
    /// it gets the bar, whose Back presses that control (LLP 1115 D5).
    /// Viewer draws one too and says `navigationBar="none"`: no bar, over a
    /// route with a bar or without, and the edge swipe and its own control
    /// still pop it.
    func testNoBarIsWhatARouteSaysNotAnAuthoredBackControl() throws {
        let (session, nav, start) = try bars("bars-said")
        let own = try push(session, nav, from: "start", "own", depth: 2)
        XCTAssertFalse(nav.isNavigationBarHidden, "a drawn Back control does not take the bar away")
        XCTAssertFalse(own.navigationItem.hidesBackButton)
        XCTAssertFalse(try node(session, "own-back").isHidden, "the authored control is drawn too, in the page")
        XCTAssertEqual(start.navigationItem.backButtonTitle, "Close", "the system's Back is shaped as the control it stands for")
        // UIKit's Back button: the authored control, pressed once.
        nav.popViewController(animated: true)
        until("back to Start") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        until("the system's Back pressed the authored control") { self.text(session, "start-backs") == "backs 1" }
        until("no bar on the first route") { nav.isNavigationBarHidden }
        // Viewer over the first route (no bar beneath), then over Detail (a bar beneath).
        let viewer = try push(session, nav, from: "start", "viewer", depth: 2)
        XCTAssertEqual(viewer.node.props["navigationBar"], "none")
        XCTAssertTrue(nav.isNavigationBarHidden, "the route asked for no bar")
        let swipe = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(session.presenter.navigation.popMayBegin(swipe, from: CGPoint(x: 4, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20)),
                      "the edge swipe still pops it")
        try tapNode(session, "viewer-back")
        until("its own control pops it") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        XCTAssertEqual(text(session, "start-backs"), "backs 2")
        _ = try push(session, nav, from: "start", "detail", depth: 2)
        XCTAssertFalse(nav.isNavigationBarHidden, "Detail's header is the bar")
        _ = try push(session, nav, from: "detail", "viewer", depth: 3)
        until("the bar leaves for the route that asked for none") { nav.isNavigationBarHidden }
        nav.popViewController(animated: true)
        until("back to Detail") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        until("and comes back with Detail") { !nav.isNavigationBarHidden }
    }

    /// The first route of a stack has nothing to go back to: with no header
    /// it keeps no bar, a sheet's first route included, pushed though it is
    /// in the router's stack. (This window finishes no presentation, so a
    /// route pushed inside the sheet is not driven here.)
    func testASheetsFirstRouteWithNoHeaderHasNoBar() throws {
        let (session, nav, start) = try bars("bars-sheet")
        let navigation = session.presenter.navigation
        try tapNode(session, "start-open-sheet")
        until("the sheet has its own stack") { navigation.modalNavigation?.viewControllers.count == 1 }
        let sheet = try XCTUnwrap(navigation.modalNavigation)
        let route = try XCTUnwrap(sheet.topViewController as? RouteController)
        XCTAssertEqual(route.node.props["testId"], "route-sheet")
        XCTAssertFalse(route.pushed, "the first route of the presented stack")
        XCTAssertTrue(sheet.isNavigationBarHidden, "a sheet's first route with no header has no bar")
        XCTAssertFalse(navigation.routeShowsBar(route, in: sheet))
        XCTAssertTrue(nav.viewControllers.count == 1 && nav.topViewController === start && nav.isNavigationBarHidden,
                      "and the stack beneath is as it was")
    }

    /// LLP 1116.020 D2: a presented stack's title is inline at any level, as
    /// UIKit's sheets and Apple's are; a level-1 heading pushed in the stack
    /// beneath is large.
    func testASheetsLevelOneTitleIsInline() throws {
        let (session, _, _) = try bars("bars-titled-sheet")
        let navigation = session.presenter.navigation
        try tapNode(session, "start-open-titled")
        until("the sheet has its own stack") { navigation.modalNavigation?.viewControllers.count == 1 }
        let route = try XCTUnwrap(navigation.modalNavigation?.topViewController as? RouteController)
        XCTAssertEqual(route.navigationItem.title, "Titled")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .never, "a sheet's title is inline")
    }


    /// A scroller UIKit insets lays its content out between those insets
    /// (LLP 1116.000 D1.2): Chat's column, its tall box gone, fills the port
    /// and its last line rests above the home indicator, with no scroll by
    /// the insets; with the tall box back the end still lands there, and the
    /// scroller still runs to the screen's edge, its content passing under.
    func testAScrollerUIKitInsetsLaysItsContentOutBetweenTheInsets() throws {
        let session = try fixture("basics-inset")
        let (_, _, chat) = try chat(session)
        let list = try node(session, "list-chat")
        let sv = try XCTUnwrap(list.scroll)
        let inset = sv.adjustedContentInset
        XCTAssertGreaterThan(inset.top, 0, "under the bar")
        XCTAssertGreaterThan(inset.bottom, 0, "over the home indicator")
        until("the kernel has the scroller's insets") {
            session.presenter.navigation.covers[list.id] == .inset(top: inset.top, bottom: inset.bottom)
        }
        let bottom = chat.view.bounds.maxY - chat.view.safeAreaInsets.bottom
        let tall = { session.presenter.views.values.contains { $0.props["testId"] == "tall-chat" } }
        try tapNode(session, "toggle-short-chat")
        until("the tall box went") { !tall() }
        spin(0.3)
        let end = try node(session, "chat-end")
        XCTAssertEqual(end.convert(end.bounds, to: chat.view).maxY, bottom - 12, accuracy: 0.5,
                       "the filled column's last line rests above the home indicator, past its 12 of padding")
        XCTAssertEqual(sv.contentSize.height + inset.bottom - sv.bounds.height, -inset.top, accuracy: 0.5,
                       "a page that fits its port does not scroll by the insets")
        try tapNode(session, "toggle-short-chat")
        until("the tall box is back") { tall() }
        spin(0.3)
        XCTAssertEqual(list.convert(list.bounds, to: chat.view).maxY, chat.view.bounds.maxY, accuracy: 0.5, "it still runs to the edge")
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertEqual(end.convert(end.bounds, to: chat.view).maxY, bottom - 12, accuracy: 0.5, "at its end, as UIKit insets it")
    }

    private func bottomToolbar(_ nav: UINavigationController, _ route: RouteController, shown: Bool) {
        route.toolbarItems = [UIBarButtonItem(title: "Action", style: .plain, target: nil, action: nil)]
        nav.setToolbarHidden(!shown, animated: false)
        nav.view.setNeedsLayout()
        nav.view.layoutIfNeeded()
        route.host?.reportCovers()
    }

    func testFullHeightPlainScrollOwnsTheNativeBottomInsetAndReachesItsEnd() throws {
        // Hostless UIKit does not finish an animated tab selection.
        let animations = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(animations) }
        let session = try fixture("bottom-plain-end")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        try tapNode(session, "tab-second")
        until("Second selected") { tabs.selectedIndex == 1 }
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let list = try node(session, "list-second"), sv = try XCTUnwrap(list.scroll)
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        until("Second's scroller is displayed with its tall content") {
            sv.window != nil && sv.bounds.height > 0 && sv.contentSize.height > sv.bounds.height + 100
        }
        UIView.setAnimationsEnabled(animations)
        bottomToolbar(nav, route, shown: true)
        until("the toolbar insets the physical scroller") {
            sv.adjustedContentInset.bottom > self.window!.safeAreaInsets.bottom + 1
        }
        XCTAssertTrue(route.edgeScroll === list)
        XCTAssertTrue(route.contentScrollView(for: .bottom) === sv)
        XCTAssertEqual(list.convert(list.bounds, to: route.view).maxY, route.view.bounds.maxY, accuracy: 0.5)
        var metrics: [Double]?
        session.presenter.onScroll = { id, values in if id == list.id { metrics = values } }
        list.handlers.insert("scroll")
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        until("the physical end and its scroll event have settled") {
            metrics != nil && abs(sv.contentOffset.y - (sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)) < 0.5
        }
        let event = try XCTUnwrap(metrics), room = list.scrollPortInsets(sv)
        XCTAssertEqual(event[5], Double(sv.bounds.height - room.top - room.bottom), accuracy: 0.5)
        XCTAssertEqual(event[3] - event[1] - event[5], 0, accuracy: 0.5)
        let end = sv.convert(CGPoint(x: 0, y: sv.contentSize.height), to: route.view).y
        XCTAssertEqual(end, route.view.bounds.maxY - sv.adjustedContentInset.bottom, accuracy: 0.5)
    }

    func testPlainShortContentRefitsWhenTheToolbarChangesAndKeepsManualInsetRoom() throws {
        let session = try fixture("bottom-plain-short")
        let (_, nav, route) = try chat(session)
        let list = try node(session, "list-chat"), sv = try XCTUnwrap(list.scroll)
        try tapNode(session, "toggle-short-chat")
        until("the tall content is gone") { !session.presenter.views.values.contains { $0.props["testId"] == "tall-chat" } }
        bottomToolbar(nav, route, shown: true)
        func automaticBottom() -> CGFloat { max(0, sv.adjustedContentInset.bottom - sv.contentInset.bottom) }
        func fits() -> Bool {
            abs(sv.contentSize.height + sv.adjustedContentInset.top + sv.adjustedContentInset.bottom - sv.bounds.height) < 0.5
        }
        until("short content fits above the toolbar") { automaticBottom() > self.window!.safeAreaInsets.bottom + 1 && fits() }
        let shown = automaticBottom(), manual = sv.contentInset.bottom, height = sv.contentSize.height
        let end = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height
        sv.contentInset.bottom += 180 // the keyboard toolbar's manual inset basis
        sv.layoutIfNeeded()
        XCTAssertEqual(sv.contentInset.bottom, manual + 180, accuracy: 0.5)
        XCTAssertEqual(sv.contentSize.height, height, accuracy: 0.5)
        XCTAssertEqual(sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height, end + 180, accuracy: 0.5)
        sv.contentInset.bottom = manual
        bottomToolbar(nav, route, shown: false)
        until("hiding the toolbar refits short content") { automaticBottom() < shown - 1 && fits() }
        let hidden = automaticBottom()
        bottomToolbar(nav, route, shown: true)
        until("showing the toolbar refits short content") { automaticBottom() > hidden + 1 && fits() }
        XCTAssertEqual(sv.contentInset.bottom, manual, accuracy: 0.5)
    }

    /// The tab bar follows the stack shown, on a phone by UIKit's
    /// `hidesBottomBarWhenPushed`, on an iPad's floating bar by Exact's
    /// `setTabBarHidden`: Chat (hiding it) over Detail (showing it) hides it;
    /// a cancelled interactive pop back to Detail keeps it hidden, a completed
    /// one shows it; another tab selected while Chat is still being pushed
    /// shows it there, and Chat hides it again on return.
    func testTheTabBarFollowsTheStackThroughCancelledPopsAndTabSwitches() throws {
        let session = try fixture("basics-follow")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let settled = { nav.transitionCoordinator == nil }
        try tapNode(session, "detail")
        until("Detail pushed") { nav.viewControllers.count == 2 && settled() && barShows(tabs) }
        spin(0.3)
        // Chat pushed, and another tab selected before the push is over.
        let tapped = Agent(session: session).tap(["id": Int(try node(session, "detail-chat").id)])
        XCTAssertNil(tapped["error"], "\(tapped)")
        until("Chat's push started") { nav.viewControllers.count == 3 }
        XCTAssertNotNil(nav.transitionCoordinator, "the tab is switched while Chat's push runs")
        tapTab(tabs, 1)
        until("Second selected") { tabs.selectedIndex == 1 }
        // (Home's push ends here, or, its stack off screen, when it shows again.)
        spin(0.6)
        XCTAssertTrue(barShows(tabs), "Second's root shows the bar, Home's late push notwithstanding")
        tapTab(tabs, 0)
        until("back on Chat over Detail, hidden") { tabs.selectedIndex == 0 && settled() && nav.viewControllers.count == 3 && !barShows(tabs) }
        spin(0.3)
        // A cancelled interactive pop back to Detail keeps it hidden; a completed one shows it.
        let pop = InteractivePop()
        session.presenter.navigation.setAppDelegate(nav, pop)
        pop.interactive = true
        nav.popViewController(animated: true)
        spin(0.05)
        pop.interaction.update(0.4)
        spin(0.1)
        pop.interaction.cancel()
        until("the cancelled pop leaves Chat, the bar hidden") { settled() && nav.viewControllers.count == 3 && !barShows(tabs) }
        spin(0.3)
        XCTAssertFalse(barShows(tabs), "still hidden over Chat")
        pop.interactive = false
        nav.popViewController(animated: true)
        until("back on Detail, the bar shows") { settled() && nav.viewControllers.count == 2 && barShows(tabs) }
        session.presenter.navigation.setAppDelegate(nav, nil)
        // A route that shows the tablist pushed over Chat, which hides it: hidden still, as Chat is under it.
        nav.popToRootViewController(animated: true)
        until("back at Home") { settled() && nav.viewControllers.count == 1 && barShows(tabs) }
        spin(0.3)
        try tapNode(session, "chat")
        until("Chat") { settled() && nav.viewControllers.count == 2 && !barShows(tabs) }
        spin(0.3)
        try tapNode(session, "chat-detail")
        until("Detail over Chat") { settled() && nav.viewControllers.count == 3 }
        spin(0.4)
        XCTAssertFalse(barShows(tabs), "Chat under Detail keeps the bar hidden")
        try tapNode(session, "compose-detail") // a batch: the stack at rest is followed again
        spin(0.4)
        XCTAssertFalse(barShows(tabs), "and after an ordinary update")
        nav.popViewController(animated: true)
        until("back on Chat, hidden") { settled() && nav.viewControllers.count == 2 && !barShows(tabs) }
        nav.popViewController(animated: true)
        until("back at Home, shown") { settled() && nav.viewControllers.count == 1 && barShows(tabs) }
    }

    /// A real interactive pop from Photo, driven by a percent-driven
    /// transition (UIKit's own coordinator, as a finger's edge swipe has):
    /// the bar comes in with Chat, goes back out on the cancel, and after a
    /// completed pop Chat's content area and its scroll range are as before.
    func testAnInteractivePopFromARouteWithoutABarCancelsAndCompletes() throws {
        let session = try fixture("basics-interactive")
        let (_, nav, chat) = try chat(session)
        let list = try node(session, "list-chat")
        let sv = try XCTUnwrap(list.scroll)
        let top = chat.view.safeAreaInsets.top, inset = sv.adjustedContentInset.top
        try tapNode(session, "photo")
        until("Photo pushed") { nav.viewControllers.count == 3 && nav.transitionCoordinator == nil }
        spin(0.3)
        let pop = InteractivePop()
        session.presenter.navigation.setAppDelegate(nav, pop)
        pop.interactive = true
        nav.popViewController(animated: true)
        spin(0.05)
        pop.interaction.update(0.4)
        spin(0.1)
        XCTAssertFalse(nav.isNavigationBarHidden, "the bar comes in with Chat, mid-pop")
        pop.interaction.cancel()
        until("the cancelled pop returns") { nav.transitionCoordinator == nil && nav.viewControllers.count == 3 }
        spin(0.2)
        XCTAssertTrue(nav.isNavigationBarHidden, "and leaves with the cancel")
        pop.interactive = false
        nav.popViewController(animated: true)
        until("the pop lands") { nav.transitionCoordinator == nil && nav.viewControllers.count == 2 }
        spin(0.4)
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertEqual(chat.view.safeAreaInsets.top, top, accuracy: 0.5)
        XCTAssertEqual(sv.adjustedContentInset.top, inset, accuracy: 0.5)
        XCTAssertEqual(list.scrollOrigin, list.scrollCollapsed, "an inline title has no collapse slack")
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        spin(0.3)
        let end = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height
        XCTAssertEqual(sv.contentOffset.y, end, accuracy: 0.5, "the end lands at the scroller's own end")
    }
}

/// A pop the test drives: a cross-fade animator under a percent-driven
/// interaction, as NavigationBarIOSTests' forwarded delegate does.
private final class InteractivePop: NSObject, UINavigationControllerDelegate, UIViewControllerAnimatedTransitioning {
    var interactive = false
    let interaction = UIPercentDrivenInteractiveTransition()
    func navigationController(_ nav: UINavigationController, animationControllerFor operation: UINavigationController.Operation,
                              from: UIViewController, to: UIViewController) -> UIViewControllerAnimatedTransitioning? { self }
    func navigationController(_ nav: UINavigationController,
                              interactionControllerFor animator: UIViewControllerAnimatedTransitioning) -> UIViewControllerInteractiveTransitioning? {
        interactive ? interaction : nil
    }
    func transitionDuration(using context: UIViewControllerContextTransitioning?) -> TimeInterval { 0.2 }
    func animateTransition(using context: UIViewControllerContextTransitioning) {
        guard let to = context.viewController(forKey: .to), let view = context.view(forKey: .to) else { return context.completeTransition(false) }
        view.frame = context.finalFrame(for: to)
        view.alpha = 0
        context.containerView.addSubview(view)
        UIView.animate(withDuration: transitionDuration(using: context), animations: { view.alpha = 1 }) { _ in
            context.completeTransition(!context.transitionWasCancelled)
        }
    }
}
#endif
