import XCTest
@testable import ExactKit

/// Platform timing's first `clock` (LLP 1080.000 §12): the host ran on the
/// wall's time until then, so the seek starts there and never goes back.
final class AgentClockTests: XCTestCase {
    func testTheFirstClockUnderPlatformTimingStartsAtTheWall() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = ExactApp.shared.makeSession(label: "agent-clock")
        defer { session.destroy() }
        XCTAssertNil(session.clock, "no clock op yet: the host reads the wall")
        Thread.sleep(forTimeInterval: 0.05)
        let wall = session.now()
        let agent = Agent(session: session)
        let first = agent.clock(["to": 100.0])
        let landed = try XCTUnwrap(first["clock"] as? Double, "\(first)")
        XCTAssertGreaterThanOrEqual(landed, wall + 100, "the driver's +100 from the runner's 0 lands 100 past the wall")
        XCTAssertEqual(session.clock, landed)
        // Later seeks are the clock's own: no shift, and still never back.
        let second = agent.clock(["to": landed + 50])
        XCTAssertEqual(second["clock"] as? Double, landed + 50)
        XCTAssertNotNil(agent.clock(["to": landed])["error"], "the clock cannot go backwards")
    }
}
