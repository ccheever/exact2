#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 Stage 1 over the native fixture's own plan and hatch module,
/// which `build.mjs --test --ios` compiles for this simulator: Exact's bar on
/// frame one from the authored header, the content area it leaves, Back as
/// UIKit's back button, the forwarded delegate, and the module's hatches —
/// their moments, the authored control a hatch-made item clicks, and the
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

    /// The fixture booted in a window, its module's hatches connected or not.
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
        // Its colour is the page's `color="#111111"`, inherited as CSS has it.
        let tint = try XCTUnwrap(top.navigationItem.rightBarButtonItems?.first?.tintColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)))
        var rgb: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        XCTAssertTrue(tint.getRed(&rgb.0, green: &rgb.1, blue: &rgb.2, alpha: &rgb.3))
        XCTAssertEqual(rgb.0 * 255, 0x11, accuracy: 1)
        XCTAssertNil(top.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" }, "no module, no hatch")
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

    /// A bar item takes its button's authored `color` (a symbol's, its
    /// tint): a header's Cancel in the app's accent is not UIKit's black.
    /// A platform colour (CSS's initial `CanvasText`) leaves UIKit's.
    func testABarItemTakesItsButtonsAuthoredColour() throws {
        let p = Presenter()
        let canvasText: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "style": ["text_color": [181, 86, 43, 255]]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Cancel"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "style": ["text_color": canvasText]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Done"], "style": ["text_color": canvasText]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1, 3]],
        ]))
        let accent = HeaderShape.Item(try XCTUnwrap(p.views[1])), plain = HeaderShape.Item(try XCTUnwrap(p.views[3]))
        XCTAssertEqual(accent.title, "Cancel")
        XCTAssertEqual(accent.tint, [[181, 86, 43, 255], [181, 86, 43, 255]])
        XCTAssertNil(plain.tint, "a platform colour is UIKit's to draw")
    }

    /// LLP 1075.003 §9.6: a button whose one child is a filled box is an
    /// image item drawn from it; a popover's invoker is an item whose menu
    /// holds the popover's rows, each pressing its authored button.
    func testABadgeButtonIsAnImageItemAndAPopoverInvokerAMenuItem() throws {
        let session = try fixture("bar-badge-menu", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let left = top.navigationItem.leftBarButtonItems ?? []
        let badge = try XCTUnwrap(left.first { $0.accessibilityLabel == "Profile" }, "the badge button is an item")
        let image = try XCTUnwrap(badge.image, "drawn from the box")
        XCTAssertEqual(image.size, CGSize(width: 36, height: 36))
        XCTAssertEqual(image.renderingMode, .alwaysOriginal, "the box's own colours, not the bar's tint")
        XCTAssertNil(badge.menu)
        try tap(badge)
        until("the badge pressed its button") { state(session, "composed") as? Double == 1 }
        let more = try XCTUnwrap(left.first { $0.accessibilityLabel == "More" }, "the popover's invoker is an item")
        let menu = try XCTUnwrap(more.menu, "with the popover as its menu")
        XCTAssertNil(more.action, "UIKit opens the menu on a tap")
        XCTAssertTrue(menu.children.first is UIDeferredMenuElement, "rows read as it opens")
        let pop = try XCTUnwrap(session.presenter.carrying("popover").first { $0.props["id"] == "bar-menu" })
        let rows = session.presenter.menus.items(of: pop)
        let row = try XCTUnwrap(rows.first as? UIAction)
        XCTAssertEqual(row.title, "Bump from the bar")
    }

    /// LLP 1075.003 §9.6: a header's `input type="search"` is the item's
    /// search controller; what the reader types is the field's `input`.
    func testAHeaderSearchFieldIsTheItemsSearchControllerAndTypingIsItsInput() throws {
        let session = try fixture("bar-search", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let search = try XCTUnwrap(top.navigationItem.searchController, "the header's search field")
        XCTAssertEqual(search.searchBar.placeholder, "Search the fixture")
        search.searchBar.text = "abc"
        search.searchResultsUpdater?.updateSearchResults(for: search)
        until("the field's input ran with the text") { state(session, "query") as? String == "abc" }
    }

    /// LLP 1075.003 §9.8: a header's text tablist is the item's title view,
    /// a segmented control; a segment's tap presses its tab.
    func testAHeaderTablistIsTheItemsSegmentedTitleView() throws {
        let session = try fixture("bar-segments", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let control = try XCTUnwrap(top.navigationItem.titleView as? UISegmentedControl, "the tablist is the title view")
        XCTAssertEqual(control.numberOfSegments, 2)
        XCTAssertEqual(control.titleForSegment(at: 1), "Missed")
        XCTAssertEqual(control.selectedSegmentIndex, 0)
        XCTAssertEqual(top.navigationItem.title, "Fixture", "the heading stays the title")
        XCTAssertFalse((top.navigationItem.rightBarButtonItems ?? []).contains { $0.title == "All" || $0.title == "Missed" }, "tabs are not items")
        let press = try XCTUnwrap(control.allTargets.first as? SegmentPress, "the control's target")
        XCTAssertNotNil(press.host)
        XCTAssertEqual(press.tabs, [try node(session, "segment-all").id, try node(session, "segment-missed").id])
        // A tap: the segment selected, then the control's action for a
        // value change sent to its target.
        control.selectedSegmentIndex = 1
        let action = try XCTUnwrap(control.actions(forTarget: press, forControlEvent: .valueChanged)?.first, "registered for a value change")
        _ = press.perform(NSSelectorFromString(action), with: control)
        until("the tab pressed") { state(session, "segment") as? String == "missed" }
        until("the selection follows aria-selected") { control.selectedSegmentIndex == 1 }
        // Sized for the text size and weight it shows, when they change and
        // no batch follows (every batch leaves it alone otherwise).
        let segments = try XCTUnwrap((top as? RouteController)?.titleSegments)
        for (category, weight) in [(UIContentSizeCategory.accessibilityExtraLarge, UILegibilityWeight.regular), (.accessibilityExtraLarge, .bold)] {
            control.frame.size = CGSize(width: 1, height: 1)
            control.traitOverrides.preferredContentSizeCategory = category
            control.traitOverrides.legibilityWeight = weight
            control.updateTraitsIfNeeded()
            XCTAssertEqual(segments.sized?.traits, [category, weight.rawValue] as [AnyHashable])
            XCTAssertGreaterThanOrEqual(control.frame.width, 180, "sized again: two segments, 90 pt each at least")
            XCTAssertGreaterThan(control.frame.height, 1)
        }
    }

    func testAPushedRouteShowsBackAsTheAuthoredControlAndAPopPressesItOnce() throws {
        let session = try fixture("bar-push", module: false)
        let agent = Agent(session: session)
        let tapped = try node(session, "detail")
        let reply = agent.tap(["id": Int(tapped.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, Int(tapped.id), "the native projection stays above its authored holders")
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

    /// A pop UIKit finishes with no enabled Back control to press (here,
    /// disabled just before the bar's pop, as a tap racing that batch does)
    /// presses nothing, and the native stack goes back to the one the router
    /// still declares, not one route short of it. The Brooks port's fork
    /// needed a repair for this; main already restores the stack, with the
    /// batch before the pop or during it, and this keeps it so.
    func testAPopThatOutlivesItsBackControlRestoresTheDeclaredStack() throws {
        let session = try fixture("bar-back-gone", module: false)
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "detail").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the detail route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let p = session.presenter
        let name = try XCTUnwrap(p.views.values.first { $0.props["navigationBack"] != nil }?.props["navigationBack"])
        let back = try XCTUnwrap(p.views.values.first { $0.props["id"] == name && $0.handlers.contains("press") })
        p.apply(wireBatch([["op": "props", "id": Int(back.id), "set": ["disabled": "true"], "clear": []]]))
        nav.popViewController(animated: true)
        until("the transition ends and the declared stack is back") { nav.transitionCoordinator == nil && nav.viewControllers.count == 2 }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 2, "the router still declares the detail route")
        XCTAssertEqual(backs(session), 0, "no enabled Back control, so nothing is pressed")
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

    func testTheModulesHatchesRunAtTheirMomentsAndActThroughAuthoredControls() throws {
        let session = try fixture("bar-hatches", module: true)
        let log = { self.journal(session) }
        until("hatches connected and replayed") { log().contains("hatch route 0: built") }
        let lines = log()
        let navigationAt = try XCTUnwrap(lines.range(of: "hatch navigation #1: built"))
        XCTAssertLessThan(navigationAt.lowerBound, try XCTUnwrap(lines.range(of: "hatch route 0: built")).lowerBound, "a stack's hatch runs before its routes'")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        XCTAssertEqual(nav.navigationBar.tintColor, .systemIndigo, "the long tail is the hatch's")
        let home = try XCTUnwrap(nav.topViewController)
        // A route prepared again with nothing changed keeps its projected
        // items and runs no hatch.
        let items = home.navigationItem.rightBarButtonItems ?? []
        session.presenter.navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        spin(0.2)
        XCTAssertFalse(log().contains("hatch route 0: changed"), "an unchanged route runs no hatch")
        XCTAssertTrue(items.elementsEqual(home.navigationItem.rightBarButtonItems ?? [], by: ===), "an unchanged route is not projected again")
        let more = home.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" }
        XCTAssertEqual(home.navigationItem.rightBarButtonItems?.count, 1, "Exact's Compose stays beside the hatch's item")
        // The hatch-made control clicks the authored one.
        try tap(more)
        until("the hatch's click pressed Compose") { state(session, "composed") as? Double == 1 }
        // A pushed route's hatch; a write to what Exact owns on its content
        // scroll view is journaled by name at the next batch; a route the
        // router drops ends its handle.
        let agent = Agent(session: session)
        _ = agent.tap(["id": Int(try node(session, "detail").id)])
        let key = try XCTUnwrap(try node(session, "route-detail").props["navigationKey"])
        until("detail built") { log().contains("hatch route \(key): built") && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController)
        XCTAssertNotNil(detail.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" })
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the hatch saw the word") { log().contains("hatch route \(key): changed") }
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the check journaled it") { log().contains("route \(key): contentInset changed outside Exact, which owns it") }
        nav.popViewController(animated: true)
        until("detail ended") { log().contains("hatch route \(key): ended") }
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
