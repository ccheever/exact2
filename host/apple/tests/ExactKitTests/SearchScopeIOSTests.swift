#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1134: a filter tablist right after a header whose search field the
/// bar projects is that search bar's scope bar (`scripts/fixtures/search-scope.contract`).
final class SearchScopeIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func fixture() throws -> (ExactSession, RouteController) {
        let plan = try fixturePlan("SEARCH_SCOPE")
        let session = ExactApp.shared.makeSession(label: "search-scope")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the route's search is the bar's") { (session.presenter.navigation.primaryNavigation?.topViewController as? RouteController)?.search != nil }
        return (session, try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController as? RouteController))
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func state(_ session: ExactSession, _ slot: String) -> Any? {
        let json = try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
        return (json?["slots"] as? [String: Any])?[slot] ?? json?[slot]
    }

    func testAFilterAfterTheSearchIsItsScopeBar() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bar is a scene window's") }
        guard UIDevice.current.userInterfaceIdiom == .phone else { throw XCTSkip("the scope bar is the iPhone's (D5)") }
        let (session, route) = try fixture()
        let p = session.presenter, filter = try node(session, "filter"), list = try node(session, "list")
        let search = try XCTUnwrap(route.search), bar = search.controller.searchBar
        until("the tabs are the scope buttons") { bar.scopeButtonTitles == ["All", "Mine", "Done"] }
        XCTAssertTrue(bar.showsScopeBar)
        XCTAssertEqual(search.controller.scopeBarActivation, .manual, "shown whether or not the search is active")
        XCTAssertEqual(bar.selectedScopeButtonIndex, 0)
        XCTAssertTrue(filter.isHidden, "the tablist is the bar's")
        XCTAssertTrue(p.navigation.hiddenForBar(filter))
        XCTAssertFalse(p.segments.projects(filter.id), "no segmented control of its own")
        XCTAssertTrue(route.collapseScroll === list, "the list after it is the bar's collapse scroller")
        until("it takes no room: the list starts where the tablist did") { abs(list.frame.minY - filter.frame.minY) < 0.5 }
        // D4: each segment carries its tab's test id; the agent aims at it.
        until("the segments carry the tabs' test ids") { search.scopeControl?.segmentViews.map(\.accessibilityIdentifier) == ["filter-all", "filter-mine", "filter-done"] }
        XCTAssertNotNil(p.segments.drawn(try node(session, "filter-done")), "a tap aims at the scope segment")
        let tab = try node(session, "filter-done"), agent = Agent(session: session)
        let row = try XCTUnwrap((agent.layout()["nodes"] as? [[String: Any]])?.first { $0["id"] as? Int == Int(tab.id) })
        XCTAssertNil(row["radii"], "a native segment has no authored corner geometry")
        let target = try XCTUnwrap(agent.layout(["id": Int(tab.id)])["node"] as? [String: Any])
        XCTAssertNil(target["radii"], "list and targeted layout omit the same native corners")


        // D3: a person's choice presses the tab, once.
        bar.selectedScopeButtonIndex = 2
        bar.delegate?.searchBar?(bar, selectedScopeButtonIndexDidChange: 2)
        until("the press chose Done") { state(session, "filter") as? String == "done" }
        XCTAssertEqual(state(session, "presses") as? Int, 1)
        XCTAssertEqual(bar.selectedScopeButtonIndex, 2)
        // The Contract's selection moves the segment, and presses nothing more.
        p.press(try node(session, "filter-mine").id)
        until("the segment follows the Contract") { bar.selectedScopeButtonIndex == 1 }
        XCTAssertEqual(state(session, "presses") as? Int, 2)
        // While the search is active, with the keyboard up, the text and the responder stay.
        search.controller.isActive = true
        until("the search is active") { search.controller.isActive }
        bar.text = "dune"
        bar.delegate?.searchBar?(bar, textDidChange: "dune")
        XCTAssertTrue(bar.searchTextField.becomeFirstResponder())
        p.press(try node(session, "filter-all").id)
        until("the segment moves while the search is active") { bar.selectedScopeButtonIndex == 0 }
        XCTAssertTrue(bar.searchTextField.isFirstResponder, "the keyboard stays")
        XCTAssertEqual(bar.text, "dune")
        XCTAssertTrue(search.controller.isActive)
        XCTAssertTrue(bar.showsScopeBar, "still shown while active")
        search.controller.isActive = false
        until("the search is dismissed") { !search.controller.isActive }
        until("still shown after the search") { bar.showsScopeBar }

        // A press the Contract declines puts the segment back.
        p.press(try node(session, "pick-stay").id)
        until("still the scope bar") { bar.scopeButtonTitles?.count == 3 }
        let before = state(session, "presses") as? Int ?? 0
        bar.selectedScopeButtonIndex = 2
        bar.delegate?.searchBar?(bar, selectedScopeButtonIndexDidChange: 2)
        until("the declined choice goes back") { bar.selectedScopeButtonIndex == 0 }
        XCTAssertEqual(state(session, "presses") as? Int, before + 1)
        XCTAssertEqual(state(session, "filter") as? String, "all")
    }

    /// LLP 1135: with no search, the filter under a large title is the
    /// item's subtitle views, under the large title and under the inline one.
    func testAFilterUnderALargeTitleIsItsSubtitleViews() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bar is a scene window's") }
        guard UIDevice.current.userInterfaceIdiom == .phone else { throw XCTSkip("the iPhone's (LLP 1135 D4)") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("the subtitle views are iOS 26's") }
        let (session, route) = try fixture()
        let p = session.presenter, filter = try node(session, "filter"), list = try node(session, "list")
        p.press(try node(session, "pick-plain").id)
        until("the filter is the subtitle views") { route.navigationItem.largeSubtitleView is UISegmentedControl && route.navigationItem.subtitleView is UISegmentedControl }
        let large = try XCTUnwrap(route.navigationItem.largeSubtitleView as? UISegmentedControl)
        let inline = try XCTUnwrap(route.navigationItem.subtitleView as? UISegmentedControl)
        for control in [large, inline] {
            XCTAssertEqual((0..<control.numberOfSegments).map { control.titleForSegment(at: $0) }, ["All", "Mine", "Done"])
            XCTAssertEqual(control.selectedSegmentIndex, 0)
        }
        XCTAssertNil(route.search?.controller.searchBar.scopeButtonTitles, "no scope bar: there is no search")
        XCTAssertTrue(filter.isHidden && p.navigation.hiddenForBar(filter), "the tablist is the bar's")
        XCTAssertFalse(p.segments.projects(filter.id))
        until("the list is the collapse scroller") { route.collapseScroll === list }
        // A choice under the inline title presses its tab once and sets the other.
        let before = state(session, "presses") as? Int ?? 0
        inline.selectedSegmentIndex = 2
        inline.sendActions(for: .valueChanged)
        until("Done is chosen") { state(session, "filter") as? String == "done" }
        XCTAssertEqual(state(session, "presses") as? Int, before + 1)
        XCTAssertEqual(large.selectedSegmentIndex, 2, "the other follows")
        // The Contract's selection moves both.
        p.press(try node(session, "filter-mine").id)
        until("both follow the Contract") { large.selectedSegmentIndex == 1 && inline.selectedSegmentIndex == 1 }
        // A search back: the scope bar, and the subtitle views go.
        p.press(try node(session, "pick-scope").id)
        until("the subtitle views go") { route.navigationItem.largeSubtitleView == nil && route.navigationItem.subtitleView == nil }
        until("the scope bar is back") { route.search?.controller.searchBar.scopeButtonTitles?.count == 3 }
    }

    func testAFilterThatIsNotAScopeKeepsItsSegmentedControl() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bar is a scene window's") }
        guard UIDevice.current.userInterfaceIdiom == .phone else { throw XCTSkip("the scope bar is the iPhone's (D5)") }
        let (session, route) = try fixture()
        let p = session.presenter, filter = try node(session, "filter"), bar = try XCTUnwrap(route.search).controller.searchBar
        until("the scope bar first") { bar.scopeButtonTitles?.count == 3 }
        for shape in ["image", "menu", "panel", "painted", "faded", "caption", "six"] {
            p.press(try node(session, "pick-\(shape)").id)
            until("\(shape): the scope bar goes") { bar.scopeButtonTitles == nil && !bar.showsScopeBar }
            until("\(shape): the tablist is the page's again") { !filter.isHidden && p.segments.projects(filter.id) }
            XCTAssertFalse(p.navigation.hiddenForBar(filter), shape)
            p.press(try node(session, "pick-scope").id)
            until("\(shape) → scope: the scope bar again") { bar.scopeButtonTitles?.count == 3 && filter.isHidden && !p.segments.projects(filter.id) }
        }
    }
}
#endif
