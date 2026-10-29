#if os(iOS) && EXACT_INSPECT
import XCTest
@testable import ExactKit
import UIKit

/// Inspect mode's hit test (LLP 1122 D5, T2c; InspectIOS.swift): a tap
/// selects the nearest view, walking up from the window's own hit view, that
/// stands for a plan node, and the walk stops at a system bar. A plan node
/// the bar hosts (the drawn title) is selected; UIKit's own parts (Back, the
/// bar's material) select nothing, and say so.
final class InspectIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        Inspect.set(false, seq: nil)
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

    /// The native fixture's plan, booted on Home.
    private func home() throws -> (ExactSession, UIWindow) {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixtures' plans")))
        let session = ExactApp.shared.makeSession(label: "inspect")
        sessions.append(session)
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
        return (session, window)
    }

    /// The native fixture's plan, Chat pushed on Home's stack.
    private func chat() throws -> (ExactSession, UIWindow, UINavigationController, RouteController) {
        let (session, window) = try home()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let chat = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "chat" })
        _ = Agent(session: session).tap(["id": Int(chat.id)])
        until("Chat pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        return (session, window, nav, try XCTUnwrap(nav.topViewController as? RouteController))
    }

    func testATapSelectsTheNodeItLandsOnAndTheBarsDrawnTitleButNotUIKitsBack() throws {
        let (session, window, nav, route) = try chat()
        // Chat's bar takes touches after its push (iPadOS once left it with none: no Back, no title).
        XCTAssertTrue(nav.navigationBar.isUserInteractionEnabled)
        // Content: the node under the point, the nearest that stands for a plan node.
        // (One whose middle the window's hit test gives to it, or to a part of
        // it that stands for no other node.)
        let content = try XCTUnwrap(session.presenter.views.values.first { node in
            guard node.window === window, !node.isHidden, node.bounds.width > 0, node.bounds.height > 0, !node.isDescendant(of: nav.navigationBar) else { return false }
            let box = node.convert(node.bounds, to: window)
            var hit = window.hitTest(CGPoint(x: box.midX, y: box.midY), with: nil)
            while let v = hit, !(v is NodeView) { hit = v.superview }
            return hit === node
        })
        let box = content.convert(content.bounds, to: window)
        let landed = InspectHost.target(at: CGPoint(x: box.midX, y: box.midY), in: window, ExactApp.shared)
        XCTAssertEqual(landed.id, content.id, "\(landed)")
        // The drawn title in the bar: the heading's group, a plan node the bar hosts.
        let title = try XCTUnwrap(route.navigationItem.titleView as? HeaderTitleView)
        let titleBox = title.convert(title.bounds, to: window)
        let picked = InspectHost.target(at: CGPoint(x: titleBox.midX, y: titleBox.midY), in: window, ExactApp.shared)
        XCTAssertNotNil(picked.id, "the bar's drawn title is the app's: \(picked)")
        XCTAssertEqual(picked.id, title.groupId)
        // Back, UIKit's own: nothing, said, and the walk does not climb out of the bar.
        let bar = nav.navigationBar.convert(nav.navigationBar.bounds, to: window)
        let back = InspectHost.target(at: CGPoint(x: bar.minX + 22, y: bar.midY), in: window, ExactApp.shared)
        XCTAssertNil(back.id, "Back is UIKit's, not a view of the app: \(back)")
        XCTAssertEqual(back.why, "That is part of UIKit's navigation bar, not a view of the app.")
    }

    /// Whether a view is UIKit's tab bar chrome: the bar, or on an iPad in a
    /// regular width (iPadOS 18), the floating bar that stands in for it at
    /// the top, a part of the container outside the selected tab's view.
    private func tabChrome(_ v: UIView, _ tabs: UITabBarController) -> Bool {
        if tabs.tabBar.window != nil { return v.isDescendant(of: tabs.tabBar) }
        guard let selected = tabs.selectedViewController?.view else { return false }
        return v.isDescendant(of: tabs.view) && !v.isDescendant(of: selected)
    }

    func testATapOnTheTabBarSelectsTheTabUnderItAndItsEmptySpaceNothing() throws {
        let (session, window) = try home()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let root = try XCTUnwrap(session.presenter.navigation.container)
        let tablist = try XCTUnwrap(NavigationTabs.of(root, session.presenter))
        // Each item's box, in order (no accessibility client runs here to make
        // elements): the bar's buttons, or the floating bar's item cells.
        let floating = tabs.tabBar.window == nil
        var buttons: [CGRect] = [], pending: [UIView] = floating ? [tabs.view] : [tabs.tabBar]
        var bars: [UIView] = []
        while !pending.isEmpty {
            let v = pending.removeFirst()
            guard tabChrome(v, tabs) || v === tabs.view else { continue }
            if floating ? v is UICollectionViewCell : v is UIControl {
                let b = v.convert(v.bounds, to: window).integral
                if !buttons.contains(b) { buttons.append(b) }
                if let list = v.superview, !bars.contains(list) { bars.append(list) }
            } else { pending += v.subviews }
        }
        buttons.sort { $0.minX < $1.minX }
        XCTAssertEqual(buttons.count, 2, "the fixture's two tabs show in UIKit's tab bar")
        let items = zip(tablist.tabs, buttons).map { (id: $0.id, box: $1) }
        let second = try XCTUnwrap(items.last)
        let picked = InspectHost.target(at: CGPoint(x: second.box.midX, y: second.box.midY), in: window, ExactApp.shared)
        XCTAssertEqual(picked.id, second.id, "\(picked)")
        // On an iPad the floating bar covers what runs under it, for reveal.
        if let list = tabs.floatingTabList, let scroller = session.presenter.views.values.compactMap(\.scrollView).first(where: { $0.window === window }) {
            XCTAssertTrue(Agent(session: session).bars(over: scroller).contains(list))
        }
        // Its box is the item's, for the press and the outline.
        let node = try XCTUnwrap(session.presenter.views[second.id])
        let place = try XCTUnwrap(InspectHost.place(of: node, in: session), "a tab has a place")
        XCTAssertEqual(try XCTUnwrap(place(), "the item's box").integral, second.box)
        // The bar's own space, on no item: nothing, said.
        let bar = floating ? try XCTUnwrap(bars.first, "the floating bar's list") : tabs.tabBar
        let box = bar.convert(bar.bounds, to: window)
        var empty: CGPoint?
        for y in stride(from: box.minY + 2, to: box.maxY, by: 4) { for x in stride(from: box.minX + 2, to: box.maxX, by: 4) {
            let p = CGPoint(x: x, y: y)
            guard empty == nil, !items.contains(where: { $0.box.contains(p) }), let hit = window.hitTest(p, with: nil), tabChrome(hit, tabs) else { continue }
            empty = p
        } }
        let space = try XCTUnwrap(empty, "the tab bar has space outside its items")
        let none = InspectHost.target(at: space, in: window, ExactApp.shared)
        XCTAssertNil(none.id, "\(none)")
        XCTAssertEqual(none.why, "That is part of UIKit's tab bar, not a view of the app.")
    }

    func testInspectModeEndsEditingAndTheStatusBarsScrollToTopTillItEnds() throws {
        let (session, window, _, route) = try chat()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let field = UITextField(frame: CGRect(x: 20, y: 300, width: 200, height: 40))
        route.view.addSubview(field)
        defer { field.removeFromSuperview() }
        XCTAssertTrue(field.becomeFirstResponder())
        func toTop() -> [UIScrollView] {
            var found: [UIScrollView] = [], pending: [UIView] = [window]
            while !pending.isEmpty { let v = pending.removeFirst(); if let s = v as? UIScrollView, s.scrollsToTop, !(s is UITextView), !s.isDescendant(of: field), !tabChrome(s, tabs) { found.append(s) }; pending += v.subviews }
            return found
        }
        // (Not counting UIKit's own: an iPad's floating tab bar is a list of
        // its items, a scroll view that fits. The one-scroller rule is the
        // iPhone's, UIScrollView.h says; the app's scrollers are all counted.)
        let before = toTop()
        XCTAssertEqual(before.count, 1, "the route's content scroller takes the status-bar tap")
        Inspect.set(true, seq: 1)
        InspectHost.show(ExactApp.shared, on: true)
        XCTAssertFalse(field.isFirstResponder, "the keyboard goes: no typing reaches the app")
        XCTAssertEqual(toTop(), [], "a status-bar tap scrolls nothing")
        session.presenter.syncScrollsToTop() // (as after any batch)
        XCTAssertEqual(toTop(), [])
        Inspect.set(false, seq: 1)
        InspectHost.show(ExactApp.shared, on: false)
        XCTAssertEqual(toTop(), before)
    }

    func testInspectSelectionsReportTheRunnersNodeWithAClickTokenOfTheSession() throws {
        let (session, window, _, _) = try chat()
        Inspect.set(true, seq: 3)
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.window === window && !$0.isHidden && $0.props["testId"] != nil })
        let body = try XCTUnwrap(Inspect.select(session, id: node.id, clicked: true))
        XCTAssertEqual(body["kind"] as? String, "selected")
        XCTAssertEqual(body["inspect"] as? Int, 3)
        let selected = try XCTUnwrap(body["selected"] as? [String: Any])
        XCTAssertEqual(selected["node"] as? Int, Int(node.id))
        XCTAssertNotNil(selected["planDigest"] as? String, "the runner's own node, with its plan's digest")
        XCTAssertFalse((selected["rows"] as? [Any] ?? []).isEmpty)
        XCTAssertEqual(selected["planDigest"] as? String, BuildChannel.planDigest(ExactApp.shared), "status and selections name the plan from one source, the runner")
        let click = try XCTUnwrap(body["click"] as? String)
        XCTAssertTrue(Inspect.answersClick(["click": click]), "the outline answering it scrolls nothing")
        XCTAssertFalse(Inspect.answersClick(["click": "elsewhere.1"]))
        // Off: the session's tokens end with it.
        Inspect.set(false, seq: 3)
        XCTAssertFalse(Inspect.answersClick(["click": click]))
        XCTAssertNil(Inspect.select(session, id: node.id, clicked: true), "no selection outside inspect mode")
    }

    /// `select {up}` (tamber/1.17) by the runner's own tree and the plan's copies, as the web pages' and the Mac's
    /// (InspectMacTests, over apps/select-walk-fixture's shapes): each new copy owning an ancestor a step, at step `up`
    /// that copy's outermost node, past the last the root, with `moved`; an ask for a selection that moved, or with no
    /// copies for its plan, is nil, the selection as it is now. (The copies here stand in for the server's map: every
    /// site its own, except the selection's two nearest ancestor sites, one copy.)
    func testSelectUpWalksTheTreeByCopiesToTheOutermostOfTheCopyAndAStaleAskOnNone() throws {
        let (session, window, _, _) = try chat()
        Inspect.set(true, seq: 4)
        defer { Inspect.set(false, seq: 4); Inspect.copies = nil }
        func above(_ id: UInt32) -> [(view: Int, site: Int)] {
            let o = (try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"ancestors\",\"id\":\(id)}").utf8))) as? [String: Any]
            return (o?["ancestors"] as? [[Any]] ?? []).dropFirst().compactMap { p in (p[0] as? Int).flatMap { v in (p[1] as? Int).map { (v, $0) } } }
        }
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.window === window && !$0.isHidden && $0.props["testId"] != nil && Set(above($0.id).map(\.site)).count >= 3 })
        let ancestors = above(node.id)
        let selected = try XCTUnwrap(Inspect.select(session, id: node.id, clicked: true)?["selected"] as? [String: Any])
        let first = ancestors[0].site, second = try XCTUnwrap(ancestors.first { $0.site != first }).site
        let sites = ancestors.map(\.site) + [(selected["site"] as? Int) ?? 0]
        var list = Array(0...(sites.max() ?? 0))
        list[second] = list[first]
        func ask(_ up: Int, plan: Any? = nil, instance: Any? = nil) -> [String: Any] {
            ["up": up, "plan": plan ?? selected["planDigest"] ?? NSNull(), "from": selected["site"] ?? NSNull(), "instance": instance ?? selected["instance"] ?? []]
        }
        Inspect.copies = ("another plan", list)
        XCTAssertNil(Inspect.selectUp(session, ask(1)), "no copies for this plan")
        Inspect.copies = (selected["planDigest"] as? String, list)
        XCTAssertNil(Inspect.selectUp(session, ask(1, plan: "another plan")), "asked of another plan's selection")
        XCTAssertNil(Inspect.selectUp(session, ask(1, instance: [["key": "elsewhere"]])), "asked of another copy")
        var moved = ask(1); moved["from"] = -1
        XCTAssertNil(Inspect.selectUp(session, moved), "asked of a selection at another site")
        XCTAssertEqual(Inspect.selected?.id, node.id, "a stale ask leaves the selection")
        let body = try XCTUnwrap(Inspect.selectUp(session, ask(1)))
        let outermost = try XCTUnwrap(ancestors.last { $0.site == first || $0.site == second })
        XCTAssertEqual((body["selected"] as? [String: Any])?["node"] as? Int, outermost.view, "\(ancestors)")
        XCTAssertEqual((body["selected"] as? [String: Any])?["moved"] as? [String: Int], ["up": 1, "stepped": 1])
        XCTAssertNil(body["click"], "a move is no press: its outline scrolls the copy into view")
        XCTAssertEqual(Inspect.selected?.id, UInt32(outermost.view))
        // Past every copy above it: the root, the steps it took said.
        _ = Inspect.select(session, id: node.id, clicked: true)
        let rooted = try XCTUnwrap(Inspect.selectUp(session, ask(64)))
        XCTAssertEqual((rooted["selected"] as? [String: Any])?["node"] as? Int, ancestors.last?.view, "\(ancestors)")
        XCTAssertLessThan(((rooted["selected"] as? [String: Any])?["moved"] as? [String: Int])?["stepped"] ?? 64, 64)
        // A `select` that is no object (the web pages take any but null or false) is no move, and leaves inspect on.
        let was = Inspect.selected?.id
        ExactApp.shared.inspectCommand(["kind": "inspect", "select": "up"])
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue(Inspect.on, "a select that is no object never turns inspect mode off")
        XCTAssertEqual(Inspect.selected?.id, was)
    }
}
#endif
