#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Sheets and their navigation stacks (LLP 1075.003 §9, LLP 1035.003 D5):
/// a second sheet waits for the first presentation, and Back inside a sheet
/// preserves the background stack.
final class ModalStackIOSTests: XCTestCase {
    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// A fixture booted in a window of the test host's scene: UIKit
    /// presents a sheet only from a window a scene holds.
    private func fixture(_ label: String, plan: String = "EXACT_FIXTURE_PLAN", fullScreen: Bool = false) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[plan], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = fullScreen ? window.screen.bounds : CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func press(_ session: ExactSession, _ testId: String) throws {
        session.presenter.press(try node(session, testId).id)
    }

    /// The controllers presented over the fixture, bottom first.
    private func presented() -> [UIViewController] {
        Array(sequence(first: host?.presentedViewController, next: { $0?.presentedViewController }).compactMap { $0 })
    }

    private func stateSlots(_ session: ExactSession) throws -> [String: Any] {
        let state = try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
        return try XCTUnwrap(state?["slots"] as? [String: Any])
    }

    /// Exercise UIKit's animated Back transition; the smoke also taps its actual control.
    func testBackUsesThePresentedStackAndKeepsTheBackgroundStack() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "the secondary navigation stack uses UIKit's presented sheet")
        }
        let session = try fixture("secondary-navigation", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
        let p = session.presenter, agent = Agent(session: session)
        try press(session, "home-open-inline")
        until("the background stack settles") { !agent.nativeInFlight() }
        let primary = try XCTUnwrap(p.navigation.primaryNavigation)
        let background = primary.viewControllers
        XCTAssertEqual(background.count, 2)
        let backgroundBack = try XCTUnwrap(p.navigation.observation()["nativeBack"] as? String)
        try press(session, "inline-open-sheet")
        until("the sheet's own navigation settles") { p.navigation.modalNavigation != nil && !agent.nativeInFlight() }
        let secondary = try XCTUnwrap(p.navigation.modalNavigation)
        XCTAssertEqual(secondary.viewControllers.count, 1)
        XCTAssertNil(p.navigation.observation()["nativeBack"] as? String, "the sheet's root cannot target Back behind it")
        try press(session, "sheet-open-long")
        until("the secondary push settles") { secondary.viewControllers.count == 2 && !agent.nativeInFlight() }
        let longKey = try XCTUnwrap((secondary.topViewController as? RouteController)?.key)
        let sheetKey = try XCTUnwrap((secondary.viewControllers.first as? RouteController)?.key)
        let secondaryBack = try XCTUnwrap(p.navigation.observation()["nativeBack"] as? String)
        XCTAssertEqual(secondaryBack, secondary.view.accessibilityIdentifier)
        XCTAssertNotEqual(secondaryBack, backgroundBack)
        let scroll = try XCTUnwrap(try node(session, "long-scroll").scroll)
        scroll.setContentOffset(CGPoint(x: 0, y: 100), animated: false)
        let offset = scroll.contentOffset
        XCTAssertGreaterThan(offset.y, 0)
        try press(session, "long-open-edit")
        until("the secondary editor settles") { secondary.viewControllers.count == 3 && !agent.nativeInFlight() }
        let editor = try node(session, "edit-draft")
        XCTAssertTrue(editor.field?.isFirstResponder == true)
        XCTAssertNil(agent.type(["id": Int(editor.id), "text": "Secondary draft"])["error"])
        _ = secondary.popViewController(animated: true)
        XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        until("Back returns within the sheet") { secondary.viewControllers.count == 2 && p.navigation.selectedKey == longKey && !agent.nativeInFlight() }
        XCTAssertTrue(p.navigation.modalNavigation === secondary)
        XCTAssertNotNil(secondary.view.window)
        let restoredScroll = try XCTUnwrap(try node(session, "long-scroll").scroll)
        XCTAssertEqual(restoredScroll.contentOffset.y, offset.y, accuracy: 0.5)
        XCTAssertEqual(primary.viewControllers, background)
        XCTAssertEqual(try stateSlots(session)["query"] as? String, "Secondary draft")
        _ = secondary.popViewController(animated: true)
        XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        until("the sheet returns to its root") { secondary.viewControllers.count == 1 && p.navigation.selectedKey == sheetKey && !agent.nativeInFlight() }
        XCTAssertEqual(p.navigation.selectedKey, sheetKey, session.agent(#"{"op":"state"}"#))
        XCTAssertNil(p.navigation.observation()["nativeBack"] as? String)
        XCTAssertEqual(primary.viewControllers, background)
        _ = agent.back()
        until("the sheet dismisses") { p.navigation.modalNavigation == nil && !agent.nativeInFlight() }
        XCTAssertEqual(primary.viewControllers, background)
        XCTAssertEqual(p.navigation.observation()["nativeBack"] as? String, backgroundBack)
    }

    func testBackHistoryUsesTheChosenDestinationInThePrimaryStack() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "native Back history uses UIKit's completed transition")
        }
        let session = try fixture("primary-back-history", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
        let agent = Agent(session: session)
        for target in ["home-open-inline", "inline-open-long", "long-open-edit"] {
            try press(session, target)
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        }
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        try checkHistoryJump(session, in: nav, destination: "/inline")
    }

    func testBackHistoryFinishesHeaderSearchBeforeDispatching() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "header search commits through UIKit's responder lifecycle")
        }
        for query in ["history", "redirect", "replace"] {
            let session = try fixture("search-back-history-\(query)", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
            defer { session.destroy(); window?.isHidden = true }
            let agent = Agent(session: session)
            for target in ["home-open-inline", "inline-open-long", "long-open-seek"] {
                try press(session, target)
                XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
            }
            let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation), original = nav.viewControllers
            let search = try XCTUnwrap(nav.topViewController?.navigationItem.searchController)
            let field = search.searchBar.searchTextField
            until("the source's search field takes focus") { field.isFirstResponder }
            let input = try node(session, "seek-search")
            field.insertText(query)
            until("typing reaches the app before Back") { input.props["value"] == query }
            until("the source's search presentation settles") {
                search.isActive && !search.isBeingPresented && search.transitionCoordinator == nil
            }
            XCTAssertEqual(original.count, 4)
            let destination = try XCTUnwrap(original.dropFirst().first)
            let popped = try XCTUnwrap(nav.popToViewController(destination, animated: true))
            XCTAssertEqual(popped.count, 2)
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
            if query == "redirect" {
                XCTAssertEqual(Array(nav.viewControllers.prefix(3)), Array(original.prefix(3)))
                XCTAssertEqual(nav.viewControllers.count, 4)
                XCTAssertEqual((nav.topViewController as? RouteController)?.node.props["testId"], "route-find")
                XCTAssertEqual(session.routerOp?["url"] as? String, "/find")
            } else {
                XCTAssertEqual(nav.viewControllers, Array(original.prefix(2)))
                XCTAssertEqual(session.routerOp?["url"] as? String, "/inline")
            }
            XCTAssertEqual(session.presenter.navigation.selectedKey, (nav.topViewController as? RouteController)?.key)
            XCTAssertFalse(field.isFirstResponder)
            let slots = try stateSlots(session)
            XCTAssertEqual(slots["searchBlurs"] as? Int, 1)
            XCTAssertEqual(slots["searchChanges"] as? Int, 1)
            XCTAssertEqual(slots["committedQuery"] as? String, query)
            XCTAssertEqual(slots["query"] as? String, query)
        }
    }

    func testCancellingHeaderSearchClearsTheLiveQuery() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "native search cancellation uses UIKit's presented controls")
        }
        // Duo's inner-display Close dismisses the controller without the bar's Cancel action;
        // the bar's Cancel action is checked on the primary stack.
        let cases: [(submitted: Bool, presented: Bool, cancelButton: Bool)] = [
            (false, false, false), (false, false, true), (false, true, false),
            (true, false, false), (true, false, true), (true, true, false),
        ]
        for (submitted, presented, cancelButton) in cases {
            let variant = "submitted \(submitted), presented \(presented), cancel button \(cancelButton)"
            let session = try fixture("cancel-header-search-\(submitted)-\(presented)-\(cancelButton)", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
            defer { session.destroy(); window?.isHidden = true }
            let agent = Agent(session: session)
            if presented {
                try press(session, "home-open-sheet")
                XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true, variant)
            }
            try press(session, presented ? "sheet-open-seek" : "home-open-seek")
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true, variant)
            let navigation = try XCTUnwrap(presented ? session.presenter.navigation.modalNavigation : session.presenter.navigation.primaryNavigation, variant)
            let item = try XCTUnwrap(navigation.topViewController?.navigationItem, variant)
            let search = try XCTUnwrap(item.searchController, variant), field = search.searchBar.searchTextField
            until("the search field takes focus (\(variant))") { field.isFirstResponder }
            let input = try node(session, "seek-search")
            field.insertText("cancel-query")
            until("the app hears the query (\(variant))") { input.props["value"] == "cancel-query" }
            until("search presentation settles (\(variant))") { search.isActive && !search.isBeingPresented && search.transitionCoordinator == nil }
            if submitted {
                search.searchBar.delegate?.searchBarSearchButtonClicked?(search.searchBar)
                until("Search ends editing without closing the search (\(variant))") { !field.isFirstResponder }
                XCTAssertTrue(search.isActive, variant)
                XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true, variant)
            }
            let before = try stateSlots(session)
            let blurs = try XCTUnwrap(before["searchBlurs"] as? Int), changes = try XCTUnwrap(before["searchChanges"] as? Int)
            XCTAssertEqual(before["searching"] as? Bool, !submitted, variant)
            if cancelButton {
                func buttons(in view: UIView) -> [UIButton] {
                    ((view as? UIButton).map { [$0] } ?? []) + view.subviews.flatMap { buttons(in: $0) }
                }
                let controls = buttons(in: try XCTUnwrap(window))
                // UIKit's registered cancellation action; its projected button has alpha zero on Duo
                let cancel = controls.filter { button in button.allTargets.contains { ($0 as AnyObject) === search.searchBar } }
                XCTAssertEqual(cancel.count, 1, "UIKit exposes one control owned by this search (\(variant))")
                guard cancel.count == 1 else { return }
                XCTAssertNotNil(cancel[0].actions(forTarget: search.searchBar, forControlEvent: .touchUpInside), variant)
                cancel[0].sendActions(for: .touchUpInside)
            } else {
                search.isActive = false
            }
            until("UIKit dismisses the search (\(variant))") { !search.isActive && !search.isBeingDismissed && !field.isFirstResponder }
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true, variant)
            XCTAssertEqual(input.props["value"], "", variant)
            XCTAssertEqual(search.searchBar.text ?? "", "", variant)
            XCTAssertTrue(item.hidesSearchBarWhenScrolling, variant)
            XCTAssertEqual(session.routerOp?["url"] as? String, "/seek", variant)
            XCTAssertNotNil(navigation.view.window, "closing search keeps its navigation stack on screen (\(variant))")
            let slots = try stateSlots(session)
            XCTAssertEqual(slots["searchBlurs"] as? Int, blurs + (submitted ? 0 : 1), "\(variant), before: \(blurs)")
            XCTAssertEqual(slots["searchChanges"] as? Int, changes + 1, "\(variant), before: \(changes)")
            XCTAssertEqual(slots["committedQuery"] as? String, "", variant)
        }
    }

    func testBackHistoryUsesTheChosenDestinationInThePresentedStack() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "native Back history uses the sheet's completed transition")
        }
        let session = try fixture("secondary-back-history", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
        let p = session.presenter, agent = Agent(session: session)
        for target in ["home-open-inline", "inline-open-sheet", "sheet-open-long", "long-open-find", "find-open-edit"] {
            try press(session, target)
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        }
        let primary = try XCTUnwrap(p.navigation.primaryNavigation), background = primary.viewControllers
        let secondary = try XCTUnwrap(p.navigation.modalNavigation)
        try checkHistoryJump(session, in: secondary, destination: "/sheet", remaining: 1)
        XCTAssertNil(p.navigation.observation()["nativeBack"] as? String)
        XCTAssertTrue(p.navigation.modalNavigation === secondary)
        XCTAssertEqual(primary.viewControllers, background)
    }

    func testBackHistoryNavigatesWithoutPressingTheSourcesBackControl() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "native Back history uses UIKit's completed transition")
        }
        for authoredNavigate in [true, false] {
            let session = try fixture("authored-back-history-\(authoredNavigate)", fullScreen: true)
            for location in ["/detail", "/plain", "/chat"] {
                XCTAssertTrue(session.navigate(location))
                XCTAssertEqual(Agent(session: session).clock(["settle": true])["settled"] as? Bool, true)
            }
            let root = try node(session, "navigation")
            if !authoredNavigate { root.handlers.remove("navigate") }
            let journalBefore = session.agent(#"{"op":"logs","since":0}"#)
            let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
            try checkHistoryJump(session, in: nav, destination: "/detail")
            let journalAfter = session.agent(#"{"op":"logs","since":0}"#)
            XCTAssertEqual(journalAfter.components(separatedBy: "(follow)").count - journalBefore.components(separatedBy: "(follow)").count, authoredNavigate ? 1 : 0)
            XCTAssertEqual(journalAfter.components(separatedBy: "(back)").count, journalBefore.components(separatedBy: "(back)").count,
                           "a history jump does not press the source's one-step Back control")
            session.destroy()
        }
    }

    func testANavigationDuringTheNativePopKeepsTheAppsNewerRoute() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "the app navigates during UIKit's animated pop")
        }
        for remaining in [2, 3] {
            let session = try fixture("back-during-navigation-\(remaining)", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
            let agent = Agent(session: session)
            for target in ["home-open-inline", "inline-open-long", "long-open-edit"] {
                try press(session, target)
                XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
            }
            let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
            let original = nav.viewControllers
            _ = nav.popToViewController(original[remaining - 1], animated: true)
            XCTAssertNotNil(nav.transitionCoordinator, "the app navigates before UIKit finishes")
            XCTAssertTrue(session.navigate("/short"))
            let selected = session.presenter.navigation.selectedKey
            XCTAssertEqual(session.routerOp?["url"] as? String, "/short")
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
            XCTAssertEqual(session.routerOp?["url"] as? String, "/short")
            XCTAssertEqual(session.presenter.navigation.selectedKey, selected)
            XCTAssertEqual(Array(nav.viewControllers.prefix(original.count)), original)
            XCTAssertEqual(nav.viewControllers.count, original.count + 1)
            session.destroy()
        }
    }

    func testAHistoryDestinationWithoutARouterVisitRestoresTheDeclaredStack() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a refused history jump reconciles UIKit's stack")
        }
        let session = try fixture("back-history-refused", plan: "EXACT_PUSHED_TITLE_PLAN", fullScreen: true)
        let p = session.presenter, agent = Agent(session: session)
        for target in ["home-open-inline", "inline-open-long", "long-open-edit"] {
            try press(session, target)
            XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        }
        let nav = try XCTUnwrap(p.navigation.primaryNavigation), original = nav.viewControllers
        let destination = try XCTUnwrap(original[1] as? RouteController)
        destination.node.props["navigationKey"] = "authored-inline"
        p.navigation.sync(wireBatch([]))
        _ = nav.popToViewController(destination, animated: true)
        XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        XCTAssertEqual(nav.viewControllers, original)
        XCTAssertEqual(p.navigation.selectedKey, (original.last as? RouteController)?.key)
        XCTAssertEqual(session.routerOp?["url"] as? String, "/edit")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("the destination has no router visit"))
    }

    private func checkHistoryJump(_ session: ExactSession, in nav: UINavigationController, destination: String, remaining: Int = 2) throws {
        XCTAssertEqual(nav.viewControllers.count, 4, "the menu can skip more than one route")
        let expected = Array(nav.viewControllers.prefix(remaining))
        let target = try XCTUnwrap(expected.last as? RouteController)
        let popped = try XCTUnwrap(nav.popToViewController(target, animated: true))
        XCTAssertEqual(popped.count, 4 - remaining)
        XCTAssertEqual(Agent(session: session).clock(["settle": true])["settled"] as? Bool, true)
        XCTAssertEqual(nav.viewControllers, expected, "the router must not reintroduce routes UIKit skipped")
        XCTAssertEqual(session.presenter.navigation.selectedKey, target.key)
        XCTAssertEqual(session.routerOp?["url"] as? String, destination)
    }

    /// Nothing presenting: the agent's wait after an input returns at once.
    func testTheWaitReturnsAtOnceWithNoSheetMoving() throws {
        let session = try fixture("sheet-wait-idle")
        let started = Date()
        XCTAssertTrue(session.agentInstance.awaitModalTransitions())
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.1)
    }

    /// The menu pushed over Sheet while Sheet is still being presented (a
    /// drive's taps a few milliseconds apart, the Bluesky clone's prompt over
    /// its muted words sheet) waits: UIKit presents from the topmost
    /// controller only once it has finished presenting, so the second route
    /// is in the tree and not yet on screen, and the agent's wait after the
    /// input covers it. This window ends no presentation (see
    /// ModalDetentIOSTests), so the wait ends at its bound, saying so; the
    /// sheets presented in order, and closed, are driven by
    /// `scripts/smoke-native.mjs` on a simulator.
    func testASheetOverAPresentingSheetWaitsAndTheAgentWaitsWithIt() throws {
        let session = try fixture("sheet-over-presenting-sheet")
        let modals = session.presenter.modals
        try press(session, "sheet")
        XCTAssertEqual(presented().count, 1, "Sheet is being presented")
        XCTAssertTrue(modals.inTransition)
        try press(session, "sheet-menu")
        XCTAssertNotNil(try node(session, "menu-close"), "the menu's route is in the tree")
        XCTAssertEqual(presented().count, 1, "the menu waits while Sheet presents")
        XCTAssertEqual(modals.routes.map { $0.node.props["testId"] }, ["route-sheet"])
        let started = Date()
        XCTAssertFalse(session.agentInstance.awaitModalTransitions(bound: 0.3), "the wait covers Sheet still presenting")
        XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(started), 0.3)
    }

    /// LLP 1035.001 D1: a root whose key names no route keeps what is left of
    /// the stack it showed. Chat-rooms (bake-off round 6) keyed its list route
    /// "rooms" while its root named the visit, `${top(nav).id}`, "0" at the
    /// stack's root: its new-room sheet opened, and Cancel (the sheet's route
    /// gone, the key "0" again) left the sheet presented over a blank screen and
    /// the list inactive beneath it. Now the list is selected again and live,
    /// and the sheet, its route gone, is owed its close. Batches as the
    /// runner writes them, to a session whose boot was refused: a view's
    /// first fit boots its session (a navigation transition's end fits it,
    /// in the batch that installs the stack), and here that would be the
    /// test host's baked plan over these batches. A refused boot boots
    /// nothing again (LLP 1031 D8).
    func testASheetWhoseRouteLeftWhileTheRootNamesNoRouteClosesAndLeavesItsBackgroundLive() throws {
        let session = ExactApp.shared.makeSession(label: "sheet-closed-root-names-none")
        sessions.append(session)
        XCTAssertNotNil(session.boot(plan: Data(), size: CGSize(width: 402, height: 874)).error, "no plan in no bytes")
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        self.window = window
        self.host = host
        session.presenter.viewport.frame = host.view.bounds
        // A sheet waits for the first drawn frame, which this window never draws.
        session.firstDrawMs = ExactEnv.wall()
        let apply = { (ops: [[String: Any]]) in session.presenter.apply(wireBatch(ops)) }
        let frame = { (id: Int) -> [String: Any] in ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0] }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "0"]], frame(1),
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "rooms"]], frame(2),
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["testId": "room-1"]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 402.0, "h": 44.0],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        let navigation = session.presenter.navigation
        let rooms = try XCTUnwrap(session.presenter.views[2]), row = try XCTUnwrap(session.presenter.views[3])
        XCTAssertFalse(navigation.isInactiveRoute(containing: row))
        // Open the sheet: the root names its visit, which its route carries.
        apply([
            ["op": "create", "id": 4, "kind": "view", "props": ["navigationKey": "1", "navigationPresentation": "modal"]], frame(4),
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["id": "back", "testId": "room-cancel"]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 80.0, "h": 44.0],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "props", "id": 1, "set": ["navigationKey": "1"]],
        ])
        until("the new-room sheet is presented") { presented().count == 1 }
        XCTAssertEqual(navigation.selectedKey, "1")
        XCTAssertTrue(navigation.isInactiveRoute(containing: row), "the list is inert beneath the sheet")
        // Cancel: the sheet's route leaves, and the root names visit 0 again, which no route carries.
        apply([
            ["op": "children", "id": 4, "ids": []],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "destroy", "id": 5],
            ["op": "destroy", "id": 4],
            ["op": "props", "id": 1, "set": ["navigationKey": "0"]],
        ])
        XCTAssertEqual(navigation.selectedKey, "rooms", "the route left of the stack shown is selected")
        XCTAssertFalse(navigation.isInactiveRoute(containing: row), "the list is live again")
        XCTAssertTrue((navigation.primaryNavigation?.viewControllers.first as? RouteController)?.node === rooms)
        // This window finishes no presentation (ModalDetentIOSTests), so the sheet's close waits for it here,
        // owed; on a simulator the sheet goes and the list shows (`agent ios`: Cancel, then tap a room).
        XCTAssertTrue(navigation.syncOwed, "the sheet's close is owed")
    }
    func testAReplacementRoutePresentsBeforeAPageDialogResumes() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "route replacement needs a presented stack")
        }
        let session = ExactApp.shared.makeSession(label: "dialog-route-replacement")
        sessions.append(session)
        XCTAssertNotNil(session.boot(plan: Data(), size: CGSize(width: 402, height: 874)).error)
        let view = ExactView(session: session), host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        self.window = window
        self.host = host
        let p = session.presenter
        p.viewport.frame = host.view.bounds
        session.firstDrawMs = ExactEnv.wall()
        func frame(_ id: Int, _ height: Double = 874) -> [String: Any] {
            ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 402.0, "h": height]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "home"]], frame(1),
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "home"]], frame(2),
            ["op": "create", "id": 20, "kind": "view", "props": ["semanticTag": "dialog", "id": "form"]], frame(20, 180),
            ["op": "create", "id": 21, "kind": "input", "props": [:]], frame(21, 40),
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 2, "ids": []],
            ["op": "children", "id": 1, "ids": [2, 20]], ["op": "roots", "ids": [1]],
        ]))
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "props": ["navigationKey": "old", "navigationPresentation": "modal"]], frame(4),
            ["op": "children", "id": 1, "ids": [2, 20, 4]],
            ["op": "props", "id": 1, "set": ["navigationKey": "old"]],
        ]))
        until("the old route is presented") { presented().count == 1 && !p.modals.inTransition }
        p.dialogCommand("showModal", "form")
        until("the page dialog is over the old route") { presented().count == 2 && !p.menus.inTransition }
        let dialog = try XCTUnwrap(p.views[20])
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "view", "props": ["navigationKey": "replacement", "navigationPresentation": "modal"]], frame(6),
            ["op": "children", "id": 1, "ids": [2, 20, 6]],
            ["op": "destroy", "id": 4],
            ["op": "props", "id": 1, "set": ["navigationKey": "replacement"]],
        ]))
        until("the replacement route and restored dialog are both presented") {
            p.modals.routes.last?.node.id == 6 && presented().count == 2 && !p.modals.inTransition && !p.menus.inTransition
        }
        XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(p.menus.sheets.top?.presentingViewController === p.modals.owner)
        XCTAssertEqual(p.navigation.selectedKey, "replacement")
    }

}
#endif
