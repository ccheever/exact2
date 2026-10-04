#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 §9.10 over the native fixture's Chat and Photo routes: a
/// pressable group around the heading (an avatar, a subtitle) is the item's
/// title, drawn and tapped; Chat, pushed while the authored tablist is
/// hidden, hides the tab bar; its inline title's scroller goes under the
/// bar; Photo, with no header, has no bar, and the bar comes back with Chat.
/// UIKit synthesizes no touches for a unit test: the title is driven by its
/// control's action, an interactive pop by the `willShow` UIKit sends.
final class NavigationBasicsIOSTests: XCTestCase {
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

    private func fixture(_ label: String) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
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
        until("the glyph line") { view.accessibilityValue == "Muted  1w" }
        let line = try XCTUnwrap(view.subtitle.attributedText)
        var attachments = 0
        line.enumerateAttribute(.attachment, in: NSRange(location: 0, length: line.length)) { value, _, _ in if value != nil { attachments += 1 } }
        XCTAssertEqual(attachments, 2, "bell.slash and timer, inline")
        XCTAssertTrue(line.string.contains("Muted") && line.string.contains("1w"))
        try tapNode(session, "toggle-muted")
        until("back to the text subtitle") { view.accessibilityValue == "Online" || view.subtitle.isHidden }
        // A title view a hook sets stays: Exact draws only its own.
        let hooks = UILabel()
        item.titleView = hooks
        try tapNode(session, "toggle-subtitle")
        until("the subtitle came back") { !view.subtitle.isHidden }
        XCTAssertTrue(item.titleView === hooks, "the hook's title view is left alone")
    }

    func testARoutePushedWhileTheTablistIsHiddenHidesTheTabBar() throws {
        let session = try fixture("basics-tabbar")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let home = try XCTUnwrap((tabs.selectedViewController as? UINavigationController)?.topViewController)
        let shown = home.view.safeAreaInsets.bottom
        XCTAssertGreaterThan(shown, 40, "the tab bar insets the root")
        try tapNode(session, "detail")
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        until("Detail pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController)
        XCTAssertFalse(detail.hidesBottomBarWhenPushed, "the tablist shows over Detail")
        XCTAssertEqual(detail.view.safeAreaInsets.bottom, shown, accuracy: 0.5)
        nav.popViewController(animated: true)
        until("back to Home") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        let (_, _, chat) = try chat(session)
        XCTAssertTrue(chat.hidesBottomBarWhenPushed, "the authored tablist is hidden over Chat")
        until("the tab bar left") { chat.view.safeAreaInsets.bottom < shown - 20 }
        nav.popViewController(animated: true)
        until("back to Home") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        until("the tab bar came back") { abs(home.view.safeAreaInsets.bottom - shown) < 0.5 }
    }

    func testAnInlineTitlesScrollerGoesUnderTheBar() throws {
        let session = try fixture("basics-under")
        let (_, nav, chat) = try chat(session)
        let list = try node(session, "list-chat")
        let sv = try XCTUnwrap(list.scroll)
        XCTAssertTrue(journal(session).contains("scrolls its content under the bar"))
        XCTAssertEqual(sv.contentInsetAdjustmentBehavior, .always, "UIKit insets it")
        XCTAssertEqual(list.convert(list.bounds, to: chat.view).minY, 0, accuracy: 0.5, "it starts under the bar")
        XCTAssertEqual(sv.adjustedContentInset.top, chat.view.safeAreaInsets.top, accuracy: 0.5, "inset to the bar's bottom")
        XCTAssertFalse(nav.isNavigationBarHidden)
        let css = { sv.contentOffset.y + sv.adjustedContentInset.top }
        XCTAssertEqual(css(), 0, accuracy: 0.5, "at rest, CSS 0")
        list.pendingScrollTop = 80
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertEqual(css(), 80, accuracy: 0.5, "an authored offset lands as the browser's")
    }

    func testARouteWithNoHeaderHasNoBarAndTheBarFollowsAPop() throws {
        let session = try fixture("basics-nobar")
        let (_, nav, chat) = try chat(session)
        let chatTop = chat.view.safeAreaInsets.top
        try tapNode(session, "photo")
        until("Photo pushed") { nav.viewControllers.count == 3 && nav.transitionCoordinator == nil }
        spin(0.3)
        let photo = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertTrue(nav.isNavigationBarHidden, "a route with no header has no bar")
        XCTAssertLessThan(photo.view.safeAreaInsets.top, chatTop, "only the status bar is left above it")
        let view = try node(session, "photo-view")
        XCTAssertEqual(view.convert(view.bounds, to: photo.view).minY, photo.view.safeAreaInsets.top, accuracy: 0.5,
                       "its content starts below the status bar, the page not covering it")
        XCTAssertFalse(journal(session).contains("navigation bar visibility changed outside Exact"))
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
