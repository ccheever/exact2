#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CoverBootIOSTests: XCTestCase {
    func testTheCoverRootUsesTheWholeViewInItsFirstSettlement() throws {
        let app = ExactApp.shared
        app.installInitial(ExactGeneration(plan: try fixturePlan("COVER_SCROLL"), assets: app.resolver))
        let session = app.makeSession(label: "cover-boot")
        defer { session.destroy() }
        let view = ExactView(session: session), host = UIViewController()
        host.additionalSafeAreaInsets = UIEdgeInsets(top: 24, left: 0, bottom: 16, right: 0)
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let frame = scene?.screen.bounds ?? CGRect(x: 0, y: 0, width: 402, height: 874)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: frame)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        host.view.addSubview(view)
        var initial: CGSize?
        session.runtime.observeBatch = { _, batch in
            guard initial == nil, let root = batch.ops.first(where: { $0.op == .roots })?.ids.first,
                  let rect = batch.ops.first(where: { $0.op == .frame && $0.id == root }) else { return }
            initial = CGSize(width: rect.w, height: rect.h)
        }
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        XCTAssertTrue(session.booted, session.bootError ?? "the view boots at its first layout")
        XCTAssertGreaterThan(view.safeAreaInsets.top + view.safeAreaInsets.bottom, 0)
        XCTAssertEqual(try XCTUnwrap(initial), view.bounds.size, "viewport-dependent resources must never see a safe-area-sized cover root")
        XCTAssertEqual(session.viewportSize, view.bounds.size)
    }
}
#endif
