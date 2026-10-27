#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A restart (a dev loop's generation reloading a running app, `presentCommitted`) over a route whose native bar covers
/// a virtualized list: the presenter's reset clears the old tree's covers, and that report must not reach the runtime
/// that replaces it before its committed batch lands. It did: the new runtime took the covers, advanced, and a
/// collection flush mounted rows that the committed batch, applied after, then dropped from the list while the runtime
/// kept them: rows that had their place and no view (the Apple pass's blank feed rows, on every dev-joined launch).
/// On a real clock and a real window, as the launch ran; the fixture's always-due timer gives the new runtime an advance.
final class RestartListIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The row ids the runner's tree mounts in the list, and those of them the presenter shows: a view in the window.
    private func rows(_ session: ExactSession) throws -> (mounted: Set<String>, shown: Set<String>) {
        let data = Data(session.agent(#"{"op":"tree"}"#).utf8)
        let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        let mounted = Set(nodes.compactMap { (($0["props"] as? [String: Any])?["testId"] as? String) ?? $0["testId"] as? String }.filter { $0.hasPrefix("row-") })
        let shown = Set(session.presenter.views.values.filter { $0.window != nil }.compactMap { $0.props["testId"] as? String }.filter { $0.hasPrefix("row-") })
        return (mounted, shown)
    }

    func testARestartOverACoveredListShowsEveryRowTheNewTreeMounts() throws {
        let plan = try fixturePlan("RESTART_LIST")
        let session = ExactApp.shared.makeSession(label: "restart-list")
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
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the list mounts its rows") { ((try? rows(session))?.mounted.count ?? 0) > 4 }
        spin(0.5)
        let before = try rows(session)
        XCTAssertGreaterThan(before.mounted.count, 4, "the list mounts the rows its port shows")
        XCTAssertEqual(before.shown, before.mounted, "before the restart, every mounted row is shown")
        XCTAssertFalse(session.presenter.navigation.covers.isEmpty, "the route's bar covers the list: the reset has covers to clear")
        // What the reset reports (the old tree's covers cleared) reaches no runtime while the views go: the replacing
        // runtime applies nothing before its committed batch.
        // (The clearing report is the one whose covers are all gone; counted whatever the presenter's flag says.)
        var clears = 0, clearsInReset = 0, appliedInClears = 0, coversAfter = 0
        let reported = session.presenter.onCovers
        defer { session.presenter.onCovers = reported }
        session.presenter.onCovers = { [weak session] covers, owners in
            guard let session else { return }
            let before = session.appliedBatches
            reported?(covers, owners)
            if !session.runtime.held, !session.presenter.resetting, covers.contains(where: { $0.1 != nil }) { coversAfter += 1 }
            guard !covers.isEmpty, covers.allSatisfy({ $0.1 == nil }) else { return }
            clears += 1
            if session.presenter.resetting { clearsInReset += 1 }
            appliedInClears += session.appliedBatches - before
        }
        // The restart, as a generation's reload makes it; then the real clock runs, as after a launch.
        var seen: [ResetGate.Seen] = []
        ResetGate.spy = { seen.append($0) }
        defer { ResetGate.spy = nil }
        XCTAssertTrue(session.apply(plan, label: "generation"))
        // The spy: nothing reached the runtime over the reset; the covers it cleared were held, every held call is
        // listed, and the replay ran once, in order, after the committed batch.
        guard case let .reset(entered, held)? = seen.first else { return XCTFail("no reset seen: \(seen)") }
        XCTAssertEqual(entered, 0, "a call reached the runtime inside the reset")
        XCTAssertTrue(held.contains("covers(_:owners:)"), "\(held)")
        let listed = Set(ResetGate.replayed.keys).union(ResetGate.dropped.keys)
        XCTAssertEqual(held.filter { !listed.contains($0) }, [], "a held call ResetGate doesn't list")
        XCTAssertEqual(seen.dropFirst().map { if case let .replayed(r, _) = $0 { r } else { [] } }, [ResetGate.replays(held)], "replayed once, after")
        XCTAssertGreaterThan(clears, 0, "the reset cleared the old tree's covers")
        XCTAssertEqual(clearsInReset, clears, "and said so inside the reset")
        XCTAssertEqual(appliedInClears, 0, "the old tree's covers went to the new runtime, and its batch applied, inside the reset")
        until("the new tree mounts its rows") { ((try? rows(session))?.mounted.count ?? 0) > 4 }
        spin(1.0)
        let after = try rows(session)
        XCTAssertGreaterThan(after.mounted.count, 4)
        XCTAssertEqual(after.mounted.subtracting(after.shown), [], "a row the new tree mounts with no view: its place blank")
        XCTAssertGreaterThan(coversAfter, 0, "the new tree's bar covers reached the new runtime")
    }
}
#endif
