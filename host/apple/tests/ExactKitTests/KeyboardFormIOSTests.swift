#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1116.003 D2.1 over `scripts/fixtures/keyboard-form.contract`:
/// workout-log's inline set form at the foot of a tab's grouped list. A
/// focused number field and the Add button under it stay above the keyboard,
/// below the bar and tappable, as in a UIKit table or a SwiftUI `Form`; the
/// floating tab bar stays under the keys rather than riding above them; and
/// Return with `enterkeyhint="next"` moves to the next field, a row the list
/// had scrolled away included, and shows it likewise. D2.2: a grouped list
/// taking a route's scroll's place in place is what the bar follows.
final class KeyboardFormIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    /// The keys' top: an iPhone 18 Pro's number pad, 302 points over the
    /// screen's bottom.
    private let keys: CGFloat = 572

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_KEYBOARD_PLAN"], "build.mjs --test --ios compiles the keyboard form")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "keyboard-form")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func box(_ v: UIView) -> CGRect { v.convert(v.bounds, to: nil) }

    /// Shown between the bar and the keys, and a touch at its middle reaches
    /// it. The keys' top is where the viewport ends (`resizes-content`): the
    /// one applied, or a real keyboard's once a field is first responder.
    private func assertReachable(_ testId: String, below bar: CGFloat, file: StaticString = #filePath, line: UInt = #line) throws {
        let v = try node(testId), r = box(v)
        let keys = try box(XCTUnwrap(session?.presenter.viewport)).maxY
        XCTAssertLessThanOrEqual(keys, self.keys + 0.5, "the viewport ends at the keys", file: file, line: line)
        XCTAssertLessThanOrEqual(r.maxY, keys + 0.5, "\(testId) above the keys: \(r)", file: file, line: line)
        XCTAssertGreaterThanOrEqual(r.minY, bar - 0.5, "\(testId) below the bar: \(r)", file: file, line: line)
        let hit = window?.hitTest(CGPoint(x: r.midX, y: r.midY), with: nil)
        XCTAssertTrue(hit.map { $0 === v || $0.isDescendant(of: v) } ?? false, "\(testId) takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
    }

    func testAFocusedFieldAndItsAddButtonStayAboveTheKeyboardInATabsGroupedList() throws {
        let session = try fixture()
        let p = session.presenter
        let tabs = try XCTUnwrap(p.navigation.tabController, "the tabs are a UITabBarController")
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let reps = try node("reps"), add = try node("add-set")
        XCTAssertGreaterThan(box(add).minY, keys, "the fixture's Add button starts where the keys will be")
        let resting = box(tabs.tabBar)
        // Focused, as UIKit tells a field that becomes first responder, and
        // the keys come up.
        reps.textFieldDidBeginEditing(try XCTUnwrap(reps.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        spin(0.3)
        let bar = box(nav.navigationBar).maxY
        try assertReachable("reps", below: bar)
        try assertReachable("add-set", below: bar)
        XCTAssertEqual(box(p.viewport).maxY, keys, accuracy: 0.5, "the viewport ends at the keys")
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar stays at the bottom, under the keys: \(box(tabs.tabBar))")

        // Return on Reps (`next`) moves to Weight, and on Weight to the next
        // exercise's Reps, whose row the list had scrolled away.
        let field = try XCTUnwrap(reps.field)
        XCTAssertTrue(field.becomeFirstResponder(), "the test host's window takes a first responder")
        _ = reps.textFieldShouldReturn(field)
        spin(0.4)
        let weight = try node("weight")
        XCTAssertEqual(weight.field?.isFirstResponder, true, "Next moved to Weight")
        try assertReachable("weight", below: bar)
        try assertReachable("add-set", below: bar)
        let bench = try node("bench-reps")
        XCTAssertTrue(sequence(first: bench as UIView, next: \.superview).contains { $0.isHidden }, "the next exercise's row is out of the list's port, beneath it")
        _ = weight.textFieldShouldReturn(try XCTUnwrap(weight.field))
        spin(0.4)
        XCTAssertEqual(bench.field?.isFirstResponder, true, "Next moved on to the next section's field")
        try assertReachable("bench-reps", below: bar)
        try assertReachable("bench-add", below: bar)
        bench.field?.resignFirstResponder()
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        spin(0.3)
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar where it was before the keys")
    }

    /// LLP 1116.003 D2.2: a route's empty `scroll` giving way in place to a
    /// grouped list (workout-log's active workout): the bar follows the list
    /// UIKit shows, its collection view, at both edges. It was handed the
    /// list's hidden authored scroll, in the batch before the collection view
    /// was made, and the large title drew nothing after; the bottom edge
    /// kept the hidden scroll for good.
    func testTheBarFollowsAGroupedListThatReplacesTheRoutesScrollInPlace() throws {
        let session = try fixture()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        _ = Agent(session: session).tap(["id": Int(try node("tab-other").id)])
        spin(0.5)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let route = try XCTUnwrap(nav.topViewController)
        XCTAssertEqual(route.navigationItem.title, "Other")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertTrue(route.contentScrollView(for: .top) is ScrollView, "the empty state's scroll")
        _ = Agent(session: session).tap(["id": Int(try node("fill").id)])
        spin(0.5)
        XCTAssertNotNil(try? node("other-list"), "the list took the scroll's place")
        for edge: NSDirectionalRectEdge in [.top, .bottom] {
            let shown = route.contentScrollView(for: edge)
            XCTAssertTrue(shown is GroupedScroller, "edge \(edge.rawValue) follows the list's collection view, not \(String(describing: shown))")
        }
    }
}
#endif
