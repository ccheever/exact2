#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// Keyboard avoidance and Next-field navigation in native grouped lists
/// (LLP 1116.003 D2), including pinned controls (LLP 1116.008 D8).
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

    private func hit(atCenterOf view: UIView) -> UIView? {
        let center = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
        return window?.hitTest(view.convert(center, to: window), with: nil)
    }

    private func assertReachable(_ testId: String, below bar: CGFloat, file: StaticString = #filePath, line: UInt = #line) throws {
        let v = try node(testId), r = box(v)
        let keys = try box(XCTUnwrap(session?.presenter.viewport)).maxY
        XCTAssertLessThanOrEqual(keys, self.keys + 0.5, "the viewport ends at the keys", file: file, line: line)
        XCTAssertLessThanOrEqual(r.maxY, keys + 0.5, "\(testId) above the keys: \(r)", file: file, line: line)
        XCTAssertGreaterThanOrEqual(r.minY, bar - 0.5, "\(testId) below the bar: \(r)", file: file, line: line)
        let hit = hit(atCenterOf: v)
        XCTAssertTrue(hit?.isDescendant(of: v) == true, "\(testId) takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
    }

    func testAFocusedFieldAndItsAddButtonStayAboveTheKeyboardInATabsGroupedList() throws {
        let session = try fixture()
        let p = session.presenter
        let tabs = try XCTUnwrap(p.navigation.tabController, "the tabs are a UITabBarController")
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let reps = try node("reps"), add = try node("add-set")
        XCTAssertGreaterThan(box(add).minY, keys, "the fixture's Add button starts where the keys will be")
        let resting = box(tabs.tabBar)
        reps.textFieldDidBeginEditing(try XCTUnwrap(reps.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        spin(0.3)
        let bar = box(nav.navigationBar).maxY
        try assertReachable("reps", below: bar)
        try assertReachable("add-set", below: bar)
        XCTAssertEqual(box(p.viewport).maxY, keys, accuracy: 0.5, "the viewport ends at the keys")
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar stays at the bottom, under the keys: \(box(tabs.tabBar))")

        let field = try XCTUnwrap(reps.field)
        XCTAssertTrue(field.becomeFirstResponder(), "the test host's window takes a first responder")
        _ = reps.textFieldShouldReturn(field)
        spin(0.4)
        let weight = try node("weight")
        XCTAssertEqual(weight.field?.isFirstResponder, true, "Next moved to Weight")
        try assertReachable("weight", below: bar)
        try assertReachable("add-set", below: bar)
        let bench = try node("bench-reps")
        // `layout form-list native 8`, retained if the test fails.
        let hierarchy = Agent(session: session).nativeSubviews(try node("form-list").id, depth: 8, limit: 200, plan: false)
        let data = try JSONSerialization.data(withJSONObject: hierarchy, options: [.prettyPrinted, .sortedKeys])
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
        attachment.name = "Native form-list hierarchy before Next"
        self.add(attachment)
        XCTAssertNotNil(bench.window, "the clipped field is still mounted")
        XCTAssertFalse(bench.bounds.isEmpty, "the offscreen field has laid-out bounds")
        XCTAssertGreaterThanOrEqual(box(bench).minY, box(p.viewport).maxY,
                                    "the next field starts below the viewport, so Next must reveal it")
        XCTAssertFalse(hit(atCenterOf: bench)?.isDescendant(of: bench) == true,
                       "the offscreen field must not receive a touch before Next")
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

    func testTheFocusedFieldIsShownAboveABarPinnedOverItsScroller() throws {
        let session = try fixture()
        let p = session.presenter
        _ = Agent(session: session).tap(["id": Int(try node("tab-pinned").id)])
        spin(0.5)
        let tabs = try XCTUnwrap(p.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let author = try node("pinned-author"), memo = try node("pinned-memo"), bar = try node("pinned-bar")
        let scroll = try XCTUnwrap(p.navigation.shownScroll(try node("pinned-scroll")))
        let base = scroll.contentInset.bottom
        XCTAssertGreaterThan(box(memo).minY, keys, "the fixture's last field starts where the keys will be")
        let top = box(nav.navigationBar).maxY

        func assertShown(_ field: NodeView, _ name: String, file: StaticString = #filePath, line: UInt = #line) {
            let r = box(field), cover = box(bar)
            XCTAssertLessThanOrEqual(r.maxY, cover.minY + 0.5, "\(name) above the pinned bar: \(r) over \(cover)", file: file, line: line)
            XCTAssertGreaterThanOrEqual(r.minY, top - 0.5, "\(name) below the navigation bar: \(r)", file: file, line: line)
            XCTAssertLessThanOrEqual(cover.maxY, keys + 0.5, "the bar rides the keys' top", file: file, line: line)
            let hit = hit(atCenterOf: field)
            XCTAssertTrue(hit?.isDescendant(of: field) == true, "\(name) takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
        }

        memo.textFieldDidBeginEditing(try XCTUnwrap(memo.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        spin(0.3)
        assertShown(memo, "memo")
        XCTAssertGreaterThan(scroll.contentInset.bottom, base, "room below the content for the field at its end")

        let room = scroll.contentInset.bottom
        author.textFieldDidBeginEditing(try XCTUnwrap(author.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        spin(0.3)
        assertShown(author, "author")
        XCTAssertLessThanOrEqual(scroll.contentInset.bottom, room + 0.5, "no more room than it had")

        memo.field?.resignFirstResponder()
        author.field?.resignFirstResponder()
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        spin(0.3)
        XCTAssertEqual(scroll.contentInset.bottom, base, accuracy: 0.5, "the room goes back with the keyboard")
        let high = max(-scroll.adjustedContentInset.top, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
        XCTAssertLessThanOrEqual(scroll.contentOffset.y, high + 0.5, "and the offset is within the content again")
    }
}
#endif
