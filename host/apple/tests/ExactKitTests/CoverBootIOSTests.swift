#if os(iOS)
import UIKit
import CExact
import XCTest
@testable import ExactKit

final class CoverBootIOSTests: XCTestCase {
    func testTheCoverRootUsesTheWholeViewInItsFirstSettlement() throws {
        let app = ExactApp.shared
        let plan = try fixturePlan("COVER_SCROLL")
        let previous = app.lastPlan.map { ExactGeneration(plan: $0, assets: app.resolver, token: app.selectedToken, module: app.lastModule) }
        app.installInitial(ExactGeneration(plan: plan, assets: app.resolver))
        let session = app.makeSession(label: "cover-boot")
        defer {
            session.destroy()
            if let previous { app.installInitial(previous) }
            else {
                app.installInitial(ExactGeneration(plan: plan, assets: app.resolver, token: 855))
                XCTAssertTrue(app.fallBackFromInitial(reason: "cover boot test finished"))
            }
        }
        let view = ExactView(session: session), host = UIViewController()
        view.fit()
        XCTAssertFalse(session.booted)
        XCTAssertNil(session.launchViewport, "a zero frame must not decode a launch policy")
        host.additionalSafeAreaInsets = UIEdgeInsets(top: 24, left: 12, bottom: 16, right: 18)
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
        XCTAssertEqual(session.launchViewport?.plan, plan)
        XCTAssertEqual(session.launchViewport?.cover, true)
        let env = try XCTUnwrap(Agent(session: session).layout()["env"] as? [String: Any])
        XCTAssertEqual(try XCTUnwrap(env["safe-area-inset-bottom"] as? Double), Double(view.safeAreaInsets.bottom), accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(env["safe-area-inset-left"] as? Double), Double(view.safeAreaInsets.left), accuracy: 0.01)
        let navigation = session.presenter.navigation
        let route = try XCTUnwrap(navigation.activeRoute)
        route.navigationController?.view.layoutIfNeeded()
        route.view.layoutIfNeeded()
        navigation.reportCovers()
        XCTAssertGreaterThan(route.view.safeAreaInsets.bottom, 0)
        XCTAssertEqual(route.bottomClearance, route.view.safeAreaInsets.bottom, accuracy: 0.5,
                       "under `cover` too the route clears its own bottom safe area, whatever `env()` holds (LLP 1123 D3)")
    }

    func testARefusedGenerationUsesTheBakesOwnViewportPolicy() throws {
        let app = ExactApp.shared
        let probe = app.makeSession(label: "baked-policy")
        let bakedCover = probe.runtime.on(busy: false) { exact_plan_covers_viewport(probe.runtime.rt, 0) != 0 }
        probe.destroy()
        let plan = try fixturePlan(bakedCover ? "FIELDLESS" : "COVER_SCROLL")
        let previous = app.lastPlan.map { ExactGeneration(plan: $0, assets: app.resolver, token: app.selectedToken, module: app.lastModule) }
        // This unregistered composition token refuses preparation before commit.
        app.installInitial(ExactGeneration(plan: plan, assets: app.resolver, token: 855))
        let session = app.makeSession(label: "cover-fallback")
        defer {
            session.destroy()
            if let previous { app.installInitial(previous) }
        }
        let view = ExactView(session: session), host = UIViewController()
        host.additionalSafeAreaInsets = UIEdgeInsets(top: 24, left: 12, bottom: 16, right: 18)
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
            guard app.selectedToken == 0, initial == nil,
                  let root = batch.ops.first(where: { $0.op == .roots })?.ids.first,
                  let rect = batch.ops.first(where: { $0.op == .frame && $0.id == root }) else { return }
            initial = CGSize(width: rect.w, height: rect.h)
        }
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        XCTAssertTrue(session.booted, session.bootError ?? "the embedded bake boots after refusal")
        XCTAssertEqual(app.selectedToken, 0)
        XCTAssertNil(app.lastPlan)
        XCTAssertNil(try XCTUnwrap(session.launchViewport).plan, "fallback must replace the rejected policy cache")
        XCTAssertEqual(session.launchViewport?.cover, bakedCover)
        XCTAssertEqual(session.presenter.viewportFit == "cover", bakedCover)
        XCTAssertNotEqual(view.bounds.size, view.bounds.inset(by: view.safeAreaInsets).size)
        let expected = bakedCover ? view.bounds.size : view.bounds.inset(by: view.safeAreaInsets).size
        XCTAssertEqual(try XCTUnwrap(initial), expected, "the rejected plan must not choose the bake's first viewport")
        XCTAssertEqual(session.presenter.insets, bakedCover ? view.safeAreaInsets : .zero)
    }

}
#endif
