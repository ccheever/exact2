#if os(macOS)
import XCTest
@testable import ExactKit

/// The wall clock owns time until the agent's takes it (the timers RFC, D1):
/// on it a repeat that missed beats fires once; on the agent's, every beat.
final class LiveClockMacTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "component App\n  state ticks = 0\n  action tick\n    ticks = ticks + 1\n  task ticker mount\n    every(1000, tick)\n  view\n    text \"x\"\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func ticks(_ session: ExactSession) throws -> Double? {
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
        return (state["slots"] as? [String: Any])?["ticks"] as? Double
    }

    func testAnIntervalAppCoalescesOnTheWallClockAndFiresEveryBeatOnTheAgents() throws {
        let session = ExactApp.shared.makeSession(label: "live-clock")
        defer { session.destroy() }
        let boot = session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844))
        XCTAssertNil(boot.error)
        // On the wall clock a task's repeat is Foundation's timer (#1144), whose wake the runtime fires.
        let row = try XCTUnwrap(boot.nativeTimers?.first, "the task's native timer")
        let due = row.due
        // No frame yet, and three beats missed: one fire.
        _ = session.runtime.fireNativeTimer(row, now: due + 2500)
        XCTAssertEqual(try ticks(session), 1)
        // The agent takes the clock, through its own `clock` op (agent.mjs `clock`): each beat fires at its own time.
        let moved = Agent(session: session).clock(["op": "clock", "to": due + 5500])
        XCTAssertNil(moved["error"], "\(moved)")
        XCTAssertNotNil(session.clock, "the agent holds the clock")
        XCTAssertEqual(try ticks(session), 4)
    }
}
#endif
