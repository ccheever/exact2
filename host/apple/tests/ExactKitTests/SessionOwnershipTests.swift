#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SessionOwnershipTests: XCTestCase {
    private func fixture() throws -> ExactSession {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "component App\n  view\n    text \"Ownership fixture\"\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let compiler = Process()
        compiler.executableURL = root.appendingPathComponent("target/debug/contract")
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let s = ExactApp.shared.makeSession(label: "ownership-test")
        s.canvases.loadRequested = true
        XCTAssertNil(s.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 100)).error)
        return s
    }

    private func request(_ s: ExactSession, _ q: [String: Any]) throws -> [String: Any] {
        let pipe = Pipe()
        let previous = Agent.out
        Agent.out = pipe.fileHandleForWriting
        defer { Agent.out = previous }
        s.agentInstance.handle(op: q["op"] as! String, q, line: String(decoding: try JSONSerialization.data(withJSONObject: q), as: UTF8.self))
        try pipe.fileHandleForWriting.close()
        return try JSONSerialization.jsonObject(with: pipe.fileHandleForReading.readDataToEndOfFile()) as! [String: Any]
    }

    func testObservationAndRefusedSeekNeverAcquireOrChangeStoredSave() throws {
        let s = try fixture(); defer { s.destroy() }
        let store = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: store) }
        let saved = Data("opaque player checkpoint".utf8)
        try SurfaceCheckpointStore.write(saved, app: "fixture", surface: "world", under: store)
        let before = s.agent("{\"op\":\"state\"}")
        let response = try request(s, ["op": "state"])
        XCTAssertEqual((response["ownership"] as? [String: Any])?["owner"] as? String, "human")
        XCTAssertNil(s.clock)
        XCTAssertNotNil(try request(s, ["op": "tap", "id": 999])["error"], "a detached driver cannot mutate human-owned input")
        XCTAssertEqual(s.agent("{\"op\":\"state\"}"), before)
        XCTAssertNotNil(s.agentInstance.clock(["to": 100])["error"])
        XCTAssertNil(s.clock)
        _ = s.agentInstance.clock(["owner": "agent"])
        _ = s.agentInstance.clock(["owner": "human"])
        XCTAssertEqual(try SurfaceCheckpointStore.read(app: "fixture", surface: "world", under: store), saved)
        XCTAssertEqual(s.agent("{\"op\":\"state\"}"), before)
    }

    func testPauseAndRestoredControlledEpochResumeWithoutWallCatchup() throws {
        let s = try fixture(); defer { s.destroy() }
        var wall = 1_000.0
        s.wallTime = { wall }
        _ = s.agentInstance.clock(["owner": "agent"])
        XCTAssertEqual(s.clock, 1_000)
        wall = 90_000
        XCTAssertEqual(s.now(), 1_000)
        // A controlled runner can be ahead of or behind the process's wall time.
        s.clock = 120_000
        _ = s.agentInstance.clock(["owner": "human"])
        XCTAssertNil(s.clock)
        XCTAssertEqual(s.now(), 120_000)
        wall += 16
        XCTAssertEqual(s.now(), 120_016)
        XCTAssertEqual(s.time(atWall: wall + 8), 120_024, "display-link target shares the rebased epoch")
        _ = s.agentInstance.clock(["owner": "agent"])
        wall += 300_000
        _ = s.agentInstance.clock(["owner": "human"])
        XCTAssertEqual(s.now(), 120_016)
    }

    func testDetachAcknowledgesAllSessionsAndResumesTimersFramesAndPacing() throws {
        let a = try fixture(), b = try fixture()
        let oldRoutes = Agent.routes, oldPolicy = Agent.keepAliveOnEOF
        defer { Agent.routes = oldRoutes; Agent.keepAliveOnEOF = oldPolicy; a.destroy(); b.destroy() }
        Agent.routes = [("a", a), ("b", b)]; Agent.keepAliveOnEOF = false
        a.clock = 10; b.clock = 20
        a.apply(Batch(ops: [], timers: true, motion: true, clock: nil, error: nil))
        XCTAssertNil(a.clockTimer); XCTAssertNil(a.frames.link)
        let r = try request(a, ["op": "clock", "world": true, "id": 999, "owner": "human", "detach": true])
        XCTAssertEqual(r["detached"] as? Bool, true)
        XCTAssertNil(a.clock); XCTAssertNil(b.clock)
        XCTAssertTrue(a.clockTimer?.isValid == true)
        XCTAssertNotNil(a.frames.link)
        XCTAssertFalse(a.freezesAnimations)
        XCTAssertTrue(Agent.disconnected())
        let now = a.now()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.03))
        XCTAssertGreaterThan(a.now(), now + 10)
        Agent.keepAliveOnEOF = false
        a.clock = a.now()
        XCTAssertFalse(Agent.disconnected(), "ordinary EOF retains isolated process shutdown")
        XCTAssertNil(a.clock, "even the shutdown path releases controlled ownership")
    }

    func testHeldKeysAndContactAreCancelledWithoutActivatingSave() throws {
        let s = try fixture(); defer { s.destroy() }
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        w.contentView = s.presenter.viewport
        let save = NodeView(id: 100, kind: "button", presenter: s.presenter)
        save.frame = NSRect(x: 0, y: 0, width: 100, height: 40); save.handlers = ["press"]
        s.presenter.root.addSubview(save); s.presenter.views[100] = save
        var saves = 0, releases = 0
        s.presenter.onPress = { _ in saves += 1 }
        let agent = s.agentInstance
        s.clock = 0
        agent.keyReleases["held-W"] = { releases += 1; return ["phase": "up"] }
        _ = agent.contact("down", ["id": 100])
        // The actual NodeView mouse-up path must see its press latch cancelled.
        save.pressed = true
        _ = agent.handoff(owner: "human")
        XCTAssertEqual(releases, 1)
        XCTAssertTrue(agent.keyReleases.isEmpty)
        XCTAssertNil(agent.contact); XCTAssertNil(agent.canvasContact)
        XCTAssertFalse(save.pressed); XCTAssertEqual(saves, 0)
        _ = agent.handoff(owner: "agent")
        XCTAssertEqual(releases, 1, "a stale driver cannot release the next owner's key")
        withExtendedLifetime(w) {}
    }
}
#endif
