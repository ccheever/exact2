#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A sheet over a sheet (LLP 1075.003 §9, LLP 1035.003 D5): the native
/// fixture's menu pushed over its Sheet route waits for Sheet's
/// presentation, and the agent's wait after an input waits with it.
final class ModalStackIOSTests: XCTestCase {
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

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    /// The native fixture booted in a window of the test host's scene: UIKit
    /// presents a sheet only from a window a scene holds.
    private func fixture(_ label: String) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func press(_ session: ExactSession, _ testId: String) throws {
        session.presenter.press(try node(session, testId).id)
    }

    /// The controllers presented over the fixture, bottom first.
    private func presented() -> [UIViewController] {
        Array(sequence(first: host?.presentedViewController, next: { $0?.presentedViewController }).compactMap { $0 })
    }

    /// Nothing presenting: the agent's wait after an input returns at once.
    func testTheWaitReturnsAtOnceWithNoSheetMoving() throws {
        let session = try fixture("sheet-wait-idle")
        let started = Date()
        XCTAssertTrue(session.agentInstance.awaitModalTransitions())
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.1)
    }

    /// The menu pushed over Sheet while Sheet is still being presented (a
    /// drive's taps a few milliseconds apart, the Bluesky clone's prompt over
    /// its muted words sheet) waits: UIKit presents from the topmost
    /// controller only once it has finished presenting, so the second route
    /// is in the tree and not yet on screen, and the agent's wait after the
    /// input covers it. This window ends no presentation (see
    /// ModalDetentIOSTests), so the wait ends at its bound, saying so; the
    /// sheets presented in order, and closed, are driven by
    /// `scripts/smoke-native.mjs` on a simulator.
    func testASheetOverAPresentingSheetWaitsAndTheAgentWaitsWithIt() throws {
        let session = try fixture("sheet-over-presenting-sheet")
        let modals = session.presenter.modals
        try press(session, "sheet")
        XCTAssertEqual(presented().count, 1, "Sheet is being presented")
        XCTAssertTrue(modals.inTransition)
        try press(session, "sheet-menu")
        XCTAssertNotNil(try node(session, "menu-close"), "the menu's route is in the tree")
        XCTAssertEqual(presented().count, 1, "the menu waits while Sheet presents")
        XCTAssertEqual(modals.routes.map { $0.node.props["testId"] }, ["route-sheet"])
        let started = Date()
        XCTAssertFalse(session.agentInstance.awaitModalTransitions(bound: 0.3), "the wait covers Sheet still presenting")
        XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(started), 0.3)
    }
}
#endif
