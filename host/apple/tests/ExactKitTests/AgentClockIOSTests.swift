#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// Platform timing's first `clock` (LLP 1080.000 §12): the host ran on the
/// wall's time until then, motion included, so the clock is taken over at the
/// wall and never set behind it; a hold begun after it is not refused.
final class AgentClockIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func booted() throws -> ExactSession {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "agent-clock")
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
        return session
    }

    /// The motion engine at the wall, as a running animation leaves it, then a hold.
    private func holdAfter(_ session: ExactSession, _ op: [String: Any]) throws -> (wall: Double, reply: [String: Any], held: Bool) {
        session.apply(session.runtime.tick(now: session.now()))
        let wall = session.now()
        let reply = Agent(session: session).clock(op)
        let node = try XCTUnwrap(session.presenter.views.keys.min())
        let (hold, batch) = session.runtime.holdBegin(node, property: 0, now: session.now())
        session.apply(batch)
        return (wall, reply, hold != nil && batch.error == nil)
    }

    func testTakingTheClockOverStartsAtTheWallAndHoldsStillBegin() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        XCTAssertNil(session.clock, "no clock op yet: the host reads the wall")
        let r = try holdAfter(session, ["take": true])
        XCTAssertNil(r.reply["error"], "\(r.reply)")
        let landed = try XCTUnwrap(r.reply["clock"] as? Double)
        XCTAssertGreaterThanOrEqual(landed, r.wall, "taken over at the wall, not the runner's 0")
        XCTAssertGreaterThanOrEqual(session.now(), landed)
        XCTAssertTrue(r.held, "a hold after the takeover is not refused")
        // Taken: a later take is a no-op seek, and a target behind the clock is refused.
        XCTAssertEqual(Agent(session: session).clock(["take": true])["clock"] as? Double, session.clock)
        XCTAssertNotNil(Agent(session: session).clock(["to": 100.0])["error"], "the clock cannot go backwards")
        // While the runner catches up its stops never set the host's clock behind the floor.
        let floor = try XCTUnwrap(session.clock) + 1000
        _ = Agent(session: session).advanceStepped(to: floor - 1000, deadline: Date().addingTimeInterval(1), floor: floor)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(session.clock), floor)
    }

    func testARunnerAheadOfTheWallSetsTheFloor() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        let agent = Agent(session: session)
        let wall = try XCTUnwrap(agent.clock(["take": true])["clock"] as? Double)
        let ahead = try XCTUnwrap(agent.clock(["to": wall + 60000])["clock"] as? Double)
        session.clock = nil // by hand: no known path leaves the runner ahead with the host's clock unset
        let taken = try XCTUnwrap(agent.clock(["take": true])["clock"] as? Double)
        XCTAssertGreaterThanOrEqual(taken, ahead, "the runner's clock, ahead of the wall, is the floor")
    }

    func testAFirstTargetBehindTheWallNeverSetsTheClockBack() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        let r = try holdAfter(session, ["to": 100.0])
        XCTAssertGreaterThanOrEqual(session.clock ?? 0, r.wall, "the clock stays at the wall")
        XCTAssertTrue(r.held, "a hold after it is not refused (before §12: ClockWentBackwards)")
    }
}
#endif
