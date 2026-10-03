#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 Stage 1 over the native fixture's own plan and hook module,
/// which `build.mjs --test --ios` compiles for this simulator: Exact's bar on
/// frame one from the authored header, the content area it leaves, Back as
/// UIKit's back button, the forwarded delegate, and the module's hooks —
/// their moments, the authored control a hook-made item clicks, and the
/// development check of what Exact owns. UIKit synthesizes no touches for a
/// unit test; a bar item is driven as a tap drives it, by its action.
final class NavigationBarIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        // The module artifact is the process's: later tests run without it.
        NativeViews.uninstallTable()
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

    /// The fixture booted in a window, its module's hooks connected or not.
    private func fixture(_ label: String, module: Bool) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let path = try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")
        let plan = try Data(contentsOf: URL(fileURLWithPath: path))
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

    private func journal(_ session: ExactSession) -> String { session.agent(#"{"op":"logs","since":0}"#) }

    /// How many times the authored Back control's action ran.
    private func backs(_ session: ExactSession) -> Int { journal(session).components(separatedBy: "(back)").count - 1 }

    private func state(_ session: ExactSession, _ slot: String) -> Any? {
        let text = session.agent(#"{"op":"state"}"#)
        let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
        return (json?["slots"] as? [String: Any])?[slot] ?? json?[slot]
    }

    /// What a tap on a bar item does: its action, sent to its target.
    private func tap(_ item: UIBarButtonItem?) throws {
        let item = try XCTUnwrap(item)
        let target = try XCTUnwrap(item.target as? NSObject, "the item's target is retained")
        _ = target.perform(try XCTUnwrap(item.action), with: item)
    }

    func testAHeaderShapedRouteIsTheBarOnFrameOneWithNoAppCode() throws {
        let session = try fixture("bar-frame-one", module: false)
        let navigation = session.presenter.navigation
        let nav = try XCTUnwrap(navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        XCTAssertFalse(nav.isNavigationBarHidden, "the first route is header-shaped")
        XCTAssertTrue(nav.navigationBar.prefersLargeTitles, "a level-1 heading is a large title")
        XCTAssertEqual(top.navigationItem.title, "Fixture")
        XCTAssertEqual(top.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertEqual(top.navigationItem.rightBarButtonItems?.count, 1, "the header's Compose button")
        XCTAssertEqual(top.navigationItem.rightBarButtonItems?.first?.accessibilityLabel, "Compose")
        XCTAssertNil(top.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hook-more" }, "no module, no hook")
        let header = try node(session, "header-home")
        XCTAssertTrue(header.isHidden, "iOS does not paint the header the bar shows")
        XCTAssertEqual(header.bounds.height, 0, "and it takes no space")
        // The content starts where the bar ends: the controller's safe area.
        let content = try node(session, "fixture")
        let bottom = top.view.safeAreaInsets.top
        XCTAssertGreaterThan(bottom, 0)
        XCTAssertEqual(content.convert(content.bounds, to: top.view).minY, bottom, accuracy: 0.5)
        // The bar item presses the authored button, as a tap on it would.
        XCTAssertEqual(state(session, "composed") as? Double, 0)
        try tap(top.navigationItem.rightBarButtonItems?.first)
        until("Compose pressed once") { state(session, "composed") as? Double == 1 }
    }

    func testAPushedRouteShowsBackAsTheAuthoredControlAndAPopPressesItOnce() throws {
        let session = try fixture("bar-push", module: false)
        let agent = Agent(session: session)
        _ = agent.tap(["id": Int(try node(session, "detail").id)])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the detail route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController), home = nav.viewControllers[0]
        XCTAssertEqual(detail.navigationItem.title, "Detail")
        XCTAssertEqual(detail.navigationItem.largeTitleDisplayMode, .never, "a level-2 heading is inline")
        XCTAssertFalse(detail.navigationItem.hidesBackButton, "the route has an enabled Back control")
        XCTAssertEqual(home.navigationItem.backButtonDisplayMode, .minimal, "the authored Back is a symbol alone")
        XCTAssertEqual(detail.navigationItem.rightBarButtonItems?.count, 1, "Back is never an item; Compose is")
        // Under a fixed bar an authored scrollTop is the browser's: the scroll
        // view sits below the bar, so 80 is an offset of 80 and 0 is 0.
        let list = try node(session, "list-detail")
        _ = agent.tap(["id": Int(try node(session, "scroll-80").id)])
        until("scrollTop 80") { list.scroll?.contentOffset.y == 80 }
        _ = agent.tap(["id": Int(try node(session, "scroll-0").id)])
        until("scrollTop 0") { list.scroll?.contentOffset.y == 0 }
        // UIKit's own pop (its back button) is a completed pop: Back, once.
        nav.popViewController(animated: true)
        until("Back dispatched and the stack follows the router") {
            (state(session, "nav") as? [String: Any]).map { (($0["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1 } ?? false
        }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 1)
        XCTAssertEqual(backs(session), 1, "Back once")
    }

    func testAForwardedCustomTransitionPopsOnceAndACancelledOneNever() throws {
        let session = try fixture("bar-delegate", module: false)
        let navigation = session.presenter.navigation
        _ = Agent(session: session).tap(["id": Int(try node(session, "detail").id)])
        let nav = try XCTUnwrap(navigation.primaryNavigation)
        until("pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let app = Transitions()
        navigation.setAppDelegate(nav, app)
        XCTAssertTrue(nav.delegate !== app, "Exact keeps the slot")
        // Cancelled: an interactive pop the app drives and abandons.
        app.interactive = true
        nav.popViewController(animated: true)
        spin(0.05)
        app.interaction.update(0.3)
        app.interaction.cancel()
        until("the cancelled pop returns") { nav.transitionCoordinator == nil && nav.viewControllers.count == 2 }
        spin(0.2)
        XCTAssertTrue(app.animated, "the app's animator ran, through the forwarded delegate")
        XCTAssertEqual(backs(session), 0, "no Back")
        // Completed: the same animator, finished.
        app.interactive = false
        nav.popViewController(animated: true)
        until("the completed pop lands") { nav.transitionCoordinator == nil && nav.viewControllers.count == 1 }
        spin(0.2)
        XCTAssertGreaterThan(app.shown, 0, "didShow forwarded after Exact's own")
        XCTAssertEqual(backs(session), 1, "Back once")
    }

    func testTheModulesHooksRunAtTheirMomentsAndActThroughAuthoredControls() throws {
        let session = try fixture("bar-hooks", module: true)
        let log = { self.journal(session) }
        until("hooks connected and replayed") { log().contains("hook route 0: built") }
        let lines = log()
        let navigationAt = try XCTUnwrap(lines.range(of: "hook navigation #1: built"))
        XCTAssertLessThan(navigationAt.lowerBound, try XCTUnwrap(lines.range(of: "hook route 0: built")).lowerBound, "a stack's hook runs before its routes'")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        XCTAssertEqual(nav.navigationBar.tintColor, .systemIndigo, "the long tail is the hook's")
        let home = try XCTUnwrap(nav.topViewController)
        let more = home.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hook-more" }
        XCTAssertEqual(home.navigationItem.rightBarButtonItems?.count, 1, "Exact's Compose stays beside the hook's item")
        // The hook-made control clicks the authored one.
        try tap(more)
        until("the hook's click pressed Compose") { state(session, "composed") as? Double == 1 }
        // A pushed route's hook; a write to what Exact owns on its content
        // scroll view is journaled by name at the next batch; a route the
        // router drops ends its handle.
        let agent = Agent(session: session)
        _ = agent.tap(["id": Int(try node(session, "detail").id)])
        let key = try XCTUnwrap(try node(session, "route-detail").props["navigationKey"])
        until("detail built") { log().contains("hook route \(key): built") && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController)
        XCTAssertNotNil(detail.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hook-more" })
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the hook saw the word") { log().contains("hook route \(key): changed") }
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the check journaled it") { log().contains("route \(key): contentInset changed outside Exact, which owns it") }
        nav.popViewController(animated: true)
        until("detail ended") { log().contains("hook route \(key): ended") }
    }
}

/// An app's delegate: a cross-fade, interactive when asked.
private final class Transitions: NSObject, UINavigationControllerDelegate, UIViewControllerAnimatedTransitioning {
    var interactive = false, animated = false, shown = 0
    let interaction = UIPercentDrivenInteractiveTransition()
    func navigationController(_ nav: UINavigationController, animationControllerFor operation: UINavigationController.Operation,
                              from: UIViewController, to: UIViewController) -> UIViewControllerAnimatedTransitioning? { self }
    func navigationController(_ nav: UINavigationController,
                              interactionControllerFor animator: UIViewControllerAnimatedTransitioning) -> UIViewControllerInteractiveTransitioning? {
        interactive ? interaction : nil
    }
    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) { shown += 1 }
    func transitionDuration(using context: UIViewControllerContextTransitioning?) -> TimeInterval { 0.2 }
    func animateTransition(using context: UIViewControllerContextTransitioning) {
        animated = true
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
