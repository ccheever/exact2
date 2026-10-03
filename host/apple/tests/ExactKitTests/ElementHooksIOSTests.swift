#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003.000 Stage 1 over the native fixture: a node marked
/// `hook="badge"` — a leaf box that would otherwise be drawn flat into its
/// parent's layer — is a view, the app module's `element` hook receives it
/// after the batch that mounts it and adds an interaction Exact leaves to
/// the app, hears its `data-*` change, and hears `ended` when the plan
/// reloads, its successor `built`. (A list row's hooked node, which this
/// host has no rows for, is driven by `smoke.mjs ios --app native-fixture`.)
final class ElementHooksIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        NativeViews.uninstallTable()
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { spin(0.02) }
        XCTAssertTrue(done(), what)
    }

    private func node(_ session: ExactSession, _ testId: String) -> NodeView? {
        session.presenter.views.values.first { $0.props["testId"] == testId }
    }

    func testAHookedLeafIsAViewItsHookRunsAtEachMomentAndAReloadEndsIt() throws {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "element")
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
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"]))
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        until("the badge's hook built") { log().contains("hook element badge #") && log().contains(": built") }
        let badge = try XCTUnwrap(node(session, "hooked-badge"), "a hooked leaf box is a view")
        XCTAssertFalse(session.presenter.flats.isFlat(badge.id), "never a flat leaf")
        XCTAssertTrue(badge.interactions.contains { $0 is UIContextMenuInteraction }, "the hook's interaction, which Exact leaves to the app")
        XCTAssertTrue(log().contains("hook element badge: a view, not a flat leaf; its row is not reused (LLP 1075.003.000)"))
        // `data-tone` follows `composed`: its change reaches the hook.
        let compose = try XCTUnwrap(node(session, "compose-home"))
        session.presenter.press(compose.id)
        until("the badge's hook heard its data change") { log().contains("hook element badge #\(badge.id): changed") }
        let hooks = try XCTUnwrap(session.presenter.elements.observation["badge"] as? [String: Any])
        XCTAssertEqual((hooks["calls"] as? [String: Int])?["changed"], 1)
        XCTAssertEqual(hooks["live"] as? Int, 1)
        // The plan boots again in this session: the node ends, a new one is built.
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        until("the old badge ended") { log().contains("hook element badge #\(badge.id): ended") }
        until("the new badge is built") { self.node(session, "hooked-badge").map { $0 !== badge && !$0.interactions.isEmpty } == true }
    }
}
#endif
