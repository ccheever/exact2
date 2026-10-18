#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1130: a page with no native container keeps the area it is laid out
/// in, the view less its safe area, and is drawn in a viewport that takes
/// the whole view, so it scrolls under the status bar and the home
/// indicator; a scroller at the area's top and foot reaches into those bands
/// with its content where it was and its `scrollTop` unchanged.
final class PageEdgesIOSTests: XCTestCase {
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

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// `scripts/fixtures/page-edges.contract` in a window of the test host's
    /// scene, the screen's size, so its safe area is the device's.
    private func fixture() throws -> (ExactSession, ExactView) {
        let plan = try fixturePlan("PAGE_EDGES")
        let session = ExactApp.shared.makeSession(label: "page-edges")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the page is laid out") { (try? node(session, "list"))?.scroll != nil }
        view.fit()
        return (session, view)
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    func testThePageKeepsItsAreaAndIsDrawnPastIt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let p = session.presenter, safe = view.safeAreaInsets
        XCTAssertGreaterThan(safe.top, 0, "the test host's window has a status bar")
        XCTAssertEqual(p.viewport.frame, view.bounds, "the viewport takes the whole view")
        XCTAssertEqual(p.viewportSize, view.bounds.inset(by: safe).size, "the kernel lays the page out in its area, as before")
        XCTAssertEqual(p.pageMargins.top, safe.top)
        XCTAssertEqual(p.pageMargins.bottom, safe.bottom)
        XCTAssertEqual(p.root.frame.minY, safe.top, "the document starts at the area's top")
        XCTAssertEqual(p.viewport.contentSize.height, view.bounds.height, accuracy: 0.5, "a page that fits its area does not scroll")
        XCTAssertEqual(p.viewport.contentOffset, .zero, "and rests at offset zero, its scrollTop")
        let top = try node(session, "list-top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, safe.top, accuracy: 0.5, "the content shows where it did")
    }

    func testTheScrollerAtTheEdgesReachesIntoTheBands() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let safe = view.safeAreaInsets, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        XCTAssertEqual(list.pageBand, UIEdgeInsets(top: safe.top, left: 0, bottom: safe.bottom, right: 0))
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).minY, 0, accuracy: 0.5, "its scroll view starts at the screen's top")
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).maxY, view.bounds.maxY, accuracy: 0.5, "and ends at its foot")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 0, accuracy: 0.5, "scrollTop 0 is the rest under the band")
        let top = try node(session, "list-top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, safe.top, accuracy: 0.5, "its content shows where it did")
        // The CSS port is the box: the band is neither port nor content.
        XCTAssertEqual(sv.bounds.height - list.scrollPortInsets(sv).top - list.scrollPortInsets(sv).bottom, list.bounds.height, accuracy: 0.5)
    }

    func testScrollTopIsKeptWhenTheBandGoes() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, _) = try fixture()
        let p = session.presenter, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        sv.contentOffset.y = 100 - list.scrollTopInset(sv)
        p.press(try node(session, "head").id)
        until("a line above the scroller takes it off the top edge") { list.pageBand.top == 0 && (try? node(session, "header")) != nil }
        XCTAssertGreaterThan(list.pageBand.bottom, 0, "it keeps the bottom band")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 100, accuracy: 0.5, "the logical scrollTop is kept")
        p.press(try node(session, "head").id)
        until("the band comes back") { list.pageBand.top > 0 }
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 100, accuracy: 0.5, "and kept again")
    }

    func testAScrollerThatFitsDoesNotScrollByItsBands() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, _) = try fixture()
        let p = session.presenter, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        p.press(try node(session, "shorten").id)
        until("the content is short") { (try? node(session, "filler"))?.bounds.height == 40 }
        let i = sv.adjustedContentInset
        XCTAssertLessThanOrEqual(sv.contentSize.height + i.top + i.bottom, sv.bounds.height + 0.5, "no range from the bands, as a table that fits")
    }
}
#endif
