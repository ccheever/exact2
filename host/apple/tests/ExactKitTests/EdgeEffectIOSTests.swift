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
    /// top edge follows. The enclosing effect follows its authored or default
    /// style and draws above the content.
    private func assertEffectOverContent(_ name: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let session = try XCTUnwrap(self.session)
        let route = try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController as? RouteController)
        let sv = try XCTUnwrap(session.presenter.navigation.shownScroll(try node("content-\(name)")), file: file, line: line)
        XCTAssertNotNil(sv.refreshControl, "the scroller pulls to refresh", file: file, line: line)
        sv.contentOffset.y = -sv.adjustedContentInset.top + 400
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertTrue(route.contentScrollView(for: .top) === sv, "the bar's top edge follows the scroller", file: file, line: line)

        let viewport = try XCTUnwrap(route.contentViewport, file: file, line: line)
        XCTAssertTrue(viewport.documentNode === route.node, file: file, line: line)
        if #available(iOS 26.0, *) {
            let style: UIScrollEdgeEffect.Style = name == "large" ? .soft : .automatic
            XCTAssertEqual(sv.topEdgeEffect.style, style, "the content scroller keeps its own effect", file: file, line: line)
            XCTAssertEqual(viewport.topEdgeEffect.style, style, "the enclosing effect follows the content style", file: file, line: line)
            XCTAssertFalse(viewport.topEdgeEffect.isHidden, "UIKit keeps its enclosing effect", file: file, line: line)
        }
        for port in [sv, viewport] {
            let rows = port.subviews.filter { $0 is NodeView }
            let content = rows.map(\.layer.zPosition).max() ?? 0
            XCTAssertGreaterThan(content, 0, "the fixture ranks its content above depth 0", file: file, line: line)
            let last = try XCTUnwrap(port.subviews.lastIndex { $0 is NodeView })
            let platform = port.subviews[(last + 1)...]
            if #available(iOS 26.0, *) {
                XCTAssertFalse(platform.isEmpty, "UIKit's edge effect hangs after the content", file: file, line: line)
            }
            for view in platform {
                XCTAssertGreaterThanOrEqual(view.layer.zPosition, content, "\(type(of: view)) draws over the content in \(type(of: port))", file: file, line: line)
            }
        }
    }

    func testTheEnclosingEffectFollowsContentPolicyAndResetsWithoutIt() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("scroll edge effects require iOS 26") }
        let session = try fixture(), p = session.presenter
        let route = try XCTUnwrap(p.navigation.primaryNavigation?.topViewController as? RouteController)
        let content = try XCTUnwrap(p.navigation.shownScroll(try node("content-large")))
        let viewport = try XCTUnwrap(route.contentViewport)
        let other = UIScrollView()
        other.topEdgeEffect.style = .soft
        other.bottomEdgeEffect.style = .soft
        content.topEdgeEffect.style = .hard
        content.topEdgeEffect.isHidden = true
        content.bottomEdgeEffect.style = .hard
        content.bottomEdgeEffect.isHidden = true
        p.reachPageEdges()
        for edge in [NSDirectionalRectEdge.top, .bottom] {
            let effect = edge == .top ? viewport.topEdgeEffect : viewport.bottomEdgeEffect
            route.setContentScrollView(content, for: edge)
            XCTAssertEqual(effect.style, .hard)
            XCTAssertTrue(effect.isHidden)
            route.setContentScrollView(other, for: edge)
            XCTAssertEqual(effect.style, .soft, "ownership changes forward the policy without a batch")
            XCTAssertFalse(effect.isHidden)
            route.setContentScrollView(nil, for: edge)
            XCTAssertEqual(effect.style, .automatic)
            XCTAssertFalse(effect.isHidden)
            route.setContentScrollView(viewport, for: edge)
            XCTAssertEqual(effect.style, .automatic, "a viewport never inherits its own stale effect")
            XCTAssertFalse(effect.isHidden)
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
