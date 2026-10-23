#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `scripts/fixtures/sticky-bar.contract`: a route's scroller under a large
/// title whose first child is `position: sticky; top: 114px`, then cards. CSS
/// measures `top` from the scrollport's top edge, which under a UIKit bar is
/// the bar's bottom, as under a web page's header in flow. So at rest the
/// picker sits 114 points below the expanded bar, over the first card, as
/// Chrome draws it; scrolled, it pins 114 points below the collapsed bar.
/// Measured from the screen's top (UIKit's raw `contentOffset`), it would
/// stay in flow at rest and pin just under the bar.
final class StickyBarIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_STICKY_BAR_PLAN"], "build.mjs --test --ios compiles the sticky-bar fixture")
        let session = ExactApp.shared.makeSession(label: "sticky-bar")
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
        let agent = Agent(session: session), deadline = Date().addingTimeInterval(5)
        func ready() -> Bool {
            let top = session.presenter.navigation.primaryNavigation?.topViewController as? RouteController
            return top?.node.props["testId"] == "route" && !agent.nativeInFlight()
        }
        while !ready() && Date() < deadline {
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        XCTAssertTrue(ready(), "the route is shown and settled")
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }, "no \(testId)")
    }

    private func frame(_ view: UIView) -> CGRect { view.convert(view.bounds, to: nil) }

    /// The picker, the first card and the scrollport, in the window, once
    /// the scroller is at CSS `scrollTop` `y`.
    private func scrolled(to y: CGFloat) throws -> (picker: CGRect, card: CGRect, port: CGRect, bar: CGRect) {
        let session = try XCTUnwrap(self.session)
        let list = try node("list")
        let sv = try XCTUnwrap(list.scrollView, "the list is a UIScrollView")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        XCTAssertTrue(session.presenter.navigation.insetByUIKit(list), "UIKit insets the scroller under its bar")
        sv.contentOffset.y = y - list.scrollTopInset(sv)
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), y, accuracy: 0.5, "scrollTop")
        return (frame(try node("picker")), frame(try node("card-1")), sv.convert(list.scrollPort(sv), to: nil), frame(nav.navigationBar))
    }

    func testAtRestAStickyChildIsItsTopBelowTheExpandedBar() throws {
        _ = try fixture()
        let rest = try scrolled(to: 0)
        XCTAssertEqual(rest.port.minY, rest.bar.maxY, accuracy: 0.5, "the scrollport starts at the expanded bar's bottom")
        XCTAssertEqual(rest.picker.minY, rest.port.minY + 114, accuracy: 0.5, "`top: 114px` from the scrollport, as Chrome at scrollTop 0")
        XCTAssertEqual(rest.card.minY, rest.port.minY + rest.picker.height, accuracy: 0.5, "the first card stays in flow after the picker's place")
    }

    func testScrolledAStickyChildPinsItsTopBelowTheCollapsedBar() throws {
        _ = try fixture()
        let rest = try scrolled(to: 0)
        let pinned = try scrolled(to: 400)
        XCTAssertLessThan(pinned.bar.maxY, rest.bar.maxY - 20, "the large title collapsed")
        XCTAssertEqual(pinned.port.minY, pinned.bar.maxY, accuracy: 0.5, "the scrollport starts at the collapsed bar's bottom")
        XCTAssertEqual(pinned.picker.minY, pinned.port.minY + 114, accuracy: 0.5, "pinned `top: 114px` below it")
        XCTAssertEqual(pinned.card.minY, rest.card.minY - 400 - (rest.port.minY - pinned.port.minY), accuracy: 0.5, "the cards scroll on under it")
    }
}
#endif
