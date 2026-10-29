#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A deadline already due when a batch lands. A `then` is armed at its
/// answer's time (LLP 1016.001 D3) and advances before control goes back to
/// UIKit, so the frame shows the answer with what its `then` does (Lexy's
/// sign-in button read "Sign In" for a frame between two requests): the
/// runner's `land_then_first` tests pin that. A timer's deadline is posted to
/// the owner instead (LLP 1072.000 §4 B), and its commit lands with a later
/// main-queue turn. Here a timer's deadline passes with no turn of the run
/// loop, and then a batch lands.
final class DueBeforeFrameIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    /// The fixture booted, its timer armed by a press and its deadline passed
    /// with no turn of the run loop; its state text.
    private func armedAndDue(driverOwned: Bool = false) throws -> (ExactSession, () -> String, Double) {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_DUE_NOW_PLAN"], "build.mjs --test --ios compiles the due-now fixture")
        let session = ExactApp.shared.makeSession(label: "due-before-frame")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: frame)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: frame.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        window.layoutIfNeeded()
        let views = session.presenter.views.values
        let arm = try XCTUnwrap(views.first { $0.props["testId"] == "arm" })
        let state = try XCTUnwrap(views.first { $0.props["testId"] == "state" })
        let shown = { state.accessibleText }
        XCTAssertEqual(shown(), "Waiting")
        // A slow press batch can outlast the fixture's 50 ms deadline.
        // Arm with time stopped so its same-turn due check cannot fire it.
        session.clock = session.now()
        let reply = Agent(session: session).tap(["id": Int(arm.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, Int(arm.id), "the tap reached Arm: \(reply)")
        let due = try XCTUnwrap(session.timerDue, "the press armed the timer; state: \(shown())")
        XCTAssertEqual(shown(), "Waiting", "the timer has not fired while arming")
        if driverOwned {
            session.clock = due
        } else {
            session.clock = nil
            while session.now() < due { usleep(100) }
        }
        XCTAssertEqual(shown(), "Waiting", "nothing has advanced yet")
        return (session, shown, due)
    }

    func testATimerAlreadyDueIsPostedAndLandsWithALaterTurn() throws {
        let (session, shown, due) = try armedAndDue()
        session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due))
        XCTAssertTrue(session.advanceInFlight, "the timer's advance is posted, main does not wait for it")
        XCTAssertEqual(shown(), "Waiting")
        session.drainFill()
        XCTAssertEqual(shown(), "Flipped", "its commit lands")
    }

    func testABatchWithChangesPostsItsDueDeadlineAfterApplyingThem() throws {
        let (session, shown, due) = try armedAndDue()
        let state = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "state" })
        // The batch's own change applies first, then the deadline's commit
        var batch = wireBatch([["op": "props", "id": Int(state.id), "set": ["accessibilityHint": "landed"]]])
        batch.timerDueMs = due
        session.apply(batch)
        XCTAssertEqual(state.props["accessibilityHint"], "landed", "the batch applied")
        XCTAssertTrue(session.advanceInFlight, "then its due deadline was posted")
        session.drainFill()
        XCTAssertEqual(shown(), "Flipped")
    }

    func testADriverOwnedClockAdvancesOnlyWhenTheDriverSaysSo() throws {
        let (session, shown, due) = try armedAndDue(driverOwned: true)
        session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due))
        XCTAssertEqual(shown(), "Waiting", "the driver's clock moves time, not the batch")
        let reply = Agent(session: session).clock(["to": due])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(shown(), "Flipped", "the driver's advance fires the timer")
    }
}
#endif
