#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// The layout regions (LLP 1123 D10): the session reports UIKit's regions of
/// the view controller's root view that holds it, from the layout viewport's
/// safe area, and the agent's facts name them.
final class LayoutRegionsIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    /// A session in a plain view inside a controller's root view, laid out.
    private func booted() throws -> (ExactSession, ExactView, UIViewController) {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "layout-regions")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return (session, view, host)
    }

    func testTheRegionsAreTheControllersRootViewsFromTheViewport() throws {
        let (session, view, host) = try booted()
        let p = session.presenter
        let regions = try XCTUnwrap(p.regions, "a booted session has the regions")
        XCTAssertEqual(regions.count, 20)
        let root = host.view!
        // The layout viewport, in the root view's coordinates, inside the
        // safe area without `cover`, where `env()` reads zero even when a
        // native container takes the whole view
        let frame = p.viewport.convert(p.viewport.bounds, to: root).intersection(root.bounds.inset(by: root.safeAreaInsets))
        func inset(_ i: UIEdgeInsets) -> [CGFloat] {
            let r = root.bounds.inset(by: i)
            return [r.minY - frame.minY, frame.maxX - r.maxX, frame.maxY - r.maxY, r.minX - frame.minX]
        }
        let safe = inset(root.safeAreaInsets).map { max(0, $0) }
        func expected(_ i: UIEdgeInsets) -> [CGFloat] { inset(i).map { max(0, $0) } }
        guard #available(iOS 26.0, *) else { return }
        func corners(_ h: UIEdgeInsets, _ v: UIEdgeInsets) -> [CGFloat] {
            let a = expected(h), b = expected(v)
            return [b[0], a[1], b[2], a[3]]
        }
        let want = corners(root.edgeInsets(for: .safeArea(cornerAdaptation: .horizontal)), root.edgeInsets(for: .safeArea(cornerAdaptation: .vertical)))
            + expected(root.edgeInsets(for: .margins()))
            + corners(root.edgeInsets(for: .margins(cornerAdaptation: .horizontal)), root.edgeInsets(for: .margins(cornerAdaptation: .vertical)))
            + expected(root.edgeInsets(for: .readableContent()))
            + corners(root.edgeInsets(for: .readableContent(cornerAdaptation: .horizontal)), root.edgeInsets(for: .readableContent(cornerAdaptation: .vertical)))
        XCTAssertEqual(regions, want)
        // The controller's margins (the system minimum), not the session
        // view's own, a plain subview's 8 points
        XCTAssertEqual(regions[4 + 3], max(0, root.edgeInsets(for: .margins()).left - frame.minX))
        XCTAssertNotEqual(root.directionalLayoutMargins.leading, view.directionalLayoutMargins.leading)
        // UIKit's default keeps every region inside the safe area
        for r in 0..<5 { for e in 0..<4 { XCTAssertGreaterThanOrEqual(regions[r * 4 + e], safe[e]) } }
    }

    func testTheAgentNamesEveryRegion() throws {
        let (session, _, _) = try booted()
        let regions = try XCTUnwrap(session.presenter.regions)
        let reply = Agent(session: session).layout()
        let env = try XCTUnwrap(reply["env"] as? [String: Any])
        for (r, prefix) in LayoutRegions.prefixes.enumerated() {
            for (e, edge) in ["top", "right", "bottom", "left"].enumerated() {
                XCTAssertEqual(env[prefix + edge] as? Double, Agent.r2(regions[r * 4 + e]), prefix + edge)
            }
        }
    }
}
#endif
