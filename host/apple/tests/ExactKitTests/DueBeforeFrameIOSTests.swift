#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A deadline already due when a batch lands advances before the turn's
/// frame is committed. A `then` is armed at its answer's time (LLP 1016.001
/// D3), so the batch that lands the answer carries a deadline already due,
/// and its advance waited for the next display frame: that frame showed the
/// answer without what its `then` does (Lexy's sign-in button read "Sign In"
/// for a frame between two requests). Here a timer's deadline passes with no
/// turn of the run loop, and then a batch lands, as the answer's does: its
/// advance runs before control goes back to UIKit, which commits the frame.
final class DueBeforeFrameIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    /// The fixture booted, its timer armed by a press and its deadline passed
    /// with no turn of the run loop; its state text.
    private func armedAndDue() throws -> (ExactSession, () -> String, Double) {
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
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let views = session.presenter.views.values
        let arm = try XCTUnwrap(views.first { $0.props["testId"] == "arm" })
        let state = try XCTUnwrap(views.first { $0.props["testId"] == "state" })
        let shown = { state.accessibleText }
        XCTAssertEqual(shown(), "Waiting")
        _ = Agent(session: session).tap(["id": Int(arm.id)])
        let due = try XCTUnwrap(session.timerDue, "the press armed the timer")
        while session.now() < due { usleep(100) }
        XCTAssertEqual(shown(), "Waiting", "nothing has advanced yet")
        return (session, shown, due)
    }

    func testADeadlineAlreadyDueAdvancesBeforeControlGoesBackToUIKit() throws {
        let (session, shown, due) = try armedAndDue()
        // A batch lands with its deadline already due, as an answer's lands
        // with its `then` due
        session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due))
        XCTAssertEqual(shown(), "Flipped", "the deadline advanced in the same main-thread work, before UIKit commits")
    }

    func testABatchWithChangesAdvancesItsDueDeadlineAfterApplyingThem() throws {
        let (session, shown, due) = try armedAndDue()
        let state = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "state" })
        // The batch's own change applies first, then the deadline's commit
        var batch = wireBatch([["op": "props", "id": Int(state.id), "set": ["accessibilityHint": "landed"]]])
        batch.timerDueMs = due
        session.apply(batch)
        XCTAssertEqual(state.props["accessibilityHint"], "landed", "the batch applied")
        XCTAssertEqual(shown(), "Flipped", "and its due deadline advanced before control went back to UIKit")
    }

    func testADriverOwnedClockAdvancesOnlyWhenTheDriverSaysSo() throws {
        let (session, shown, due) = try armedAndDue()
        session.clock = session.now()
        session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due))
        XCTAssertEqual(shown(), "Waiting", "the driver's clock moves time, not the batch")
    }
}
#endif
