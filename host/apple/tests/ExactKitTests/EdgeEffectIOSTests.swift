#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1141.001 D4 over `scripts/fixtures/edge-effect.contract`: content
/// scrolled under the bar takes iOS 26's scroll edge effect, as a hand-built
/// screen's does, when the scroller's rows paint above their in-flow
/// siblings. UIKit hangs the effect in a view after the content; a row ranked
/// above depth 0 (one holding a native button, a `position: relative` card,
/// a sticky child with a `z-index`) drew over it, and ran sharp under the
/// title. Under a large title and an inline one, each scroller with pull to
/// refresh.
final class EdgeEffectIOSTests: XCTestCase {
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
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_EDGE_EFFECT_PLAN"], "build.mjs --test --ios compiles the edge-effect fixture")
        let session = ExactApp.shared.makeSession(label: "edge-effect")
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
        try waitForRoute("large")
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }, "no \(testId)")
    }

    private func waitForRoute(_ name: String) throws {
        let session = try XCTUnwrap(session), agent = Agent(session: session)
        let deadline = Date().addingTimeInterval(5)
        func ready() -> Bool {
            let top = session.presenter.navigation.primaryNavigation?.topViewController as? RouteController
            return top?.node.props["testId"] == "route-\(name)" && !agent.nativeInFlight()
        }
        while !ready() && Date() < deadline {
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        XCTAssertTrue(ready(), "route \(name) is shown and settled")
    }

    /// The route's scroller, scrolled under its bar, is the one the bar's
    /// top edge follows, and UIKit's views after its content (the edge
    /// effect's) draw over every row: at a depth no row is above, and later
    /// in `subviews`, which orders equal depths.
    private func assertEffectOverContent(_ name: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let session = try XCTUnwrap(self.session)
        let route = try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController as? RouteController)
        let sv = try XCTUnwrap(session.presenter.navigation.shownScroll(try node("content-\(name)")), file: file, line: line)
        XCTAssertNotNil(sv.refreshControl, "the scroller pulls to refresh", file: file, line: line)
        sv.contentOffset.y = -sv.adjustedContentInset.top + 400
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertTrue(route.contentScrollView(for: .top) === sv, "the bar's top edge follows the scroller", file: file, line: line)

        let rows = sv.subviews.filter { $0 is NodeView }
        let content = rows.map(\.layer.zPosition).max() ?? 0
        XCTAssertGreaterThan(content, 0, "the fixture ranks its rows above depth 0", file: file, line: line)
        let last = try XCTUnwrap(sv.subviews.lastIndex { $0 is NodeView })
        let platform = sv.subviews[(last + 1)...]
        if #available(iOS 26.0, *) {
            XCTAssertFalse(platform.isEmpty, "UIKit's edge effect hangs after the content", file: file, line: line)
        }
        for view in platform {
            XCTAssertGreaterThanOrEqual(view.layer.zPosition, content, "\(type(of: view)) draws over the rows", file: file, line: line)
        }
    }

    func testTheEdgeEffectDrawsOverRowsUnderALargeTitle() throws {
        _ = try fixture()
        try assertEffectOverContent("large")
    }

    func testTheEdgeEffectDrawsOverRowsUnderAnInlineTitle() throws {
        let session = try fixture()
        let reply = Agent(session: session).tap(["id": Int(try node("tab-inline").id)])
        XCTAssertNil(reply["error"], "\(reply)")
        try waitForRoute("inline")
        try assertEffectOverContent("inline")
    }
}
#endif
