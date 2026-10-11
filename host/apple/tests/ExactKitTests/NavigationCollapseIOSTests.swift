#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 Stage 3: a large title collapses with its route's content
/// scroll view (the fixture's Second tab). CSS `scrollTop` counts from the
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

    func testAnAuthoredScrollTopLandsAsTheBrowsersWhileTheTitleCollapses() throws {
        let (session, nav) = try second("collapse")
        let node = try self.node(session, "list-second")
        let sv = try XCTUnwrap(node.scroll)
        spin(0.3)
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
    /// below the bar, and the title collapsed over the page before expands.
    func testAScrollerReplacedInPlaceOpensAtItsTopBelowTheBar() throws {
        let (session, nav) = try second("collapse-replaced")
        let list = try node(session, "list-second")
        spin(0.3)
        let expanded = nav.navigationBar.frame.height
        list.pendingScrollTop = 100_000
        list.applyPendingScroll()
        spin(0.3)
        XCTAssertLessThan(nav.navigationBar.frame.height, expanded, "the title collapsed over the first page's end")
        for (press, page) in [("second-taller", "taller"), ("second-shorter", "shorter")] {
            _ = Agent(session: session).tap(["id": Int(try node(session, press).id)])
            until("the \(page) scroller in place") { (try? self.node(session, "list-second-\(page)")) != nil }
            spin(0.3)
            let sv = try XCTUnwrap(try node(session, "list-second-\(page)").scroll)
            let top = try node(session, "second-\(page)-top"), bar = nav.navigationBar
            XCTAssertEqual(sv.contentOffset.y + sv.adjustedContentInset.top, 0, accuracy: 0.5, "the \(page) page at its top (CSS 0)")
            XCTAssertEqual(top.convert(top.bounds, to: nil).minY, bar.convert(bar.bounds, to: nil).maxY + 12, accuracy: 0.5,
                           "its first line just below the bar, past its 12 of padding")
            XCTAssertEqual(bar.frame.height, expanded, accuracy: 0.5, "the title expanded, as a new page's is")
        }
    }
}
#endif
